//! Final collection-scoped foot-cycle producer.
//!
//! Preparation, serialization/readback proof, evidence encoding, and the one
//! generation-directory publication are deliberately separate phases.  No
//! filesystem write occurs until every member and every exact output byte is
//! available and the shared retained/staged budget has passed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use animsmith_core::{
    DependencyClosureIdentityV1, FootCycleProofPolicyV1, InputIdentity, ToolInfo,
};
use serde::{Deserialize, Serialize};

use crate::Format;
use crate::foot_cycle_proof::{
    FootCycleMemberProofFactsV1, FootCycleProofError, ProvedFootCycleCollectionV1,
    serialize_and_prove_foot_cycle_v1,
};
use crate::foot_cycle_source_prep::{
    FootCycleSourcePrepError, FootCycleSourcePrepKind, PreparedFootCycleCollectionV1,
    prepare_foot_cycle_parameterization_v1,
};
use crate::producer::{self, Command, Kind, Rejection, Stage};
use crate::publish::{BoundedSerializationError, serialize_record_bounded};
use crate::publish::{GenerationFile, GenerationPublicationLimits, emit, publish_generation};

pub(crate) const MEMBER_EVIDENCE_V1_ID: &str = "urn:animsmith:schema:foot-cycle-member-evidence:1";
pub(crate) const AGGREGATE_EVIDENCE_V1_ID: &str =
    "urn:animsmith:schema:foot-cycle-aggregate-evidence:1";
const EVIDENCE_VERSION: u32 = 1;
const COMMAND: &str = "collection-transform-foot-cycle";
const MAX_SHARED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MEMBER_EVIDENCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_AGGREGATE_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ALIAS_COMPONENTS: u64 = 4;
const MAX_ALIAS_COMPONENT_BYTES: u64 = 64;

#[derive(Clone, Copy)]
struct EncodingLimits {
    shared_bytes: u64,
    member_evidence_bytes: usize,
    aggregate_evidence_bytes: usize,
    aggregate_convergence_iterations: usize,
}

const ENCODING_LIMITS: EncodingLimits = EncodingLimits {
    shared_bytes: MAX_SHARED_BYTES,
    member_evidence_bytes: MAX_MEMBER_EVIDENCE_BYTES,
    aggregate_evidence_bytes: MAX_AGGREGATE_EVIDENCE_BYTES,
    aggregate_convergence_iterations: 32,
};

#[derive(Clone, Copy)]
struct Finite(f64);

impl Serialize for Finite {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if !self.0.is_finite() {
            return Err(serde::ser::Error::custom(
                "non-finite foot-cycle evidence value",
            ));
        }
        serializer.serialize_f64(self.0)
    }
}

#[derive(Serialize)]
struct PathsRecord<'a> {
    artifact: &'a str,
    contact_fragment: &'a str,
    evidence: &'a str,
}

#[derive(Serialize)]
struct SourceBindingRecord<'a> {
    source_key: &'a str,
    artifact: &'a InputIdentity,
    dependency_closure_identity: &'a DependencyClosureIdentityV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<&'a InputIdentity>,
}

#[derive(Serialize)]
struct OutputBindingRecord<'a> {
    artifact: &'a InputIdentity,
    dependency_closure_identity: &'a DependencyClosureIdentityV1,
    contact_fragment: &'a InputIdentity,
    independently_detected_contact_fragment: &'a InputIdentity,
}

#[derive(Serialize)]
struct ProofPolicyRecord {
    max_gait_phase_spread: Finite,
    min_lr_amplitude_m: Finite,
    max_contact_boundary_phase_error: Finite,
}

impl From<&FootCycleProofPolicyV1> for ProofPolicyRecord {
    fn from(value: &FootCycleProofPolicyV1) -> Self {
        Self {
            max_gait_phase_spread: Finite(value.max_gait_phase_spread()),
            min_lr_amplitude_m: Finite(value.min_lr_amplitude_m()),
            max_contact_boundary_phase_error: Finite(value.max_contact_boundary_phase_error()),
        }
    }
}

#[derive(Serialize)]
struct ProofFactsRecord {
    duration_s: Finite,
    gait_phase: Finite,
    lr_amplitude_m: Finite,
    max_contact_boundary_phase_error: Finite,
    root_endpoint_displacement_x_m: Finite,
    root_endpoint_displacement_z_m: Finite,
    root_accumulated_yaw_deg: Finite,
    max_loop_position_delta_m: Finite,
    max_loop_rotation_delta_deg: Finite,
    max_loop_velocity_delta_mps: Finite,
    max_loop_angular_velocity_delta_degps: Finite,
}

impl From<&FootCycleMemberProofFactsV1> for ProofFactsRecord {
    fn from(value: &FootCycleMemberProofFactsV1) -> Self {
        Self {
            duration_s: Finite(value.duration_s()),
            gait_phase: Finite(value.gait_phase()),
            lr_amplitude_m: Finite(value.lr_amplitude_m()),
            max_contact_boundary_phase_error: Finite(value.max_contact_boundary_phase_error()),
            root_endpoint_displacement_x_m: Finite(value.root_endpoint_displacement_x_m()),
            root_endpoint_displacement_z_m: Finite(value.root_endpoint_displacement_z_m()),
            root_accumulated_yaw_deg: Finite(value.root_accumulated_yaw_deg()),
            max_loop_position_delta_m: Finite(value.max_loop_position_delta_m()),
            max_loop_rotation_delta_deg: Finite(value.max_loop_rotation_delta_deg()),
            max_loop_velocity_delta_mps: Finite(value.max_loop_velocity_delta_mps()),
            max_loop_angular_velocity_delta_degps: Finite(
                value.max_loop_angular_velocity_delta_degps(),
            ),
        }
    }
}

#[derive(Serialize)]
struct MemberResources {
    artifact_bytes: u64,
    contact_fragment_bytes: u64,
}

#[derive(Serialize)]
struct MemberEvidenceRecord<'a> {
    schema: &'static str,
    schema_version: u32,
    tool: &'a ToolInfo,
    command: &'static str,
    member_index: u64,
    member_id: &'a str,
    paths: PathsRecord<'a>,
    manifest_input: &'a InputIdentity,
    parameterization_input: &'a InputIdentity,
    source: SourceBindingRecord<'a>,
    output: OutputBindingRecord<'a>,
    operation: &'a InputIdentity,
    proof_policy: ProofPolicyRecord,
    proof: ProofFactsRecord,
    resources: MemberResources,
}

