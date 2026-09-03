//! Strict collection foot-cycle parameterization and pure V1 map planning.
//!
//! The CLI owns bounded TOML decoding and eventual rooted filesystem access.
//! This module owns the format-neutral declaration, exact collection/contact
//! bindings, alternating stance topology, and deterministic source-to-output
//! piecewise-linear plan. It does not mutate animation tracks, transform
//! contact fragments, prove serialized output, or publish files.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::{
    COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES, CollectionIdV1, CollectionLogicalIdV1,
    CollectionManifestV1, CollectionRuntimeSetKindV1, ContactClipReferenceV1, ContactEventKindV1,
    ContactFragmentV1, ContactPhaseV1, ContactRoleV1, ContactTimeWarpControlPointV1,
    ContactTransformBindingV1, ContactTransformOperationV1, DependencyClosureIdentityV1,
    DependencyResourceKeyV1, InputIdentity,
};

/// Immutable schema identity for the foot-cycle parameterization declaration.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_ID: &str =
    "urn:animsmith:schema:foot-cycle-parameterization:1";
/// Immutable schema version for the foot-cycle parameterization declaration.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_SCHEMA_VERSION: u32 = 1;
/// Maximum accepted parameterization TOML bytes.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum declared members and supplied evidence rows.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS: usize = 4_096;
/// Maximum aggregate contact events inspected across all supplied members.
///
/// This admits the minimum bilateral window/marker evidence for every member
/// at the member cap while preventing per-fragment limits from multiplying.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS: usize =
    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS * 4;
/// Maximum aggregate canonical contact-fragment bytes inspected by one plan.
///
/// Exact supplied byte identities are preflighted before canonicalization. A
/// lying identity can therefore cost at most one fragment's existing 8 MiB
/// canonical-output bound before exact identity comparison refuses it.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum source-to-output control points, shared with contact-transform V1.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS: usize =
    crate::CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS;
/// Largest accepted finite segment-slope bound.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_SLOPE: f64 = 1_000_000.0;
/// Inclusive V1 in-place horizontal endpoint-displacement limit in metres.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M: f64 = 0.01;
/// Inclusive V1 in-place accumulated-yaw limit in degrees.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG: f64 = 1.0;
/// Known stance-support detector extension emitted by AnimSmith.
pub const CONTACT_SUPPORT_DETECTOR_V1_ID: &str = "urn:animsmith:contact-support-detector:1";

const CONTACT_SUPPORT_DETECTOR_V1_SCHEMA_VERSION: u32 = 1;
const CONTACT_SUPPORT_DETECTOR_V1_ALGORITHM: &str = "stance-support-v1";
const CONTACT_SUPPORT_DETECTOR_V1_SAMPLING: &str = "metric-grid-longest-authored-channel";
const CONTACT_SUPPORT_DETECTOR_V1_MAX_FRAMES: u64 = 1_000_000;

/// Exact collection-manifest identity bound by one parameterization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FootCycleManifestBindingV1 {
    collection_id: CollectionIdV1,
    input: InputIdentity,
}

impl FootCycleManifestBindingV1 {
    /// Construct an exact bounded manifest binding.
    ///
    /// # Errors
    ///
    /// Returns [`FootCycleParameterizationError::ManifestTooLarge`] when the
    /// byte identity exceeds collection-manifest V1's reader limit.
    pub fn new(
        collection_id: CollectionIdV1,
        input: InputIdentity,
    ) -> Result<Self, FootCycleParameterizationError> {
        if input.bytes() > COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES {
            return Err(FootCycleParameterizationError::ManifestTooLarge);
        }
        Ok(Self {
            collection_id,
            input,
        })
    }

    /// Collection namespace token.
    pub fn collection_id(&self) -> &CollectionIdV1 {
        &self.collection_id
    }

    /// Identity of the exact manifest bytes consumed by the parser.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }
}

/// One member and its safe parameterization-local contact-fragment locator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FootCycleParameterizationMemberV1 {
    id: CollectionLogicalIdV1,
    contact_fragment: DependencyResourceKeyV1,
}

/// Exact proof thresholds declared by one foot-cycle parameterization.
///
/// These values are part of the parameterization document's exact byte
/// identity. V1 has no defaults and does not merge proof policy from another
/// source.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FootCycleProofPolicyV1 {
    max_gait_phase_spread: f64,
    min_lr_amplitude_m: f64,
    max_contact_boundary_phase_error: f64,
}

impl FootCycleProofPolicyV1 {
    /// Construct one complete finite V1 proof policy.
    ///
    /// # Errors
    ///
    /// Returns [`FootCycleParameterizationError::InvalidProofPolicy`] unless
    /// phase tolerances are within `[0, 0.5]` and amplitude is non-negative.
    pub fn new(
        max_gait_phase_spread: f64,
        min_lr_amplitude_m: f64,
        max_contact_boundary_phase_error: f64,
    ) -> Result<Self, FootCycleParameterizationError> {
        if !max_gait_phase_spread.is_finite()
            || !(0.0..=0.5).contains(&max_gait_phase_spread)
            || !min_lr_amplitude_m.is_finite()
            || min_lr_amplitude_m < 0.0
            || !max_contact_boundary_phase_error.is_finite()
            || !(0.0..=0.5).contains(&max_contact_boundary_phase_error)
        {
            return Err(FootCycleParameterizationError::InvalidProofPolicy);
        }
        Ok(Self {
            max_gait_phase_spread: canonical_zero(max_gait_phase_spread),
            min_lr_amplitude_m: canonical_zero(min_lr_amplitude_m),
            max_contact_boundary_phase_error: canonical_zero(max_contact_boundary_phase_error),
        })
    }

    /// Inclusive maximum circular gait-phase spread.
    pub const fn max_gait_phase_spread(&self) -> f64 {
        self.max_gait_phase_spread
    }

    /// Inclusive minimum left/right gait amplitude in metres.
    pub const fn min_lr_amplitude_m(&self) -> f64 {
        self.min_lr_amplitude_m
    }

    /// Inclusive maximum circular contact-boundary phase error.
    pub const fn max_contact_boundary_phase_error(&self) -> f64 {
        self.max_contact_boundary_phase_error
    }
}

impl FootCycleParameterizationMemberV1 {
    /// Construct one explicit member declaration.
    pub fn new(id: CollectionLogicalIdV1, contact_fragment: DependencyResourceKeyV1) -> Self {
        Self {
            id,
            contact_fragment,
        }
    }

    /// Logical collection clip id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Safe locator relative to the parameterization document.
    pub fn contact_fragment(&self) -> &DependencyResourceKeyV1 {
        &self.contact_fragment
    }
}

/// Fully validated foot-cycle parameterization V1 declaration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FootCycleParameterizationV1 {
    schema: &'static str,
    schema_version: u32,
    manifest: FootCycleManifestBindingV1,
    runtime_set_id: CollectionLogicalIdV1,
    reference_member: CollectionLogicalIdV1,
    output_directory: DependencyResourceKeyV1,
    minimum_segment_slope: f64,
    maximum_segment_slope: f64,
    proof: FootCycleProofPolicyV1,
    members: Vec<FootCycleParameterizationMemberV1>,
}

impl FootCycleParameterizationV1 {
    /// Construct one strict declaration in authored member order.
    ///
    /// # Errors
    ///
    /// Returns [`FootCycleParameterizationError`] when membership, paths,
    /// proof policy, or slope bounds violate the frozen V1 declaration
    /// contract.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        manifest: FootCycleManifestBindingV1,
        runtime_set_id: CollectionLogicalIdV1,
        reference_member: CollectionLogicalIdV1,
        output_directory: DependencyResourceKeyV1,
        minimum_segment_slope: f64,
        maximum_segment_slope: f64,
        proof: FootCycleProofPolicyV1,
        members: Vec<FootCycleParameterizationMemberV1>,
    ) -> Result<Self, FootCycleParameterizationError> {
        if members.len() < 2 {
            return Err(FootCycleParameterizationError::TooFewMembers {
                found: members.len(),
            });
        }
        if members.len() > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS {
            return Err(FootCycleParameterizationError::TooManyMembers {
                found: members.len(),
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS,
            });
        }
        if !minimum_segment_slope.is_finite()
            || !maximum_segment_slope.is_finite()
            || minimum_segment_slope <= 0.0
            || minimum_segment_slope > 1.0
            || maximum_segment_slope < 1.0
            || minimum_segment_slope > maximum_segment_slope
            || maximum_segment_slope > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_SLOPE
        {
            return Err(FootCycleParameterizationError::InvalidSlopeBounds);
        }
        let mut ids = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut has_reference = false;
        for member in &members {
            if !ids.insert(member.id.clone()) {
                return Err(FootCycleParameterizationError::DuplicateMember {
                    member: member.id.as_str().to_owned(),
                });
            }
            if !paths.insert(member.contact_fragment.clone()) {
                return Err(FootCycleParameterizationError::DuplicateFragmentPath);
            }
            if member.contact_fragment == output_directory {
                return Err(FootCycleParameterizationError::OutputPathCollision);
            }
            has_reference |= member.id == reference_member;
        }
        if !has_reference {
            return Err(FootCycleParameterizationError::MissingReferenceMember);
        }
        Ok(Self {
            schema: FOOT_CYCLE_PARAMETERIZATION_V1_ID,
            schema_version: FOOT_CYCLE_PARAMETERIZATION_V1_SCHEMA_VERSION,
            manifest,
            runtime_set_id,
            reference_member,
            output_directory,
            minimum_segment_slope: canonical_zero(minimum_segment_slope),
            maximum_segment_slope: canonical_zero(maximum_segment_slope),
            proof,
            members,
        })
    }

    /// Immutable schema identity.
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Immutable schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Exact manifest binding.
    pub fn manifest(&self) -> &FootCycleManifestBindingV1 {
        &self.manifest
    }

    /// Declared gait-group runtime set.
    pub fn runtime_set_id(&self) -> &CollectionLogicalIdV1 {
        &self.runtime_set_id
    }

    /// Member whose contact boundaries own canonical output phases.
    pub fn reference_member(&self) -> &CollectionLogicalIdV1 {
        &self.reference_member
    }

    /// Safe future generation-directory locator.
    pub fn output_directory(&self) -> &DependencyResourceKeyV1 {
        &self.output_directory
    }

    /// Inclusive minimum accepted segment slope.
    pub const fn minimum_segment_slope(&self) -> f64 {
        self.minimum_segment_slope
    }

    /// Inclusive maximum accepted segment slope.
    pub const fn maximum_segment_slope(&self) -> f64 {
        self.maximum_segment_slope
    }

    /// Exact required output-proof policy bound by this declaration.
    pub const fn proof(&self) -> &FootCycleProofPolicyV1 {
        &self.proof
    }

    /// Members in exact declaration and runtime-set order.
    pub fn members(&self) -> &[FootCycleParameterizationMemberV1] {
        &self.members
    }
}

/// Exact source witness for independently measured Root/Hips facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FootCycleRootMotionBindingV1 {
    artifact: InputIdentity,
    dependency_closure_identity: DependencyClosureIdentityV1,
    clip: ContactClipReferenceV1,
}

impl FootCycleRootMotionBindingV1 {
    /// Bind a measurement to one exact loaded source and selected clip.
    pub fn new(
        artifact: InputIdentity,
        dependency_closure_identity: DependencyClosureIdentityV1,
        clip: ContactClipReferenceV1,
    ) -> Self {
        Self {
            artifact,
            dependency_closure_identity,
            clip,
        }
    }

    /// Exact primary source artifact identity.
    pub const fn artifact(&self) -> &InputIdentity {
        &self.artifact
    }

    /// Exact dependency closure from the same load.
    pub const fn dependency_closure_identity(&self) -> &DependencyClosureIdentityV1 {
        &self.dependency_closure_identity
    }

    /// Exact selected collection source/take witness.
    pub const fn clip(&self) -> &ContactClipReferenceV1 {
        &self.clip
    }
}

/// Independently measured in-place evidence for one exact source clip.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FootCycleRootMotionEvidenceV1 {
    /// Complete finite source measurements.
    Measured {
        /// Exact source and selected-clip witness measured.
        binding: FootCycleRootMotionBindingV1,
        /// Signed Root/Hips endpoint displacement on the X axis in metres.
        endpoint_displacement_x_m: f64,
        /// Signed Root/Hips endpoint displacement on the Z axis in metres.
        endpoint_displacement_z_m: f64,
        /// Signed accumulated Root/Hips yaw in degrees.
        accumulated_yaw_deg: f64,
    },
    /// Required Root/Hips evidence was absent.
    Missing {
        /// Exact source and selected-clip witness inspected.
        binding: FootCycleRootMotionBindingV1,
    },
    /// More than one source authority could own the measurement.
    Ambiguous {
        /// Exact source and selected-clip witness inspected.
        binding: FootCycleRootMotionBindingV1,
    },
    /// Source sampling encountered a non-finite value.
    NonFinite {
        /// Exact source and selected-clip witness inspected.
        binding: FootCycleRootMotionBindingV1,
    },
}

impl FootCycleRootMotionEvidenceV1 {
    /// Construct complete measured evidence.
    pub fn measured(
        binding: FootCycleRootMotionBindingV1,
        endpoint_displacement_x_m: f64,
        endpoint_displacement_z_m: f64,
        accumulated_yaw_deg: f64,
    ) -> Self {
        Self::Measured {
            binding,
            endpoint_displacement_x_m,
            endpoint_displacement_z_m,
            accumulated_yaw_deg,
        }
    }

