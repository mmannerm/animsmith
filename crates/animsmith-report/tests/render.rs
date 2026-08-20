use animsmith_core::metrics::{MetricGrids, metric_frame_count};
use animsmith_core::profile::{ResolvedRoles, Role};
use animsmith_core::sample::sample_clip;
use animsmith_core::{
    CheckEvaluation, CheckOutput, CoverageGap, CoverageGapCode, Finding, Severity,
};
use animsmith_core::{
    EnginePredictionBasisV1, EnginePredictionFacetV1, EnginePredictionV1, EvaluationScope,
    EvaluationScopeCode, PredictionBasisReferenceV1, PredictionProvenanceV1,
    PredictionUnavailableReasonV1,
};
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

fn chart_roles(doc: &animsmith_core::Document) -> ResolvedRoles {
    ResolvedRoles::from_names(
        &doc.skeleton,
        [
            (Role::Root, "root".to_string()),
            (Role::Hips, "hips".to_string()),
            (Role::LeftFoot, "foot".to_string()),
            (Role::RightFoot, "foot".to_string()),
        ],
    )
}

fn evaluations(findings: Vec<Finding>) -> Vec<CheckEvaluation> {
    let Some(check_id) = findings.first().map(|finding| finding.check_id) else {
        return Vec::new();
    };
    assert!(findings.iter().all(|finding| finding.check_id == check_id));
    vec![
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(findings, Vec::new(), Vec::new()),
        )
        .expect("test findings form one valid evaluation"),
    ]
}

fn prediction_provenance() -> (animsmith_core::LoadedSource, PredictionProvenanceV1) {
    let source = animsmith_gltf::load_source(&fixture()).expect("fixture source loads");
    let clip_names = source
        .document()
        .clips
        .iter()
        .map(|clip| clip.name.clone())
        .collect::<Vec<_>>();
    let resolved = animsmith_engine::resolve_static(animsmith_engine::EngineDeclaration {
        selection: Some(animsmith_engine::ProfileSelection::new(
            "bevy",
            1,
            "0.19.0",
            "gltf-asset-loader",
        )),
        ..Default::default()
    })
    .expect("profile declaration is valid")
    .expect("profile selected")
    .resolve_input(source.source_facts().format(), &clip_names)
    .expect("fixture format is accepted");
    let provenance = animsmith_engine::project_prediction_provenance_v1(&resolved, &source)
        .expect("same-load provenance projects");
    (source, provenance)
}

fn prediction_check(
    provenance: &PredictionProvenanceV1,
    available: bool,
    unavailable: bool,
) -> CheckEvaluation {
    let available_scope = EvaluationScope::new(EvaluationScopeCode::custom("test:available"));
    let unavailable_scope = EvaluationScope::new(EvaluationScopeCode::custom("test:unavailable"));
    let mut facets = Vec::new();
    let mut evaluated = Vec::new();
    let mut findings = Vec::new();
    if available {
        let basis = EnginePredictionBasisV1::new(vec![
            PredictionBasisReferenceV1::profile_fact("accepted_inputs")
                .expect("known profile fact reference"),
        ])
        .expect("nonempty basis");
        facets.push(
            EnginePredictionFacetV1::available(available_scope.clone(), basis)
                .expect("available facet"),
        );
        evaluated.push(available_scope.clone());
        findings.push(
            Finding::new("test:engine", Severity::Warning, "available facet finding")
                .prediction_scope(available_scope),
        );
    }
    if unavailable {
        facets.push(
            EnginePredictionFacetV1::required_unavailable(
                unavailable_scope,
                EnginePredictionBasisV1::new(Vec::new()).expect("empty unavailable prefix"),
                vec![PredictionUnavailableReasonV1::ProjectIntentUnavailable],
            )
            .expect("required-unavailable facet"),
        );
    }
    let prediction = EnginePredictionV1::new(provenance.identity().clone(), facets)
        .expect("canonical prediction");
    CheckEvaluation::evaluated(
        "test:engine",
        CheckOutput::from_coverage(findings, evaluated, Vec::new())
            .with_engine_prediction(prediction),
    )
    .expect("prediction lifecycle is valid")
}

