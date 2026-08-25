//! Strict, format-neutral transition-pose evaluation V1.
//!
//! This module consumes the validated transition-family declaration and a
//! mutable loader-facing [`Document`]. It deliberately owns strict endpoint
//! sampling rather than changing the tolerant general-purpose sampler, and it
//! deliberately has no filesystem, config, collection, or command authority.

use serde::Serialize;
use std::io::{self, Write};

use crate::{
    Bone, Clip, Document, DocumentShapeError, InputIdentity, Property, Skeleton, TrackSample,
    TransitionFamilyBoundaryV1, TransitionFamilyDeclarationInputV1, TransitionFamilyTolerancesV1,
    validate_document_shape,
};

/// Schema identity for a transition-pose evaluation result.
pub const TRANSITION_POSE_EVALUATION_V1_ID: &str =
    "urn:animsmith:schema:transition-pose-evaluation:1";
/// Schema version for a transition-pose evaluation result.
pub const TRANSITION_POSE_EVALUATION_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum skeleton bones admitted by one V1 basis.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_BONES: usize = 4_096;
/// Maximum pair/boundary comparisons in one family.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_FAMILY_PAIR_BOUNDARIES: usize = 4_096;
/// Maximum pair/boundary comparisons across one result.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_PAIR_BOUNDARIES: usize = 65_536;
/// Maximum pair/boundary/bone comparisons across one result.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_COMPARISONS: usize = 16_777_216;
/// Maximum retained translation offenders in one pair/boundary row.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS: usize = 16;
/// Maximum retained rotation offenders in one pair/boundary row.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS: usize = 16;
/// Maximum retained offenders across one result.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_OFFENDERS: usize = 65_536;
/// Maximum serialized V1 result bytes.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES: usize = 256 * 1024 * 1024;

/// A normalized local-rest bone record contributing to [`SkeletonBasisV1`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkeletonBasisBoneV1 {
    ordinal: usize,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_ordinal: Option<usize>,
    rest_translation_m: [f64; 3],
    rest_rotation: [f64; 4],
}

impl SkeletonBasisBoneV1 {
    /// Parent-before-child ordinal.
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
    /// Exact normalized bone name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Parent ordinal, when this bone is not a root.
    pub const fn parent_ordinal(&self) -> Option<usize> {
        self.parent_ordinal
    }
    /// Finite local-rest translation in metres.
    pub const fn rest_translation_m(&self) -> [f64; 3] {
        self.rest_translation_m
    }
    /// Unit, hemisphere-canonical local-rest quaternion in `[x, y, z, w]` order.
    pub const fn rest_rotation(&self) -> [f64; 4] {
        self.rest_rotation
    }
}

/// First-class normalized skeleton identity used by V1 comparisons.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SkeletonBasisV1 {
    schema: &'static str,
    schema_version: u32,
    bones: Vec<SkeletonBasisBoneV1>,
    #[serde(skip)]
    identity: InputIdentity,
}

impl SkeletonBasisV1 {
    /// Build a strict scale/bind/mesh-independent skeleton basis.
    pub fn from_skeleton(skeleton: &Skeleton) -> Result<Self, SkeletonBasisError> {
        if skeleton.bones.len() > TRANSITION_POSE_EVALUATION_V1_MAX_BONES {
            return Err(SkeletonBasisError::TooManyBones);
        }
        let mut bones = Vec::with_capacity(skeleton.bones.len());
        for (ordinal, bone) in skeleton.bones.iter().enumerate() {
            let parent_ordinal = match bone.parent {
                Some(parent) if parent < ordinal => Some(parent),
                Some(_) => return Err(SkeletonBasisError::InvalidParent { ordinal }),
                None => None,
            };
            bones.push(SkeletonBasisBoneV1 {
                ordinal,
                name: bone.name.clone(),
                parent_ordinal,
                rest_translation_m: finite_translation(bone, ordinal)?,
                rest_rotation: canonical_quaternion(bone.rest.rotation, ordinal)?,
            });
        }
        let wire = SkeletonBasisWire {
            schema: "urn:animsmith:schema:skeleton-basis:1",
            schema_version: 1,
            bones: &bones,
        };
        let bytes = canonical_bytes(&wire, TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES)
            .map_err(|_| SkeletonBasisError::IdentityTooLarge)?;
        Ok(Self {
            schema: "urn:animsmith:schema:skeleton-basis:1",
            schema_version: 1,
            bones,
            identity: InputIdentity::from_bytes(&bytes),
        })
    }
    /// Skeleton-basis schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    /// Skeleton-basis schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Parent-before-child normalized bones.
    pub fn bones(&self) -> &[SkeletonBasisBoneV1] {
        &self.bones
    }
    /// Exact JCS identity of this basis.
    pub const fn identity(&self) -> &InputIdentity {
        &self.identity
    }
}

#[derive(Serialize)]
struct SkeletonBasisWire<'a> {
    schema: &'static str,
    schema_version: u32,
    bones: &'a [SkeletonBasisBoneV1],
}

