//! Pure, manifest-bound directional-speed policy evaluation V1.
//!
//! The format/CLI layer decodes current collection-output evidence strictly, then adapts
//! its retained root-travel evidence into [`CollectionDirectionalSpeedEvidenceV1`].
//! This module deliberately has no filesystem or JSON-reader authority.

use serde::Serialize;
use std::collections::BTreeSet;

use crate::{
    CollectionDirectionalSpeedDiagonalBehaviorV1, CollectionDirectionalSpeedManifestIdentityV1,
    CollectionDirectionalSpeedModeV1, CollectionDirectionalSpeedPolicyV1, CollectionLogicalIdV1,
    CollectionRuntimeSetKindV1, InputIdentity,
};

/// Schema identity for a directional-speed evaluation result.
pub const COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_ID: &str =
    "urn:animsmith:schema:collection-directional-speed-evaluation:1";
/// Schema version for a directional-speed evaluation result.
pub const COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_SCHEMA_VERSION: u32 = 1;

/// The retained root-travel lifecycle from collection-output evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
#[serde(rename_all = "snake_case")]
pub enum CollectionDirectionalSpeedLifecycleV1 {
    Complete,
    Incomplete,
}

/// Raw root-travel values for one manifest member. `None` values retain an
/// unavailable/not-applicable prerequisite without creating a subset result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedEvidenceMemberV1 {
    id: CollectionLogicalIdV1,
    /// Raw measured duration.
    pub duration_s: Option<f64>,
    /// Raw positive-X endpoint displacement.
    pub horizontal_displacement_x_m: Option<f64>,
    /// Raw positive-Z endpoint displacement.
    pub horizontal_displacement_z_m: Option<f64>,
    /// Raw sampled horizontal travel, retained but never used as speed.
    pub horizontal_travel_m: Option<f64>,
    /// Published collection speed measurement.
    pub speed_mps: Option<f64>,
}

impl CollectionDirectionalSpeedEvidenceMemberV1 {
    /// Construct a raw member row.
    pub fn new(
        id: CollectionLogicalIdV1,
        duration_s: Option<f64>,
        x: Option<f64>,
        z: Option<f64>,
        travel: Option<f64>,
        speed: Option<f64>,
    ) -> Self {
        Self {
            id,
            duration_s,
            horizontal_displacement_x_m: x,
            horizontal_displacement_z_m: z,
            horizontal_travel_m: travel,
            speed_mps: speed,
        }
    }
    /// Logical member identity.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }
    fn measured(&self) -> bool {
        self.duration_s.is_some_and(valid_nonnegative)
            && self.horizontal_displacement_x_m.is_some_and(f64::is_finite)
            && self.horizontal_displacement_z_m.is_some_and(f64::is_finite)
            && self.horizontal_travel_m.is_some_and(valid_nonnegative)
            && self.speed_mps.is_some_and(valid_nonnegative)
    }
    fn valid_partial(&self) -> bool {
        self.duration_s.is_none_or(valid_nonnegative)
            && self.horizontal_displacement_x_m.is_none_or(f64::is_finite)
            && self.horizontal_displacement_z_m.is_none_or(f64::is_finite)
            && self.horizontal_travel_m.is_none_or(valid_nonnegative)
            && self.speed_mps.is_none_or(valid_nonnegative)
    }
}

/// Strict collection evidence as consumed by the evaluator.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedEvidenceV1 {
    manifest: CollectionDirectionalSpeedManifestIdentityV1,
    runtime_set_id: CollectionLogicalIdV1,
    kind: CollectionRuntimeSetKindV1,
    lifecycle: CollectionDirectionalSpeedLifecycleV1,
    gaps: Vec<CollectionLogicalIdV1>,
    members: Vec<CollectionDirectionalSpeedEvidenceMemberV1>,
}

