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

/// A full report: sampled poses embedded, the historical default.
fn full() -> animsmith_report::ReportOptions {
    animsmith_report::ReportOptions::default()
}

/// A report that keeps the evidence and leaves the motion out.
fn evidence_only() -> animsmith_report::ReportOptions {
    animsmith_report::ReportOptions {
        evidence_only: true,
    }
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

fn comparison_side_with_provenance<'a>(
    source: &'a animsmith_core::LoadedSource,
    grids: &'a MetricGrids<'a>,
    roles: &'a ResolvedRoles,
    checks: &'a [CheckEvaluation],
    config: &'a animsmith_core::Config,
    provenance: Option<&'a PredictionProvenanceV1>,
    clip: &'a str,
) -> animsmith_report::ComparisonSide<'a> {
    animsmith_report::ComparisonSide {
        source,
        grids,
        roles,
        checks,
        config,
        prediction_provenance: provenance,
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
    let first =
        animsmith_report::render_comparison(before, after, full()).expect("comparison renders");
    let second = animsmith_report::render_comparison(before, after, full())
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
    let before_clip_anchor = data["before"]["clip"]["anchor"].as_str().unwrap();
    let after_clip_anchor = data["after"]["clip"]["anchor"].as_str().unwrap();
    assert_eq!(
        before_clip_anchor,
        embedded_json(&second, "comparison-report-data")["before"]["clip"]["anchor"]
    );
    assert!(first.contains(&format!("before-{before_clip_anchor}")));
    assert!(first.contains(&format!("after-{after_clip_anchor}")));
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
    let error =
        animsmith_report::preflight_comparison(&before_doc, "walk", &after_doc, "walk", full())
            .expect_err("different named parent must refuse");
    assert!(matches!(
        error,
        animsmith_report::ComparisonError::IncompatibleSkeleton { .. }
    ));
}

#[test]
fn comparison_public_preflight_refuses_normalized_document_clip_ambiguity() {
    let mut before = animsmith_testkit::comparison_report_before_doc();
    let after = animsmith_testkit::comparison_report_before_doc();
    before.clips.push(before.clips[0].clone());
    assert!(matches!(
        animsmith_report::preflight_comparison(
            &before,
            "acceptance-matrix",
            &after,
            "acceptance-matrix",
            full(),
        ),
        Err(animsmith_report::ComparisonError::AmbiguousClip { side: "before", .. })
    ));
}

#[test]
fn comparison_public_input_text_boundary_accepts_max_and_refuses_n_plus_one() {
    const LIMIT: usize = 1024 * 1024;
    let mut before = animsmith_testkit::comparison_report_before_doc();
    let mut after = before.clone();
    let retained = before
        .skeleton
        .bones
        .iter()
        .skip(1)
        .map(|bone| bone.name.len())
        .chain(before.clips.iter().map(|clip| clip.name.len()))
        .sum::<usize>();
    let exact = "x".repeat(LIMIT - retained);
    before.skeleton.bones[0].name = exact.clone();
    after.skeleton.bones[0].name = exact;
    animsmith_report::preflight_comparison(
        &before,
        "acceptance-matrix",
        &after,
        "acceptance-matrix",
        full(),
    )
    .expect("exact input-text limit is admitted");
    before.skeleton.bones[0].name.push('x');
    after.skeleton.bones[0].name.push('x');
    assert_eq!(
        animsmith_report::preflight_comparison(
            &before,
            "acceptance-matrix",
            &after,
            "acceptance-matrix",
            full(),
        ),
        Err(animsmith_report::ComparisonError::InputTextWorkExceeded {
            side: "before",
            bytes: LIMIT + 1,
            limit: LIMIT,
        })
    );
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
                full(),
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &duplicate,
                "acceptance-matrix",
                full(),
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
                full(),
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &later_parent,
                "acceptance-matrix",
                full(),
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
                full(),
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &invalid_parent,
                "acceptance-matrix",
                full(),
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
                full(),
            )
        } else {
            animsmith_report::preflight_comparison(
                &baseline,
                "acceptance-matrix",
                &unavailable,
                "acceptance-matrix",
                full(),
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
            "acceptance-matrix",
            full(),
        ),
        Err(animsmith_report::ComparisonError::IncompatibleSkeleton { .. })
    ));
}

