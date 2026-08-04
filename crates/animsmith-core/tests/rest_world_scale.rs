use animsmith_core::config::CheckSettings;
use animsmith_core::glam::{Mat4, Quat, Vec3};
use animsmith_core::measure::{LinearTransformClassification, measure_assets};
use animsmith_core::{
    Applicability, CheckCtx, CheckEvaluation, CheckSelection, Config, ConfigValidationError,
    Document, EvaluationState, MetricGrids, ResolvedRoles, SourceNodeAsset, SourceNodeLocalRest,
    SourceSkeletonAssets, SourceSkeletonCoverage, evaluate_checks,
};

fn node(
    index: usize,
    name: &str,
    parent: Option<usize>,
    local_rest: SourceNodeLocalRest,
) -> SourceNodeAsset {
    SourceNodeAsset {
        source_node_index: index,
        name: Some(name.into()),
        parent_source_node_index: parent,
        scene_root_indices: parent.is_none().then_some(0).into_iter().collect(),
        local_rest,
    }
}

fn trs(scale: Vec3) -> SourceNodeLocalRest {
    SourceNodeLocalRest::Trs {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale,
    }
}

fn document(nodes: Vec<SourceNodeAsset>) -> Document {
    Document {
        assets: animsmith_core::model::SceneAssets {
            source_skeleton: SourceSkeletonAssets {
                coverage: SourceSkeletonCoverage::Complete,
                nodes,
                skins: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

fn config(selectors: &[&str], expected: f64, tolerance: f64) -> Config {
    let mut config = Config::default();
    config.checks.insert(
        "rest-world-scale".into(),
        CheckSettings {
            node_selectors: Some(selectors.iter().map(|value| (*value).into()).collect()),
            expected_uniform_scale: Some(expected),
            uniform_scale_tolerance: Some(tolerance),
            ..Default::default()
        },
    );
    config
}

fn default_policy_config(selectors: &[&str]) -> Config {
    let mut config = Config::default();
    config.checks.insert(
        "rest-world-scale".into(),
        CheckSettings {
            node_selectors: Some(selectors.iter().map(|value| (*value).into()).collect()),
            ..Default::default()
        },
    );
    config
}

fn evaluate(doc: &Document, config: &Config) -> CheckEvaluation {
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, config);
    evaluate_checks(&ctx, &animsmith_core::all_checks(), CheckSelection::All)
        .expect("valid built-in evaluation")
        .into_iter()
        .find(|evaluation| evaluation.check_id() == "rest-world-scale")
        .expect("rest-world-scale belongs to the full catalog")
}

#[test]
fn rest_world_scale_is_quiet_until_node_policy_is_declared() {
    let doc = document(vec![node(0, "socket", None, trs(Vec3::splat(0.01)))]);
    let evaluation = evaluate(&doc, &Config::default());

    assert_eq!(evaluation.applicability(), Applicability::NotApplicable);
    assert_eq!(evaluation.evaluation(), EvaluationState::NotEvaluated);
    assert!(evaluation.findings().is_empty());
    assert!(evaluation.gaps().is_empty());
}

#[test]
fn rest_world_scale_reports_source_transform_evidence_unavailable_as_measurement_gap() {
    let evaluation = evaluate(&Document::default(), &config(&["socket"], 1.0, 1.0e-4));

    assert_eq!(evaluation.evaluation(), EvaluationState::NotEvaluated);
    assert_eq!(evaluation.gaps().len(), 1);
    assert_eq!(
        evaluation.gaps()[0].code.as_str(),
        "measurement_unavailable"
    );
    assert!(
        evaluation.gaps()[0]
            .message
            .contains("source-node transform evidence")
    );
}

#[test]
fn rest_world_scale_classifies_local_inherited_compensated_and_affine_failures() {
    let shear = Mat4::from_cols_array(&[
        1.0, 0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    let non_finite = Mat4::from_cols_array(&[
        f32::NAN,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]);
    let doc = document(vec![
        node(0, "local-scaled", None, trs(Vec3::splat(2.0))),
        node(1, "unit-helper", None, trs(Vec3::splat(0.01))),
        node(2, "inherited", Some(1), trs(Vec3::ONE)),
        node(3, "compensated", Some(1), trs(Vec3::splat(100.0))),
        node(
            4,
            "non-uniform",
            None,
            SourceNodeLocalRest::Matrix(Mat4::from_scale(Vec3::new(1.0, 2.0, 1.0))),
        ),
        node(5, "sheared", None, SourceNodeLocalRest::Matrix(shear)),
        node(
            6,
            "reflected",
            None,
            SourceNodeLocalRest::Matrix(Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))),
        ),
        node(
            7,
            "singular",
            None,
            SourceNodeLocalRest::Matrix(Mat4::from_scale(Vec3::new(0.0, 1.0, 1.0))),
        ),
        node(
            8,
            "non-finite",
            None,
            SourceNodeLocalRest::Matrix(non_finite),
        ),
    ]);
    let config = config(
        &[
            "local-*",
            "inherited",
            "compensated",
            "non-uniform",
            "sheared",
            "reflected",
            "singular",
            "non-finite",
        ],
        1.0,
        1.0e-4,
    );
    let evaluation = evaluate(&doc, &config);

    assert_eq!(evaluation.evaluation(), EvaluationState::Partial);
    assert_eq!(evaluation.evaluated_scopes().len(), 7);
    assert_eq!(evaluation.gaps().len(), 1);
    assert_eq!(
        evaluation.gaps()[0].code.as_str(),
        "measurement_unavailable"
    );
    assert!(evaluation.gaps()[0].message.contains("#8(non-finite)"));
    assert_eq!(evaluation.findings().len(), 6);
    let serialized = evaluation
        .findings()
        .iter()
        .map(|finding| serde_json::to_value(finding).expect("finding serializes"))
        .collect::<Vec<_>>();
    assert!(serialized.iter().any(|finding| {
        finding["node"] == "#0(local-scaled)"
            && finding["measured"] == 2.0
            && finding["expected"] == 1.0
    }));
    assert!(serialized.iter().any(|finding| {
        finding["node"] == "#1(unit-helper)/#2(inherited)"
            && finding["measured"]
                .as_f64()
                .is_some_and(|value| (value - 0.01).abs() < 1.0e-8)
    }));
    assert!(
        !serialized
            .iter()
            .any(|finding| finding["node"] == "#1(unit-helper)/#3(compensated)")
    );
    for (name, evidence) in [
        (
            "#4(non-uniform)",
            "non-uniform axes=(1.000000,2.000000,1.000000)",
        ),
        ("#5(sheared)", "sheared axes=(1.000000,1.118034,1.000000)"),
        (
            "#6(reflected)",
            "reflected axes=(1.000000,1.000000,1.000000)",
        ),
        ("#7(singular)", "singular axes=(0.000000,1.000000,1.000000)"),
    ] {
        assert!(
            serialized
                .iter()
                .any(|finding| { finding["node"] == name && finding["measured"] == evidence })
        );
    }

    let measured = measure_assets(&doc);
    assert_eq!(
        measured.skeleton_nodes[3].rest_world_linear.classification,
        LinearTransformClassification::UnitOrthonormal
    );
    assert!(doc.assets.meshes.is_empty());
}

#[test]
fn rest_world_scale_reports_selector_miss_and_ambiguity_without_guessing() {
    let doc = document(vec![
        node(0, "root", None, trs(Vec3::ONE)),
        node(1, "duplicate", Some(0), trs(Vec3::ONE)),
        node(2, "duplicate", Some(0), trs(Vec3::ONE)),
        node(3, "root-extra", None, trs(Vec3::ONE)),
    ]);
    let config = config(&["root", "missing*node", "duplic*ate", "root"], 1.0, 1.0e-4);
    let evaluation = evaluate(&doc, &config);

    assert_eq!(evaluation.evaluation(), EvaluationState::Partial);
    assert_eq!(evaluation.evaluated_scopes().len(), 1);
    assert_eq!(evaluation.gaps().len(), 2);
    assert_eq!(evaluation.gaps()[0].code.as_str(), "node_selector_no_match");
    assert_eq!(
        evaluation.gaps()[1].code.as_str(),
        "node_selector_ambiguous"
    );
    assert_eq!(
        evaluation.gaps()[1].message,
        "source-node selector \"duplic*ate\" matched 2 nodes (#0(root)/#1(duplicate), #0(root)/#2(duplicate)); use a selector that resolves exactly once"
    );
    assert!(evaluation.findings().is_empty());
}

#[test]
fn rest_world_scale_tolerance_is_inclusive_and_expected_factor_is_configurable() {
    let doc = document(vec![node(0, "socket", None, trs(Vec3::splat(1.25)))]);
    assert!(
        evaluate(&doc, &config(&["socket"], 1.0, 0.25))
            .findings()
            .is_empty()
    );
    assert_eq!(
        evaluate(&doc, &config(&["socket"], 1.0, 0.249))
            .findings()
            .len(),
        1
    );
    assert!(
        evaluate(&doc, &config(&["socket"], 1.25, 0.0))
            .findings()
            .is_empty()
    );

    let below_unit = document(vec![node(0, "socket", None, trs(Vec3::splat(0.5)))]);
    assert!(
        evaluate(&below_unit, &config(&["socket"], 0.5, 0.0))
            .findings()
            .is_empty()
    );
    let scale_two = document(vec![node(0, "socket", None, trs(Vec3::splat(2.0)))]);
    assert!(
        evaluate(&scale_two, &config(&["socket"], 1.0, 1.0))
            .findings()
            .is_empty()
    );
    assert_eq!(
        evaluate(&scale_two, &config(&["socket"], 1.0, 0.99))
            .findings()
            .len(),
        1
    );
}

#[test]
fn rest_world_scale_uses_documented_defaults_when_numeric_policy_is_omitted() {
    let clean = document(vec![node(0, "socket", None, trs(Vec3::splat(1.00005)))]);
    let warning = document(vec![node(0, "socket", None, trs(Vec3::splat(1.0002)))]);
    let config = default_policy_config(&["socket"]);

    assert!(evaluate(&clean, &config).findings().is_empty());
    let evaluation = evaluate(&warning, &config);
    assert_eq!(evaluation.findings().len(), 1);
    let finding = serde_json::to_value(&evaluation.findings()[0]).expect("finding serializes");
    assert_eq!(finding["expected"], 1.0);
}

#[test]
fn rest_world_scale_reports_full_ancestry_and_the_selected_leaf_measurement() {
    let doc = document(vec![
        node(0, "root", None, trs(Vec3::splat(0.1))),
        node(1, "conversion-helper", Some(0), trs(Vec3::splat(0.1))),
        node(2, "attachment-socket", Some(1), trs(Vec3::splat(2.0))),
    ]);
    let evaluation = evaluate(&doc, &config(&["attach*socket"], 1.0, 1.0e-4));
    let finding = serde_json::to_value(&evaluation.findings()[0]).expect("finding serializes");

    assert_eq!(
        finding["node"],
        "#0(root)/#1(conversion-helper)/#2(attachment-socket)"
    );
    assert!(
        (finding["measured"]
            .as_f64()
            .expect("numeric selected-leaf factor")
            - 0.02)
            .abs()
            < 1.0e-8
    );
}

#[test]
fn rest_world_scale_retains_f64_policy_ordering() {
    let doc = document(vec![node(0, "socket", None, trs(Vec3::ONE))]);
    let evaluation = evaluate(&doc, &config(&["socket"], 1.0e300, 1.0e299));

    assert_eq!(evaluation.findings().len(), 1);
}

#[test]
fn rest_world_scale_rejects_invalid_direct_numeric_policy() {
    for (field, expected, tolerance) in [
        ("expected_uniform_scale", f64::NAN, 0.0),
        ("expected_uniform_scale", 0.0, 0.0),
        ("uniform_scale_tolerance", 1.0, -0.1),
        ("uniform_scale_tolerance", 1.0, f64::INFINITY),
    ] {
        let config = config(&["socket"], expected, tolerance);
        assert!(matches!(
            config.validate(),
            Err(ConfigValidationError::InvalidCheckSetting {
                check_id,
                field: invalid_field,
            }) if check_id == "rest-world-scale" && invalid_field == field
        ));
    }
}