/// Strict skeleton-basis construction failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SkeletonBasisError {
    /// A basis exceeded its fixed bone limit.
    #[error("transition-pose skeleton basis exceeds the bone cap")]
    TooManyBones,
    /// A parent was absent from the required parent-before-child prefix.
    #[error("transition-pose skeleton basis has an invalid parent at bone {ordinal}")]
    InvalidParent {
        /// Child ordinal whose parent was invalid.
        ordinal: usize,
    },
    /// One local-rest translation or quaternion was non-finite or degenerate.
    #[error("transition-pose skeleton basis has an invalid rest transform at bone {ordinal}")]
    InvalidRest {
        /// Bone ordinal whose rest transform was invalid.
        ordinal: usize,
    },
    /// The normalized identity could not fit the bounded canonical writer.
    #[error("transition-pose skeleton basis identity exceeds the result cap")]
    IdentityTooLarge,
}

/// Closed result lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum TransitionPoseStatusV1 {
    Complete,
    Incomplete,
}
/// Closed V1 decision vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum TransitionPoseDecisionV1 {
    Pass,
    Finding,
    NotEvaluated,
}
/// Closed typed reason vocabulary for no-family and incomplete outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(missing_docs)]
pub enum TransitionPoseReasonV1 {
    NoConfiguredFamilies,
    MemberUnavailable,
    ZeroDuration,
    SkeletonBasisMismatch,
    TimeToleranceUnsupported,
    UnsupportedSampling,
    InputLimit,
    FamilyWorkLimit,
    AggregateWorkLimit,
    RetentionLimit,
    ResultLimit,
}

/// Exact resolved member authority retained in a family result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransitionPoseMemberV1 {
    take_index: u64,
    take_name: String,
    source_input: InputIdentity,
}
impl TransitionPoseMemberV1 {
    /// Witnessed embedded take index.
    pub const fn take_index(&self) -> u64 {
        self.take_index
    }
    /// Witnessed embedded take name.
    pub fn take_name(&self) -> &str {
        &self.take_name
    }
    /// Exact raw source identity for this selected member.
    pub const fn source_input(&self) -> &InputIdentity {
        &self.source_input
    }
}

/// One translation offender, sorted independently from rotation offenders.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransitionPoseTranslationOffenderV1 {
    bone_ordinal: usize,
    bone_name: String,
    delta_m: f64,
}
impl TransitionPoseTranslationOffenderV1 {
    /// Stable skeleton-basis bone ordinal.
    pub const fn bone_ordinal(&self) -> usize {
        self.bone_ordinal
    }
    /// Exact skeleton-basis bone name.
    pub fn bone_name(&self) -> &str {
        &self.bone_name
    }
    /// Measured Euclidean translation delta in metres.
    pub const fn delta_m(&self) -> f64 {
        self.delta_m
    }
}
/// One rotation offender, sorted independently from translation offenders.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransitionPoseRotationOffenderV1 {
    bone_ordinal: usize,
    bone_name: String,
    delta_deg: f64,
}
impl TransitionPoseRotationOffenderV1 {
    /// Stable skeleton-basis bone ordinal.
    pub const fn bone_ordinal(&self) -> usize {
        self.bone_ordinal
    }
    /// Exact skeleton-basis bone name.
    pub fn bone_name(&self) -> &str {
        &self.bone_name
    }
    /// Measured shortest-path rotation delta in degrees.
    pub const fn delta_deg(&self) -> f64 {
        self.delta_deg
    }
}

/// Canonically ordered member-pair endpoint comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransitionPosePairEvaluationV1 {
    member_indices: [usize; 2],
    boundary: TransitionFamilyBoundaryV1,
    max_translation_delta_m: f64,
    max_rotation_delta_deg: f64,
    translation_tolerance_m: f64,
    rotation_tolerance_deg: f64,
    translation_offenders: Vec<TransitionPoseTranslationOffenderV1>,
    rotation_offenders: Vec<TransitionPoseRotationOffenderV1>,
}
impl TransitionPosePairEvaluationV1 {
    /// Canonically ordered declaration-member indices.
    pub const fn member_indices(&self) -> [usize; 2] {
        self.member_indices
    }
    /// Compared endpoint boundary.
    pub const fn boundary(&self) -> TransitionFamilyBoundaryV1 {
        self.boundary
    }
    /// Maximum measured translation delta in metres.
    pub const fn max_translation_delta_m(&self) -> f64 {
        self.max_translation_delta_m
    }
    /// Maximum measured rotation delta in degrees.
    pub const fn max_rotation_delta_deg(&self) -> f64 {
        self.max_rotation_delta_deg
    }
    /// Applied inclusive translation tolerance in metres.
    pub const fn translation_tolerance_m(&self) -> f64 {
        self.translation_tolerance_m
    }
    /// Applied inclusive rotation tolerance in degrees.
    pub const fn rotation_tolerance_deg(&self) -> f64 {
        self.rotation_tolerance_deg
    }
    /// Translation offenders in V1 order.
    pub fn translation_offenders(&self) -> &[TransitionPoseTranslationOffenderV1] {
        &self.translation_offenders
    }
    /// Rotation offenders in V1 order.
    pub fn rotation_offenders(&self) -> &[TransitionPoseRotationOffenderV1] {
        &self.rotation_offenders
    }
}