#[test]
fn comparison_refuses_the_same_complete_loader_authority() {
    let source = animsmith_gltf::load_source(&fixture()).expect("fixture loads");
    assert_eq!(
        animsmith_report::preflight_comparison_sources(&source, "walk", &source, "idle", full()),
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
        full(),
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
fn comparison_projects_node_only_finding_subjects_from_exact_source_authority() {
    let directory = tempfile::tempdir().unwrap();
    let mut source_json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture()).unwrap()).unwrap();
    let special = "hip(s)/[authored] → normalized";
    source_json["nodes"][1]["name"] = special.into();
    let mut paths = Vec::new();
    for marker in ["before", "after"] {
        source_json["asset"]["extras"] = serde_json::json!({"authority": marker});
        let path = directory.path().join(format!("{marker}.gltf"));
        std::fs::write(&path, serde_json::to_vec(&source_json).unwrap()).unwrap();
        paths.push(path);
    }
    let before_source = animsmith_gltf::load_source(&paths[0]).unwrap();
    let after_source = animsmith_gltf::load_source(&paths[1]).unwrap();
    assert_eq!(before_source.document().skeleton.bones[1].name, special);
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let checks = evaluations(vec![
        Finding::new("fixture-check", Severity::Warning, "node-only")
            .clip("walk")
            .node(format!("#0(root)/#1({special})")),
        Finding::new("fixture-check", Severity::Warning, "normalized-name")
            .clip("walk")
            .bone(special),
        Finding::new("fixture-check", Severity::Warning, "unprojected")
            .clip("walk")
            .node("#99(unprojected(special)/[])"),
    ]);
    let html = animsmith_report::render_comparison(
        comparison_side(
            &before_source,
            &before_grids,
            &roles,
            &checks,
            &config,
            "walk",
        ),
        comparison_side(&after_source, &after_grids, &roles, &[], &config, "walk"),
        full(),
    )
    .unwrap();
    let data = embedded_json(&html, "comparison-report-data");
    assert_eq!(data["before"]["findings"][0]["subject_bone"], 1);
    assert_eq!(data["before"]["findings"][1]["subject_bone"], 1);
    assert!(data["before"]["findings"][2]["subject_bone"].is_null());
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
        full(),
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
            full(),
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

#[test]
fn comparison_public_report_text_boundary_counts_repeated_arbitrary_check_ids() {
    const LIMIT: usize = 4 * 1024 * 1024;
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let clip = "acceptance-matrix";
    let empty = Finding::new("", Severity::Warning, "bounded context").clip(clip);
    let fixed = serde_json::to_vec(&empty).unwrap().len() * 3
        + serde_json::to_vec(&clip).unwrap().len() * 16;
    let exact_id_bytes = (LIMIT - fixed) / 3;
    let render = |id_bytes| {
        let id: &'static str = Box::leak("x".repeat(id_bytes).into_boxed_str());
        let checks = evaluations(vec![
            Finding::new(id, Severity::Warning, "bounded context").clip(clip),
        ]);
        animsmith_report::render_comparison(
            comparison_side(
                &before_source,
                &before_grids,
                &roles,
                &checks,
                &config,
                clip,
            ),
            comparison_side(&after_source, &after_grids, &roles, &[], &config, clip),
            full(),
        )
    };
    render(exact_id_bytes).expect("exact aggregate report-text limit is admitted");
    assert_eq!(
        render(exact_id_bytes + 1).unwrap_err(),
        animsmith_report::ComparisonError::ReportTextWorkExceeded {
            side: "before",
            limit: LIMIT,
        }
    );
}

#[test]
fn comparison_public_boundary_refuses_real_gap_facet_and_context_n_plus_one() {
    const ROW_LIMIT: usize = 4096;
    const CONTEXT_LIMIT: usize = 8192;
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let gap = CheckEvaluation::evaluated(
        "test:gaps",
        CheckOutput::from_coverage(
            Vec::new(),
            Vec::new(),
            vec![CoverageGap::new(CoverageGapCode::custom("test:gap"), "gap")],
        ),
    )
    .unwrap();
    let gaps = vec![gap; ROW_LIMIT + 1];
    let render = |checks: &[CheckEvaluation], provenance| {
        animsmith_report::render_comparison(
            comparison_side_with_provenance(
                &before_source,
                &before_grids,
                &roles,
                checks,
                &config,
                provenance,
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
            full(),
        )
    };
    assert_eq!(
        render(&gaps, None).unwrap_err(),
        animsmith_report::ComparisonError::ReportRowsExceeded {
            side: "before",
            kind: "coverage gaps",
            found: ROW_LIMIT + 1,
            limit: ROW_LIMIT,
        }
    );

    let provenance = prediction_provenance_for(&before_source);
    let prediction = prediction_check(&provenance, false, true);
    let predictions = vec![prediction; ROW_LIMIT + 1];
    assert_eq!(
        render(&predictions, Some(&provenance)).unwrap_err(),
        animsmith_report::ComparisonError::ReportRowsExceeded {
            side: "before",
            kind: "prediction facets",
            found: ROW_LIMIT + 1,
            limit: ROW_LIMIT,
        }
    );

    for frames in [CONTEXT_LIMIT - 2, CONTEXT_LIMIT - 1] {
        let directory = tempfile::tempdir().unwrap();
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture()).unwrap()).unwrap();
        let mut bytes = Vec::with_capacity(frames * 16);
        for frame in 0..frames {
            bytes.extend_from_slice(&(frame as f32).to_le_bytes());
        }
        let output_offset = bytes.len();
        bytes.resize(output_offset + frames * 12, 0);
        json["buffers"][0]["uri"] = format!(
            "data:application/octet-stream;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        )
        .into();
        json["buffers"][0]["byteLength"] = bytes.len().into();
        json["bufferViews"] = serde_json::json!([
            {"buffer":0,"byteOffset":0,"byteLength":output_offset},
            {"buffer":0,"byteOffset":output_offset,"byteLength":frames * 12}
        ]);
        json["accessors"] = serde_json::json!([
            {"bufferView":0,"componentType":5126,"count":frames,"type":"SCALAR","min":[0.0],"max":[(frames - 1) as f32]},
            {"bufferView":1,"componentType":5126,"count":frames,"type":"VEC3"}
        ]);
        json["animations"] = serde_json::json!([{
            "name":"walk",
            "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
            "channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]
        }]);
        let mut long_paths = Vec::new();
        for marker in ["before", "after"] {
            json["asset"]["extras"] = serde_json::json!({"authority": marker});
            let path = directory.path().join(format!("{marker}.gltf"));
            std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
            long_paths.push(path);
        }
        let long_before = animsmith_gltf::load_source(&long_paths[0]).unwrap();
        let long_after = animsmith_gltf::load_source(&long_paths[1]).unwrap();
        let long_before_grids = MetricGrids::new(long_before.document());
        let long_after_grids = MetricGrids::new(long_after.document());
        let long_roles = ResolvedRoles::from_names(
            &long_before.document().skeleton,
            [
                (Role::Root, "root".to_owned()),
                (Role::Hips, "hips".to_owned()),
                (Role::LeftFoot, "foot".to_owned()),
                (Role::RightFoot, "foot".to_owned()),
            ],
        );
        let stance_scopes = ["left_foot_stance", "right_foot_stance"]
            .map(|code| EvaluationScope::new(EvaluationScopeCode::custom(code)).subject("walk"));
        let stance_checks = vec![
            CheckEvaluation::evaluated(
                "foot-slide",
                CheckOutput::from_coverage(Vec::new(), stance_scopes.into(), Vec::new()),
            )
            .unwrap(),
        ];
        let result = animsmith_report::render_comparison(
            comparison_side(
                &long_before,
                &long_before_grids,
                &long_roles,
                &stance_checks,
                &config,
                "walk",
            ),
            comparison_side(
                &long_after,
                &long_after_grids,
                &long_roles,
                &[],
                &config,
                "walk",
            ),
            full(),
        );
        if frames == CONTEXT_LIMIT - 2 {
            result.expect("the exact diagnostic-context work limit renders");
        } else {
            assert_eq!(
                result.unwrap_err(),
                animsmith_report::ComparisonError::ReportRowsExceeded {
                    side: "before",
                    kind: "diagnostic contexts",
                    found: CONTEXT_LIMIT + 1,
                    limit: CONTEXT_LIMIT,
                }
            );
        }
    }
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
        full(),
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
    assert!(html.contains("× relative to the trail panels"));
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
    let provenance = prediction_provenance_for(&source);
    (source, provenance)
}

fn prediction_provenance_for(source: &animsmith_core::LoadedSource) -> PredictionProvenanceV1 {
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
    animsmith_engine::project_prediction_provenance_v1(&resolved, source)
        .expect("same-load provenance projects")
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
fn comparison_binds_prediction_provenance_to_each_exact_side() {
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let before_provenance = prediction_provenance_for(&before_source);
    let after_provenance = prediction_provenance_for(&after_source);
    let before_checks = vec![prediction_check(&before_provenance, true, false)];
    let after_checks = vec![prediction_check(&after_provenance, true, false)];
    let render = |before_checks: &[CheckEvaluation],
                  before_provenance: Option<&PredictionProvenanceV1>,
                  after_checks: &[CheckEvaluation],
                  after_provenance: Option<&PredictionProvenanceV1>| {
        animsmith_report::render_comparison(
            comparison_side_with_provenance(
                &before_source,
                &before_grids,
                &roles,
                before_checks,
                &config,
                before_provenance,
                "acceptance-matrix",
            ),
            comparison_side_with_provenance(
                &after_source,
                &after_grids,
                &roles,
                after_checks,
                &config,
                after_provenance,
                "acceptance-matrix",
            ),
            full(),
        )
    };
    render(
        &before_checks,
        Some(&before_provenance),
        &after_checks,
        Some(&after_provenance),
    )
    .expect("exact side provenance renders");

    for side in ["before", "after"] {
        let result = if side == "before" {
            render(&before_checks, None, &after_checks, Some(&after_provenance))
        } else {
            render(
                &before_checks,
                Some(&before_provenance),
                &after_checks,
                None,
            )
        };
        assert!(matches!(
            result,
            Err(animsmith_report::ComparisonError::PredictionAuthorityMismatch {
                side: found,
                detail: "prediction attachment has no supplied provenance"
            }) if found == side
        ));

        let result = if side == "before" {
            render(
                &before_checks,
                Some(&after_provenance),
                &after_checks,
                Some(&after_provenance),
            )
        } else {
            render(
                &before_checks,
                Some(&before_provenance),
                &after_checks,
                Some(&before_provenance),
            )
        };
        assert!(matches!(
            result,
            Err(animsmith_report::ComparisonError::PredictionAuthorityMismatch {
                side: found,
                detail: "provenance dependency closure differs from the loaded source"
            }) if found == side
        ));

        let result = if side == "before" {
            render(
                &after_checks,
                Some(&before_provenance),
                &after_checks,
                Some(&after_provenance),
            )
        } else {
            render(
                &before_checks,
                Some(&before_provenance),
                &before_checks,
                Some(&after_provenance),
            )
        };
        assert!(matches!(
            result,
            Err(animsmith_report::ComparisonError::PredictionAuthorityMismatch {
                side: found,
                detail: "prediction attachment identity differs from supplied provenance"
            }) if found == side
        ));
    }
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
    let provenance = prediction_provenance_for(&before_source);
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
        comparison_side_with_provenance(
            &before_source,
            &before_grids,
            &roles,
            &checks,
            &config,
            Some(&provenance),
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
        full(),
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

    let html = animsmith_report::render(&grids, &roles, &checks, None, None, full());
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

    let html = animsmith_report::render(&grids, &roles, &checks, None, None, full());
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

    let html = animsmith_report::render(&grids, &roles, &checks, None, Some("missing"), full());
    assert_self_contained(&html);
    let data = report_data(&html);
    assert_eq!(
        data["clips"].as_array().expect("clips array").len(),
        0,
        "unknown --clip filter excludes every pose grid"
    );

    for name in ["walk", "idle"] {
        let html = animsmith_report::render(&grids, &roles, &checks, None, Some(name), full());
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
        let html =
            animsmith_report::render(&grids, &roles, &checks, Some(&provenance), None, full());
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

// ---------------------------------------------------------------------------
// Generated-document contracts: one token set, one shared runtime, charts that
// survive extraction, and an evidence-only form that carries no motion.
// ---------------------------------------------------------------------------

/// Token names in the order both value tables use.
const TOKEN_NAMES: [&str; 11] = [
    "ground", "surface", "raised", "ink", "muted", "line", "accent", "error", "warning", "pass",
    "note",
];
const DARK_TOKENS: [&str; 11] = [
    "#17171f", "#1e1e2a", "#232331", "#d5d9e5", "#9099b2", "#3a3a4e", "#7aa2f7", "#f7768e",
    "#e0af68", "#9ece6a", "#bb9af7",
];
const LIGHT_TOKENS: [&str; 11] = [
    "#f4f5f9", "#ffffff", "#eef0f6", "#1a1e2c", "#5b6382", "#d9deea", "#3b67d6", "#cf3f5b",
    "#946414", "#287a3b", "#6b7390",
];

/// Everything the browser executes or styles: the embedded JSON payload is
/// asset-derived text, not part of the document's own paint.
fn document_code(html: &str) -> String {
    let mut kept = String::new();
    let mut rest = html;
    while let Some(open) = rest.find("<script type=\"application/json\"") {
        kept.push_str(&rest[..open]);
        let close = rest[open..].find("</script>").expect("data script closes") + open;
        rest = &rest[close..];
    }
    kept.push_str(rest);
    kept
}

/// Every colour literal, in any CSS hex form. Short and alpha forms are
/// reported as themselves so a `#abc` or `#rrggbbaa` cannot slip past a
/// six-digit comparison by being invisible to it.
fn hex_colours(source: &str) -> std::collections::BTreeSet<String> {
    let bytes = source.as_bytes();
    let mut found = std::collections::BTreeSet::new();
    for (index, _) in source.match_indices('#') {
        let digits = bytes[index + 1..]
            .iter()
            .take_while(|byte| byte.is_ascii_hexdigit())
            .count();
        if [3, 4, 6, 8].contains(&digits) {
            found.insert(source[index..index + 1 + digits].to_ascii_lowercase());
        }
    }
    found
}

/// The `{ … }` body of the first rule whose selector contains `selector`,
/// with the byte range it occupies in `code`.
fn rule_block(code: &str, selector: &str) -> (usize, usize) {
    let at = code
        .find(selector)
        .unwrap_or_else(|| panic!("stylesheet declares {selector}"));
    let open = code[at..].find('{').expect("rule opens") + at;
    let mut depth = 0usize;
    for (offset, byte) in code[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return (at, open + offset + 1);
                }
            }
            _ => {}
        }
    }
    panic!("rule {selector} never closes");
}

/// Stylesheet text with its `/* … */` comments removed, so prose about a
/// rule cannot be read as part of the rule.
fn without_block_comments(source: &str) -> String {
    let mut kept = String::new();
    let mut rest = source;
    while let Some(open) = rest.find("/*") {
        kept.push_str(&rest[..open]);
        let close = rest[open..]
            .find("*/")
            .map_or(rest.len(), |at| open + at + 2);
        rest = &rest[close..];
    }
    kept.push_str(rest);
    kept
}

/// The opening tag carrying `id`, plus any text up to its closing tag.
fn element_with_id(html: &str, id: &str) -> String {
    let key = format!("id=\"{id}\"");
    let at = html
        .find(&key)
        .unwrap_or_else(|| panic!("no element carries id {id}"));
    let start = html[..at].rfind('<').expect("tag opens");
    let end = html[start..].find('>').expect("tag closes") + start + 1;
    let tail = &html[end..];
    let text = tail.find('<').unwrap_or(0);
    format!("{}{}", &html[start..end], &tail[..text])
}

fn has_id(html: &str, id: &str) -> bool {
    html.contains(&format!("id=\"{id}\""))
}

fn chart_roles_fixture() -> animsmith_core::Document {
    let mut doc = animsmith_gltf::load(&fixture()).expect("fixture loads");
    let mut right_foot = doc.skeleton.bones[2].clone();
    right_foot.name = "right_foot".into();
    doc.skeleton.bones.push(right_foot);
    doc
}

fn comparison_documents(options: animsmith_report::ReportOptions) -> String {
    let before_source = animsmith_gltf::load_source(&comparison_fixture("before")).unwrap();
    let after_source = animsmith_gltf::load_source(&comparison_fixture("after")).unwrap();
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    animsmith_report::render_comparison(
        comparison_side(
            &before_source,
            &before_grids,
            &roles,
            &[],
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
        options,
    )
    .expect("comparison renders")
}

fn themed_documents() -> Vec<(&'static str, String)> {
    let doc = chart_roles_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    vec![
        (
            "single-clip",
            animsmith_report::render(&grids, &roles, &[], None, None, full()),
        ),
        ("comparison", comparison_documents(full())),
    ]
}

#[test]
fn both_reports_paint_only_from_the_shared_token_set() {
    let expected: std::collections::BTreeSet<String> = DARK_TOKENS
        .iter()
        .chain(LIGHT_TOKENS.iter())
        .map(|value| (*value).to_owned())
        .collect();
    for (kind, html) in themed_documents() {
        assert_eq!(
            hex_colours(&document_code(&html)),
            expected,
            "{kind} report must resolve every colour through the shared tokens"
        );
    }
}

#[test]
fn the_runtime_fallback_palette_is_emitted_from_the_stylesheet() {
    // The viewers paint through the tokens, but a browser that cannot resolve
    // a custom property falls back to a table inside the runtime. That table
    // is not written twice: the report substitutes the stylesheet's own dark
    // values into the runtime as it emits the document.
    for (kind, html) in themed_documents() {
        let code = document_code(&html);
        assert!(
            !code.contains("__ANIMSMITH_DARK_TOKENS__"),
            "{kind}: the fallback placeholder is always substituted"
        );
        let start = code
            .find("ANIMSMITH_DEFAULT_PALETTE = ")
            .expect("runtime declares its fallback palette")
            + "ANIMSMITH_DEFAULT_PALETTE = ".len();
        let end = code[start..].find(";\n").expect("declaration ends") + start;
        let fallback: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&code[start..end]).expect("the fallback palette is a JS object");

        let (dark_start, dark_end) = rule_block(&code, ":root {");
        let declared: std::collections::BTreeMap<String, String> = code[dark_start..dark_end]
            .split_once('{')
            .expect("rule opens")
            .1
            .split(';')
            .filter_map(|declaration| declaration.split_once(':'))
            .filter_map(|(name, value)| {
                Some((
                    name.trim().strip_prefix("--")?.to_owned(),
                    value.trim().to_owned(),
                ))
            })
            .collect();
        assert_eq!(
            fallback, declared,
            "{kind}: the runtime falls back to exactly the stylesheet's dark tokens"
        );
        assert_eq!(
            declared.len(),
            TOKEN_NAMES.len(),
            "{kind}: every token has a dark value"
        );
        for (name, value) in TOKEN_NAMES.iter().zip(DARK_TOKENS) {
            assert_eq!(
                declared.get(*name).map(String::as_str),
                Some(value),
                "--{name}"
            );
        }
    }
}

#[test]
fn light_values_live_only_in_the_two_guarded_blocks() {
    for (kind, html) in themed_documents() {
        let code = document_code(&html);
        let (_, bare_end) = rule_block(&code, ":root {");
        let (media_start, media_end) = rule_block(&code, "@media (prefers-color-scheme: light)");
        let (pinned_start, pinned_end) = rule_block(&code, ":root[data-theme=\"light\"]");
        assert!(
            bare_end <= media_start && media_end <= pinned_start,
            "{kind}: dark defaults, then the scheme query, then the explicit pin"
        );
        assert!(
            code[media_start..media_end].contains(":root:not([data-theme=\"dark\"])"),
            "{kind}: a document pinned to dark ignores a light system scheme"
        );
        for (name, light) in TOKEN_NAMES.iter().zip(LIGHT_TOKENS) {
            let declaration = format!("--{name}: {light}");
            let places: Vec<usize> = code.match_indices(&declaration).map(|(at, _)| at).collect();
            assert_eq!(places.len(), 2, "{kind}: --{name} is set light twice");
            assert!(
                (media_start..media_end).contains(&places[0])
                    && (pinned_start..pinned_end).contains(&places[1]),
                "{kind}: --{name}'s light value only ever appears inside the two guarded blocks"
            );
        }
    }
}

#[test]
fn embed_hides_the_same_chrome_in_both_documents_and_hides_no_evidence() {
    for (kind, html) in themed_documents() {
        let code = without_block_comments(&document_code(&html));
        let mut hidden: Vec<String> = Vec::new();
        let mut rest = code.as_str();
        while let Some(at) = rest.find("[data-embed]") {
            let selector_start = rest[..at].rfind('}').map_or(0, |end| end + 1);
            let (_, block_end) = rule_block(&rest[selector_start..], "[data-embed]");
            let rule = &rest[selector_start..selector_start + block_end];
            let (selectors, body) = rule.split_once('{').expect("rule opens");
            for evidence in [
                "findings",
                "gaps",
                "predictions",
                "chart",
                "notice",
                "side",
                "disclosure",
                "warning",
            ] {
                assert!(
                    !selectors.contains(evidence),
                    "{kind}: `#embed=1` must not touch {evidence}: {selectors}"
                );
            }
            if body.contains("display: none") || body.contains("display:none") {
                hidden.extend(
                    selectors
                        .split(',')
                        .map(|one| one.trim().replace(":root[data-embed] ", "")),
                );
            }
            rest = &rest[selector_start + block_end..];
        }
        hidden.sort();
        assert_eq!(
            hidden,
            vec![".hint".to_owned(), "header".to_owned()],
            "{kind}: `#embed=1` hides the running title and the hint, and nothing else"
        );

        // The rule only means something if the documents are shaped for it.
        let header_start = html
            .find("<header>")
            .unwrap_or_else(|| panic!("{kind}: the document has a header to hide"));
        let header_end = html[header_start..]
            .find("</header>")
            .expect("header closes")
            + header_start;
        for evidence in ["<main", "id=\"findings\"", "class=\"disclosure\""] {
            if let Some(at) = html.find(evidence) {
                assert!(
                    at > header_end,
                    "{kind}: {evidence} must sit outside the header the embed rule hides"
                );
            }
        }
    }

    // The hint is single-clip chrome: the comparison has no interaction hint
    // to hide, and its phase mapping and evidence caveats live in the
    // disclosure section instead, which the embed rule leaves alone.
    let documents = themed_documents();
    assert!(
        documents[0].1.contains("class=\"hint\""),
        "the single-clip report has a hint"
    );
    assert!(
        !documents[1].1.contains("class=\"hint\""),
        "the comparison has none"
    );
    assert!(
        documents[1].1.contains("<section class=\"disclosure\">"),
        "the comparison keeps its disclosures outside the header"
    );
}

/// Every colour-bearing declaration in the emitted stylesheets, as
/// (property, value). Custom properties are the token definitions themselves
/// and are pinned by the token tests instead.
fn colour_declarations(code: &str) -> Vec<(String, String)> {
    const PROPERTIES: [&str; 8] = [
        "color",
        "background",
        "background-color",
        "fill",
        "stroke",
        "border-color",
        "outline-color",
        "box-shadow",
    ];
    let mut found = Vec::new();
    let mut rest = code;
    while let Some(open) = rest.find("<style>") {
        let close = rest[open..].find("</style>").expect("style closes") + open;
        let sheet = without_block_comments(&rest[open + "<style>".len()..close]);
        for declaration in sheet.split([';', '{', '}']) {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let (property, value) = (property.trim(), value.trim());
            if PROPERTIES.contains(&property) {
                found.push((property.to_owned(), value.to_owned()));
            }
        }
        rest = &rest[close + "</style>".len()..];
    }
    found
}

#[test]
fn both_reports_spell_colour_one_way() {
    // A colour written as rgb()/hsl() would sit outside the token set without
    // any hex literal for the token test to catch, so the syntax itself is
    // pinned: declarations resolve to a token reference or a keyword, and the
    // documents contain no colour-function spelling at all.
    for (kind, html) in themed_documents() {
        let code = document_code(&html);
        for function in ["rgb(", "rgba(", "hsl(", "hsla("] {
            assert!(
                !code.contains(function),
                "{kind}: a colour is spelled {function}…), which no token can express"
            );
        }
        let declarations = colour_declarations(&code);
        assert!(
            declarations.len() >= 8,
            "{kind}: the sheets must actually declare colours: {declarations:?}"
        );
        for (property, value) in declarations {
            let resolved = value.starts_with("var(--")
                || ["none", "transparent", "currentColor", "inherit"].contains(&value.as_str())
                || hex_colours(&value).len() == 1;
            assert!(
                resolved,
                "{kind}: {property}: {value} is neither a token reference, a hex literal, \
                 nor a colour keyword"
            );
        }
    }
}

#[test]
fn evidence_rows_and_controls_sit_on_the_raised_token() {
    // A row or control on --ground reads as a hole cut through its panel;
    // these two use sites are why the token exists.
    for (kind, html) in themed_documents() {
        let code = without_block_comments(&document_code(&html));
        let (start, end) = rule_block(&code, ".finding {");
        assert!(
            code[start..end].contains("background: var(--raised)"),
            "{kind}: evidence rows sit on --raised"
        );
    }
    let single = &themed_documents()[0].1;
    let code = without_block_comments(&document_code(single));
    let (start, end) = rule_block(&code, "#controls select, #controls button");
    assert!(
        code[start..end].contains("background: var(--raised)"),
        "single-clip: controls sit on --raised"
    );
}

#[test]
fn both_reports_embed_one_shared_fragment_runtime() {
    for (kind, html) in themed_documents() {
        let code = document_code(&html);
        assert_eq!(
            code.matches("function animsmithFragmentOptions(").count(),
            1,
            "{kind}: exactly one fragment parser is embedded"
        );
        assert_eq!(
            code.matches("// animsmith report shared runtime").count(),
            1,
            "{kind}: the shared runtime is embedded once"
        );
        // That the viewer actually applies those options is a behaviour, and
        // the browser harness executes it; nothing is pinned by spelling here.
    }
}

/// Each `<figure class="chart">` block, which is the unit the documentation
/// site lifts out of a report.
fn chart_figures(html: &str) -> Vec<String> {
    let mut figures = Vec::new();
    let mut rest = html;
    while let Some(open) = rest.find("<figure class=\"chart\"") {
        let close = rest[open..].find("</figure>").expect("figure closes") + open + 9;
        figures.push(rest[open..close].to_owned());
        rest = &rest[close..];
    }
    figures
}

fn attribute(source: &str, name: &str) -> String {
    attribute_values(source, name)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("{name} attribute is present"))
}

fn attribute_values(source: &str, name: &str) -> Vec<String> {
    let key = format!("{name}=\"");
    let mut values = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(&key) {
        let start = at + key.len();
        let end = rest[start..].find('"').expect("attribute closes") + start;
        values.push(rest[start..end].to_owned());
        rest = &rest[end..];
    }
    values
}

/// The text content of every element carrying `class`.
fn class_texts(source: &str, class: &str) -> Vec<String> {
    let key = format!("class=\"{class}\"");
    let mut texts = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find(&key) {
        let open = rest[at..].find('>').expect("tag closes") + at + 1;
        let close = rest[open..].find('<').unwrap_or(0) + open;
        texts.push(rest[open..close].to_owned());
        rest = &rest[close..];
    }
    texts
}

/// Every `<tag …>` in a fragment, so a test can ask about one element
/// instead of about every attribute in the figure.
fn tags(source: &str) -> Vec<String> {
    source
        .split('<')
        .skip(1)
        .filter_map(|rest| rest.split_once('>').map(|(tag, _)| tag.to_owned()))
        .collect()
}

/// The box every legend entry occupies: swatch lines carry a series class,
/// labels carry `class="legend"`. The playhead is a line but not a legend.
fn legend_bounds(figure: &str) -> (f64, f64, f64, f64) {
    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);
    for tag in tags(figure) {
        let Some(class) = attribute_values(&tag, "class").into_iter().next() else {
            continue;
        };
        if class != "legend" && !(tag.starts_with("line") && class != "playhead") {
            continue;
        }
        for name in ["x", "x1", "x2"] {
            for value in attribute_values(&tag, name) {
                if let Ok(at) = value.parse::<f64>() {
                    min_x = min_x.min(at);
                    max_x = max_x.max(at);
                }
            }
        }
        for name in ["y", "y1", "y2"] {
            for value in attribute_values(&tag, name) {
                if let Ok(at) = value.parse::<f64>() {
                    min_y = min_y.min(at);
                    max_y = max_y.max(at);
                }
            }
        }
    }
    assert!(min_x <= max_x && min_y <= max_y, "the chart has a legend");
    (min_x, max_x, min_y, max_y)
}

