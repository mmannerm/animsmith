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
fn the_shared_runtime_falls_back_to_the_stylesheet_dark_values() {
    // The viewers paint through the tokens, but a browser that cannot resolve
    // a custom property falls back to a table inside the runtime. That table
    // is only right if it is the stylesheet's own dark set.
    let code = document_code(&themed_documents()[0].1);
    let (dark_start, dark_end) = rule_block(&code, ":root {");
    let dark = &code[dark_start..dark_end];
    let fallback_start = code
        .find("ANIMSMITH_DEFAULT_PALETTE = {")
        .expect("runtime declares its fallback palette");
    let fallback_end = code[fallback_start..].find("};").expect("palette closes") + fallback_start;
    let fallback = &code[fallback_start..fallback_end];
    for (name, value) in TOKEN_NAMES.iter().zip(DARK_TOKENS) {
        assert!(
            dark.contains(&format!("--{name}: {value}")),
            "bare :root sets --{name} to {value}"
        );
        assert!(
            fallback.contains(&format!("{name}: \"{value}\"")),
            "the runtime falls back to the stylesheet's --{name}"
        );
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
    }
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
        assert!(
            code.contains("animsmithApplyDocument(animsmithFragmentOptions(location.hash))"),
            "{kind}: the viewer applies the fragment's document switches"
        );
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

/// Every `<tag …>` in a fragment, so a test can ask about one element
/// instead of about every attribute in the figure.
fn tags(source: &str) -> Vec<String> {
    source
        .split('<')
        .skip(1)
        .filter_map(|rest| rest.split_once('>').map(|(tag, _)| tag.to_owned()))
        .collect()
}

/// The right-most edge any legend entry reaches: swatch lines carry a series
/// class, labels carry `class="legend"`.
fn legend_right_edge(figure: &str) -> f64 {
    tags(figure)
        .iter()
        .filter_map(|tag| {
            let class = attribute_values(tag, "class").into_iter().next()?;
            if class == "legend" {
                attribute_values(tag, "x").first()?.parse().ok()
            } else if tag.starts_with("line") {
                attribute_values(tag, "x2").first()?.parse().ok()
            } else {
                None
            }
        })
        .fold(f64::MIN, f64::max)
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
        assert!(figure.contains("class=\"axis\""), "{kind}: has axis labels");

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
        assert!(
            legend_right_edge(figure) < width,
            "{kind}: the legend stays inside the chart"
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
    assert!(
        legend_right_edge(gait) <= pad + plot_width,
        "the legend fits the plot rectangle: {} vs {}",
        legend_right_edge(gait),
        pad + plot_width
    );

    let path = &figures[1];
    assert!(path.contains("class=\"root-path\""));
    assert!(path.contains("class=\"pathdot\""));
    assert!(path.contains("<template class=\"pathpoints\">"));
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
    assert_eq!(
        chart_figures(&html),
        chart_figures(&full_html),
        "the metric charts are the same evidence"
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
    // this coordinate exists only inside the sampled pose grid.
    const WITNESS: f32 = 123.456;
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
                values: TrackValues::Vec3s(vec![
                    Vec3::new(WITNESS, 0.0, 0.0),
                    Vec3::new(WITNESS, 0.0, 0.0),
                    Vec3::new(WITNESS, 0.0, 0.0),
                ]),
            }],
        }],
        ..Document::default()
    };
    let grids = MetricGrids::new(&doc);
    let roles = ResolvedRoles::from_names(&doc.skeleton, [(Role::Root, "root".to_string())]);
    let full_html = animsmith_report::render(&grids, &roles, &[], None, None, full());
    let html = animsmith_report::render(&grids, &roles, &[], None, None, evidence_only());

    let witness = WITNESS.to_le_bytes();
    let carries = |bytes: &[u8]| bytes.windows(witness.len()).any(|slice| slice == witness);
    assert!(
        carries(&embedded_pose_bytes(&full_html, "report-data")),
        "the fixture must really be a witness in a full report"
    );
    assert!(
        !carries(&embedded_pose_bytes(&html, "report-data")),
        "no sampled coordinate survives in an evidence-only report"
    );
    assert!(
        !html.contains("123.456"),
        "and none survives in the document's own text"
    );
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

    // Every pose surface is replaced by its own notice, and the shared phase
    // has nothing left to scrub.
    for surface in ["before-gl", "after-gl", "comparison-root-path"] {
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
