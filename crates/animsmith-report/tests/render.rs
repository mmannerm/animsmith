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

fn comparison_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(format!("report-comparison-{name}.glb"))
}

fn report_data(html: &str) -> Value {
    embedded_json(html, "report-data")
}

fn embedded_json(html: &str, id: &str) -> Value {
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

fn comparison_side<'a>(
    source: &'a animsmith_core::LoadedSource,
    grids: &'a MetricGrids<'a>,
    roles: &'a ResolvedRoles,
    checks: &'a [CheckEvaluation],
    config: &'a animsmith_core::Config,
    clip: &'a str,
) -> animsmith_report::ComparisonSide<'a> {
    animsmith_report::ComparisonSide {
        source,
        grids,
        roles,
        checks,
        config,
        prediction_provenance: None,
        clip,
    }
}

#[test]
fn comparison_is_deterministic_escaped_and_keeps_sides_separate() {
    let before_source =
        animsmith_gltf::load_source(&comparison_fixture("before")).expect("before fixture loads");
    let after_source =
        animsmith_gltf::load_source(&comparison_fixture("after")).expect("after fixture loads");
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let checks = evaluations(vec![
        Finding::new(
            "fixture-check",
            Severity::Warning,
            "</script><img onerror=alert(1)>",
        )
        .clip("acceptance-matrix")
        .bone("hips")
        .time(0.5),
        Finding::new(
            "fixture-check",
            Severity::Warning,
            "second semantic at the same subject and time",
        )
        .clip("acceptance-matrix")
        .bone("hips")
        .time(0.5),
    ]);
    let before = comparison_side(
        &before_source,
        &before_grids,
        &roles,
        &checks,
        &config,
        "acceptance-matrix",
    );
    let after = comparison_side(
        &after_source,
        &after_grids,
        &roles,
        &[],
        &config,
        "acceptance-matrix",
    );
    let first = animsmith_report::render_comparison(before, after).expect("comparison renders");
    let second = animsmith_report::render_comparison(before, after)
        .expect("comparison renders deterministically");

    assert_eq!(first, second);
    assert_self_contained(&first);
    assert!(
        !first.contains("</script><img"),
        "untrusted HTML must stay JSON data"
    );
    assert!(
        first.contains("item.id ="),
        "viewer gives findings stable in-document anchors"
    );
    let data = embedded_json(&first, "comparison-report-data");
    assert_eq!(data["kind"], "animsmith-comparison-v1");
    assert_eq!(data["correspondence"]["before_clip"], "acceptance-matrix");
    assert_eq!(data["correspondence"]["after_clip"], "acceptance-matrix");
    assert_eq!(
        data["before"]["dependency_closure_identity"],
        serde_json::to_value(before_source.dependency_closure().identity().unwrap()).unwrap()
    );
    assert_eq!(
        data["after"]["dependency_closure_identity"],
        serde_json::to_value(after_source.dependency_closure().identity().unwrap()).unwrap()
    );
    assert_ne!(
        data["before"]["dependency_closure_identity"],
        data["after"]["dependency_closure_identity"]
    );
    assert_eq!(data["before"]["findings"][0]["bone"], "hips");
    assert_ne!(
        data["before"]["findings"][0]["anchor"], data["before"]["findings"][1]["anchor"],
        "distinct findings cannot produce duplicate in-document ids"
    );
    assert!(
        data["after"]["findings"]
            .as_array()
            .expect("after findings")
            .is_empty()
    );
}

#[test]
fn comparison_refuses_incompatible_named_hierarchy_before_rendering() {
    let before_doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let mut after_doc = before_doc.clone();
    after_doc.skeleton.bones[1].parent = None;
    let error = animsmith_report::preflight_comparison(&before_doc, "walk", &after_doc, "walk")
        .expect_err("different named parent must refuse");
    assert!(matches!(
        error,
        animsmith_report::ComparisonError::IncompatibleSkeleton { .. }
    ));
}