fn number(source: &str, name: &str) -> f64 {
    attribute(source, name)
        .parse()
        .unwrap_or_else(|_| panic!("{name} is numeric"))
}

#[test]
fn charts_keep_their_sync_hooks_and_describe_themselves() {
    let doc = chart_roles_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let html = animsmith_report::render(&grids, &roles, &[], None, Some("walk"), full());
    assert_self_contained(&html);

    let figures = chart_figures(&html);
    let kinds: Vec<String> = figures
        .iter()
        .map(|figure| attribute(figure, "data-kind"))
        .collect();
    assert_eq!(kinds, vec!["gait", "rootpath"]);

    for figure in &figures {
        let kind = attribute(figure, "data-kind");
        assert_eq!(attribute(figure, "data-clip"), "walk", "{kind}");
        assert_eq!(attribute(figure, "role"), "img", "{kind}");
        let view_box: Vec<f64> = attribute(figure, "viewBox")
            .split_whitespace()
            .map(|part| part.parse().expect("viewBox number"))
            .collect();
        assert_eq!(view_box.len(), 4, "{kind}: a scalable viewBox");
        let (width, height) = (view_box[2], view_box[3]);
        assert!(width > 0.0 && height > 0.0, "{kind}");

        let described = attribute(figure, "aria-label");
        assert!(
            described.starts_with("walk — ") && described.contains(" m"),
            "{kind}: the label names the clip and states its unit: {described}"
        );
        assert!(figure.contains("<title>walk — "), "{kind}: titled");
        let axis = class_texts(figure, "axis");
        assert!(
            axis.iter().all(|label| !label.trim().is_empty()),
            "{kind}: every axis label says something"
        );
        assert!(
            axis.iter().filter(|label| label.ends_with(" m")).count() >= 2,
            "{kind}: the unit is in the label a reader sees, not only in the \
             aria description: {axis:?}"
        );

        // Colour comes from the classes alone, so an extracted figure keeps
        // its meaning under an injected style block.
        assert!(
            !figure.contains("style="),
            "{kind}: no inline style overrides the classes"
        );
        assert!(
            !figure.contains("stroke=\""),
            "{kind}: stroke comes from a class"
        );
        for fill in attribute_values(figure, "fill") {
            assert_eq!(fill, "none", "{kind}: the only fill attribute is none");
        }

        // Every label sits inside the box it describes, and the legend
        // inside the plot it labels.
        for tag in tags(figure) {
            for name in ["x", "x2", "cx"] {
                for value in attribute_values(&tag, name) {
                    if let Ok(at) = value.parse::<f64>() {
                        assert!(
                            (0.0..=width).contains(&at),
                            "{kind}: {name}={at} escapes the {width}-wide viewBox"
                        );
                    }
                }
            }
        }
        let (legend_min_x, legend_max_x, legend_min_y, legend_max_y) = legend_bounds(figure);
        assert!(
            legend_min_x >= 0.0
                && legend_max_x <= width
                && legend_min_y >= 0.0
                && legend_max_y <= height,
            "{kind}: the legend stays inside the {width}x{height} chart on every side: \
             x {legend_min_x}..{legend_max_x}, y {legend_min_y}..{legend_max_y}"
        );
    }

    let gait = &figures[0];
    for class in ["series-left", "series-right", "series-diff"] {
        assert!(gait.contains(&format!("class=\"{class}\"")), "{class}");
    }
    // The viewer places the playhead at `data-pad + data-plotw * phase`, so
    // those two numbers must describe a real rectangle inside the chart and
    // the playhead must start at its origin.
    let pad = number(gait, "data-pad");
    let plot_width = number(gait, "data-plotw");
    let width: f64 = attribute(gait, "viewBox")
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    assert!(pad > 0.0, "the plot leaves room for its y-axis labels");
    assert!(plot_width > 0.0, "the plot has width");
    assert!(pad + plot_width <= width, "the plot fits its viewBox");
    let playhead = gait
        .split_once("class=\"playhead\"")
        .expect("playhead line")
        .1;
    assert_eq!(number(playhead, "x1"), pad, "playhead starts at the origin");
    assert_eq!(number(playhead, "x2"), pad);
    let (gait_min_x, gait_max_x, gait_min_y, gait_max_y) = legend_bounds(gait);
    assert!(
        gait_min_x >= pad && gait_max_x <= pad + plot_width,
        "the legend fits the plot rectangle horizontally: {gait_min_x}..{gait_max_x} \
         within {pad}..{}",
        pad + plot_width
    );
    let playhead_top = number(playhead, "y1");
    assert!(
        gait_max_y <= playhead_top,
        "the legend sits above the plot it labels: {gait_max_y} vs {playhead_top}"
    );
    assert!(gait_min_y > 0.0, "and inside the chart: {gait_min_y}");

    let path = &figures[1];
    assert!(path.contains("class=\"root-path\""));
    assert!(path.contains("class=\"pathdot\""));
    assert!(path.contains("<template class=\"pathpoints\">"));
}

/// The vertical band one plotted series occupies, in viewBox units.
fn series_band(figure: &str, class: &str) -> (f64, f64) {
    let path = figure
        .split_once(&format!("class=\"{class}\" d=\""))
        .unwrap_or_else(|| panic!("{class} is plotted"))
        .1;
    let d = path.split_once('"').expect("the path data closes").0;
    let ys: Vec<f64> = d
        .split(['M', 'L'])
        .filter(|point| !point.is_empty())
        .map(|point| {
            point
                .split_once(',')
                .expect("x,y point")
                .1
                .parse()
                .expect("y")
        })
        .collect();
    (
        ys.iter().copied().fold(f64::MAX, f64::min),
        ys.iter().copied().fold(f64::MIN, f64::max),
    )
}

/// The vertical extent of the band `classes` occupy together.
fn shared_extent(figure: &str, classes: &[&str]) -> f64 {
    let bands: Vec<(f64, f64)> = classes
        .iter()
        .map(|class| series_band(figure, class))
        .collect();
    bands.iter().map(|band| band.1).fold(f64::MIN, f64::max)
        - bands.iter().map(|band| band.0).fold(f64::MAX, f64::min)
}

/// A hips-plus-two-feet rig whose feet swing five centimetres a metre
/// below the hips. Their difference swings ten centimetres about zero, so
/// the two signals live an order of magnitude apart: on one shared scale
/// the foot curves collapse into 4% of the plot height.
fn squashed_gait_fixture() -> animsmith_core::Document {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    let foot = |name: &str, x: f32| Bone {
        name: name.into(),
        parent: Some(0),
        rest: Transform {
            translation: Vec3::new(x, -1.0, 0.0),
            ..Transform::IDENTITY
        },
        inverse_bind: None,
    };
    let swing = |bone: usize, x: f32, sign: f32| Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: vec![0.0, 0.5, 1.0],
        values: TrackValues::Vec3s(vec![
            Vec3::new(x, -1.0, 0.0),
            Vec3::new(x, -1.0 + sign * 0.05, 0.0),
            Vec3::new(x, -1.0, 0.0),
        ]),
    };
    Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "hips".into(),
                    parent: None,
                    rest: Transform {
                        translation: Vec3::Y,
                        ..Transform::IDENTITY
                    },
                    inverse_bind: None,
                },
                foot("foot", 0.1),
                foot("right_foot", -0.1),
            ],
        },
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![swing(1, 0.1, 1.0), swing(2, -0.1, -1.0)],
        }],
        ..Document::default()
    }
}