#[derive(Serialize)]
struct AggregateMemberRecord {
    member_index: u64,
    member_id: String,
    artifact_path: String,
    contact_fragment_path: String,
    evidence_path: String,
    source_artifact: InputIdentity,
    source_dependency_closure_identity: DependencyClosureIdentityV1,
    output_artifact: InputIdentity,
    output_dependency_closure_identity: DependencyClosureIdentityV1,
    output_contact_fragment: InputIdentity,
    independently_detected_contact_fragment: InputIdentity,
    evidence: InputIdentity,
}

#[derive(Serialize)]
struct AggregateResources {
    members: u64,
    files: u64,
    artifact_bytes: u64,
    contact_fragment_bytes: u64,
    member_evidence_bytes: u64,
    aggregate_evidence_bytes: u64,
    total_bytes: u64,
    retained_candidate_bytes: u64,
    source_metric_pose_cells: u64,
    source_metric_sample_evaluations: u64,
    output_metric_pose_cells: u64,
    output_metric_sample_evaluations: u64,
    metric_pose_cells: u64,
    metric_sample_evaluations: u64,
}

#[derive(Serialize)]
struct AggregateEvidenceRecord<'a> {
    schema: &'static str,
    schema_version: u32,
    tool: &'a ToolInfo,
    command: &'static str,
    outcome: &'static str,
    manifest_input: &'a InputIdentity,
    parameterization_input: &'a InputIdentity,
    runtime_set_id: &'a str,
    reference_member: &'a str,
    proof_policy: ProofPolicyRecord,
    gait_phase_spread: Finite,
    members: &'a [AggregateMemberRecord],
    resources: AggregateResources,
}

fn serialize_bounded<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, ProducerFailure> {
    serialize_record_bounded(value, limit).map_err(|error| match error {
        BoundedSerializationError::Limit { .. } => evidence_budget_refusal(limit),
        BoundedSerializationError::Serialize(error) => {
            ProducerFailure::Operator(format!("cannot serialize foot-cycle evidence: {error}"))
        }
    })
}

struct EncodedMember {
    artifact_alias: PathBuf,
    fragment_alias: PathBuf,
    evidence_alias: PathBuf,
    artifact_bytes: Vec<u8>,
    fragment_bytes: Vec<u8>,
    evidence_bytes: Vec<u8>,
}

struct EncodedGeneration {
    destination: PathBuf,
    members: Vec<EncodedMember>,
    aggregate_bytes: Vec<u8>,
    file_count: u64,
    total_bytes: u64,
}

#[derive(Debug)]
enum ProducerFailure {
    Operator(String),
    Refusal(Rejection),
}

impl From<String> for ProducerFailure {
    fn from(error: String) -> Self {
        Self::Operator(error)
    }
}

fn generation_budget_refusal(limit: u64) -> ProducerFailure {
    ProducerFailure::Refusal(Rejection::new(
        Stage::Encode,
        Kind::UnrepresentableArtifact,
        format!("foot-cycle generation exceeds {limit} bytes"),
    ))
}

fn evidence_budget_refusal(limit: usize) -> ProducerFailure {
    ProducerFailure::Refusal(Rejection::new(
        Stage::Encode,
        Kind::UnrepresentableArtifact,
        format!("foot-cycle evidence exceeds {limit} bytes"),
    ))
}

/// Run the one JSON-only public producer command.
pub(crate) fn run(
    manifest: &Path,
    parameterization: &Path,
    tool: ToolInfo,
) -> Result<ExitCode, String> {
    match produce(manifest, parameterization, &tool) {
        Ok(generation) => publish_and_emit_with(&generation, emit),
        Err(ProducerFailure::Operator(error)) => Err(error),
        Err(ProducerFailure::Refusal(rejection)) => producer::emit_rejection(
            Command::CollectionTransformFootCycle,
            Format::Json,
            tool,
            rejection,
            &mut producer::ProcessRefusalDelivery,
        ),
    }
}

fn publish_and_emit_with(
    generation: &EncodedGeneration,
    emit_record: impl FnOnce(&[u8]),
) -> Result<ExitCode, String> {
    publish_encoded_generation(generation)?;
    // Publication is durable before stdout. A failed stream cannot
    // truthfully turn the completed generation into an operator error.
    emit_record(&generation.aggregate_bytes);
    Ok(ExitCode::SUCCESS)
}

fn publish_encoded_generation(generation: &EncodedGeneration) -> Result<(), String> {
    let mut files = Vec::with_capacity(generation.file_count as usize);
    for member in &generation.members {
        files.push(GenerationFile {
            alias: &member.artifact_alias,
            bytes: &member.artifact_bytes,
        });
        files.push(GenerationFile {
            alias: &member.fragment_alias,
            bytes: &member.fragment_bytes,
        });
        files.push(GenerationFile {
            alias: &member.evidence_alias,
            bytes: &member.evidence_bytes,
        });
    }
    let aggregate_alias = Path::new("aggregate-evidence.json");
    files.push(GenerationFile {
        alias: aggregate_alias,
        bytes: &generation.aggregate_bytes,
    });
    publish_generation(
        &generation.destination,
        &files,
        GenerationPublicationLimits {
            max_files: generation.file_count,
            max_file_bytes: MAX_SHARED_BYTES,
            max_total_bytes: MAX_SHARED_BYTES,
            max_alias_components: MAX_ALIAS_COMPONENTS,
            max_alias_component_bytes: MAX_ALIAS_COMPONENT_BYTES,
            max_total_alias_bytes: generation
                .file_count
                .checked_mul(96)
                .ok_or_else(|| "foot-cycle alias budget overflow".to_owned())?,
        },
    )?;
    debug_assert_eq!(
        generation.total_bytes,
        files
            .iter()
            .map(|file| file.bytes.len() as u64)
            .sum::<u64>()
    );
    Ok(())
}

fn produce(
    manifest: &Path,
    parameterization: &Path,
    tool: &ToolInfo,
) -> Result<EncodedGeneration, ProducerFailure> {
    let prepared = prepare_foot_cycle_parameterization_v1(manifest, parameterization)
        .map_err(classify_preparation)?;
    let proved = serialize_and_prove_foot_cycle_v1(&prepared).map_err(classify_proof)?;
    encode_generation(&prepared, &proved, tool)
}

