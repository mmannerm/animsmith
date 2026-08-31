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
    ContactTransformBindingV1, ContactTransformOperationV1, DependencyResourceKeyV1, InputIdentity,
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
/// Maximum source-to-output control points, shared with contact-transform V1.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS: usize =
    crate::CONTACT_TRANSFORM_RESULT_V1_MAX_CONTROL_POINTS;
/// Largest accepted finite segment-slope bound.
pub const FOOT_CYCLE_PARAMETERIZATION_V1_MAX_SLOPE: f64 = 1_000_000.0;
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
    members: Vec<FootCycleParameterizationMemberV1>,
}

impl FootCycleParameterizationV1 {
    /// Construct one strict declaration in authored member order.
    ///
    /// # Errors
    ///
    /// Returns [`FootCycleParameterizationError`] when membership, paths, or
    /// slope bounds violate the frozen V1 declaration contract.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        manifest: FootCycleManifestBindingV1,
        runtime_set_id: CollectionLogicalIdV1,
        reference_member: CollectionLogicalIdV1,
        output_directory: DependencyResourceKeyV1,
        minimum_segment_slope: f64,
        maximum_segment_slope: f64,
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

    /// Members in exact declaration and runtime-set order.
    pub fn members(&self) -> &[FootCycleParameterizationMemberV1] {
        &self.members
    }
}

/// One already-read contact fragment bound to its exact source bytes and path.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCycleMemberEvidenceV1 {
    id: CollectionLogicalIdV1,
    contact_fragment_path: DependencyResourceKeyV1,
    input: InputIdentity,
    fragment: ContactFragmentV1,
}

impl FootCycleMemberEvidenceV1 {
    /// Bind one decoded fragment to the exact bytes and declaration path used.
    pub fn new(
        id: CollectionLogicalIdV1,
        contact_fragment_path: DependencyResourceKeyV1,
        input: InputIdentity,
        fragment: ContactFragmentV1,
    ) -> Self {
        Self {
            id,
            contact_fragment_path,
            input,
            fragment,
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
}

/// One member's exact contact binding and future time-warp operation.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCycleMemberPlanV1 {
    id: CollectionLogicalIdV1,
    input: ContactTransformBindingV1,
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

    /// Validated time-warp operation preserving the source duration.
    pub const fn operation(&self) -> &ContactTransformOperationV1 {
        &self.operation
    }
}

/// Complete pure plan for one declared collection ring.
#[derive(Debug, Clone, PartialEq)]
pub struct FootCyclePlanV1 {
    parameterization_input: InputIdentity,
    manifest_input: InputIdentity,
    runtime_set_id: CollectionLogicalIdV1,
    reference_member: CollectionLogicalIdV1,
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
    /// Slope bounds were not finite, positive, ordered, and bounded.
    #[error("segment-slope bounds must be finite, positive, ordered, and within V1")]
    InvalidSlopeBounds,
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
    validate_manifest_binding(parameterization, manifest, &manifest_input)?;
    if evidence.len() != parameterization.members.len() {
        return Err(FootCycleParameterizationError::EvidenceCountMismatch);
    }

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
            .canonical_identity()
            .map_err(|_| FootCycleParameterizationError::NonCanonicalFragment)?;
        if canonical != evidence.input {
            return Err(FootCycleParameterizationError::NonCanonicalFragment);
        }
        validate_clip_witness(manifest, &declaration.id, &evidence.fragment)?;
        let topology = topology(&evidence.fragment)?;
        topologies.push(MemberTopology {
            evidence_index: index,
            signature: topology.0,
            rotated_boundaries: topology.1,
        });
    }

    let reference_index = parameterization
        .members
        .iter()
        .position(|member| member.id == parameterization.reference_member)
        .ok_or(FootCycleParameterizationError::MissingReferenceMember)?;
    let reference = &topologies[reference_index];
    for member in &topologies {
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
        members: plans,
    })
}