impl CollectionDirectionalSpeedEvidenceV1 {
    /// Construct evidence after strict collection-output decoding. The constructor rejects
    /// contradictory lifecycle/member representations; incomplete evidence is
    /// nevertheless a normal evaluator input and result.
    pub fn new(
        manifest: CollectionDirectionalSpeedManifestIdentityV1,
        runtime_set_id: CollectionLogicalIdV1,
        kind: CollectionRuntimeSetKindV1,
        lifecycle: CollectionDirectionalSpeedLifecycleV1,
        gaps: Vec<CollectionLogicalIdV1>,
        members: Vec<CollectionDirectionalSpeedEvidenceMemberV1>,
    ) -> Result<Self, CollectionDirectionalSpeedEvaluationControlError> {
        if manifest.input().bytes() > crate::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES {
            return Err(CollectionDirectionalSpeedEvaluationControlError::ContradictoryEvidence);
        }
        let ids = members
            .iter()
            .map(|member| member.id.clone())
            .collect::<BTreeSet<_>>();
        if members.len() < 2 || ids.len() != members.len() {
            return Err(CollectionDirectionalSpeedEvaluationControlError::ContradictoryEvidence);
        }
        let gap_ids = gaps.iter().cloned().collect::<BTreeSet<_>>();
        if gap_ids.len() != gaps.len() || !gap_ids.is_subset(&ids) {
            return Err(CollectionDirectionalSpeedEvaluationControlError::ContradictoryEvidence);
        }
        if members.iter().any(|member| !member.valid_partial()) {
            return Err(CollectionDirectionalSpeedEvaluationControlError::ContradictoryEvidence);
        }
        let all_measured = members
            .iter()
            .all(CollectionDirectionalSpeedEvidenceMemberV1::measured);
        if (lifecycle == CollectionDirectionalSpeedLifecycleV1::Complete
            && (!gaps.is_empty() || !all_measured))
            || (lifecycle == CollectionDirectionalSpeedLifecycleV1::Incomplete && all_measured)
        {
            return Err(CollectionDirectionalSpeedEvaluationControlError::ContradictoryEvidence);
        }
        Ok(Self {
            manifest,
            runtime_set_id,
            kind,
            lifecycle,
            gaps,
            members,
        })
    }
    /// Exact manifest identity.
    pub fn manifest(&self) -> &CollectionDirectionalSpeedManifestIdentityV1 {
        &self.manifest
    }
    /// Set id.
    pub fn runtime_set_id(&self) -> &CollectionLogicalIdV1 {
        &self.runtime_set_id
    }
    /// Set kind.
    pub const fn kind(&self) -> CollectionRuntimeSetKindV1 {
        self.kind
    }
    /// Root-travel lifecycle.
    pub const fn lifecycle(&self) -> CollectionDirectionalSpeedLifecycleV1 {
        self.lifecycle
    }
    /// Retained gaps in source order.
    pub fn gaps(&self) -> &[CollectionLogicalIdV1] {
        &self.gaps
    }
    /// Retained member evidence in manifest order.
    pub fn members(&self) -> &[CollectionDirectionalSpeedEvidenceMemberV1] {
        &self.members
    }
}

/// Typed complete/not-evaluated reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[allow(missing_docs)]
#[serde(rename_all = "snake_case")]
pub enum CollectionDirectionalSpeedNotEvaluatedReasonV1 {
    IncompleteRootTravel,
    ZeroNetDisplacement,
    ZeroReferenceSpeed,
    NumericRange,
}

/// One policy finding. Order is deterministic: manifest member order, then
/// direction, speed, ratio.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[allow(missing_docs)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CollectionDirectionalSpeedFindingV1 {
    /// Endpoint heading exceeded the inclusive angular tolerance.
    Direction {
        member_id: CollectionLogicalIdV1,
        angle_deg: f64,
        tolerance_deg: f64,
    },
    /// Published collection speed exceeded the inclusive absolute tolerance.
    Speed {
        member_id: CollectionLogicalIdV1,
        measured_speed_mps: f64,
        expected_speed_mps: f64,
        tolerance_mps: f64,
    },
    /// Published collection speed ratio exceeded the inclusive ratio tolerance.
    Ratio {
        member_id: CollectionLogicalIdV1,
        measured_ratio: f64,
        expected_ratio: f64,
        tolerance: f64,
    },
}

