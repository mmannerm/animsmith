//! [`render`] turns [`animsmith_core::MetricGrids`],
//! [`animsmith_core::ResolvedRoles`], and a slice of
//! [`animsmith_core::Finding`] values into a self-contained HTML report.
//! The viewer is driven by the same [`animsmith_core::PoseGrid`] samples
//! the checks judged.
//!
//! The returned HTML is self-contained: CSS, JavaScript, findings, charts,
//! and sampled pose data are embedded in the string. There is no runtime
//! CDN dependency and no JavaScript-side resampling of the clip.

//! See the GitHub [embedding guide] for composing this crate with checks and
//! the [pipeline scenario guide] for CI and outsourced-acceptance reporting
//! workflows.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md

#![warn(missing_docs)]

use animsmith_core::finding::Finding;
use animsmith_core::metrics::MetricGrids;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::PoseGrid;
use base64::Engine as _;
use serde_json::{Value, json};

const VIEWER_JS: &str = include_str!("../assets/viewer.js");
const VIEWER_CSS: &str = include_str!("../assets/viewer.css");

/// Escape untrusted text (clip/bone names, paths from the linted
/// asset) for interpolation into HTML markup and attributes.
fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Render report HTML from shared metric pose grids.
///
/// `clip_filter` restricts the report to one clip name when present. The
/// function performs no filesystem I/O and cannot report write errors;
/// callers choose where to store or serve the returned self-contained HTML
/// string.
pub fn render(
    grids: &MetricGrids<'_>,
    roles: &ResolvedRoles,
    findings: &[Finding],
    clip_filter: Option<&str>,
) -> String {
    let doc = grids.document();
    let bones: Vec<Value> = doc
        .skeleton
        .bones
        .iter()
        .map(|b| json!({ "name": b.name, "parent": b.parent.map(|p| p as i64).unwrap_or(-1) }))
        .collect();

    let trail_roles = [
        (Role::Root, "root"),
        (Role::Hips, "hips"),
        (Role::LeftFoot, "left_foot"),
        (Role::RightFoot, "right_foot"),
    ];

    let mut clips_json: Vec<Value> = Vec::new();
    let mut charts_html = String::new();
    for (clip_index, clip) in doc.clips.iter().enumerate() {
        if clip_filter.is_some_and(|f| f != clip.name) {
            continue;
        }
        let Some(grid) = grids.grid(clip_index) else {
            continue;
        };
        let frames = grid.frame_count();
        let nb = doc.skeleton.bones.len();
        let mut positions = Vec::with_capacity(frames * nb * 3 * 4);
        for f in 0..frames {
            for b in 0..nb {
                let p = grid.model_position(f, b);
                positions.extend_from_slice(&p.x.to_le_bytes());
                positions.extend_from_slice(&p.y.to_le_bytes());
                positions.extend_from_slice(&p.z.to_le_bytes());
            }
        }
        let trails: Value = trail_roles
            .iter()
            .filter_map(|&(role, name)| roles.get(role).map(|id| (name.to_string(), json!(id))))
            .collect::<serde_json::Map<_, _>>()
            .into();
        clips_json.push(json!({
            "name": clip.name,
            "duration": clip.duration_s,
            "frames": frames,
            "positions": base64::engine::general_purpose::STANDARD.encode(&positions),
            "trails": trails,
        }));
        charts_html.push_str(&clip_charts(&clip.name, grid.as_ref(), roles));
    }

    let findings_json: Vec<Value> = findings
        .iter()
        .filter(|f| clip_filter.is_none() || f.clip.as_deref() == clip_filter || f.clip.is_none())
        .map(|f| {
            json!({
                "check": f.check_id,
                "severity": f.severity.to_string(),
                "clip": f.clip,
                "bone": f.bone,
                "time": f.time_s,
                "message": f.message,
            })
        })
        .collect();

    let data = json!({
        "file": doc.source.path,
        "profile": roles.profile,
        "bones": bones,
        "clips": clips_json,
        "findings": findings_json,
    });

    let title = esc(doc
        .source
        .path
        .as_deref()
        .and_then(|p| p.rsplit(['/', '\\']).next())
        .unwrap_or("animsmith report"));
    // A `</script>`-bearing string inside the JSON would terminate the
    // data block early; escaping `<` inside JSON strings is lossless.
    let data = data.to_string().replace('<', "\\u003c");

    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>animsmith — {title}</title>\n<style>{VIEWER_CSS}</style>\n</head>\n<body>\n\
         <header><h1>animsmith report</h1><span id=\"file\"></span></header>\n\
         <main>\n\
         <section id=\"viewer-panel\">\n\
           <div id=\"controls\">\n\
             <select id=\"clip-select\"></select>\n\
             <button id=\"play\">▶</button>\n\
             <input type=\"range\" id=\"scrub\" min=\"0\" value=\"0\" step=\"1\">\n\
             <span id=\"time\"></span>\n\
           </div>\n\
           <canvas id=\"gl\"></canvas>\n\
           <p class=\"hint\">drag to orbit · wheel to zoom · frames shown are exactly the \
           grid the checks judged</p>\n\
         </section>\n\
         <section id=\"side\">\n\
           <h2>Findings</h2>\n<ul id=\"findings\"></ul>\n\
           <h2>Charts</h2>\n<div id=\"charts\">{charts_html}</div>\n\
         </section>\n\
         </main>\n\
         <script type=\"application/json\" id=\"report-data\">{data}</script>\n\
         <script>{VIEWER_JS}</script>\n</body>\n</html>\n"
    )
}