    /// Construct evidence that the exact source lacked Root/Hips authority.
    pub fn missing(binding: FootCycleRootMotionBindingV1) -> Self {
        Self::Missing { binding }
    }

    /// Construct evidence that the exact source had ambiguous Root/Hips authority.
    pub fn ambiguous(binding: FootCycleRootMotionBindingV1) -> Self {
        Self::Ambiguous { binding }
    }

    /// Construct evidence that the exact source produced non-finite samples.
    pub fn non_finite(binding: FootCycleRootMotionBindingV1) -> Self {
        Self::NonFinite { binding }
    }

    /// Exact source and selected-clip witness inspected.
    pub const fn binding(&self) -> &FootCycleRootMotionBindingV1 {
        match self {
            Self::Measured { binding, .. }
            | Self::Missing { binding }
            | Self::Ambiguous { binding }
            | Self::NonFinite { binding } => binding,
        }
    }
}

/// One already-read contact fragment and root witness bound to exact source bytes and path.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCycleMemberEvidenceV1 {
    id: CollectionLogicalIdV1,
    contact_fragment_path: DependencyResourceKeyV1,
    input: InputIdentity,
    fragment: ContactFragmentV1,
    root_motion: FootCycleRootMotionEvidenceV1,
}

impl FootCycleMemberEvidenceV1 {
    /// Bind one decoded fragment to the exact bytes and declaration path used.
    pub fn new(
        id: CollectionLogicalIdV1,
        contact_fragment_path: DependencyResourceKeyV1,
        input: InputIdentity,
        fragment: ContactFragmentV1,
        root_motion: FootCycleRootMotionEvidenceV1,
    ) -> Self {
        Self {
            id,
            contact_fragment_path,
            input,
            fragment,
            root_motion,
        }
    }

    /// Logical member id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Exact bytes read from the declared fragment path.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Strict decoded fragment.
    pub const fn fragment(&self) -> &ContactFragmentV1 {
        &self.fragment
    }

    /// Independently measured Root/Hips in-place evidence.
    pub const fn root_motion(&self) -> &FootCycleRootMotionEvidenceV1 {
        &self.root_motion
    }
}

/// One member's exact contact binding and future time-warp operation.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCycleMemberPlanV1 {
    id: CollectionLogicalIdV1,
    input: ContactTransformBindingV1,
    root_motion: FootCycleRootMotionEvidenceV1,
    operation: ContactTransformOperationV1,
}

impl FootCycleMemberPlanV1 {
    /// Logical member id.
    pub fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    /// Exact contact-transform input binding.
    pub const fn input(&self) -> &ContactTransformBindingV1 {
        &self.input
    }

    /// Accepted in-place root evidence consumed by this plan.
    pub const fn root_motion(&self) -> &FootCycleRootMotionEvidenceV1 {
        &self.root_motion
    }

    /// Validated time-warp operation preserving the source duration.
    pub const fn operation(&self) -> &ContactTransformOperationV1 {
        &self.operation
    }
}

#[cfg(test)]
pub(crate) fn clip_test_member_plan(
    operation: ContactTransformOperationV1,
) -> FootCycleMemberPlanV1 {
    let artifact = InputIdentity::from_bytes(b"foot-cycle-clip-test");
    let closure = crate::DependencyClosureBuilderV1::new(
        artifact.clone(),
        crate::SourceSetCoverageV1::complete(),
        0,
    )
    .finish()
    .expect("test dependency closure");
    let closure_identity = closure.identity().expect("complete closure").clone();
    let clip =
        ContactClipReferenceV1::collection("com.example/test", "sources/test.glb", 0, "test")
            .expect("test clip reference");
    let root_motion = FootCycleRootMotionEvidenceV1::measured(
        FootCycleRootMotionBindingV1::new(artifact.clone(), closure_identity.clone(), clip),
        0.0,
        0.0,
        0.0,
    );
    FootCycleMemberPlanV1 {
        id: CollectionLogicalIdV1::new("com.example/test").expect("test logical id"),
        input: ContactTransformBindingV1::new(
            artifact,
            closure_identity,
            InputIdentity::from_bytes(b"test-contact-fragment"),
        ),
        root_motion,
        operation,
    }
}

/// Complete pure plan for one declared collection ring.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCyclePlanV1 {
    parameterization_input: InputIdentity,
    manifest_input: InputIdentity,
    runtime_set_id: CollectionLogicalIdV1,
    reference_member: CollectionLogicalIdV1,
    proof: FootCycleProofPolicyV1,
    members: Vec<FootCycleMemberPlanV1>,
}

impl FootCyclePlanV1 {
    /// Identity of the exact parameterization bytes consumed by the parser.
    pub const fn parameterization_input(&self) -> &InputIdentity {
        &self.parameterization_input
    }

    /// Identity of the exact bound manifest bytes.
    pub const fn manifest_input(&self) -> &InputIdentity {
        &self.manifest_input
    }

    /// Planned runtime-set id.
    pub fn runtime_set_id(&self) -> &CollectionLogicalIdV1 {
        &self.runtime_set_id
    }

    /// Reference member that owns canonical boundary phases.
    pub fn reference_member(&self) -> &CollectionLogicalIdV1 {
        &self.reference_member
    }

    /// Exact proof policy copied from the identity-bound declaration.
    pub const fn proof(&self) -> &FootCycleProofPolicyV1 {
        &self.proof
    }

    /// Member plans in exact declared runtime-set order.
    pub fn members(&self) -> &[FootCycleMemberPlanV1] {
        &self.members
    }
}