/// One immutable per-member comparison row. It always retains its raw
/// evidence and policy coordinate; comparison fields are absent only when the
/// set's typed lifecycle/reason makes comparison unavailable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedMemberEvaluationV1 {
    /// Semantic policy coordinate.
    pub coordinate: [f64; 2],
    /// Retained raw collection-evidence row.
    pub evidence: CollectionDirectionalSpeedEvidenceMemberV1,
    /// Normalized projected semantic heading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projected_heading: Option<[f64; 2]>,
    /// Angular difference from the declared coordinate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub angle_deg: Option<f64>,
    /// Expected speed for uniform/authored mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_speed_mps: Option<f64>,
    /// Measured/expected ratio for ratios mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured_ratio: Option<f64>,
    /// Expected ratio for ratios mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_ratio: Option<f64>,
    /// Applied inclusive speed or ratio tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude_tolerance: Option<f64>,
    /// Absolute speed or ratio deviation from the expected value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude_deviation: Option<f64>,
    /// Direction comparison passed its inclusive tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_passed: Option<bool>,
    /// Speed/ratio comparison passed its inclusive tolerance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub magnitude_passed: Option<bool>,
}

/// Immutable result. `policy_input` and `evidence_input` identify exactly the
/// raw TOML and JSON bytes, never a normalized representation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CollectionDirectionalSpeedEvaluationV1 {
    schema: &'static str,
    schema_version: u32,
    manifest: CollectionDirectionalSpeedManifestIdentityV1,
    policy_input: InputIdentity,
    evidence_input: InputIdentity,
    runtime_set_id: CollectionLogicalIdV1,
    lifecycle: CollectionDirectionalSpeedLifecycleV1,
    gaps: Vec<CollectionLogicalIdV1>,
    members: Vec<CollectionDirectionalSpeedMemberEvaluationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    not_evaluated_reason: Option<CollectionDirectionalSpeedNotEvaluatedReasonV1>,
    findings: Vec<CollectionDirectionalSpeedFindingV1>,
}

impl CollectionDirectionalSpeedEvaluationV1 {
    /// Result schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    /// Result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Exact collection-manifest identity bound by this result.
    pub fn manifest(&self) -> &CollectionDirectionalSpeedManifestIdentityV1 {
        &self.manifest
    }
    /// Exact raw policy-byte identity.
    pub const fn policy_input(&self) -> &InputIdentity {
        &self.policy_input
    }
    /// Exact raw collection-output-byte identity.
    pub const fn evidence_input(&self) -> &InputIdentity {
        &self.evidence_input
    }
    /// Evaluated directional runtime-set id.
    pub fn runtime_set_id(&self) -> &CollectionLogicalIdV1 {
        &self.runtime_set_id
    }
    /// Retained root-travel lifecycle.
    pub const fn lifecycle(&self) -> CollectionDirectionalSpeedLifecycleV1 {
        self.lifecycle
    }
    /// Retained collection-evidence gaps in source order.
    pub fn gaps(&self) -> &[CollectionLogicalIdV1] {
        &self.gaps
    }
    /// Typed not-evaluated reason, if evaluation was not possible.
    pub const fn not_evaluated_reason(
        &self,
    ) -> Option<CollectionDirectionalSpeedNotEvaluatedReasonV1> {
        self.not_evaluated_reason
    }
    /// Deterministic policy findings.
    pub fn findings(&self) -> &[CollectionDirectionalSpeedFindingV1] {
        &self.findings
    }
    /// Audit rows in manifest member order.
    pub fn members(&self) -> &[CollectionDirectionalSpeedMemberEvaluationV1] {
        &self.members
    }
}

/// Invalid/stale/wrong-kind/contradictory input. Ordinary incomplete and
/// not-evaluable sets are represented by successful typed results instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CollectionDirectionalSpeedEvaluationControlError {
    /// Raw policy or evidence identity exceeds its immutable input cap.
    #[error("directional-speed raw input identity exceeds its byte limit")]
    OverBudgetInput,
    /// Policy did not bind to the evidence manifest/set/kind/member sequence.
    #[error("policy does not bind to directional-speed evidence")]
    InvalidBinding,
    /// The strict evidence adapter was contradictory.
    #[error("collection-output root-travel evidence is contradictory")]
    ContradictoryEvidence,
}