fn classify_preparation(error: FootCycleSourcePrepError) -> ProducerFailure {
    use FootCycleSourcePrepKind as K;
    match error.kind() {
        K::Control
        | K::UnsafePathSet
        | K::SourceUnavailable
        | K::SourceLoadOperator
        | K::SourceDigestMismatch
        | K::TakeMismatch
        | K::ContactRead
        | K::ContactInvalid
        | K::DurationMismatch
        | K::PlanBindingMismatch => ProducerFailure::Operator(error.to_string()),
        K::SourceBudget | K::ContactBudget | K::IncompleteClosure => ProducerFailure::Refusal(
            Rejection::new(Stage::Analysis, Kind::IncompleteEvidence, error.to_string()),
        ),
        K::SourceLoadRefused => ProducerFailure::Refusal(Rejection::new(
            Stage::Load,
            Kind::InvalidAssetStructure,
            error.to_string(),
        )),
        K::RootEvidenceUnavailable => ProducerFailure::Refusal(Rejection::new(
            Stage::Analysis,
            Kind::IncompleteEvidence,
            error.to_string(),
        )),
        K::PlanRefused => ProducerFailure::Refusal(Rejection::new(
            Stage::Analysis,
            Kind::AssetRecipeMismatch,
            error.to_string(),
        )),
        K::ClipTransformRefused | K::ExtensionTransformRefused => ProducerFailure::Refusal(
            Rejection::new(Stage::Transform, Kind::TransformRefused, error.to_string()),
        ),
        K::ContactTransformRefused => ProducerFailure::Refusal(Rejection::new(
            Stage::Proof,
            Kind::ProofFailed,
            error.to_string(),
        )),
    }
}

fn classify_proof(error: FootCycleProofError) -> ProducerFailure {
    ProducerFailure::Refusal(Rejection::new(
        Stage::Proof,
        Kind::ProofFailed,
        format!("foot-cycle proof refused ({:?})", error.kind()),
    ))
}

fn aliases(index: usize) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let index = u64::try_from(index).map_err(|_| "foot-cycle member index exceeds u64")?;
    let root = format!("members/{index:06}");
    Ok((
        PathBuf::from(format!("{root}/artifact.glb")),
        PathBuf::from(format!("{root}/contact-fragment.json")),
        PathBuf::from(format!("{root}/evidence.json")),
    ))
}

fn alias_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "internal foot-cycle alias is not UTF-8".to_owned())
}

fn operation_identity(
    operation: &animsmith_core::ContactTransformOperationV1,
) -> Result<InputIdentity, String> {
    let bytes = serde_jcs::to_vec(operation)
        .map_err(|error| format!("cannot canonicalize foot-cycle operation: {error}"))?;
    Ok(InputIdentity::from_bytes(&bytes))
}

fn checked_add(total: u64, next: usize, limit: u64) -> Result<u64, ProducerFailure> {
    let next = u64::try_from(next)
        .map_err(|_| ProducerFailure::Operator("foot-cycle byte count exceeds u64".to_owned()))?;
    total
        .checked_add(next)
        .filter(|total| *total <= limit)
        .ok_or_else(|| generation_budget_refusal(limit))
}

fn encode_generation(
    prepared: &PreparedFootCycleCollectionV1,
    proved: &ProvedFootCycleCollectionV1,
    tool: &ToolInfo,
) -> Result<EncodedGeneration, ProducerFailure> {
    encode_generation_with_limits(prepared, proved, tool, ENCODING_LIMITS)
}