#[test]
fn comparison_preflight_refuses_each_structural_and_grid_ambiguity_on_both_sides() {
    let baseline = animsmith_testkit::comparison_report_before_doc();
    for side in ["before", "after"] {
        let mut duplicate = baseline.clone();
        duplicate.skeleton.bones[2].name = duplicate.skeleton.bones[3].name.clone();
        let result = if side == "before" {
            animsmith_report::preflight_comparison(
                &duplicate,
                "acceptance-matrix",
                &baseline,
                "acceptance-matrix",
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &duplicate,
                "acceptance-matrix",
            )
        };
        assert!(
            matches!(result, Err(animsmith_report::ComparisonError::DuplicateBoneName { side: found, .. }) if found == side)
        );

        let mut later_parent = baseline.clone();
        later_parent.skeleton.bones[2].parent = Some(3);
        let result = if side == "before" {
            animsmith_report::preflight_comparison(
                &later_parent,
                "acceptance-matrix",
                &baseline,
                "acceptance-matrix",
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &later_parent,
                "acceptance-matrix",
            )
        };
        assert!(
            matches!(result, Err(animsmith_report::ComparisonError::InvalidHierarchy { side: found, .. }) if found == side)
        );

        let mut invalid_parent = baseline.clone();
        invalid_parent.skeleton.bones[2].parent = Some(usize::MAX);
        let result = if side == "before" {
            animsmith_report::preflight_comparison(
                &invalid_parent,
                "acceptance-matrix",
                &baseline,
                "acceptance-matrix",
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &invalid_parent,
                "acceptance-matrix",
            )
        };
        assert!(
            matches!(result, Err(animsmith_report::ComparisonError::InvalidHierarchy { side: found, .. }) if found == side)
        );

        let mut unavailable = baseline.clone();
        unavailable.clips[0].duration_s = 0.0;
        let result = if side == "before" {
            animsmith_report::preflight_comparison(
                &unavailable,
                "acceptance-matrix",
                &baseline,
                "acceptance-matrix",
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &unavailable,
                "acceptance-matrix",
            )
        };
        assert!(
            matches!(result, Err(animsmith_report::ComparisonError::UnavailableSampleGrid { side: found, .. }) if found == side)
        );
    }

    let mut reordered = baseline.clone();
    reordered.skeleton.bones.swap(2, 3);
    assert!(matches!(
        animsmith_report::preflight_comparison(
            &baseline,
            "acceptance-matrix",
            &reordered,
            "acceptance-matrix"
        ),
        Err(animsmith_report::ComparisonError::IncompatibleSkeleton { .. })
    ));
}

#[test]
fn comparison_refuses_the_same_complete_loader_authority() {
    let source = animsmith_gltf::load_source(&fixture()).expect("fixture loads");
    assert_eq!(
        animsmith_report::preflight_comparison_sources(&source, "walk", &source, "idle"),
        Err(animsmith_report::ComparisonError::IdenticalAuthorities)
    );
}