/// Both foot curves stay readable next to a difference series that lives
/// an order of magnitude away from them.
///
/// The three series once shared one scale, so a metre of hips-relative
/// offset set the range and both feet were drawn as a flat pair of lines
/// along the bottom of every gait chart in the documentation. Each axis
/// is now scaled on its own, and the chart says which series is read
/// against the right-hand one.
#[test]
fn the_gait_chart_scales_its_two_value_axes_independently() {
    let doc = squashed_gait_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let gait = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "gait")
        .expect("gait chart");

    // The plot is 116 units tall. On one shared scale the two feet
    // together covered 10 of them and each foot 5; the difference filled
    // the rest. The feet keep sharing a scale — they are only comparable
    // that way — but that scale is now theirs alone.
    let feet = shared_extent(&gait, &["series-left", "series-right"]);
    assert!(
        feet > 100.0,
        "the two foot curves fill the plot they share: {feet} units"
    );
    for class in ["series-left", "series-right"] {
        let (top, bottom) = series_band(&gait, class);
        assert!(
            bottom - top > 50.0,
            "{class} is drawn across the plot rather than squashed into a \
             corner of it: {} units",
            bottom - top
        );
    }
    let difference = shared_extent(&gait, &["series-diff"]);
    assert!(
        difference > 100.0,
        "and the difference fills its own axis: {difference} units"
    );
    let axis = class_texts(&gait, "axis");
    // The feet share one axis spanning both their swings; the difference
    // has its own, spanning only its own.
    for expected in ["-0.95 m", "-1.05 m", "0.10 m", "0.00 m"] {
        assert!(
            axis.iter().any(|label| label == expected),
            "each axis states its own range: {expected:?} missing from {axis:?}"
        );
    }
    let legend = class_texts(&gait, "legend");
    assert_eq!(
        legend,
        vec!["L foot", "R foot", "L−R (right axis)"],
        "the legend says which series is read against the second axis"
    );
    assert!(
        attribute(&gait, "aria-label").contains("on its own right-hand axis"),
        "and so does the description a screen reader gets"
    );
}

/// A clip whose sampled positions are all non-finite still renders a
/// chart, and that chart says the samples are unavailable.
///
/// The extents were folded from `f64::MAX`/`f64::MIN` seeds, so a channel
/// that is NaN on every frame left the seeds in place and the resulting
/// negative span read as "stationary": the chart claimed the root stayed
/// at `X 179769313486231570000…000.00 m`. A non-finite channel is what the
/// `nan` check exists to report, so it is a shape a real report reaches.
#[test]
fn non_finite_samples_are_reported_as_unavailable_rather_than_plotted() {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    let nan = f32::NAN;
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "poisoned".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.5, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::splat(nan); 3]),
            }],
        }],
        ..Document::default()
    };
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let path = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");

    let axis = class_texts(&path, "axis");
    assert!(
        axis.iter()
            .any(|label| label == "root path unavailable: sampled positions are non-finite"),
        "{axis:?}"
    );
    for label in &axis {
        assert!(
            !label.contains("root stays"),
            "a non-finite root is not a stationary one: {label:?}"
        );
        assert!(
            !label.contains("e30") && !label.contains("17976931"),
            "no float seed leaks into a label: {label:?}"
        );
    }
    assert!(
        attribute(&path, "aria-label")
            .contains("not one of the 3 sampled root frames has a finite X and Z together"),
        "{}",
        attribute(&path, "aria-label")
    );
    assert!(!path.contains("NaN"), "no NaN reaches the markup: {path}");
}

/// A root track whose keyed samples carry the given `(x, y, z)` values.
fn root_path_document(values: Vec<animsmith_core::glam::Vec3>) -> animsmith_core::Document {
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };
    let times = (0..values.len())
        .map(|index| index as f32 / (values.len() - 1) as f32)
        .collect();
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
            name: "trajectory".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times,
                values: TrackValues::Vec3s(values),
            }],
        }],
        ..Document::default()
    }
}

/// The root-path figure of a document whose root carries `values`.
fn root_path_figure(values: Vec<animsmith_core::glam::Vec3>) -> String {
    let doc = root_path_document(values);
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart")
}

/// A gap in the sampled trajectory breaks the path, rather than being
/// bridged by a straight line the clip never travelled.
///
/// Dropping the unplottable frames and then joining what is left with `L`
/// draws a segment between the last sample before the hole and the first
/// one after it — a trajectory the reader is being shown as measured
/// evidence, and which no frame recorded.
#[test]
fn a_gap_in_the_root_path_starts_a_new_subpath_rather_than_being_bridged() {
    use animsmith_core::glam::Vec3;
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    // One non-finite key in the middle of nine. A frame reads
    // `lerp(key j, key j+1, 0)`, so the non-finite key at 4 makes frames 3
    // and 4 unplottable: the runs are frames 0-2 and 5-8, both long enough
    // to draw, with a hole between them.
    let figure = root_path_figure(vec![
        at(0.0, 0.0),
        at(1.0, 0.0),
        at(2.0, 0.0),
        at(3.0, 0.0),
        at(f32::NAN, f32::NAN),
        at(5.0, 1.0),
        at(6.0, 1.0),
        at(7.0, 1.0),
        at(8.0, 1.0),
    ]);
    let d = figure
        .split_once("class=\"root-path\" d=\"")
        .expect("the root path is plotted")
        .1
        .split_once('"')
        .expect("the path data closes")
        .0;
    assert!(!d.contains("NaN"), "no NaN reaches the path data: {d:?}");
    let subpaths: Vec<&str> = d.split('M').filter(|part| !part.is_empty()).collect();
    assert_eq!(
        subpaths.len(),
        2,
        "each run of plottable frames is its own subpath, rather than one \
         line drawn through the hole: {d:?}"
    );
    for subpath in &subpaths {
        assert!(subpath.contains('L'), "each subpath draws its run: {d:?}");
    }
    // The bridge the previous drawing invented: a segment from the last
    // sample before the hole straight to the first one after it.
    let last_before = subpaths[0]
        .rsplit('L')
        .next()
        .expect("the run before the hole ends somewhere");
    let first_after = subpaths[1]
        .split('L')
        .next()
        .expect("the run after the hole starts somewhere");
    assert!(
        !d.contains(&format!("{last_before}L{first_after}")),
        "the runs either side of the hole must not be joined: {d:?}"
    );
}

/// The `<circle class="pathdot" …/>` element of a root-path figure.
fn path_dot(figure: &str) -> &str {
    let start = figure
        .find("<circle class=\"pathdot\"")
        .expect("the figure carries its playhead dot");
    let end = figure[start..].find("/>").expect("the dot element closes") + start + 2;
    &figure[start..end]
}

/// One element's attributes, by name.
///
/// Substring checks on the raw markup cannot say what an element does not
/// carry: `display="none"` is satisfied by markup that also carries a
/// `style="display:block"` overriding it, and "no `cx=`" is satisfied by a
/// `transform` that moves the element instead.
fn element_attributes(element: &str) -> std::collections::BTreeMap<String, String> {
    let mut attributes = std::collections::BTreeMap::new();
    let mut rest = element;
    while let Some(equals) = rest.find("=\"") {
        let name = rest[..equals]
            .rsplit([' ', '<'])
            .next()
            .expect("an attribute has a name")
            .to_owned();
        let start = equals + 2;
        let end = rest[start..].find('"').expect("the value closes") + start;
        attributes.insert(name, rest[start..end].to_owned());
        rest = &rest[end + 1..];
    }
    attributes
}

/// The points of a root-path figure's plotted `d`, in plot coordinates.
fn plotted_points(figure: &str) -> Vec<(f64, f64)> {
    figure
        .split_once("class=\"root-path\" d=\"")
        .expect("the root path is plotted")
        .1
        .split_once('"')
        .expect("the path data closes")
        .0
        .split(['M', 'L'])
        .filter(|part| !part.is_empty())
        .map(|point| {
            let (x, y) = point.split_once(',').expect("an x,y point");
            (x.parse().expect("x"), y.parse().expect("y"))
        })
        .collect()
}

/// The `pathpoints` entries of a root-path figure, in frame order.
fn path_points(figure: &str) -> Vec<String> {
    figure
        .split_once("<template class=\"pathpoints\">")
        .expect("the figure publishes its per-frame points")
        .1
        .split_once("</template>")
        .expect("the template closes")
        .0
        .split(';')
        .map(str::to_owned)
        .collect()
}

/// A frame with no sampled position carries an explicit no-position entry,
/// and the template still holds exactly one entry per frame.
///
/// The viewer places the playhead dot by frame index, so the entries have
/// to stay aligned with the frames — but an unavailable frame used to be
/// filled in from a neighbouring frame's position, which for a leading gap
/// meant a coordinate the clip only reaches later. Both directions are the
/// same invention, so neither is done: the frame says it has no position
/// and the viewer hides the dot.
#[test]
fn an_unavailable_frame_carries_no_position_rather_than_a_neighbours() {
    use animsmith_core::glam::Vec3;
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let nan = f32::NAN;
    // The first two frames have no position; the clip only reaches x = 2
    // later, and nothing may show that coordinate before it happens.
    let figure = root_path_figure(vec![
        at(nan, nan),
        at(nan, nan),
        at(2.0, 0.0),
        at(3.0, 1.0),
        at(4.0, 1.0),
    ]);
    let points = path_points(&figure);
    assert_eq!(
        points.len(),
        5,
        "one entry per sampled frame keeps the viewer's indexing aligned: {points:?}"
    );
    assert_eq!(
        points[0], "-",
        "a frame with no position says so: {points:?}"
    );
    assert_eq!(points[1], "-", "{points:?}");
    for available in &points[2..] {
        assert!(
            available.contains(','),
            "an available frame carries its own coordinate: {points:?}"
        );
    }

    // The dot the renderer writes into the markup is what an extracted
    // chart, or a document whose script has not run, shows. Frame 0 has no
    // position, so that dot must be hidden and name no position by any
    // route — the attribute set is pinned exactly, because a `style` can
    // override `display` and a `transform` can move an element that names
    // no `cx`/`cy` at all.
    let hidden = element_attributes(path_dot(&figure));
    for named in ["style", "cx", "cy", "transform", "x", "y"] {
        assert!(
            !hidden.contains_key(named),
            "a hidden dot carries no {named}: {hidden:?}"
        );
    }
    assert_eq!(
        hidden.keys().map(String::as_str).collect::<Vec<_>>(),
        ["class", "display", "r"],
        "and nothing else at all: {hidden:?}"
    );
    assert_eq!(hidden["display"], "none", "{hidden:?}");
    assert_eq!(hidden["class"], "pathdot", "{hidden:?}");

    // The other side of the same contract: a document whose frame 0 does
    // have a position opens with the dot visible, at that frame's own
    // coordinate.
    //
    // The expectation comes from the fixture rather than from the template
    // the same renderer wrote: frames 1 and 2 are placed symmetrically
    // about frame 0 in model space, and the projection is affine with one
    // scale for both axes, so frame 0's plotted point is exactly the
    // midpoint of theirs. A renderer that wrote frame 1's coordinate into
    // the dot puts it at an end of that segment instead of its middle.
    let available = root_path_figure(vec![at(0.0, 0.0), at(-1.0, -1.0), at(1.0, 1.0)]);
    let visible = element_attributes(path_dot(&available));
    assert_eq!(
        visible.keys().map(String::as_str).collect::<Vec<_>>(),
        ["class", "cx", "cy", "r"],
        "a visible dot names its position and nothing else: {visible:?}"
    );
    let dot = (
        visible["cx"].parse::<f64>().expect("cx"),
        visible["cy"].parse::<f64>().expect("cy"),
    );
    let plotted = plotted_points(&available);
    assert_eq!(plotted.len(), 3, "the fixture plots its three frames");
    let midpoint = (
        (plotted[1].0 + plotted[2].0) / 2.0,
        (plotted[1].1 + plotted[2].1) / 2.0,
    );
    assert!(
        (dot.0 - midpoint.0).abs() < 0.05 && (dot.1 - midpoint.1).abs() < 0.05,
        "the dot opens on frame 0, the midpoint of the symmetric frames \
         either side of it: {dot:?} against {midpoint:?} of {plotted:?}"
    );
    for (frame, point) in [(1, plotted[1]), (2, plotted[2])] {
        assert!(
            (dot.0 - point.0).abs() > 1.0 || (dot.1 - point.1).abs() > 1.0,
            "the dot is frame 0's position, not frame {frame}'s: {dot:?} against {point:?}"
        );
    }
}

/// The no-position marker sits on exactly the frames that have no
/// position, wherever the hole is.
///
/// The expected indices come from the sampling rule rather than from the
/// tool: a frame sampled exactly at key `j` reads `lerp(key j, key j+1, 0)`
/// for any `j` before the last, so a non-finite key makes both the frame at
/// it and the frame before it unplottable. Frame 0 and the last frame read
/// their own key directly and are poisoned only by that key.
#[test]
fn the_no_position_marker_sits_on_exactly_the_unavailable_frames() {
    use animsmith_core::glam::Vec3;
    let at = |x: f32, z: f32| Vec3::new(x, 0.0, z);
    let nan = f32::NAN;
    for (case, values, unavailable) in [
        (
            "a leading hole: key 0 is its own frame and poisons no other",
            vec![at(nan, nan), at(1.0, 0.0), at(2.0, 0.0), at(3.0, 1.0)],
            vec![0usize],
        ),
        (
            "a hole in the middle: key 1 poisons frame 1 only, because \
             frame 0 reads key 0 directly",
            vec![at(0.0, 0.0), at(nan, nan), at(2.0, 0.0), at(3.0, 1.0)],
            vec![1],
        ),
        (
            "a trailing hole: the last key is read by the last frame and by \
             the one before it",
            vec![at(0.0, 0.0), at(1.0, 0.0), at(2.0, 0.0), at(nan, nan)],
            vec![2, 3],
        ),
        (
            "finite in one coordinate only: still no position",
            vec![at(0.0, 0.0), at(1.0, nan), at(2.0, 0.0), at(3.0, 1.0)],
            vec![1],
        ),
    ] {
        let figure = root_path_figure(values);
        let points = path_points(&figure);
        assert_eq!(points.len(), 4, "{case}: {points:?}");
        for (index, entry) in points.iter().enumerate() {
            if unavailable.contains(&index) {
                assert_eq!(
                    entry, "-",
                    "{case}: frame {index} has no position, so it carries the \
                     marker and not a coordinate: {points:?}"
                );
                continue;
            }
            assert_ne!(
                entry, "-",
                "{case}: frame {index} has a position, so it must carry it: {points:?}"
            );
            assert!(
                entry.split(',').count() == 2
                    && entry.split(',').all(|part| part.parse::<f64>().is_ok()),
                "{case}: an available frame carries a plain coordinate pair: {points:?}"
            );
        }
        assert!(!figure.contains("NaN"), "{case}: no NaN reaches the markup");
    }
}