/// A declaration or evidence set could not produce one strict V1 map plan.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FootCycleParameterizationError {
    /// Manifest byte identity exceeded collection-manifest V1's bound.
    #[error("manifest identity exceeds the V1 byte limit")]
    ManifestTooLarge,
    /// Parameterization byte identity exceeded its V1 reader bound.
    #[error("parameterization identity exceeds the V1 byte limit")]
    ParameterizationTooLarge,
    /// Fewer than two members were declared.
    #[error("foot-cycle parameterization needs at least two members, found {found}")]
    TooFewMembers {
        /// Observed count.
        found: usize,
    },
    /// Member count exceeded V1.
    #[error("foot-cycle parameterization has {found} members, exceeding V1 limit {max}")]
    TooManyMembers {
        /// Observed count, including the N+1 witness.
        found: usize,
        /// Frozen maximum.
        max: usize,
    },
    /// Slope bounds were not finite, positive, ordered, bounded, and inclusive of identity.
    #[error("segment-slope bounds must be finite, positive, ordered, include 1, and be within V1")]
    InvalidSlopeBounds,
    /// Required proof thresholds were non-finite or outside their V1 ranges.
    #[error("proof policy values must be finite and within their V1 ranges")]
    InvalidProofPolicy,
    /// A logical member id appeared more than once.
    #[error("duplicate foot-cycle member {member:?}")]
    DuplicateMember {
        /// Repeated id.
        member: String,
    },
    /// Two members declared one lexical fragment path.
    #[error("contact-fragment paths must be unique")]
    DuplicateFragmentPath,
    /// The future output directory duplicated a consumed fragment path.
    #[error("output directory must differ from every contact-fragment path")]
    OutputPathCollision,
    /// The declared reference did not occur in the member list.
    #[error("reference_member must name one declared member")]
    MissingReferenceMember,
    /// The declaration's manifest identity was stale or for another collection.
    #[error("parameterization manifest binding does not match current manifest")]
    ManifestMismatch,
    /// The selected runtime set was absent or ambiguous.
    #[error("declared runtime set is not present exactly once")]
    RuntimeSetMismatch,
    /// V1 accepts only a gait-group set.
    #[error("foot-cycle parameterization runtime set must be gait-group")]
    WrongRuntimeSetKind,
    /// Declaration members did not exactly preserve runtime-set order.
    #[error("parameterization members must exactly preserve runtime-set membership and order")]
    MemberOrderMismatch,
    /// Evidence row count differed from declaration count.
    #[error("foot-cycle evidence row count does not match declared members")]
    EvidenceCountMismatch,
    /// Aggregate contact-event work exceeded V1 before topology retention.
    #[error("foot-cycle evidence has {found} contact events, exceeding V1 aggregate limit {max}")]
    TooManyContactEvents {
        /// Observed aggregate count, capped at the first excess witness.
        found: usize,
        /// Frozen maximum.
        max: usize,
    },
    /// Aggregate canonical-fragment bytes exceeded V1 before canonicalization.
    #[error(
        "foot-cycle evidence has {found} canonical contact-fragment bytes, exceeding V1 aggregate limit {max}"
    )]
    TooManyContactFragmentBytes {
        /// Observed aggregate bytes, capped at the first excess witness.
        found: u64,
        /// Frozen aggregate maximum.
        max: u64,
    },
    /// Evidence id or locator differed from the corresponding declaration row.
    #[error("foot-cycle evidence does not match declared member order and paths")]
    EvidenceMemberMismatch,
    /// Fragment source bytes were not exactly its canonical V1 bytes.
    #[error("contact fragment must be supplied as its exact canonical bytes")]
    NonCanonicalFragment,
    /// Fragment's collection witness differed from the manifest clip row.
    #[error("contact fragment collection clip witness does not match the manifest")]
    FragmentClipMismatch,
    /// Required detector extension was absent, repeated, or unsupported.
    #[error("contact fragment must carry exactly the supported stance detector extension")]
    UnsupportedContactExtension,
    /// Detector provenance payload was malformed or contradicted event roles.
    #[error("contact fragment stance detector provenance is invalid")]
    InvalidDetectorProvenance,
    /// Detector thresholds differed, so support boundaries were not comparable.
    #[error("contact fragments must use one exact stance-detector threshold across the ring")]
    DetectorPolicyMismatch,
    /// Root/Hips facts named a different artifact, closure, or selected clip.
    #[error("member {member:?} Root/Hips evidence does not match its exact source clip")]
    RootMotionBindingMismatch {
        /// Logical member whose source witness refused.
        member: String,
    },
    /// Root/Hips measurements were absent, ambiguous, malformed, or non-finite.
    #[error("member {member:?} does not have complete finite Root/Hips in-place evidence")]
    RootMotionEvidenceUnavailable {
        /// Logical member whose evidence refused.
        member: String,
    },
    /// Root/Hips motion exceeded the inclusive V1 in-place thresholds.
    #[error("member {member:?} exceeds V1 in-place root-motion thresholds")]
    RootMotionOutOfRange {
        /// Logical member whose evidence refused.
        member: String,
    },
    /// Bilateral support runs violated the exact V1 topology grammar.
    #[error("contact fragment has incomplete or non-alternating bilateral stance topology")]
    InvalidContactTopology,
    /// A member's cyclic boundary signature differed from the reference.
    #[error("contact boundary topology does not match the reference member")]
    TopologyMismatch,
    /// Corresponding boundaries cannot form an endpoint-preserving monotone map.
    #[error("corresponding contact boundaries do not form a strict monotone map")]
    NonMonotoneMapping,
    /// One segment's slope fell outside the inclusive declaration bounds.
    #[error("piecewise-linear segment slope is outside declared bounds")]
    SegmentSlopeOutOfRange,
    /// The control-point cap was exceeded before retaining N+1.
    #[error("time-warp plan needs {found} control points, exceeding V1 limit {max}")]
    TooManyControlPoints {
        /// Observed count, including N+1.
        found: usize,
        /// Frozen maximum.
        max: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Onset,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct SupportWindow {
    side: Side,
    start: f64,
    end: f64,
}

#[derive(Debug, Clone, Copy)]
struct LogicalSupportWindow {
    side: Side,
    onset: f64,
    release: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryKind {
    side: Side,
    edge: Edge,
}

#[derive(Debug, Clone, Copy)]
struct Boundary {
    kind: BoundaryKind,
    time: f64,
}

struct MemberTopology {
    evidence_index: usize,
    signature: Vec<BoundaryKind>,
    rotated_boundaries: Vec<Boundary>,
    contact_height_m_bits: u64,
}

/// Plan exact source-to-reference normalized maps for one collection ring.
///
/// The supplied manifest and both raw control identities are external facts;
/// no file is reopened. Evidence must be in the declaration's exact member and
/// path order, and each input identity must equal the fragment's canonical
/// bytes. The reference owns boundary phases without moving phase zero: a
/// correspondence that would require cyclic key rotation is therefore refused
/// as non-monotone.
///
/// # Errors
///
/// Returns [`FootCycleParameterizationError`] for stale bindings, unsupported
/// evidence, topology disagreement, excessive work, or an invalid map.
pub fn plan_foot_cycle_parameterization_v1(
    parameterization: &FootCycleParameterizationV1,
    parameterization_input: InputIdentity,
    manifest: &CollectionManifestV1,
    manifest_input: InputIdentity,
    evidence: &[FootCycleMemberEvidenceV1],
) -> Result<FootCyclePlanV1, FootCycleParameterizationError> {
    if parameterization_input.bytes() > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES {
        return Err(FootCycleParameterizationError::ParameterizationTooLarge);
    }
    validate_foot_cycle_manifest_binding_v1(parameterization, manifest, &manifest_input)?;
    if evidence.len() != parameterization.members.len() {
        return Err(FootCycleParameterizationError::EvidenceCountMismatch);
    }
    validate_aggregate_contact_event_budget(
        evidence.iter().map(|row| row.fragment.events().len()),
    )?;
    validate_aggregate_contact_fragment_byte_budget(evidence.iter().map(|row| row.input.bytes()))?;

    let mut topologies = Vec::with_capacity(evidence.len());
    for (index, (declaration, evidence)) in
        parameterization.members.iter().zip(evidence).enumerate()
    {
        if declaration.id != evidence.id
            || declaration.contact_fragment != evidence.contact_fragment_path
        {
            return Err(FootCycleParameterizationError::EvidenceMemberMismatch);
        }
        let canonical = evidence
            .fragment
            .canonical_json()
            .map_err(|_| FootCycleParameterizationError::NonCanonicalFragment)?;
        if canonical.len() as u64 != evidence.input.bytes()
            || InputIdentity::from_bytes(&canonical) != evidence.input
        {
            return Err(FootCycleParameterizationError::NonCanonicalFragment);
        }
        validate_clip_witness(manifest, &declaration.id, &evidence.fragment)?;
        validate_root_motion(&declaration.id, &evidence.fragment, &evidence.root_motion)?;
        let topology = topology(&evidence.fragment)?;
        topologies.push(MemberTopology {
            evidence_index: index,
            signature: topology.0,
            rotated_boundaries: topology.1,
            contact_height_m_bits: topology.2,
        });
    }

    let reference_index = parameterization
        .members
        .iter()
        .position(|member| member.id == parameterization.reference_member)
        .ok_or(FootCycleParameterizationError::MissingReferenceMember)?;
    let reference = &topologies[reference_index];
    for member in &topologies {
        if member.contact_height_m_bits != reference.contact_height_m_bits {
            return Err(FootCycleParameterizationError::DetectorPolicyMismatch);
        }
        if member.signature != reference.signature {
            return Err(FootCycleParameterizationError::TopologyMismatch);
        }
    }

    let mut plans = Vec::with_capacity(topologies.len());
    for topology in &topologies {
        let source = &evidence[topology.evidence_index];
        let control_points = build_control_points(
            &topology.rotated_boundaries,
            &reference.rotated_boundaries,
            parameterization.minimum_segment_slope,
            parameterization.maximum_segment_slope,
        )?;
        let binding = ContactTransformBindingV1::new(
            source.fragment.artifact().clone(),
            source.fragment.dependency_closure_identity().clone(),
            source.input.clone(),
        );
        plans.push(FootCycleMemberPlanV1 {
            id: source.id.clone(),
            input: binding,
            root_motion: source.root_motion.clone(),
            operation: ContactTransformOperationV1::time_warp(
                source.fragment.duration_s(),
                control_points,
            ),
        });
    }

    Ok(FootCyclePlanV1 {
        parameterization_input,
        manifest_input,
        runtime_set_id: parameterization.runtime_set_id.clone(),
        reference_member: parameterization.reference_member.clone(),
        proof: parameterization.proof.clone(),
        members: plans,
    })
}

fn validate_root_motion(
    member: &CollectionLogicalIdV1,
    fragment: &ContactFragmentV1,
    evidence: &FootCycleRootMotionEvidenceV1,
) -> Result<(), FootCycleParameterizationError> {
    let binding = evidence.binding();
    if binding.artifact != *fragment.artifact()
        || binding.dependency_closure_identity != *fragment.dependency_closure_identity()
        || binding.clip != *fragment.clip()
    {
        return Err(FootCycleParameterizationError::RootMotionBindingMismatch {
            member: member.as_str().to_owned(),
        });
    }
    let FootCycleRootMotionEvidenceV1::Measured {
        endpoint_displacement_x_m,
        endpoint_displacement_z_m,
        accumulated_yaw_deg,
        ..
    } = evidence
    else {
        return Err(
            FootCycleParameterizationError::RootMotionEvidenceUnavailable {
                member: member.as_str().to_owned(),
            },
        );
    };
    if !endpoint_displacement_x_m.is_finite()
        || !endpoint_displacement_z_m.is_finite()
        || !accumulated_yaw_deg.is_finite()
    {
        return Err(
            FootCycleParameterizationError::RootMotionEvidenceUnavailable {
                member: member.as_str().to_owned(),
            },
        );
    }
    if endpoint_displacement_x_m.hypot(*endpoint_displacement_z_m)
        > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M
        || accumulated_yaw_deg.abs() > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG
    {
        return Err(FootCycleParameterizationError::RootMotionOutOfRange {
            member: member.as_str().to_owned(),
        });
    }
    Ok(())
}

fn validate_aggregate_contact_event_budget(
    counts: impl IntoIterator<Item = usize>,
) -> Result<(), FootCycleParameterizationError> {
    let mut total = 0usize;
    for count in counts {
        total = total
            .checked_add(count)
            .unwrap_or(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS + 1);
        if total > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS {
            return Err(FootCycleParameterizationError::TooManyContactEvents {
                found: total.min(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS + 1),
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS,
            });
        }
    }
    Ok(())
}

fn validate_aggregate_contact_fragment_byte_budget(
    counts: impl IntoIterator<Item = u64>,
) -> Result<(), FootCycleParameterizationError> {
    let mut total = 0u64;
    for count in counts {
        total = total
            .checked_add(count)
            .unwrap_or(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES + 1);
        if total > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES {
            return Err(
                FootCycleParameterizationError::TooManyContactFragmentBytes {
                    found: total.min(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES + 1),
                    max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
                },
            );
        }
    }
    Ok(())
}

/// Validate one parameterization's pure manifest/runtime-set binding.
///
/// This performs no evidence inspection or host I/O. Frontends may call it
/// before resolving member-reachable source paths; the full planner calls it
/// again defensively before accepting evidence. `CollectionManifestV1`'s own
/// construction contract already guarantees every ordered member has one
/// unique clip row bound to a declared source row.
///
/// # Errors
///
/// Returns [`FootCycleParameterizationError`] when the exact manifest identity,
/// runtime-set id/kind, or ordered member ring does not match the declaration.
pub fn validate_foot_cycle_manifest_binding_v1(
    parameterization: &FootCycleParameterizationV1,
    manifest: &CollectionManifestV1,
    manifest_input: &InputIdentity,
) -> Result<(), FootCycleParameterizationError> {
    if parameterization.manifest.collection_id != *manifest.collection_id()
        || parameterization.manifest.input != *manifest_input
    {
        return Err(FootCycleParameterizationError::ManifestMismatch);
    }
    let mut matches = manifest
        .runtime_sets()
        .iter()
        .filter(|set| set.id() == &parameterization.runtime_set_id);
    let set = matches
        .next()
        .ok_or(FootCycleParameterizationError::RuntimeSetMismatch)?;
    if matches.next().is_some() {
        return Err(FootCycleParameterizationError::RuntimeSetMismatch);
    }
    if set.kind() != CollectionRuntimeSetKindV1::GaitGroup {
        return Err(FootCycleParameterizationError::WrongRuntimeSetKind);
    }
    if !set
        .members()
        .iter()
        .eq(parameterization.members.iter().map(|member| &member.id))
    {
        return Err(FootCycleParameterizationError::MemberOrderMismatch);
    }
    Ok(())
}

fn validate_clip_witness(
    manifest: &CollectionManifestV1,
    member: &CollectionLogicalIdV1,
    fragment: &ContactFragmentV1,
) -> Result<(), FootCycleParameterizationError> {
    let clip = manifest
        .clips()
        .binary_search_by(|clip| clip.id().cmp(member))
        .ok()
        .and_then(|index| manifest.clips().get(index))
        .ok_or(FootCycleParameterizationError::FragmentClipMismatch)?;
    match fragment.clip() {
        ContactClipReferenceV1::Collection {
            logical_id,
            source,
            take_index,
            take_name,
        } if logical_id == member.as_str()
            && source == clip.source().as_str()
            && *take_index == clip.take_index()
            && take_name == clip.take_name() =>
        {
            Ok(())
        }
        ContactClipReferenceV1::Document { .. } | ContactClipReferenceV1::Collection { .. } => {
            Err(FootCycleParameterizationError::FragmentClipMismatch)
        }
    }
}

fn topology(
    fragment: &ContactFragmentV1,
) -> Result<(Vec<BoundaryKind>, Vec<Boundary>, u64), FootCycleParameterizationError> {
    let detector = validate_detector_extension(fragment)?;
    let mut windows = [Vec::new(), Vec::new()];
    let mut markers = [Vec::new(), Vec::new()];
    for event in fragment.events() {
        let side = event_side(event.role(), detector.roles)
            .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
        let side_index = side_index(side);
        match (event.phase(), event.kind()) {
            (ContactPhaseV1::Begin, ContactEventKindV1::Window(window))
                if window.start() < window.end() =>
            {
                windows[side_index].push(SupportWindow {
                    side,
                    start: window.start(),
                    end: window.end(),
                });
            }
            (ContactPhaseV1::Marker, ContactEventKindV1::Point(time)) => {
                markers[side_index].push(time);
            }
            _ => return Err(FootCycleParameterizationError::InvalidContactTopology),
        }
    }
    let mut logical = Vec::new();
    for (side_windows, side_markers) in windows.iter_mut().zip(&mut markers) {
        side_windows.sort_by(|left, right| left.start.total_cmp(&right.start));
        side_markers.sort_by(|left, right| left.total_cmp(right));
        if side_windows.is_empty()
            || side_windows.len() != side_markers.len()
            || side_windows
                .windows(2)
                .any(|pair| pair[0].end >= pair[1].start)
            || side_windows
                .iter()
                .zip(side_markers)
                .any(|(window, marker)| *marker < window.start || *marker > window.end)
        {
            return Err(FootCycleParameterizationError::InvalidContactTopology);
        }

        let first = side_windows[0];
        let last = side_windows[side_windows.len() - 1];
        if first.start == 0.0 && last.end == 1.0 {
            if side_windows.len() == 1 {
                return Err(FootCycleParameterizationError::InvalidContactTopology);
            }
            logical.push(LogicalSupportWindow {
                side: first.side,
                onset: last.start,
                release: first.end,
            });
            logical.extend(
                side_windows[1..side_windows.len() - 1]
                    .iter()
                    .map(|window| LogicalSupportWindow {
                        side: window.side,
                        onset: window.start,
                        release: window.end,
                    }),
            );
        } else {
            logical.extend(side_windows.iter().map(|window| LogicalSupportWindow {
                side: window.side,
                onset: window.start,
                release: window.end,
            }));
        }
    }

    let left_count = logical
        .iter()
        .filter(|window| window.side == Side::Left)
        .count();
    let right_count = logical.len() - left_count;
    if left_count == 0 || left_count != right_count {
        return Err(FootCycleParameterizationError::InvalidContactTopology);
    }
    logical.sort_by(|left, right| {
        left.onset
            .total_cmp(&right.onset)
            .then_with(|| side_index(left.side).cmp(&side_index(right.side)))
    });
    if logical
        .iter()
        .zip(logical.iter().cycle().skip(1))
        .take(logical.len())
        .any(|(left, right)| left.side == right.side)
    {
        return Err(FootCycleParameterizationError::InvalidContactTopology);
    }

    let mut boundaries = Vec::with_capacity(logical.len() * 2);
    for window in &logical {
        boundaries.push(Boundary {
            kind: BoundaryKind {
                side: window.side,
                edge: Edge::Onset,
            },
            time: window.onset,
        });
        boundaries.push(Boundary {
            kind: BoundaryKind {
                side: window.side,
                edge: Edge::Release,
            },
            time: window.release,
        });
    }
    boundaries.sort_by(|left, right| {
        left.time
            .total_cmp(&right.time)
            .then_with(|| boundary_kind_key(left.kind).cmp(&boundary_kind_key(right.kind)))
    });
    let origin = boundaries
        .iter()
        .position(|boundary| boundary.kind.side == Side::Left && boundary.kind.edge == Edge::Onset)
        .ok_or(FootCycleParameterizationError::InvalidContactTopology)?;
    boundaries.rotate_left(origin);
    let signature = boundaries.iter().map(|boundary| boundary.kind).collect();
    Ok((signature, boundaries, detector.contact_height_m_bits))
}

const fn side_index(side: Side) -> usize {
    match side {
        Side::Left => 0,
        Side::Right => 1,
    }
}

const fn boundary_kind_key(kind: BoundaryKind) -> (usize, usize) {
    (
        side_index(kind.side),
        match kind.edge {
            Edge::Onset => 0,
            Edge::Release => 1,
        },
    )
}

#[derive(Debug, Clone, Copy)]
struct DetectorProvenance {
    roles: [ContactRoleV1; 2],
    contact_height_m_bits: u64,
}

fn validate_detector_extension(
    fragment: &ContactFragmentV1,
) -> Result<DetectorProvenance, FootCycleParameterizationError> {
    let [extension] = fragment.extensions() else {
        return Err(FootCycleParameterizationError::UnsupportedContactExtension);
    };
    validate_detector_extension_value(extension)
}

fn validate_detector_extension_value(
    extension: &crate::ContactExtensionV1,
) -> Result<DetectorProvenance, FootCycleParameterizationError> {
    if extension.schema() != CONTACT_SUPPORT_DETECTOR_V1_ID
        || extension.schema_version() != CONTACT_SUPPORT_DETECTOR_V1_SCHEMA_VERSION
    {
        return Err(FootCycleParameterizationError::UnsupportedContactExtension);
    }
    let payload = extension
        .payload()
        .as_object()
        .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
    let expected = [
        "algorithm",
        "contact_height_m",
        "max_frames",
        "roles",
        "sampling",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let Some(contact_height_m) = payload
        .get("contact_height_m")
        .and_then(serde_json::Value::as_f64)
    else {
        return Err(FootCycleParameterizationError::InvalidDetectorProvenance);
    };
    if payload.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || payload.get("algorithm").and_then(serde_json::Value::as_str)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_ALGORITHM)
        || payload.get("sampling").and_then(serde_json::Value::as_str)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_SAMPLING)
        || payload
            .get("max_frames")
            .and_then(serde_json::Value::as_u64)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_MAX_FRAMES)
        || !contact_height_m.is_finite()
        || contact_height_m < 0.0
    {
        return Err(FootCycleParameterizationError::InvalidDetectorProvenance);
    }
    let roles = payload
        .get("roles")
        .and_then(serde_json::Value::as_object)
        .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
    if roles.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != ["left", "right"].into_iter().collect::<BTreeSet<_>>()
    {
        return Err(FootCycleParameterizationError::InvalidDetectorProvenance);
    }
    let left = match roles.get("left").and_then(serde_json::Value::as_str) {
        Some("left_foot") => ContactRoleV1::LeftFoot,
        Some("left_toe") => ContactRoleV1::LeftToe,
        _ => return Err(FootCycleParameterizationError::InvalidDetectorProvenance),
    };
    let right = match roles.get("right").and_then(serde_json::Value::as_str) {
        Some("right_foot") => ContactRoleV1::RightFoot,
        Some("right_toe") => ContactRoleV1::RightToe,
        _ => return Err(FootCycleParameterizationError::InvalidDetectorProvenance),
    };
    Ok(DetectorProvenance {
        roles: [left, right],
        contact_height_m_bits: canonical_zero(contact_height_m).to_bits(),
    })
}