#[test]
fn comparison_binds_equal_primary_bytes_to_changed_sidecar_closures() {
    let directory = tempfile::tempdir().unwrap();
    let source_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture()).unwrap()).unwrap();
    let encoded = source_json["buffers"][0]["uri"]
        .as_str()
        .unwrap()
        .split_once(',')
        .unwrap()
        .1;
    let mut sidecar = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let mut external = source_json;
    external["buffers"][0]["uri"] = serde_json::Value::String("clip.bin".to_owned());
    let primary = serde_json::to_vec(&external).unwrap();
    let mut paths = Vec::new();
    for name in ["before", "after"] {
        let root = directory.path().join(name);
        std::fs::create_dir(&root).unwrap();
        let path = root.join("asset.gltf");
        std::fs::write(&path, &primary).unwrap();
        std::fs::write(root.join("clip.bin"), &sidecar).unwrap();
        paths.push(path);
        let last = sidecar.len() - 4;
        sidecar[last] ^= 1;
    }
    let before_source = animsmith_gltf::load_source(&paths[0]).unwrap();
    let after_source = animsmith_gltf::load_source(&paths[1]).unwrap();
    assert_eq!(
        before_source.dependency_closure().primary_input(),
        after_source.dependency_closure().primary_input()
    );
    assert_ne!(
        before_source.dependency_closure().identity(),
        after_source.dependency_closure().identity()
    );
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let html = animsmith_report::render_comparison(
        comparison_side(&before_source, &before_grids, &roles, &[], &config, "walk"),
        comparison_side(&after_source, &after_grids, &roles, &[], &config, "walk"),
    )
    .unwrap();
    let data = embedded_json(&html, "comparison-report-data");
    assert_eq!(data["before"]["identity"], data["after"]["identity"]);
    assert_ne!(
        data["before"]["dependency_closure_identity"],
        data["after"]["dependency_closure_identity"]
    );
}

#[test]
fn identical_findings_get_side_and_occurrence_unique_navigation_anchors() {
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let identical = Finding::new("fixture-check", Severity::Warning, "identical")
        .clip("acceptance-matrix")
        .bone("hips")
        .time(0.5);
    let before_checks = evaluations(vec![identical.clone(), identical.clone()]);
    let after_checks = evaluations(vec![identical]);
    let html = animsmith_report::render_comparison(
        comparison_side(
            &before_source,
            &before_grids,
            &roles,
            &before_checks,
            &config,
            "acceptance-matrix",
        ),
        comparison_side(
            &after_source,
            &after_grids,
            &roles,
            &after_checks,
            &config,
            "acceptance-matrix",
        ),
    )
    .unwrap();
    let data = embedded_json(&html, "comparison-report-data");
    let first = data["before"]["findings"][0]["anchor"].as_str().unwrap();
    let second = data["before"]["findings"][1]["anchor"].as_str().unwrap();
    let other_side = data["after"]["findings"][0]["anchor"].as_str().unwrap();
    assert_ne!(first, second);
    assert_ne!(first, other_side);
    assert_ne!(second, other_side);
    assert!(html.contains("#(?:finding|time)-(before|after)-"));
}

#[test]
fn comparison_public_boundary_admits_finding_max_and_refuses_n_plus_one() {
    const LIMIT: usize = 4096;
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let finding =
        Finding::new("fixture-check", Severity::Warning, "bounded").clip("acceptance-matrix");
    let render = |count| {
        let checks = evaluations(vec![finding.clone(); count]);
        animsmith_report::render_comparison(
            comparison_side(
                &before_source,
                &before_grids,
                &roles,
                &checks,
                &config,
                "acceptance-matrix",
            ),
            comparison_side(
                &after_source,
                &after_grids,
                &roles,
                &[],
                &config,
                "acceptance-matrix",
            ),
        )
    };
    render(LIMIT).expect("the exact finding limit renders");
    assert_eq!(
        render(LIMIT + 1).unwrap_err(),
        animsmith_report::ComparisonError::ReportRowsExceeded {
            side: "before",
            kind: "findings",
            found: LIMIT + 1,
            limit: LIMIT,
        }
    );
}

fn comparison_matrix_config() -> animsmith_core::Config {
    use animsmith_core::config::{CheckSettings, ClipExpectations};
    use animsmith_core::{MovementOwner, Pinned};

    let mut config = animsmith_core::Config::default();
    config.rig.roles = [
        (Role::Root, "root".to_owned()),
        (Role::Hips, "hips".to_owned()),
        (Role::LeftFoot, "left_foot".to_owned()),
        (Role::RightFoot, "right_foot".to_owned()),
    ]
    .into_iter()
    .collect();
    config.clips.insert(
        "acceptance-matrix".to_owned(),
        ClipExpectations {
            looping: Some(true),
            speed_mps: Some(Pinned {
                value: 1.0,
                tolerance: 0.1,
            }),
            movement_owner_xz: Some(MovementOwner::Gameplay),
            ..Default::default()
        },
    );
    config.checks.insert(
        "foot-slide".to_owned(),
        CheckSettings {
            contact_height_m: Some(0.03),
            max_slide_mps: Some(0.3),
            ..Default::default()
        },
    );
    config
}