fn validate_manifest_binding(
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
        .iter()
        .find(|clip| clip.id() == member)
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
) -> Result<(Vec<BoundaryKind>, Vec<Boundary>), FootCycleParameterizationError> {
    let roles = validate_detector_extension(fragment)?;
    let mut windows = Vec::new();
    let mut markers = Vec::new();
    for event in fragment.events() {
        let side = event_side(event.role(), roles)
            .ok_or(FootCycleParameterizationError::InvalidDetectorProvenance)?;
        match (event.phase(), event.kind()) {
            (ContactPhaseV1::Begin, ContactEventKindV1::Window(window))
                if window.start() < window.end() =>
            {
                windows.push(SupportWindow {
                    side,
                    start: window.start(),
                    end: window.end(),
                });
            }
            (ContactPhaseV1::Marker, ContactEventKindV1::Point(time)) => {
                markers.push((side, time));
            }
            _ => return Err(FootCycleParameterizationError::InvalidContactTopology),
        }
    }
    if windows.is_empty() {
        return Err(FootCycleParameterizationError::InvalidContactTopology);
    }
    windows.sort_by(|left, right| left.start.total_cmp(&right.start));
    let left_count = windows
        .iter()
        .filter(|window| window.side == Side::Left)
        .count();
    let right_count = windows.len() - left_count;
    if left_count == 0 || left_count != right_count {
        return Err(FootCycleParameterizationError::InvalidContactTopology);
    }
    for pair in windows.windows(2) {
        if pair[0].end >= pair[1].start || pair[0].side == pair[1].side {
            return Err(FootCycleParameterizationError::InvalidContactTopology);
        }
    }
    if windows
        .first()
        .zip(windows.last())
        .is_none_or(|(first, last)| {
            first.side == last.side || first.start == 0.0 && last.end == 1.0
        })
    {
        return Err(FootCycleParameterizationError::InvalidContactTopology);
    }
    for window in &windows {
        if markers
            .iter()
            .filter(|(side, time)| {
                *side == window.side && window.start <= *time && *time <= window.end
            })
            .count()
            != 1
        {
            return Err(FootCycleParameterizationError::InvalidContactTopology);
        }
    }
    for (side, time) in &markers {
        if windows
            .iter()
            .filter(|window| window.side == *side && window.start <= *time && *time <= window.end)
            .count()
            != 1
        {
            return Err(FootCycleParameterizationError::InvalidContactTopology);
        }
    }

    let origin = windows
        .iter()
        .position(|window| window.side == Side::Left)
        .ok_or(FootCycleParameterizationError::InvalidContactTopology)?;
    let mut signature = Vec::with_capacity(windows.len() * 2);
    let mut boundaries = Vec::with_capacity(windows.len() * 2);
    for window in windows.iter().cycle().skip(origin).take(windows.len()) {
        for (edge, time) in [(Edge::Onset, window.start), (Edge::Release, window.end)] {
            let kind = BoundaryKind {
                side: window.side,
                edge,
            };
            signature.push(kind);
            boundaries.push(Boundary { kind, time });
        }
    }
    Ok((signature, boundaries))
}

