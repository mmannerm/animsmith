//! `rest-world-scale` — explicitly selected source nodes must have the
//! configured effective rest-world uniform scale. The check consumes the
//! source-domain measurements used by `measure`; it does not infer units from
//! geometry or reimplement affine classification.

use std::collections::{BTreeMap, BTreeSet};

use crate::check::{Check, CheckCtx};
use crate::config::glob_match;
use crate::evaluation::{
    Applicability, CheckOutput, CoverageGap, CoverageGapCode, EvaluationScope, EvaluationScopeCode,
};
use crate::finding::{Finding, Severity};
use crate::measure::{
    LinearTransformClassification, LinearTransformMeasurements, SkeletonNodeMeasurements,
    SkeletonSourceCoverage, measure_source_skeleton,
};

/// Default expected effective rest-world uniform scale.
pub const DEFAULT_EXPECTED_UNIFORM_SCALE: f64 = 1.0;
/// Default inclusive absolute tolerance for the expected uniform factor.
pub const DEFAULT_UNIFORM_SCALE_TOLERANCE: f64 = 1.0e-4;

pub struct RestWorldScale;

impl Check for RestWorldScale {
    fn id(&self) -> &'static str {
        "rest-world-scale"
    }

    fn applicability(&self, ctx: &CheckCtx) -> Applicability {
        if ctx
            .config
            .check_settings(self.id())
            .node_selectors
            .is_some_and(|selectors| !selectors.is_empty())
        {
            Applicability::Applicable
        } else {
            Applicability::NotApplicable
        }
    }

    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput {
        let settings = ctx.config.check_settings(self.id());
        let selectors = settings
            .node_selectors
            .as_deref()
            .expect("applicability only permits non-empty node selectors");
        let expected = settings
            .expected_uniform_scale
            .unwrap_or(DEFAULT_EXPECTED_UNIFORM_SCALE);
        let tolerance = settings
            .uniform_scale_tolerance
            .unwrap_or(DEFAULT_UNIFORM_SCALE_TOLERANCE);
        let (coverage, nodes, _) = measure_source_skeleton(ctx.doc);
        let by_index = nodes
            .iter()
            .map(|node| (node.node_index, node))
            .collect::<BTreeMap<_, _>>();
        let mut findings = Vec::new();
        let mut scopes = Vec::new();
        let mut gaps = Vec::new();
        let mut seen_selectors = BTreeSet::new();
        let mut reported_nodes = BTreeSet::new();

        for selector in selectors {
            if !seen_selectors.insert(selector.as_str()) {
                continue;
            }
            let scope = EvaluationScope::new(EvaluationScopeCode::SELECTED_NODE_REST_SCALE)
                .subject(selector.clone());
            if coverage == SkeletonSourceCoverage::Unavailable {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        "selected-node rest-world scale cannot be checked because source-node transform evidence is unavailable",
                    )
                    .scope(scope),
                );
                continue;
            }

            let matches = nodes
                .iter()
                .filter(|node| {
                    node.name
                        .as_deref()
                        .is_some_and(|name| glob_match(selector, name))
                })
                .collect::<Vec<_>>();
            let node = match matches.as_slice() {
                [] => {
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::NODE_SELECTOR_NO_MATCH,
                            format!("source-node selector {selector:?} matched no named node"),
                        )
                        .scope(scope),
                    );
                    continue;
                }
                [node] => *node,
                _ => {
                    let identities = matches
                        .iter()
                        .map(|node| source_node_path(node, &by_index))
                        .collect::<Vec<_>>()
                        .join(", ");
                    gaps.push(
                        CoverageGap::new(
                            CoverageGapCode::NODE_SELECTOR_AMBIGUOUS,
                            format!(
                                "source-node selector {selector:?} matched {} nodes ({identities}); use a selector that resolves exactly once",
                                matches.len()
                            ),
                        )
                        .scope(scope),
                    );
                    continue;
                }
            };

            if node.rest_world_linear.classification == LinearTransformClassification::NonFinite {
                gaps.push(
                    CoverageGap::new(
                        CoverageGapCode::MEASUREMENT_UNAVAILABLE,
                        format!(
                            "rest-world linear transform is unavailable for source node {} ({})",
                            source_node_path(node, &by_index),
                            node.rest_world_matrix_unavailable_reason
                                .map(|reason| format!("{reason:?}"))
                                .unwrap_or_else(|| "unclassified source evidence".into())
                        ),
                    )
                    .scope(scope),
                );
                continue;
            }

            scopes.push(scope);
            if reported_nodes.insert(node.node_index)
                && let Some(finding) = evaluate_node(node, &by_index, expected, tolerance)
            {
                findings.push(finding);
            }
        }

        CheckOutput::from_coverage(findings, scopes, gaps)
    }
}