/// A root that is never jointly finite renders as unavailable instead of
/// panicking, even when each coordinate on its own has finite samples.
///
/// The extents were taken per coordinate, so a track finite in X on one
/// frame and in Z on the next produced a finite range for both — and then
/// the first jointly finite sample the plot needs did not exist. An
/// `expect` on that is a panic on untrusted input: a malformed GLB is
/// exactly where alternating non-finite components come from.
#[test]
fn alternating_finite_coordinates_render_unavailable_without_panicking() {
    use animsmith_core::glam::Vec3;
    let nan = f32::NAN;
    let figure = root_path_figure(vec![
        Vec3::new(0.0, 0.0, nan),
        Vec3::new(nan, 0.0, 1.0),
        Vec3::new(2.0, 0.0, nan),
        Vec3::new(nan, 0.0, 3.0),
    ]);
    let axis = class_texts(&figure, "axis");
    assert!(
        axis.iter()
            .any(|label| label == "root path unavailable: sampled positions are non-finite"),
        "{axis:?}"
    );
    assert!(
        !figure.contains("NaN"),
        "no NaN reaches the markup: {figure}"
    );
    for label in &axis {
        assert!(
            !label.contains("root stays"),
            "a root with no jointly finite sample is not a stationary one: {label:?}"
        );
    }
}

/// A series that is non-finite on every frame is not plotted, and the
/// chart says so instead of printing `NaN m` in its gutter.
#[test]
fn an_all_non_finite_series_is_named_rather_than_plotted_as_nan() {
    let mut doc = squashed_gait_fixture();
    // The right foot is NaN throughout, so `L−R` is NaN throughout too.
    let track = doc.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.bone == 2)
        .expect("right foot track");
    track.values =
        animsmith_core::model::TrackValues::Vec3s(vec![
            animsmith_core::glam::Vec3::splat(f32::NAN);
            3
        ]);
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let gait = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "gait")
        .expect("gait chart");

    for label in class_texts(&gait, "axis") {
        assert!(!label.contains("NaN"), "a gutter label reads {label:?}");
    }
    assert!(!gait.contains("NaN"), "no NaN reaches the markup: {gait}");
    assert!(
        attribute(&gait, "aria-label").contains("has no finite sample and is not plotted"),
        "{}",
        attribute(&gait, "aria-label")
    );
    // The left foot is still finite, so its curve is still drawn.
    assert!(gait.contains("class=\"series-left\" d=\"M"), "{gait}");
}

/// A series that never changes is centred and labelled once, rather than
/// pinned to the bottom row with the same number in both gutter labels.
///
/// Two feet exactly in phase make `L−R` identically zero, which a real
/// clip does; the zero-span clamp turned that into a flat line along the
/// axis captioned `0.00 m` above `0.00 m`.
#[test]
fn a_flat_series_is_centred_and_labelled_once() {
    let mut doc = squashed_gait_fixture();
    // Both feet swing together, so their difference is exactly zero.
    let mirrored = doc.clips[0]
        .tracks
        .iter()
        .find(|track| track.bone == 1)
        .expect("left foot track")
        .values
        .clone();
    let animsmith_core::model::TrackValues::Vec3s(left) = mirrored else {
        panic!("the fixture keys translations")
    };
    let track = doc.clips[0]
        .tracks
        .iter_mut()
        .find(|track| track.bone == 2)
        .expect("right foot track");
    track.values = animsmith_core::model::TrackValues::Vec3s(
        left.iter()
            .map(|value| animsmith_core::glam::Vec3::new(-0.1, value.y, value.z))
            .collect(),
    );
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let gait = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "gait")
        .expect("gait chart");

    let axis = class_texts(&gait, "axis");
    assert_eq!(
        axis.iter()
            .filter(|label| label.starts_with("flat "))
            .count(),
        1,
        "a flat axis is labelled exactly once: {axis:?}"
    );
    assert!(axis.iter().any(|label| label == "flat 0.00 m"), "{axis:?}");
    // Centred, not pinned to the bottom row the unscaled clamp produced.
    let (top, bottom) = series_band(&gait, "series-diff");
    let centre = 18.0 + (150.0 - 18.0 - 16.0) / 2.0;
    assert!(
        (top - centre).abs() < 0.2 && (bottom - centre).abs() < 0.2,
        "the flat series sits on the plot's centre line: {top}..{bottom} against {centre}"
    );
    assert!(
        attribute(&gait, "aria-label").contains("flat at 0.00 m"),
        "{}",
        attribute(&gait, "aria-label")
    );
}

/// An in-place clip's root path says the root does not move, instead of
/// leaving an empty square captioned `X 0.00…0.00 m`.
#[test]
fn a_stationary_root_path_says_so_in_words() {
    let doc = squashed_gait_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "hips".to_string())]);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let path = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");

    let axis = class_texts(&path, "axis");
    assert!(
        axis.iter().any(|label| label == "root stays at the origin"),
        "{axis:?}"
    );
    assert!(
        axis.iter().filter(|label| label.ends_with(" m")).count() >= 2,
        "the ranges and their unit stay where every other root path puts \
         them: {axis:?}"
    );
    assert!(
        attribute(&path, "aria-label").contains("the root does not move"),
        "{}",
        attribute(&path, "aria-label")
    );
    // The figure keeps the classes the documentation extractor pins.
    assert!(path.contains("class=\"root-path\"") && path.contains("class=\"pathdot\""));

    // A root that stands still somewhere other than the origin says where.
    let mut parked = squashed_gait_fixture();
    parked.skeleton.bones[0].rest.translation = animsmith_core::glam::Vec3::new(2.0, 1.0, -3.5);
    let parked_html = animsmith_report::render(
        &MetricGrids::new(&parked),
        &ResolvedRoles::from_names(&parked.skeleton, [(Role::Root, "hips".to_string())]),
        &[],
        None,
        None,
        full(),
    );
    let parked_path = chart_figures(&parked_html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");
    let parked_axis = class_texts(&parked_path, "axis");
    assert!(
        parked_axis
            .iter()
            .any(|label| label == "root stays at X 2.00 m, Z -3.50 m"),
        "a stationary root away from the origin names where it stands: {parked_axis:?}"
    );
    assert!(
        !parked_axis.iter().any(|label| label.contains("the origin")),
        "and does not claim the origin: {parked_axis:?}"
    );
    assert!(
        attribute(&parked_path, "aria-label").contains("X 2.00 m, Z -3.50 m"),
        "{}",
        attribute(&parked_path, "aria-label")
    );

    // A root that travels keeps the plotted ranges and gains no sentence.
    let travelled = animsmith_report::render(
        &MetricGrids::new(&chart_roles_fixture()),
        &chart_roles(&chart_roles_fixture()),
        &[],
        None,
        None,
        full(),
    );
    let moving = chart_figures(&travelled)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");
    assert!(
        !class_texts(&moving, "axis")
            .iter()
            .any(|label| label.starts_with("root stays")),
        "a moving root is plotted, not narrated"
    );
}

#[test]
fn the_root_path_chart_plots_x_and_z_on_one_scale() {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };

    // Two metres along +X, then one metre along +Z. Per-axis normalization
    // would draw both legs the same length; one shared scale draws the X leg
    // exactly twice as long.
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: "corner".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.5, 1.0],
                values: TrackValues::Vec3s(vec![
                    Vec3::ZERO,
                    Vec3::new(2.0, 0.0, 0.0),
                    Vec3::new(2.0, 0.0, 1.0),
                ]),
            }],
        }],
        ..Document::default()
    };
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
    let html = animsmith_report::render(&grids, &roles, &[], None, None, full());

    let figure = chart_figures(&html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");
    let points = figure
        .split_once("<template class=\"pathpoints\">")
        .expect("plotted points")
        .1
        .split_once("</template>")
        .expect("points close")
        .0;
    let plotted: Vec<(f64, f64)> = points
        .split(';')
        .map(|point| {
            let (x, y) = point.split_once(',').expect("x,y point");
            (x.parse().expect("x"), y.parse().expect("y"))
        })
        .collect();
    let extent = |values: Vec<f64>| {
        values.iter().copied().fold(f64::MIN, f64::max)
            - values.iter().copied().fold(f64::MAX, f64::min)
    };
    let width = extent(plotted.iter().map(|point| point.0).collect());
    let height = extent(plotted.iter().map(|point| point.1).collect());
    assert!(width > 1.0, "the X leg is plotted");
    assert!(
        (width - 2.0 * height).abs() <= 0.2,
        "two metres of X must plot twice as long as one metre of Z: {width} vs {height}"
    );
}

/// Maximal runs of base64 characters over one alphabet's two extra symbols.
/// Sixteen characters is the shortest run that could carry the twelve bytes
/// of one sampled position.
fn encodable_runs<'a>(source: &'a str, extra: &[u8]) -> Vec<&'a str> {
    let encodable =
        |byte: u8| byte.is_ascii_alphanumeric() || byte == b'=' || extra.contains(&byte);
    let bytes = source.as_bytes();
    let mut runs = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        if !encodable(bytes[start]) {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && encodable(bytes[end]) {
            end += 1;
        }
        if end - start >= 16 {
            runs.push(&source[start..end]);
        }
        start = end;
    }
    runs
}

/// Every base64-looking run in a source, decoded. The search deliberately
/// does not trust the key a sample was stored under, nor the alphabet it was
/// spelled with: standard and URL-safe runs are scanned separately, each with
/// its own engine and with or without padding.
fn decoded_runs(source: &str) -> Vec<Vec<u8>> {
    use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
    let mut runs = Vec::new();
    for text in encodable_runs(source, b"+/") {
        if let Ok(decoded) = STANDARD
            .decode(text)
            .or_else(|_| STANDARD_NO_PAD.decode(text))
        {
            runs.push(decoded);
        }
    }
    for text in encodable_runs(source, b"-_") {
        if let Ok(decoded) = URL_SAFE
            .decode(text)
            .or_else(|_| URL_SAFE_NO_PAD.decode(text))
        {
            runs.push(decoded);
        }
    }
    runs
}

/// Every string value in the document's embedded JSON, unescaped, so a
/// spelling hidden behind `\u` escapes is still read as its characters.
fn json_strings(html: &str, id: &str) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => out.push(text.clone()),
            Value::Object(map) => map.values().for_each(|item| walk(item, out)),
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let mut strings = Vec::new();
    walk(&embedded_json(html, id), &mut strings);
    strings
}

/// Every payload a document carries, whatever key it sits under: base64 runs
/// anywhere in the markup, plus base64 inside any embedded JSON string.
fn document_payloads(html: &str, id: &str) -> Vec<Vec<u8>> {
    let mut payloads = decoded_runs(html);
    for value in json_strings(html, id) {
        payloads.extend(decoded_runs(&value));
    }
    payloads
}

/// Whether any payload carries this exact four-byte sample.
fn payloads_carry(payloads: &[Vec<u8>], needle: &[u8; 4]) -> bool {
    payloads
        .iter()
        .any(|payload| payload.windows(needle.len()).any(|slice| slice == needle))
}