/// Evaluate one fully declared set without I/O.
///
/// # Errors
/// Returns a control error only for invalid/stale policy binding or
/// contradictory typed evidence. Incomplete and not-evaluable coverage is a
/// successful typed result.
pub fn evaluate_collection_directional_speed_v1(
    policy: &CollectionDirectionalSpeedPolicyV1,
    policy_input: InputIdentity,
    evidence_input: InputIdentity,
    evidence: &CollectionDirectionalSpeedEvidenceV1,
) -> Result<CollectionDirectionalSpeedEvaluationV1, CollectionDirectionalSpeedEvaluationControlError>
{
    if policy_input.bytes() > crate::COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES
        || evidence_input.bytes() > crate::COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES
    {
        return Err(CollectionDirectionalSpeedEvaluationControlError::OverBudgetInput);
    }
    policy
        .validate_binding(
            evidence.manifest(),
            evidence.runtime_set_id(),
            evidence.kind(),
            &evidence
                .members()
                .iter()
                .map(|m| m.id().clone())
                .collect::<Vec<_>>(),
        )
        .map_err(|_| CollectionDirectionalSpeedEvaluationControlError::InvalidBinding)?;
    let mut result = CollectionDirectionalSpeedEvaluationV1 {
        schema: COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_ID,
        schema_version: COLLECTION_DIRECTIONAL_SPEED_EVALUATION_V1_SCHEMA_VERSION,
        manifest: evidence.manifest().clone(),
        policy_input,
        evidence_input,
        runtime_set_id: evidence.runtime_set_id().clone(),
        lifecycle: evidence.lifecycle(),
        gaps: evidence.gaps().to_vec(),
        members: policy
            .members()
            .iter()
            .zip(evidence.members())
            .map(
                |(policy, evidence)| CollectionDirectionalSpeedMemberEvaluationV1 {
                    coordinate: policy.coordinate(),
                    evidence: evidence.clone(),
                    projected_heading: None,
                    angle_deg: None,
                    expected_speed_mps: None,
                    measured_ratio: None,
                    expected_ratio: None,
                    magnitude_tolerance: None,
                    magnitude_deviation: None,
                    direction_passed: None,
                    magnitude_passed: None,
                },
            )
            .collect(),
        not_evaluated_reason: None,
        findings: Vec::new(),
    };
    if evidence.lifecycle() == CollectionDirectionalSpeedLifecycleV1::Incomplete {
        result.not_evaluated_reason =
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::IncompleteRootTravel);
        return Ok(result);
    }
    let headings = evidence
        .members()
        .iter()
        .map(|member| heading(member, policy))
        .collect::<Option<Vec<_>>>();
    let Some(headings) = headings else {
        result.not_evaluated_reason =
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroNetDisplacement);
        return Ok(result);
    };
    if let CollectionDirectionalSpeedModeV1::Ratios {
        reference_member, ..
    } = policy.mode()
    {
        let reference = evidence
            .members()
            .iter()
            .find(|member| member.id() == reference_member)
            .expect("binding checked")
            .speed_mps
            .expect("complete evidence");
        if reference == 0.0 {
            result.not_evaluated_reason =
                Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroReferenceSpeed);
            return Ok(result);
        }
    }
    let Some(expected) = expectations(policy, evidence.members()) else {
        result.not_evaluated_reason =
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::NumericRange);
        return Ok(result);
    };
    for (index, (((member, policy_member), heading), expected)) in evidence
        .members()
        .iter()
        .zip(policy.members())
        .zip(headings)
        .zip(expected)
        .enumerate()
    {
        let coordinate = normalize(policy_member.coordinate()).expect("policy coordinate checked");
        let angle = heading
            .0
            .mul_add(coordinate[1], -heading.1 * coordinate[0])
            .abs()
            .atan2(heading.0 * coordinate[0] + heading.1 * coordinate[1])
            .to_degrees();
        let row = &mut result.members[index];
        row.projected_heading = Some([heading.0, heading.1]);
        row.angle_deg = Some(angle);
        row.direction_passed = Some(angle <= policy.direction_tolerance_deg());
        if angle > policy.direction_tolerance_deg() {
            result
                .findings
                .push(CollectionDirectionalSpeedFindingV1::Direction {
                    member_id: member.id().clone(),
                    angle_deg: angle,
                    tolerance_deg: policy.direction_tolerance_deg(),
                });
        }
        match expected {
            Expected::Speed { value, tolerance } => {
                let measured = member.speed_mps.expect("complete evidence");
                row.expected_speed_mps = Some(value);
                row.magnitude_tolerance = Some(tolerance);
                row.magnitude_deviation = Some((measured - value).abs());
                row.magnitude_passed = Some(row.magnitude_deviation.expect("set") <= tolerance);
                if (measured - value).abs() > tolerance {
                    result
                        .findings
                        .push(CollectionDirectionalSpeedFindingV1::Speed {
                            member_id: member.id().clone(),
                            measured_speed_mps: measured,
                            expected_speed_mps: value,
                            tolerance_mps: tolerance,
                        });
                }
            }
            Expected::Ratio {
                value,
                tolerance,
                reference,
            } => {
                let measured = checked_div(member.speed_mps.expect("complete evidence"), reference)
                    .expect("numeric range checked before findings");
                row.measured_ratio = Some(measured);
                row.expected_ratio = Some(value);
                row.magnitude_tolerance = Some(tolerance);
                row.magnitude_deviation = Some((measured - value).abs());
                row.magnitude_passed = Some(row.magnitude_deviation.expect("set") <= tolerance);
                if (measured - value).abs() > tolerance {
                    result
                        .findings
                        .push(CollectionDirectionalSpeedFindingV1::Ratio {
                            member_id: member.id().clone(),
                            measured_ratio: measured,
                            expected_ratio: value,
                            tolerance,
                        });
                }
            }
        }
    }
    Ok(result)
}