/// SVG metric charts for one clip: gait signal (L/R foot heights and
/// their difference) and the top-down root path. Rust-rendered; a JS
/// playhead line is moved across them in sync with the 3D view.
fn clip_charts(clip_name: &str, grid: &PoseGrid, roles: &ResolvedRoles) -> String {
    let mut out = String::new();
    let frames = grid.frame_count();
    let hips = roles.get(Role::Hips);
    let left = roles.get(Role::LeftFoot);
    let right = roles.get(Role::RightFoot);

    if let (Some(hips), Some(left), Some(right)) = (hips, left, right) {
        let rel_y = |f: usize, b: usize| {
            (grid.model_position(f, b).y - grid.model_position(f, hips).y) as f64
        };
        let l: Vec<f64> = (0..frames).map(|f| rel_y(f, left)).collect();
        let r: Vec<f64> = (0..frames).map(|f| rel_y(f, right)).collect();
        let d: Vec<f64> = l.iter().zip(&r).map(|(a, b)| a - b).collect();
        out.push_str(&line_chart(
            clip_name,
            "gait",
            "foot height rel hips (m) — L blue · R orange · L−R grey",
            &[("#7aa2f7", &l), ("#e0af68", &r), ("#9099b2", &d)],
        ));
    }

    let root = roles.get(Role::Root).or(hips);
    if let Some(root) = root {
        let xs: Vec<f64> = (0..frames)
            .map(|f| grid.model_position(f, root).x as f64)
            .collect();
        let zs: Vec<f64> = (0..frames)
            .map(|f| grid.model_position(f, root).z as f64)
            .collect();
        out.push_str(&path_chart(clip_name, "root path (top-down, m)", &xs, &zs));
    }
    out
}

const W: f64 = 360.0;
const H: f64 = 120.0;
const PAD: f64 = 8.0;

fn line_chart(clip: &str, kind: &str, label: &str, series: &[(&str, &Vec<f64>)]) -> String {
    let clip = &esc(clip);
    let all: Vec<f64> = series.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    if all.is_empty() {
        return String::new();
    }
    let min = all.iter().copied().fold(f64::MAX, f64::min);
    let max = all.iter().copied().fold(f64::MIN, f64::max);
    let span = (max - min).max(1e-6);
    let n = series[0].1.len().max(2);
    let x = |i: usize| PAD + (W - 2.0 * PAD) * i as f64 / (n - 1) as f64;
    let y = |v: f64| H - PAD - (H - 2.0 * PAD) * (v - min) / span;
    let mut paths = String::new();
    for (color, values) in series {
        let d: Vec<String> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(i), y(v)))
            .collect();
        paths.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\"/>",
            d.join("")
        ));
    }
    format!(
        "<figure class=\"chart\" data-clip=\"{clip}\" data-kind=\"{kind}\" data-pad=\"{PAD}\" \
         data-plotw=\"{}\"><figcaption>{clip} — {label}</figcaption>\
         <svg viewBox=\"0 0 {W} {H}\" width=\"100%\">{paths}\
         <line class=\"playhead\" x1=\"{PAD}\" x2=\"{PAD}\" y1=\"0\" y2=\"{H}\"/></svg></figure>",
        W - 2.0 * PAD
    )
}