/// Immutable per-family result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransitionPoseFamilyEvaluationV1 {
    family_id: String,
    status: TransitionPoseStatusV1,
    decision: TransitionPoseDecisionV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<TransitionPoseReasonV1>,
    members: Vec<TransitionPoseMemberV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skeleton_basis_input: Option<InputIdentity>,
    pairs: Vec<TransitionPosePairEvaluationV1>,
}
impl TransitionPoseFamilyEvaluationV1 {
    /// Stable transition-family identifier.
    pub fn family_id(&self) -> &str {
        &self.family_id
    }
    /// Family lifecycle.
    pub const fn status(&self) -> TransitionPoseStatusV1 {
        self.status
    }
    /// Family decision.
    pub const fn decision(&self) -> TransitionPoseDecisionV1 {
        self.decision
    }
    /// Typed non-evaluation reason, when present.
    pub const fn reason(&self) -> Option<TransitionPoseReasonV1> {
        self.reason
    }
    /// Exact selected member/source authorities in declared member order.
    pub fn members(&self) -> &[TransitionPoseMemberV1] {
        &self.members
    }
    /// Exact matching skeleton-basis identity, when one was established.
    pub const fn skeleton_basis_input(&self) -> Option<&InputIdentity> {
        self.skeleton_basis_input.as_ref()
    }
    /// Canonical pair/boundary comparison rows.
    pub fn pairs(&self) -> &[TransitionPosePairEvaluationV1] {
        &self.pairs
    }
}

/// Immutable document-bound V1 result contract.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransitionPoseEvaluationV1 {
    schema: &'static str,
    schema_version: u32,
    status: TransitionPoseStatusV1,
    decision: TransitionPoseDecisionV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<TransitionPoseReasonV1>,
    declaration_input: InputIdentity,
    declaration_normalized: InputIdentity,
    document_input: InputIdentity,
    families: Vec<TransitionPoseFamilyEvaluationV1>,
}

impl TransitionPoseEvaluationV1 {
    /// Result schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }
    /// Result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Aggregate lifecycle.
    pub const fn status(&self) -> TransitionPoseStatusV1 {
        self.status
    }
    /// Aggregate decision.
    pub const fn decision(&self) -> TransitionPoseDecisionV1 {
        self.decision
    }
    /// Aggregate typed reason, when there are no configured families.
    pub const fn reason(&self) -> Option<TransitionPoseReasonV1> {
        self.reason
    }
    /// Exact declaration source identity.
    pub const fn declaration_input(&self) -> &InputIdentity {
        &self.declaration_input
    }
    /// Independently normalized declaration identity.
    pub const fn declaration_normalized(&self) -> &InputIdentity {
        &self.declaration_normalized
    }
    /// Exact raw document input identity.
    pub const fn document_input(&self) -> &InputIdentity {
        &self.document_input
    }
    /// Family results in canonical declaration-family order.
    pub fn families(&self) -> &[TransitionPoseFamilyEvaluationV1] {
        &self.families
    }
    /// Serialize this result through the V1 bounded canonical writer.
    pub fn normalized_jcs(&self) -> Result<Vec<u8>, TransitionPoseEvaluationControlError> {
        canonical_bytes(self, TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES)
            .map_err(|_| TransitionPoseEvaluationControlError::ResultTooLarge)
    }
}

/// Invalid evaluator input is a control error. Incomplete families are normal
/// successful results so callers can emit their immutable contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TransitionPoseEvaluationControlError {
    /// The declaration is collection-owned and needs the deferred collection adapter.
    #[error("document transition-pose evaluation requires a document declaration")]
    WrongDeclarationScope,
    /// The mutable loader-facing document failed strict revalidation.
    #[error("transition-pose document shape is invalid: {0}")]
    InvalidDocumentShape(DocumentShapeError),
    /// The strict normalized skeleton identity could not be constructed.
    #[error("transition-pose skeleton basis is invalid: {0}")]
    InvalidSkeletonBasis(SkeletonBasisError),
    /// A declaration member witness did not resolve exactly once in the document.
    #[error("transition-pose declaration member witness is structurally contradictory")]
    InvalidMemberWitness,
    /// Even the deterministic bounded result summary could not be serialized.
    #[error("transition-pose result exceeds the V1 result cap")]
    ResultTooLarge,
}