enum Expected {
    Speed {
        value: f64,
        tolerance: f64,
    },
    Ratio {
        value: f64,
        tolerance: f64,
        reference: f64,
    },
}

fn expectations(
    policy: &CollectionDirectionalSpeedPolicyV1,
    evidence: &[CollectionDirectionalSpeedEvidenceMemberV1],
) -> Option<Vec<Expected>> {
    let gains = policy
        .members()
        .iter()
        .map(|m| gain(policy.diagonal_behavior(), m.coordinate()))
        .collect::<Option<Vec<_>>>()?;
    match policy.mode() {
        CollectionDirectionalSpeedModeV1::Uniform {
            speed_mps,
            speed_tolerance_mps,
        } => policy
            .members()
            .iter()
            .zip(gains)
            .map(|(_, gain)| {
                Some(Expected::Speed {
                    value: checked_mul(*speed_mps, gain)?,
                    tolerance: *speed_tolerance_mps,
                })
            })
            .collect(),
        CollectionDirectionalSpeedModeV1::Authored {
            speed_tolerance_mps,
        } => policy
            .members()
            .iter()
            .zip(gains)
            .map(|(m, gain)| {
                Some(Expected::Speed {
                    value: checked_mul(m.speed_mps().expect("policy checked"), gain)?,
                    tolerance: *speed_tolerance_mps,
                })
            })
            .collect(),
        CollectionDirectionalSpeedModeV1::Ratios {
            reference_member,
            ratio_tolerance,
        } => {
            let index = policy
                .members()
                .iter()
                .position(|m| m.id() == reference_member)
                .expect("policy checked");
            let reference_gain = gains[index];
            let reference = evidence[index].speed_mps.expect("complete evidence");
            policy
                .members()
                .iter()
                .zip(gains)
                .zip(evidence)
                .map(|((m, gain), evidence_member)| {
                    let value = if m.id() == reference_member {
                        1.0
                    } else {
                        checked_div(
                            checked_mul(m.expected_ratio().expect("policy checked"), gain)?,
                            reference_gain,
                        )?
                    };
                    // Deliberately make every member's measured ratio representable
                    // before any direction finding can be emitted.
                    checked_div(
                        evidence_member.speed_mps.expect("complete evidence"),
                        reference,
                    )?;
                    Some(Expected::Ratio {
                        value,
                        tolerance: *ratio_tolerance,
                        reference,
                    })
                })
                .collect()
        }
    }
}