fn evaluate_node(
    node: &SkeletonNodeMeasurements,
    by_index: &BTreeMap<usize, &SkeletonNodeMeasurements>,
    expected: f64,
    tolerance: f64,
) -> Option<Finding> {
    let linear = &node.rest_world_linear;
    let path = source_node_path(node, by_index);
    match linear.classification {
        LinearTransformClassification::UnitOrthonormal
        | LinearTransformClassification::UniformScaled => {
            let measured = linear
                .uniform_scale
                .expect("finite uniform classifications carry their factor");
            if exceeds_inclusive_f32_range(measured, expected, tolerance) {
                Some(
                    Finding::new(
                        "rest-world-scale",
                        Severity::Warning,
                        "effective rest-world uniform scale differs from selected-node policy",
                    )
                    .node(path)
                    .measured(measured)
                    .expected(expected),
                )
            } else {
                None
            }
        }
        LinearTransformClassification::NonUniform
        | LinearTransformClassification::Sheared
        | LinearTransformClassification::Reflected
        | LinearTransformClassification::Singular => Some(
            Finding::new(
                "rest-world-scale",
                Severity::Warning,
                format!(
                    "effective rest-world transform is {}; selected-node policy requires a uniform factor near {expected}",
                    classification_name(linear.classification)
                ),
            )
            .node(path)
            .measured(linear_evidence(linear))
            .expected(format!("uniform={expected} tolerance={tolerance}")),
        ),
        LinearTransformClassification::NonFinite => None,
    }
}

/// Whether an `f32`-derived scale falls outside a user-facing inclusive range.
///
/// Source matrices store `f32` components. Quantize the configured range ends
/// to that evidence precision so an authored boundary value such as `1.0001`
/// does not warn only because its exact `f32` representation is slightly
/// farther from the `f64` policy value.
fn exceeds_inclusive_f32_range(measured: f64, expected: f64, tolerance: f64) -> bool {
    let lower = expected - tolerance;
    let upper = expected + tolerance;
    let measured_f32 = measured as f32;
    let lower_f32 = lower as f32;
    let upper_f32 = upper as f32;
    if measured_f32.is_finite() && lower_f32.is_finite() && upper_f32.is_finite() {
        measured_f32 < lower_f32 || measured_f32 > upper_f32
    } else {
        measured < lower || measured > upper
    }
}

fn source_node_path(
    node: &SkeletonNodeMeasurements,
    by_index: &BTreeMap<usize, &SkeletonNodeMeasurements>,
) -> String {
    let mut components = Vec::new();
    let mut current = Some(node.node_index);
    let mut visited = BTreeSet::new();
    while let Some(index) = current {
        if !visited.insert(index) {
            break;
        }
        let Some(node) = by_index.get(&index) else {
            break;
        };
        components.push(format!(
            "#{}({})",
            node.node_index,
            node.name.as_deref().unwrap_or("<unnamed>")
        ));
        current = node.parent_node_index;
    }
    components.reverse();
    components.join("/")
}

fn linear_evidence(linear: &LinearTransformMeasurements) -> String {
    let axes = linear
        .axis_lengths
        .map(|axes| format!("axes=({:.6},{:.6},{:.6})", axes[0], axes[1], axes[2]))
        .unwrap_or_else(|| "axes=unavailable".into());
    format!("{} {axes}", classification_name(linear.classification))
}

fn classification_name(classification: LinearTransformClassification) -> &'static str {
    match classification {
        LinearTransformClassification::UnitOrthonormal => "unit orthonormal",
        LinearTransformClassification::UniformScaled => "uniform scaled",
        LinearTransformClassification::NonUniform => "non-uniform",
        LinearTransformClassification::Sheared => "sheared",
        LinearTransformClassification::Reflected => "reflected",
        LinearTransformClassification::Singular => "singular",
        LinearTransformClassification::NonFinite => "non-finite",
    }
}