fn validate_detector_extension(
    fragment: &ContactFragmentV1,
) -> Result<[ContactRoleV1; 2], FootCycleParameterizationError> {
    let [extension] = fragment.extensions() else {
        return Err(FootCycleParameterizationError::UnsupportedContactExtension);
    };
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
    if payload.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || payload.get("algorithm").and_then(serde_json::Value::as_str)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_ALGORITHM)
        || payload.get("sampling").and_then(serde_json::Value::as_str)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_SAMPLING)
        || payload
            .get("max_frames")
            .and_then(serde_json::Value::as_u64)
            != Some(CONTACT_SUPPORT_DETECTOR_V1_MAX_FRAMES)
        || payload
            .get("contact_height_m")
            .and_then(serde_json::Value::as_f64)
            .is_none_or(|value| !value.is_finite() || value < 0.0)
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
    Ok([left, right])
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
    let needed = source
        .iter()
        .zip(reference)
        .filter(|(source, reference)| {
            !((source.time == 0.0 && reference.time == 0.0)
                || (source.time == 1.0 && reference.time == 1.0))
        })
        .try_fold(2_usize, |count, _| count.checked_add(1))
        .ok_or(FootCycleParameterizationError::TooManyControlPoints {
            found: usize::MAX,
            max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS,
        })?;
    if needed > FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS {
        return Err(FootCycleParameterizationError::TooManyControlPoints {
            found: needed,
            max: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTROL_POINTS,
        });
    }

    let mut pairs = source
        .iter()
        .zip(reference)
        .map(|(source, reference)| (source.time, reference.time))
        .collect::<Vec<_>>();
    pairs.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut points = Vec::with_capacity(needed);
    points.push((0.0, 0.0));
    for (input, output) in pairs {
        if input == 0.0 && output == 0.0 || input == 1.0 && output == 1.0 {
            continue;
        }
        points.push((canonical_zero(input), canonical_zero(output)));
    }
    points.push((1.0, 1.0));
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
        let collection = CollectionIdV1::new("com.example").unwrap();
        let source = CollectionSourceKeyV1::new("motions").unwrap();
        let clips = ["reference", "member"]
            .into_iter()
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
        let set_members = clips.iter().map(|clip| clip.id().clone()).collect();
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
                CollectionRuntimeSetKindV1::GaitGroup,
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

    fn detector_extension(left: &str, right: &str) -> ContactExtensionV1 {
        ContactExtensionV1::new(
            CONTACT_SUPPORT_DETECTOR_V1_ID,
            1,
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": left, "right": right},
            }),
        )
        .unwrap()
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
        let artifact = InputIdentity::from_bytes(member.as_bytes());
        let closure =
            DependencyClosureBuilderV1::new(artifact.clone(), SourceSetCoverageV1::complete(), 0)
                .finish()
                .unwrap();
        let mut events = Vec::with_capacity(windows.len() * 2);
        for (index, &(side, start, end)) in windows.iter().enumerate() {
            let (role, label) = match side {
                Side::Left => (ContactRoleV1::LeftFoot, "left_foot"),
                Side::Right => (ContactRoleV1::RightFoot, "right_foot"),
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
            events.push(
                ContactEventV1::point(
                    format!("marker/{label}/{index}"),
                    role,
                    ContactPhaseV1::Marker,
                    (start + end) / 2.0,
                    None,
                )
                .unwrap(),
            );
        }
        ContactFragmentV1::new(
            ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
            artifact,
            closure.identity().unwrap().clone(),
            ContactClipReferenceV1::collection(
                format!("com.example/{member}"),
                "motions",
                take_index,
                format!("Take {take_index}"),
            )
            .unwrap(),
            1.0,
            events,
            extensions,
        )
        .unwrap()
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
            FootCycleMemberEvidenceV1::new(
                id(&format!("com.example/{member}")),
                path(source),
                fragment.canonical_identity().unwrap(),
                fragment,
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

    #[test]
    fn planner_maps_exact_boundaries_to_reference_phases_and_is_deterministic() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let evidence = evidence(
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
            &[(Side::Left, 0.2, 0.3), (Side::Right, 0.7, 0.8)],
        );
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
            first.members()[1].operation().output_duration_s(),
            Some(1.0)
        );
        assert_eq!(first.members()[1].input().fragment(), evidence[1].input());
    }

    #[test]
    fn cyclic_topology_is_compared_from_each_first_left_onset() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let evidence = evidence(
            &[(Side::Right, 0.1, 0.2), (Side::Left, 0.6, 0.7)],
            &[(Side::Right, 0.15, 0.25), (Side::Left, 0.65, 0.75)],
        );
        assert!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence,
            )
            .is_ok()
        );
    }

    #[test]
    fn phase_rotation_that_cannot_preserve_endpoints_refuses() {
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.01, 100.0);
        let evidence = evidence(
            &[(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)],
            &[(Side::Right, 0.1, 0.2), (Side::Left, 0.6, 0.7)],
        );
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &evidence,
            ),
            Err(FootCycleParameterizationError::NonMonotoneMapping)
        );
    }

    #[test]
    fn overlapping_simultaneous_repeated_and_missing_runs_refuse() {
        let reference = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        for invalid in [
            vec![(Side::Left, 0.1, 0.4), (Side::Right, 0.3, 0.6)],
            vec![(Side::Left, 0.1, 0.3), (Side::Right, 0.3, 0.6)],
            vec![
                (Side::Left, 0.1, 0.2),
                (Side::Left, 0.3, 0.4),
                (Side::Right, 0.6, 0.7),
                (Side::Right, 0.8, 0.9),
            ],
            vec![(Side::Left, 0.0, 0.2), (Side::Right, 0.7, 1.0)],
            vec![(Side::Left, 0.1, 0.2)],
        ] {
            let (manifest, manifest_input) = manifest();
            let declaration = declaration(&manifest_input, 0.01, 100.0);
            assert_eq!(
                plan_foot_cycle_parameterization_v1(
                    &declaration,
                    InputIdentity::from_bytes(b"parameterization"),
                    &manifest,
                    manifest_input,
                    &evidence(&reference, &invalid),
                ),
                Err(FootCycleParameterizationError::InvalidContactTopology)
            );
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

        let mut wrong_path = source.clone();
        wrong_path[1].contact_fragment_path = path("contacts/other.json");
        cases.push((
            wrong_path,
            FootCycleParameterizationError::EvidenceMemberMismatch,
        ));

        let mut wrong_clip = source.clone();
        wrong_clip[1].fragment = fragment("reference", 0, &windows);
        wrong_clip[1].input = wrong_clip[1].fragment.canonical_identity().unwrap();
        cases.push((
            wrong_clip,
            FootCycleParameterizationError::FragmentClipMismatch,
        ));

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
    fn unsupported_or_malformed_detector_extension_refuses() {
        let windows = [(Side::Left, 0.1, 0.2), (Side::Right, 0.6, 0.7)];
        let (manifest, manifest_input) = manifest();
        let declaration = declaration(&manifest_input, 0.5, 2.0);
        let mut source = evidence(&windows, &windows);
        source[1].fragment = fragment_with_extensions("member", 1, &windows, Vec::new());
        source[1].input = source[1].fragment.canonical_identity().unwrap();
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

        let mut source = evidence(&windows, &windows);
        source[1].fragment = fragment_with_extensions(
            "member",
            1,
            &windows,
            vec![detector_extension("left_foot", "right_toe")],
        );
        source[1].input = source[1].fragment.canonical_identity().unwrap();
        assert_eq!(
            plan_foot_cycle_parameterization_v1(
                &declaration,
                InputIdentity::from_bytes(b"parameterization"),
                &manifest,
                manifest_input,
                &source,
            ),
            Err(FootCycleParameterizationError::InvalidDetectorProvenance)
        );
    }

    #[test]
    fn control_point_cap_accepts_exact_and_rejects_first_excess() {
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
    fn declaration_rejects_invalid_slopes_duplicates_and_path_collisions() {
        let (_, manifest_input) = manifest();
        for (minimum, maximum) in [
            (0.0, 1.0),
            (-1.0, 1.0),
            (f64::NAN, 1.0),
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
        assert!(matches!(
            FootCycleParameterizationV1::new(
                binding(),
                id("com.example/sets/walk"),
                id("com.example/reference"),
                path("generated/aligned"),
                0.5,
                2.0,
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
}