fn heading(
    member: &CollectionDirectionalSpeedEvidenceMemberV1,
    policy: &CollectionDirectionalSpeedPolicyV1,
) -> Option<(f64, f64)> {
    let raw = normalize([
        member.horizontal_displacement_x_m?,
        member.horizontal_displacement_z_m?,
    ])?;
    let x = normalize(policy.source_basis().x())?;
    let z = normalize(policy.source_basis().z())?;
    normalize([raw[0] * x[0] + raw[1] * z[0], raw[0] * x[1] + raw[1] * z[1]]).map(|v| (v[0], v[1]))
}
fn gain(mode: CollectionDirectionalSpeedDiagonalBehaviorV1, c: [f64; 2]) -> Option<f64> {
    match mode {
        CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize => Some(1.0),
        CollectionDirectionalSpeedDiagonalBehaviorV1::Preserve => {
            let n = c[0].hypot(c[1]);
            (n.is_finite() && n != 0.0).then_some(n)
        }
    }
}
fn normalize(v: [f64; 2]) -> Option<[f64; 2]> {
    if !v.into_iter().all(f64::is_finite) {
        return None;
    }
    let scale = v[0].abs().max(v[1].abs());
    if scale == 0.0 {
        return None;
    }
    let x = v[0] / scale;
    let z = v[1] / scale;
    let norm = x.hypot(z);
    (norm.is_finite() && norm != 0.0).then_some([x / norm, z / norm])
}
fn checked_mul(a: f64, b: f64) -> Option<f64> {
    let value = a * b;
    (value.is_finite() && !(a != 0.0 && b != 0.0 && value == 0.0)).then_some(value)
}
fn checked_div(a: f64, b: f64) -> Option<f64> {
    if b == 0.0 {
        return None;
    }
    let value = a / b;
    (value.is_finite() && !(a != 0.0 && value == 0.0)).then_some(value)
}
fn valid_nonnegative(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CollectionDirectionalSpeedMemberV1, CollectionDirectionalSpeedSourceBasisV1, CollectionIdV1,
    };

    fn id(value: &str) -> CollectionLogicalIdV1 {
        CollectionLogicalIdV1::new(value).unwrap()
    }

    fn policy() -> CollectionDirectionalSpeedPolicyV1 {
        CollectionDirectionalSpeedPolicyV1::new(
            CollectionDirectionalSpeedManifestIdentityV1::new(
                CollectionIdV1::new("com.example.collection").unwrap(),
                InputIdentity::from_bytes(b"manifest"),
            )
            .unwrap(),
            id("com.example/set"),
            CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
            CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
            1e-9,
            CollectionDirectionalSpeedModeV1::Uniform {
                speed_mps: 1.0,
                speed_tolerance_mps: 0.1,
            },
            vec![
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/x"),
                    [1.0, 0.0],
                    None,
                    None,
                ),
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/z"),
                    [0.0, 1.0],
                    None,
                    None,
                ),
            ],
        )
        .unwrap()
    }

    fn evidence(
        x_speed: f64,
        z_speed: f64,
        x_displacement: f64,
    ) -> CollectionDirectionalSpeedEvidenceV1 {
        let policy = policy();
        CollectionDirectionalSpeedEvidenceV1::new(
            policy.manifest().clone(),
            policy.runtime_set_id().clone(),
            CollectionRuntimeSetKindV1::DirectionalBlend,
            CollectionDirectionalSpeedLifecycleV1::Complete,
            vec![],
            vec![
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/x"),
                    Some(1.0),
                    Some(x_displacement),
                    Some(0.0),
                    Some(1.0),
                    Some(x_speed),
                ),
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/z"),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    Some(1.0),
                    Some(z_speed),
                ),
            ],
        )
        .unwrap()
    }

    #[test]
    fn uniform_heading_speed_and_zero_endpoint_are_typed_and_ordered() {
        let policy = policy();
        let passing = evaluate_collection_directional_speed_v1(
            &policy,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &evidence(1.0, 1.0, 1.0),
        )
        .unwrap();
        assert!(passing.findings().is_empty());
        let failing = evaluate_collection_directional_speed_v1(
            &policy,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &evidence(1.2, 1.0, 1.0),
        )
        .unwrap();
        assert!(
            matches!(failing.findings(), [CollectionDirectionalSpeedFindingV1::Speed { member_id, .. }] if member_id.as_str() == "com.example/x")
        );
        let zero = evaluate_collection_directional_speed_v1(
            &policy,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &evidence(1.0, 1.0, 0.0),
        )
        .unwrap();
        assert_eq!(
            zero.not_evaluated_reason(),
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroNetDisplacement)
        );
    }

    #[test]
    fn heading_uses_forward_basis_image_not_the_transpose() {
        let base = policy();
        let rotated = CollectionDirectionalSpeedPolicyV1::new(
            base.manifest().clone(),
            base.runtime_set_id().clone(),
            CollectionDirectionalSpeedSourceBasisV1::new([0.0, 1.0], [-1.0, 0.0]).unwrap(),
            CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
            0.0,
            CollectionDirectionalSpeedModeV1::Uniform {
                speed_mps: 1.0,
                speed_tolerance_mps: 0.1,
            },
            vec![
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/x"),
                    [0.0, 1.0],
                    None,
                    None,
                ),
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/z"),
                    [-1.0, 0.0],
                    None,
                    None,
                ),
            ],
        )
        .unwrap();
        let result = evaluate_collection_directional_speed_v1(
            &rotated,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &evidence(1.0, 1.0, 1.0),
        )
        .unwrap();
        assert!(result.findings().is_empty());
        assert_eq!(result.members()[0].projected_heading, Some([0.0, 1.0]));
    }

    #[test]
    fn authored_preserve_ratios_and_numeric_outcomes_are_explicit() {
        let base = policy();
        let authored = CollectionDirectionalSpeedPolicyV1::new(
            base.manifest().clone(),
            base.runtime_set_id().clone(),
            CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
            CollectionDirectionalSpeedDiagonalBehaviorV1::Preserve,
            1e-9,
            CollectionDirectionalSpeedModeV1::Authored {
                speed_tolerance_mps: 0.0,
            },
            vec![
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/x"),
                    [1.0, 0.0],
                    Some(1.0),
                    None,
                ),
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/z"),
                    [1.0, 1.0],
                    Some(1.0),
                    None,
                ),
            ],
        )
        .unwrap();
        let authored_evidence = CollectionDirectionalSpeedEvidenceV1::new(
            authored.manifest().clone(),
            authored.runtime_set_id().clone(),
            CollectionRuntimeSetKindV1::DirectionalBlend,
            CollectionDirectionalSpeedLifecycleV1::Complete,
            vec![],
            vec![
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/x"),
                    Some(1.0),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    Some(1.0),
                ),
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/z"),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(2.0_f64.sqrt()),
                ),
            ],
        )
        .unwrap();
        assert!(
            evaluate_collection_directional_speed_v1(
                &authored,
                InputIdentity::from_bytes(b"p"),
                InputIdentity::from_bytes(b"e"),
                &authored_evidence
            )
            .unwrap()
            .findings()
            .is_empty()
        );

        let ratios = CollectionDirectionalSpeedPolicyV1::new(
            base.manifest().clone(),
            base.runtime_set_id().clone(),
            CollectionDirectionalSpeedSourceBasisV1::new([1.0, 0.0], [0.0, 1.0]).unwrap(),
            CollectionDirectionalSpeedDiagonalBehaviorV1::Normalize,
            0.0,
            CollectionDirectionalSpeedModeV1::Ratios {
                reference_member: id("com.example/x"),
                ratio_tolerance: 0.0,
            },
            vec![
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/x"),
                    [1.0, 0.0],
                    None,
                    Some(1.0),
                ),
                CollectionDirectionalSpeedMemberV1::new(
                    id("com.example/z"),
                    [0.0, 1.0],
                    None,
                    Some(2.0),
                ),
            ],
        )
        .unwrap();
        let zero_reference = evidence(0.0, 0.0, 1.0);
        let zero = evaluate_collection_directional_speed_v1(
            &ratios,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &zero_reference,
        )
        .unwrap();
        assert_eq!(
            zero.not_evaluated_reason(),
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::ZeroReferenceSpeed)
        );
        let range_evidence = CollectionDirectionalSpeedEvidenceV1::new(
            ratios.manifest().clone(),
            ratios.runtime_set_id().clone(),
            CollectionRuntimeSetKindV1::DirectionalBlend,
            CollectionDirectionalSpeedLifecycleV1::Complete,
            vec![],
            vec![
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/x"),
                    Some(1.0),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    Some(f64::MIN_POSITIVE),
                ),
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/z"),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    Some(1.0),
                    Some(f64::MAX),
                ),
            ],
        )
        .unwrap();
        let range = evaluate_collection_directional_speed_v1(
            &ratios,
            InputIdentity::from_bytes(b"p"),
            InputIdentity::from_bytes(b"e"),
            &range_evidence,
        )
        .unwrap();
        assert_eq!(
            range.not_evaluated_reason(),
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::NumericRange)
        );
    }

    #[test]
    fn incomplete_and_binding_errors_are_not_silent() {
        let policy = policy();
        let incomplete = CollectionDirectionalSpeedEvidenceV1::new(
            policy.manifest().clone(),
            policy.runtime_set_id().clone(),
            CollectionRuntimeSetKindV1::DirectionalBlend,
            CollectionDirectionalSpeedLifecycleV1::Incomplete,
            vec![],
            vec![
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/x"),
                    Some(1.0),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    None,
                ),
                CollectionDirectionalSpeedEvidenceMemberV1::new(
                    id("com.example/z"),
                    Some(1.0),
                    Some(0.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_collection_directional_speed_v1(
                &policy,
                InputIdentity::from_bytes(b"p"),
                InputIdentity::from_bytes(b"e"),
                &incomplete
            )
            .unwrap()
            .not_evaluated_reason(),
            Some(CollectionDirectionalSpeedNotEvaluatedReasonV1::IncompleteRootTravel)
        );
        assert!(
            CollectionDirectionalSpeedEvidenceV1::new(
                policy.manifest().clone(),
                policy.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                CollectionDirectionalSpeedLifecycleV1::Incomplete,
                vec![],
                vec![
                    CollectionDirectionalSpeedEvidenceMemberV1::new(
                        id("com.example/x"),
                        Some(f64::NAN),
                        None,
                        None,
                        None,
                        None
                    ),
                    CollectionDirectionalSpeedEvidenceMemberV1::new(
                        id("com.example/z"),
                        None,
                        None,
                        None,
                        None,
                        None
                    ),
                ]
            )
            .is_err()
        );
    }

    #[test]
    fn scaled_normalization_keeps_max_and_subnormal_headings_defined() {
        let max = normalize([f64::MAX, f64::MAX]).unwrap();
        let tiny = normalize([f64::from_bits(1), f64::from_bits(1)]).unwrap();
        assert_eq!(max, tiny);
        assert!(normalize([0.0, 0.0]).is_none());
    }

    #[test]
    fn raw_input_caps_and_complete_root_travel_lifecycle_fail_closed() {
        let policy = policy();
        let complete = evidence(1.0, 1.0, 1.0);
        let at_policy = InputIdentity::from_sha256_digest(
            [1; 32],
            crate::COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES,
        );
        let at_evidence = InputIdentity::from_sha256_digest(
            [2; 32],
            crate::COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES,
        );
        assert!(
            evaluate_collection_directional_speed_v1(&policy, at_policy, at_evidence, &complete)
                .is_ok()
        );
        assert_eq!(
            evaluate_collection_directional_speed_v1(
                &policy,
                InputIdentity::from_sha256_digest(
                    [1; 32],
                    crate::COLLECTION_DIRECTIONAL_SPEED_POLICY_V1_MAX_BYTES + 1
                ),
                InputIdentity::from_bytes(b"e"),
                &complete
            ),
            Err(CollectionDirectionalSpeedEvaluationControlError::OverBudgetInput)
        );
        assert_eq!(
            evaluate_collection_directional_speed_v1(
                &policy,
                InputIdentity::from_bytes(b"policy"),
                InputIdentity::from_sha256_digest(
                    [2; 32],
                    crate::COLLECTION_DIRECTIONAL_SPEED_EVIDENCE_V1_MAX_BYTES + 1
                ),
                &complete
            ),
            Err(CollectionDirectionalSpeedEvaluationControlError::OverBudgetInput)
        );
        assert!(
            CollectionDirectionalSpeedEvidenceV1::new(
                policy.manifest().clone(),
                policy.runtime_set_id().clone(),
                CollectionRuntimeSetKindV1::DirectionalBlend,
                CollectionDirectionalSpeedLifecycleV1::Incomplete,
                vec![id("com.example/x")],
                complete.members().to_vec()
            )
            .is_err()
        );
    }
}