#[test]
fn render_embeds_pose_grid_and_uses_no_external_urls() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::default();
    let checks = Vec::new();

    let html = animsmith_report::render(&grids, &roles, &checks, None, None);
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
fn render_self_contained_with_roles_findings_and_charts() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let checks = evaluations(vec![
        Finding::new("fixture-check", Severity::Warning, "fixture finding")
            .clip("walk")
            .bone("hips")
            .node("#0(root)/#1(hips)")
            .time(0.5),
    ]);

    let html = animsmith_report::render(&grids, &roles, &checks, None, None);
    assert_self_contained(&html);
    assert!(
        html.contains(r#"data-kind="gait""#),
        "resolved foot roles render the gait chart"
    );
    assert!(
        html.contains(r#"data-kind="rootpath""#),
        "resolved root role renders the root path chart"
    );

    let data = report_data(&html);
    assert_eq!(data["profile"], "custom");
    assert_eq!(data["clips"][0]["trails"]["root"], 0);
    assert_eq!(data["clips"][0]["trails"]["hips"], 1);
    assert_eq!(data["clips"][0]["trails"]["left_foot"], 2);
    assert_eq!(data["clips"][0]["trails"]["right_foot"], 2);
    assert_eq!(
        data["findings"].as_array().expect("findings array").len(),
        1
    );
    assert_eq!(data["findings"][0]["check"], "fixture-check");
    assert_eq!(data["findings"][0]["severity"], "warning");
    assert_eq!(data["findings"][0]["clip"], "walk");
    assert_eq!(data["findings"][0]["bone"], "hips");
    assert_eq!(data["findings"][0]["node"], "#0(root)/#1(hips)");
    assert_eq!(data["findings"][0]["message"], "fixture finding");
    assert!(
        html.contains("[f.clip, f.bone, f.node].filter(Boolean)"),
        "the embedded viewer must render node context, not only carry it in JSON"
    );
}

#[test]
fn render_respects_clip_filter() {
    let doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::default();
    let checks = Vec::new();

    let html = animsmith_report::render(&grids, &roles, &checks, None, Some("missing"));
    assert_self_contained(&html);
    let data = report_data(&html);
    assert_eq!(
        data["clips"].as_array().expect("clips array").len(),
        0,
        "unknown --clip filter excludes every pose grid"
    );

    for name in ["walk", "idle"] {
        let html = animsmith_report::render(&grids, &roles, &checks, None, Some(name));
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

#[test]
fn render_keeps_available_mixed_and_unavailable_predictions_distinct() {
    let (source, provenance) = prediction_provenance();
    let grids = MetricGrids::new(source.document());
    let roles = ResolvedRoles::default();

    for (available, unavailable, expected_states) in [
        (true, false, vec!["available"]),
        (
            true,
            true,
            vec!["available", "required_prediction_unavailable"],
        ),
        (false, true, vec!["required_prediction_unavailable"]),
    ] {
        let gap_scope = EvaluationScope::new(EvaluationScopeCode::custom("test:gap-scope"))
            .subject("clip with a gap");
        let gap_check = CheckEvaluation::evaluated(
            "test:gap",
            CheckOutput::from_coverage(
                Vec::new(),
                Vec::new(),
                vec![
                    CoverageGap::new(CoverageGapCode::custom("test:gap"), "ordinary gap")
                        .scope(gap_scope),
                ],
            ),
        )
        .expect("ordinary coverage gap is valid");
        let checks = vec![
            prediction_check(&provenance, available, unavailable),
            gap_check,
        ];
        let html = animsmith_report::render(&grids, &roles, &checks, Some(&provenance), None);
        let data = report_data(&html);
        let states = data["predictions"][0]["prediction"]["facets"]
            .as_array()
            .expect("facet array")
            .iter()
            .map(|facet| facet["state"].as_str().expect("state"))
            .collect::<Vec<_>>();
        assert_eq!(states, expected_states);
        assert_eq!(data["gaps"][0]["check_id"], "test:gap");
        assert_eq!(data["gaps"][0]["code"], "test:gap");
        assert_eq!(data["gaps"][0]["scope"]["code"], "test:gap-scope");
        assert_eq!(data["gaps"][0]["message"], "ordinary gap");
        assert_eq!(
            data["prediction_provenance"]["identity"],
            serde_json::to_value(provenance.identity()).expect("identity serializes")
        );
        assert!(html.contains("required prediction unavailable"));
        assert!(html.contains("available"));
        assert!(html.contains("Coverage gaps"));
    }
}