/// Evaluate document-local transition families without I/O.
///
/// The document is revalidated at this public boundary because its public
/// fields are mutable. All work planning happens before endpoint sampling; an
/// unavailable family is retained as `incomplete/not_evaluated`, never
/// evaluated as a survivor subset.
pub fn evaluate_document_transition_poses_v1(
    declaration: &TransitionFamilyDeclarationInputV1,
    document_input: InputIdentity,
    document: &Document,
) -> Result<TransitionPoseEvaluationV1, TransitionPoseEvaluationControlError> {
    validate_document_shape(document)
        .map_err(TransitionPoseEvaluationControlError::InvalidDocumentShape)?;
    let families = declaration
        .declaration()
        .document_families()
        .ok_or(TransitionPoseEvaluationControlError::WrongDeclarationScope)?;
    let mut result = TransitionPoseEvaluationV1 {
        schema: TRANSITION_POSE_EVALUATION_V1_ID,
        schema_version: TRANSITION_POSE_EVALUATION_V1_SCHEMA_VERSION,
        status: TransitionPoseStatusV1::Complete,
        decision: TransitionPoseDecisionV1::Pass,
        reason: None,
        declaration_input: declaration.source_identity().clone(),
        declaration_normalized: declaration.normalized_identity().clone(),
        document_input: document_input.clone(),
        families: Vec::with_capacity(families.len()),
    };
    if families.is_empty() {
        result.reason = Some(TransitionPoseReasonV1::NoConfiguredFamilies);
        return Ok(result);
    }
    let basis = SkeletonBasisV1::from_skeleton(&document.skeleton)
        .map_err(TransitionPoseEvaluationControlError::InvalidSkeletonBasis)?;
    let plans = plan_families(families, basis.bones().len());
    for (family, plan) in families.iter().zip(plans) {
        let members = family
            .members()
            .iter()
            .map(|member| TransitionPoseMemberV1 {
                take_index: member.take_index(),
                take_name: member.take_name().to_owned(),
                source_input: document_input.clone(),
            })
            .collect::<Vec<_>>();
        let mut row = TransitionPoseFamilyEvaluationV1 {
            family_id: family.family_id().to_owned(),
            status: TransitionPoseStatusV1::Incomplete,
            decision: TransitionPoseDecisionV1::NotEvaluated,
            reason: None,
            members,
            skeleton_basis_input: Some(basis.identity().clone()),
            pairs: Vec::new(),
        };
        if family.tolerances().time_normalized() != 0.0 {
            row.reason = Some(TransitionPoseReasonV1::TimeToleranceUnsupported);
        } else if let Some(reason) = plan.reason {
            row.reason = Some(reason);
        } else {
            let clips = resolve_family_clips(document, family.members())?;
            let endpoints = match strict_endpoints(document, &clips) {
                Ok(value) => value,
                Err(reason) => {
                    row.reason = Some(reason);
                    result.families.push(row);
                    continue;
                }
            };
            row.pairs = compare_pairs(
                &endpoints,
                family.boundary(),
                family.tolerances(),
                &document.skeleton,
            );
            row.status = TransitionPoseStatusV1::Complete;
            row.decision = if row.pairs.iter().any(pair_has_finding) {
                TransitionPoseDecisionV1::Finding
            } else {
                TransitionPoseDecisionV1::Pass
            };
        }
        result.families.push(row);
    }
    derive_result_state(&mut result);
    if result.normalized_jcs().is_err() {
        for family in &mut result.families {
            family.status = TransitionPoseStatusV1::Incomplete;
            family.decision = TransitionPoseDecisionV1::NotEvaluated;
            family.reason = Some(TransitionPoseReasonV1::ResultLimit);
            family.pairs.clear();
        }
        derive_result_state(&mut result);
        result.normalized_jcs()?;
    }
    Ok(result)
}

#[derive(Clone, Copy)]
struct FamilyPlan {
    reason: Option<TransitionPoseReasonV1>,
}

fn plan_families(
    families: &[crate::DocumentTransitionFamilyV1],
    bone_count: usize,
) -> Vec<FamilyPlan> {
    let mut aggregate_pairs = 0usize;
    let mut aggregate_comparisons = 0usize;
    let mut aggregate_retention = 0usize;
    families
        .iter()
        .map(|family| {
            let boundaries = match family.boundary() {
                TransitionFamilyBoundaryV1::Entry | TransitionFamilyBoundaryV1::Exit => 1usize,
                TransitionFamilyBoundaryV1::Both => 2usize,
            };
            let pairs = checked_pair_count(family.members().len());
            let pair_boundaries = pairs.and_then(|value| value.checked_mul(boundaries));
            let comparisons = pair_boundaries.and_then(|value| value.checked_mul(bone_count));
            let retention = pair_boundaries.and_then(|value| {
                value.checked_mul(
                    TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS
                        + TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS,
                )
            });
            let reason = match (pair_boundaries, comparisons, retention) {
                (Some(pair_boundaries), _, _)
                    if pair_boundaries
                        > TRANSITION_POSE_EVALUATION_V1_MAX_FAMILY_PAIR_BOUNDARIES =>
                {
                    Some(TransitionPoseReasonV1::FamilyWorkLimit)
                }
                (Some(pair_boundaries), Some(comparisons), Some(_retention))
                    if aggregate_pairs
                        .checked_add(pair_boundaries)
                        .is_none_or(|value| {
                            value > TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_PAIR_BOUNDARIES
                        })
                        || aggregate_comparisons
                            .checked_add(comparisons)
                            .is_none_or(|value| {
                                value > TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_COMPARISONS
                            }) =>
                {
                    Some(TransitionPoseReasonV1::AggregateWorkLimit)
                }
                (Some(_pair_boundaries), Some(_comparisons), Some(retention))
                    if aggregate_retention
                        .checked_add(retention)
                        .is_none_or(|value| {
                            value > TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_OFFENDERS
                        }) =>
                {
                    Some(TransitionPoseReasonV1::RetentionLimit)
                }
                (Some(pair_boundaries), Some(comparisons), Some(retention)) => {
                    aggregate_pairs += pair_boundaries;
                    aggregate_comparisons += comparisons;
                    aggregate_retention += retention;
                    None
                }
                _ => Some(TransitionPoseReasonV1::AggregateWorkLimit),
            };
            FamilyPlan { reason }
        })
        .collect()
}

