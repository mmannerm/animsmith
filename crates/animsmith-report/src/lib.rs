//! [`render`] turns [`animsmith_core::MetricGrids`],
//! [`animsmith_core::ResolvedRoles`], typed check evaluations, and optional
//! engine-prediction provenance into a self-contained HTML report.
//! The viewer is driven by the same [`animsmith_core::PoseGrid`] samples
//! the checks judged.
//!
//! The returned HTML is self-contained: CSS, JavaScript, findings, charts,
//! and sampled pose data are embedded in the string. There is no runtime
//! CDN dependency and no JavaScript-side resampling of the clip.
//!
//! # Quick start
//!
//! ```no_run
//! fn write_report(
//!     doc: &animsmith_core::Document,
//!     roles: &animsmith_core::ResolvedRoles,
//!     checks: &[animsmith_core::CheckEvaluation],
//! ) -> std::io::Result<()> {
//!     let grids = animsmith_core::MetricGrids::new(doc);
//!     let html = animsmith_report::render(&grids, roles, checks, None, None);
//!     std::fs::write("report.html", html)
//! }
//! ```
//!
//! # Build and API status
//!
//! The library crate has no public feature flags and supports the workspace
//! MSRV, Rust 1.88. Its Rust API is pre-1.0; see `animsmith-core`'s crate-level
//! API status for the shared stability boundary.
//!
//! See the GitHub [embedding guide] for composing this crate with checks and
//! the [pipeline scenario guide] for CI and outsourced-acceptance reporting
//! workflows.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//!
#![warn(missing_docs)]

use animsmith_core::metrics::MetricGrids;
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::PoseGrid;
use animsmith_core::{CheckEvaluation, PredictionProvenanceV1};
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
    checks: &[CheckEvaluation],
    prediction_provenance: Option<&PredictionProvenanceV1>,
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

    let findings_json: Vec<Value> = checks
        .iter()
        .flat_map(CheckEvaluation::findings)
        .filter(|f| clip_filter.is_none() || f.clip.as_deref() == clip_filter || f.clip.is_none())
        .map(|f| {
            json!({
                "check": f.check_id,
                "severity": f.severity.to_string(),
                "clip": f.clip,
                "bone": f.bone,
                "node": f.node,
                "time": f.time_s,
                "message": f.message,
            })
        })
        .collect();

    let predictions_json: Vec<Value> = checks
        .iter()
        .filter_map(|check| {
            check.engine_prediction().map(|prediction| {
                json!({
                    "check_id": check.check_id(),
                    "prediction": prediction,
                })
            })
        })
        .collect();

    let gaps_json: Vec<Value> = checks
        .iter()
        .flat_map(|check| {
            check.gaps().iter().map(|gap| {
                json!({
                    "check_id": check.check_id(),
                    "code": gap.code,
                    "message": gap.message,
                    "scope": gap.scope,
                })
            })
        })
        .collect();

    let data = json!({
        "file": doc.source.path,
        "profile": roles.profile,
        "bones": bones,
        "clips": clips_json,
        "findings": findings_json,
        "gaps": gaps_json,
        "prediction_provenance": prediction_provenance,
        "predictions": predictions_json,
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
           <h2>Coverage gaps</h2>\n<ul id=\"gaps\"></ul>\n\
           <h2>Engine predictions</h2>\n<ul id=\"predictions\"></ul>\n\
           <h2>Charts</h2>\n<div id=\"charts\">{charts_html}</div>\n\
         </section>\n\
         </main>\n\
         <script type=\"application/json\" id=\"report-data\">{data}</script>\n\
         <script>{VIEWER_JS}</script>\n</body>\n</html>\n"
    )
}

