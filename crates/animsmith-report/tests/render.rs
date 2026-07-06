use animsmith_core::Finding;
use animsmith_core::profile::ResolvedRoles;
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
        "http://",
        "https://",
        "<link",
        "<scriptsrc=",
        "src=",
        "href=",
        "fetch(",
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
fn render_embeds_pose_grid_and_uses_no_external_urls() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let roles = ResolvedRoles::default();
    let findings: Vec<Finding> = Vec::new();

    let html = animsmith_report::render(&doc, &roles, &findings, None);
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

    for clip in clips {
        assert_eq!(clip["frames"], 3);
        assert!(
            clip["positions"].as_str().is_some_and(|p| !p.is_empty()),
            "pose-grid blob embedded for {clip:?}"
        );
    }
}

#[test]
fn render_respects_clip_filter() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let roles = ResolvedRoles::default();
    let findings: Vec<Finding> = Vec::new();

    let html = animsmith_report::render(&doc, &roles, &findings, Some("missing"));
    assert_self_contained(&html);
    let data = report_data(&html);
    assert_eq!(
        data["clips"].as_array().expect("clips array").len(),
        0,
        "unknown --clip filter excludes every pose grid"
    );

    let html = animsmith_report::render(&doc, &roles, &findings, Some("idle"));
    let data = report_data(&html);
    let clips = data["clips"].as_array().expect("clips array");
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0]["name"], "idle");
}
