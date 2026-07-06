use animsmith_core::Finding;
use animsmith_core::metrics::{MetricGrids, metric_frame_count};
use animsmith_core::profile::ResolvedRoles;
use animsmith_core::sample::sample_clip;
use base64::Engine as _;
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rig.gltf")
}

fn report_data(html: &str) -> Value {
    let id = "report-data";
    let id_pos = html.find(id).expect("report data script id");
    let script_start = html[..id_pos].rfind("<script").expect("report data script");
    let start = html[id_pos..].find('>').expect("script tag close") + id_pos + 1;
    assert!(
        script_start < id_pos && id_pos < start,
        "report data id lives on the script tag"
    );
    let end = html[start..].find("</script>").expect("script close") + start;
    serde_json::from_str(&html[start..end]).expect("report data JSON")
}

fn assert_self_contained(html: &str) {
    let compact = html.split_ascii_whitespace().collect::<String>();
    let lower = compact.to_ascii_lowercase();
    for needle in [
        "://",
        "http://",
        "https://",
        "<link",
        "<scriptsrc=",
        "src=",
        "href=",
        "fetch(",
        "import(",
        "import'//",
        "import\"//",
        "from'//",
        "from\"//",
        "xmlhttprequest",
        "@import",
        "url(",
    ] {
        assert!(
            !lower.contains(needle),
            "external reference marker {needle:?}"
        );
    }
}

#[test]
#[should_panic(expected = "external reference marker")]
fn self_contained_rejects_protocol_relative_module_import() {
    assert_self_contained("<script type=\"module\">import '//cdn.example.test/viewer.js'</script>");
}

fn pose_grid_bytes(doc: &animsmith_core::Document, clip_name: &str) -> Vec<u8> {
    let clip = doc
        .clips
        .iter()
        .find(|clip| clip.name == clip_name)
        .expect("source clip");
    let frames = metric_frame_count(clip).expect("metric frame count");
    let grid = sample_clip(&doc.skeleton, clip, frames);
    let mut positions = Vec::with_capacity(frames * grid.bone_count() * 3 * 4);
    for frame in 0..frames {
        for bone in 0..grid.bone_count() {
            let p = grid.model_position(frame, bone);
            positions.extend_from_slice(&p.x.to_le_bytes());
            positions.extend_from_slice(&p.y.to_le_bytes());
            positions.extend_from_slice(&p.z.to_le_bytes());
        }
    }
    positions
}

#[test]
fn render_embeds_pose_grid_and_uses_no_external_urls() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::default();
    let findings: Vec<Finding> = Vec::new();

    let html = animsmith_report::render(&grids, &roles, &findings, None);
    assert_self_contained(&html);
    let data = report_data(&html);
    let clips = data["clips"].as_array().expect("clips array");

    assert_eq!(clips.len(), doc.clips.len(), "one pose-grid blob per clip");
    let rendered_names: Vec<&str> = clips
        .iter()
        .map(|clip| clip["name"].as_str().expect("clip name"))
        .collect();
    let source_names: Vec<&str> = doc.clips.iter().map(|clip| clip.name.as_str()).collect();
    assert_eq!(rendered_names, source_names);
    assert_eq!(rendered_names, vec!["walk", "idle"]);
    assert_ne!(
        pose_grid_bytes(&doc, "walk"),
        pose_grid_bytes(&doc, "idle"),
        "fixture clips must prove per-clip pose grid data"
    );

    for clip in clips {
        let name = clip["name"].as_str().expect("clip name");
        assert_eq!(clip["frames"], 3);
        let encoded = clip["positions"].as_str().expect("encoded positions");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("pose grid base64");
        assert_eq!(decoded, pose_grid_bytes(&doc, name));
    }
}

#[test]
fn render_respects_clip_filter() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::default();
    let findings: Vec<Finding> = Vec::new();

    let html = animsmith_report::render(&grids, &roles, &findings, Some("missing"));
    assert_self_contained(&html);
    let data = report_data(&html);
    assert_eq!(
        data["clips"].as_array().expect("clips array").len(),
        0,
        "unknown --clip filter excludes every pose grid"
    );

    for name in ["walk", "idle"] {
        let html = animsmith_report::render(&grids, &roles, &findings, Some(name));
        let data = report_data(&html);
        let clips = data["clips"].as_array().expect("clips array");
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0]["name"], name);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(clips[0]["positions"].as_str().expect("encoded positions"))
            .expect("pose grid base64");
        assert_eq!(decoded, pose_grid_bytes(&doc, name));
    }
}