fn encode_generation_with_limits(
    prepared: &PreparedFootCycleCollectionV1,
    proved: &ProvedFootCycleCollectionV1,
    tool: &ToolInfo,
    limits: EncodingLimits,
) -> Result<EncodedGeneration, ProducerFailure> {
    if prepared.members().len() != proved.members().len()
        || prepared.members().len() != prepared.plan().members().len()
    {
        return Err("foot-cycle evidence member binding mismatch"
            .to_owned()
            .into());
    }
    let member_count = u64::try_from(proved.members().len())
        .map_err(|_| "foot-cycle member count exceeds u64".to_owned())?;
    let file_count = member_count
        .checked_mul(3)
        .and_then(|count| count.checked_add(1))
        .ok_or_else(|| "foot-cycle 3N+1 file count overflow".to_owned())?;

    let mut encoded = Vec::with_capacity(proved.members().len());
    let mut aggregate_members = Vec::with_capacity(proved.members().len());
    let mut artifact_total = 0_u64;
    let mut fragment_total = 0_u64;
    let mut evidence_total = 0_u64;
    let mut shared_total = 0_u64;

    for (index, ((prepared_member, plan), proved_member)) in prepared
        .members()
        .iter()
        .zip(prepared.plan().members())
        .zip(proved.members())
        .enumerate()
    {
        if prepared_member.id() != plan.id() || plan.id() != proved_member.id() {
            return Err("foot-cycle evidence ordered member mismatch"
                .to_owned()
                .into());
        }
        let source = prepared
            .sources()
            .get(prepared_member.source_index())
            .ok_or_else(|| "foot-cycle evidence source binding mismatch".to_owned())?;
        let source_closure = source
            .dependency_closure()
            .identity()
            .ok_or_else(|| "foot-cycle source closure is incomplete".to_owned())?;
        let output_closure = proved_member
            .dependency_closure()
            .identity()
            .ok_or_else(|| "foot-cycle output closure is incomplete".to_owned())?;
        let transformed = proved_member
            .contact_transform()
            .output()
            .ok_or_else(|| "foot-cycle transformed contact output is absent".to_owned())?;
        let detected_fragment = proved_member
            .independently_detected_contact()
            .canonical_identity()
            .map_err(|error| format!("cannot identify independently detected contact: {error}"))?;
        let fragment_bytes = transformed
            .contact_fragment()
            .canonical_json()
            .map_err(|error| format!("cannot serialize transformed contact fragment: {error}"))?;
        let (artifact_alias, fragment_alias, evidence_alias) = aliases(index)?;
        let operation = operation_identity(plan.operation())?;
        let member_index =
            u64::try_from(index).map_err(|_| "foot-cycle member index exceeds u64".to_owned())?;
        let member_record = MemberEvidenceRecord {
            schema: MEMBER_EVIDENCE_V1_ID,
            schema_version: EVIDENCE_VERSION,
            tool,
            command: COMMAND,
            member_index,
            member_id: proved_member.id().as_str(),
            paths: PathsRecord {
                artifact: alias_text(&artifact_alias)?,
                contact_fragment: alias_text(&fragment_alias)?,
                evidence: alias_text(&evidence_alias)?,
            },
            manifest_input: prepared.manifest_input(),
            parameterization_input: prepared.parameterization_input(),
            source: SourceBindingRecord {
                source_key: source.key(),
                artifact: source.artifact(),
                dependency_closure_identity: source_closure,
                config: source.config_input(),
            },
            output: OutputBindingRecord {
                artifact: proved_member.artifact(),
                dependency_closure_identity: output_closure,
                contact_fragment: transformed.fragment(),
                independently_detected_contact_fragment: &detected_fragment,
            },
            operation: &operation,
            proof_policy: prepared.plan().proof().into(),
            proof: proved_member.facts().into(),
            resources: MemberResources {
                artifact_bytes: proved_member.artifact().bytes(),
                contact_fragment_bytes: fragment_bytes.len() as u64,
            },
        };
        let evidence_bytes = serialize_bounded(&member_record, limits.member_evidence_bytes)?;
        // The strict reader is part of the producer boundary, not only a test helper.
        read_member_evidence_v1(&evidence_bytes)?;

        shared_total = checked_add(
            shared_total,
            proved_member.artifact_bytes().len(),
            limits.shared_bytes,
        )?;
        shared_total = checked_add(shared_total, fragment_bytes.len(), limits.shared_bytes)?;
        shared_total = checked_add(shared_total, evidence_bytes.len(), limits.shared_bytes)?;
        artifact_total = artifact_total
            .checked_add(proved_member.artifact().bytes())
            .ok_or_else(|| "foot-cycle artifact bytes overflow".to_owned())?;
        fragment_total = fragment_total
            .checked_add(fragment_bytes.len() as u64)
            .ok_or_else(|| "foot-cycle fragment bytes overflow".to_owned())?;
        evidence_total = evidence_total
            .checked_add(evidence_bytes.len() as u64)
            .ok_or_else(|| "foot-cycle evidence bytes overflow".to_owned())?;
        let evidence_identity = InputIdentity::from_bytes(&evidence_bytes);
        aggregate_members.push(AggregateMemberRecord {
            member_index,
            member_id: proved_member.id().as_str().to_owned(),
            artifact_path: alias_text(&artifact_alias)?.to_owned(),
            contact_fragment_path: alias_text(&fragment_alias)?.to_owned(),
            evidence_path: alias_text(&evidence_alias)?.to_owned(),
            source_artifact: source.artifact().clone(),
            source_dependency_closure_identity: source_closure.clone(),
            output_artifact: proved_member.artifact().clone(),
            output_dependency_closure_identity: output_closure.clone(),
            output_contact_fragment: transformed.fragment().clone(),
            independently_detected_contact_fragment: detected_fragment,
            evidence: evidence_identity,
        });
        encoded.push(EncodedMember {
            artifact_alias,
            fragment_alias,
            evidence_alias,
            artifact_bytes: proved_member.artifact_bytes().to_vec(),
            fragment_bytes,
            evidence_bytes,
        });
    }

    let retained_candidate_bytes = u64::try_from(proved.retained_candidate_bytes())
        .map_err(|_| "retained foot-cycle bytes exceed u64".to_owned())?;
    if retained_candidate_bytes != artifact_total {
        return Err("retained foot-cycle artifact byte accounting mismatch"
            .to_owned()
            .into());
    }
    let fixed_without_aggregate = shared_total;
    let mut aggregate_size = 0_u64;
    let mut aggregate_bytes = Vec::new();
    let mut aggregate_converged = false;
    for _ in 0..limits.aggregate_convergence_iterations {
        let total_bytes = fixed_without_aggregate
            .checked_add(aggregate_size)
            .filter(|total| *total <= limits.shared_bytes)
            .ok_or_else(|| generation_budget_refusal(limits.shared_bytes))?;
        let record = AggregateEvidenceRecord {
            schema: AGGREGATE_EVIDENCE_V1_ID,
            schema_version: EVIDENCE_VERSION,
            tool,
            command: COMMAND,
            outcome: "published",
            manifest_input: prepared.manifest_input(),
            parameterization_input: prepared.parameterization_input(),
            runtime_set_id: prepared.plan().runtime_set_id().as_str(),
            reference_member: prepared.plan().reference_member().as_str(),
            proof_policy: prepared.plan().proof().into(),
            gait_phase_spread: Finite(proved.gait_phase_spread()),
            members: &aggregate_members,
            resources: AggregateResources {
                members: member_count,
                files: file_count,
                artifact_bytes: artifact_total,
                contact_fragment_bytes: fragment_total,
                member_evidence_bytes: evidence_total,
                aggregate_evidence_bytes: aggregate_size,
                total_bytes,
                retained_candidate_bytes,
                source_metric_pose_cells: proved.source_metric_pose_cells() as u64,
                source_metric_sample_evaluations: proved.source_metric_sample_evaluations() as u64,
                output_metric_pose_cells: proved.output_metric_pose_cells() as u64,
                output_metric_sample_evaluations: proved.output_metric_sample_evaluations() as u64,
                metric_pose_cells: proved.metric_pose_cells() as u64,
                metric_sample_evaluations: proved.metric_sample_evaluations() as u64,
            },
        };
        let candidate = serialize_bounded(&record, limits.aggregate_evidence_bytes)?;
        let next = candidate.len() as u64;
        aggregate_bytes = candidate;
        if next == aggregate_size {
            aggregate_converged = true;
            break;
        }
        aggregate_size = next;
    }
    if !aggregate_converged || aggregate_bytes.len() as u64 != aggregate_size {
        return Err("aggregate foot-cycle evidence size did not converge"
            .to_owned()
            .into());
    }
    read_aggregate_evidence_v1(&aggregate_bytes)?;
    shared_total = fixed_without_aggregate
        .checked_add(aggregate_size)
        .filter(|total| *total <= limits.shared_bytes)
        .ok_or_else(|| generation_budget_refusal(limits.shared_bytes))?;

    Ok(EncodedGeneration {
        destination: prepared.output_directory().to_path_buf(),
        members: encoded,
        aggregate_bytes,
        file_count,
        total_bytes: shared_total,
    })
}