/// Reconstruct the known stance-detector extension for a V1 time warp.
///
/// Detector configuration and role provenance are operation-invariant; event
/// times live in the fragment rows transformed by the contact-transform
/// authority. This function nevertheless validates the complete closed
/// detector payload and operation kind before reconstructing the handler-owned
/// output, so frontends never authorize an opaque extension copy.
///
/// # Errors
///
/// Returns [`FootCycleParameterizationError`] when the operation is not a V1
/// time warp or the extension is not the exact known stance-detector payload.
pub fn transform_contact_support_detector_extension_time_warp_v1(
    extension: &crate::ContactExtensionV1,
    operation: &ContactTransformOperationV1,
) -> Result<crate::ContactExtensionV1, FootCycleParameterizationError> {
    if !matches!(
        operation,
        ContactTransformOperationV1::TimeWarp { version: 1, .. }
    ) {
        return Err(FootCycleParameterizationError::UnsupportedContactExtension);
    }
    let detector = validate_detector_extension_value(extension)?;
    let left = detector_role_name(detector.roles[0])
        .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
    let right = detector_role_name(detector.roles[1])
        .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
    let payload = serde_json::json!({
        "algorithm": CONTACT_SUPPORT_DETECTOR_V1_ALGORITHM,
        "contact_height_m": f64::from_bits(detector.contact_height_m_bits),
        "max_frames": CONTACT_SUPPORT_DETECTOR_V1_MAX_FRAMES,
        "roles": {
            "left": left,
            "right": right,
        },
        "sampling": CONTACT_SUPPORT_DETECTOR_V1_SAMPLING,
    });
    crate::ContactExtensionV1::new(
        CONTACT_SUPPORT_DETECTOR_V1_ID,
        CONTACT_SUPPORT_DETECTOR_V1_SCHEMA_VERSION,
        payload,
    )
    .map_err(|_| FootCycleParameterizationError::InvalidDetectorProvenance)
}

fn detector_role_name(role: ContactRoleV1) -> Option<&'static str> {
    match role {
        ContactRoleV1::LeftFoot => Some("left_foot"),
        ContactRoleV1::LeftToe => Some("left_toe"),
        ContactRoleV1::RightFoot => Some("right_foot"),
        ContactRoleV1::RightToe => Some("right_toe"),
        _ => None,
    }
}

fn event_side(role: ContactRoleV1, roles: [ContactRoleV1; 2]) -> Option<Side> {
    if role == roles[0] {
        Some(Side::Left)
    } else if role == roles[1] {
        Some(Side::Right)
    } else {
        None
    }
}

