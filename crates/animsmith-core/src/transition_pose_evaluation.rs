//! Strict, format-neutral transition-pose evaluation V1.
//!
//! This module consumes the validated transition-family declaration and a
//! mutable loader-facing [`Document`] plus the same-load [`DependencyClosureV1`]
//! that binds every byte on which that document depends. It deliberately owns
//! strict endpoint sampling rather than changing the tolerant general-purpose
//! sampler, and it deliberately has no filesystem, config, collection, or
//! command authority.

use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::model::validate_track_shape;
use crate::{
    Bone, Clip, DependencyClosureIdentityV1, DependencyClosureV1, Document, InputIdentity,
    Property, Skeleton, Track, TrackSample, TransitionFamilyBoundaryV1,
    TransitionFamilyDeclarationInputV1, TransitionFamilyTolerancesV1,
};

/// Schema identity for a transition-pose evaluation result.
pub const TRANSITION_POSE_EVALUATION_V1_ID: &str =
    "urn:animsmith:schema:transition-pose-evaluation:1";
/// Schema version for a transition-pose evaluation result.
pub const TRANSITION_POSE_EVALUATION_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum skeleton bones admitted by one V1 basis.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_BONES: usize = 4_096;
/// Maximum total authored UTF-8 bone-name bytes admitted to one V1 basis.
///
/// This shares the declaration V1 normalized-byte budget so basis identity
/// construction never clones unbounded skeleton text.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_BASIS_TEXT_BYTES: usize =
    crate::TRANSITION_FAMILY_V1_MAX_NORMALIZED_BYTES as usize;
/// Maximum clips admitted to document transition-pose witness resolution.
///
/// This aligns with the V1 collection/source clip domain. Above it, direct
/// declaration index/name contradictions still fail structurally, while an
/// otherwise valid declaration receives normal `input_limit` result rows
/// without a global duplicate-name scan.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_DOCUMENT_CLIPS: usize = 4_096;
/// Maximum raw flat track rows admitted for one selected clip.
///
/// The loader-facing document stores every property in one vector, so V1 must
/// bound that vector before it can discover the T/R rows it consumes. Scale
/// remains semantically ignored after this resource admission.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_RAW_TRACK_ROWS_PER_CLIP: usize =
    TRANSITION_POSE_EVALUATION_V1_MAX_BONES * 3;
/// Maximum selected tracks in one clip: translation and rotation per admitted
/// V1 skeleton bone. Scale is outside the V1 endpoint domain and is ignored.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP: usize =
    TRANSITION_POSE_EVALUATION_V1_MAX_BONES * 2;