fn checked_pair_count(member_count: usize) -> Option<usize> {
    member_count
        .checked_mul(member_count.checked_sub(1)?)
        .and_then(|value| value.checked_div(2))
}

fn resolve_family_clips<'a>(
    document: &'a Document,
    members: &[crate::DocumentTransitionFamilyMemberV1],
) -> Result<Vec<&'a Clip>, TransitionPoseEvaluationControlError> {
    members
        .iter()
        .map(|member| {
            let index = usize::try_from(member.take_index())
                .map_err(|_| TransitionPoseEvaluationControlError::InvalidMemberWitness)?;
            let clip = document
                .clips
                .get(index)
                .filter(|clip| clip.name == member.take_name())
                .ok_or(TransitionPoseEvaluationControlError::InvalidMemberWitness)?;
            if document
                .clips
                .iter()
                .filter(|candidate| candidate.name == member.take_name())
                .count()
                != 1
            {
                return Err(TransitionPoseEvaluationControlError::InvalidMemberWitness);
            }
            Ok(clip)
        })
        .collect()
}

#[derive(Clone)]
struct Endpoints {
    entry: Vec<EndpointPose>,
    exit: Vec<EndpointPose>,
}

#[derive(Clone, Copy)]
struct EndpointPose {
    translation: [f64; 3],
    rotation: [f64; 4],
}

fn strict_endpoints(
    document: &Document,
    clips: &[&Clip],
) -> Result<Vec<Endpoints>, TransitionPoseReasonV1> {
    clips
        .iter()
        .map(|clip| {
            if !clip.duration_s.is_finite() || clip.duration_s <= 0.0 {
                return Err(TransitionPoseReasonV1::ZeroDuration);
            }
            let exit_time = clip.duration_s as f32;
            if !exit_time.is_finite() || f64::from(exit_time) != clip.duration_s {
                return Err(TransitionPoseReasonV1::UnsupportedSampling);
            }
            Ok(Endpoints {
                entry: strict_endpoint(&document.skeleton, clip, 0.0)?,
                exit: strict_endpoint(&document.skeleton, clip, exit_time)?,
            })
        })
        .collect()
}