fn path_chart(clip: &str, label: &str, xs: &[f64], zs: &[f64]) -> String {
    let clip = &esc(clip);
    if xs.is_empty() {
        return String::new();
    }
    let (min_x, max_x) = (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    );
    let (min_z, max_z) = (
        zs.iter().copied().fold(f64::MAX, f64::min),
        zs.iter().copied().fold(f64::MIN, f64::max),
    );
    let span = (max_x - min_x).max(max_z - min_z).max(1e-3);
    let x = |v: f64| PAD + (W - 2.0 * PAD) * (v - min_x) / span;
    let y = |v: f64| H - PAD - (H - 2.0 * PAD) * (v - min_z) / span;
    let d: Vec<String> = xs
        .iter()
        .zip(zs)
        .enumerate()
        .map(|(i, (&px, &pz))| {
            format!(
                "{}{:.1},{:.1}",
                if i == 0 { "M" } else { "L" },
                x(px),
                y(pz)
            )
        })
        .collect();
    format!(
        "<figure class=\"chart\" data-clip=\"{clip}\" data-kind=\"rootpath\">\
         <figcaption>{clip} — {label}</figcaption>\
         <svg viewBox=\"0 0 {W} {H}\" width=\"100%\">\
         <path d=\"{}\" fill=\"none\" stroke=\"#9ece6a\" stroke-width=\"1.5\"/>\
         <circle class=\"pathdot\" r=\"3\" cx=\"{:.1}\" cy=\"{:.1}\"/></svg>\
         <template class=\"pathpoints\">{}</template></figure>",
        d.join(""),
        x(xs[0]),
        y(zs[0]),
        xs.iter()
            .zip(zs)
            .map(|(&px, &pz)| format!("{:.1},{:.1}", x(px), y(pz)))
            .collect::<Vec<_>>()
            .join(";")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track, TrackValues,
        Transform,
    };
    use animsmith_core::profile::Role;
    use animsmith_core::{CheckCtx, Config};

    fn report_document() -> Document {
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.5, 1.0],
                    values: TrackValues::Vec3s(vec![
                        Vec3::ZERO,
                        Vec3::new(1.0, 0.0, 0.0),
                        Vec3::new(2.0, 0.0, 0.0),
                    ]),
                }],
            }],
            source: SourceInfo {
                path: Some("walk.glb".into()),
                format: Some("gltf".into()),
            },
            ..Document::default()
        }
    }

    fn report_data(html: &str) -> Value {
        let marker = r#"<script type="application/json" id="report-data">"#;
        let (_, tail) = html.split_once(marker).expect("report data marker");
        let (raw, _) = tail.split_once("</script>").expect("report data close");
        serde_json::from_str(raw).expect("report data is JSON")
    }

    #[test]
    fn shared_grid_render_embeds_clip_data() {
        let doc = report_document();
        let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
        let config = Config::default();
        let findings = Vec::new();

        let fresh = render(&MetricGrids::new(&doc), &roles, &findings, None);
        let grids = MetricGrids::new(&doc);
        let ctx = CheckCtx::new(&grids, &roles, &config);
        assert!(ctx.grid(0).is_some());
        let shared = render(&grids, &roles, &findings, None);

        assert_eq!(fresh, shared);
        assert!(shared.contains(r#"data-kind="rootpath""#));

        let data = report_data(&shared);
        assert_eq!(data["file"], "walk.glb");
        assert_eq!(data["clips"][0]["name"], "walk");
        assert_eq!(data["clips"][0]["frames"], 3);
        assert_eq!(data["clips"][0]["trails"]["root"], 0);

        let positions = data["clips"][0]["positions"]
            .as_str()
            .expect("encoded positions");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(positions)
            .expect("positions decode");
        assert_eq!(bytes.len(), 3 * 3 * std::mem::size_of::<f32>());
    }
}
