use animsmith_core::Finding;
use animsmith_core::profile::ResolvedRoles;
use serde_json::Value;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rig.gltf")
}

fn report_data(html: &str) -> Value {
    let marker = r#"<script type="application/json" id="report-data">"#;
    let start = html.find(marker).expect("report data script") + marker.len();
    let end = html[start..].find("</script>").expect("script close") + start;
    serde_json::from_str(&html[start..end]).expect("report data JSON")
}

fn assert_self_contained(html: &str) {
    for needle in [
        "http://",
        "https://",
        "<link",
        "<script src=",
        "src=\"",
        "href=\"",
        "fetch(",
        "XMLHttpRequest",
        "@import",
        "url(",
    ] {
        assert!(
            !html.contains(needle),
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
    let clip = &clips[0];
    assert_eq!(clip["name"], "walk");
    assert_eq!(clip["frames"], 3);
    assert!(
        clip["positions"].as_str().is_some_and(|p| !p.is_empty()),
        "pose-grid blob embedded"
    );
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

    let html = animsmith_report::render(&doc, &roles, &findings, Some("walk"));
    let data = report_data(&html);
    let clips = data["clips"].as_array().expect("clips array");
    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0]["name"], "walk");
}