fn strict_endpoint(
    skeleton: &Skeleton,
    clip: &Clip,
    time: f32,
) -> Result<Vec<EndpointPose>, TransitionPoseReasonV1> {
    if !time.is_finite() {
        return Err(TransitionPoseReasonV1::UnsupportedSampling);
    }
    let mut poses = skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(ordinal, bone)| {
            Ok(EndpointPose {
                translation: finite_translation(bone, ordinal)
                    .map_err(|_| TransitionPoseReasonV1::UnsupportedSampling)?,
                rotation: canonical_quaternion(bone.rest.rotation, ordinal)
                    .map_err(|_| TransitionPoseReasonV1::UnsupportedSampling)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for track in &clip.tracks {
        let target = poses
            .get_mut(track.bone)
            .ok_or(TransitionPoseReasonV1::UnsupportedSampling)?;
        match (track.property, crate::sample_track(track, time)) {
            (Property::Translation, TrackSample::Vec3(value)) => {
                target.translation =
                    finite_vec3(value).ok_or(TransitionPoseReasonV1::UnsupportedSampling)?;
            }
            (Property::Rotation, TrackSample::Quat(value)) => {
                target.rotation = canonical_quaternion(value, track.bone)
                    .map_err(|_| TransitionPoseReasonV1::UnsupportedSampling)?;
            }
            (Property::Scale, TrackSample::Vec3(_)) => {}
            _ => return Err(TransitionPoseReasonV1::UnsupportedSampling),
        }
    }
    Ok(poses)
}

fn compare_pairs(
    endpoints: &[Endpoints],
    boundary: TransitionFamilyBoundaryV1,
    tolerances: TransitionFamilyTolerancesV1,
    skeleton: &Skeleton,
) -> Vec<TransitionPosePairEvaluationV1> {
    let boundaries: &[TransitionFamilyBoundaryV1] = match boundary {
        TransitionFamilyBoundaryV1::Entry => &[TransitionFamilyBoundaryV1::Entry],
        TransitionFamilyBoundaryV1::Exit => &[TransitionFamilyBoundaryV1::Exit],
        TransitionFamilyBoundaryV1::Both => &[
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyBoundaryV1::Exit,
        ],
    };
    let mut output = Vec::new();
    for left in 0..endpoints.len() {
        for right in left + 1..endpoints.len() {
            for &boundary in boundaries {
                let (left_pose, right_pose) = match boundary {
                    TransitionFamilyBoundaryV1::Entry => {
                        (&endpoints[left].entry, &endpoints[right].entry)
                    }
                    TransitionFamilyBoundaryV1::Exit => {
                        (&endpoints[left].exit, &endpoints[right].exit)
                    }
                    TransitionFamilyBoundaryV1::Both => unreachable!("expanded above"),
                };
                output.push(compare_one_pair(
                    [left, right],
                    boundary,
                    left_pose,
                    right_pose,
                    tolerances,
                    skeleton,
                ));
            }
        }
    }
    output
}

fn compare_one_pair(
    member_indices: [usize; 2],
    boundary: TransitionFamilyBoundaryV1,
    left: &[EndpointPose],
    right: &[EndpointPose],
    tolerances: TransitionFamilyTolerancesV1,
    skeleton: &Skeleton,
) -> TransitionPosePairEvaluationV1 {
    let mut max_translation_delta_m = 0.0f64;
    let mut max_rotation_delta_deg = 0.0f64;
    let mut translation_offenders = Vec::new();
    let mut rotation_offenders = Vec::new();
    for (ordinal, ((left, right), bone)) in left.iter().zip(right).zip(&skeleton.bones).enumerate()
    {
        let translation = translation_delta(left.translation, right.translation);
        let rotation = rotation_delta_deg(left.rotation, right.rotation);
        max_translation_delta_m = max_translation_delta_m.max(translation);
        max_rotation_delta_deg = max_rotation_delta_deg.max(rotation);
        if translation > tolerances.translation_m() {
            translation_offenders.push(TransitionPoseTranslationOffenderV1 {
                bone_ordinal: ordinal,
                bone_name: bone.name.clone(),
                delta_m: translation,
            });
        }
        if rotation > tolerances.rotation_deg() {
            rotation_offenders.push(TransitionPoseRotationOffenderV1 {
                bone_ordinal: ordinal,
                bone_name: bone.name.clone(),
                delta_deg: rotation,
            });
        }
    }
    translation_offenders.sort_by(|left, right| {
        right
            .delta_m
            .total_cmp(&left.delta_m)
            .then_with(|| left.bone_ordinal.cmp(&right.bone_ordinal))
            .then_with(|| left.bone_name.cmp(&right.bone_name))
    });
    rotation_offenders.sort_by(|left, right| {
        right
            .delta_deg
            .total_cmp(&left.delta_deg)
            .then_with(|| left.bone_ordinal.cmp(&right.bone_ordinal))
            .then_with(|| left.bone_name.cmp(&right.bone_name))
    });
    translation_offenders.truncate(TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS);
    rotation_offenders.truncate(TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS);
    TransitionPosePairEvaluationV1 {
        member_indices,
        boundary,
        max_translation_delta_m,
        max_rotation_delta_deg,
        translation_tolerance_m: tolerances.translation_m(),
        rotation_tolerance_deg: tolerances.rotation_deg(),
        translation_offenders,
        rotation_offenders,
    }
}

fn pair_has_finding(pair: &TransitionPosePairEvaluationV1) -> bool {
    !pair.translation_offenders.is_empty() || !pair.rotation_offenders.is_empty()
}

fn derive_result_state(result: &mut TransitionPoseEvaluationV1) {
    if result
        .families
        .iter()
        .any(|family| family.status == TransitionPoseStatusV1::Incomplete)
    {
        result.status = TransitionPoseStatusV1::Incomplete;
        result.decision = TransitionPoseDecisionV1::NotEvaluated;
        result.reason = None;
    } else if result
        .families
        .iter()
        .any(|family| family.decision == TransitionPoseDecisionV1::Finding)
    {
        result.status = TransitionPoseStatusV1::Complete;
        result.decision = TransitionPoseDecisionV1::Finding;
        result.reason = None;
    } else {
        result.status = TransitionPoseStatusV1::Complete;
        result.decision = TransitionPoseDecisionV1::Pass;
        result.reason = None;
    }
}

fn finite_translation(bone: &Bone, ordinal: usize) -> Result<[f64; 3], SkeletonBasisError> {
    finite_vec3(bone.rest.translation).ok_or(SkeletonBasisError::InvalidRest { ordinal })
}

fn finite_vec3(value: crate::glam::Vec3) -> Option<[f64; 3]> {
    if !value.is_finite() {
        return None;
    }
    Some([
        canonical_zero(f64::from(value.x)),
        canonical_zero(f64::from(value.y)),
        canonical_zero(f64::from(value.z)),
    ])
}

fn canonical_quaternion(
    value: crate::glam::Quat,
    ordinal: usize,
) -> Result<[f64; 4], SkeletonBasisError> {
    let mut q = [
        f64::from(value.x),
        f64::from(value.y),
        f64::from(value.z),
        f64::from(value.w),
    ];
    if q.iter().any(|value| !value.is_finite()) {
        return Err(SkeletonBasisError::InvalidRest { ordinal });
    }
    let norm_squared = q.iter().map(|value| value * value).sum::<f64>();
    if !norm_squared.is_finite() || norm_squared == 0.0 {
        return Err(SkeletonBasisError::InvalidRest { ordinal });
    }
    let norm = norm_squared.sqrt();
    if !norm.is_finite() || norm == 0.0 {
        return Err(SkeletonBasisError::InvalidRest { ordinal });
    }
    for component in &mut q {
        *component /= norm;
    }
    if q.iter().any(|value| !value.is_finite()) {
        return Err(SkeletonBasisError::InvalidRest { ordinal });
    }
    if hemisphere_negative(q) {
        for component in &mut q {
            *component = -*component;
        }
    }
    for component in &mut q {
        *component = canonical_zero(*component);
    }
    Ok(q)
}

fn hemisphere_negative(q: [f64; 4]) -> bool {
    for index in [3usize, 0, 1, 2] {
        if q[index] < 0.0 {
            return true;
        }
        if q[index] > 0.0 {
            return false;
        }
    }
    false
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn translation_delta(left: [f64; 3], right: [f64; 3]) -> f64 {
    let dx = left[0] - right[0];
    let dy = left[1] - right[1];
    let dz = left[2] - right[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn rotation_delta_deg(left: [f64; 4], right: [f64; 4]) -> f64 {
    let dot = left
        .into_iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    (2.0 * dot.abs().clamp(0.0, 1.0).acos()).to_degrees()
}

fn canonical_bytes(value: &impl Serialize, limit: usize) -> io::Result<Vec<u8>> {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit,
        overflow: false,
    };
    serde_jcs::to_writer(&mut writer, value).map_err(io::Error::other)?;
    if writer.overflow {
        return Err(io::Error::other("bounded JCS result exceeded cap"));
    }
    Ok(writer.bytes)
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
    overflow: bool,
}
impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|length| length > self.limit)
        {
            self.overflow = true;
            return Err(io::Error::other("bounded JCS result exceeded cap"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Bone, Clip, Document, DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1,
        Interpolation, Skeleton, Track, TrackValues, Transform, TransitionFamilyDeclarationV1,
    };

    fn document(second_translation: Option<f32>) -> Document {
        let mut second = Clip {
            name: "Run".into(),
            duration_s: 1.0,
            tracks: Vec::new(),
        };
        if let Some(value) = second_translation {
            second.tracks.push(Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![
                    crate::glam::Vec3::splat(value),
                    crate::glam::Vec3::splat(value),
                ]),
            });
        }
        Document {
            skeleton: Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![
                Clip {
                    name: "Walk".into(),
                    duration_s: 1.0,
                    tracks: Vec::new(),
                },
                second,
            ],
            ..Document::default()
        }
    }

    fn declaration(
        boundary: TransitionFamilyBoundaryV1,
        translation_m: f64,
        time_normalized: f64,
    ) -> TransitionFamilyDeclarationInputV1 {
        let family = DocumentTransitionFamilyV1::new(
            "walk_to_run".into(),
            boundary,
            TransitionFamilyTolerancesV1::new(translation_m, 180.0, time_normalized).unwrap(),
            vec![
                DocumentTransitionFamilyMemberV1::new(0, "Walk".into()).unwrap(),
                DocumentTransitionFamilyMemberV1::new(1, "Run".into()).unwrap(),
            ],
        )
        .unwrap();
        TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
            b"declaration",
        )
        .unwrap()
    }

    #[test]
    fn basis_normalizes_quaternions_and_excludes_scale_bind_and_assets() {
        let first = document(None);
        let mut second = first.clone();
        second.skeleton.bones[0].rest.rotation = crate::glam::Quat::from_xyzw(0.0, -0.0, 0.0, -1.0);
        second.skeleton.bones[0].rest.scale = crate::glam::Vec3::splat(7.0);
        second.skeleton.bones[0].inverse_bind =
            Some(crate::glam::Mat4::from_scale(crate::glam::Vec3::splat(3.0)));
        let left = SkeletonBasisV1::from_skeleton(&first.skeleton).unwrap();
        let right = SkeletonBasisV1::from_skeleton(&second.skeleton).unwrap();
        assert_eq!(left.identity(), right.identity());
        second.skeleton.bones[0].name = "other".into();
        assert_ne!(
            left.identity(),
            SkeletonBasisV1::from_skeleton(&second.skeleton)
                .unwrap()
                .identity()
        );
    }

    #[test]
    fn endpoint_comparison_is_inclusive_and_binds_raw_and_normalized_identities() {
        let declared = declaration(TransitionFamilyBoundaryV1::Both, 3.0_f64.sqrt(), 0.0);
        let document = document(Some(1.0));
        let raw = InputIdentity::from_bytes(b"document\r\n");
        let result =
            evaluate_document_transition_poses_v1(&declared, raw.clone(), &document).unwrap();
        assert_eq!(result.status(), TransitionPoseStatusV1::Complete);
        assert_eq!(result.decision(), TransitionPoseDecisionV1::Pass);
        assert_eq!(result.document_input(), &raw);
        assert_eq!(result.declaration_input(), declared.source_identity());
        assert_eq!(
            result.declaration_normalized(),
            declared.normalized_identity()
        );
        assert_eq!(result.families()[0].pairs.len(), 2);
        assert!(
            result.families()[0]
                .pairs
                .iter()
                .all(|pair| pair.translation_offenders.is_empty())
        );

        let finding = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 1.0, 0.0),
            raw,
            &document,
        )
        .unwrap();
        assert_eq!(finding.decision(), TransitionPoseDecisionV1::Finding);
        assert_eq!(
            finding.families()[0].pairs[0].translation_offenders.len(),
            1
        );
    }

    #[test]
    fn time_and_duration_refusals_are_complete_family_unavailability() {
        let document = document(None);
        let time = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.01),
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        assert_eq!(time.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(
            time.families()[0].reason,
            Some(TransitionPoseReasonV1::TimeToleranceUnsupported)
        );

        let mut zero = document;
        zero.clips[1].duration_s = 0.0;
        let duration = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &zero,
        )
        .unwrap();
        assert_eq!(
            duration.families()[0].reason,
            Some(TransitionPoseReasonV1::ZeroDuration)
        );
    }

    #[test]
    fn mutable_document_shape_is_refused_before_no_family_shortcut() {
        let mut document = document(None);
        document.skeleton.bones[0].parent = Some(0);
        let empty = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
            b"empty",
        )
        .unwrap();
        assert!(matches!(
            evaluate_document_transition_poses_v1(
                &empty,
                InputIdentity::from_bytes(b"document"),
                &document
            ),
            Err(TransitionPoseEvaluationControlError::InvalidDocumentShape(
                _
            ))
        ));
    }

    #[test]
    fn pair_work_math_is_checked_before_sampling() {
        let members = (0..65)
            .map(|index| DocumentTransitionFamilyMemberV1::new(index, format!("clip-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let family = DocumentTransitionFamilyV1::new(
            "too_many_pairs".into(),
            TransitionFamilyBoundaryV1::Both,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            members,
        )
        .unwrap();
        let input = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
            b"declaration",
        )
        .unwrap();
        let result = evaluate_document_transition_poses_v1(
            &input,
            InputIdentity::from_bytes(b"document"),
            &document(None),
        )
        .unwrap();
        assert_eq!(
            result.families()[0].reason,
            Some(TransitionPoseReasonV1::FamilyWorkLimit)
        );
        assert_eq!(checked_pair_count(64), Some(2_016));
        assert_eq!(checked_pair_count(usize::MAX), None);
    }

    #[test]
    fn bounded_writer_refuses_its_first_excess_byte() {
        assert!(canonical_bytes(&"1234", 6).is_ok());
        assert!(canonical_bytes(&"1234", 5).is_err());
    }

    #[test]
    fn result_wire_covers_the_closed_no_config_pass_finding_and_incomplete_matrix() {
        let empty = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
            b"empty",
        )
        .unwrap();
        let pass = evaluate_document_transition_poses_v1(
            &empty,
            InputIdentity::from_bytes(b"document"),
            &document(None),
        )
        .unwrap();
        let pass_wire = serde_json::to_value(&pass).unwrap();
        assert_eq!(pass_wire["schema"], TRANSITION_POSE_EVALUATION_V1_ID);
        assert_eq!(pass_wire["status"], "complete");
        assert_eq!(pass_wire["decision"], "pass");
        assert_eq!(pass_wire["reason"], "no_configured_families");

        let finding = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document(Some(1.0)),
        )
        .unwrap();
        let finding_wire = serde_json::to_value(&finding).unwrap();
        assert_eq!(finding_wire["status"], "complete");
        assert_eq!(finding_wire["decision"], "finding");
        assert!(finding_wire.get("reason").is_none());

        let incomplete = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.1),
            InputIdentity::from_bytes(b"document"),
            &document(None),
        )
        .unwrap();
        let incomplete_wire = serde_json::to_value(&incomplete).unwrap();
        assert_eq!(incomplete_wire["status"], "incomplete");
        assert_eq!(incomplete_wire["decision"], "not_evaluated");
        assert_eq!(
            incomplete_wire["families"][0]["reason"],
            "time_tolerance_unsupported"
        );
    }
}