/// Every sampled pose byte the document embeds, from any clip or any side.
fn embedded_pose_bytes(html: &str, id: &str) -> Vec<u8> {
    fn walk(value: &Value, out: &mut Vec<u8>) {
        match value {
            Value::Object(map) => {
                for (key, item) in map {
                    if key == "positions" {
                        let encoded = item.as_str().expect("positions is base64 text");
                        out.extend(
                            base64::engine::general_purpose::STANDARD
                                .decode(encoded)
                                .expect("positions decode"),
                        );
                    }
                    walk(item, out);
                }
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            _ => {}
        }
    }
    let mut bytes = Vec::new();
    walk(&embedded_json(html, id), &mut bytes);
    bytes
}

#[test]
fn an_evidence_only_report_keeps_every_finding_and_chart_without_the_motion() {
    let doc = chart_roles_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let checks = evaluations(vec![
        Finding::new("fixture-check", Severity::Warning, "fixture finding")
            .clip("walk")
            .bone("hips")
            .time(0.5),
    ]);

    let full_html = animsmith_report::render(&grids, &roles, &checks, None, None, full());
    let html = animsmith_report::render(&grids, &roles, &checks, None, None, evidence_only());
    assert_self_contained(&html);

    let full_data = report_data(&full_html);
    let data = report_data(&html);
    assert_eq!(full_data["evidence_only"], false);
    assert_eq!(data["evidence_only"], true);
    assert!(
        embedded_pose_bytes(&html, "report-data").is_empty(),
        "an evidence-only report embeds no sampled motion"
    );
    assert!(!embedded_pose_bytes(&full_html, "report-data").is_empty());
    // The single-clip form has no identity block to keep — its provenance is
    // the source path and the resolved profile — which is what the docs claim
    // for it; the comparison's per-side identities are asserted separately.
    let keys: Vec<&String> = data
        .as_object()
        .expect("report data object")
        .keys()
        .collect();
    assert!(
        keys.iter().all(|key| !key.contains("identity")),
        "the single-clip payload carries no identity block: {keys:?}"
    );
    assert!(
        data["file"].is_string() && data["profile"].is_string(),
        "it carries the file path and profile instead: {keys:?}"
    );
    for clip in data["clips"].as_array().expect("clips array") {
        assert!(
            clip.get("positions").is_none(),
            "the key is absent, not empty"
        );
        assert!(clip["frames"].as_u64().expect("frame count") > 0);
    }
    for key in [
        "file",
        "profile",
        "bones",
        "findings",
        "gaps",
        "predictions",
        "prediction_provenance",
    ] {
        assert_eq!(data[key], full_data[key], "{key} is unchanged");
    }
    let figures = chart_figures(&html);
    assert!(
        !figures.is_empty(),
        "the fixture must really render charts for this to mean anything"
    );
    assert_eq!(
        figures,
        chart_figures(&full_html),
        "the single-clip charts are the same evidence"
    );

    // The document, not the viewer, decides there is no pose view.
    assert!(!has_id(&html, "gl"), "no canvas is rendered");
    assert!(
        element_with_id(&html, "gl-notice").contains("Pose playback omitted"),
        "a notice stands where the pose view would be"
    );
    assert!(
        element_with_id(&html, "play").contains("disabled"),
        "playback is disabled"
    );
    assert!(has_id(&full_html, "gl"), "a full report keeps its canvas");
    assert!(
        !element_with_id(&full_html, "play").contains("disabled"),
        "a full report can play"
    );
    assert!(
        html.len() < full_html.len(),
        "omitting the pose grid must shrink the document"
    );
}

#[test]
fn an_evidence_only_report_carries_no_unplotted_sample() {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };

    // A bone that is neither the root nor a foot never reaches a chart, so
    // this whole coordinate triple exists only inside the sampled pose grid.
    const WITNESS: [f32; 3] = [123.456, 234.567, 345.678];
    let doc = Document {
        skeleton: Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "spine".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        },
        clips: vec![Clip {
            name: "witness".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 1,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 0.5, 1.0],
                values: TrackValues::Vec3s(vec![Vec3::from_array(WITNESS); 3]),
            }],
        }],
        ..Document::default()
    };
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
    let full_html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let html = animsmith_report::render(&grids, &roles, &[], None, None, evidence_only());

    // Every payload the document carries, not only the field the pose grid
    // used to occupy.
    let channels: Vec<[u8; 4]> = WITNESS
        .iter()
        .map(|channel| channel.to_le_bytes())
        .collect();
    let spelled: Vec<String> = WITNESS
        .iter()
        .flat_map(|channel| [format!("{channel}"), format!("{channel:.2}")])
        .collect();

    let full_payloads = document_payloads(&full_html, "report-data");
    let payloads = document_payloads(&html, "report-data");
    for needle in &channels {
        assert!(
            payloads_carry(&full_payloads, needle),
            "the fixture must really be a witness in a full report"
        );
        assert!(
            !payloads_carry(&payloads, needle),
            "an evidence-only report carries a sampled coordinate in some encoded field"
        );
    }
    for spelling in &spelled {
        assert!(
            !html.contains(spelling.as_str()),
            "the evidence-only document still spells {spelling}"
        );
        for value in json_strings(&html, "report-data") {
            assert!(
                !value.contains(spelling.as_str()),
                "an embedded JSON string still spells {spelling}"
            );
        }
    }
}

#[test]
fn an_evidence_only_comparison_drops_both_pose_grids() {
    let full_html = comparison_documents(full());
    let html = comparison_documents(evidence_only());
    assert_self_contained(&html);

    let full_data = embedded_json(&full_html, "comparison-report-data");
    let data = embedded_json(&html, "comparison-report-data");
    assert_eq!(full_data["evidence_only"], false);
    assert_eq!(data["evidence_only"], true);
    assert!(
        embedded_pose_bytes(&html, "comparison-report-data").is_empty(),
        "neither side embeds sampled motion"
    );
    assert!(!embedded_pose_bytes(&full_html, "comparison-report-data").is_empty());
    for side in ["before", "after"] {
        assert!(data[side]["clip"].get("positions").is_none(), "{side}");
        assert!(
            data[side]["clip"]["times"].is_array(),
            "{side}: judged frame times remain"
        );
        for key in [
            "identity",
            "dependency_closure_identity",
            "findings",
            "gaps",
            "contexts",
            "predictions",
            "prediction_provenance",
        ] {
            assert_eq!(data[side][key], full_data[side][key], "{side} {key}");
        }
    }
    assert_eq!(data["correspondence"], full_data["correspondence"]);

    // Every comparison panel is drawn by the viewer from the pose grid — the
    // two canvases, the shared root chart, and both sides' trajectory and
    // gait panels — so all of them are replaced by the notice rather than
    // left as boxes that could never be filled. The shared phase then has
    // nothing left to scrub.
    assert!(
        chart_figures(&html).is_empty() && chart_figures(&full_html).is_empty(),
        "a comparison has no Rust-rendered chart figures in either form"
    );
    assert!(
        !html.contains("<svg id="),
        "no comparison chart surface survives without its poses"
    );
    for surface in [
        "before-gl",
        "after-gl",
        "comparison-root-path",
        "before-path",
        "after-path",
        "before-gait",
        "after-gait",
    ] {
        assert!(!has_id(&html, surface), "{surface} is not rendered");
        assert!(
            element_with_id(&html, &format!("{surface}-notice")).contains("Pose playback omitted"),
            "{surface} is replaced by a notice"
        );
        assert!(
            has_id(&full_html, surface),
            "{surface} stands in a full report"
        );
    }
    assert!(element_with_id(&html, "scrub").contains("disabled"));
    assert!(!element_with_id(&full_html, "scrub").contains("disabled"));
    assert!(html.len() < full_html.len());
}

#[test]
fn an_evidence_only_comparison_is_not_bound_by_the_embedded_pose_budget() {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };

    // 300 bones over 10,000 judged frames is ~36 MiB of pose grid, past the
    // embedded-pose budget. An evidence-only report embeds none of it, so
    // that budget must not decide which pairs it can describe.
    const BONES: usize = 300;
    const FRAMES: usize = 10_000;
    const {
        assert!(
            BONES * FRAMES * 12 > animsmith_report::MAX_COMPARISON_POSE_BYTES,
            "the fixture must exceed the budget it is testing"
        )
    };
    let bones = (0..BONES)
        .map(|index| Bone {
            name: format!("bone{index}"),
            parent: index.checked_sub(1),
            rest: Transform::IDENTITY,
            inverse_bind: None,
        })
        .collect();
    let doc = Document {
        skeleton: Skeleton { bones },
        clips: vec![Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: (0..FRAMES)
                    .map(|frame| frame as f32 / FRAMES as f32)
                    .collect(),
                values: TrackValues::Vec3s(vec![Vec3::ZERO; FRAMES]),
            }],
        }],
        ..Document::default()
    };

    let refused = animsmith_report::preflight_comparison(&doc, "walk", &doc, "walk", full())
        .expect_err("a full report would have to embed the grid");
    assert!(
        matches!(
            refused,
            animsmith_report::ComparisonError::PoseWorkExceeded { .. }
        ),
        "{refused:?}"
    );
    animsmith_report::preflight_comparison(&doc, "walk", &doc, "walk", evidence_only())
        .expect("an evidence-only comparison embeds no grid, so the budget does not apply");
}

/// A before/after pair whose *declared* pose grid is larger than the embedded
/// budget: `bones` nodes and `frames` judged keys on each side, with distinct
/// authorities. Only the key count and the node count matter here; the
/// sampled values are zero.
fn oversized_pair(directory: &std::path::Path, bones: usize, frames: usize) -> Vec<PathBuf> {
    let mut json: Value = serde_json::from_slice(&std::fs::read(fixture()).unwrap()).unwrap();
    let mut bytes = Vec::with_capacity(frames * 16);
    for frame in 0..frames {
        bytes.extend_from_slice(&(frame as f32).to_le_bytes());
    }
    let output_offset = bytes.len();
    bytes.resize(output_offset + frames * 12, 0);
    json["buffers"][0]["uri"] = format!(
        "data:application/octet-stream;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    )
    .into();
    json["buffers"][0]["byteLength"] = bytes.len().into();
    json["bufferViews"] = serde_json::json!([
        {"buffer":0,"byteOffset":0,"byteLength":output_offset},
        {"buffer":0,"byteOffset":output_offset,"byteLength":frames * 12}
    ]);
    json["accessors"] = serde_json::json!([
        {"bufferView":0,"componentType":5126,"count":frames,"type":"SCALAR","min":[0.0],"max":[(frames - 1) as f32]},
        {"bufferView":1,"componentType":5126,"count":frames,"type":"VEC3"}
    ]);
    json["animations"] = serde_json::json!([{
        "name":"walk",
        "samplers":[{"input":0,"output":1,"interpolation":"LINEAR"}],
        "channels":[{"sampler":0,"target":{"node":0,"path":"translation"}}]
    }]);
    let nodes = json["nodes"].as_array_mut().expect("fixture nodes");
    let scene_roots: Vec<Value> = (nodes.len()..bones).map(Value::from).collect();
    for index in nodes.len()..bones {
        nodes.push(serde_json::json!({"name": format!("bone{index}")}));
    }
    let roots = json["scenes"][0]["nodes"]
        .as_array_mut()
        .expect("fixture scene");
    roots.extend(scene_roots);

    let mut paths = Vec::new();
    for marker in ["before", "after"] {
        json["asset"]["extras"] = serde_json::json!({"authority": marker});
        let path = directory.join(format!("{marker}.gltf"));
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        paths.push(path);
    }
    paths
}

#[test]
fn an_evidence_only_comparison_renders_past_the_embedded_pose_budget() {
    // The budget bounds what a document embeds. A full report of this pair
    // would have to carry the grid and is refused; the evidence-only report
    // carries none of it and must render — through `render_comparison`, not
    // only through the preflight it also calls.
    const BONES: usize = 400;
    const FRAMES: usize = 7_200;
    const {
        assert!(
            BONES * FRAMES * 12 > animsmith_report::MAX_COMPARISON_POSE_BYTES,
            "the fixture must exceed the budget it is testing"
        )
    };
    let directory = tempfile::tempdir().unwrap();
    let paths = oversized_pair(directory.path(), BONES, FRAMES);
    let before_source = animsmith_gltf::load_source(&paths[0]).unwrap();
    let after_source = animsmith_gltf::load_source(&paths[1]).unwrap();
    assert_eq!(before_source.document().skeleton.bones.len(), BONES);
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let sides = || {
        (
            comparison_side(&before_source, &before_grids, &roles, &[], &config, "walk"),
            comparison_side(&after_source, &after_grids, &roles, &[], &config, "walk"),
        )
    };

    let (before, after) = sides();
    let refused = animsmith_report::render_comparison(before, after, full())
        .expect_err("a full comparison would have to embed the grid");
    assert!(
        matches!(
            refused,
            animsmith_report::ComparisonError::PoseWorkExceeded { .. }
        ),
        "{refused:?}"
    );

    let (before, after) = sides();
    let html = animsmith_report::render_comparison(before, after, evidence_only())
        .expect("an evidence-only comparison embeds no grid, so the budget does not apply");
    let data = embedded_json(&html, "comparison-report-data");
    assert_eq!(data["evidence_only"], true);
    assert_eq!(data["before"]["clip"]["frames"], FRAMES);
    assert!(
        embedded_pose_bytes(&html, "comparison-report-data").is_empty(),
        "and it really embeds none of it"
    );
}

/// A before/after pair carrying a distinctive rest coordinate on a bone that
/// no comparison panel plots and no track animates, so that coordinate can
/// only reach a document through the sampled pose grid.
fn witness_pair(directory: &std::path::Path, witness: [f32; 3]) -> Vec<PathBuf> {
    let mut json: Value = serde_json::from_slice(&std::fs::read(fixture()).unwrap()).unwrap();
    let index = json["nodes"].as_array().expect("fixture nodes").len();
    json["nodes"]
        .as_array_mut()
        .expect("fixture nodes")
        .push(serde_json::json!({
            "name": "witness",
            "translation": [witness[0] as f64, witness[1] as f64, witness[2] as f64],
        }));
    json["scenes"][0]["nodes"]
        .as_array_mut()
        .expect("fixture scene")
        .push(Value::from(index));
    let mut paths = Vec::new();
    for marker in ["before", "after"] {
        json["asset"]["extras"] = serde_json::json!({"authority": marker});
        let path = directory.join(format!("{marker}.gltf"));
        std::fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        paths.push(path);
    }
    paths
}

#[test]
fn an_evidence_only_comparison_carries_no_unplotted_sample() {
    // The same key-agnostic search the single-clip form gets: a comparison
    // embeds two pose grids, so it has two chances to leak one.
    const WITNESS: [f32; 3] = [123.456, 234.567, 345.678];
    let directory = tempfile::tempdir().unwrap();
    let paths = witness_pair(directory.path(), WITNESS);
    let before_source = animsmith_gltf::load_source(&paths[0]).unwrap();
    let after_source = animsmith_gltf::load_source(&paths[1]).unwrap();
    assert!(
        before_source
            .document()
            .skeleton
            .bones
            .iter()
            .any(|bone| bone.name == "witness"),
        "the witness bone must reach the loaded skeleton"
    );
    let before_grids = MetricGrids::new(before_source.document());
    let after_grids = MetricGrids::new(after_source.document());
    let roles = ResolvedRoles::default();
    let config = animsmith_core::Config::default();
    let sides = || {
        (
            comparison_side(&before_source, &before_grids, &roles, &[], &config, "walk"),
            comparison_side(&after_source, &after_grids, &roles, &[], &config, "walk"),
        )
    };
    let (before, after) = sides();
    let full_html = animsmith_report::render_comparison(before, after, full()).unwrap();
    let (before, after) = sides();
    let html = animsmith_report::render_comparison(before, after, evidence_only()).unwrap();

    let full_payloads = document_payloads(&full_html, "comparison-report-data");
    let payloads = document_payloads(&html, "comparison-report-data");
    for channel in WITNESS {
        let needle = channel.to_le_bytes();
        assert!(
            payloads_carry(&full_payloads, &needle),
            "the fixture must really be a witness in a full comparison"
        );
        assert!(
            !payloads_carry(&payloads, &needle),
            "an evidence-only comparison carries a sampled coordinate in some encoded field"
        );
        for spelling in [format!("{channel}"), format!("{channel:.2}")] {
            assert!(
                !html.contains(spelling.as_str()),
                "the evidence-only comparison still spells {spelling}"
            );
            for value in json_strings(&html, "comparison-report-data") {
                assert!(
                    !value.contains(spelling.as_str()),
                    "an embedded JSON string still spells {spelling}"
                );
            }
        }
    }
}

#[test]
fn the_payload_search_reads_both_base64_alphabets() {
    // The two alphabets differ only in the characters for the 6-bit values 62
    // and 63, so a payload that produces them is the case a standard-only
    // scan cannot recover: it splits the run at every `-` and `_`.
    // At least twelve bytes, so every spelling clears the run-length floor
    // the scan uses.
    let mut payload = vec![0xFB, 0xEF, 0xBE, 0xFB, 0xEF, 0xBE];
    payload.extend(123.456f32.to_le_bytes());
    payload.extend([0xFB, 0xEF, 0xBE, 0xFB, 0xEF, 0xBE]);
    let url_safe = base64::engine::general_purpose::URL_SAFE.encode(&payload);
    assert!(
        url_safe.contains('-') || url_safe.contains('_'),
        "the fixture must exercise the alphabet difference: {url_safe}"
    );

    for encoded in [
        base64::engine::general_purpose::STANDARD.encode(&payload),
        base64::engine::general_purpose::STANDARD_NO_PAD.encode(&payload),
        url_safe.clone(),
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&payload),
    ] {
        let document = format!("<p data-sample=\"{encoded}\"></p>");
        assert!(
            decoded_runs(&document)
                .iter()
                .any(|run| run.as_slice() == payload.as_slice()),
            "the search must recover a payload spelled {encoded}"
        );
    }
}