// Readers intentionally use closed wire types even though the producer only
// needs their validation side effect. They catch duplicate/unknown fields,
// wrong schema/command, non-finite proof values, and path/order drift.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    sha256: String,
    bytes: u64,
}

impl IdentityWire {
    fn validate(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            && self.bytes <= 9_007_199_254_740_991
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSourceWire {
    revision: RequiredNullableString,
    dirty: RequiredNullableBool,
}

#[derive(Deserialize)]
#[serde(transparent)]
struct RequiredNullableString(Option<String>);

#[derive(Deserialize)]
#[serde(transparent)]
struct RequiredNullableBool(Option<bool>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolWire {
    name: String,
    version: String,
    source: ToolSourceWire,
}

impl ToolWire {
    fn valid(&self) -> bool {
        let revision_valid = self.source.revision.0.as_deref().is_none_or(|revision| {
            revision.len() == 40
                && revision
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
        });
        let _ = self.source.dirty.0;
        self.name == "animsmith" && !self.version.is_empty() && revision_valid
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathsWire {
    artifact: String,
    contact_fragment: String,
    evidence: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceWire {
    source_key: String,
    artifact: IdentityWire,
    dependency_closure_identity: IdentityWire,
    #[serde(default)]
    config: Option<IdentityWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputWire {
    artifact: IdentityWire,
    dependency_closure_identity: IdentityWire,
    contact_fragment: IdentityWire,
    independently_detected_contact_fragment: IdentityWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyWire {
    max_gait_phase_spread: f64,
    min_lr_amplitude_m: f64,
    max_contact_boundary_phase_error: f64,
}

impl PolicyWire {
    fn valid(&self) -> bool {
        self.max_gait_phase_spread.is_finite()
            && (0.0..=0.5).contains(&self.max_gait_phase_spread)
            && self.min_lr_amplitude_m.is_finite()
            && self.min_lr_amplitude_m >= 0.0
            && self.max_contact_boundary_phase_error.is_finite()
            && (0.0..=0.5).contains(&self.max_contact_boundary_phase_error)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProofWire {
    duration_s: f64,
    gait_phase: f64,
    lr_amplitude_m: f64,
    max_contact_boundary_phase_error: f64,
    root_endpoint_displacement_x_m: f64,
    root_endpoint_displacement_z_m: f64,
    root_accumulated_yaw_deg: f64,
    max_loop_position_delta_m: f64,
    max_loop_rotation_delta_deg: f64,
    max_loop_velocity_delta_mps: f64,
    max_loop_angular_velocity_delta_degps: f64,
}

impl ProofWire {
    fn valid(&self) -> bool {
        self.duration_s.is_finite()
            && self.duration_s > 0.0
            && self.gait_phase.is_finite()
            && (0.0..=1.0).contains(&self.gait_phase)
            && self.lr_amplitude_m.is_finite()
            && self.lr_amplitude_m >= 0.0
            && self.max_contact_boundary_phase_error.is_finite()
            && (0.0..=0.5).contains(&self.max_contact_boundary_phase_error)
            && [
                self.root_endpoint_displacement_x_m,
                self.root_endpoint_displacement_z_m,
                self.root_accumulated_yaw_deg,
            ]
            .into_iter()
            .all(f64::is_finite)
            && [
                self.max_loop_position_delta_m,
                self.max_loop_rotation_delta_deg,
                self.max_loop_velocity_delta_mps,
                self.max_loop_angular_velocity_delta_degps,
            ]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberResourcesWire {
    artifact_bytes: u64,
    contact_fragment_bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberWire {
    schema: String,
    schema_version: u32,
    tool: ToolWire,
    command: String,
    member_index: u64,
    member_id: String,
    paths: PathsWire,
    manifest_input: IdentityWire,
    parameterization_input: IdentityWire,
    source: SourceWire,
    output: OutputWire,
    operation: IdentityWire,
    proof_policy: PolicyWire,
    proof: ProofWire,
    resources: MemberResourcesWire,
}

pub(crate) fn read_member_evidence_v1(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_MEMBER_EVIDENCE_BYTES {
        return Err("foot-cycle member evidence exceeds its V1 limit".to_owned());
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let wire = MemberWire::deserialize(&mut decoder)
        .map_err(|_| "invalid foot-cycle member evidence V1".to_owned())?;
    decoder
        .end()
        .map_err(|_| "invalid foot-cycle member evidence V1".to_owned())?;
    let expected = aliases(
        usize::try_from(wire.member_index)
            .map_err(|_| "invalid foot-cycle member evidence V1".to_owned())?,
    )?;
    let identities_valid = [
        &wire.manifest_input,
        &wire.parameterization_input,
        &wire.source.artifact,
        &wire.source.dependency_closure_identity,
        &wire.output.artifact,
        &wire.output.dependency_closure_identity,
        &wire.output.contact_fragment,
        &wire.output.independently_detected_contact_fragment,
        &wire.operation,
    ]
    .into_iter()
    .all(IdentityWire::validate)
        && wire
            .source
            .config
            .as_ref()
            .is_none_or(IdentityWire::validate);
    if wire.schema != MEMBER_EVIDENCE_V1_ID
        || wire.schema_version != EVIDENCE_VERSION
        || wire.command != COMMAND
        || wire.member_index > 4095
        || wire.member_id.is_empty()
        || wire.member_id.len() > 255
        || wire.source.source_key.is_empty()
        || wire.source.source_key.len() > 4096
        || !wire.tool.valid()
        || !identities_valid
        || !wire.proof_policy.valid()
        || !wire.proof.valid()
        || wire.resources.artifact_bytes == 0
        || wire.resources.contact_fragment_bytes == 0
        || wire.resources.artifact_bytes > MAX_SHARED_BYTES
        || wire.resources.contact_fragment_bytes > MAX_MEMBER_EVIDENCE_BYTES as u64
        || wire.resources.artifact_bytes != wire.output.artifact.bytes
        || wire.resources.contact_fragment_bytes != wire.output.contact_fragment.bytes
        || wire.paths.artifact != alias_text(&expected.0)?
        || wire.paths.contact_fragment != alias_text(&expected.1)?
        || wire.paths.evidence != alias_text(&expected.2)?
    {
        return Err("invalid foot-cycle member evidence V1".to_owned());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateMemberWire {
    member_index: u64,
    member_id: String,
    artifact_path: String,
    contact_fragment_path: String,
    evidence_path: String,
    source_artifact: IdentityWire,
    source_dependency_closure_identity: IdentityWire,
    output_artifact: IdentityWire,
    output_dependency_closure_identity: IdentityWire,
    output_contact_fragment: IdentityWire,
    independently_detected_contact_fragment: IdentityWire,
    evidence: IdentityWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateResourcesWire {
    members: u64,
    files: u64,
    artifact_bytes: u64,
    contact_fragment_bytes: u64,
    member_evidence_bytes: u64,
    aggregate_evidence_bytes: u64,
    total_bytes: u64,
    retained_candidate_bytes: u64,
    source_metric_pose_cells: u64,
    source_metric_sample_evaluations: u64,
    output_metric_pose_cells: u64,
    output_metric_sample_evaluations: u64,
    metric_pose_cells: u64,
    metric_sample_evaluations: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateWire {
    schema: String,
    schema_version: u32,
    tool: ToolWire,
    command: String,
    outcome: String,
    manifest_input: IdentityWire,
    parameterization_input: IdentityWire,
    runtime_set_id: String,
    reference_member: String,
    proof_policy: PolicyWire,
    gait_phase_spread: f64,
    members: Vec<AggregateMemberWire>,
    resources: AggregateResourcesWire,
}

pub(crate) fn read_aggregate_evidence_v1(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_AGGREGATE_EVIDENCE_BYTES {
        return Err("foot-cycle aggregate evidence exceeds its V1 limit".to_owned());
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let wire = AggregateWire::deserialize(&mut decoder)
        .map_err(|_| "invalid foot-cycle aggregate evidence V1".to_owned())?;
    decoder
        .end()
        .map_err(|_| "invalid foot-cycle aggregate evidence V1".to_owned())?;
    let member_count = u64::try_from(wire.members.len())
        .map_err(|_| "invalid foot-cycle aggregate evidence V1".to_owned())?;
    let expected_files = member_count
        .checked_mul(3)
        .and_then(|value| value.checked_add(1));
    let ordered = wire.members.iter().enumerate().all(|(index, member)| {
        let Ok(index_u64) = u64::try_from(index) else {
            return false;
        };
        let Ok(expected) = aliases(index) else {
            return false;
        };
        member.member_index == index_u64
            && !member.member_id.is_empty()
            && member.member_id.len() <= 255
            && member.artifact_path == expected.0.to_string_lossy()
            && member.contact_fragment_path == expected.1.to_string_lossy()
            && member.evidence_path == expected.2.to_string_lossy()
            && [
                &member.source_artifact,
                &member.source_dependency_closure_identity,
                &member.output_artifact,
                &member.output_dependency_closure_identity,
                &member.output_contact_fragment,
                &member.independently_detected_contact_fragment,
                &member.evidence,
            ]
            .into_iter()
            .all(IdentityWire::validate)
    });
    let aggregate_len = bytes.len() as u64;
    let artifact_bytes = wire.members.iter().try_fold(0_u64, |total, member| {
        total.checked_add(member.output_artifact.bytes)
    });
    let contact_fragment_bytes = wire.members.iter().try_fold(0_u64, |total, member| {
        total.checked_add(member.output_contact_fragment.bytes)
    });
    let member_evidence_bytes = wire.members.iter().try_fold(0_u64, |total, member| {
        total.checked_add(member.evidence.bytes)
    });
    let components = wire
        .resources
        .artifact_bytes
        .checked_add(wire.resources.contact_fragment_bytes)
        .and_then(|value| value.checked_add(wire.resources.member_evidence_bytes))
        .and_then(|value| value.checked_add(wire.resources.aggregate_evidence_bytes));
    if wire.schema != AGGREGATE_EVIDENCE_V1_ID
        || wire.schema_version != EVIDENCE_VERSION
        || wire.command != COMMAND
        || wire.outcome != "published"
        || wire.runtime_set_id.is_empty()
        || wire.runtime_set_id.len() > 255
        || wire.reference_member.is_empty()
        || wire.reference_member.len() > 255
        || !wire.manifest_input.validate()
        || !wire.parameterization_input.validate()
        || !wire.proof_policy.valid()
        || !wire.gait_phase_spread.is_finite()
        || !(0.0..=0.5).contains(&wire.gait_phase_spread)
        || !ordered
        || !(2..=4096).contains(&member_count)
        || wire.resources.members != member_count
        || Some(wire.resources.files) != expected_files
        || artifact_bytes != Some(wire.resources.artifact_bytes)
        || contact_fragment_bytes != Some(wire.resources.contact_fragment_bytes)
        || member_evidence_bytes != Some(wire.resources.member_evidence_bytes)
        || wire.resources.aggregate_evidence_bytes != aggregate_len
        || components != Some(wire.resources.total_bytes)
        || wire.resources.total_bytes > MAX_SHARED_BYTES
        || wire.resources.retained_candidate_bytes != wire.resources.artifact_bytes
        || wire
            .resources
            .source_metric_pose_cells
            .checked_add(wire.resources.output_metric_pose_cells)
            != Some(wire.resources.metric_pose_cells)
        || wire
            .resources
            .source_metric_sample_evaluations
            .checked_add(wire.resources.output_metric_sample_evaluations)
            != Some(wire.resources.metric_sample_evaluations)
        || !wire.tool.valid()
    {
        return Err("invalid foot-cycle aggregate evidence V1".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foot_cycle_source_prep::tests::{Fixture, FixtureOptions};

    struct BrokenPipe;

    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "producer stdout is closed",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn fixture_generation(fixture: &Fixture) -> Result<EncodedGeneration, ProducerFailure> {
        let prepared = fixture.prepare_proof_ready();
        let proved = serialize_and_prove_foot_cycle_v1(&prepared).map_err(classify_proof)?;
        encode_generation(&prepared, &proved, &crate::current_tool())
    }

    fn mutate_number(bytes: &[u8], key: &str, value: u64) -> Vec<u8> {
        let replacement = if value % 10 == 9 {
            value - 1
        } else {
            value + 1
        };
        assert_eq!(value.to_string().len(), replacement.to_string().len());
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let needle = format!("\"{key}\": {value}");
        let replacement = format!("\"{key}\": {replacement}");
        let mutated = text.replacen(&needle, &replacement, 1);
        assert_ne!(mutated, text, "missing field {key}");
        assert_eq!(mutated.len(), text.len());
        mutated.into_bytes()
    }

    #[test]
    fn three_n_plus_one_and_alias_order_are_checked() {
        assert_eq!(2_u64.checked_mul(3).and_then(|n| n.checked_add(1)), Some(7));
        assert_eq!(
            aliases(0).unwrap().0,
            PathBuf::from("members/000000/artifact.glb")
        );
        assert_eq!(
            aliases(1).unwrap().2,
            PathBuf::from("members/000001/evidence.json")
        );
        assert!(
            u64::MAX
                .checked_mul(3)
                .and_then(|n| n.checked_add(1))
                .is_none()
        );
    }

    #[test]
    fn bounded_serializer_accepts_exact_and_refuses_first_excess() {
        #[derive(Serialize)]
        struct Payload<'a> {
            value: &'a str,
        }
        let bytes = serialize_bounded(&Payload { value: "x" }, 19).unwrap();
        assert_eq!(bytes, b"{\n  \"value\": \"x\"\n}\n");
        assert!(matches!(
            serialize_bounded(&Payload { value: "x" }, 18),
            Err(ProducerFailure::Refusal(_))
        ));
        assert!(matches!(
            serialize_bounded(&Finite(f64::NAN), 18),
            Err(ProducerFailure::Operator(_))
        ));
    }

    #[test]
    fn checked_shared_cap_is_inclusive_and_overflow_safe() {
        assert_eq!(
            checked_add(MAX_SHARED_BYTES - 1, 1, MAX_SHARED_BYTES).unwrap(),
            MAX_SHARED_BYTES
        );
        assert!(matches!(
            checked_add(MAX_SHARED_BYTES, 1, MAX_SHARED_BYTES),
            Err(ProducerFailure::Refusal(_))
        ));
        assert!(matches!(
            checked_add(u64::MAX, 1, MAX_SHARED_BYTES),
            Err(ProducerFailure::Refusal(_))
        ));
    }

    #[test]
    fn actual_evidence_and_generation_limits_are_inclusive_and_convergence_is_required() {
        let fixture = Fixture::create(FixtureOptions::default());
        let prepared = fixture.prepare_proof_ready();
        let proved = serialize_and_prove_foot_cycle_v1(&prepared).unwrap();
        let full = encode_generation(&prepared, &proved, &crate::current_tool()).unwrap();
        let member_limit = full
            .members
            .iter()
            .map(|member| member.evidence_bytes.len())
            .max()
            .unwrap();
        let aggregate_limit = full.aggregate_bytes.len();

        let exact = EncodingLimits {
            shared_bytes: full.total_bytes,
            member_evidence_bytes: member_limit,
            aggregate_evidence_bytes: aggregate_limit,
            aggregate_convergence_iterations: ENCODING_LIMITS.aggregate_convergence_iterations,
        };
        let encoded =
            encode_generation_with_limits(&prepared, &proved, &crate::current_tool(), exact)
                .unwrap();
        assert_eq!(encoded.total_bytes, full.total_bytes);
        assert_eq!(encoded.aggregate_bytes, full.aggregate_bytes);

        for limits in [
            EncodingLimits {
                member_evidence_bytes: member_limit - 1,
                ..exact
            },
            EncodingLimits {
                aggregate_evidence_bytes: aggregate_limit - 1,
                ..exact
            },
            EncodingLimits {
                shared_bytes: full.total_bytes - 1,
                ..exact
            },
        ] {
            assert!(matches!(
                encode_generation_with_limits(&prepared, &proved, &crate::current_tool(), limits),
                Err(ProducerFailure::Refusal(_))
            ));
        }

        assert!(matches!(
            encode_generation_with_limits(
                &prepared,
                &proved,
                &crate::current_tool(),
                EncodingLimits {
                    aggregate_convergence_iterations: 1,
                    ..exact
                }
            ),
            Err(ProducerFailure::Operator(_))
        ));
    }

    #[test]
    fn two_members_publish_exact_seven_files_in_declared_order() {
        let fixture = Fixture::create(FixtureOptions::default());
        let generation = fixture_generation(&fixture).unwrap();
        assert_eq!(generation.file_count, 7);
        assert_eq!(generation.members.len(), 2);
        assert_eq!(
            generation.members[0].artifact_alias,
            PathBuf::from("members/000000/artifact.glb")
        );
        assert_eq!(
            generation.members[0].fragment_alias,
            PathBuf::from("members/000000/contact-fragment.json")
        );
        assert_eq!(
            generation.members[0].evidence_alias,
            PathBuf::from("members/000000/evidence.json")
        );
        assert_eq!(
            generation.members[1].artifact_alias,
            PathBuf::from("members/000001/artifact.glb")
        );

        publish_encoded_generation(&generation).unwrap();
        let aggregate =
            std::fs::read(generation.destination.join("aggregate-evidence.json")).unwrap();
        assert_eq!(aggregate, generation.aggregate_bytes);
        let mut files = Vec::new();
        for index in 0..2 {
            let root = generation.destination.join(format!("members/{index:06}"));
            for name in ["artifact.glb", "contact-fragment.json", "evidence.json"] {
                assert!(root.join(name).is_file());
                files.push(root.join(name));
            }
        }
        files.push(generation.destination.join("aggregate-evidence.json"));
        assert_eq!(files.len(), 7);
        for member in &generation.members {
            assert!(
                animsmith_gltf::load_source_bytes(
                    Path::new("artifact.glb"),
                    &member.artifact_bytes
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn stdout_is_the_exact_aggregate_file_and_a_broken_stream_cannot_reverse_publication() {
        let exact = Fixture::create(FixtureOptions::default());
        let exact_generation = fixture_generation(&exact).unwrap();
        let mut stdout = Vec::new();
        let status =
            publish_and_emit_with(&exact_generation, |bytes| stdout.extend_from_slice(bytes))
                .expect("the complete generation publishes");
        assert_eq!(status, ExitCode::SUCCESS);
        assert_eq!(stdout, exact_generation.aggregate_bytes);
        assert_eq!(
            stdout,
            std::fs::read(exact_generation.destination.join("aggregate-evidence.json")).unwrap()
        );

        let broken = Fixture::create(FixtureOptions::default());
        let broken_generation = fixture_generation(&broken).unwrap();
        let mut diagnostics = Vec::new();
        let status = publish_and_emit_with(&broken_generation, |bytes| {
            crate::publish::emit_with(&mut BrokenPipe, &mut diagnostics, bytes);
        })
        .expect("report delivery cannot reverse a durable publication");
        assert_eq!(status, ExitCode::SUCCESS);
        assert!(
            broken_generation
                .destination
                .join("aggregate-evidence.json")
                .is_file()
        );
        assert!(
            String::from_utf8(diagnostics)
                .unwrap()
                .contains("cannot write JSON output to stdout")
        );
    }

    #[test]
    fn strict_readers_reject_actual_record_mutations_and_accounting_drift() {
        let fixture = Fixture::create(FixtureOptions::default());
        let generation = fixture_generation(&fixture).unwrap();

        let member_bytes = &generation.members[0].evidence_bytes;
        read_member_evidence_v1(member_bytes).unwrap();
        let member = String::from_utf8(member_bytes.clone()).unwrap();
        let unknown = member.replacen("{\n", "{\n  \"unknown\": true,\n", 1);
        assert!(read_member_evidence_v1(unknown.as_bytes()).is_err());

        let stale_path = member.replacen(
            "members/000000/artifact.glb",
            "members/000001/artifact.glb",
            1,
        );
        assert!(read_member_evidence_v1(stale_path.as_bytes()).is_err());

        let member_wire: MemberWire = serde_json::from_slice(member_bytes).unwrap();
        for field in ["artifact_bytes", "contact_fragment_bytes"] {
            let value = match field {
                "artifact_bytes" => member_wire.resources.artifact_bytes,
                "contact_fragment_bytes" => member_wire.resources.contact_fragment_bytes,
                _ => unreachable!(),
            };
            assert!(read_member_evidence_v1(&mutate_number(member_bytes, field, value)).is_err());
        }

        let aggregate_bytes = &generation.aggregate_bytes;
        read_aggregate_evidence_v1(aggregate_bytes).unwrap();
        let aggregate = String::from_utf8(aggregate_bytes.clone()).unwrap();
        let unknown = aggregate.replacen("{\n", "{\n  \"unknown\": true,\n", 1);
        assert!(read_aggregate_evidence_v1(unknown.as_bytes()).is_err());
        let wrong_order = aggregate.replacen("\"member_index\": 0", "\"member_index\": 1", 1);
        assert_ne!(wrong_order, aggregate);
        assert!(read_aggregate_evidence_v1(wrong_order.as_bytes()).is_err());

        let aggregate_wire: AggregateWire = serde_json::from_slice(aggregate_bytes).unwrap();
        for field in [
            "members",
            "files",
            "artifact_bytes",
            "contact_fragment_bytes",
            "member_evidence_bytes",
            "aggregate_evidence_bytes",
            "total_bytes",
            "retained_candidate_bytes",
            "source_metric_pose_cells",
            "source_metric_sample_evaluations",
            "output_metric_pose_cells",
            "output_metric_sample_evaluations",
            "metric_pose_cells",
            "metric_sample_evaluations",
        ] {
            let value = match field {
                "members" => aggregate_wire.resources.members,
                "files" => aggregate_wire.resources.files,
                "artifact_bytes" => aggregate_wire.resources.artifact_bytes,
                "contact_fragment_bytes" => aggregate_wire.resources.contact_fragment_bytes,
                "member_evidence_bytes" => aggregate_wire.resources.member_evidence_bytes,
                "aggregate_evidence_bytes" => aggregate_wire.resources.aggregate_evidence_bytes,
                "total_bytes" => aggregate_wire.resources.total_bytes,
                "retained_candidate_bytes" => aggregate_wire.resources.retained_candidate_bytes,
                "source_metric_pose_cells" => aggregate_wire.resources.source_metric_pose_cells,
                "source_metric_sample_evaluations" => {
                    aggregate_wire.resources.source_metric_sample_evaluations
                }
                "output_metric_pose_cells" => aggregate_wire.resources.output_metric_pose_cells,
                "output_metric_sample_evaluations" => {
                    aggregate_wire.resources.output_metric_sample_evaluations
                }
                "metric_pose_cells" => aggregate_wire.resources.metric_pose_cells,
                "metric_sample_evaluations" => aggregate_wire.resources.metric_sample_evaluations,
                _ => unreachable!(),
            };
            assert!(
                read_aggregate_evidence_v1(&mutate_number(aggregate_bytes, field, value)).is_err(),
                "aggregate reader accepted mutated {field}"
            );
        }
    }

    #[test]
    fn independent_runs_are_byte_deterministic_and_destination_race_is_no_replace() {
        let first = Fixture::create(FixtureOptions::default());
        let second = Fixture::create(FixtureOptions::default());
        let first_generation = fixture_generation(&first).unwrap();
        let second_generation = fixture_generation(&second).unwrap();
        assert_eq!(
            first_generation.aggregate_bytes,
            second_generation.aggregate_bytes
        );
        for (left, right) in first_generation
            .members
            .iter()
            .zip(&second_generation.members)
        {
            assert_eq!(left.artifact_bytes, right.artifact_bytes);
            assert_eq!(left.fragment_bytes, right.fragment_bytes);
            assert_eq!(left.evidence_bytes, right.evidence_bytes);
        }

        std::fs::create_dir(&first_generation.destination).unwrap();
        std::fs::write(first_generation.destination.join("winner"), b"race").unwrap();
        assert!(publish_encoded_generation(&first_generation).is_err());
        assert_eq!(
            std::fs::read(first_generation.destination.join("winner")).unwrap(),
            b"race"
        );
        assert!(
            !first_generation
                .destination
                .join("aggregate-evidence.json")
                .exists()
        );
    }

    #[test]
    fn refusal_and_stale_control_are_split_and_publish_nothing() {
        let refusal = Fixture::create(FixtureOptions::with_nonconstant_cubic_b());
        assert!(matches!(
            produce(
                &refusal.manifest,
                &refusal.parameterization,
                &crate::current_tool()
            ),
            Err(ProducerFailure::Refusal(_))
        ));
        assert!(!refusal.root.join("generated/aligned").exists());

        let stale = Fixture::create(FixtureOptions::default());
        std::fs::write(&stale.manifest, b"stale").unwrap();
        assert!(matches!(
            produce(
                &stale.manifest,
                &stale.parameterization,
                &crate::current_tool()
            ),
            Err(ProducerFailure::Operator(_))
        ));
        assert!(!stale.root.join("generated/aligned").exists());
    }
}