/// Render a collection inventory dashboard from its already-validated,
/// versioned machine authority.
///
/// The dashboard deliberately has no asset loader, policy evaluator, or
/// filesystem access. The CLI constructs and writes the authority first; this
/// presentation embeds exactly that authority as escaped JSON, so filtering
/// cannot change its meaning or turn an incomplete collection into a pass.
pub fn render_collection_dashboard(authority_json: &str) -> String {
    // A `</script>`-bearing untrusted string in the authority must not escape
    // the data element. Escaping `<` in JSON strings is lossless.
    let data = authority_json.replace('<', "\\u003c");
    format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>animsmith collection dashboard</title><style>
body{{font:16px system-ui,sans-serif;margin:2rem;max-width:1200px;color:#17202a}}
header,section{{margin-bottom:1.5rem}} label{{margin-right:1rem}}
table{{border-collapse:collapse;width:100%}}th,td{{border:1px solid #ccd;padding:.4rem;text-align:left;vertical-align:top}}
.warning{{background:#fff4d6}} .muted{{color:#57606a}} code{{word-break:break-word}}
</style></head><body>
<header><h1>animsmith collection dashboard</h1><p id="identity"></p><p id="summary" class="muted"></p><p class="warning">This is an inventory of declared evidence, not a quality score or game-ready verdict. Engine loading/playback, retargeting, contacts, visual/artistic quality, and gameplay acceptance remain separate gates.</p></header>
<section><h2>Filters</h2><label>Source <select id="source"><option value="">all</option></select></label><label>Role <select id="role"><option value="">all</option></select></label><label>Runtime set <select id="set"><option value="">all</option></select></label><label>Severity <select id="severity"><option value="">all</option></select></label><label>Outcome <select id="outcome"><option value="">all</option></select></label><label>Availability <select id="availability"><option value="">all</option></select></label><label>Group <select id="group"><option value="source">source</option><option value="roles">role</option><option value="runtime_sets">runtime set</option><option value="severities">severity</option><option value="outcome">outcome</option><option value="availability">availability</option></select></label><p id="groups" class="muted"></p></section>
<section><h2>Logical clips</h2><p id="count" class="muted"></p><table><thead><tr><th>Logical clip</th><th>Physical take</th><th>Roles</th><th>Availability</th><th>Outcome</th><th>Findings / gaps / prediction unavailable</th><th>Runtime sets</th><th>Per-asset report</th></tr></thead><tbody id="clips"></tbody></table></section>
<section><h2>Runtime sets</h2><div id="sets"></div></section><section><h2>Evaluation authority</h2><pre id="evaluation"></pre></section>
<script type="application/json" id="collection-dashboard-data">{data}</script>
<script>
const d=JSON.parse(document.getElementById('collection-dashboard-data').textContent);
const q=id=>document.getElementById(id),esc=s=>String(s??'').replace(/[&<>\"]/g,c=>({{'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}}[c]));
q('identity').textContent=`collection input ${{d.collection_output.sha256}} (${{d.collection_output.bytes}} bytes)`;
q('summary').textContent=`${{d.summary.sources}} sources · ${{d.summary.clips}} clips · ${{d.summary.runtime_sets}} runtime sets · ${{d.summary.findings}} findings · ${{d.summary.coverage_gaps}} gaps · ${{d.summary.prediction_unavailable}} prediction unavailable · ${{d.summary.with_findings}} with findings · ${{d.summary.evaluated}} evaluated · ${{d.summary.partial}} partial · ${{d.summary.excluded}} excluded · ${{d.summary.unavailable}} unavailable · ${{d.summary.not_evaluated}} not evaluated`;
const clips=d.view.clips,sets=d.view.runtime_sets,facet=(item,key)=>{{const value=item[key];return Array.isArray(value)?(value.length?value:['none']):[value||'none']}},values=(items,key)=>[...new Set(items.flatMap(x=>facet(x,key)))].sort();
for(const [id,key,items] of [['source','source',clips],['role','roles',clips],['set','runtime_sets',clips],['severity','severities',clips],['outcome','outcome',clips],['availability','availability',clips]]){{for(const x of values(items,key)){{const o=document.createElement('option');o.value=o.textContent=x;q(id).append(o)}}}}
function selected(id){{return q(id).value}}function matches(c){{return(!selected('source')||c.source===selected('source'))&&(!selected('role')||facet(c,'roles').includes(selected('role')))&&(!selected('set')||facet(c,'runtime_sets').includes(selected('set')))&&(!selected('severity')||facet(c,'severities').includes(selected('severity')))&&(!selected('outcome')||c.outcome===selected('outcome'))&&(!selected('availability')||c.availability===selected('availability'))}}
function draw(){{const shown=clips.filter(matches);q('count').textContent=`showing ${{shown.length}} of ${{clips.length}} declared clips; filters do not change collection completeness`;const group=q('group').value;const counts={{}};for(const c of shown){{for(const key of facet(c,group)){{counts[key]=(counts[key]||0)+1}}}}q('groups').textContent=Object.entries(counts).sort(([a],[b])=>a<b?-1:a>b?1:0).map(([key,count])=>`${{key}}: ${{count}}`).join(' · ')||'no matching declared clips';q('clips').innerHTML=shown.map(c=>`<tr><td><code>${{esc(c.id)}}</code></td><td><code>${{esc(c.source)}} #${{c.take_index}} ${{esc(c.take_name)}}</code></td><td>${{esc(facet(c,'roles').join(', '))}}</td><td>${{esc(c.availability)}}</td><td>${{esc(c.outcome)}}</td><td>${{c.findings}} / ${{c.coverage_gaps}} / ${{c.prediction_unavailable}}<br><small>coverage ${{c.coverage.complete}} complete / ${{c.coverage.partial}} partial / ${{c.coverage.excluded}} excluded / ${{c.coverage.not_evaluated}} not evaluated</small></td><td>${{facet(c,'runtime_sets').map(esc).join('<br>')}}</td><td>${{c.report_link?`<a href="${{esc(c.report_link)}}">open report</a>`:'—'}}</td></tr>`).join('')}}
for(const id of ['source','role','set','severity','outcome','availability','group'])q(id).onchange=draw;draw();
q('sets').innerHTML=sets.map(s=>`<article><h3><code>${{esc(s.id)}}</code> — ${{esc(s.lifecycle)}}</h3><ol>${{s.members.map(m=>`<li><code>${{esc(m)}}</code></li>`).join('')}}</ol><p>${{esc((s.gaps||[]).join(', ')||'no recorded set gaps')}}</p></article>`).join('');
q('evaluation').textContent=JSON.stringify(d.evaluation,null,2);
</script></body></html>"#,
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
    fn collection_dashboard_escapes_authority_script_terminators() {
        let authority = r#"{"schema":"x","text":"</script><img src=x>"}"#;
        let html = render_collection_dashboard(authority);
        let marker = r#"<script type="application/json" id="collection-dashboard-data">"#;
        let (_, tail) = html.split_once(marker).expect("dashboard data marker");
        let (raw, _) = tail.split_once("</script>").expect("dashboard data close");
        assert!(!raw.contains("</script>"));
        assert!(raw.contains(r#"\u003c/script>"#));
        assert_eq!(
            serde_json::from_str::<Value>(raw).unwrap()["text"],
            "</script><img src=x>"
        );
    }

    #[test]
    fn shared_grid_render_embeds_clip_data() {
        let doc = report_document();
        let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
        let config = Config::default();
        let checks = Vec::new();

        let fresh = render(&MetricGrids::new(&doc), &roles, &checks, None, None);
        let grids = MetricGrids::new(&doc);
        let ctx = CheckCtx::new(&grids, &roles, &config);
        assert!(ctx.grid(0).is_some());
        let shared = render(&grids, &roles, &checks, None, None);

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