/// The text of one figure's `<figcaption>`, which is the caption a reader
/// sees beside the picture.
fn figcaption(figure: &str) -> String {
    let open = figure
        .find("<figcaption>")
        .expect("the figure carries a caption")
        + "<figcaption>".len();
    let close = figure[open..]
        .find("</figcaption>")
        .expect("caption closes")
        + open;
    figure[open..close].to_owned()
}

/// Every chart says what a reader should look for in it.
///
/// The report is read by the game developer or artist who owns the fix, not
/// by whoever wrote the check, so each picture opens with what to look for in
/// it. What it may claim is bounded by what the run declared, which
/// `chart_guidance_follows_what_the_clip_declares` covers variant by variant;
/// this pins the shape every caption has, and the measured facts the root
/// path adds to its own.
#[test]
fn every_single_clip_chart_caption_says_what_to_look_for() {
    let doc = chart_roles_fixture();
    let grids = MetricGrids::new(&doc);
    let roles = chart_roles(&doc);
    let html = animsmith_report::render(&grids, &roles, &[], None, Some("walk"), full());
    let figures = chart_figures(&html);
    let caption = |kind: &str| {
        figcaption(
            figures
                .iter()
                .find(|figure| attribute(figure, "data-kind") == kind)
                .unwrap_or_else(|| panic!("{kind} chart")),
        )
    };

    let gait = caption("gait");
    assert!(
        gait.starts_with("walk — foot height relative to hips · what to look for: "),
        "the caption names its clip and figure, then says what to look for: {gait}"
    );
    assert!(
        !gait.contains("shaded"),
        "the single-clip gait chart draws no stance bands, so it must not \
         claim any: {gait}"
    );

    let root = caption("rootpath");
    assert!(
        root.starts_with("walk — root path (top-down) · what to look for: "),
        "{root}"
    );
    assert!(
        root.contains(
            "the dot is the current frame, the hollow circle where the track starts and the \
             square where it ends"
        ),
        "the root caption names the marks it draws: {root}"
    );
    assert!(
        root.contains(" m at its widest"),
        "the root caption states the measured extent in metres: {root}"
    );

    // No check judged this render, so neither picture may prescribe: both say
    // what the document does not declare and show the measurement instead.
    for (kind, text) in [("gait", &gait), ("rootpath", &root)] {
        assert!(
            text.contains("contract declared and no check judged this clip"),
            "{kind} claims a contract nothing declared: {text}"
        );
        // Neither sentence promises anything about how the clip will look or
        // play: the report presents checked evidence, and an absent finding
        // is not acceptance.
        for word in ["acceptable", "looks good", "correct", "approved", "quality"] {
            assert!(
                !text.to_lowercase().contains(word),
                "{kind} caption promises acceptance with {word:?}: {text}"
            );
        }
    }
}

/// A root track is marked at both ends.
///
/// One line says nothing about which way it was walked: a clip that travels
/// out and never returns and a clip that comes back over its own line draw
/// the same picture. A hollow circle where the track starts and a filled
/// square where it ends tell them apart, and the caption says in words
/// whether the two coincide.
#[test]
fn the_root_path_marks_where_the_track_starts_and_ends() {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };

    let track = |name: &str, values: Vec<Vec3>| Document {
        skeleton: Skeleton {
            bones: vec![Bone {
                name: "root".into(),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        },
        clips: vec![Clip {
            name: name.to_owned(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: (0..values.len()).map(|at| at as f32 / 4.0).collect(),
                values: TrackValues::Vec3s(values),
            }],
        }],
        ..Document::default()
    };
    let render_document = |doc: &Document| {
        let grids = MetricGrids::new(doc);
        let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
        animsmith_report::render(&grids, &roles, &[], None, None, full())
    };
    let render = |doc: &Document| {
        let html = render_document(doc);
        chart_figures(&html)
            .into_iter()
            .find(|figure| attribute(figure, "data-kind") == "rootpath")
            .expect("root path chart")
    };

    // Out along +X and back over its own line: one line, two different ends.
    let there_and_back_doc = track(
        "there-and-back",
        vec![
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
        ],
    );
    let there_and_back_html = render_document(&there_and_back_doc);
    let there_and_back = chart_figures(&there_and_back_html)
        .into_iter()
        .find(|figure| attribute(figure, "data-kind") == "rootpath")
        .expect("root path chart");
    let points: Vec<(f64, f64)> = there_and_back
        .split_once("<template class=\"pathpoints\">")
        .expect("plotted points")
        .1
        .split_once("</template>")
        .expect("points close")
        .0
        .split(';')
        .map(|point| {
            let (x, y) = point.split_once(',').expect("x,y point");
            (x.parse().expect("x"), y.parse().expect("y"))
        })
        .collect();
    let start = points.first().copied().expect("a first plotted frame");
    let end = points.last().copied().expect("a last plotted frame");

    // Each mark is drawn twice: once as its legend swatch and once on the
    // track. The legend comes first, so the plotted one is the second.
    let marked = |class: &str| -> Vec<String> {
        tags(&there_and_back)
            .into_iter()
            .filter(|tag| {
                attribute_values(tag, "class")
                    .iter()
                    .any(|name| name == class)
            })
            .collect()
    };
    let mark = |class: &str| {
        let drawn = marked(class);
        assert_eq!(
            drawn.len(),
            2,
            "{class} is drawn once in the legend and once on the track: {drawn:?}"
        );
        drawn.into_iter().next_back().expect("the plotted mark")
    };
    let circle = mark("pathstart");
    assert!(
        circle.starts_with("circle"),
        "the start mark is hollow: {circle}"
    );
    assert_eq!((number(&circle, "cx"), number(&circle, "cy")), start);
    let square = mark("pathend");
    assert!(
        square.starts_with("rect"),
        "the end mark is a square: {square}"
    );
    let side = number(&square, "width");
    assert_eq!(number(&square, "height"), side, "the end mark is square");
    let square_centre = (
        number(&square, "x") + side / 2.0,
        number(&square, "y") + side / 2.0,
    );

    // The current frame stays distinguishable from both ends. On this track
    // the two ends and the playhead dot are one coordinate, which is exactly
    // where a ring inside a square inside a dot reads as a single blob, so
    // the geometry has to keep them apart rather than the paint order.
    let dot = marked("pathdot")
        .into_iter()
        .next()
        .expect("the playhead dot");
    assert!(dot.starts_with("circle"), "{dot}");
    let dot_r = number(&dot, "r");
    assert!(
        number(&circle, "r") >= dot_r + 2.0,
        "the start ring (r {}) does not stand clear of the playhead dot (r {dot_r})",
        number(&circle, "r")
    );
    let leader = marked("pathleader");
    assert_eq!(leader.len(), 1, "a coincident end mark carries one leader");
    assert_eq!(
        (number(&leader[0], "x1"), number(&leader[0], "y1")),
        end,
        "the leader starts at the last plotted frame"
    );
    assert_eq!(
        (number(&leader[0], "x2"), number(&leader[0], "y2")),
        square_centre,
        "the leader reaches the end mark it explains"
    );
    let clearance = ((square_centre.0 - number(&dot, "cx")).powi(2)
        + (square_centre.1 - number(&dot, "cy")).powi(2))
    .sqrt()
        + side / 2.0;
    assert!(
        clearance > dot_r + 2.0,
        "the end mark lies inside the playhead dot: {clearance} against r {dot_r}"
    );
    assert!(
        !square.contains("class=\"pathdot\"") && !circle.contains("class=\"pathdot\""),
        "the dot and the two marks take their own classes, so the \
         stylesheet paints them apart"
    );
    // And the square carries the plot's own ground as a stroke, so it reads
    // as a square even where it lands on a line of its own colour.
    assert!(
        there_and_back_html.contains(".pathend { fill: var(--pass); stroke: var(--ground)"),
        "the end mark has no contrasting stroke"
    );

    // Both marks are named in the legend, beside a swatch of their own shape.
    let legend = class_texts(&there_and_back, "legend");
    assert!(
        legend.contains(&"start".to_owned()) && legend.contains(&"end".to_owned()),
        "the legend names both marks: {legend:?}"
    );

    // A track that does not return says how far short it ends.
    let travelled = render(&track(
        "travelled",
        vec![
            Vec3::ZERO,
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(1.5, 0.0, 0.0),
        ],
    ));
    assert!(
        figcaption(&travelled).contains("the track ends 1.500 m from its start"),
        "{}",
        figcaption(&travelled)
    );
    assert!(
        !figcaption(&travelled).contains("closes on itself"),
        "{}",
        figcaption(&travelled)
    );

    // A track that comes back to where it began says so instead.
    assert!(
        figcaption(&there_and_back).contains("the track closes on itself"),
        "{}",
        figcaption(&there_and_back)
    );
    assert!(
        !figcaption(&there_and_back).contains("from its start"),
        "{}",
        figcaption(&there_and_back)
    );
}

/// The comparison leads with the judged poses.
///
/// The shared root panel is one panel about one channel, and it used to open
/// the document at full width — so a fixture whose root sways two
/// centimetres read as the point of the comparison. The poses come first,
/// then the root, then the trails and the gait.
#[test]
fn the_comparison_orders_its_panels_pose_root_trails_gait() {
    let html = comparison_documents(full());
    let at = |id: &str| {
        html.find(&format!("id=\"{id}\""))
            .unwrap_or_else(|| panic!("the document renders #{id}"))
    };
    let poses = at("before-gl").max(at("after-gl"));
    let root = at("comparison-root-path");
    let trails = at("before-path").min(at("after-path"));
    let gait = at("before-gait").min(at("after-gait"));
    assert!(
        poses < root,
        "both judged poses come before the shared root panel"
    );
    assert!(root < trails, "the root panel comes before the trails");
    assert!(trails < gait, "the trails come before the gait");
    for side in ["before", "after"] {
        assert!(
            at(&format!("{side}-path")) < at(&format!("{side}-gait")),
            "{side}: its own trails come before its own gait"
        );
    }

    // The panel is as tall as the region the viewer draws into, and no
    // taller: captions are the HTML paragraph beside a panel, so a strip
    // reserved for one inside the box is empty space in what was already
    // the largest panel of the document.
    assert!(
        element_with_id(&html, "comparison-root-path").contains("viewBox=\"0 0 720 180\""),
        "{}",
        element_with_id(&html, "comparison-root-path")
    );
}