/// Maximum aggregate selected time/value elements inspected by one evaluator
/// call. This reuses the immutable aggregate comparison-work bound.
pub const TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACK_ELEMENTS: usize =
    TRANSITION_POSE_EVALUATION_V1_MAX_AGGREGATE_COMPARISONS;
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
/// Conservative fixed JCS bytes reserved for every detailed pair/boundary
/// before endpoint sampling. It covers a complete pair row and its two
/// bounded offender arrays excluding escaped bone-name text.
const MAX_DETAILED_PAIR_FIXED_BYTES: usize = 4_096;

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
        if !skeleton_text_is_within_limit(
            skeleton,
            TRANSITION_POSE_EVALUATION_V1_MAX_BASIS_TEXT_BYTES,
        ) {
            return Err(SkeletonBasisError::TooMuchText);
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
    /// Authored bone names exceeded the basis text cap before normalization.
    #[error("transition-pose skeleton basis exceeds the bone-name text cap")]
    TooMuchText,
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
    DependencyClosureIncomplete,
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
    source_input: Option<InputIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_dependency_closure_identity: Option<DependencyClosureIdentityV1>,
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
    /// Exact raw source identity when the selected source was available.
    pub const fn source_input(&self) -> Option<&InputIdentity> {
        self.source_input.as_ref()
    }
    /// Exact complete dependency-closure identity for this selected source.
    pub const fn source_dependency_closure_identity(&self) -> Option<&DependencyClosureIdentityV1> {
        self.source_dependency_closure_identity.as_ref()
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

/// Immutable scope-neutral V1 result contract.
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
    subject_input: InputIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject_dependency_closure_identity: Option<DependencyClosureIdentityV1>,
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
    /// Exact raw identity of the declaration scope subject.
    ///
    /// The document evaluator binds the loaded document bytes here; a future
    /// collection adapter binds the manifest bytes under the same V1 schema.
    pub const fn subject_input(&self) -> &InputIdentity {
        &self.subject_input
    }
    /// Exact complete dependency-closure identity for the declaration subject.
    ///
    /// A document result omits this only for `no_configured_families` (which
    /// evaluates no source data) or `incomplete/not_evaluated` with
    /// `dependency_closure_incomplete`. A collection result omits it because
    /// its manifest subject has no asset dependency closure; member closure
    /// identities are the collection evaluation authority.
    pub const fn subject_dependency_closure_identity(
        &self,
    ) -> Option<&DependencyClosureIdentityV1> {
        self.subject_dependency_closure_identity.as_ref()
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
/// `dependency_closure.primary_input()` is the sole primary-byte authority.
/// Configured-family evaluation requires both complete closure coverage and
/// its exact [`DependencyClosureIdentityV1`]; otherwise every family is
/// retained as `dependency_closure_incomplete`. An empty declaration evaluates
/// no source data and preserves `no_configured_families` without requiring a
/// closure identity.
///
/// The mutable document's admitted skeleton and selected T/R tracks are
/// revalidated at this public boundary. All work planning happens before
/// endpoint sampling; an unavailable family is retained as
/// `incomplete/not_evaluated`, never evaluated as a survivor subset.
pub fn evaluate_document_transition_poses_v1(
    declaration: &TransitionFamilyDeclarationInputV1,
    dependency_closure: &DependencyClosureV1,
    document: &Document,
) -> Result<TransitionPoseEvaluationV1, TransitionPoseEvaluationControlError> {
    evaluate_document_transition_poses_v1_with_result_limit(
        declaration,
        dependency_closure,
        document,
        TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES,
    )
}

fn evaluate_document_transition_poses_v1_with_result_limit(
    declaration: &TransitionFamilyDeclarationInputV1,
    dependency_closure: &DependencyClosureV1,
    document: &Document,
    result_limit: usize,
) -> Result<TransitionPoseEvaluationV1, TransitionPoseEvaluationControlError> {
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
        subject_input: dependency_closure.primary_input().clone(),
        subject_dependency_closure_identity: complete_dependency_closure_identity(
            dependency_closure,
        ),
        families: Vec::with_capacity(families.len()),
    };
    if families.is_empty() {
        result.reason = Some(TransitionPoseReasonV1::NoConfiguredFamilies);
        return Ok(result);
    }
    // A declaration witness is a structural authority, not an evaluation
    // policy. Resolve every family before any tolerance or work-cap outcome
    // can produce a retained incomplete row.
    let clip_admission = resolve_document_family_clips(document, families)?;
    if result.subject_dependency_closure_identity.is_none() {
        for family in families {
            let mut row = family_result_row(family, dependency_closure, None);
            row.reason = Some(TransitionPoseReasonV1::DependencyClosureIncomplete);
            result.families.push(row);
        }
        derive_result_state(&mut result);
        canonical_bytes(&result, result_limit)
            .map_err(|_| TransitionPoseEvaluationControlError::ResultTooLarge)?;
        return Ok(result);
    }
    let resolved_clips = match clip_admission {
        DocumentClipAdmission::InputLimit => {
            for family in families {
                let mut row = family_result_row(family, dependency_closure, None);
                row.reason = Some(TransitionPoseReasonV1::InputLimit);
                result.families.push(row);
            }
            derive_result_state(&mut result);
            return Ok(result);
        }
        DocumentClipAdmission::Resolved(clips) => clips,
    };
    if !skeleton_input_is_within_limits(&document.skeleton) {
        for family in families {
            let mut row = family_result_row(family, dependency_closure, None);
            row.reason = Some(TransitionPoseReasonV1::InputLimit);
            result.families.push(row);
        }
        derive_result_state(&mut result);
        return Ok(result);
    }
    validate_transition_pose_skeleton(&document.skeleton)
        .map_err(TransitionPoseEvaluationControlError::InvalidSkeletonBasis)?;
    let basis = SkeletonBasisV1::from_skeleton(&document.skeleton)
        .map_err(TransitionPoseEvaluationControlError::InvalidSkeletonBasis)?;
    let policy_rejected = families
        .iter()
        .map(|family| family.tolerances().time_normalized() != 0.0)
        .collect::<Vec<_>>();
    let track_limits =
        plan_selected_track_input_limits(&resolved_clips, basis.bones().len(), &policy_rejected);
    let plans = plan_families(families, basis.bones().len(), &track_limits);
    let mut detailed_name_budget = detailed_result_budget_after_base(
        &result,
        families,
        dependency_closure,
        basis.identity(),
        result_limit,
    )?;
    for (((family, plan), clips), track_limit) in families
        .iter()
        .zip(plans)
        .zip(resolved_clips)
        .zip(track_limits)
    {
        let mut row = family_result_row(family, dependency_closure, Some(basis.identity()));
        if family.tolerances().time_normalized() != 0.0 {
            row.reason = Some(TransitionPoseReasonV1::TimeToleranceUnsupported);
        } else if let Some(reason) = plan.reason {
            row.reason = Some(reason);
        } else if track_limit {
            row.reason = Some(TransitionPoseReasonV1::InputLimit);
        } else if !reserve_detailed_name_budget(
            &mut detailed_name_budget,
            family,
            &document.skeleton,
        ) {
            row.reason = Some(TransitionPoseReasonV1::ResultLimit);
        } else {
            let endpoints = match strict_endpoints(document, &clips, family.boundary()) {
                Ok(value) => value,
                Err(reason) => {
                    row.reason = Some(reason);
                    result.families.push(row);
                    continue;
                }
            };
            row.pairs = match compare_pairs(
                &endpoints,
                family.boundary(),
                family.tolerances(),
                &document.skeleton,
            ) {
                Ok(pairs) => pairs,
                Err(reason) => {
                    row.reason = Some(reason);
                    result.families.push(row);
                    continue;
                }
            };
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
    enforce_result_limit(&mut result, result_limit)?;
    Ok(result)
}

fn family_result_row(
    family: &crate::DocumentTransitionFamilyV1,
    dependency_closure: &DependencyClosureV1,
    skeleton_basis_input: Option<&InputIdentity>,
) -> TransitionPoseFamilyEvaluationV1 {
    TransitionPoseFamilyEvaluationV1 {
        family_id: family.family_id().to_owned(),
        status: TransitionPoseStatusV1::Incomplete,
        decision: TransitionPoseDecisionV1::NotEvaluated,
        reason: None,
        members: family
            .members()
            .iter()
            .map(|member| TransitionPoseMemberV1 {
                take_index: member.take_index(),
                take_name: member.take_name().to_owned(),
                source_input: Some(dependency_closure.primary_input().clone()),
                source_dependency_closure_identity: complete_dependency_closure_identity(
                    dependency_closure,
                ),
            })
            .collect(),
        skeleton_basis_input: skeleton_basis_input.cloned(),
        pairs: Vec::new(),
    }
}

fn complete_dependency_closure_identity(
    dependency_closure: &DependencyClosureV1,
) -> Option<DependencyClosureIdentityV1> {
    if !dependency_closure.coverage().is_complete() {
        return None;
    }
    dependency_closure.identity().cloned()
}

/// Keep the immutable binding/member rows when detailed comparisons exceed a
/// bounded result envelope. The retry has one strictly smaller representation,
/// so it cannot oscillate.
fn enforce_result_limit(
    result: &mut TransitionPoseEvaluationV1,
    limit: usize,
) -> Result<(), TransitionPoseEvaluationControlError> {
    if canonical_bytes(result, limit).is_ok() {
        return Ok(());
    }
    for family in &mut result.families {
        family.status = TransitionPoseStatusV1::Incomplete;
        family.decision = TransitionPoseDecisionV1::NotEvaluated;
        family.reason = Some(TransitionPoseReasonV1::ResultLimit);
        family.pairs.clear();
    }
    derive_result_state(result);
    canonical_bytes(result, limit)
        .map(|_| ())
        .map_err(|_| TransitionPoseEvaluationControlError::ResultTooLarge)
}

#[derive(Clone, Copy)]
struct FamilyPlan {
    reason: Option<TransitionPoseReasonV1>,
}

fn plan_families(
    families: &[crate::DocumentTransitionFamilyV1],
    bone_count: usize,
    input_limited: &[bool],
) -> Vec<FamilyPlan> {
    let mut aggregate_pairs = 0usize;
    let mut aggregate_comparisons = 0usize;
    let mut aggregate_retention = 0usize;
    families
        .iter()
        .zip(input_limited.iter().copied())
        .map(|(family, input_limited)| {
            if input_limited || family.tolerances().time_normalized() != 0.0 {
                return FamilyPlan { reason: None };
            }
            let boundaries = match family.boundary() {
                TransitionFamilyBoundaryV1::Entry | TransitionFamilyBoundaryV1::Exit => 1usize,
                TransitionFamilyBoundaryV1::Both => 2usize,
            };
            let pairs = checked_pair_count(family.members().len());
            let pair_boundaries = pairs.and_then(|value| value.checked_mul(boundaries));
            let comparisons = pair_boundaries.and_then(|value| value.checked_mul(bone_count));
            let retained_per_pair_boundary = bone_count
                .min(TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS)
                .checked_add(bone_count.min(TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS));
            let retention = pair_boundaries.and_then(|value| {
                retained_per_pair_boundary.and_then(|cap| value.checked_mul(cap))
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

enum DocumentClipAdmission<'a> {
    InputLimit,
    Resolved(Vec<Vec<&'a Clip>>),
}

/// Resolve all declaration witnesses with one bounded document-name pass.
/// Direct index/name contradictions are checked before the document clip cap;
/// global duplicate-name proof is intentionally only attempted in the
/// admitted domain.
fn resolve_document_family_clips<'a>(
    document: &'a Document,
    families: &[crate::DocumentTransitionFamilyV1],
) -> Result<DocumentClipAdmission<'a>, TransitionPoseEvaluationControlError> {
    let mut names = BTreeMap::<&str, usize>::new();
    for family in families {
        for member in family.members() {
            let index = usize::try_from(member.take_index())
                .map_err(|_| TransitionPoseEvaluationControlError::InvalidMemberWitness)?;
            document
                .clips
                .get(index)
                .filter(|clip| clip.name == member.take_name())
                .ok_or(TransitionPoseEvaluationControlError::InvalidMemberWitness)?;
            names.entry(member.take_name()).or_insert(0);
        }
    }
    if document.clips.len() > TRANSITION_POSE_EVALUATION_V1_MAX_DOCUMENT_CLIPS {
        return Ok(DocumentClipAdmission::InputLimit);
    }
    for clip in &document.clips {
        if let Some(count) = names.get_mut(clip.name.as_str()) {
            *count = count.saturating_add(1);
        }
    }
    if names.values().any(|&count| count != 1) {
        return Err(TransitionPoseEvaluationControlError::InvalidMemberWitness);
    }
    families
        .iter()
        .map(|family| {
            family
                .members()
                .iter()
                .map(|member| {
                    let index = usize::try_from(member.take_index())
                        .map_err(|_| TransitionPoseEvaluationControlError::InvalidMemberWitness)?;
                    document
                        .clips
                        .get(index)
                        .ok_or(TransitionPoseEvaluationControlError::InvalidMemberWitness)
                })
                .collect()
        })
        .collect::<Result<Vec<Vec<_>>, _>>()
        .map(DocumentClipAdmission::Resolved)
}

/// Bound skeleton text before `SkeletonBasisV1` clones a single bone name.
fn skeleton_input_is_within_limits(skeleton: &Skeleton) -> bool {
    skeleton_input_is_within_limits_with(
        skeleton,
        TRANSITION_POSE_EVALUATION_V1_MAX_BASIS_TEXT_BYTES,
    )
}

fn skeleton_input_is_within_limits_with(skeleton: &Skeleton, text_limit: usize) -> bool {
    if skeleton.bones.len() > TRANSITION_POSE_EVALUATION_V1_MAX_BONES {
        return false;
    }
    skeleton_text_is_within_limit(skeleton, text_limit)
}

fn skeleton_text_is_within_limit(skeleton: &Skeleton, text_limit: usize) -> bool {
    skeleton
        .bones
        .iter()
        .try_fold(0usize, |total, bone| total.checked_add(bone.name.len()))
        .is_some_and(|total| total <= text_limit)
}

/// Validate only transition-pose's skeleton authority. Source projections,
/// mesh assets, inverse binds, and unselected clips are deliberately not an
/// evaluator input domain.
fn validate_transition_pose_skeleton(skeleton: &Skeleton) -> Result<(), SkeletonBasisError> {
    for (ordinal, bone) in skeleton.bones.iter().enumerate() {
        if matches!(bone.parent, Some(parent) if parent >= ordinal) {
            return Err(SkeletonBasisError::InvalidParent { ordinal });
        }
        if finite_translation(bone, ordinal).is_err()
            || canonical_quaternion(bone.rest.rotation, ordinal).is_err()
        {
            return Err(SkeletonBasisError::InvalidRest { ordinal });
        }
    }
    Ok(())
}

/// Bound selected-track shape work before allocating duplicate-target state or
/// sampling. Only translation/rotation tracks are selected: scale is outside
/// V1 and is never counted, validated, or sampled. Each selected clip can
/// therefore target two channels per admitted bone.
fn plan_selected_track_input_limits(
    resolved_clips: &[Vec<&Clip>],
    bone_count: usize,
    policy_rejected: &[bool],
) -> Vec<bool> {
    let (Some(raw_track_limit), Some(track_limit)) =
        (bone_count.checked_mul(3), bone_count.checked_mul(2))
    else {
        return vec![true; resolved_clips.len()];
    };
    plan_selected_track_input_limits_with(
        resolved_clips,
        raw_track_limit,
        track_limit,
        TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACK_ELEMENTS,
        policy_rejected,
    )
}

fn plan_selected_track_input_limits_with(
    resolved_clips: &[Vec<&Clip>],
    raw_track_limit: usize,
    track_limit: usize,
    element_limit: usize,
    policy_rejected: &[bool],
) -> Vec<bool> {
    let mut aggregate_elements = 0usize;
    resolved_clips
        .iter()
        .zip(policy_rejected.iter().copied())
        .map(|(clips, policy_rejected)| {
            if policy_rejected {
                return false;
            }
            let mut family_elements = 0usize;
            for clip in clips {
                if clip.tracks.len() > raw_track_limit {
                    return true;
                }
                let mut selected_tracks = 0usize;
                for track in clip
                    .tracks
                    .iter()
                    .filter(|track| is_transition_pose_property(track.property))
                {
                    let Some(count) = selected_tracks.checked_add(1) else {
                        return true;
                    };
                    if count > track_limit {
                        return true;
                    }
                    selected_tracks = count;
                    let elements = track.times.len().checked_add(track.values.len());
                    let Some(total) =
                        elements.and_then(|elements| family_elements.checked_add(elements))
                    else {
                        return true;
                    };
                    if total > element_limit {
                        return true;
                    }
                    family_elements = total;
                }
            }
            let Some(total) = aggregate_elements.checked_add(family_elements) else {
                return true;
            };
            if total > element_limit {
                return true;
            }
            aggregate_elements = total;
            false
        })
        .collect()
}

/// Serialize a conservative pair-free authority before retaining any detailed
/// row. Every family is represented with the longest V1 incomplete reason,
/// so this is an upper bound for all final no-pair rows while preserving the
/// exact declaration, subject, member, and basis bindings.
fn detailed_result_budget_after_base(
    result: &TransitionPoseEvaluationV1,
    families: &[crate::DocumentTransitionFamilyV1],
    dependency_closure: &DependencyClosureV1,
    basis_input: &InputIdentity,
    limit: usize,
) -> Result<usize, TransitionPoseEvaluationControlError> {
    let mut base = result.clone();
    base.status = TransitionPoseStatusV1::Incomplete;
    base.decision = TransitionPoseDecisionV1::NotEvaluated;
    base.reason = None;
    base.families = families
        .iter()
        .map(|family| {
            let mut row = family_result_row(family, dependency_closure, Some(basis_input));
            row.reason = Some(TransitionPoseReasonV1::TimeToleranceUnsupported);
            row
        })
        .collect();
    let base_bytes = canonical_bytes(&base, limit)
        .map_err(|_| TransitionPoseEvaluationControlError::ResultTooLarge)?
        .len();
    limit
        .checked_sub(base_bytes)
        .ok_or(TransitionPoseEvaluationControlError::ResultTooLarge)
}

/// Reserve a conservative complete detailed-pair envelope before sampling.
/// The fixed component covers every pair field and the two 16-row offender
/// arrays. Names are escaped by JSON/JCS, so six bytes per source byte is a
/// safe extra bound for every retained name. A row's ordinal is unique,
/// permitting this bound without cloning names just to decide whether they
/// would be retained.
fn reserve_detailed_name_budget(
    remaining: &mut usize,
    family: &crate::DocumentTransitionFamilyV1,
    skeleton: &Skeleton,
) -> bool {
    let boundaries = match family.boundary() {
        TransitionFamilyBoundaryV1::Entry | TransitionFamilyBoundaryV1::Exit => 1usize,
        TransitionFamilyBoundaryV1::Both => 2usize,
    };
    let Some(pair_boundaries) =
        checked_pair_count(family.members().len()).and_then(|pairs| pairs.checked_mul(boundaries))
    else {
        return false;
    };
    let Some(rows_per_pair_boundary) = skeleton
        .bones
        .len()
        .min(TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS)
        .checked_add(
            skeleton
                .bones
                .len()
                .min(TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS),
        )
    else {
        return false;
    };
    let fixed_bytes = pair_boundaries.checked_mul(MAX_DETAILED_PAIR_FIXED_BYTES);
    let max_escaped_name_bytes = skeleton
        .bones
        .iter()
        .map(|bone| bone.name.len().checked_mul(6))
        .max()
        .unwrap_or(Some(0));
    let Some(bytes) = fixed_bytes.and_then(|fixed| {
        max_escaped_name_bytes.and_then(|name| {
            pair_boundaries
                .checked_mul(rows_per_pair_boundary)?
                .checked_mul(name)
                .and_then(|names| fixed.checked_add(names))
        })
    }) else {
        return false;
    };
    if bytes > *remaining {
        return false;
    }
    *remaining -= bytes;
    true
}

#[derive(Clone)]
struct Endpoints {
    entry: Option<Vec<EndpointPose>>,
    exit: Option<Vec<EndpointPose>>,
}

#[derive(Clone, Copy)]
struct EndpointPose {
    translation: [f64; 3],
    rotation: [f64; 4],
}

fn strict_endpoints(
    document: &Document,
    clips: &[&Clip],
    boundary: TransitionFamilyBoundaryV1,
) -> Result<Vec<Endpoints>, TransitionPoseReasonV1> {
    clips
        .iter()
        .map(|clip| {
            if !selected_tracks_are_strict(clip, document.skeleton.bones.len()) {
                return Err(TransitionPoseReasonV1::UnsupportedSampling);
            }
            if clip.duration_s == 0.0 {
                return Err(TransitionPoseReasonV1::ZeroDuration);
            }
            if !clip.duration_s.is_finite() || clip.duration_s < 0.0 {
                return Err(TransitionPoseReasonV1::UnsupportedSampling);
            }
            let needs_exit = matches!(
                boundary,
                TransitionFamilyBoundaryV1::Exit | TransitionFamilyBoundaryV1::Both
            );
            let exit_time = needs_exit.then(|| {
                let time = clip.duration_s as f32;
                if !time.is_finite() || f64::from(time) != clip.duration_s {
                    Err(TransitionPoseReasonV1::UnsupportedSampling)
                } else {
                    Ok(time)
                }
            });
            Ok(Endpoints {
                entry: matches!(
                    boundary,
                    TransitionFamilyBoundaryV1::Entry | TransitionFamilyBoundaryV1::Both
                )
                .then(|| strict_endpoint(&document.skeleton, clip, 0.0))
                .transpose()?,
                exit: exit_time
                    .transpose()?
                    .map(|time| strict_endpoint(&document.skeleton, clip, time))
                    .transpose()?,
            })
        })
        .collect()
}

fn selected_tracks_are_strict(clip: &Clip, bone_count: usize) -> bool {
    selected_tracks_are_strict_with(clip, bone_count, |_| {})
}

/// Validate each selected T/R track exactly once. The two-channel seen table
/// is bounded by the already-admitted skeleton and avoids quadratic duplicate
/// checks at the V1 track cap.
fn selected_tracks_are_strict_with(
    clip: &Clip,
    bone_count: usize,
    mut observe_selected: impl FnMut(&Track),
) -> bool {
    let (Some(raw_track_limit), Some(seen_len)) =
        (bone_count.checked_mul(3), bone_count.checked_mul(2))
    else {
        return false;
    };
    if clip.tracks.len() > raw_track_limit {
        return false;
    }
    let mut seen = vec![false; seen_len];
    for track in &clip.tracks {
        let Some(channel) = transition_pose_channel(track.property) else {
            continue;
        };
        observe_selected(track);
        if track.bone >= bone_count {
            return false;
        }
        let Some(index) = track
            .bone
            .checked_mul(2)
            .and_then(|base| base.checked_add(channel))
        else {
            return false;
        };
        if seen[index] || validate_track_shape(0, track).is_err() {
            return false;
        }
        seen[index] = true;
    }
    true
}

fn is_transition_pose_property(property: Property) -> bool {
    transition_pose_channel(property).is_some()
}

fn transition_pose_channel(property: Property) -> Option<usize> {
    match property {
        Property::Translation => Some(0),
        Property::Rotation => Some(1),
        Property::Scale => None,
    }
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
        match track.property {
            Property::Scale => continue,
            Property::Translation => match crate::sample_track(track, time) {
                TrackSample::Vec3(value) => {
                    let target = poses
                        .get_mut(track.bone)
                        .ok_or(TransitionPoseReasonV1::UnsupportedSampling)?;
                    target.translation =
                        finite_vec3(value).ok_or(TransitionPoseReasonV1::UnsupportedSampling)?;
                }
                _ => return Err(TransitionPoseReasonV1::UnsupportedSampling),
            },
            Property::Rotation => match crate::sample_track(track, time) {
                TrackSample::Quat(value) => {
                    let target = poses
                        .get_mut(track.bone)
                        .ok_or(TransitionPoseReasonV1::UnsupportedSampling)?;
                    target.rotation = canonical_quaternion(value, track.bone)
                        .map_err(|_| TransitionPoseReasonV1::UnsupportedSampling)?;
                }
                _ => return Err(TransitionPoseReasonV1::UnsupportedSampling),
            },
        }
    }
    Ok(poses)
}

fn compare_pairs(
    endpoints: &[Endpoints],
    boundary: TransitionFamilyBoundaryV1,
    tolerances: TransitionFamilyTolerancesV1,
    skeleton: &Skeleton,
) -> Result<Vec<TransitionPosePairEvaluationV1>, TransitionPoseReasonV1> {
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
                let (left_pose, right_pose) = if boundary == TransitionFamilyBoundaryV1::Entry {
                    (
                        endpoints[left].entry.as_deref(),
                        endpoints[right].entry.as_deref(),
                    )
                } else {
                    (
                        endpoints[left].exit.as_deref(),
                        endpoints[right].exit.as_deref(),
                    )
                };
                let (left_pose, right_pose) = match (left_pose, right_pose) {
                    (Some(left_pose), Some(right_pose)) => (left_pose, right_pose),
                    _ => return Err(TransitionPoseReasonV1::UnsupportedSampling),
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
    Ok(output)
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
    let mut translation_candidates =
        Vec::with_capacity(TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS);
    let mut rotation_candidates =
        Vec::with_capacity(TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS);
    for (ordinal, (left, right)) in left.iter().zip(right).enumerate() {
        let translation = translation_delta(left.translation, right.translation);
        let rotation = rotation_delta_deg(left.rotation, right.rotation);
        max_translation_delta_m = max_translation_delta_m.max(translation);
        max_rotation_delta_deg = max_rotation_delta_deg.max(rotation);
        if translation > tolerances.translation_m() {
            retain_top_candidate(
                &mut translation_candidates,
                ordinal,
                translation,
                TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS,
            );
        }
        if rotation > tolerances.rotation_deg() {
            retain_top_candidate(
                &mut rotation_candidates,
                ordinal,
                rotation,
                TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS,
            );
        }
    }
    sort_top_candidates(&mut translation_candidates);
    sort_top_candidates(&mut rotation_candidates);
    let translation_offenders = translation_candidates
        .into_iter()
        .map(
            |(bone_ordinal, delta_m)| TransitionPoseTranslationOffenderV1 {
                bone_ordinal,
                bone_name: skeleton.bones[bone_ordinal].name.clone(),
                delta_m,
            },
        )
        .collect();
    let rotation_offenders = rotation_candidates
        .into_iter()
        .map(
            |(bone_ordinal, delta_deg)| TransitionPoseRotationOffenderV1 {
                bone_ordinal,
                bone_name: skeleton.bones[bone_ordinal].name.clone(),
                delta_deg,
            },
        )
        .collect();
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

/// Retain a candidate only when it belongs in the bounded final ordering.
/// Ordinals are unique within one skeleton, so the public tertiary bone-name
/// tie-break is unreachable after the ordinal tie-break and no name is needed
/// while selecting.
fn retain_top_candidate(
    candidates: &mut Vec<(usize, f64)>,
    ordinal: usize,
    delta: f64,
    cap: usize,
) {
    let candidate = (ordinal, delta);
    if candidates.len() < cap {
        candidates.push(candidate);
        return;
    }
    let worst = candidates
        .iter()
        .enumerate()
        .reduce(|worst, current| {
            if candidate_precedes(worst.1, current.1) {
                current
            } else {
                worst
            }
        })
        .map(|(index, _)| index);
    if let Some(worst) = worst
        && candidate_precedes(&candidate, &candidates[worst])
    {
        candidates[worst] = candidate;
    }
}

fn sort_top_candidates(candidates: &mut [(usize, f64)]) {
    candidates.sort_by(candidate_order);
}

fn candidate_precedes(left: &(usize, f64), right: &(usize, f64)) -> bool {
    candidate_order(left, right).is_lt()
}

fn candidate_order(left: &(usize, f64), right: &(usize, f64)) -> std::cmp::Ordering {
    right
        .1
        .total_cmp(&left.1)
        .then_with(|| left.0.cmp(&right.0))
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
    if left == right
        || left
            .into_iter()
            .zip(right)
            .all(|(left, right)| left == -right)
    {
        return 0.0;
    }
    let dot = left
        .into_iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let right = if dot < 0.0 {
        right.map(|value| -value)
    } else {
        right
    };
    // `conjugate(left) * right`: atan2 is stable at identity, unlike acos of
    // a dot product rounded a hair below one after f32-to-f64 normalization.
    let vector = [
        left[3] * right[0] - left[0] * right[3] - left[1] * right[2] + left[2] * right[1],
        left[3] * right[1] + left[0] * right[2] - left[1] * right[3] - left[2] * right[0],
        left[3] * right[2] - left[0] * right[1] + left[1] * right[0] - left[2] * right[3],
    ];
    let vector_norm = vector
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    let scalar =
        (left[3] * right[3] + left[0] * right[0] + left[1] * right[1] + left[2] * right[2]).abs();
    (2.0 * vector_norm.atan2(scalar)).to_degrees()
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
        Bone, Clip, CollectionIdV1, CollectionLogicalIdV1, CollectionSourceKeyV1,
        CollectionTransitionFamilyMemberV1, CollectionTransitionFamilyV1,
        DependencyClosureBuilderV1, DependencyResourceKeyV1, Document,
        DocumentTransitionFamilyMemberV1, DocumentTransitionFamilyV1, Interpolation,
        ResourceKeySyntaxV1, Skeleton, SourceResourceKindV1, SourceSetCoverageV1, Track,
        TrackValues, Transform, TransitionFamilyDeclarationV1, TransitionFamilyManifestIdentityV1,
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

    fn complete_closure(primary_input: InputIdentity) -> DependencyClosureV1 {
        DependencyClosureBuilderV1::new(primary_input, SourceSetCoverageV1::complete(), 0)
            .finish()
            .unwrap()
    }

    fn closure_with_external_buffer(
        primary_input: InputIdentity,
        buffer_bytes: &[u8],
    ) -> DependencyClosureV1 {
        let key =
            DependencyResourceKeyV1::from_source_str("animation.bin", ResourceKeySyntaxV1::GltfUri)
                .unwrap();
        let mut builder =
            DependencyClosureBuilderV1::new(primary_input, SourceSetCoverageV1::complete(), 1);
        assert!(builder.begin_reference("animation.bin".len(), 1));
        assert_eq!(builder.prepare_external_key(&key).unwrap(), Some(true));
        builder.record_external_open_attempt(&key).unwrap();
        assert!(
            builder
                .push_captured_external(
                    0,
                    SourceResourceKindV1::Buffer,
                    0,
                    key,
                    InputIdentity::from_bytes(buffer_bytes),
                )
                .unwrap()
        );
        builder.finish().unwrap()
    }

    fn evaluate_document_transition_poses_v1(
        declaration: &TransitionFamilyDeclarationInputV1,
        subject_input: InputIdentity,
        document: &Document,
    ) -> Result<TransitionPoseEvaluationV1, TransitionPoseEvaluationControlError> {
        let closure = complete_closure(subject_input);
        super::evaluate_document_transition_poses_v1(declaration, &closure, document)
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
        let parent_after_child = Skeleton {
            bones: vec![
                Bone {
                    name: "child".into(),
                    parent: Some(1),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "parent".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        assert_eq!(
            SkeletonBasisV1::from_skeleton(&parent_after_child),
            Err(SkeletonBasisError::InvalidParent { ordinal: 0 })
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
        assert_eq!(result.subject_input(), &raw);
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
    fn complete_results_bind_external_dependency_changes_beyond_primary_bytes() {
        let declared = declaration(TransitionFamilyBoundaryV1::Entry, 100.0, 0.0);
        let primary = InputIdentity::from_bytes(br#"{"buffers":[{"uri":"animation.bin"}]}"#);
        let first_closure = closure_with_external_buffer(primary.clone(), b"first animation");
        let second_closure = closure_with_external_buffer(primary.clone(), b"second animation");

        let first = super::evaluate_document_transition_poses_v1(
            &declared,
            &first_closure,
            &document(Some(1.0)),
        )
        .unwrap();
        let second = super::evaluate_document_transition_poses_v1(
            &declared,
            &second_closure,
            &document(Some(2.0)),
        )
        .unwrap();

        assert_eq!(first.subject_input(), second.subject_input());
        assert_eq!(first.subject_input(), &primary);
        assert_ne!(
            first.subject_dependency_closure_identity(),
            second.subject_dependency_closure_identity()
        );
        assert_eq!(first.status(), TransitionPoseStatusV1::Complete);
        assert_eq!(second.status(), TransitionPoseStatusV1::Complete);
        assert_ne!(
            first.families()[0].pairs()[0].max_translation_delta_m(),
            second.families()[0].pairs()[0].max_translation_delta_m()
        );
        for result in [&first, &second] {
            let subject_closure = result
                .subject_dependency_closure_identity()
                .expect("complete result closure identity");
            assert!(result.families()[0].members().iter().all(|member| {
                member.source_input() == Some(result.subject_input())
                    && member.source_dependency_closure_identity() == Some(subject_closure)
            }));
        }
        assert_ne!(
            first.normalized_jcs().unwrap(),
            second.normalized_jcs().unwrap()
        );
    }

    #[test]
    fn incomplete_dependency_closure_cannot_produce_a_complete_outcome() {
        let primary = InputIdentity::from_bytes(b"document");
        let closure = DependencyClosureV1::unavailable(primary.clone());
        assert!(!closure.coverage().is_complete());
        assert!(closure.identity().is_none());

        let result = super::evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 100.0, 0.0),
            &closure,
            &document(Some(1.0)),
        )
        .unwrap();
        assert_eq!(result.subject_input(), &primary);
        assert_eq!(result.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(result.decision(), TransitionPoseDecisionV1::NotEvaluated);
        assert!(result.subject_dependency_closure_identity().is_none());
        assert_eq!(
            result.families()[0].reason(),
            Some(TransitionPoseReasonV1::DependencyClosureIncomplete)
        );
        assert!(result.families()[0].members().iter().all(|member| {
            member.source_input() == Some(&primary)
                && member.source_dependency_closure_identity().is_none()
        }));
        let serialized_len = result.normalized_jcs().unwrap().len();
        let exact = evaluate_document_transition_poses_v1_with_result_limit(
            &declaration(TransitionFamilyBoundaryV1::Entry, 100.0, 0.0),
            &closure,
            &document(Some(1.0)),
            serialized_len,
        )
        .unwrap();
        assert_eq!(
            exact.families()[0].reason(),
            Some(TransitionPoseReasonV1::DependencyClosureIncomplete)
        );
        assert_eq!(
            evaluate_document_transition_poses_v1_with_result_limit(
                &declaration(TransitionFamilyBoundaryV1::Entry, 100.0, 0.0),
                &closure,
                &document(Some(1.0)),
                serialized_len - 1,
            ),
            Err(TransitionPoseEvaluationControlError::ResultTooLarge)
        );

        let empty = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
            b"empty",
        )
        .unwrap();
        let no_config =
            super::evaluate_document_transition_poses_v1(&empty, &closure, &document(None))
                .unwrap();
        assert_eq!(no_config.status(), TransitionPoseStatusV1::Complete);
        assert_eq!(no_config.decision(), TransitionPoseDecisionV1::Pass);
        assert_eq!(
            no_config.reason(),
            Some(TransitionPoseReasonV1::NoConfiguredFamilies)
        );
        assert!(no_config.subject_dependency_closure_identity().is_none());
    }

    #[test]
    fn rotation_delta_is_reflexive_sign_invariant_and_has_the_right_angle() {
        let axis = crate::glam::Vec3::new(1.0, 2.0, 3.0).normalize();
        for angle in [0.1, 0.9, 1.7] {
            let rotation =
                canonical_quaternion(crate::glam::Quat::from_axis_angle(axis, angle), 0).unwrap();
            assert_eq!(rotation_delta_deg(rotation, rotation), 0.0);
            assert_eq!(
                rotation_delta_deg(rotation, rotation.map(|value| -value)),
                0.0
            );
        }
        let left = canonical_quaternion(
            crate::glam::Quat::from_axis_angle(axis, 40f32.to_radians()),
            0,
        )
        .unwrap();
        let right = canonical_quaternion(
            crate::glam::Quat::from_axis_angle(axis, 130f32.to_radians()),
            0,
        )
        .unwrap();
        assert!((rotation_delta_deg(left, right) - 90.0).abs() < 1e-5);

        let mut document = document(None);
        let rotation = crate::glam::Quat::from_axis_angle(axis, 0.9);
        for clip in &mut document.clips {
            clip.tracks.push(Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Quats(vec![rotation]),
            });
        }
        let result = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        assert_eq!(result.decision(), TransitionPoseDecisionV1::Pass);
        assert_eq!(
            result.families()[0].pairs()[0].max_rotation_delta_deg(),
            0.0
        );
        document.clips[1].tracks[0].values = TrackValues::Quats(vec![-rotation]);
        let sign_equivalent = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        assert_eq!(sign_equivalent.decision(), TransitionPoseDecisionV1::Pass);
        assert_eq!(
            sign_equivalent.families()[0].pairs()[0].max_rotation_delta_deg(),
            0.0
        );
    }

    #[test]
    fn scale_tracks_are_wholly_ignored_by_v1_admission_validation_and_sampling() {
        let declared = declaration(TransitionFamilyBoundaryV1::Both, 0.0, 0.0);
        let raw = InputIdentity::from_bytes(b"document");
        let baseline =
            evaluate_document_transition_poses_v1(&declared, raw.clone(), &document(Some(1.0)))
                .unwrap();
        let mut with_scale_noise = document(Some(1.0));
        let malformed_scale = Track {
            bone: usize::MAX,
            property: Property::Scale,
            interpolation: Interpolation::Linear,
            times: vec![f32::NAN],
            values: TrackValues::Vec3s(Vec::new()),
        };
        // One selected translation plus two malformed scale rows is exactly
        // the raw 3*bones admission cap. The scale rows remain semantically
        // invisible once that resource boundary is admitted.
        with_scale_noise.clips[1]
            .tracks
            .extend(std::iter::repeat_n(malformed_scale.clone(), 2));
        let with_scale_noise =
            evaluate_document_transition_poses_v1(&declared, raw.clone(), &with_scale_noise)
                .unwrap();
        assert_eq!(with_scale_noise, baseline);

        let mut one_too_many = document(Some(1.0));
        one_too_many.clips[1]
            .tracks
            .extend(std::iter::repeat_n(malformed_scale, 3));
        let one_too_many =
            evaluate_document_transition_poses_v1(&declared, raw, &one_too_many).unwrap();
        assert_eq!(
            one_too_many.families()[0].reason(),
            Some(TransitionPoseReasonV1::InputLimit)
        );
    }

    #[test]
    fn selected_track_validation_is_linear_and_rejects_duplicate_tr_channels() {
        let mut tracks =
            Vec::with_capacity(TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP);
        for bone in 0..TRANSITION_POSE_EVALUATION_V1_MAX_BONES {
            tracks.push(Track {
                bone,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Vec3s(vec![crate::glam::Vec3::ZERO]),
            });
            tracks.push(Track {
                bone,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values: TrackValues::Quats(vec![crate::glam::Quat::IDENTITY]),
            });
        }
        let mut clip = Clip {
            name: "all-tr".into(),
            duration_s: 1.0,
            tracks,
        };
        let mut visits = 0usize;
        assert!(selected_tracks_are_strict_with(
            &clip,
            TRANSITION_POSE_EVALUATION_V1_MAX_BONES,
            |_| visits += 1,
        ));
        assert_eq!(
            visits,
            TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP
        );

        clip.tracks.push(clip.tracks[0].clone());
        assert!(!selected_tracks_are_strict(
            &clip,
            TRANSITION_POSE_EVALUATION_V1_MAX_BONES
        ));
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
    fn no_configured_families_does_not_traverse_mutable_document_payloads() {
        let mut document = document(None);
        document.skeleton.bones[0].parent = Some(0);
        let empty = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(Vec::new()).unwrap(),
            b"empty",
        )
        .unwrap();
        let result = evaluate_document_transition_poses_v1(
            &empty,
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        assert_eq!(
            result.reason(),
            Some(TransitionPoseReasonV1::NoConfiguredFamilies)
        );
    }

    #[test]
    fn admission_limits_precede_selected_allocation_but_not_witness_control() {
        let mut oversized = document(None);
        oversized.skeleton.bones = (0..TRANSITION_POSE_EVALUATION_V1_MAX_BONES + 1)
            .map(|ordinal| Bone {
                name: format!("bone-{ordinal}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect();
        let invalid = DocumentTransitionFamilyV1::new(
            "invalid".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            vec![
                DocumentTransitionFamilyMemberV1::new(2, "missing-a".into()).unwrap(),
                DocumentTransitionFamilyMemberV1::new(3, "missing-b".into()).unwrap(),
            ],
        )
        .unwrap();
        let invalid = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![invalid]).unwrap(),
            b"declaration",
        )
        .unwrap();
        assert_eq!(
            evaluate_document_transition_poses_v1(
                &invalid,
                InputIdentity::from_bytes(b"document"),
                &oversized,
            ),
            Err(TransitionPoseEvaluationControlError::InvalidMemberWitness)
        );

        let mut selected = document(None);
        let track = Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0],
            values: TrackValues::Vec3s(vec![crate::glam::Vec3::ZERO]),
        };
        selected.clips[1].tracks =
            vec![track.clone(); TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP + 1];
        let limited = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &selected,
        )
        .unwrap();
        assert_eq!(
            limited.families()[0].reason(),
            Some(TransitionPoseReasonV1::InputLimit)
        );

        let small_selected = document(Some(1.0));
        let selected_clip = &small_selected.clips[1];
        assert_eq!(
            plan_selected_track_input_limits_with(&[vec![selected_clip]], 1, 1, 1, &[false]),
            vec![true]
        );
        let oversized_tracks = Clip {
            name: "many".into(),
            duration_s: 1.0,
            tracks: vec![track.clone(); 2],
        };
        let small = Clip {
            name: "small".into(),
            duration_s: 1.0,
            tracks: vec![track.clone()],
        };
        assert_eq!(
            plan_selected_track_input_limits_with(
                &[vec![&oversized_tracks], vec![&small]],
                1,
                1,
                8,
                &[false, false],
            ),
            vec![true, false]
        );
        assert_eq!(
            plan_selected_track_input_limits_with(
                &[vec![&small], vec![&oversized_tracks], vec![&small]],
                3,
                3,
                5,
                &[false, false, false],
            ),
            vec![false, true, false]
        );
        let huge_unsupported = Clip {
            name: "policy-only".into(),
            duration_s: 1.0,
            tracks: vec![track.clone(); 4],
        };
        assert_eq!(
            plan_selected_track_input_limits_with(
                &[vec![&huge_unsupported], vec![&small]],
                3,
                3,
                5,
                &[true, false],
            ),
            vec![false, false]
        );
        let named = Skeleton {
            bones: vec![Bone {
                name: "x".repeat(16),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        assert!(!skeleton_input_is_within_limits_with(&named, 15));
        assert_eq!(
            TRANSITION_POSE_EVALUATION_V1_MAX_RAW_TRACK_ROWS_PER_CLIP,
            TRANSITION_POSE_EVALUATION_V1_MAX_BONES * 3
        );
        let oversized_name = Skeleton {
            bones: vec![Bone {
                name: "x".repeat(TRANSITION_POSE_EVALUATION_V1_MAX_BASIS_TEXT_BYTES + 1),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            }],
        };
        assert_eq!(
            SkeletonBasisV1::from_skeleton(&oversized_name),
            Err(SkeletonBasisError::TooMuchText)
        );

        let four_tracks = Clip {
            name: "four".into(),
            duration_s: 1.0,
            tracks: vec![track.clone(); 4],
        };
        assert_eq!(
            plan_selected_track_input_limits(&[vec![&four_tracks]], 1, &[false]),
            vec![true]
        );

        let mut scaled = document(None);
        scaled.skeleton.bones[0].rest.scale = crate::glam::Vec3::NAN;
        assert_eq!(
            evaluate_document_transition_poses_v1(
                &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
                InputIdentity::from_bytes(b"document"),
                &scaled,
            )
            .unwrap()
            .status(),
            TransitionPoseStatusV1::Complete
        );
    }

    #[test]
    fn clip_admission_bounds_global_witness_lookup_after_direct_witness_checks() {
        let declared = declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0);
        let mut over_cap = document(None);
        for ordinal in over_cap.clips.len()..TRANSITION_POSE_EVALUATION_V1_MAX_DOCUMENT_CLIPS {
            over_cap.clips.push(Clip {
                name: format!("extra-{ordinal}"),
                duration_s: 1.0,
                tracks: Vec::new(),
            });
        }
        // The first excess row duplicates a declared name. It deliberately is
        // not globally scanned once the document leaves the admitted domain.
        over_cap.clips.push(Clip {
            name: "Walk".into(),
            duration_s: 1.0,
            tracks: Vec::new(),
        });
        let limited = evaluate_document_transition_poses_v1(
            &declared,
            InputIdentity::from_bytes(b"document"),
            &over_cap,
        )
        .unwrap();
        assert_eq!(
            limited.families()[0].reason(),
            Some(TransitionPoseReasonV1::InputLimit)
        );

        over_cap.clips[0].name = "stale".into();
        assert_eq!(
            evaluate_document_transition_poses_v1(
                &declared,
                InputIdentity::from_bytes(b"document"),
                &over_cap,
            ),
            Err(TransitionPoseEvaluationControlError::InvalidMemberWitness)
        );
    }

    #[test]
    fn unrelated_assets_and_clips_do_not_block_transition_pose() {
        let mut document = document(None);
        document.assets.instances.push(crate::model::MeshInstance {
            node: 99,
            ..crate::model::MeshInstance::default()
        });
        let track = Track {
            bone: 99,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: Vec::new(),
            values: TrackValues::Vec3s(Vec::new()),
        };
        document.clips.push(Clip {
            name: "Unselected".into(),
            duration_s: 1.0,
            tracks: vec![track; TRANSITION_POSE_EVALUATION_V1_MAX_SELECTED_TRACKS_PER_CLIP + 1],
        });
        assert_eq!(
            evaluate_document_transition_poses_v1(
                &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
                InputIdentity::from_bytes(b"document"),
                &document,
            )
            .unwrap()
            .status(),
            TransitionPoseStatusV1::Complete
        );
    }

    #[test]
    fn public_evaluator_rejects_wrong_scope_and_oversized_basis() {
        let collection_id = CollectionIdV1::new("collection").unwrap();
        let manifest = TransitionFamilyManifestIdentityV1::new(
            collection_id.clone(),
            InputIdentity::from_bytes(b"manifest"),
        )
        .unwrap();
        let members = ["one", "two"]
            .into_iter()
            .map(|suffix| {
                CollectionTransitionFamilyMemberV1::new(
                    CollectionLogicalIdV1::new(format!("collection/{suffix}")).unwrap(),
                    CollectionSourceKeyV1::new("source").unwrap(),
                    0,
                    suffix.into(),
                )
                .unwrap()
            })
            .collect();
        let collection = TransitionFamilyDeclarationV1::collection(
            manifest,
            vec![
                CollectionTransitionFamilyV1::new(
                    CollectionLogicalIdV1::new("collection/family").unwrap(),
                    TransitionFamilyBoundaryV1::Entry,
                    TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
                    members,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let collection =
            TransitionFamilyDeclarationInputV1::new(collection, b"collection").unwrap();
        assert_eq!(
            evaluate_document_transition_poses_v1(
                &collection,
                InputIdentity::from_bytes(b"document"),
                &Document::default(),
            ),
            Err(TransitionPoseEvaluationControlError::WrongDeclarationScope)
        );

        let mut oversized = document(None);
        oversized.skeleton.bones = (0..TRANSITION_POSE_EVALUATION_V1_MAX_BONES + 1)
            .map(|ordinal| Bone {
                name: format!("bone-{ordinal}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect();
        let limited = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &oversized,
        )
        .unwrap();
        assert_eq!(limited.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(
            limited.families()[0].reason(),
            Some(TransitionPoseReasonV1::InputLimit)
        );
        assert_eq!(limited.families()[0].skeleton_basis_input(), None);
    }

    #[test]
    fn selected_track_shape_gaps_are_incomplete_but_unselected_tracks_are_ignored() {
        let declaration = declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0);
        let mut variants = Vec::new();

        let mut empty = document(Some(1.0));
        empty.clips[1].tracks[0].times.clear();
        variants.push(empty);

        let mut nonfinite_time = document(Some(1.0));
        nonfinite_time.clips[1].tracks[0].times[0] = f32::NAN;
        variants.push(nonfinite_time);

        let mut non_increasing = document(Some(1.0));
        non_increasing.clips[1].tracks[0].times[1] = 0.0;
        variants.push(non_increasing);

        let mut wrong_cardinality = document(Some(1.0));
        wrong_cardinality.clips[1].tracks[0].values = TrackValues::Vec3s(Vec::new());
        variants.push(wrong_cardinality);

        let mut wrong_type = document(Some(1.0));
        wrong_type.clips[1].tracks[0].values = TrackValues::Quats(vec![
            crate::glam::Quat::IDENTITY,
            crate::glam::Quat::IDENTITY,
        ]);
        variants.push(wrong_type);

        let mut nonfinite_value = document(Some(1.0));
        nonfinite_value.clips[1].tracks[0].values =
            TrackValues::Vec3s(vec![crate::glam::Vec3::NAN, crate::glam::Vec3::ZERO]);
        variants.push(nonfinite_value);

        let mut duplicate = document(Some(1.0));
        let duplicate_track = duplicate.clips[1].tracks[0].clone();
        duplicate.clips[1].tracks.push(duplicate_track);
        variants.push(duplicate);

        let mut out_of_range = document(Some(1.0));
        out_of_range.clips[1].tracks[0].bone = 1;
        variants.push(out_of_range);

        for document in variants {
            let result = evaluate_document_transition_poses_v1(
                &declaration,
                InputIdentity::from_bytes(b"document"),
                &document,
            )
            .expect("selected track gaps are result data, not control errors");
            assert_eq!(result.status(), TransitionPoseStatusV1::Incomplete);
            assert_eq!(
                result.families()[0].reason,
                Some(TransitionPoseReasonV1::UnsupportedSampling)
            );
        }

        let mut unrelated = document(None);
        unrelated.clips.push(Clip {
            name: "Unselected".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 99,
                property: Property::Translation,
                interpolation: Interpolation::Linear,
                times: Vec::new(),
                values: TrackValues::Vec3s(Vec::new()),
            }],
        });
        let result = evaluate_document_transition_poses_v1(
            &declaration,
            InputIdentity::from_bytes(b"document"),
            &unrelated,
        )
        .unwrap();
        assert_eq!(result.status(), TransitionPoseStatusV1::Complete);
    }

    #[test]
    fn endpoint_sampling_does_not_consume_the_unconfigured_boundary() {
        let mut exit_only_gap = document(Some(0.0));
        exit_only_gap.clips[1].tracks.push(Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                crate::glam::Quat::IDENTITY,
                crate::glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
            ]),
        });
        let entry = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &exit_only_gap,
        )
        .unwrap();
        assert_eq!(entry.status(), TransitionPoseStatusV1::Complete);
        let exit = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Exit, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &exit_only_gap,
        )
        .unwrap();
        assert_eq!(exit.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(
            exit.families()[0].reason,
            Some(TransitionPoseReasonV1::UnsupportedSampling)
        );

        let mut entry_only_gap = document(Some(0.0));
        entry_only_gap.clips[1].tracks.push(Track {
            bone: 0,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![
                crate::glam::Quat::from_xyzw(0.0, 0.0, 0.0, 0.0),
                crate::glam::Quat::IDENTITY,
            ]),
        });
        let entry = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &entry_only_gap,
        )
        .unwrap();
        assert_eq!(entry.status(), TransitionPoseStatusV1::Incomplete);
        let exit = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Exit, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &entry_only_gap,
        )
        .unwrap();
        assert_eq!(exit.status(), TransitionPoseStatusV1::Complete);

        let mut nonrepresentable_duration = document(None);
        nonrepresentable_duration.clips[1].duration_s = 0.1;
        let entry = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &nonrepresentable_duration,
        )
        .unwrap();
        assert_eq!(entry.status(), TransitionPoseStatusV1::Complete);
        let exit = evaluate_document_transition_poses_v1(
            &declaration(TransitionFamilyBoundaryV1::Exit, 0.0, 0.0),
            InputIdentity::from_bytes(b"document"),
            &nonrepresentable_duration,
        )
        .unwrap();
        assert_eq!(exit.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(
            exit.families()[0].reason,
            Some(TransitionPoseReasonV1::UnsupportedSampling)
        );
    }

    #[test]
    fn multi_bone_pairs_and_independent_offender_caps_are_canonical() {
        let bones = (0..17)
            .map(|ordinal| Bone {
                name: format!("bone-{ordinal:02}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect::<Vec<_>>();
        let animated = |name: &str, translation_multiplier: f32| Clip {
            name: name.into(),
            duration_s: 1.0,
            tracks: (0..17)
                .flat_map(|ordinal| {
                    [
                        Track {
                            bone: ordinal,
                            property: Property::Translation,
                            interpolation: Interpolation::Linear,
                            times: vec![0.0, 1.0],
                            values: TrackValues::Vec3s(vec![
                                crate::glam::Vec3::X
                                    * ((ordinal + 1) as f32
                                        * translation_multiplier);
                                2
                            ]),
                        },
                        Track {
                            bone: ordinal,
                            property: Property::Rotation,
                            interpolation: Interpolation::Linear,
                            times: vec![0.0, 1.0],
                            values: TrackValues::Quats(vec![
                                crate::glam::Quat::from_rotation_x(
                                    std::f32::consts::FRAC_PI_2
                                );
                                2
                            ]),
                        },
                    ]
                })
                .collect(),
        };
        let document = Document {
            skeleton: Skeleton { bones },
            clips: vec![
                Clip {
                    name: "A".into(),
                    duration_s: 1.0,
                    tracks: Vec::new(),
                },
                animated("B", 1.0),
                animated("C", 2.0),
            ],
            ..Document::default()
        };
        let family = DocumentTransitionFamilyV1::new(
            "three_way".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            ["A", "B", "C"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| {
                    DocumentTransitionFamilyMemberV1::new(index as u64, name.into())
                })
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
        )
        .unwrap();
        let declaration = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
            b"declaration",
        )
        .unwrap();
        let result = evaluate_document_transition_poses_v1(
            &declaration,
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        let pairs = result.families()[0].pairs();
        assert_eq!(
            pairs
                .iter()
                .map(TransitionPosePairEvaluationV1::member_indices)
                .collect::<Vec<_>>(),
            vec![[0, 1], [0, 2], [1, 2]]
        );
        let first = &pairs[0];
        assert_eq!(first.translation_offenders().len(), 16);
        assert_eq!(first.rotation_offenders().len(), 16);
        assert_eq!(
            first
                .translation_offenders()
                .iter()
                .map(TransitionPoseTranslationOffenderV1::bone_ordinal)
                .collect::<Vec<_>>(),
            (1..17).rev().collect::<Vec<_>>()
        );
        assert_eq!(
            first
                .rotation_offenders()
                .iter()
                .map(TransitionPoseRotationOffenderV1::bone_ordinal)
                .collect::<Vec<_>>(),
            (0..16).collect::<Vec<_>>()
        );
        assert!(
            first
                .translation_offenders()
                .iter()
                .any(|offender| offender.bone_ordinal() == 1)
        );
        assert!(
            first
                .rotation_offenders()
                .iter()
                .any(|offender| offender.bone_ordinal() == 1)
        );
    }

    #[test]
    fn offender_selection_never_retains_more_than_its_final_cap() {
        let mut descending = Vec::new();
        for ordinal in 0..TRANSITION_POSE_EVALUATION_V1_MAX_BONES {
            retain_top_candidate(
                &mut descending,
                ordinal,
                ordinal as f64,
                TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS,
            );
            assert!(descending.len() <= TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS);
        }
        sort_top_candidates(&mut descending);
        assert_eq!(
            descending
                .iter()
                .map(|(ordinal, _)| *ordinal)
                .collect::<Vec<_>>(),
            (TRANSITION_POSE_EVALUATION_V1_MAX_BONES
                - TRANSITION_POSE_EVALUATION_V1_MAX_TRANSLATION_OFFENDERS
                ..TRANSITION_POSE_EVALUATION_V1_MAX_BONES)
                .rev()
                .collect::<Vec<_>>()
        );

        let mut tied = Vec::new();
        for ordinal in (0..TRANSITION_POSE_EVALUATION_V1_MAX_BONES).rev() {
            retain_top_candidate(
                &mut tied,
                ordinal,
                1.0,
                TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS,
            );
            assert!(tied.len() <= TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS);
        }
        sort_top_candidates(&mut tied);
        assert_eq!(
            tied.iter().map(|(ordinal, _)| *ordinal).collect::<Vec<_>>(),
            (0..TRANSITION_POSE_EVALUATION_V1_MAX_ROTATION_OFFENDERS).collect::<Vec<_>>()
        );
    }

    fn numbered_members(count: u64) -> Vec<DocumentTransitionFamilyMemberV1> {
        (0..count)
            .map(|index| DocumentTransitionFamilyMemberV1::new(index, format!("clip-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    fn numbered_family(
        family_id: String,
        count: u64,
        boundary: TransitionFamilyBoundaryV1,
    ) -> DocumentTransitionFamilyV1 {
        DocumentTransitionFamilyV1::new(
            family_id,
            boundary,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            numbered_members(count),
        )
        .unwrap()
    }

    fn numbered_document(clips: u64, bones: usize) -> Document {
        Document {
            skeleton: Skeleton {
                bones: (0..bones)
                    .map(|ordinal| Bone {
                        name: format!("bone-{ordinal}"),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    })
                    .collect(),
            },
            clips: (0..clips)
                .map(|index| Clip {
                    name: format!("clip-{index}"),
                    duration_s: 1.0,
                    tracks: Vec::new(),
                })
                .collect(),
            ..Document::default()
        }
    }

    #[test]
    fn member_witnesses_are_control_preflight_before_all_unavailability_reasons() {
        let input = |families| {
            TransitionFamilyDeclarationInputV1::new(
                TransitionFamilyDeclarationV1::document(families).unwrap(),
                b"declaration",
            )
            .unwrap()
        };
        let assert_invalid_witness = |input: &TransitionFamilyDeclarationInputV1,
                                      document: &Document| {
            assert_eq!(
                evaluate_document_transition_poses_v1(
                    input,
                    InputIdentity::from_bytes(b"document"),
                    document,
                ),
                Err(TransitionPoseEvaluationControlError::InvalidMemberWitness)
            );
        };

        let time_family = DocumentTransitionFamilyV1::new(
            "time".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.1).unwrap(),
            vec![
                DocumentTransitionFamilyMemberV1::new(2, "missing-a".into()).unwrap(),
                DocumentTransitionFamilyMemberV1::new(3, "missing-b".into()).unwrap(),
            ],
        )
        .unwrap();
        assert_invalid_witness(&input(vec![time_family]), &document(None));

        let family_work =
            numbered_family("family_work".into(), 65, TransitionFamilyBoundaryV1::Both);
        assert_eq!(
            plan_families(std::slice::from_ref(&family_work), 1, &[false])[0].reason,
            Some(TransitionPoseReasonV1::FamilyWorkLimit)
        );
        assert_invalid_witness(&input(vec![family_work]), &Document::default());

        let aggregate = (0..33)
            .map(|ordinal| {
                numbered_family(
                    format!("aggregate-{ordinal}"),
                    65,
                    TransitionFamilyBoundaryV1::Entry,
                )
            })
            .collect::<Vec<_>>();
        assert!(
            plan_families(&aggregate, 0, &vec![false; aggregate.len()])
                .iter()
                .any(|plan| plan.reason == Some(TransitionPoseReasonV1::AggregateWorkLimit))
        );
        assert_invalid_witness(&input(aggregate), &Document::default());

        let retention = vec![
            numbered_family("retention-a".into(), 64, TransitionFamilyBoundaryV1::Entry),
            numbered_family("retention-b".into(), 64, TransitionFamilyBoundaryV1::Entry),
        ];
        assert_eq!(
            plan_families(&retention, 16, &[false, false])[1].reason,
            Some(TransitionPoseReasonV1::RetentionLimit)
        );
        assert_invalid_witness(&input(retention), &numbered_document(0, 16));

        let earlier_time = DocumentTransitionFamilyV1::new(
            "a-time".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.1).unwrap(),
            vec![
                DocumentTransitionFamilyMemberV1::new(0, "Walk".into()).unwrap(),
                DocumentTransitionFamilyMemberV1::new(1, "Run".into()).unwrap(),
            ],
        )
        .unwrap();
        let later_invalid = DocumentTransitionFamilyV1::new(
            "z-invalid".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            vec![
                DocumentTransitionFamilyMemberV1::new(2, "missing-a".into()).unwrap(),
                DocumentTransitionFamilyMemberV1::new(3, "missing-b".into()).unwrap(),
            ],
        )
        .unwrap();
        assert_invalid_witness(&input(vec![earlier_time, later_invalid]), &document(None));
    }

    #[test]
    fn pair_work_math_is_checked_before_sampling() {
        let family = numbered_family(
            "too_many_pairs".into(),
            65,
            TransitionFamilyBoundaryV1::Both,
        );
        let input = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
            b"declaration",
        )
        .unwrap();
        let result = evaluate_document_transition_poses_v1(
            &input,
            InputIdentity::from_bytes(b"document"),
            &numbered_document(65, 1),
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
    fn retention_preflight_uses_the_actual_bone_bounded_offender_capacity() {
        let members = (0..64)
            .map(|index| DocumentTransitionFamilyMemberV1::new(index, format!("clip-{index}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let family = DocumentTransitionFamilyV1::new(
            "near_retention_limit".into(),
            TransitionFamilyBoundaryV1::Entry,
            TransitionFamilyTolerancesV1::new(0.0, 0.0, 0.0).unwrap(),
            members,
        )
        .unwrap();

        // 16 * C(64, 2) * (1 translation + 1 rotation) = 64,512, so a
        // one-bone document remains below the aggregate 65,536 record cap.
        let low_bone_families = vec![family.clone(); 16];
        assert!(
            plan_families(&low_bone_families, 1, &vec![false; low_bone_families.len()])
                .iter()
                .all(|plan| plan.reason.is_none())
        );

        // With at least 16 bones each pair/boundary can retain 16 records per
        // channel. The second family crosses the same aggregate cap.
        let high_bone_plans = plan_families(&[family.clone(), family], 16, &[false, false]);
        assert_eq!(high_bone_plans[0].reason, None);
        assert_eq!(
            high_bone_plans[1].reason,
            Some(TransitionPoseReasonV1::RetentionLimit)
        );
    }

    #[test]
    fn long_bone_names_hit_result_limit_before_offender_name_cloning() {
        let family = numbered_family("long_names".into(), 64, TransitionFamilyBoundaryV1::Entry);
        let input = TransitionFamilyDeclarationInputV1::new(
            TransitionFamilyDeclarationV1::document(vec![family]).unwrap(),
            b"declaration",
        )
        .unwrap();
        let mut document = numbered_document(64, 1);
        document.skeleton.bones[0].name = "x".repeat(200_000);
        let result = evaluate_document_transition_poses_v1(
            &input,
            InputIdentity::from_bytes(b"document"),
            &document,
        )
        .unwrap();
        assert_eq!(
            result.families()[0].reason(),
            Some(TransitionPoseReasonV1::ResultLimit)
        );
        assert!(result.families()[0].pairs().is_empty());
    }

    #[test]
    fn detailed_reservation_subtracts_pair_free_authority_before_name_rows() {
        let declared = declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0);
        let families = declared.declaration().document_families().unwrap();
        let mut document = document(Some(1.0));
        document.skeleton.bones[0].name = "n".repeat(128);
        let subject = InputIdentity::from_bytes(b"document");
        let closure = complete_closure(subject.clone());
        let result =
            evaluate_document_transition_poses_v1(&declared, subject.clone(), &document).unwrap();
        let basis = SkeletonBasisV1::from_skeleton(&document.skeleton).unwrap();
        let remaining = detailed_result_budget_after_base(
            &result,
            families,
            &closure,
            basis.identity(),
            TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES,
        )
        .unwrap();
        let base_bytes = TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES - remaining;
        let mut reservation = remaining;
        assert!(reserve_detailed_name_budget(
            &mut reservation,
            &families[0],
            &document.skeleton,
        ));
        let detailed_bytes = remaining - reservation;
        let cap = base_bytes + detailed_bytes - 1;
        let result = evaluate_document_transition_poses_v1_with_result_limit(
            &declared, &closure, &document, cap,
        )
        .unwrap();
        assert_eq!(
            result.families()[0].reason(),
            Some(TransitionPoseReasonV1::ResultLimit)
        );
        assert!(result.families()[0].pairs().is_empty());
    }

    #[test]
    fn result_limit_retry_retains_bindings_and_terminates_at_a_tiny_seam() {
        let declaration = declaration(TransitionFamilyBoundaryV1::Entry, 0.0, 0.0);
        let mut result = evaluate_document_transition_poses_v1(
            &declaration,
            InputIdentity::from_bytes(b"document"),
            &document(Some(1.0)),
        )
        .unwrap();
        let before_members = result.families()[0].members().to_vec();
        let before_basis = result.families()[0].skeleton_basis_input().cloned();
        let before_declaration = result.declaration_input().clone();
        let before_normalized = result.declaration_normalized().clone();
        let before_subject = result.subject_input().clone();
        let before_subject_closure = result.subject_dependency_closure_identity().cloned();
        let before_member_closures = result.families()[0]
            .members()
            .iter()
            .map(|member| member.source_dependency_closure_identity().cloned())
            .collect::<Vec<_>>();
        let detailed_bytes =
            canonical_bytes(&result, TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES)
                .unwrap()
                .len();

        let mut degraded = result.clone();
        for family in &mut degraded.families {
            family.status = TransitionPoseStatusV1::Incomplete;
            family.decision = TransitionPoseDecisionV1::NotEvaluated;
            family.reason = Some(TransitionPoseReasonV1::ResultLimit);
            family.pairs.clear();
        }
        derive_result_state(&mut degraded);
        let degraded_bytes =
            canonical_bytes(&degraded, TRANSITION_POSE_EVALUATION_V1_MAX_RESULT_BYTES)
                .unwrap()
                .len();
        assert!(degraded_bytes < detailed_bytes);

        enforce_result_limit(&mut result, degraded_bytes).unwrap();
        assert_eq!(result.status(), TransitionPoseStatusV1::Incomplete);
        assert_eq!(result.decision(), TransitionPoseDecisionV1::NotEvaluated);
        assert_eq!(result.declaration_input(), &before_declaration);
        assert_eq!(result.declaration_normalized(), &before_normalized);
        assert_eq!(result.subject_input(), &before_subject);
        assert_eq!(
            result.subject_dependency_closure_identity(),
            before_subject_closure.as_ref()
        );
        assert_eq!(result.families()[0].members(), before_members);
        assert_eq!(
            result.families()[0]
                .members()
                .iter()
                .map(|member| member.source_dependency_closure_identity().cloned())
                .collect::<Vec<_>>(),
            before_member_closures
        );
        assert_eq!(
            result.families()[0].skeleton_basis_input(),
            before_basis.as_ref()
        );
        assert_eq!(result.families()[0].pairs(), &[]);
        assert_eq!(
            result.families()[0].reason(),
            Some(TransitionPoseReasonV1::ResultLimit)
        );
        assert!(canonical_bytes(&result, degraded_bytes).is_ok());
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