fn matrix_evaluations<'a>(
    grids: &'a MetricGrids<'a>,
    roles: &'a ResolvedRoles,
    config: &'a animsmith_core::Config,
) -> Vec<CheckEvaluation> {
    let context = animsmith_core::CheckCtx::new(grids, roles, config);
    animsmith_core::evaluate_checks(
        &context,
        &animsmith_core::all_checks(),
        animsmith_core::CheckSelection::All,
    )
    .expect("matrix checks evaluate")
}

#[test]
fn comparison_matrix_projects_typed_visual_acceptance_context() {
    let before_source =
        animsmith_gltf::load_source(&comparison_fixture("before")).expect("before fixture loads");
    let after_source =
        animsmith_gltf::load_source(&comparison_fixture("after")).expect("after fixture loads");
    let before_doc = before_source.document();
    let after_doc = after_source.document();
    let config = comparison_matrix_config();
    let roles = ResolvedRoles::from_names(&before_doc.skeleton, config.rig.roles.clone());
    let after_roles = ResolvedRoles::from_names(&after_doc.skeleton, config.rig.roles.clone());
    let before_grids = MetricGrids::new(before_doc);
    let after_grids = MetricGrids::new(after_doc);
    let before_checks = matrix_evaluations(&before_grids, &roles, &config);
    let after_checks = matrix_evaluations(&after_grids, &after_roles, &config);
    let html = animsmith_report::render_comparison(
        comparison_side(
            &before_source,
            &before_grids,
            &roles,
            &before_checks,
            &config,
            "acceptance-matrix",
        ),
        comparison_side(
            &after_source,
            &after_grids,
            &after_roles,
            &after_checks,
            &config,
            "acceptance-matrix",
        ),
    )
    .expect("matrix comparison renders");
    let data = embedded_json(&html, "comparison-report-data");

    for side in ["before", "after"] {
        assert_eq!(data[side]["clip"]["trails"]["root"], 0);
        assert_eq!(data[side]["clip"]["trails"]["hips"], 1);
        assert_eq!(data[side]["clip"]["trails"]["left_foot"], 2);
        assert_eq!(data[side]["clip"]["trails"]["right_foot"], 3);
        assert_eq!(
            data[side]["contexts"]["stances"].as_array().unwrap().len(),
            2
        );
        assert!(data[side]["contexts"]["gait"].is_object());
    }
    let before_stances = data["before"]["contexts"]["stances"]
        .as_array()
        .expect("before stances");
    assert_eq!(before_stances[0]["selected_role"], "left_foot");
    assert_eq!(before_stances[0]["runs"][0]["start_s"], 0.0);
    assert_eq!(before_stances[0]["runs"][0]["end_s"], 0.25);

    let seam = data["before"]["contexts"]["seams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["check"] == "loop-closure")
        .expect("loop closure endpoint context");
    assert_eq!(seam["first_frame"], 0);
    assert_eq!(seam["last_frame"], 4);
    assert_eq!(seam["subject_bone_name"], "left_foot");
    assert_eq!(seam["subject_bone"], 2);

    let structural = data["before"]["contexts"]["structural"]
        .as_array()
        .expect("before structural context");
    assert_eq!(structural.len(), 1);
    assert_eq!(structural[0]["check"], "constant-track");
    assert_eq!(structural[0]["evidence_kind"], "structural");
    assert!(
        structural[0]["label"]
            .as_str()
            .unwrap()
            .contains("poses may look unchanged")
    );
    assert!(
        data["after"]["contexts"]["structural"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(html.contains("one shared uniform metres scale"));
}

#[test]
fn constant_quaternion_track_removal_leaves_sampled_pose_positions_unchanged() {
    let before = animsmith_testkit::comparison_report_before_doc();
    let mut structural_after = before.clone();
    structural_after.clips[0]
        .tracks
        .retain(|track| !(track.bone == 4 && track.property == animsmith_core::Property::Rotation));
    let before_grid = MetricGrids::new(&before).grid(0).expect("before grid");
    let after_grid = MetricGrids::new(&structural_after)
        .grid(0)
        .expect("structural after grid");

    assert_eq!(before_grid.times, after_grid.times);
    for frame in 0..before_grid.frame_count() {
        for bone in 0..before_grid.bone_count() {
            assert_eq!(
                before_grid.model_position(frame, bone),
                after_grid.model_position(frame, bone),
                "constant quaternion removal changed visible position at frame {frame}, bone {bone}"
            );
        }
    }
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
            (Role::RightFoot, "right_foot".to_string()),
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
fn comparison_filters_scoped_gaps_and_prediction_facets_to_selected_clip() {
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let selected_scope = EvaluationScope::new(EvaluationScopeCode::custom("test:selected"))
        .subject("acceptance-matrix");
    let other_scope =
        EvaluationScope::new(EvaluationScopeCode::custom("test:other")).subject("other-clip");
    let gaps = CheckEvaluation::evaluated(
        "test:gaps",
        CheckOutput::from_coverage(
            Vec::new(),
            Vec::new(),
            vec![
                CoverageGap::new(CoverageGapCode::custom("test:selected-gap"), "selected")
                    .scope(selected_scope.clone()),
                CoverageGap::new(CoverageGapCode::custom("test:other-gap"), "other")
                    .scope(other_scope.clone()),
            ],
        ),
    )
    .unwrap();
    let (_, provenance) = prediction_provenance();
    let basis = EnginePredictionBasisV1::new(vec![
        PredictionBasisReferenceV1::profile_fact("accepted_inputs").unwrap(),
    ])
    .unwrap();
    let prediction = EnginePredictionV1::new(
        provenance.identity().clone(),
        vec![
            EnginePredictionFacetV1::available(selected_scope.clone(), basis.clone()).unwrap(),
            EnginePredictionFacetV1::available(other_scope.clone(), basis).unwrap(),
        ],
    )
    .unwrap();
    let prediction = CheckEvaluation::evaluated(
        "test:prediction",
        CheckOutput::from_coverage(
            Vec::new(),
            vec![selected_scope.clone(), other_scope],
            Vec::new(),
        )
        .with_engine_prediction(prediction),
    )
    .unwrap();
    let checks = vec![gaps, prediction];
    let html = animsmith_report::render_comparison(
        comparison_side(
            &before_source,
            &before_grids,
            &roles,
            &checks,
            &config,
            "acceptance-matrix",
        ),
        comparison_side(
            &after_source,
            &after_grids,
            &roles,
            &[],
            &config,
            "acceptance-matrix",
        ),
    )
    .unwrap();
    let data = embedded_json(&html, "comparison-report-data");
    assert_eq!(data["before"]["gaps"].as_array().unwrap().len(), 1);
    assert_eq!(
        data["before"]["gaps"][0]["scope"]["subject"],
        "acceptance-matrix"
    );
    assert_eq!(
        data["before"]["predictions"][0]["prediction"]["facets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        data["before"]["predictions"][0]["prediction"]["facets"][0]["scope"]["subject"],
        "acceptance-matrix"
    );
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
    let mut doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let mut right_foot = doc.skeleton.bones[2].clone();
    right_foot.name = "right_foot".into();
    doc.skeleton.bones.push(right_foot);
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
    assert_eq!(data["clips"][0]["trails"]["right_foot"], 3);
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