/// The shared phase is playable as well as scrubbable.
///
/// Both sides already draw whatever that one number says, so playing it runs
/// the two clips together — which is what a reader compares a repair on. The
/// control sits beside the scrub rather than inside a panel, because it
/// drives the document rather than one picture.
#[test]
fn the_comparison_can_play_its_shared_phase() {
    let html = comparison_documents(full());
    let sync = html
        .split_once("<section class=\"sync\">")
        .expect("the shared-phase controls")
        .1
        .split_once("</section>")
        .expect("the controls close")
        .0;
    assert!(
        sync.contains("<button id=\"play\"") && sync.contains("id=\"scrub\""),
        "play sits beside the scrub: {sync}"
    );
    assert!(
        !element_with_id(&html, "play").contains("disabled"),
        "a comparison with poses can play them"
    );

    // With no pose grid there is no shared phase to advance, so the control
    // is disabled in the document rather than left to fail on a press.
    let evidence = comparison_documents(evidence_only());
    assert!(
        element_with_id(&evidence, "play").contains("disabled"),
        "an evidence-only comparison leaves playback enabled"
    );
}

/// The two judged poses can be drawn in one pane instead of two.
///
/// The overlay draws the same two skeletons the panes already draw, in one
/// box, so its control belongs beside the shared phase they are drawn at
/// rather than inside a pane. The two-pane layout stays the document's
/// default: the box is emitted unchecked, and a document with no pose grid
/// disables it exactly as it disables the scrub and playback.
#[test]
fn the_comparison_can_overlay_the_after_skeleton_on_the_before_pane() {
    let html = comparison_documents(full());
    let sync = html
        .split_once("<section class=\"sync\">")
        .expect("the shared-phase controls")
        .1
        .split_once("</section>")
        .expect("the controls close")
        .0;
    assert!(
        sync.contains("id=\"overlay\""),
        "the overlay toggle sits beside the shared phase it draws at: {sync}"
    );
    let overlay = element_with_id(&html, "overlay");
    assert!(
        overlay.contains("type=\"checkbox\""),
        "the overlay is a checkbox: {overlay}"
    );
    assert!(
        !overlay.contains("checked"),
        "the two-pane layout stays the default: {overlay}"
    );
    assert!(
        !overlay.contains("disabled"),
        "a comparison carrying poses can overlay them: {overlay}"
    );
    // The control is named by the label that wraps it, so it has an
    // accessible name without an attribute repeating one, and that name is
    // the words a reader looks for rather than any text at all.
    let at = html.find("id=\"overlay\"").expect("the overlay control");
    let opens = html[..at].rfind('<').expect("its tag opens");
    assert!(
        html[..opens].ends_with("<label>"),
        "the overlay checkbox is wrapped in the label naming it: {overlay}"
    );
    let name = overlay.split_once('>').expect("the tag closes").1;
    assert_eq!(
        name.trim(),
        "Overlay after on before",
        "the overlay control's visible name: {overlay}"
    );

    // With no pose grid there is nothing to overlay, so the control is
    // disabled in the document rather than left to do nothing when it is
    // used — the same as the scrub and playback beside it.
    let evidence = comparison_documents(evidence_only());
    assert!(
        element_with_id(&evidence, "overlay").contains("disabled"),
        "an evidence-only comparison leaves the overlay enabled"
    );
}

/// A walking rig whose feet alternate and whose root travels along +X, so
/// every guidance variant below differs only in what its configuration
/// declares rather than in what the clip contains.
fn contract_fixture() -> animsmith_core::Document {
    use animsmith_core::glam::Vec3;
    use animsmith_core::model::{
        Bone, Clip, Document, Interpolation, Property, Skeleton, Track, TrackValues, Transform,
    };

    let bone = |name: &str, parent: Option<usize>| Bone {
        name: name.into(),
        parent,
        rest: Transform::IDENTITY,
        inverse_bind: None,
    };
    let times: Vec<f32> = (0..5).map(|frame| frame as f32 / 4.0).collect();
    let swing = |offset: f32| {
        TrackValues::Vec3s(
            (0..5)
                .map(|frame| {
                    let phase = (frame as f32 / 4.0 + offset) * std::f32::consts::TAU;
                    Vec3::new(frame as f32 * 0.25, 0.1 * phase.sin().abs(), 0.0)
                })
                .collect(),
        )
    };
    let track = |bone: usize, values: TrackValues| Track {
        bone,
        property: Property::Translation,
        interpolation: Interpolation::Linear,
        times: times.clone(),
        values,
    };
    Document {
        skeleton: Skeleton {
            bones: vec![
                bone("root", None),
                bone("hips", Some(0)),
                bone("left_foot", Some(1)),
                bone("right_foot", Some(1)),
            ],
        },
        clips: vec![Clip {
            name: "take".into(),
            duration_s: 1.0,
            tracks: vec![
                track(
                    0,
                    TrackValues::Vec3s(
                        (0..5)
                            .map(|frame| Vec3::new(frame as f32 * 0.25, 0.0, 0.0))
                            .collect(),
                    ),
                ),
                track(
                    1,
                    TrackValues::Vec3s(
                        (0..5)
                            .map(|frame| Vec3::new(frame as f32 * 0.25, 1.0, 0.0))
                            .collect(),
                    ),
                ),
                track(2, swing(0.0)),
                track(3, swing(0.5)),
            ],
        }],
        ..Document::default()
    }
}

/// Render `contract_fixture` under one configuration and return its chart
/// captions by kind, so a test can read what the document told its reader.
fn contract_captions(
    configure: impl FnOnce(&mut animsmith_core::Config),
    roles: &[(Role, &str)],
) -> std::collections::BTreeMap<String, String> {
    let doc = contract_fixture();
    let mut config = animsmith_core::Config::default();
    config.rig.roles = roles
        .iter()
        .map(|(role, name)| (*role, (*name).to_owned()))
        .collect();
    configure(&mut config);
    let grids = MetricGrids::new(&doc);
    let resolved = ResolvedRoles::from_names(
        &doc.skeleton,
        roles.iter().map(|(role, name)| (*role, (*name).to_owned())),
    );
    let checks = matrix_evaluations(&grids, &resolved, &config);
    let html = animsmith_report::render(&grids, &resolved, &checks, None, None, full());
    chart_figures(&html)
        .into_iter()
        .map(|figure| (attribute(&figure, "data-kind"), figcaption(&figure)))
        .collect()
}

/// Every role a walking rig resolves.
fn walking_roles() -> Vec<(Role, &'static str)> {
    vec![
        (Role::Root, "root"),
        (Role::Hips, "hips"),
        (Role::LeftFoot, "left_foot"),
        (Role::RightFoot, "right_foot"),
    ]
}

/// A caption may only tell a reader what to expect where the document says
/// what the clip owes.
///
/// The guidance used to assert universals — a travelling clip traces a
/// straight line to a declared distance, the two feet alternate with one
/// planted — and both are false for valid clips. Authored root motion that
/// curves or turns is legitimate, and an idle, a jump or any other
/// non-locomotion take has no alternating stance at all. What the report can
/// honestly say is what this run declared and judged, so each variant below
/// is the same motion under a different contract.
#[test]
fn chart_guidance_follows_what_the_clip_declares() {
    use animsmith_core::config::ClipExpectations;
    use animsmith_core::{MovementOwner, Pinned};

    // A declared loop: both pictures are read against their own endpoints.
    let looped = contract_captions(
        |config| {
            config.clips.insert(
                "take".to_owned(),
                ClipExpectations {
                    looping: Some(true),
                    ..Default::default()
                },
            );
        },
        &walking_roles(),
    );
    assert!(
        looped["rootpath"]
            .contains("this clip is declared a loop, so its root path should end where it began"),
        "{}",
        looped["rootpath"]
    );
    assert!(
        looped["gait"]
            .contains("this clip is declared a loop, so the curves should end where they began"),
        "{}",
        looped["gait"]
    );
    for caption in looped.values() {
        assert!(
            !caption.contains("straight line") && !caption.contains("alternate"),
            "a loop declaration says nothing about a route or a stride: {caption}"
        );
    }

    // Animation-owned travel at a pinned speed: the speed is the expectation,
    // and the shape the clip travels is not.
    let travelling = contract_captions(
        |config| {
            config.clips.insert(
                "take".to_owned(),
                ClipExpectations {
                    speed_mps: Some(Pinned {
                        value: 1.0,
                        tolerance: 0.5,
                    }),
                    movement_owner_xz: Some(MovementOwner::Animation),
                    ..Default::default()
                },
            );
        },
        &walking_roles(),
    );
    assert!(
        travelling["rootpath"].contains(
            "this clip declares animation-owned root travel at a pinned speed, so the path should \
             keep travelling at that speed"
        ),
        "{}",
        travelling["rootpath"]
    );
    assert!(
        travelling["rootpath"].contains("a turn is not a defect"),
        "a declared speed is not a declared route: {}",
        travelling["rootpath"]
    );
    assert!(
        !travelling["rootpath"].contains("declared a loop"),
        "{}",
        travelling["rootpath"]
    );
    // `foot-slide` judges stance on a clip that pins a speed, so the gait
    // chart may name what it found — and, drawing no bands, may not claim any.
    assert!(
        travelling["gait"].contains(
            "the foot-slide check judged stance intervals on this clip, and a foot that moves \
             horizontally during a plant is the slide it reports"
        ),
        "{}",
        travelling["gait"]
    );
    assert!(
        !travelling["gait"].contains("shaded"),
        "the single-clip chart plots no stance bands: {}",
        travelling["gait"]
    );

    // Nothing declared: the caption says so and names what was judged
    // instead of prescribing anything.
    let undeclared = contract_captions(|_| {}, &walking_roles());
    for kind in ["rootpath", "gait"] {
        assert!(
            undeclared[kind].contains("contract declared"),
            "{kind}: {}",
            undeclared[kind]
        );
        assert!(
            undeclared[kind].contains("shown as measured rather than against an expectation"),
            "{kind}: {}",
            undeclared[kind]
        );
        for prescription in [
            "should end where",
            "should keep travelling",
            "straight line",
            "alternate",
        ] {
            assert!(
                !undeclared[kind].contains(prescription),
                "{kind} prescribes {prescription:?} for a clip that declares nothing: {}",
                undeclared[kind]
            );
        }
    }
    assert!(
        undeclared["rootpath"].contains("no loop or root-motion contract declared"),
        "{}",
        undeclared["rootpath"]
    );
    assert!(
        undeclared["gait"].contains("no loop or stance contract declared"),
        "{}",
        undeclared["gait"]
    );

    // A clip with no resolved feet has no foot-height chart to caption at
    // all, and its root path still speaks only for what was declared.
    let footless = contract_captions(
        |config| {
            config.clips.insert(
                "take".to_owned(),
                ClipExpectations {
                    looping: Some(true),
                    ..Default::default()
                },
            );
        },
        &[(Role::Root, "root")],
    );
    assert!(
        !footless.contains_key("gait"),
        "a clip with no resolved feet renders no foot-height chart: {footless:?}"
    );
    assert!(
        footless["rootpath"].contains("this clip is declared a loop"),
        "{}",
        footless["rootpath"]
    );
    assert!(
        !footless["rootpath"].contains("foot"),
        "a rootless-footed clip's root caption says nothing about feet: {}",
        footless["rootpath"]
    );
}

/// The comparison's panels are viewer drawings, so the words in them come
/// from this crate through the payload rather than from a sentence the
/// JavaScript holds: one clip must read the same way in a single-clip report
/// and in a comparison.
#[test]
fn the_comparison_payload_carries_each_side_its_own_derived_guidance() {
    let html = comparison_documents(full());
    let data = embedded_json(&html, "comparison-report-data");
    for side in ["before", "after"] {
        let guidance = &data[side]["guidance"];
        for key in ["root_path", "gait"] {
            let sentence = guidance[key].as_str().unwrap_or_default();
            assert!(
                sentence.starts_with("what to look for: "),
                "{side} {key}: {sentence}"
            );
            assert!(
                !sentence.contains("straight line") && !sentence.contains("alternate"),
                "{side} {key} asserts a universal a valid clip can break: {sentence}"
            );
        }
    }
}

/// The comparison viewer maps into the boxes this document emits.
///
/// The panel geometry is written twice — as a `viewBox` here and as the
/// projection targets in `assets/comparison.js` — and nothing at runtime
/// reconciles them, so a panel resized on one side would silently draw off
/// the other's edge.
#[test]
fn the_comparison_viewer_maps_into_the_panels_this_document_emits() {
    let html = comparison_documents(full());
    let viewer = html
        .rsplit_once("<script>")
        .expect("the inline comparison viewer")
        .1;
    let box_of = |id: &str| {
        let element = element_with_id(&html, id);
        let view_box = attribute(&element, "viewBox");
        let numbers: Vec<f64> = view_box
            .split_whitespace()
            .map(|part| part.parse().expect("viewBox number"))
            .collect();
        (numbers[2], numbers[3])
    };
    let declared = |name: &str| {
        let at = viewer
            .find(&format!("const {name} = {{"))
            .unwrap_or_else(|| panic!("the viewer declares {name}"));
        let body = &viewer[at..][..viewer[at..].find('}').expect("declaration closes")];
        let number = |key: &str| -> f64 {
            let start = body
                .find(&format!("{key}: "))
                .unwrap_or_else(|| panic!("{name} declares {key}"))
                + key.len()
                + 2;
            body[start..]
                .split([',', ' '])
                .next()
                .expect("a value")
                .parse()
                .expect("a number")
        };
        (number("width"), number("height"))
    };
    assert_eq!(
        box_of("comparison-root-path"),
        declared("ROOT_PANEL"),
        "the shared root panel and the box its viewer maps into"
    );
    for id in ["before-path", "after-path"] {
        assert_eq!(
            box_of(id),
            declared("TRAIL_PANEL"),
            "{id} and the box its viewer maps into"
        );
    }
}