fn build_control_points(
    source: &[Boundary],
    reference: &[Boundary],
    minimum_slope: f64,
    maximum_slope: f64,
) -> Result<Vec<ContactTimeWarpControlPointV1>, FootCycleParameterizationError> {
    if source.len() != reference.len()
        || source
            .iter()
            .zip(reference)
            .any(|(left, right)| left.kind != right.kind)
    {
        return Err(FootCycleParameterizationError::TopologyMismatch);
    }
    let mut points = source
        .iter()
        .zip(reference)
        .map(|(source, reference)| (canonical_zero(source.time), canonical_zero(reference.time)))
        .collect::<Vec<_>>();
    points.push((0.0, 0.0));
    points.push((1.0, 1.0));
    points.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
    });
    for group in points.chunk_by(|left, right| left.0 == right.0) {
        if group
            .first()
            .zip(group.last())
            .is_some_and(|(first, last)| first.1 != last.1)
        {
            return Err(FootCycleParameterizationError::NonMonotoneMapping);
        }
    }
    points.dedup();
    let needed = points.len();
    if needed > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS {
        return Err(FootCycleParameterizationError::TooManyControlPoints {
            found: needed,
            max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS,
        });
    }

    for pair in points.windows(2) {
        let (x0, y0) = pair[0];
        let (x1, y1) = pair[1];
        if !(x0 < x1 && y0 < y1) {
            return Err(FootCycleParameterizationError::NonMonotoneMapping);
        }
        let slope = (y1 - y0) / (x1 - x0);
        if !slope.is_finite() || slope < minimum_slope || slope > maximum_slope {
            return Err(FootCycleParameterizationError::SegmentSlopeOutOfRange);
        }
    }
    Ok(points
        .into_iter()
        .map(|(input, output)| ContactTimeWarpControlPointV1::new(input, output))
        .collect())
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CollectionClipV1, CollectionRuntimeSetV1, CollectionSourceKeyV1, CollectionSourceV1,
        ContactEventV1, ContactEventWindowV1, ContactExtensionV1, ContactProducerV1,
        DependencyClosureBuilderV1, ResourceKeySyntaxV1, SourceSetCoverageV1,
    };
    use serde_json::json;

    fn id(value: &str) -> CollectionLogicalIdV1 {
        CollectionLogicalIdV1::new(value).unwrap()
    }

    fn path(value: &str) -> DependencyResourceKeyV1 {
        DependencyResourceKeyV1::from_source_str(value, ResourceKeySyntaxV1::ParserRelativePath)
            .unwrap()
    }

    fn manifest() -> (CollectionManifestV1, InputIdentity) {
        manifest_with_set(
            CollectionRuntimeSetKindV1::GaitGroup,
            &["reference", "member"],
        )
    }

    fn manifest_with_set(
        kind: CollectionRuntimeSetKindV1,
        set_member_names: &[&str],
    ) -> (CollectionManifestV1, InputIdentity) {
        manifest_for_members(kind, &["reference", "member"], set_member_names)
    }

    fn manifest_for_members(
        kind: CollectionRuntimeSetKindV1,
        clip_names: &[&str],
        set_member_names: &[&str],
    ) -> (CollectionManifestV1, InputIdentity) {
        let collection = CollectionIdV1::new("com.example").unwrap();
        let source = CollectionSourceKeyV1::new("motions").unwrap();
        let clips = clip_names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                CollectionClipV1::new(
                    id(&format!("com.example/{name}")),
                    source.clone(),
                    index as u32,
                    format!("Take {index}"),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let set_members = set_member_names
            .iter()
            .map(|name| id(&format!("com.example/{name}")))
            .collect();
        let manifest = CollectionManifestV1::new(
            collection,
            None,
            vec![CollectionSourceV1::new(
                source,
                path("motions.glb"),
                None,
                None,
            )],
            clips,
            vec![CollectionRuntimeSetV1::new(
                id("com.example/sets/walk"),
                kind,
                set_members,
            )],
        )
        .unwrap();
        (manifest, InputIdentity::from_bytes(b"manifest"))
    }

    fn declaration(
        manifest_input: &InputIdentity,
        minimum_slope: f64,
        maximum_slope: f64,
    ) -> FootCycleParameterizationV1 {
        declaration_with_proof(manifest_input, minimum_slope, maximum_slope, proof_policy())
    }

    fn declaration_with_proof(
        manifest_input: &InputIdentity,
        minimum_slope: f64,
        maximum_slope: f64,
        proof: FootCycleProofPolicyV1,
    ) -> FootCycleParameterizationV1 {
        FootCycleParameterizationV1::new(
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                manifest_input.clone(),
            )
            .unwrap(),
            id("com.example/sets/walk"),
            id("com.example/reference"),
            path("generated/aligned"),
            minimum_slope,
            maximum_slope,
            proof,
            vec![
                FootCycleParameterizationMemberV1::new(
                    id("com.example/reference"),
                    path("contacts/reference.json"),
                ),
                FootCycleParameterizationMemberV1::new(
                    id("com.example/member"),
                    path("contacts/member.json"),
                ),
            ],
        )
        .unwrap()
    }

    fn proof_policy() -> FootCycleProofPolicyV1 {
        FootCycleProofPolicyV1::new(0.08, 0.05, 0.01).unwrap()
    }

    fn detector_extension(left: &str, right: &str) -> ContactExtensionV1 {
        detector_extension_with_height(left, right, 0.03)
    }

    fn detector_extension_with_height(
        left: &str,
        right: &str,
        contact_height_m: f64,
    ) -> ContactExtensionV1 {
        detector_extension_with_payload(json!({
            "algorithm": "stance-support-v1",
            "sampling": "metric-grid-longest-authored-channel",
            "max_frames": 1_000_000,
            "contact_height_m": contact_height_m,
            "roles": {"left": left, "right": right},
        }))
    }

    fn detector_extension_with_payload(payload: serde_json::Value) -> ContactExtensionV1 {
        ContactExtensionV1::new(CONTACT_SUPPORT_DETECTOR_V1_ID, 1, payload).unwrap()
    }

    fn fragment(member: &str, take_index: u32, windows: &[(Side, f64, f64)]) -> ContactFragmentV1 {
        fragment_with_extensions(
            member,
            take_index,
            windows,
            vec![detector_extension("left_foot", "right_foot")],
        )
    }

    fn fragment_with_extensions(
        member: &str,
        take_index: u32,
        windows: &[(Side, f64, f64)],
        extensions: Vec<ContactExtensionV1>,
    ) -> ContactFragmentV1 {
        fragment_with_clip(
            member,
            "motions",
            take_index,
            &format!("Take {take_index}"),
            1.0,
            windows,
            None,
            None,
            extensions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn fragment_with_clip(
        member: &str,
        source: &str,
        take_index: u32,
        take_name: &str,
        duration_s: f64,
        windows: &[(Side, f64, f64)],
        markers: Option<&[(Side, f64)]>,
        event_roles: Option<[ContactRoleV1; 2]>,
        extensions: Vec<ContactExtensionV1>,
    ) -> ContactFragmentV1 {
        let mut events = Vec::with_capacity(windows.len() * 2);
        for (index, &(side, start, end)) in windows.iter().enumerate() {
            let (role, label) = match side {
                Side::Left => (
                    event_roles.map_or(ContactRoleV1::LeftFoot, |roles| roles[0]),
                    "left",
                ),
                Side::Right => (
                    event_roles.map_or(ContactRoleV1::RightFoot, |roles| roles[1]),
                    "right",
                ),
            };
            events.push(
                ContactEventV1::window(
                    format!("support/{label}/{index}"),
                    role,
                    ContactPhaseV1::Begin,
                    ContactEventWindowV1::new(start, end).unwrap(),
                    None,
                )
                .unwrap(),
            );
        }
        let default_markers = windows
            .iter()
            .map(|&(side, start, end)| (side, (start + end) / 2.0))
            .collect::<Vec<_>>();
        for (index, &(side, time)) in markers.unwrap_or(&default_markers).iter().enumerate() {
            let (role, label) = match side {
                Side::Left => (
                    event_roles.map_or(ContactRoleV1::LeftFoot, |roles| roles[0]),
                    "left",
                ),
                Side::Right => (
                    event_roles.map_or(ContactRoleV1::RightFoot, |roles| roles[1]),
                    "right",
                ),
            };
            events.push(
                ContactEventV1::point(
                    format!("marker/{label}/{index}"),
                    role,
                    ContactPhaseV1::Marker,
                    time,
                    None,
                )
                .unwrap(),
            );
        }
        fragment_with_events(
            member, source, take_index, take_name, duration_s, events, extensions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn fragment_with_events(
        member: &str,
        source: &str,
        take_index: u32,
        take_name: &str,
        duration_s: f64,
        events: Vec<ContactEventV1>,
        extensions: Vec<ContactExtensionV1>,
    ) -> ContactFragmentV1 {
        let artifact = InputIdentity::from_bytes(member.as_bytes());
        let closure =
            DependencyClosureBuilderV1::new(artifact.clone(), SourceSetCoverageV1::complete(), 0)
                .finish()
                .unwrap();
        ContactFragmentV1::new(
            ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
            artifact,
            closure.identity().unwrap().clone(),
            ContactClipReferenceV1::collection(
                format!("com.example/{member}"),
                source,
                take_index,
                take_name,
            )
            .unwrap(),
            duration_s,
            events,
            extensions,
        )
        .unwrap()
    }

    fn root_binding(fragment: &ContactFragmentV1) -> FootCycleRootMotionBindingV1 {
        FootCycleRootMotionBindingV1::new(
            fragment.artifact().clone(),
            fragment.dependency_closure_identity().clone(),
            fragment.clip().clone(),
        )
    }

    fn measured_root(
        fragment: &ContactFragmentV1,
        horizontal_endpoint_displacement_m: f64,
        accumulated_yaw_deg: f64,
    ) -> FootCycleRootMotionEvidenceV1 {
        FootCycleRootMotionEvidenceV1::measured(
            root_binding(fragment),
            horizontal_endpoint_displacement_m,
            0.0,
            accumulated_yaw_deg,
        )
    }

    fn evidence(
        reference_windows: &[(Side, f64, f64)],
        member_windows: &[(Side, f64, f64)],
    ) -> Vec<FootCycleMemberEvidenceV1> {
        [
            ("reference", 0, "contacts/reference.json", reference_windows),
            ("member", 1, "contacts/member.json", member_windows),
        ]
        .into_iter()
        .map(|(member, take, source, windows)| {
            let fragment = fragment(member, take, windows);
            let root_motion = measured_root(&fragment, 0.0, 0.0);
            FootCycleMemberEvidenceV1::new(
                id(&format!("com.example/{member}")),
                path(source),
                fragment.canonical_identity().unwrap(),
                fragment,
                root_motion,
            )
        })
        .collect()
    }

    fn points(plan: &FootCycleMemberPlanV1) -> Vec<(f64, f64)> {
        plan.operation()
            .control_points()
            .unwrap()
            .iter()
            .map(|point| (point.input_time(), point.output_time()))
            .collect()
    }

    fn alternating_windows(count: usize) -> Vec<(Side, f64, f64)> {
        let denominator = (count * 3 + 1) as f64;
        (0..count)
            .map(|index| {
                (
                    if index % 2 == 0 {
                        Side::Left
                    } else {
                        Side::Right
                    },
                    (index * 3 + 1) as f64 / denominator,
                    (index * 3 + 2) as f64 / denominator,
                )
            })
            .collect()
    }

    #[test]
    fn planner_maps_exact_boundaries_to_reference_phases_and_is_deterministic() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let mut evidence = evidence(
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
            &[(Side::Left, 0.2, 0.3), (Side::Right, 0.7, 0.8)],
        );
        let reference = fragment_with_clip(
            "reference",
            "motions",
            0,
            "Take 0",
            3.5,
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
            None,
            None,
            vec![detector_extension("left_foot", "right_foot")],
        );
        evidence[0].input = reference.canonical_identity().unwrap();
        evidence[0].fragment = reference;
        let member = fragment_with_clip(
            "member",
            "motions",
            1,
            "Take 1",
            2.5,
            &[(Side::Left, 0.2, 0.3), (Side::Right, 0.7, 0.8)],
            None,
            None,
            vec![detector_extension("left_foot", "right_foot")],
        );
        evidence[1].input = member.canonical_identity().unwrap();
        evidence[1].fragment = member;
        let parameterization_input = InputIdentity::from_bytes(b"parameterization");
        let first = plan_foot_cycle_parameterization_v1(
            &declaration,
            parameterization_input.clone(),
            &manifest,
            manifest_input.clone(),
            &evidence,
        )
        .unwrap();
        let second = plan_foot_cycle_parameterization_v1(
            &declaration,
            parameterization_input.clone(),
            &manifest,
            manifest_input.clone(),
            &evidence,
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.parameterization_input(), &parameterization_input);
        assert_eq!(first.manifest_input(), &manifest_input);
        assert_eq!(first.runtime_set_id(), declaration.runtime_set_id());
        assert_eq!(first.reference_member(), declaration.reference_member());
        assert_eq!(first.members().len(), 2);
        assert_eq!(
            points(&first.members()[0]),
            [
                (0.0, 0.0),
                (0.1, 0.1),
                (0.2, 0.2),
                (0.6, 0.6),
                (0.7, 0.7),
                (1.0, 1.0)
            ]
        );
        assert_eq!(
            points(&first.members()[1]),
            [
                (0.0, 0.0),
                (0.2, 0.1),
                (0.3, 0.2),
                (0.7, 0.6),
                (0.8, 0.7),
                (1.0, 1.0)
            ]
        );
        assert_eq!(
            first.members()[0].operation().output_duration_s(),
            Some(3.5)
        );
        assert_eq!(
            first.members()[1].operation().output_duration_s(),
            Some(2.5)
        );
        for (plan, source) in first.members().iter().zip(&evidence) {
            assert_eq!(plan.id(), source.id());
            assert_eq!(plan.input().artifact(), source.fragment().artifact());
            assert_eq!(
                plan.input().dependency_closure_identity(),
                source.fragment().dependency_closure_identity()
            );
            assert_eq!(plan.input().fragment(), source.input());
            assert_eq!(plan.root_motion(), source.root_motion());
            assert_eq!(
                plan.operation().output_duration_s(),
                Some(source.fragment().duration_s())
            );
        }
    }

    #[test]
    fn planner_positionally_maps_every_boundary_in_a_larger_ring() {
        let reference = [
            (Side::Left, 0.05, 0.1),
            (Side::Right, 0.25, 0.3),
            (Side::Left, 0.5, 0.55),
            (Side::Right, 0.75, 0.8),
        ];
        let member = [
            (Side::Left, 0.08, 0.14),
            (Side::Right, 0.3, 0.36),
            (Side::Left, 0.56, 0.62),
            (Side::Right, 0.82, 0.88),
        ];
        let (manifest, manifest_input) = manifest();
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration(&manifest_input, 0.1, 10.0),
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input,
            &evidence(&reference, &member),
        )
        .unwrap();
        assert_eq!(
            points(&plan.members()[1]),
            [
                (0.0, 0.0),
                (0.08, 0.05),
                (0.14, 0.1),
                (0.3, 0.25),
                (0.36, 0.3),
                (0.56, 0.5),
                (0.62, 0.55),
                (0.82, 0.75),
                (0.88, 0.8),
                (1.0, 1.0),
            ]
        );
    }

    #[test]
    fn root_motion_gate_is_inclusive_and_refuses_unavailable_or_excess_evidence() {
        assert_eq!(
            FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M,
            0.01
        );
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG, 1.0);
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let parameterization_input = || InputIdentity::from_bytes(b"parameterization");

        for member_index in 0..2 {
            let mut accepted = evidence(&windows, &windows);
            let binding = accepted[member_index].root_motion.binding().clone();
            accepted[member_index].root_motion = FootCycleRootMotionEvidenceV1::measured(
                binding,
                0.006,
                -0.008,
                -FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG,
            );
            let plan = plan_foot_cycle_parameterization_v1(
                &declaration,
                parameterization_input(),
                &manifest,
                manifest_input.clone(),
                &accepted,
            )
            .unwrap();
            assert!(matches!(
                plan.members()[member_index].root_motion(),
                FootCycleRootMotionEvidenceV1::Measured {
                    endpoint_displacement_x_m: 0.006,
                    endpoint_displacement_z_m: -0.008,
                    accumulated_yaw_deg: -1.0,
                    ..
                }
            ));
        }

        for member_index in 0..2 {
            for unavailable_index in 0..9 {
                let mut source = evidence(&windows, &windows);
                let binding = source[member_index].root_motion.binding().clone();
                let unavailable = match unavailable_index {
                    0 => FootCycleRootMotionEvidenceV1::missing(binding),
                    1 => FootCycleRootMotionEvidenceV1::ambiguous(binding),
                    2 => FootCycleRootMotionEvidenceV1::non_finite(binding),
                    3 => FootCycleRootMotionEvidenceV1::measured(binding, f64::NAN, 0.0, 0.0),
                    4 => FootCycleRootMotionEvidenceV1::measured(binding, 0.0, f64::NAN, 0.0),
                    5 => FootCycleRootMotionEvidenceV1::measured(binding, 0.0, 0.0, f64::NAN),
                    6 => FootCycleRootMotionEvidenceV1::measured(binding, f64::INFINITY, 0.0, 0.0),
                    7 => FootCycleRootMotionEvidenceV1::measured(
                        binding,
                        0.0,
                        f64::NEG_INFINITY,
                        0.0,
                    ),
                    8 => FootCycleRootMotionEvidenceV1::measured(
                        binding,
                        0.0,
                        0.0,
                        f64::NEG_INFINITY,
                    ),
                    _ => unreachable!(),
                };
                source[member_index].root_motion = unavailable;
                assert!(matches!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        parameterization_input(),
                        &manifest,
                        manifest_input.clone(),
                        &source,
                    ),
                    Err(FootCycleParameterizationError::RootMotionEvidenceUnavailable { .. })
                ));
            }
        }

        for (endpoint_x, endpoint_z, yaw) in [
            (
                f64::from_bits(
                    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M.to_bits() + 1,
                ),
                0.0,
                0.0,
            ),
            (
                0.0,
                f64::from_bits(
                    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M.to_bits() + 1,
                ),
                0.0,
            ),
            (
                0.0,
                0.0,
                f64::from_bits(
                    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG.to_bits() + 1,
                ),
            ),
            (
                0.0,
                0.0,
                -f64::from_bits(
                    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG.to_bits() + 1,
                ),
            ),
        ] {
            for member_index in 0..2 {
                let mut source = evidence(&windows, &windows);
                let binding = source[member_index].root_motion.binding().clone();
                source[member_index].root_motion =
                    FootCycleRootMotionEvidenceV1::measured(binding, endpoint_x, endpoint_z, yaw);
                assert!(matches!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        parameterization_input(),
                        &manifest,
                        manifest_input.clone(),
                        &source,
                    ),
                    Err(FootCycleParameterizationError::RootMotionOutOfRange { .. })
                ));
            }
        }
    }

    #[test]
    fn root_motion_artifact_closure_and_clip_witnesses_cannot_be_cross_wired() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        for member_index in 0..2 {
            let other_index = 1 - member_index;
            for coordinate in 0..3 {
                let mut source = evidence(&windows, &windows);
                let other = source[other_index].root_motion.binding().clone();
                let FootCycleRootMotionEvidenceV1::Measured { binding, .. } =
                    &mut source[member_index].root_motion
                else {
                    unreachable!();
                };
                match coordinate {
                    0 => binding.artifact = other.artifact,
                    1 => binding.dependency_closure_identity = other.dependency_closure_identity,
                    2 => binding.clip = other.clip,
                    _ => unreachable!(),
                }
                assert!(matches!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input.clone(),
                        &source,
                    ),
                    Err(FootCycleParameterizationError::RootMotionBindingMismatch { .. })
                ));
            }
        }
    }

    #[test]
    fn cyclic_topology_is_compared_from_each_first_left_onset() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let evidence = evidence(
            &[(Side::Right, 0.1, 0.2), (Side::Left, 0.6, 0.7)],
            &[(Side::Right, 0.15, 0.25), (Side::Left, 0.65, 0.75)],
        );
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input.clone(),
            &evidence,
        )
        .unwrap();
        assert_eq!(
            points(&plan.members()[0]),
            [
                (0.0, 0.0),
                (0.1, 0.1),
                (0.2, 0.2),
                (0.6, 0.6),
                (0.7, 0.7),
                (1.0, 1.0),
            ]
        );
        assert_eq!(
            points(&plan.members()[1]),
            [
                (0.0, 0.0),
                (0.15, 0.1),
                (0.25, 0.2),
                (0.65, 0.6),
                (0.75, 0.7),
                (1.0, 1.0),
            ]
        );
    }

    #[test]
    fn nonfirst_reference_member_owns_exact_phases_without_reordering_plans() {
        let (manifest, manifest_input) = manifest();
        let declaration = FootCycleParameterizationV1::new(
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                manifest_input.clone(),
            )
            .unwrap(),
            id("com.example/sets/walk"),
            id("com.example/member"),
            path("generated/aligned"),
            0.5,
            2.0,
            proof_policy(),
            vec![
                FootCycleParameterizationMemberV1::new(
                    id("com.example/reference"),
                    path("contacts/reference.json"),
                ),
                FootCycleParameterizationMemberV1::new(
                    id("com.example/member"),
                    path("contacts/member.json"),
                ),
            ],
        )
        .unwrap();
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input,
            &evidence(
                &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
                &[(Side::Left, 0.2, 0.3), (Side::Right, 0.7, 0.8)],
            ),
        )
        .unwrap();
        assert_eq!(plan.reference_member(), &id("com.example/member"));
        assert_eq!(plan.members()[0].id(), &id("com.example/reference"));
        assert_eq!(plan.members()[1].id(), &id("com.example/member"));
        assert_eq!(
            points(&plan.members()[0]),
            [
                (0.0, 0.0),
                (0.1, 0.2),
                (0.2, 0.3),
                (0.6, 0.7),
                (0.7, 0.8),
                (1.0, 1.0),
            ]
        );
        assert_eq!(
            points(&plan.members()[1]),
            [
                (0.0, 0.0),
                (0.2, 0.2),
                (0.3, 0.3),
                (0.7, 0.7),
                (0.8, 0.8),
                (1.0, 1.0),
            ]
        );
    }

    #[test]
    fn planner_refuses_missing_and_extra_evidence_rows() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let source = evidence(&windows, &windows);
        for invalid in [source[..1].to_vec(), {
            let mut extra = source.clone();
            extra.push(source[0].clone());
            extra
        }] {
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &invalid,
                ),
                Err(FootCycleParameterizationError::EvidenceCountMismatch)
            );
        }
    }

    #[test]
    fn phase_rotation_that_cannot_preserve_endpoints_refuses() {
        let (first_manifest, first_manifest_input) = manifest();
        let first_declaration = declaration(&first_manifest_input, 0.01, 100.0);
        let first_evidence = evidence(
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
            &[(Side::Right, 0.1, 0.2), (Side::Left, 0.6, 0.7)],
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &first_declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &first_manifest,
                first_manifest_input,
                &first_evidence,
            ),
            Err(FootCycleParameterizationError::NonMonotoneMapping)
        );

        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.01, 100.0);
        let reference = [
            (Side::Left, 0.05, 0.1),
            (Side::Right, 0.25, 0.3),
            (Side::Left, 0.5, 0.55),
            (Side::Right, 0.75, 0.8),
        ];
        let rotated = [
            (Side::Right, 0.05, 0.1),
            (Side::Left, 0.25, 0.3),
            (Side::Right, 0.5, 0.55),
            (Side::Left, 0.75, 0.8),
        ];
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence(&reference, &rotated),
            ),
            Err(FootCycleParameterizationError::NonMonotoneMapping)
        );
    }

    #[test]
    fn same_side_overlap_touch_nonalternation_and_missing_runs_refuse() {
        let reference = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        for invalid in [
            vec![
                (Side::Left, 0.1, 0.3),
                (Side::Left, 0.2, 0.4),
                (Side::Right, 0.5, 0.6),
                (Side::Right, 0.7, 0.8),
            ],
            vec![
                (Side::Left, 0.1, 0.3),
                (Side::Left, 0.3, 0.4),
                (Side::Right, 0.5, 0.6),
                (Side::Right, 0.7, 0.8),
            ],
            vec![
                (Side::Left, 0.1, 0.15),
                (Side::Left, 0.2, 0.25),
                (Side::Right, 0.4, 0.45),
                (Side::Right, 0.6, 0.65),
            ],
            vec![
                (Side::Left, 0.1, 0.2),
                (Side::Left, 0.4, 0.5),
                (Side::Right, 0.6, 0.7),
            ],
            vec![(Side::Left, 0.1, 0.2)],
            vec![(Side::Left, 0.0, 1.0), (Side::Right, 0.3, 0.7)],
        ] {
            for invalid_index in 0..2 {
                let (manifest, manifest_input) = manifest();
                let declaration = declaration(&manifest_input, 0.01, 100.0);
                let source = if invalid_index == 0 {
                    evidence(&invalid, &reference)
                } else {
                    evidence(&reference, &invalid)
                };
                assert_eq!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input,
                        &source,
                    ),
                    Err(FootCycleParameterizationError::InvalidContactTopology)
                );
            }
        }
    }

    #[test]
    fn opposite_side_overlap_and_matching_touch_are_admitted() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.01, 100.0);
        let overlap = evidence(
            &[(Side::Left, 0.1, 0.45), (Side::Right, 0.3, 0.7)],
            &[(Side::Left, 0.15, 0.5), (Side::Right, 0.35, 0.75)],
        );
        assert!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input.clone(),
                &overlap,
            )
            .is_ok()
        );

        let touching = evidence(
            &[(Side::Left, 0.1, 0.3), (Side::Right, 0.3, 0.6)],
            &[(Side::Left, 0.2, 0.4), (Side::Right, 0.4, 0.7)],
        );
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input,
            &touching,
        )
        .unwrap();
        assert_eq!(
            points(&plan.members()[1]),
            [(0.0, 0.0), (0.2, 0.1), (0.4, 0.3), (0.7, 0.6), (1.0, 1.0)]
        );
    }

    #[test]
    fn one_and_both_side_seam_stances_are_logically_coalesced() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.01, 100.0);
        let one_side = [
            (Side::Left, 0.0, 0.3),
            (Side::Right, 0.2, 0.8),
            (Side::Left, 0.7, 1.0),
        ];
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input.clone(),
            &evidence(&one_side, &one_side),
        )
        .unwrap();
        assert_eq!(
            points(&plan.members()[0]),
            [
                (0.0, 0.0),
                (0.2, 0.2),
                (0.3, 0.3),
                (0.7, 0.7),
                (0.8, 0.8),
                (1.0, 1.0)
            ]
        );

        let both_sides = [
            (Side::Left, 0.0, 0.2),
            (Side::Right, 0.0, 0.3),
            (Side::Left, 0.7, 1.0),
            (Side::Right, 0.8, 1.0),
        ];
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            InputIdentity::from_bytes(b"parameterization"),
            &manifest,
            manifest_input,
            &evidence(&both_sides, &both_sides),
        )
        .unwrap();
        assert_eq!(
            points(&plan.members()[0]),
            [
                (0.0, 0.0),
                (0.2, 0.2),
                (0.3, 0.3),
                (0.7, 0.7),
                (0.8, 0.8),
                (1.0, 1.0)
            ]
        );
    }

    #[test]
    fn seam_topology_contains_only_true_boundaries_in_cyclic_time_order() {
        let fragment = fragment(
            "member",
            0,
            &[
                (Side::Left, 0.0, 0.3),
                (Side::Right, 0.2, 0.8),
                (Side::Left, 0.7, 1.0),
            ],
        );

        let (signature, boundaries, _) = topology(&fragment).unwrap();
        let observed = boundaries
            .iter()
            .map(|boundary| (boundary.kind.side, boundary.kind.edge, boundary.time))
            .collect::<Vec<_>>();

        assert_eq!(
            observed,
            [
                (Side::Left, Edge::Onset, 0.7),
                (Side::Right, Edge::Release, 0.8),
                (Side::Right, Edge::Onset, 0.2),
                (Side::Left, Edge::Release, 0.3),
            ]
        );
        assert_eq!(
            signature,
            observed
                .iter()
                .map(|&(side, edge, _)| BoundaryKind { side, edge })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn simultaneous_boundary_groups_must_match_on_both_axes() {
        let simultaneous = [(Side::Left, 0.1, 0.3), (Side::Right, 0.3, 0.6)];
        let separated = [(Side::Left, 0.1, 0.25), (Side::Right, 0.3, 0.6)];
        for (reference, member) in [
            (&simultaneous[..], &separated[..]),
            (&separated[..], &simultaneous[..]),
        ] {
            let (manifest, manifest_input) = manifest();
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration(&manifest_input, 0.01, 100.0),
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input,
                    &evidence(reference, member),
                ),
                Err(FootCycleParameterizationError::NonMonotoneMapping)
            );
        }
    }

    #[test]
    fn incompatible_wrap_and_nonwrap_correspondence_stays_nonmonotone() {
        let left_wrap = [
            (Side::Left, 0.0, 0.3),
            (Side::Right, 0.2, 0.8),
            (Side::Left, 0.7, 1.0),
        ];
        let right_wrap = [
            (Side::Right, 0.0, 0.3),
            (Side::Left, 0.2, 0.8),
            (Side::Right, 0.7, 1.0),
        ];
        let (manifest, manifest_input) = manifest();
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration(&manifest_input, 0.01, 100.0),
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence(&left_wrap, &right_wrap),
            ),
            Err(FootCycleParameterizationError::NonMonotoneMapping)
        );
    }

    #[test]
    fn missing_extra_and_misplaced_markers_refuse() {
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let marker_cases = [
            Vec::new(),
            vec![(Side::Left, 0.15), (Side::Right, 0.65), (Side::Left, 0.18)],
            vec![(Side::Left, 0.65), (Side::Right, 0.15)],
        ];
        for markers in marker_cases {
            for invalid_index in 0..2 {
                let (manifest, manifest_input) = manifest();
                let declaration = declaration(&manifest_input, 0.01, 100.0);
                let (member_id, take_index, take_name) = if invalid_index == 0 {
                    ("reference", 0, "Take 0")
                } else {
                    ("member", 1, "Take 1")
                };
                let invalid = fragment_with_clip(
                    member_id,
                    "motions",
                    take_index,
                    take_name,
                    1.0,
                    &windows,
                    Some(&markers),
                    None,
                    vec![detector_extension("left_foot", "right_foot")],
                );
                let mut source = evidence(&windows, &windows);
                source[invalid_index].input = invalid.canonical_identity().unwrap();
                source[invalid_index].fragment = invalid;
                assert_eq!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input,
                        &source,
                    ),
                    Err(FootCycleParameterizationError::InvalidContactTopology)
                );
            }
        }

        for marker_as_window in [false, true] {
            for invalid_index in 0..2 {
                let (member_id, take_index, take_name) = if invalid_index == 0 {
                    ("reference", 0, "Take 0")
                } else {
                    ("member", 1, "Take 1")
                };
                let invalid_event = if marker_as_window {
                    ContactEventV1::window(
                        "invalid/marker-window",
                        ContactRoleV1::LeftFoot,
                        ContactPhaseV1::Marker,
                        ContactEventWindowV1::new(0.1, 0.2).unwrap(),
                        None,
                    )
                    .unwrap()
                } else {
                    ContactEventV1::point(
                        "invalid/begin-point",
                        ContactRoleV1::LeftFoot,
                        ContactPhaseV1::Begin,
                        0.15,
                        None,
                    )
                    .unwrap()
                };
                let invalid = fragment_with_events(
                    member_id,
                    "motions",
                    take_index,
                    take_name,
                    1.0,
                    vec![
                        ContactEventV1::window(
                            "support/left/0",
                            ContactRoleV1::LeftFoot,
                            ContactPhaseV1::Begin,
                            ContactEventWindowV1::new(0.1, 0.2).unwrap(),
                            None,
                        )
                        .unwrap(),
                        ContactEventV1::window(
                            "support/right/1",
                            ContactRoleV1::RightFoot,
                            ContactPhaseV1::Begin,
                            ContactEventWindowV1::new(0.6, 0.7).unwrap(),
                            None,
                        )
                        .unwrap(),
                        invalid_event,
                        ContactEventV1::point(
                            "marker/right/1",
                            ContactRoleV1::RightFoot,
                            ContactPhaseV1::Marker,
                            0.65,
                            None,
                        )
                        .unwrap(),
                    ],
                    vec![detector_extension("left_foot", "right_foot")],
                );
                let mut source = evidence(&windows, &windows);
                source[invalid_index].input = invalid.canonical_identity().unwrap();
                source[invalid_index].fragment = invalid;
                let (manifest, manifest_input) = manifest();
                assert_eq!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration(&manifest_input, 0.01, 100.0),
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input,
                        &source,
                    ),
                    Err(FootCycleParameterizationError::InvalidContactTopology)
                );
            }
        }
    }

    #[test]
    fn topology_count_mismatch_refuses_without_pairing_heuristics() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.01, 100.0);
        let reference = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let member = [
            (Side::Left, 0.05, 0.1),
            (Side::Right, 0.2, 0.3),
            (Side::Left, 0.5, 0.6),
            (Side::Right, 0.8, 0.9),
        ];
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence(&reference, &member),
            ),
            Err(FootCycleParameterizationError::TopologyMismatch)
        );
    }

    #[test]
    fn inclusive_slope_bounds_accept_exact_and_reject_successor() {
        let reference = [(Side::Left, 0.125, 0.25), (Side::Right, 0.5, 0.625)];
        let member = [(Side::Left, 0.25, 0.375), (Side::Right, 0.625, 0.75)];
        let (manifest, manifest_input) = manifest();
        let accepted = declaration(&manifest_input, 0.5, 1.5);
        assert!(
            plan_foot_cycle_parameterization_v1(
                &accepted,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input.clone(),
                &evidence(&reference, &member),
            )
            .is_ok()
        );
        let refused = declaration(&manifest_input, f64::from_bits(0.5_f64.to_bits() + 1), 1.5);
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &refused,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input.clone(),
                &evidence(&reference, &member),
            ),
            Err(FootCycleParameterizationError::SegmentSlopeOutOfRange)
        );
        let refused = declaration(&manifest_input, 0.5, f64::from_bits(1.5_f64.to_bits() - 1));
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &refused,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence(&reference, &member),
            ),
            Err(FootCycleParameterizationError::SegmentSlopeOutOfRange)
        );
    }

    #[test]
    fn exact_manifest_member_path_and_canonical_fragment_bindings_are_required() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let source = evidence(&windows, &windows);
        let mut cases = Vec::new();

        let mut stale = source.clone();
        stale[1].input = InputIdentity::from_bytes(b"noncanonical");
        cases.push((stale, FootCycleParameterizationError::NonCanonicalFragment));

        let mut stale_reference = source.clone();
        stale_reference[0].input = InputIdentity::from_bytes(b"noncanonical-reference");
        cases.push((
            stale_reference,
            FootCycleParameterizationError::NonCanonicalFragment,
        ));

        for member_index in 0..2 {
            let mut wrong_digest = source.clone();
            wrong_digest[member_index].input = InputIdentity::from_sha256_digest(
                [0; 32],
                wrong_digest[member_index].input.bytes(),
            );
            cases.push((
                wrong_digest,
                FootCycleParameterizationError::NonCanonicalFragment,
            ));

            let mut wrong_id = source.clone();
            wrong_id[member_index].id = id("com.example/wrong");
            cases.push((
                wrong_id,
                FootCycleParameterizationError::EvidenceMemberMismatch,
            ));

            let mut wrong_path = source.clone();
            wrong_path[member_index].contact_fragment_path = path("contacts/other.json");
            cases.push((
                wrong_path,
                FootCycleParameterizationError::EvidenceMemberMismatch,
            ));
        }

        let mut wrong_clip = source.clone();
        wrong_clip[1].fragment = fragment("reference", 0, &windows);
        wrong_clip[1].input = wrong_clip[1].fragment.canonical_identity().unwrap();
        cases.push((
            wrong_clip,
            FootCycleParameterizationError::FragmentClipMismatch,
        ));

        let mut wrong_reference = source.clone();
        wrong_reference[0].fragment = fragment("member", 1, &windows);
        wrong_reference[0].input = wrong_reference[0].fragment.canonical_identity().unwrap();
        cases.push((
            wrong_reference,
            FootCycleParameterizationError::FragmentClipMismatch,
        ));

        let wrong_reference_source = fragment_with_clip(
            "reference",
            "other-source",
            0,
            "Take 0",
            1.0,
            &windows,
            None,
            None,
            vec![detector_extension("left_foot", "right_foot")],
        );
        let mut wrong_reference_witness = source.clone();
        wrong_reference_witness[0].input = wrong_reference_source.canonical_identity().unwrap();
        wrong_reference_witness[0].fragment = wrong_reference_source;
        cases.push((
            wrong_reference_witness,
            FootCycleParameterizationError::FragmentClipMismatch,
        ));

        for fragment in [
            fragment_with_clip(
                "member",
                "other-source",
                1,
                "Take 1",
                1.0,
                &windows,
                None,
                None,
                vec![detector_extension("left_foot", "right_foot")],
            ),
            fragment_with_clip(
                "member",
                "motions",
                0,
                "Take 1",
                1.0,
                &windows,
                None,
                None,
                vec![detector_extension("left_foot", "right_foot")],
            ),
            fragment_with_clip(
                "member",
                "motions",
                1,
                "Other take",
                1.0,
                &windows,
                None,
                None,
                vec![detector_extension("left_foot", "right_foot")],
            ),
        ] {
            let mut wrong_witness = source.clone();
            wrong_witness[1].input = fragment.canonical_identity().unwrap();
            wrong_witness[1].fragment = fragment;
            cases.push((
                wrong_witness,
                FootCycleParameterizationError::FragmentClipMismatch,
            ));
        }

        for (evidence, expected) in cases {
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &evidence,
                ),
                Err(expected)
            );
        }
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                InputIdentity::from_bytes(b"stale-manifest"),
                &source,
            ),
            Err(FootCycleParameterizationError::ManifestMismatch)
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                InputIdentity::from_sha256_digest([0; 32], manifest_input.bytes()),
                &source,
            ),
            Err(FootCycleParameterizationError::ManifestMismatch)
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    FOOT_CYCLE_PARAMETERIZATION_V1_MAX_BYTES + 1,
                ),
                &manifest,
                manifest_input,
                &source,
            ),
            Err(FootCycleParameterizationError::ParameterizationTooLarge)
        );
    }

    #[test]
    fn manifest_collection_kind_and_exact_member_order_are_required() {
        let (manifest, manifest_input) = manifest();
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let source = evidence(&windows, &windows);
        let parameterization_input = || InputIdentity::from_bytes(b"parameterization");

        let mut missing_set = declaration(&manifest_input, 0.5, 2.0);
        missing_set.runtime_set_id = id("com.example/sets/missing");
        assert_eq!(
            validate_foot_cycle_manifest_binding_v1(&missing_set, &manifest, &manifest_input,),
            Err(FootCycleParameterizationError::RuntimeSetMismatch)
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &missing_set,
                parameterization_input(),
                &manifest,
                manifest_input.clone(),
                &source,
            ),
            Err(FootCycleParameterizationError::RuntimeSetMismatch)
        );

        let mut wrong_collection = declaration(&manifest_input, 0.5, 2.0);
        wrong_collection.manifest.collection_id = CollectionIdV1::new("com.other").unwrap();
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &wrong_collection,
                parameterization_input(),
                &manifest,
                manifest_input.clone(),
                &source,
            ),
            Err(FootCycleParameterizationError::ManifestMismatch)
        );

        let (wrong_kind, _) = manifest_with_set(
            CollectionRuntimeSetKindV1::SyncGroup,
            &["reference", "member"],
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration(&manifest_input, 0.5, 2.0),
                parameterization_input(),
                &wrong_kind,
                manifest_input.clone(),
                &source,
            ),
            Err(FootCycleParameterizationError::WrongRuntimeSetKind)
        );

        let (reordered, _) = manifest_with_set(
            CollectionRuntimeSetKindV1::GaitGroup,
            &["member", "reference"],
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration(&manifest_input, 0.5, 2.0),
                parameterization_input(),
                &reordered,
                manifest_input,
                &source,
            ),
            Err(FootCycleParameterizationError::MemberOrderMismatch)
        );
    }

    #[test]
    fn manifest_clip_lookup_accepts_late_maximum_long_prefix_member() {
        let prefix = "x".repeat(220);
        let names = (0..FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS)
            .map(|index| format!("{prefix}{index:04}"))
            .collect::<Vec<_>>();
        let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
        let (manifest, _) = manifest_for_members(
            CollectionRuntimeSetKindV1::GaitGroup,
            &name_refs,
            &name_refs[..2],
        );
        let last_index = FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS - 1;
        let fragment = fragment(
            &names[last_index],
            last_index as u32,
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
        );
        assert_eq!(
            validate_clip_witness(
                &manifest,
                &id(&format!("com.example/{}", names[last_index])),
                &fragment,
            ),
            Ok(())
        );
    }

    #[test]
    fn unsupported_or_malformed_detector_extension_refuses() {
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        for invalid_index in 0..2 {
            let (member, take_index) = if invalid_index == 0 {
                ("reference", 0)
            } else {
                ("member", 1)
            };
            let invalid = fragment_with_extensions(member, take_index, &windows, Vec::new());
            let mut source = evidence(&windows, &windows);
            source[invalid_index].input = invalid.canonical_identity().unwrap();
            source[invalid_index].fragment = invalid;
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &source,
                ),
                Err(FootCycleParameterizationError::UnsupportedContactExtension)
            );
        }

        let payload = json!({
            "algorithm": "stance-support-v1",
            "sampling": "metric-grid-longest-authored-channel",
            "max_frames": 1_000_000,
            "contact_height_m": 0.03,
            "roles": {"left": "left_foot", "right": "right_foot"},
        });
        let detector = detector_extension("left_foot", "right_foot");
        for extensions in [
            vec![ContactExtensionV1::new("urn:other:detector:1", 1, payload.clone()).unwrap()],
            vec![
                ContactExtensionV1::new(CONTACT_SUPPORT_DETECTOR_V1_ID, 2, payload.clone())
                    .unwrap(),
            ],
            vec![detector.clone(), detector],
        ] {
            for invalid_index in 0..2 {
                let (member, take_index) = if invalid_index == 0 {
                    ("reference", 0)
                } else {
                    ("member", 1)
                };
                let invalid =
                    fragment_with_extensions(member, take_index, &windows, extensions.clone());
                let mut source = evidence(&windows, &windows);
                source[invalid_index].input = invalid.canonical_identity().unwrap();
                source[invalid_index].fragment = invalid;
                assert_eq!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input.clone(),
                        &source,
                    ),
                    Err(FootCycleParameterizationError::UnsupportedContactExtension)
                );
            }
        }

        for member_index in 0..2 {
            let (member, take_index, take_name) = if member_index == 0 {
                ("reference", 0, "Take 0")
            } else {
                ("member", 1, "Take 1")
            };
            let toe_fragment = fragment_with_clip(
                member,
                "motions",
                take_index,
                take_name,
                1.0,
                &windows,
                None,
                Some([ContactRoleV1::LeftToe, ContactRoleV1::RightToe]),
                vec![detector_extension("left_toe", "right_toe")],
            );
            let mut source = evidence(&windows, &windows);
            source[member_index].input = toe_fragment.canonical_identity().unwrap();
            source[member_index].fragment = toe_fragment;
            assert!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &source,
                )
                .is_ok()
            );
        }

        let invalid_payloads = [
            json!({
                "algorithm": "other",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_foot"},
            }),
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "other",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_foot"},
            }),
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 999_999,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_foot"},
            }),
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": -0.01,
                "roles": {"left": "left_foot", "right": "right_foot"},
            }),
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_foot"},
                "extra": true,
            }),
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {
                    "left": "left_foot",
                    "right": "right_foot",
                    "extra": "unexpected",
                },
            }),
        ];
        for payload in invalid_payloads {
            for invalid_index in 0..2 {
                let (member, take_index) = if invalid_index == 0 {
                    ("reference", 0)
                } else {
                    ("member", 1)
                };
                let invalid = fragment_with_extensions(
                    member,
                    take_index,
                    &windows,
                    vec![detector_extension_with_payload(payload.clone())],
                );
                let mut source = evidence(&windows, &windows);
                source[invalid_index].input = invalid.canonical_identity().unwrap();
                source[invalid_index].fragment = invalid;
                assert_eq!(
                    plan_foot_cycle_parameterization_v1(
                        &declaration,
                        InputIdentity::from_bytes(b"parameterization"),
                        &manifest,
                        manifest_input.clone(),
                        &source,
                    ),
                    Err(FootCycleParameterizationError::InvalidDetectorProvenance)
                );
            }
        }

        for invalid_index in 0..2 {
            let (member, take_index) = if invalid_index == 0 {
                ("reference", 0)
            } else {
                ("member", 1)
            };
            let mismatched_roles = fragment_with_extensions(
                member,
                take_index,
                &windows,
                vec![detector_extension("left_foot", "right_toe")],
            );
            let mut source = evidence(&windows, &windows);
            source[invalid_index].input = mismatched_roles.canonical_identity().unwrap();
            source[invalid_index].fragment = mismatched_roles;
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &source,
                ),
                Err(FootCycleParameterizationError::InvalidDetectorProvenance)
            );
        }
    }

    #[test]
    fn mixed_detector_thresholds_refuse_in_either_member_order() {
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let threshold_successor = f64::from_bits(0.03_f64.to_bits() + 1);

        let mut common_non_default = evidence(&windows, &windows);
        for (member_index, (member, take_index)) in
            [("reference", 0), ("member", 1)].into_iter().enumerate()
        {
            let changed = fragment_with_extensions(
                member,
                take_index,
                &windows,
                vec![detector_extension_with_height(
                    "left_foot",
                    "right_foot",
                    threshold_successor,
                )],
            );
            common_non_default[member_index].input = changed.canonical_identity().unwrap();
            common_non_default[member_index].root_motion = measured_root(&changed, 0.0, 0.0);
            common_non_default[member_index].fragment = changed;
        }
        assert!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input.clone(),
                &common_non_default,
            )
            .is_ok()
        );

        for changed_index in 0..2 {
            let (member, take_index) = if changed_index == 0 {
                ("reference", 0)
            } else {
                ("member", 1)
            };
            let changed = fragment_with_extensions(
                member,
                take_index,
                &windows,
                vec![detector_extension_with_height(
                    "left_foot",
                    "right_foot",
                    threshold_successor,
                )],
            );
            let mut source = evidence(&windows, &windows);
            source[changed_index].input = changed.canonical_identity().unwrap();
            source[changed_index].fragment = changed;
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input.clone(),
                    &source,
                ),
                Err(FootCycleParameterizationError::DetectorPolicyMismatch)
            );
        }
    }

    #[test]
    fn stance_extension_time_warp_handler_revalidates_closed_payload() {
        let extension = detector_extension("left_foot", "right_foot");
        let operation = ContactTransformOperationV1::time_warp(
            1.0,
            vec![
                ContactTimeWarpControlPointV1::new(0.0, 0.0),
                ContactTimeWarpControlPointV1::new(1.0, 1.0),
            ],
        );
        assert_eq!(
            transform_contact_support_detector_extension_time_warp_v1(&extension, &operation)
                .unwrap(),
            extension
        );
        for (left, right) in [
            ("left_toe", "right_toe"),
            ("left_toe", "right_foot"),
            ("left_foot", "right_toe"),
        ] {
            let extension = detector_extension(left, right);
            assert_eq!(
                transform_contact_support_detector_extension_time_warp_v1(&extension, &operation,)
                    .unwrap(),
                extension
            );
        }
        let negative_zero = detector_extension_with_payload(json!({
            "algorithm": "stance-support-v1",
            "sampling": "metric-grid-longest-authored-channel",
            "max_frames": 1_000_000,
            "contact_height_m": -0.0,
            "roles": {"left": "left_foot", "right": "right_foot"},
        }));
        let reconstructed =
            transform_contact_support_detector_extension_time_warp_v1(&negative_zero, &operation)
                .unwrap();
        assert_eq!(
            reconstructed
                .payload()
                .get("contact_height_m")
                .and_then(serde_json::Value::as_f64)
                .unwrap()
                .to_bits(),
            0.0_f64.to_bits()
        );
        assert_eq!(
            transform_contact_support_detector_extension_time_warp_v1(
                &extension,
                &ContactTransformOperationV1::resample(),
            ),
            Err(FootCycleParameterizationError::UnsupportedContactExtension)
        );
        let opaque_lookalike = detector_extension_with_payload(json!({
            "algorithm": "stance-support-v1",
            "sampling": "metric-grid-longest-authored-channel",
            "max_frames": 1_000_000,
            "contact_height_m": 0.03,
            "roles": {"left": "left_foot", "right": "right_foot"},
            "opaque": true,
        }));
        assert_eq!(
            transform_contact_support_detector_extension_time_warp_v1(
                &opaque_lookalike,
                &operation,
            ),
            Err(FootCycleParameterizationError::InvalidDetectorProvenance)
        );
    }

    #[test]
    fn control_point_cap_accepts_exact_and_rejects_first_excess() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS, 4_096);
        fn boundaries(count: usize) -> Vec<Boundary> {
            (0..count)
                .map(|index| Boundary {
                    kind: BoundaryKind {
                        side: if index % 2 == 0 {
                            Side::Left
                        } else {
                            Side::Right
                        },
                        edge: if index % 2 == 0 {
                            Edge::Onset
                        } else {
                            Edge::Release
                        },
                    },
                    time: (index + 1) as f64 / (count + 1) as f64,
                })
                .collect()
        }

        let exact = boundaries(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS - 2);
        assert_eq!(
            build_control_points(&exact, &exact, 1.0, 1.0)
                .unwrap()
                .len(),
            FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS
        );

        let excess = boundaries(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS - 1);
        assert_eq!(
            build_control_points(&excess, &excess, 1.0, 1.0),
            Err(FootCycleParameterizationError::TooManyControlPoints {
                found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS + 1,
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS,
            })
        );
    }

    #[test]
    fn planner_enforces_control_point_cap_on_supplied_ring() {
        let windows = alternating_windows(2_048);
        let (manifest, manifest_input) = manifest();
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration(&manifest_input, 0.5, 2.0),
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence(&windows, &windows),
            ),
            Err(FootCycleParameterizationError::TooManyControlPoints {
                found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS + 2,
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS,
            })
        );
    }

    #[test]
    fn aggregate_contact_event_budget_accepts_exact_and_stops_at_first_excess() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS, 16_384);
        assert_eq!(
            validate_aggregate_contact_event_budget([
                FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS - 1,
                1,
            ]),
            Ok(())
        );
        assert_eq!(
            validate_aggregate_contact_event_budget([
                FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS,
                usize::MAX,
            ]),
            Err(FootCycleParameterizationError::TooManyContactEvents {
                found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS + 1,
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS,
            })
        );
    }

    #[test]
    fn aggregate_fragment_byte_budget_accepts_exact_and_stops_at_first_excess() {
        assert_eq!(
            FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
            32 * 1024 * 1024
        );
        assert_eq!(
            validate_aggregate_contact_fragment_byte_budget([
                FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES - 1,
                1,
            ]),
            Ok(())
        );
        assert_eq!(
            validate_aggregate_contact_fragment_byte_budget([
                FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
                u64::MAX,
            ]),
            Err(
                FootCycleParameterizationError::TooManyContactFragmentBytes {
                    found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES + 1,
                    max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
                }
            )
        );
    }

    #[test]
    fn planner_preflights_fragment_byte_budget_before_canonicalization() {
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let mut source = evidence(&windows, &windows);
        let second_bytes = source[1].input.bytes();
        source[0].input = InputIdentity::from_sha256_digest(
            [0; 32],
            FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES - second_bytes,
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input.clone(),
                &source,
            ),
            Err(FootCycleParameterizationError::NonCanonicalFragment)
        );

        source[0].input = InputIdentity::from_sha256_digest(
            [0; 32],
            FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES - second_bytes + 1,
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &source,
            ),
            Err(
                FootCycleParameterizationError::TooManyContactFragmentBytes {
                    found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES + 1,
                    max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
                }
            )
        );
    }

    #[test]
    fn planner_preflights_aggregate_contact_events_before_topology_retention() {
        let names = ["reference", "member-1", "member-2", "member-3", "member-4"];
        let (manifest, manifest_input) =
            manifest_for_members(CollectionRuntimeSetKindV1::GaitGroup, &names, &names);
        let declaration = FootCycleParameterizationV1::new(
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                manifest_input.clone(),
            )
            .unwrap(),
            id("com.example/sets/walk"),
            id("com.example/reference"),
            path("generated/aligned"),
            0.5,
            2.0,
            proof_policy(),
            names
                .iter()
                .map(|name| {
                    FootCycleParameterizationMemberV1::new(
                        id(&format!("com.example/{name}")),
                        path(&format!("contacts/{name}.json")),
                    )
                })
                .collect(),
        )
        .unwrap();
        let windows = alternating_windows(2_048);
        let oversized_fragment = fragment("reference", 0, &windows);
        let fragment_input = oversized_fragment.canonical_identity().unwrap();
        let evidence = names
            .iter()
            .map(|name| {
                FootCycleMemberEvidenceV1::new(
                    id(&format!("com.example/{name}")),
                    path(&format!("contacts/{name}.json")),
                    fragment_input.clone(),
                    oversized_fragment.clone(),
                    measured_root(&oversized_fragment, 0.0, 0.0),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence,
            ),
            Err(FootCycleParameterizationError::TooManyContactEvents {
                found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS + 1,
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_EVENTS,
            })
        );
    }

    #[test]
    fn proof_policy_requires_complete_finite_values_at_exact_inclusive_boundaries() {
        let lower = FootCycleProofPolicyV1::new(0.0, 0.0, 0.0).unwrap();
        assert_eq!(lower.max_gait_phase_spread(), 0.0);
        assert_eq!(lower.min_lr_amplitude_m(), 0.0);
        assert_eq!(lower.max_contact_boundary_phase_error(), 0.0);

        let upper = FootCycleProofPolicyV1::new(0.5, 0.05, 0.5).unwrap();
        assert_eq!(upper.max_gait_phase_spread(), 0.5);
        assert_eq!(upper.min_lr_amplitude_m(), 0.05);
        assert_eq!(upper.max_contact_boundary_phase_error(), 0.5);
        assert_eq!(
            serde_json::to_value(&upper).unwrap(),
            json!({
                "max_gait_phase_spread": 0.5,
                "min_lr_amplitude_m": 0.05,
                "max_contact_boundary_phase_error": 0.5,
            })
        );

        let above_half = f64::from_bits(0.5_f64.to_bits() + 1);
        for result in [
            FootCycleProofPolicyV1::new(above_half, 0.0, 0.0),
            FootCycleProofPolicyV1::new(0.0, -f64::MIN_POSITIVE, 0.0),
            FootCycleProofPolicyV1::new(0.0, 0.0, above_half),
            FootCycleProofPolicyV1::new(f64::NAN, 0.0, 0.0),
            FootCycleProofPolicyV1::new(0.0, f64::INFINITY, 0.0),
            FootCycleProofPolicyV1::new(0.0, 0.0, f64::NEG_INFINITY),
        ] {
            assert_eq!(
                result,
                Err(FootCycleParameterizationError::InvalidProofPolicy)
            );
        }
    }

    #[test]
    fn plan_retains_identity_bound_proof_policy() {
        let (manifest, manifest_input) = manifest();
        let distinct = FootCycleProofPolicyV1::new(0.321, 0.123, 0.234).unwrap();
        let declaration = declaration_with_proof(&manifest_input, 0.5, 2.0, distinct.clone());
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let parameterization_input = InputIdentity::from_bytes(b"parameterization-with-proof");
        let plan = plan_foot_cycle_parameterization_v1(
            &declaration,
            parameterization_input.clone(),
            &manifest,
            manifest_input,
            &evidence(&windows, &windows),
        )
        .unwrap();
        assert_eq!(plan.parameterization_input(), &parameterization_input);
        assert_eq!(plan.proof(), &distinct);
        assert_eq!(plan.proof(), declaration.proof());
    }

    #[test]
    fn declaration_rejects_invalid_slopes_duplicates_and_path_collisions() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_SLOPE, 1_000_000.0);
        let (_, manifest_input) = manifest();
        assert!(
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES,
                ),
            )
            .is_ok()
        );
        assert_eq!(
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                InputIdentity::from_sha256_digest(
                    [0; 32],
                    COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES + 1,
                ),
            ),
            Err(FootCycleParameterizationError::ManifestTooLarge)
        );
        for (minimum, maximum) in [
            (0.0, 1.0),
            (-1.0, 1.0),
            (f64::NAN, 1.0),
            (0.5, f64::NAN),
            (f64::NEG_INFINITY, 1.0),
            (0.5, f64::INFINITY),
            (1.0 + f64::EPSILON, 2.0),
            (0.5, 1.0 - f64::EPSILON),
            (2.0, 1.0),
            (1.0, FOOT_CYCLE_PARAMETERIZATION_V1_MAX_SLOPE * 2.0),
        ] {
            assert_eq!(
                FootCycleParameterizationV1::new(
                    FootCycleManifestBindingV1::new(
                        CollectionIdV1::new("com.example").unwrap(),
                        manifest_input.clone(),
                    )
                    .unwrap(),
                    id("com.example/sets/walk"),
                    id("com.example/reference"),
                    path("generated/aligned"),
                    minimum,
                    maximum,
                    proof_policy(),
                    vec![
                        FootCycleParameterizationMemberV1::new(
                            id("com.example/reference"),
                            path("contacts/reference.json"),
                        ),
                        FootCycleParameterizationMemberV1::new(
                            id("com.example/member"),
                            path("contacts/member.json"),
                        ),
                    ],
                ),
                Err(FootCycleParameterizationError::InvalidSlopeBounds)
            );
        }

        let binding = || {
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                manifest_input.clone(),
            )
            .unwrap()
        };
        for members in [
            Vec::new(),
            vec![FootCycleParameterizationMemberV1::new(
                id("com.example/reference"),
                path("contacts/reference.json"),
            )],
        ] {
            let found = members.len();
            assert_eq!(
                FootCycleParameterizationV1::new(
                    binding(),
                    id("com.example/sets/walk"),
                    id("com.example/reference"),
                    path("generated/aligned"),
                    0.5,
                    2.0,
                    proof_policy(),
                    members,
                ),
                Err(FootCycleParameterizationError::TooFewMembers { found })
            );
        }
        assert!(matches!(
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/reference"),
                path("generated/aligned"),
                0.5,
                2.0,
                proof_policy(),
                vec![
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/reference"),
                        path("contacts/reference.json"),
                    ),
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/reference"),
                        path("contacts/other.json"),
                    ),
                ],
            ),
            Err(FootCycleParameterizationError::DuplicateMember { .. })
        ));
        assert_eq!(
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/reference"),
                path("generated/aligned"),
                0.5,
                2.0,
                proof_policy(),
                vec![
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/reference"),
                        path("contacts/shared.json"),
                    ),
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/member"),
                        path("contacts/shared.json"),
                    ),
                ],
            ),
            Err(FootCycleParameterizationError::DuplicateFragmentPath)
        );
        assert_eq!(
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/reference"),
                path("contacts/member.json"),
                0.5,
                2.0,
                proof_policy(),
                vec![
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/reference"),
                        path("contacts/reference.json"),
                    ),
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/member"),
                        path("contacts/member.json"),
                    ),
                ],
            ),
            Err(FootCycleParameterizationError::OutputPathCollision)
        );
        assert_eq!(
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/missing"),
                path("generated/aligned"),
                0.5,
                2.0,
                proof_policy(),
                vec![
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/reference"),
                        path("contacts/reference.json"),
                    ),
                    FootCycleParameterizationMemberV1::new(
                        id("com.example/member"),
                        path("contacts/member.json"),
                    ),
                ],
            ),
            Err(FootCycleParameterizationError::MissingReferenceMember)
        );
    }

    #[test]
    fn declaration_member_cap_accepts_exact_and_rejects_n_plus_one() {
        assert_eq!(FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS, 4_096);
        let mut members = (0..FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS)
            .map(|index| {
                FootCycleParameterizationMemberV1::new(
                    id(&format!("com.example/member-{index}")),
                    path(&format!("contacts/member-{index}.json")),
                )
            })
            .collect::<Vec<_>>();
        let binding = || {
            FootCycleManifestBindingV1::new(
                CollectionIdV1::new("com.example").unwrap(),
                InputIdentity::from_bytes(b"manifest"),
            )
            .unwrap()
        };
        let construct = |members| {
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/member-0"),
                path("generated/aligned"),
                0.5,
                2.0,
                proof_policy(),
                members,
            )
        };

        assert!(construct(members.clone()).is_ok());
        let index = FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS;
        members.push(FootCycleParameterizationMemberV1::new(
            id(&format!("com.example/member-{index}")),
            path(&format!("contacts/member-{index}.json")),
        ));
        assert_eq!(
            construct(members),
            Err(FootCycleParameterizationError::TooManyMembers {
                found: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS + 1,
                max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_MEMBERS,
            })
        );
    }
}
