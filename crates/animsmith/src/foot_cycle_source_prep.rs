//! Source-bound, in-memory preparation for foot-cycle parameterization V1.
//!
//! This private CLI-crate adapter is deliberately before serialization and
//! publication. It consumes exact rooted inputs, produces bounded transformed
//! clip candidates, and retains a typed contact-transform continuation. A
//! truthful fresh output artifact identity does not exist until the candidate
//! document is serialized, so this seam never invents one.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use animsmith_core::foot_cycle_clip::FOOT_CYCLE_CLIP_V1_MAX_CANDIDATE_BYTES;
use animsmith_core::metrics::root_trajectory_metrics;
use animsmith_core::{
    CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES, Clip, CollectionLogicalIdV1, ContactClipReferenceV1,
    ContactExtensionV1, ContactFragmentV1, ContactProducerV1, ContactTransformContextV1,
    ContactTransformOperationV1, ContactTransformResultV1, DependencyClosureIdentityV1,
    DependencyClosureV1, Document, FOOT_CYCLE_CLIP_V1_MAX_GENERATED_KEYS,
    FOOT_CYCLE_CLIP_V1_MAX_INPUT_VALUES, FOOT_CYCLE_CLIP_V1_MAX_WORK, FootCycleClipPreflightV1,
    FootCycleMemberEvidenceV1, FootCycleMemberPlanV1, FootCyclePlanV1,
    FootCycleRootMotionBindingV1, FootCycleRootMotionEvidenceV1, InputIdentity, MetricGrids,
    PoseGrid, ResolutionOutcome, Role, plan_foot_cycle_parameterization_v1,
    preflight_time_warp_clip_v1, resolve_configured_roles, time_warp_clip_v1,
    transform_contact_fragment_v1, transform_contact_support_detector_extension_time_warp_v1,
    validate_document_shape, validate_foot_cycle_manifest_binding_v1,
};

use super::collection_lint::{
    LoadedCollectionConfigForPreparation, load_collection_config_for_preparation,
};
use super::collection_manifest::{
    CollectionConfigResolution, CollectionPathResolver, CollectionResolvedPath,
    CollectionSourceResolution, load_collection_manifest_with_identity,
};
use super::collection_output::{
    COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES, COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
};
use super::contact_producer::{
    MAX_METRIC_GRID_WORK, MetricGridWork, checked_metric_grid_work, complete_closure,
    resolve_collection_take_witness,
};
use super::foot_cycle_parameterization::load_foot_cycle_parameterization_with_identity;
use super::{LoadedConfig, LoadedInput, load_with_config_for_producer_bounded};

/// Exact retained semantic payload budget for one normalized source. This is
/// separate from primary and dependency byte caps because the loader retains
/// parsed vectors and strings in addition to their source encoding.
const MAX_RETAINED_DECODED_SOURCE_BYTES: u64 = COLLECTION_OUTPUT_MAX_SOURCE_BYTES;
const MAX_AGGREGATE_METRIC_GRID_WORK: usize = MAX_METRIC_GRID_WORK;
const MAX_AGGREGATE_CONFIG_BYTES: u64 = 32 * 1024 * 1024;
const MAX_AGGREGATE_CANDIDATE_KEYS: usize = FOOT_CYCLE_CLIP_V1_MAX_GENERATED_KEYS;
const MAX_AGGREGATE_CANDIDATE_VALUES: usize = FOOT_CYCLE_CLIP_V1_MAX_INPUT_VALUES;
const MAX_AGGREGATE_CANDIDATE_BYTES: usize = FOOT_CYCLE_CLIP_V1_MAX_CANDIDATE_BYTES;
const MAX_AGGREGATE_CANDIDATE_WORK: usize = FOOT_CYCLE_CLIP_V1_MAX_WORK;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CandidateBatchWork {
    keys: usize,
    values: usize,
    bytes: usize,
    work: usize,
}

impl From<FootCycleClipPreflightV1> for CandidateBatchWork {
    fn from(preflight: FootCycleClipPreflightV1) -> Self {
        Self {
            keys: preflight.candidate_keys(),
            values: preflight.candidate_values(),
            bytes: preflight.candidate_bytes(),
            work: preflight.work(),
        }
    }
}

trait FootCyclePreparationRuntime {
    fn validate_source_document(
        &mut self,
        document: &Document,
    ) -> Result<(), FootCycleSourcePrepError>;

    fn metric_grid_work(
        &mut self,
        loaded: &LoadedInput,
        clip_index: usize,
    ) -> Result<MetricGridWork, FootCycleSourcePrepError>;

    fn build_metric_grid(
        &mut self,
        loaded: &LoadedInput,
        clip_index: usize,
    ) -> Result<Rc<PoseGrid>, FootCycleSourcePrepError>;

    fn preflight_candidate(
        &mut self,
        clip: &Clip,
        plan: &FootCycleMemberPlanV1,
    ) -> Result<CandidateBatchWork, FootCycleSourcePrepError>;

    fn build_candidate(
        &mut self,
        clip: &Clip,
        plan: &FootCycleMemberPlanV1,
    ) -> Result<Clip, FootCycleSourcePrepError>;
}

struct ProductionFootCyclePreparationRuntime;

impl FootCyclePreparationRuntime for ProductionFootCyclePreparationRuntime {
    fn validate_source_document(
        &mut self,
        document: &Document,
    ) -> Result<(), FootCycleSourcePrepError> {
        validate_document_shape(document)
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceLoad))
    }

    fn metric_grid_work(
        &mut self,
        loaded: &LoadedInput,
        clip_index: usize,
    ) -> Result<MetricGridWork, FootCycleSourcePrepError> {
        metric_grid_work_for_clip(loaded, clip_index)
    }

    fn build_metric_grid(
        &mut self,
        loaded: &LoadedInput,
        clip_index: usize,
    ) -> Result<Rc<PoseGrid>, FootCycleSourcePrepError> {
        MetricGrids::new(loaded.document())
            .grid(clip_index)
            .ok_or_else(|| {
                FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable)
            })
    }

    fn preflight_candidate(
        &mut self,
        clip: &Clip,
        plan: &FootCycleMemberPlanV1,
    ) -> Result<CandidateBatchWork, FootCycleSourcePrepError> {
        preflight_time_warp_clip_v1(clip, plan)
            .map(Into::into)
            .map_err(|_| {
                FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ClipTransformRefused)
            })
    }

    fn build_candidate(
        &mut self,
        clip: &Clip,
        plan: &FootCycleMemberPlanV1,
    ) -> Result<Clip, FootCycleSourcePrepError> {
        time_warp_clip_v1(clip, plan).map_err(|_| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ClipTransformRefused)
        })
    }
}

/// Stable category for this private preparation seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FootCycleSourcePrepKind {
    Control,
    UnsafePathSet,
    SourceUnavailable,
    SourceBudget,
    SourceLoad,
    SourceDigestMismatch,
    IncompleteClosure,
    TakeMismatch,
    ContactRead,
    ContactInvalid,
    ContactBudget,
    DurationMismatch,
    RootEvidenceUnavailable,
    PlanRefused,
    PlanBindingMismatch,
    ClipTransformRefused,
    ExtensionTransformRefused,
    ContactTransformRefused,
}

/// One closed preparation failure without host-path or parser-detail leakage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FootCycleSourcePrepError {
    kind: FootCycleSourcePrepKind,
}

impl std::fmt::Display for FootCycleSourcePrepError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "foot-cycle source preparation failed ({})",
            self.kind.label()
        )
    }
}

impl std::error::Error for FootCycleSourcePrepError {}

impl FootCycleSourcePrepKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::UnsafePathSet => "unsafe-path-set",
            Self::SourceUnavailable => "source-unavailable",
            Self::SourceBudget => "source-budget",
            Self::SourceLoad => "source-load",
            Self::SourceDigestMismatch => "source-digest-mismatch",
            Self::IncompleteClosure => "incomplete-closure",
            Self::TakeMismatch => "take-mismatch",
            Self::ContactRead => "contact-read",
            Self::ContactInvalid => "contact-invalid",
            Self::ContactBudget => "contact-budget",
            Self::DurationMismatch => "duration-mismatch",
            Self::RootEvidenceUnavailable => "root-evidence-unavailable",
            Self::PlanRefused => "plan-refused",
            Self::PlanBindingMismatch => "plan-binding-mismatch",
            Self::ClipTransformRefused => "clip-transform-refused",
            Self::ExtensionTransformRefused => "extension-transform-refused",
            Self::ContactTransformRefused => "contact-transform-refused",
        }
    }
}

impl FootCycleSourcePrepError {
    const fn new(kind: FootCycleSourcePrepKind) -> Self {
        Self { kind }
    }

    #[cfg(test)]
    const fn kind(self) -> FootCycleSourcePrepKind {
        self.kind
    }
}

/// One exact loaded manifest source retained for later candidate replacement.
pub(crate) struct PreparedFootCycleSourceV1 {
    key: String,
    artifact: InputIdentity,
    config: Arc<LoadedCollectionConfigForPreparation>,
    dependency_closure: DependencyClosureV1,
    document: Document,
}

impl PreparedFootCycleSourceV1 {
    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) const fn artifact(&self) -> &InputIdentity {
        &self.artifact
    }

    pub(crate) fn config_input(&self) -> Option<&InputIdentity> {
        self.config.input.as_ref()
    }

    pub(crate) fn config(&self) -> &animsmith_core::Config {
        &self.config.loaded.config
    }

    pub(crate) const fn dependency_closure(&self) -> &DependencyClosureV1 {
        &self.dependency_closure
    }

    pub(crate) const fn document(&self) -> &Document {
        &self.document
    }
}

/// Validated contact operation awaiting truthful serialized-output identities.
pub(crate) struct PreparedContactTransformV1 {
    operation: ContactTransformOperationV1,
    input_fragment: ContactFragmentV1,
    current_input_artifact: InputIdentity,
    current_input_dependency_closure: DependencyClosureV1,
    transformed_extensions: Vec<ContactExtensionV1>,
}

impl PreparedContactTransformV1 {
    pub(crate) const fn operation(&self) -> &ContactTransformOperationV1 {
        &self.operation
    }

    pub(crate) const fn input_fragment(&self) -> &ContactFragmentV1 {
        &self.input_fragment
    }

    /// Finish only after serialization has captured exact output identities.
    pub(crate) fn transform_after_serialization(
        &self,
        output_artifact: InputIdentity,
        output_dependency_closure: DependencyClosureV1,
        output_producer: ContactProducerV1,
    ) -> Result<ContactTransformResultV1, FootCycleSourcePrepError> {
        transform_contact_fragment_v1(
            self.operation.clone(),
            &self.input_fragment,
            ContactTransformContextV1::new(
                self.current_input_artifact.clone(),
                self.current_input_dependency_closure.clone(),
                output_artifact,
                output_dependency_closure,
                output_producer,
                Some(self.transformed_extensions.clone()),
            ),
        )
        .map_err(|_| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactTransformRefused)
        })
    }
}

/// One declared member's source witness and transformed in-memory clip.
pub(crate) struct PreparedFootCycleMemberV1 {
    id: CollectionLogicalIdV1,
    source_index: usize,
    clip_index: usize,
    candidate_clip: Clip,
    contact_transform: PreparedContactTransformV1,
}

impl PreparedFootCycleMemberV1 {
    pub(crate) fn id(&self) -> &CollectionLogicalIdV1 {
        &self.id
    }

    pub(crate) const fn source_index(&self) -> usize {
        self.source_index
    }

    pub(crate) const fn clip_index(&self) -> usize {
        self.clip_index
    }

    pub(crate) const fn candidate_clip(&self) -> &Clip {
        &self.candidate_clip
    }

    pub(crate) const fn contact_transform(&self) -> &PreparedContactTransformV1 {
        &self.contact_transform
    }
}

/// Complete all-or-nothing, in-memory preparation result.
pub(crate) struct PreparedFootCycleCollectionV1 {
    manifest_input: InputIdentity,
    parameterization_input: InputIdentity,
    output_directory: PathBuf,
    sources: Vec<PreparedFootCycleSourceV1>,
    members: Vec<PreparedFootCycleMemberV1>,
    plan: FootCyclePlanV1,
    source_metric_pose_cells: usize,
    source_metric_sample_evaluations: usize,
}

impl PreparedFootCycleCollectionV1 {
    pub(crate) const fn manifest_input(&self) -> &InputIdentity {
        &self.manifest_input
    }

    pub(crate) const fn parameterization_input(&self) -> &InputIdentity {
        &self.parameterization_input
    }

    pub(crate) fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    pub(crate) fn sources(&self) -> &[PreparedFootCycleSourceV1] {
        &self.sources
    }

    pub(crate) fn members(&self) -> &[PreparedFootCycleMemberV1] {
        &self.members
    }

    pub(crate) const fn plan(&self) -> &FootCyclePlanV1 {
        &self.plan
    }

    pub(crate) const fn source_metric_pose_cells(&self) -> usize {
        self.source_metric_pose_cells
    }

    pub(crate) const fn source_metric_sample_evaluations(&self) -> usize {
        self.source_metric_sample_evaluations
    }
}

struct LoadedSourceState {
    loaded: LoadedInput,
    config: Arc<LoadedCollectionConfigForPreparation>,
}

struct SelectedMember {
    id: CollectionLogicalIdV1,
    source_index: usize,
    clip_index: usize,
    clip_reference: ContactClipReferenceV1,
    fragment: ContactFragmentV1,
    fragment_input: InputIdentity,
    root_motion: FootCycleRootMotionEvidenceV1,
}

struct PendingSelectedMember {
    id: CollectionLogicalIdV1,
    source_index: usize,
    clip_index: usize,
    clip_reference: ContactClipReferenceV1,
    fragment: ContactFragmentV1,
    fragment_input: InputIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathRole {
    Declaration,
    Source,
    Config,
    Contact,
}

struct SeenPath {
    role: PathRole,
    declaration: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ExistingFileIdentity {
    #[cfg(unix)]
    Unix {
        device: u64,
        inode: u64,
    },
    #[cfg(windows)]
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
    Canonical(PathBuf),
}

/// Prepare every declared input and member without writing or publishing.
pub(crate) fn prepare_foot_cycle_parameterization_v1(
    manifest_path: &Path,
    parameterization_path: &Path,
) -> Result<PreparedFootCycleCollectionV1, FootCycleSourcePrepError> {
    prepare_foot_cycle_parameterization_v1_with_runtime(
        manifest_path,
        parameterization_path,
        &mut ProductionFootCyclePreparationRuntime,
    )
}

fn prepare_foot_cycle_parameterization_v1_with_runtime(
    manifest_path: &Path,
    parameterization_path: &Path,
    runtime: &mut impl FootCyclePreparationRuntime,
) -> Result<PreparedFootCycleCollectionV1, FootCycleSourcePrepError> {
    let loaded_manifest = load_collection_manifest_with_identity(manifest_path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    let loaded_parameterization =
        load_foot_cycle_parameterization_with_identity(parameterization_path)
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    let manifest = loaded_manifest.manifest;
    let parameterization = loaded_parameterization.parameterization;

    // Pure declaration validation must precede every member-reachable path.
    // The planner repeats this check after exact evidence is available.
    validate_foot_cycle_manifest_binding_v1(&parameterization, &manifest, &loaded_manifest.input)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanRefused))?;

    let mut reachable_source_keys = std::collections::BTreeSet::new();
    for member in parameterization.members() {
        let clip = manifest
            .clips()
            .binary_search_by(|clip| clip.id().cmp(member.id()))
            .ok()
            .and_then(|index| manifest.clips().get(index))
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanRefused))?;
        reachable_source_keys.insert(clip.source().as_str());
    }
    let reachable_sources = manifest
        .sources()
        .iter()
        .filter(|source| reachable_source_keys.contains(source.key().as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let manifest_resolver = CollectionPathResolver::new(manifest_path, manifest.input_root())
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    let parameterization_resolver = CollectionPathResolver::new(parameterization_path, None)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    let source_resolutions = manifest_resolver
        .resolve_sources(&reachable_sources)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::UnsafePathSet))?;

    let mut config_resolutions = Vec::with_capacity(reachable_sources.len());
    for source in &reachable_sources {
        config_resolutions.push(
            manifest_resolver
                .resolve_config(source.config())
                .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?,
        );
    }
    let mut fragment_paths = Vec::with_capacity(parameterization.members().len());
    for member in parameterization.members() {
        fragment_paths.push(
            parameterization_resolver
                .resolve_required_control_file(member.contact_fragment())
                .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?,
        );
    }
    let output_directory = parameterization_resolver
        .resolve_absent_control_directory(parameterization.output_directory())
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;

    preflight_path_set(
        manifest_path,
        parameterization_path,
        &reachable_sources,
        &source_resolutions,
        &config_resolutions,
        &fragment_paths,
        &output_directory,
    )?;
    preflight_source_byte_budget(&reachable_sources, &source_resolutions)?;
    preflight_config_byte_budget(&config_resolutions)?;
    preflight_contact_byte_budget(&fragment_paths)?;

    let mut fragments = Vec::with_capacity(fragment_paths.len());
    let mut actual_contact_bytes = 0u64;
    for resolved in &fragment_paths {
        let bytes = read_bounded(resolved.path(), CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES as u64)?;
        actual_contact_bytes = checked_contact_budget(actual_contact_bytes, bytes.len() as u64)?;
        let input = InputIdentity::from_bytes(&bytes);
        let fragment = ContactFragmentV1::read_json(&bytes)
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactInvalid))?;
        fragments.push((input, fragment));
    }

    // Each distinct canonical config is captured exactly once. Sources that
    // share it retain the same immutable snapshot, so a concurrent control
    // mutation cannot split their identity or normalized semantics.
    let mut configs = BTreeMap::<Option<PathBuf>, Arc<LoadedCollectionConfigForPreparation>>::new();
    let mut actual_config_bytes = 0u64;
    for resolution in &config_resolutions {
        let key = config_cache_key(resolution);
        if let std::collections::btree_map::Entry::Vacant(entry) = configs.entry(key) {
            let config = load_collection_config_for_preparation(resolution.clone())
                .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
            actual_config_bytes = checked_config_budget(
                actual_config_bytes,
                config.input.as_ref().map_or(0, InputIdentity::bytes),
            )?;
            entry.insert(Arc::new(config));
        }
    }

    // Config and source loading starts only after every declared path and byte
    // budget has passed. No output directory or temporary file exists yet.
    let mut loaded_sources = Vec::with_capacity(reachable_sources.len());
    let mut source_indices = BTreeMap::new();
    let mut actual_retained_bytes = 0u64;
    for (index, (source, config_resolution)) in reachable_sources
        .iter()
        .zip(&config_resolutions)
        .enumerate()
    {
        let config = Arc::clone(
            configs
                .get(&config_cache_key(config_resolution))
                .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?,
        );
        let resolution = source_resolutions
            .get(source.key().as_str())
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
        let CollectionSourceResolution::Ready(path) = resolution else {
            return Err(FootCycleSourcePrepError::new(
                FootCycleSourcePrepKind::SourceUnavailable,
            ));
        };
        let loaded = load_with_config_for_producer_bounded(
            path.path(),
            &config.loaded,
            COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
        )
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceLoad))?;
        runtime.validate_source_document(loaded.document())?;
        if source
            .expected_sha256()
            .is_some_and(|expected| expected.as_str() != loaded.input().sha256())
        {
            return Err(FootCycleSourcePrepError::new(
                FootCycleSourcePrepKind::SourceDigestMismatch,
            ));
        }
        complete_closure(loaded.dependency_closure(), loaded.input()).map_err(|_| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::IncompleteClosure)
        })?;
        let external_bytes = closure_external_bytes(loaded.dependency_closure())?;
        let decoded_bytes = retained_document_bytes(loaded.document())?;
        actual_retained_bytes = checked_retained_source_budget(
            actual_retained_bytes,
            loaded.input().bytes(),
            external_bytes,
            decoded_bytes,
        )?;
        source_indices.insert(source.key().as_str().to_owned(), index);
        loaded_sources.push(LoadedSourceState { loaded, config });
    }

    let mut pending_selected = Vec::with_capacity(parameterization.members().len());
    for ((declaration, fragment_path), (fragment_input, fragment)) in parameterization
        .members()
        .iter()
        .zip(&fragment_paths)
        .zip(fragments)
    {
        let clip = manifest
            .clips()
            .iter()
            .find(|clip| clip.id() == declaration.id())
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        let source_index = *source_indices
            .get(clip.source().as_str())
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        let source = loaded_sources
            .get(source_index)
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        let clip_index = resolve_collection_take_witness(
            source.loaded.source_facts().clips(),
            source.loaded.document().clips.len(),
            clip.take_index() as usize,
            clip.take_name(),
        )
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        let clip_reference = ContactClipReferenceV1::collection(
            clip.id().as_str(),
            clip.source().as_str(),
            clip.take_index(),
            clip.take_name(),
        )
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        let loaded_clip = source
            .loaded
            .document()
            .clips
            .get(clip_index)
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch))?;
        if fragment.duration_s() != loaded_clip.duration_s {
            return Err(FootCycleSourcePrepError::new(
                FootCycleSourcePrepKind::DurationMismatch,
            ));
        }
        pending_selected.push(PendingSelectedMember {
            id: declaration.id().clone(),
            source_index,
            clip_index,
            clip_reference,
            fragment,
            fragment_input,
        });
        debug_assert_eq!(
            fragment_path.declared(),
            declaration.contact_fragment().as_str()
        );
    }

    let mut metric_work = Vec::with_capacity(pending_selected.len());
    for member in &pending_selected {
        let source = loaded_sources.get(member.source_index).ok_or_else(|| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable)
        })?;
        metric_work.push(runtime.metric_grid_work(&source.loaded, member.clip_index)?);
    }
    let ((selected, evidence), source_metric_work) =
        build_after_metric_grid_batch_preflight(&metric_work, || {
            let mut selected = Vec::with_capacity(pending_selected.len());
            let mut evidence = Vec::with_capacity(pending_selected.len());
            for (member, declaration) in
                pending_selected.into_iter().zip(parameterization.members())
            {
                let source = loaded_sources.get(member.source_index).ok_or_else(|| {
                    FootCycleSourcePrepError::new(FootCycleSourcePrepKind::TakeMismatch)
                })?;
                let closure_identity =
                    complete_closure(source.loaded.dependency_closure(), source.loaded.input())
                        .map_err(|_| {
                            FootCycleSourcePrepError::new(
                                FootCycleSourcePrepKind::IncompleteClosure,
                            )
                        })?;
                let root_motion = derive_root_motion(
                    &source.loaded,
                    &source.config.loaded,
                    member.clip_index,
                    member.clip_reference.clone(),
                    closure_identity,
                    runtime,
                )?;
                evidence.push(FootCycleMemberEvidenceV1::new(
                    member.id.clone(),
                    declaration.contact_fragment().clone(),
                    member.fragment_input.clone(),
                    member.fragment.clone(),
                    root_motion.clone(),
                ));
                selected.push(SelectedMember {
                    id: member.id,
                    source_index: member.source_index,
                    clip_index: member.clip_index,
                    clip_reference: member.clip_reference,
                    fragment: member.fragment,
                    fragment_input: member.fragment_input,
                    root_motion,
                });
            }
            Ok((selected, evidence))
        })?;

    let plan = plan_foot_cycle_parameterization_v1(
        &parameterization,
        loaded_parameterization.input.clone(),
        &manifest,
        loaded_manifest.input.clone(),
        &evidence,
    )
    .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanRefused))?;

    let mut candidate_preflights = Vec::with_capacity(selected.len());
    for (member, member_plan) in selected.iter().zip(plan.members()) {
        let source = &loaded_sources
            .get(member.source_index)
            .ok_or_else(|| {
                FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanBindingMismatch)
            })?
            .loaded;
        cross_check_plan(member, member_plan, source)?;
        let input_clip = source
            .document()
            .clips
            .get(member.clip_index)
            .ok_or_else(|| {
                FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanBindingMismatch)
            })?;
        candidate_preflights.push(runtime.preflight_candidate(input_clip, member_plan)?);
    }

    let prepared_members = build_after_candidate_batch_preflight(&candidate_preflights, || {
        let mut prepared_members = Vec::with_capacity(selected.len());
        for (member, member_plan) in selected.iter().zip(plan.members()) {
            let source = &loaded_sources
                .get(member.source_index)
                .ok_or_else(|| {
                    FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanBindingMismatch)
                })?
                .loaded;
            let input_clip = source
                .document()
                .clips
                .get(member.clip_index)
                .ok_or_else(|| {
                    FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanBindingMismatch)
                })?;
            let candidate_clip = runtime.build_candidate(input_clip, member_plan)?;
            if candidate_clip.duration_s != input_clip.duration_s {
                return Err(FootCycleSourcePrepError::new(
                    FootCycleSourcePrepKind::DurationMismatch,
                ));
            }
            let transformed_extensions =
                prepare_stance_extensions(member_plan.operation(), member.fragment.extensions())?;
            prepared_members.push(PreparedFootCycleMemberV1 {
                id: member.id.clone(),
                source_index: member.source_index,
                clip_index: member.clip_index,
                candidate_clip,
                contact_transform: PreparedContactTransformV1 {
                    operation: member_plan.operation().clone(),
                    input_fragment: member.fragment.clone(),
                    current_input_artifact: source.input().clone(),
                    current_input_dependency_closure: source.dependency_closure().clone(),
                    transformed_extensions,
                },
            });
        }
        Ok(prepared_members)
    })?;

    let sources = loaded_sources
        .into_iter()
        .zip(&reachable_sources)
        .map(|(source, declaration)| PreparedFootCycleSourceV1 {
            key: declaration.key().as_str().to_owned(),
            artifact: source.loaded.input().clone(),
            config: source.config,
            dependency_closure: source.loaded.dependency_closure().clone(),
            document: source.loaded.into_document(),
        })
        .collect();
    Ok(PreparedFootCycleCollectionV1 {
        manifest_input: loaded_manifest.input,
        parameterization_input: loaded_parameterization.input,
        output_directory: output_directory.path().to_path_buf(),
        sources,
        members: prepared_members,
        plan,
        source_metric_pose_cells: source_metric_work.pose_cells,
        source_metric_sample_evaluations: source_metric_work.sample_evaluations,
    })
}

fn build_after_candidate_batch_preflight<T>(
    preflights: &[CandidateBatchWork],
    build: impl FnOnce() -> Result<T, FootCycleSourcePrepError>,
) -> Result<T, FootCycleSourcePrepError> {
    let mut total = CandidateBatchWork::default();
    for preflight in preflights {
        total = checked_candidate_batch_work(total, *preflight).ok_or_else(|| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ClipTransformRefused)
        })?;
    }
    build()
}

fn checked_candidate_batch_work(
    total: CandidateBatchWork,
    next: CandidateBatchWork,
) -> Option<CandidateBatchWork> {
    let total = CandidateBatchWork {
        keys: total.keys.checked_add(next.keys)?,
        values: total.values.checked_add(next.values)?,
        bytes: total.bytes.checked_add(next.bytes)?,
        work: total.work.checked_add(next.work)?,
    };
    if total.keys > MAX_AGGREGATE_CANDIDATE_KEYS
        || total.values > MAX_AGGREGATE_CANDIDATE_VALUES
        || total.bytes > MAX_AGGREGATE_CANDIDATE_BYTES
        || total.work > MAX_AGGREGATE_CANDIDATE_WORK
    {
        return None;
    }
    Some(total)
}

fn preflight_path_set(
    manifest_path: &Path,
    parameterization_path: &Path,
    sources: &[animsmith_core::CollectionSourceV1],
    source_resolutions: &BTreeMap<String, CollectionSourceResolution>,
    config_resolutions: &[CollectionConfigResolution],
    fragment_paths: &[CollectionResolvedPath],
    output_directory: &CollectionResolvedPath,
) -> Result<(), FootCycleSourcePrepError> {
    let mut seen = BTreeMap::<ExistingFileIdentity, SeenPath>::new();
    let mut canonical_paths = BTreeMap::<PathBuf, ExistingFileIdentity>::new();
    retain_path(
        &mut seen,
        &mut canonical_paths,
        canonical_regular_argument(manifest_path)?,
        PathRole::Declaration,
        "manifest",
    )?;
    retain_path(
        &mut seen,
        &mut canonical_paths,
        canonical_regular_argument(parameterization_path)?,
        PathRole::Declaration,
        "parameterization",
    )?;
    for (source, config) in sources.iter().zip(config_resolutions) {
        let resolution = source_resolutions
            .get(source.key().as_str())
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
        let CollectionSourceResolution::Ready(path) = resolution else {
            return Err(FootCycleSourcePrepError::new(
                FootCycleSourcePrepKind::SourceUnavailable,
            ));
        };
        retain_path(
            &mut seen,
            &mut canonical_paths,
            path.path().to_path_buf(),
            PathRole::Source,
            source.path().as_str(),
        )?;
        if let CollectionConfigResolution::Explicit(path) = config {
            retain_path(
                &mut seen,
                &mut canonical_paths,
                path.path().to_path_buf(),
                PathRole::Config,
                path.declared(),
            )?;
        }
    }
    for path in fragment_paths {
        retain_path(
            &mut seen,
            &mut canonical_paths,
            path.path().to_path_buf(),
            PathRole::Contact,
            path.declared(),
        )?;
    }
    if canonical_paths.contains_key(output_directory.path()) {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::UnsafePathSet,
        ));
    }
    Ok(())
}

fn retain_path(
    seen: &mut BTreeMap<ExistingFileIdentity, SeenPath>,
    canonical_paths: &mut BTreeMap<PathBuf, ExistingFileIdentity>,
    canonical: PathBuf,
    role: PathRole,
    declaration: &str,
) -> Result<(), FootCycleSourcePrepError> {
    let identity = existing_file_identity(&canonical)?;
    if let Some(existing) = seen.get(&identity) {
        // Multiple source rows may deliberately share one exact config basis.
        if role == PathRole::Config
            && existing.role == PathRole::Config
            && existing.declaration == declaration
        {
            return Ok(());
        }
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::UnsafePathSet,
        ));
    }
    if canonical_paths.contains_key(&canonical) {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::UnsafePathSet,
        ));
    }
    seen.insert(
        identity.clone(),
        SeenPath {
            role,
            declaration: declaration.to_owned(),
        },
    );
    canonical_paths.insert(canonical, identity);
    Ok(())
}

#[cfg(unix)]
fn existing_file_identity(path: &Path) -> Result<ExistingFileIdentity, FootCycleSourcePrepError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::metadata(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    Ok(ExistingFileIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn existing_file_identity(path: &Path) -> Result<ExistingFileIdentity, FootCycleSourcePrepError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let file = fs::File::open(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: `file` keeps this valid owned handle alive for the call and
    // `information` is a writable instance of the exact Win32 output type.
    let succeeded = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut information) };
    if succeeded == 0 {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::Control,
        ));
    }
    Ok(ExistingFileIdentity::Windows {
        volume_serial_number: information.dwVolumeSerialNumber,
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
    })
}

#[cfg(not(any(unix, windows)))]
fn existing_file_identity(path: &Path) -> Result<ExistingFileIdentity, FootCycleSourcePrepError> {
    fs::metadata(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    Ok(ExistingFileIdentity::Canonical(path.to_path_buf()))
}

fn canonical_regular_argument(path: &Path) -> Result<PathBuf, FootCycleSourcePrepError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::UnsafePathSet,
        ));
    }
    fs::canonicalize(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))
}

fn config_cache_key(resolution: &CollectionConfigResolution) -> Option<PathBuf> {
    match resolution {
        CollectionConfigResolution::Default => None,
        CollectionConfigResolution::Explicit(path) => Some(path.path().to_path_buf()),
    }
}

fn preflight_config_byte_budget(
    resolutions: &[CollectionConfigResolution],
) -> Result<(), FootCycleSourcePrepError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for resolution in resolutions {
        let CollectionConfigResolution::Explicit(path) = resolution else {
            continue;
        };
        if seen.insert(path.path()) {
            let bytes = fs::metadata(path.path())
                .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::Control))?
                .len();
            total = checked_config_budget(total, bytes)?;
        }
    }
    Ok(())
}

fn checked_config_budget(total: u64, next: u64) -> Result<u64, FootCycleSourcePrepError> {
    total
        .checked_add(next)
        .filter(|sum| *sum <= MAX_AGGREGATE_CONFIG_BYTES)
        .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))
}

fn preflight_source_byte_budget(
    sources: &[animsmith_core::CollectionSourceV1],
    resolutions: &BTreeMap<String, CollectionSourceResolution>,
) -> Result<(), FootCycleSourcePrepError> {
    let mut total = 0u64;
    for source in sources {
        let Some(CollectionSourceResolution::Ready(path)) = resolutions.get(source.key().as_str())
        else {
            return Err(FootCycleSourcePrepError::new(
                FootCycleSourcePrepKind::SourceUnavailable,
            ));
        };
        let bytes = fs::metadata(path.path())
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceUnavailable))?
            .len();
        total = checked_source_budget(total, bytes)?;
    }
    Ok(())
}

fn checked_source_budget(total: u64, next: u64) -> Result<u64, FootCycleSourcePrepError> {
    if next > COLLECTION_OUTPUT_MAX_SOURCE_BYTES {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::SourceBudget,
        ));
    }
    total
        .checked_add(next)
        .filter(|sum| *sum <= COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES)
        .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))
}

fn closure_external_bytes(closure: &DependencyClosureV1) -> Result<u64, FootCycleSourcePrepError> {
    closure
        .external_resources()
        .iter()
        .try_fold(0u64, |total, resource| {
            total
                .checked_add(resource.identity().bytes())
                .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))
        })
}

fn checked_retained_source_budget(
    aggregate: u64,
    primary: u64,
    external: u64,
    decoded: u64,
) -> Result<u64, FootCycleSourcePrepError> {
    if primary > COLLECTION_OUTPUT_MAX_SOURCE_BYTES || decoded > MAX_RETAINED_DECODED_SOURCE_BYTES {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::SourceBudget,
        ));
    }
    aggregate
        .checked_add(primary)
        .and_then(|sum| sum.checked_add(external))
        .and_then(|sum| sum.checked_add(decoded))
        .filter(|sum| *sum <= COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES)
        .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))
}

/// Count the allocation payload retained by a normalized document: every
/// owned vector capacity, string capacity, and embedded image capacity at the
/// current target's element sizes. Allocator bookkeeping is deliberately not
/// claimed, but spare capacity is included rather than hidden behind `len`.
fn retained_document_bytes(document: &Document) -> Result<u64, FootCycleSourcePrepError> {
    fn add(total: &mut u64, bytes: usize) -> Result<(), FootCycleSourcePrepError> {
        let bytes = u64::try_from(bytes)
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))?;
        *total = total
            .checked_add(bytes)
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))?;
        Ok(())
    }
    #[allow(
        clippy::ptr_arg,
        reason = "Vec capacity is the retained allocation basis"
    )]
    fn add_vec<T>(total: &mut u64, values: &Vec<T>) -> Result<(), FootCycleSourcePrepError> {
        let bytes = values
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::SourceBudget))?;
        add(total, bytes)
    }
    #[allow(
        clippy::ptr_arg,
        reason = "String capacity is the retained allocation basis"
    )]
    fn add_text(total: &mut u64, value: &String) -> Result<(), FootCycleSourcePrepError> {
        add(total, value.capacity())
    }

    let mut total = 0u64;
    add_vec(&mut total, &document.skeleton.bones)?;
    for bone in &document.skeleton.bones {
        add_text(&mut total, &bone.name)?;
    }
    add_vec(&mut total, &document.clips)?;
    for clip in &document.clips {
        add_text(&mut total, &clip.name)?;
        add_vec(&mut total, &clip.tracks)?;
        for track in &clip.tracks {
            add_vec(&mut total, &track.times)?;
            match &track.values {
                animsmith_core::TrackValues::Vec3s(values) => add_vec(&mut total, values)?,
                animsmith_core::TrackValues::Quats(values) => add_vec(&mut total, values)?,
            }
        }
    }
    if let Some(path) = &document.source.path {
        add_text(&mut total, path)?;
    }
    if let Some(format) = &document.source.format {
        add_text(&mut total, format)?;
    }

    let assets = &document.assets;
    add_vec(&mut total, &assets.meshes)?;
    for mesh in &assets.meshes {
        add_text(&mut total, &mesh.name)?;
        add_vec(&mut total, &mesh.primitives)?;
        for primitive in &mesh.primitives {
            add_vec(&mut total, &primitive.indices)?;
            add_vec(&mut total, &primitive.positions)?;
            add_vec(&mut total, &primitive.normals)?;
            add_vec(&mut total, &primitive.uvs)?;
            add_vec(&mut total, &primitive.joints)?;
            add_vec(&mut total, &primitive.weights)?;
            add_vec(&mut total, &primitive.additional_influence_sets)?;
        }
    }
    add_vec(&mut total, &assets.instances)?;
    for instance in &assets.instances {
        add_vec(&mut total, &instance.skin_joints)?;
        add_vec(&mut total, &instance.skin_ibms)?;
    }
    add_vec(&mut total, &assets.materials)?;
    for material in &assets.materials {
        add_text(&mut total, &material.name)?;
        if let Some(texture) = &material.base_color_texture {
            add_vec(&mut total, &texture.bytes)?;
            add_text(&mut total, &texture.mime)?;
        }
        if let Some(texture) = &material.normal_texture {
            add_vec(&mut total, &texture.texture.bytes)?;
            add_text(&mut total, &texture.texture.mime)?;
        }
        if let Some(texture) = &material.metallic_roughness_texture {
            add_vec(&mut total, &texture.bytes)?;
            add_text(&mut total, &texture.mime)?;
        }
        if let Some(texture) = &material.occlusion_texture {
            add_vec(&mut total, &texture.texture.bytes)?;
            add_text(&mut total, &texture.texture.mime)?;
        }
    }
    add_vec(&mut total, &assets.scenes)?;
    for scene in &assets.scenes {
        if let Some(name) = &scene.name {
            add_text(&mut total, name)?;
        }
        add_vec(&mut total, &scene.roots)?;
    }

    let resources = &assets.material_resources;
    add_vec(&mut total, &resources.materials)?;
    for material in &resources.materials {
        if let Some(name) = &material.name {
            add_text(&mut total, name)?;
        }
        add_vec(&mut total, &material.texture_bindings)?;
    }
    add_vec(&mut total, &resources.textures)?;
    for texture in &resources.textures {
        if let Some(name) = &texture.name {
            add_text(&mut total, name)?;
        }
    }
    add_vec(&mut total, &resources.images)?;
    for image in &resources.images {
        for text in [
            image.name.as_ref(),
            image.declared_mime_type.as_ref(),
            image.leading_magic_hex.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            add_text(&mut total, text)?;
        }
    }

    let source_skeleton = &assets.source_skeleton;
    add_vec(&mut total, &source_skeleton.nodes)?;
    for node in &source_skeleton.nodes {
        if let Some(name) = &node.name {
            add_text(&mut total, name)?;
        }
        add_vec(&mut total, &node.scene_root_indices)?;
    }
    add_vec(&mut total, &source_skeleton.skins)?;
    for skin in &source_skeleton.skins {
        if let Some(name) = &skin.name {
            add_text(&mut total, name)?;
        }
        add_vec(&mut total, &skin.joint_source_node_indices)?;
        add_vec(&mut total, &skin.inverse_bind_accessor.matrices)?;
        add_vec(&mut total, &skin.attachments)?;
    }
    Ok(total)
}

fn preflight_contact_byte_budget(
    paths: &[CollectionResolvedPath],
) -> Result<(), FootCycleSourcePrepError> {
    let mut total = 0u64;
    for path in paths {
        let bytes = fs::metadata(path.path())
            .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactRead))?
            .len();
        total = checked_contact_budget(total, bytes)?;
    }
    Ok(())
}

fn checked_contact_budget(total: u64, next: u64) -> Result<u64, FootCycleSourcePrepError> {
    if next > CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES as u64 {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::ContactBudget,
        ));
    }
    total
        .checked_add(next)
        .filter(|sum| {
            *sum <= animsmith_core::FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES
        })
        .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactBudget))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, FootCycleSourcePrepError> {
    let file = fs::File::open(path)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactRead))?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::ContactRead))?;
    if bytes.len() as u64 > limit {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::ContactBudget,
        ));
    }
    Ok(bytes)
}

fn metric_grid_work_for_clip(
    loaded: &LoadedInput,
    clip_index: usize,
) -> Result<MetricGridWork, FootCycleSourcePrepError> {
    let clip = loaded.document().clips.get(clip_index).ok_or_else(|| {
        FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable)
    })?;
    let frame_count = animsmith_core::metrics::metric_frame_count(clip).ok_or_else(|| {
        FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable)
    })?;
    checked_metric_grid_work(
        frame_count,
        loaded.document().skeleton.bones.len(),
        clip.tracks.len(),
    )
    .ok_or_else(|| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable))
}

fn build_after_metric_grid_batch_preflight<T>(
    members: &[MetricGridWork],
    build: impl FnOnce() -> Result<T, FootCycleSourcePrepError>,
) -> Result<(T, MetricGridWork), FootCycleSourcePrepError> {
    let mut total = MetricGridWork {
        pose_cells: 0,
        sample_evaluations: 0,
    };
    for member in members {
        total = checked_aggregate_metric_work(total, *member).ok_or_else(|| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::RootEvidenceUnavailable)
        })?;
    }
    Ok((build()?, total))
}

fn derive_root_motion(
    loaded: &LoadedInput,
    config: &LoadedConfig,
    clip_index: usize,
    clip: ContactClipReferenceV1,
    closure_identity: DependencyClosureIdentityV1,
    runtime: &mut impl FootCyclePreparationRuntime,
) -> Result<FootCycleRootMotionEvidenceV1, FootCycleSourcePrepError> {
    let binding = FootCycleRootMotionBindingV1::new(loaded.input().clone(), closure_identity, clip);
    let roles = resolve_configured_roles(&loaded.document().skeleton, &config.config.rig);
    if matches!(
        roles.outcome(),
        ResolutionOutcome::AmbiguousExactMatch
            | ResolutionOutcome::AmbiguousFoldedMatch
            | ResolutionOutcome::RoleCollision
            | ResolutionOutcome::AmbiguousProfile
    ) {
        return Ok(FootCycleRootMotionEvidenceV1::ambiguous(binding));
    }
    let Some(root) = roles.get(Role::Root).or_else(|| roles.get(Role::Hips)) else {
        return Ok(FootCycleRootMotionEvidenceV1::missing(binding));
    };
    let grid = runtime.build_metric_grid(loaded, clip_index)?;
    let Some(trajectory) = root_trajectory_metrics(&grid, root) else {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::RootEvidenceUnavailable,
        ));
    };
    let (Some(translation), Some(yaw)) = (trajectory.translation, trajectory.yaw) else {
        return Ok(FootCycleRootMotionEvidenceV1::non_finite(binding));
    };
    let endpoint_x = translation.horizontal_displacement_x_m;
    let endpoint_z = translation.horizontal_displacement_z_m;
    let accumulated_yaw = yaw.unwrapped_yaw_deg;
    if !endpoint_x.is_finite() || !endpoint_z.is_finite() || !accumulated_yaw.is_finite() {
        return Ok(FootCycleRootMotionEvidenceV1::non_finite(binding));
    }
    Ok(FootCycleRootMotionEvidenceV1::measured(
        binding,
        endpoint_x,
        endpoint_z,
        accumulated_yaw,
    ))
}

fn checked_aggregate_metric_work(
    total: MetricGridWork,
    next: MetricGridWork,
) -> Option<MetricGridWork> {
    Some(MetricGridWork {
        pose_cells: total
            .pose_cells
            .checked_add(next.pose_cells)
            .filter(|value| *value <= MAX_AGGREGATE_METRIC_GRID_WORK)?,
        sample_evaluations: total
            .sample_evaluations
            .checked_add(next.sample_evaluations)
            .filter(|value| *value <= MAX_AGGREGATE_METRIC_GRID_WORK)?,
    })
}

fn cross_check_plan(
    selected: &SelectedMember,
    plan: &animsmith_core::FootCycleMemberPlanV1,
    source: &LoadedInput,
) -> Result<(), FootCycleSourcePrepError> {
    let closure_identity = complete_closure(source.dependency_closure(), source.input())
        .map_err(|_| FootCycleSourcePrepError::new(FootCycleSourcePrepKind::IncompleteClosure))?;
    let duration = source
        .document()
        .clips
        .get(selected.clip_index)
        .map(|clip| clip.duration_s)
        .ok_or_else(|| {
            FootCycleSourcePrepError::new(FootCycleSourcePrepKind::PlanBindingMismatch)
        })?;
    if plan.id() != &selected.id
        || plan.input().artifact() != source.input()
        || plan.input().dependency_closure_identity() != &closure_identity
        || plan.input().fragment() != &selected.fragment_input
        || plan.operation().output_duration_s() != Some(duration)
        || !root_motion_matches(&selected.root_motion, plan.root_motion())
        || selected.fragment.clip() != &selected.clip_reference
        || selected.root_motion.binding().artifact() != source.input()
        || selected.root_motion.binding().dependency_closure_identity() != &closure_identity
        || selected.root_motion.binding().clip() != &selected.clip_reference
    {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::PlanBindingMismatch,
        ));
    }
    Ok(())
}

fn root_motion_matches(
    selected: &FootCycleRootMotionEvidenceV1,
    planned: &FootCycleRootMotionEvidenceV1,
) -> bool {
    selected == planned
}

fn prepare_stance_extensions(
    operation: &ContactTransformOperationV1,
    extensions: &[ContactExtensionV1],
) -> Result<Vec<ContactExtensionV1>, FootCycleSourcePrepError> {
    if !matches!(
        operation,
        ContactTransformOperationV1::TimeWarp { version: 1, .. }
    ) {
        return Err(FootCycleSourcePrepError::new(
            FootCycleSourcePrepKind::ExtensionTransformRefused,
        ));
    }
    extensions
        .iter()
        .map(|extension| {
            transform_contact_support_detector_extension_time_warp_v1(extension, operation).map_err(
                |_| {
                    FootCycleSourcePrepError::new(
                        FootCycleSourcePrepKind::ExtensionTransformRefused,
                    )
                },
            )
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use animsmith_core::glam::{Mat4, Vec3};
    use animsmith_core::model;
    use animsmith_core::{
        CONTACT_SUPPORT_DETECTOR_V1_ID, ContactEventV1, ContactEventWindowV1, ContactPhaseV1,
        ContactRoleV1, DependencyClosureBuilderV1, DependencyResourceKeyV1,
        FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG,
        FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M, ResourceKeySyntaxV1,
        SourceSetCoverageV1, TrackValues,
    };
    use serde_json::json;
    use std::collections::VecDeque;
    use tempfile::TempDir;

    #[derive(Clone, Copy)]
    enum ConfigMode {
        Complete,
        HipsOnly,
        MissingRoot,
        AmbiguousRoot,
    }

    #[derive(Clone, Copy)]
    struct FixtureOptions {
        end_x_a: f32,
        end_x_b: f32,
        end_z_b: f32,
        yaw_deg_b: f32,
        nonfinite_b: bool,
        duplicate_take_name_b: bool,
        nonconstant_cubic_b: bool,
        config: ConfigMode,
        take_index_b: u32,
        take_name_b: &'static str,
        fragment_duration_b: f64,
    }

    impl Default for FixtureOptions {
        fn default() -> Self {
            Self {
                end_x_a: 0.0,
                end_x_b: 0.0,
                end_z_b: 0.0,
                yaw_deg_b: 0.0,
                nonfinite_b: false,
                duplicate_take_name_b: false,
                nonconstant_cubic_b: false,
                config: ConfigMode::Complete,
                take_index_b: 0,
                take_name_b: "Take 001",
                fragment_duration_b: 1.0,
            }
        }
    }

    struct Fixture {
        _directory: TempDir,
        root: PathBuf,
        manifest: PathBuf,
        parameterization: PathBuf,
    }

    impl Fixture {
        fn create(options: FixtureOptions) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().to_path_buf();
            fs::create_dir(root.join("assets")).unwrap();
            fs::create_dir(root.join("contacts")).unwrap();
            fs::create_dir(root.join("generated")).unwrap();
            let ambiguous = matches!(options.config, ConfigMode::AmbiguousRoot);
            write_source(
                &root.join("assets/a.gltf"),
                "a.bin",
                options.end_x_a,
                0.0,
                0.0,
                false,
                ambiguous,
                false,
                false,
            );
            write_source(
                &root.join("assets/b.gltf"),
                "b.bin",
                options.end_x_b,
                options.end_z_b,
                options.yaw_deg_b,
                options.nonfinite_b,
                ambiguous,
                options.duplicate_take_name_b,
                options.nonconstant_cubic_b,
            );
            let rig = match options.config {
                ConfigMode::Complete => {
                    "[rig]\nprofile = \"auto\"\nroles = { root = \"R0\", hips = \"H0\", left_foot = \"LF0\", right_foot = \"RF0\" }\n"
                }
                ConfigMode::HipsOnly => {
                    "[rig]\nprofile = \"auto\"\nroles = { hips = \"H0\", left_foot = \"LF0\", right_foot = \"RF0\" }\n"
                }
                ConfigMode::MissingRoot => {
                    "[rig]\nprofile = \"auto\"\nroles = { left_foot = \"LF0\", right_foot = \"RF0\" }\n"
                }
                ConfigMode::AmbiguousRoot => "[rig]\nprofile = \"ue-mannequin\"\n",
            };
            let config = format!(
                "{rig}\n[clips.\"Take 001\"]\nloop = true\n\n[gait_groups.fixture]\nclips = [\"Take 001\", \"Take 002\"]\nmax_gait_phase_spread = 0.125\nmin_lr_amplitude_m = 0.02\n"
            );
            fs::write(root.join("config.toml"), config).unwrap();
            let a_bytes = fs::read(root.join("assets/a.gltf")).unwrap();
            let b_bytes = fs::read(root.join("assets/b.gltf")).unwrap();
            let manifest_text = format!(
                r#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example"
input_root = "assets"

[[sources]]
key = "a"
path = "a.gltf"
config = "config.toml"
expected_sha256 = "{}"

[[sources]]
key = "b"
path = "b.gltf"
config = "config.toml"
expected_sha256 = "{}"

[[clips]]
id = "com.example/a"
source = "a"
take_index = 0
take_name = "Take 001"

[[clips]]
id = "com.example/b"
source = "b"
take_index = {}
take_name = "{}"

[[runtime_sets]]
id = "com.example/sets/walk"
kind = "gait-group"
members = ["com.example/a", "com.example/b"]
"#,
                InputIdentity::from_bytes(&a_bytes).sha256(),
                InputIdentity::from_bytes(&b_bytes).sha256(),
                options.take_index_b,
                options.take_name_b,
            );
            let manifest = root.join("collection.toml");
            fs::write(&manifest, manifest_text.as_bytes()).unwrap();
            let loaded_config = crate::load_config(Some(&root.join("config.toml"))).unwrap();
            write_fragment(
                &root,
                "a",
                "com.example/a",
                0,
                "Take 001",
                1.0,
                &loaded_config,
                &[
                    (ContactRoleV1::LeftFoot, 0.125, 0.25),
                    (ContactRoleV1::RightFoot, 0.625, 0.75),
                ],
            );
            write_fragment(
                &root,
                "b",
                "com.example/b",
                options.take_index_b,
                options.take_name_b,
                options.fragment_duration_b,
                &loaded_config,
                &[
                    (ContactRoleV1::LeftFoot, 0.25, 0.375),
                    (ContactRoleV1::RightFoot, 0.75, 0.875),
                ],
            );
            let manifest_input = InputIdentity::from_bytes(&fs::read(&manifest).unwrap());
            let parameterization_text = format!(
                r#"schema = "urn:animsmith:schema:foot-cycle-parameterization:1"
schema_version = 1
runtime_set_id = "com.example/sets/walk"
reference_member = "com.example/a"
output_directory = "generated/aligned"
minimum_segment_slope = 0.25
maximum_segment_slope = 4.0

[proof]
max_gait_phase_spread = 0.08
min_lr_amplitude_m = 0.05
max_contact_boundary_phase_error = 0.01

[manifest]
schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "com.example"

[manifest.input]
sha256 = "{}"
bytes = {}

[[members]]
id = "com.example/a"
contact_fragment = "contacts/a.json"

[[members]]
id = "com.example/b"
contact_fragment = "contacts/b.json"
"#,
                manifest_input.sha256(),
                manifest_input.bytes(),
            );
            let parameterization = root.join("foot-cycle.toml");
            fs::write(&parameterization, parameterization_text).unwrap();
            Self {
                _directory: directory,
                root,
                manifest,
                parameterization,
            }
        }

        fn prepare(&self) -> Result<PreparedFootCycleCollectionV1, FootCycleSourcePrepError> {
            prepare_foot_cycle_parameterization_v1(&self.manifest, &self.parameterization)
        }

        fn prepare_with_runtime(
            &self,
            runtime: &mut impl FootCyclePreparationRuntime,
        ) -> Result<PreparedFootCycleCollectionV1, FootCycleSourcePrepError> {
            prepare_foot_cycle_parameterization_v1_with_runtime(
                &self.manifest,
                &self.parameterization,
                runtime,
            )
        }

        fn error_kind(&self) -> FootCycleSourcePrepKind {
            match self.prepare() {
                Ok(_) => panic!("fixture unexpectedly prepared"),
                Err(error) => error.kind(),
            }
        }

        fn rewrite_manifest(&self, rewrite: impl FnOnce(String) -> String) {
            let old_bytes = fs::read(&self.manifest).unwrap();
            let old_input = InputIdentity::from_bytes(&old_bytes);
            fs::write(
                &self.manifest,
                rewrite(String::from_utf8(old_bytes).unwrap()),
            )
            .unwrap();
            let new_input = InputIdentity::from_bytes(&fs::read(&self.manifest).unwrap());
            let old_binding = format!(
                "sha256 = \"{}\"\nbytes = {}",
                old_input.sha256(),
                old_input.bytes()
            );
            let new_binding = format!(
                "sha256 = \"{}\"\nbytes = {}",
                new_input.sha256(),
                new_input.bytes()
            );
            let parameterization = fs::read_to_string(&self.parameterization).unwrap();
            assert!(parameterization.contains(&old_binding));
            fs::write(
                &self.parameterization,
                parameterization.replace(&old_binding, &new_binding),
            )
            .unwrap();
        }
    }

    pub(crate) fn prepared_fixture_for_proof_tests() -> PreparedFootCycleCollectionV1 {
        Fixture::create(FixtureOptions::default())
            .prepare()
            .expect("proof test fixture must prepare")
    }

    pub(crate) fn proof_ready_fixture() -> PreparedFootCycleCollectionV1 {
        let mut prepared = prepared_fixture_for_proof_tests();
        for (source_index, source) in prepared.sources.iter_mut().enumerate() {
            let clip = &mut source.document.clips[0];
            let template = clip.tracks[0].clone();
            let TrackValues::Vec3s(template_values) = &template.values else {
                panic!("fixture translation template");
            };
            let mut high = template_values[0];
            high.x = 0.0;
            high.y = 0.1;
            high.z = 0.0;
            let mut low = high;
            low.y = 0.0;
            let times = (0..=16)
                .map(|index| index as f32 / 16.0)
                .collect::<Vec<_>>();
            let (left_start, right_start) = if source_index == 0 { (2, 10) } else { (4, 12) };
            let foot_track = |bone: usize, start: usize| {
                let mut track = template.clone();
                track.bone = bone;
                track.interpolation = animsmith_core::Interpolation::Linear;
                track.times = times.clone();
                track.values = TrackValues::Vec3s(
                    (0..=16)
                        .map(|index| {
                            if (start..=start + 2).contains(&index) {
                                low
                            } else {
                                high
                            }
                        })
                        .collect(),
                );
                track
            };
            clip.tracks.push(foot_track(2, left_start));
            clip.tracks.push(foot_track(3, right_start));
        }
        for (member, plan) in prepared.members.iter_mut().zip(prepared.plan.members()) {
            let source_clip =
                &prepared.sources[member.source_index].document.clips[member.clip_index];
            member.candidate_clip = time_warp_clip_v1(source_clip, plan).unwrap();
        }
        prepared.source_metric_pose_cells = 2 * 17 * 4;
        prepared.source_metric_sample_evaluations = 2 * 17 * 4;
        prepared
    }

    pub(crate) fn proof_ready_fixture_with_source_metric_work(
        pose_cells: usize,
        sample_evaluations: usize,
    ) -> PreparedFootCycleCollectionV1 {
        let mut prepared = proof_ready_fixture();
        prepared.source_metric_pose_cells = pose_cells;
        prepared.source_metric_sample_evaluations = sample_evaluations;
        prepared
    }

    #[derive(Clone, Copy)]
    enum InjectedShapeFailure {
        InvalidParent,
        InvalidTrackBone,
    }

    #[derive(Default)]
    struct ObservedPreparationRuntime {
        shape_failure: Option<InjectedShapeFailure>,
        metric_work: VecDeque<MetricGridWork>,
        candidate_preflights: VecDeque<CandidateBatchWork>,
        grids_built: usize,
        candidates_built: usize,
    }

    impl FootCyclePreparationRuntime for ObservedPreparationRuntime {
        fn validate_source_document(
            &mut self,
            document: &Document,
        ) -> Result<(), FootCycleSourcePrepError> {
            let mut document = document.clone();
            if let Some(failure) = self.shape_failure.take() {
                match failure {
                    InjectedShapeFailure::InvalidParent => {
                        let bone_count = document.skeleton.bones.len();
                        document.skeleton.bones[0].parent = Some(bone_count);
                    }
                    InjectedShapeFailure::InvalidTrackBone => {
                        document.clips[0].tracks[0].bone = document.skeleton.bones.len();
                    }
                }
            }
            ProductionFootCyclePreparationRuntime.validate_source_document(&document)
        }

        fn metric_grid_work(
            &mut self,
            loaded: &LoadedInput,
            clip_index: usize,
        ) -> Result<MetricGridWork, FootCycleSourcePrepError> {
            self.metric_work.pop_front().map_or_else(
                || ProductionFootCyclePreparationRuntime.metric_grid_work(loaded, clip_index),
                Ok,
            )
        }

        fn build_metric_grid(
            &mut self,
            loaded: &LoadedInput,
            clip_index: usize,
        ) -> Result<Rc<PoseGrid>, FootCycleSourcePrepError> {
            self.grids_built += 1;
            ProductionFootCyclePreparationRuntime.build_metric_grid(loaded, clip_index)
        }

        fn preflight_candidate(
            &mut self,
            clip: &Clip,
            plan: &FootCycleMemberPlanV1,
        ) -> Result<CandidateBatchWork, FootCycleSourcePrepError> {
            self.candidate_preflights.pop_front().map_or_else(
                || ProductionFootCyclePreparationRuntime.preflight_candidate(clip, plan),
                Ok,
            )
        }

        fn build_candidate(
            &mut self,
            clip: &Clip,
            plan: &FootCycleMemberPlanV1,
        ) -> Result<Clip, FootCycleSourcePrepError> {
            self.candidates_built += 1;
            ProductionFootCyclePreparationRuntime.build_candidate(clip, plan)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_source(
        path: &Path,
        buffer_name: &str,
        end_x: f32,
        end_z: f32,
        yaw_deg: f32,
        nonfinite: bool,
        ambiguous_root: bool,
        duplicate_take_name: bool,
        nonconstant_cubic: bool,
    ) {
        let mut bytes = Vec::new();
        for value in [0.0f32, 0.5, 1.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        if nonconstant_cubic {
            for x in [0.0f32, 0.005, 0.0] {
                for vector in [[0.0f32; 3], [x, 0.0, 0.0], [0.0f32; 3]] {
                    for value in vector {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        } else {
            for value in [
                0.0f32,
                0.0,
                0.0,
                0.0,
                0.0,
                0.0,
                if nonfinite { f32::NAN } else { end_x },
                0.0,
                end_z,
            ] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        let rotation_offset = bytes.len();
        for angle_deg in [0.0f32, yaw_deg / 2.0, yaw_deg] {
            let half = angle_deg.to_radians() / 2.0;
            for value in [0.0f32, half.sin(), 0.0, half.cos()] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        fs::write(path.with_file_name(buffer_name), &bytes).unwrap();
        let nodes = if ambiguous_root {
            r#"[{"name":"root","children":[2]},{"name":"root"},{"name":"pelvis","children":[3,4]},{"name":"foot_l"},{"name":"foot_r"}]"#
        } else {
            r#"[{"name":"R0","children":[1]},{"name":"H0","children":[2,3]},{"name":"LF0"},{"name":"RF0"}]"#
        };
        let translation_interpolation = if nonconstant_cubic {
            "CUBICSPLINE"
        } else {
            "LINEAR"
        };
        let translation_count = if nonconstant_cubic { 9 } else { 3 };
        let animation = format!(
            r#"{{"name":"Take 001","samplers":[{{"input":0,"output":1,"interpolation":"{translation_interpolation}"}},{{"input":0,"output":2,"interpolation":"LINEAR"}}],"channels":[{{"sampler":0,"target":{{"node":0,"path":"translation"}}}},{{"sampler":1,"target":{{"node":0,"path":"rotation"}}}}]}}"#
        );
        let animations = if duplicate_take_name {
            format!("{animation},{animation}")
        } else {
            animation
        };
        let source = format!(
            r#"{{
  "asset": {{"version":"2.0"}},
  "buffers": [{{"uri":"{buffer_name}","byteLength":{buffer_length}}}],
  "bufferViews": [{{"buffer":0,"byteOffset":0,"byteLength":12}},{{"buffer":0,"byteOffset":12,"byteLength":{translation_length}}},{{"buffer":0,"byteOffset":{rotation_offset},"byteLength":48}}],
  "accessors": [
    {{"bufferView":0,"componentType":5126,"count":3,"type":"SCALAR","min":[0.0],"max":[1.0]}},
    {{"bufferView":1,"componentType":5126,"count":{translation_count},"type":"VEC3"}},
    {{"bufferView":2,"componentType":5126,"count":3,"type":"VEC4"}}
  ],
  "nodes": {nodes},
  "animations": [{animations}],
  "scenes": [{{"nodes":[0]}}],
  "scene": 0
}}"#,
            buffer_length = bytes.len(),
            translation_length = rotation_offset - 12,
        );
        fs::write(path, source).unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fragment(
        root: &Path,
        source_key: &str,
        logical_id: &str,
        take_index: u32,
        take_name: &str,
        duration: f64,
        config: &LoadedConfig,
        windows: &[(ContactRoleV1, f64, f64)],
    ) {
        let source_path = root.join(format!("assets/{source_key}.gltf"));
        let loaded = match load_with_config_for_producer_bounded(
            &source_path,
            config,
            COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
        ) {
            Ok(loaded) => loaded,
            Err(_) => panic!("synthetic source must load"),
        };
        let closure = complete_closure(loaded.dependency_closure(), loaded.input()).unwrap();
        let mut events = Vec::new();
        for (index, &(role, start, end)) in windows.iter().enumerate() {
            events.push(
                ContactEventV1::window(
                    format!("support/{index}"),
                    role,
                    ContactPhaseV1::Begin,
                    ContactEventWindowV1::new(start, end).unwrap(),
                    None,
                )
                .unwrap(),
            );
            events.push(
                ContactEventV1::point(
                    format!("marker/{index}"),
                    role,
                    ContactPhaseV1::Marker,
                    (start + end) / 2.0,
                    None,
                )
                .unwrap(),
            );
        }
        let extension = ContactExtensionV1::new(
            CONTACT_SUPPORT_DETECTOR_V1_ID,
            1,
            json!({
                "algorithm": "stance-support-v1",
                "sampling": "metric-grid-longest-authored-channel",
                "max_frames": 1_000_000,
                "contact_height_m": 0.03,
                "roles": {"left": "left_foot", "right": "right_foot"},
            }),
        )
        .unwrap();
        let fragment = ContactFragmentV1::new(
            ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
            loaded.input().clone(),
            closure,
            ContactClipReferenceV1::collection(logical_id, source_key, take_index, take_name)
                .unwrap(),
            duration,
            events,
            vec![extension],
        )
        .unwrap();
        fs::write(
            root.join(format!("contacts/{source_key}.json")),
            fragment.canonical_json().unwrap(),
        )
        .unwrap();
    }

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, current: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, out);
                } else {
                    out.insert(
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(path).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        visit(root, root, &mut out);
        out
    }

    #[test]
    fn prepares_successful_multi_member_batch_without_mutation() {
        let fixture = Fixture::create(FixtureOptions::default());
        let config_bytes = fs::read(fixture.root.join("config.toml")).unwrap();
        let parameterization_bytes = fs::read(&fixture.parameterization).unwrap();
        let expected_config = crate::load_config(Some(&fixture.root.join("config.toml"))).unwrap();
        let before = snapshot(&fixture.root);
        let prepared = fixture.prepare().unwrap();
        assert_eq!(prepared.sources().len(), 2);
        assert_eq!(prepared.members().len(), 2);
        assert_eq!(prepared.plan().members().len(), 2);
        assert_eq!(
            prepared.parameterization_input(),
            &InputIdentity::from_bytes(&parameterization_bytes)
        );
        assert_eq!(
            prepared.plan().parameterization_input(),
            prepared.parameterization_input()
        );
        assert_eq!(prepared.plan().proof().max_gait_phase_spread(), 0.08);
        assert_eq!(prepared.plan().proof().min_lr_amplitude_m(), 0.05);
        assert_eq!(
            prepared.plan().proof().max_contact_boundary_phase_error(),
            0.01
        );
        assert!(prepared.source_metric_pose_cells() > 0);
        assert!(prepared.source_metric_sample_evaluations() > 0);
        assert_eq!(prepared.sources()[0].key(), "a");
        assert!(Arc::ptr_eq(
            &prepared.sources()[0].config,
            &prepared.sources()[1].config
        ));
        assert_eq!(
            prepared.sources()[0].config_input(),
            Some(&InputIdentity::from_bytes(&config_bytes))
        );
        assert_eq!(
            prepared.sources()[1].config_input(),
            Some(&InputIdentity::from_bytes(&config_bytes))
        );
        assert_eq!(
            format!("{:#?}", prepared.sources()[0].config()),
            format!("{:#?}", expected_config.config)
        );
        assert_eq!(
            format!("{:#?}", prepared.sources()[1].config()),
            format!("{:#?}", expected_config.config)
        );
        assert_eq!(
            prepared.sources()[0]
                .config()
                .rig
                .roles
                .get(&Role::Root)
                .map(String::as_str),
            Some("R0")
        );
        assert_eq!(
            prepared.sources()[0]
                .config()
                .expectations_for("Take 001")
                .looping,
            Some(true)
        );
        assert_eq!(
            prepared.sources()[0].config().gait_groups["fixture"].max_gait_phase_spread,
            0.125
        );
        assert_eq!(
            prepared.sources()[0].config().gait_groups["fixture"].clips,
            ["Take 001", "Take 002"]
        );
        assert_eq!(
            prepared.sources()[0].config().gait_groups["fixture"].min_lr_amplitude_m,
            0.02
        );
        assert_eq!(prepared.members()[1].id().as_str(), "com.example/b");
        assert_eq!(prepared.members()[1].candidate_clip().duration_s, 1.0);
        assert_eq!(
            prepared.members()[1]
                .contact_transform()
                .operation()
                .output_duration_s(),
            Some(1.0)
        );
        assert!(!prepared.output_directory().exists());
        assert_eq!(snapshot(&fixture.root), before);
    }

    #[test]
    fn plan_cross_check_requires_the_full_selected_root_evidence() {
        let fixture = Fixture::create(FixtureOptions::default());
        let prepared = fixture.prepare().unwrap();
        let planned = prepared.plan().members()[1].root_motion();
        assert!(root_motion_matches(planned, planned));
        let corrupted = FootCycleRootMotionEvidenceV1::missing(planned.binding().clone());
        let numerically_corrupted =
            FootCycleRootMotionEvidenceV1::measured(planned.binding().clone(), 0.005, 0.0, 0.5);
        assert!(!root_motion_matches(&corrupted, planned));
        assert!(!root_motion_matches(planned, &corrupted));
        assert!(!root_motion_matches(&numerically_corrupted, planned));

        let config = crate::load_config(Some(&fixture.root.join("config.toml"))).unwrap();
        let loaded = match load_with_config_for_producer_bounded(
            &fixture.root.join("assets/b.gltf"),
            &config,
            COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
        ) {
            Ok(loaded) => loaded,
            Err(_) => panic!("fixture source must reload"),
        };
        let fragment_bytes = fs::read(fixture.root.join("contacts/b.json")).unwrap();
        let fragment = ContactFragmentV1::read_json(&fragment_bytes).unwrap();
        let selected = SelectedMember {
            id: prepared.members()[1].id().clone(),
            source_index: 1,
            clip_index: 0,
            clip_reference: fragment.clip().clone(),
            fragment,
            fragment_input: InputIdentity::from_bytes(&fragment_bytes),
            root_motion: numerically_corrupted,
        };
        assert_eq!(
            cross_check_plan(&selected, &prepared.plan().members()[1], &loaded)
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::PlanBindingMismatch
        );
    }

    #[test]
    fn source_preparation_retains_signed_root_endpoint_and_yaw_facts() {
        let fixture = Fixture::create(FixtureOptions {
            end_x_b: -0.003,
            end_z_b: 0.004,
            yaw_deg_b: -0.5,
            ..FixtureOptions::default()
        });
        let prepared = fixture.prepare().unwrap();
        let FootCycleRootMotionEvidenceV1::Measured {
            endpoint_displacement_x_m,
            endpoint_displacement_z_m,
            accumulated_yaw_deg,
            ..
        } = prepared.plan().members()[1].root_motion()
        else {
            panic!("source preparation must retain measured root facts");
        };
        assert!(*endpoint_displacement_x_m < 0.0);
        assert!(*endpoint_displacement_z_m > 0.0);
        assert!(*accumulated_yaw_deg < 0.0);
    }

    #[test]
    fn shared_config_is_one_exact_immutable_snapshot() {
        let fixture = Fixture::create(FixtureOptions::default());
        let exact_bytes = fs::read(fixture.root.join("config.toml")).unwrap();
        let prepared = fixture.prepare().unwrap();
        fs::write(
            fixture.root.join("config.toml"),
            "[rig]\nroles = { hips = \"changed\" }\n",
        )
        .unwrap();
        assert!(Arc::ptr_eq(
            &prepared.sources()[0].config,
            &prepared.sources()[1].config
        ));
        for source in prepared.sources() {
            assert_eq!(
                source.config_input(),
                Some(&InputIdentity::from_bytes(&exact_bytes))
            );
            assert_eq!(
                source
                    .config()
                    .rig
                    .roles
                    .get(&Role::Root)
                    .map(String::as_str),
                Some("R0")
            );
            assert_eq!(
                source.config().expectations_for("Take 001").looping,
                Some(true)
            );
        }
    }

    #[test]
    fn stale_manifest_binding_and_take_index_or_name_mismatch_refuse() {
        let fixture = Fixture::create(FixtureOptions::default());
        let text = fs::read_to_string(&fixture.parameterization).unwrap();
        fs::write(
            &fixture.parameterization,
            text.replace(
                fixture.prepare().unwrap().manifest_input().sha256(),
                &"0".repeat(64),
            ),
        )
        .unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::PlanRefused);

        for options in [
            FixtureOptions {
                take_index_b: 1,
                ..FixtureOptions::default()
            },
            FixtureOptions {
                take_name_b: "Wrong Take",
                ..FixtureOptions::default()
            },
        ] {
            assert_eq!(
                Fixture::create(options).error_kind(),
                FootCycleSourcePrepKind::TakeMismatch
            );
        }
    }

    #[test]
    fn raw_duplicate_take_names_are_disambiguated_only_by_exact_index_witness() {
        let fixture = Fixture::create(FixtureOptions {
            duplicate_take_name_b: true,
            take_index_b: 1,
            ..FixtureOptions::default()
        });
        let prepared = fixture.prepare().unwrap();
        assert_eq!(prepared.members()[1].clip_index(), 1);
        assert_eq!(prepared.sources()[1].document().clips[1].name, "Take 001#1");
    }

    #[test]
    fn only_runtime_set_reachable_sources_gate_preparation() {
        const UNUSED_SOURCE: &str = r#"
[[sources]]
key = "a-unused"
path = "missing.gltf"
config = "config.toml"
expected_sha256 = "0000000000000000000000000000000000000000000000000000000000000000"

"#;

        let fixture = Fixture::create(FixtureOptions::default());
        fixture.rewrite_manifest(|manifest| {
            manifest
                .replace(
                    "\n[[sources]]\nkey = \"b\"",
                    &format!("\n{UNUSED_SOURCE}[[sources]]\nkey = \"b\""),
                )
                .replace(
                    "members = [\"com.example/a\", \"com.example/b\"]",
                    "members = [\"com.example/b\", \"com.example/a\"]",
                )
        });
        let parameterization = fs::read_to_string(&fixture.parameterization).unwrap();
        fs::write(
            &fixture.parameterization,
            parameterization.replace(
                "[[members]]\nid = \"com.example/a\"\ncontact_fragment = \"contacts/a.json\"\n\n[[members]]\nid = \"com.example/b\"\ncontact_fragment = \"contacts/b.json\"",
                "[[members]]\nid = \"com.example/b\"\ncontact_fragment = \"contacts/b.json\"\n\n[[members]]\nid = \"com.example/a\"\ncontact_fragment = \"contacts/a.json\"",
            ),
        )
        .unwrap();
        let prepared = fixture.prepare().unwrap();
        assert_eq!(
            prepared
                .sources()
                .iter()
                .map(PreparedFootCycleSourceV1::key)
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            prepared
                .members()
                .iter()
                .map(|member| member.id().as_str())
                .collect::<Vec<_>>(),
            ["com.example/b", "com.example/a"]
        );

        let fixture = Fixture::create(FixtureOptions::default());
        fixture.rewrite_manifest(|manifest| {
            manifest.replace("path = \"b.gltf\"", "path = \"missing.gltf\"")
        });
        assert_eq!(
            fixture.error_kind(),
            FootCycleSourcePrepKind::SourceUnavailable
        );

        let fixture = Fixture::create(FixtureOptions::default());
        fixture.rewrite_manifest(|manifest| {
            manifest
                .replace("\n[[sources]]\nkey = \"b\"", &format!("\n{UNUSED_SOURCE}[[sources]]\nkey = \"b\""))
                .replace(
                    "id = \"com.example/b\"\nsource = \"b\"",
                    "id = \"com.example/b\"\nsource = \"a-unused\"",
                )
                .replace(
                    "[[runtime_sets]]",
                    "[[clips]]\nid = \"com.example/z-unused\"\nsource = \"b\"\ntake_index = 0\ntake_name = \"Take 001\"\n\n[[runtime_sets]]",
                )
                .replace(
                    "members = [\"com.example/a\", \"com.example/b\"]",
                    "members = [\"com.example/a\", \"com.example/z-unused\"]",
                )
        });
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::PlanRefused);
    }

    #[test]
    fn stale_source_pin_and_noncanonical_fragment_identity_refuse() {
        let fixture = Fixture::create(FixtureOptions::default());
        let mut source = fs::read(fixture.root.join("assets/b.gltf")).unwrap();
        source.push(b'\n');
        fs::write(fixture.root.join("assets/b.gltf"), source).unwrap();
        assert_eq!(
            fixture.error_kind(),
            FootCycleSourcePrepKind::SourceDigestMismatch
        );

        let fixture = Fixture::create(FixtureOptions::default());
        let mut fragment = fs::read(fixture.root.join("contacts/b.json")).unwrap();
        fragment.push(b'\n');
        fs::write(fixture.root.join("contacts/b.json"), fragment).unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::PlanRefused);

        let fixture = Fixture::create(FixtureOptions::default());
        fixture.rewrite_manifest(|manifest| {
            manifest
                .lines()
                .filter(|line| !line.starts_with("expected_sha256 = "))
                .collect::<Vec<_>>()
                .join("\n")
        });
        assert!(fixture.prepare().is_ok());
    }

    #[test]
    fn duration_root_threshold_missing_ambiguous_and_nonfinite_refuse() {
        assert!(
            Fixture::create(FixtureOptions {
                end_x_b: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M as f32,
                ..FixtureOptions::default()
            })
            .prepare()
            .is_ok()
        );
        assert!(
            Fixture::create(FixtureOptions {
                end_z_b: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M as f32,
                ..FixtureOptions::default()
            })
            .prepare()
            .is_ok()
        );
        assert!(
            Fixture::create(FixtureOptions {
                config: ConfigMode::HipsOnly,
                ..FixtureOptions::default()
            })
            .prepare()
            .is_ok()
        );
        assert!(
            Fixture::create(FixtureOptions {
                yaw_deg_b: FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG as f32,
                ..FixtureOptions::default()
            })
            .prepare()
            .is_ok()
        );
        let cases = [
            (
                FixtureOptions {
                    fragment_duration_b: 2.0,
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::DurationMismatch,
            ),
            (
                FixtureOptions {
                    end_x_b: f32::from_bits(
                        (FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M as f32)
                            .to_bits()
                            + 1,
                    ),
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    end_x_b: 0.008,
                    end_z_b: 0.008,
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    end_z_b: f32::from_bits(
                        (FOOT_CYCLE_PARAMETERIZATION_V1_MAX_HORIZONTAL_DISPLACEMENT_M as f32)
                            .to_bits()
                            + 1,
                    ),
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    yaw_deg_b: f32::from_bits(
                        (FOOT_CYCLE_PARAMETERIZATION_V1_MAX_ACCUMULATED_YAW_DEG as f32).to_bits()
                            + 1,
                    ),
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    config: ConfigMode::MissingRoot,
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    config: ConfigMode::AmbiguousRoot,
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::PlanRefused,
            ),
            (
                FixtureOptions {
                    nonfinite_b: true,
                    ..FixtureOptions::default()
                },
                FootCycleSourcePrepKind::SourceLoad,
            ),
        ];
        for (options, expected) in cases {
            assert_eq!(Fixture::create(options).error_kind(), expected);
        }
    }

    #[test]
    fn path_escape_symlink_collision_and_existing_output_refuse_before_work() {
        let fixture = Fixture::create(FixtureOptions::default());
        let text = fs::read_to_string(&fixture.parameterization).unwrap();
        fs::write(
            &fixture.parameterization,
            text.replace("contacts/b.json", "../contacts/b.json"),
        )
        .unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::Control);

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let fixture = Fixture::create(FixtureOptions::default());
            fs::remove_file(fixture.root.join("contacts/b.json")).unwrap();
            symlink("a.json", fixture.root.join("contacts/b.json")).unwrap();
            assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::Control);
        }

        #[cfg(any(unix, windows))]
        {
            let fixture = Fixture::create(FixtureOptions::default());
            fs::remove_file(fixture.root.join("contacts/b.json")).unwrap();
            fs::hard_link(
                fixture.root.join("config.toml"),
                fixture.root.join("contacts/b.json"),
            )
            .unwrap();
            assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::UnsafePathSet);
        }

        let fixture = Fixture::create(FixtureOptions::default());
        let text = fs::read_to_string(&fixture.parameterization).unwrap();
        fs::write(
            &fixture.parameterization,
            text.replace("contacts/b.json", "config.toml"),
        )
        .unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::UnsafePathSet);

        let fixture = Fixture::create(FixtureOptions::default());
        fs::create_dir(fixture.root.join("generated/aligned")).unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::Control);
    }

    #[test]
    fn source_caps_refuse_first_excess_without_allocating_it() {
        assert!(checked_source_budget(0, COLLECTION_OUTPUT_MAX_SOURCE_BYTES).is_ok());
        assert!(
            checked_source_budget(
                COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES - COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
                COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
            )
            .is_ok()
        );
        assert_eq!(
            checked_source_budget(0, COLLECTION_OUTPUT_MAX_SOURCE_BYTES + 1)
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::SourceBudget
        );
        assert_eq!(
            checked_source_budget(COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES, 1)
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::SourceBudget
        );

        let fixture = Fixture::create(FixtureOptions::default());
        fs::OpenOptions::new()
            .write(true)
            .open(fixture.root.join("assets/b.gltf"))
            .unwrap()
            .set_len(COLLECTION_OUTPUT_MAX_SOURCE_BYTES + 1)
            .unwrap();
        assert_eq!(fixture.error_kind(), FootCycleSourcePrepKind::SourceBudget);

        assert!(
            checked_retained_source_budget(
                COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES - 6,
                1,
                2,
                3,
            )
            .is_ok()
        );
        assert_eq!(
            checked_retained_source_budget(
                COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES - 6,
                1,
                2,
                4,
            )
            .unwrap_err()
            .kind(),
            FootCycleSourcePrepKind::SourceBudget
        );
        assert!(checked_retained_source_budget(0, 0, 0, MAX_RETAINED_DECODED_SOURCE_BYTES).is_ok());
        assert_eq!(
            checked_retained_source_budget(0, 0, 0, MAX_RETAINED_DECODED_SOURCE_BYTES + 1)
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::SourceBudget
        );

        let fixture = Fixture::create(FixtureOptions::default());
        let source = fixture.root.join("assets/a.gltf");
        let exact = fs::metadata(&source).unwrap().len();
        let config = crate::load_config(Some(&fixture.root.join("config.toml"))).unwrap();
        let loaded = match load_with_config_for_producer_bounded(&source, &config, exact) {
            Ok(loaded) => loaded,
            Err(_) => panic!("exact bounded source read must load"),
        };
        assert_eq!(
            closure_external_bytes(loaded.dependency_closure()).unwrap(),
            fs::metadata(fixture.root.join("assets/a.bin"))
                .unwrap()
                .len()
        );
        assert!(retained_document_bytes(loaded.document()).unwrap() > 0);
        assert!(load_with_config_for_producer_bounded(&source, &config, exact - 1).is_err());
    }

    #[test]
    fn decoded_retention_counts_every_owned_document_capacity() {
        fn text(value: &str, capacity: usize) -> String {
            let mut out = String::with_capacity(capacity);
            out.push_str(value);
            out
        }
        fn one<T>(capacity: usize, value: T) -> Vec<T> {
            let mut out = Vec::with_capacity(capacity);
            out.push(value);
            out
        }
        fn texture(seed: u8) -> model::TextureAsset {
            model::TextureAsset {
                bytes: one(7, seed),
                mime: text("m", 9),
            }
        }

        let primitive = model::Primitive {
            indices: one(2, 0),
            positions: one(3, Vec3::ZERO),
            normals: one(4, Vec3::Y),
            uvs: one(5, [0.0, 0.0]),
            joints: one(6, [0; 4]),
            weights: one(7, [0.0; 4]),
            additional_influence_sets: one(
                8,
                model::AdditionalInfluenceSet {
                    set_index: 1,
                    joints_present: true,
                    weights_present: true,
                },
            ),
            ..model::Primitive::default()
        };
        let mesh = model::MeshAsset {
            name: text("mesh", 10),
            primitives: one(3, primitive),
            ..model::MeshAsset::default()
        };
        let instance = model::MeshInstance {
            skin_joints: one(3, 0),
            skin_ibms: one(4, Mat4::IDENTITY),
            ..model::MeshInstance::default()
        };
        let material = model::MaterialAsset {
            name: text("material", 12),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: Some(texture(1)),
            normal_texture: Some(model::NormalTextureAsset {
                texture: texture(2),
                scale: 1.0,
            }),
            metallic_roughness_texture: Some(texture(3)),
            occlusion_texture: Some(model::OcclusionTextureAsset {
                texture: texture(4),
                strength: 1.0,
            }),
        };
        let scene = model::SceneAsset {
            name: Some(text("scene", 11)),
            roots: one(3, 0),
            ..model::SceneAsset::default()
        };
        let source_material = model::SourceMaterialAsset {
            name: Some(text("source-material", 20)),
            texture_bindings: one(
                3,
                model::SourceMaterialTextureBinding {
                    slot: model::MaterialTextureSlot::BaseColor,
                    texture_index: 0,
                },
            ),
            ..model::SourceMaterialAsset::default()
        };
        let source_texture = model::SourceTextureAsset {
            name: Some(text("source-texture", 19)),
            ..model::SourceTextureAsset::default()
        };
        let source_image = model::SourceImageAsset {
            image_index: 0,
            name: Some(text("source-image", 18)),
            source_kind: model::ImageSourceKind::Embedded,
            declared_mime_type: Some(text("image/png", 17)),
            detected_container: Some(model::ImageContainerFormat::Png),
            leading_magic_hex: Some(text("89", 13)),
            inspection: model::SourceImageInspection::Available {
                width: 1,
                height: 1,
                channel_count: 4,
                color_type: model::DecodedImageColorType::Rgba8,
            },
        };
        let mut source_node = model::SourceNodeAsset::new(
            0,
            model::SourceNodeLocalRest::Trs {
                translation: Vec3::ZERO,
                rotation: animsmith_core::glam::Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        source_node.name = Some(text("source-node", 16));
        source_node.scene_root_indices = one(4, 0);
        source_node.bone = Some(0);
        let source_skin = model::SourceSkinAsset {
            name: Some(text("source-skin", 15)),
            joint_source_node_indices: one(5, 0),
            inverse_bind_accessor: model::SourceInverseBindAccessor {
                matrices: one(3, Mat4::IDENTITY),
                ..model::SourceInverseBindAccessor::default()
            },
            attachments: one(
                6,
                model::SourceSkinAttachment {
                    source_node_index: 0,
                    source_mesh_index: Some(0),
                },
            ),
            ..model::SourceSkinAsset::default()
        };
        let track = model::Track {
            bone: 0,
            property: model::Property::Translation,
            interpolation: model::Interpolation::Linear,
            times: one(4, 0.0),
            values: model::TrackValues::Vec3s(one(5, Vec3::ZERO)),
        };
        let mut document = Document::default();
        document.skeleton.bones = one(
            3,
            model::Bone {
                name: text("bone", 9),
                parent: None,
                rest: model::Transform::default(),
                inverse_bind: None,
            },
        );
        document.clips = one(
            4,
            model::Clip {
                name: text("clip", 10),
                duration_s: 1.0,
                tracks: one(3, track),
            },
        );
        document.source.path = Some(text("path", 8));
        document.source.format = Some(text("format", 12));
        document.assets.meshes = one(2, mesh);
        document.assets.instances = one(3, instance);
        document.assets.materials = one(4, material);
        document.assets.scenes = one(5, scene);
        document.assets.material_resources.materials = one(2, source_material);
        document.assets.material_resources.textures = one(3, source_texture);
        document.assets.material_resources.images = one(4, source_image);
        document.assets.source_skeleton.nodes = one(2, source_node);
        document.assets.source_skeleton.skins = one(3, source_skin);

        let mut expected = 0usize;
        macro_rules! count_vec {
            ($values:expr) => {{
                let values = $values;
                assert_eq!(values.len(), 1);
                expected += std::mem::size_of_val(values.as_slice()) * values.capacity();
            }};
        }
        macro_rules! count_text {
            ($value:expr) => {
                expected += $value.capacity();
            };
        }
        count_vec!(&document.skeleton.bones);
        count_text!(&document.skeleton.bones[0].name);
        count_vec!(&document.clips);
        count_text!(&document.clips[0].name);
        count_vec!(&document.clips[0].tracks);
        count_vec!(&document.clips[0].tracks[0].times);
        let model::TrackValues::Vec3s(values) = &document.clips[0].tracks[0].values else {
            unreachable!()
        };
        count_vec!(values);
        count_text!(document.source.path.as_ref().unwrap());
        count_text!(document.source.format.as_ref().unwrap());
        count_vec!(&document.assets.meshes);
        count_text!(&document.assets.meshes[0].name);
        count_vec!(&document.assets.meshes[0].primitives);
        let primitive = &document.assets.meshes[0].primitives[0];
        count_vec!(&primitive.indices);
        count_vec!(&primitive.positions);
        count_vec!(&primitive.normals);
        count_vec!(&primitive.uvs);
        count_vec!(&primitive.joints);
        count_vec!(&primitive.weights);
        count_vec!(&primitive.additional_influence_sets);
        count_vec!(&document.assets.instances);
        count_vec!(&document.assets.instances[0].skin_joints);
        count_vec!(&document.assets.instances[0].skin_ibms);
        count_vec!(&document.assets.materials);
        let material = &document.assets.materials[0];
        count_text!(&material.name);
        for texture in [
            material.base_color_texture.as_ref().unwrap(),
            &material.normal_texture.as_ref().unwrap().texture,
            material.metallic_roughness_texture.as_ref().unwrap(),
            &material.occlusion_texture.as_ref().unwrap().texture,
        ] {
            count_vec!(&texture.bytes);
            count_text!(&texture.mime);
        }
        count_vec!(&document.assets.scenes);
        count_text!(document.assets.scenes[0].name.as_ref().unwrap());
        count_vec!(&document.assets.scenes[0].roots);
        let resources = &document.assets.material_resources;
        count_vec!(&resources.materials);
        count_text!(resources.materials[0].name.as_ref().unwrap());
        count_vec!(&resources.materials[0].texture_bindings);
        count_vec!(&resources.textures);
        count_text!(resources.textures[0].name.as_ref().unwrap());
        count_vec!(&resources.images);
        count_text!(resources.images[0].name.as_ref().unwrap());
        count_text!(resources.images[0].declared_mime_type.as_ref().unwrap());
        count_text!(resources.images[0].leading_magic_hex.as_ref().unwrap());
        let source_skeleton = &document.assets.source_skeleton;
        count_vec!(&source_skeleton.nodes);
        count_text!(source_skeleton.nodes[0].name.as_ref().unwrap());
        count_vec!(&source_skeleton.nodes[0].scene_root_indices);
        count_vec!(&source_skeleton.skins);
        count_text!(source_skeleton.skins[0].name.as_ref().unwrap());
        count_vec!(&source_skeleton.skins[0].joint_source_node_indices);
        count_vec!(&source_skeleton.skins[0].inverse_bind_accessor.matrices);
        count_vec!(&source_skeleton.skins[0].attachments);
        assert_eq!(retained_document_bytes(&document).unwrap(), expected as u64);
    }

    #[test]
    fn contact_caps_accept_exact_and_refuse_first_excess() {
        fn resolved_paths(root: &Path, sizes: &[u64]) -> Vec<CollectionResolvedPath> {
            let control = root.join("control.toml");
            fs::write(&control, b"control").unwrap();
            let resolver = CollectionPathResolver::new(&control, None).unwrap();
            sizes
                .iter()
                .enumerate()
                .map(|(index, size)| {
                    let name = format!("contact-{index}.json");
                    let path = root.join(&name);
                    fs::write(&path, []).unwrap();
                    fs::OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .unwrap()
                        .set_len(*size)
                        .unwrap();
                    let declared = DependencyResourceKeyV1::from_source_str(
                        &name,
                        ResourceKeySyntaxV1::ParserRelativePath,
                    )
                    .unwrap();
                    resolver.resolve_required_control_file(&declared).unwrap()
                })
                .collect()
        }

        let directory = tempfile::tempdir().unwrap();
        let cap = CONTACT_FRAGMENT_V1_MAX_SOURCE_BYTES as u64;
        assert!(checked_contact_budget(0, cap).is_ok());
        assert!(
            checked_contact_budget(
                animsmith_core::FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES - cap,
                cap,
            )
            .is_ok()
        );
        assert_eq!(
            checked_contact_budget(
                animsmith_core::FOOT_CYCLE_PARAMETERIZATION_V1_MAX_CONTACT_FRAGMENT_BYTES,
                1,
            )
            .unwrap_err()
            .kind(),
            FootCycleSourcePrepKind::ContactBudget
        );
        assert!(preflight_contact_byte_budget(&resolved_paths(directory.path(), &[cap])).is_ok());
        assert_eq!(
            preflight_contact_byte_budget(&resolved_paths(directory.path(), &[cap + 1]))
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::ContactBudget
        );
        assert!(
            preflight_contact_byte_budget(&resolved_paths(directory.path(), &[cap; 4])).is_ok()
        );
        assert_eq!(
            preflight_contact_byte_budget(&resolved_paths(
                directory.path(),
                &[cap, cap, cap, cap, 1],
            ))
            .unwrap_err()
            .kind(),
            FootCycleSourcePrepKind::ContactBudget
        );

        let exact_file = directory.path().join("actual-reader.json");
        fs::write(&exact_file, b"1234").unwrap();
        assert_eq!(read_bounded(&exact_file, 4).unwrap(), b"1234");
        assert_eq!(
            read_bounded(&exact_file, 3).unwrap_err().kind(),
            FootCycleSourcePrepKind::ContactBudget
        );
    }

    #[test]
    fn aggregate_config_bytes_accept_exact_and_refuse_first_excess() {
        assert_eq!(
            checked_config_budget(0, MAX_AGGREGATE_CONFIG_BYTES).unwrap(),
            MAX_AGGREGATE_CONFIG_BYTES
        );
        assert_eq!(
            checked_config_budget(MAX_AGGREGATE_CONFIG_BYTES, 1)
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::SourceBudget
        );
        assert_eq!(
            checked_config_budget(u64::MAX, 1).unwrap_err().kind(),
            FootCycleSourcePrepKind::SourceBudget
        );

        let directory = tempfile::tempdir().unwrap();
        let control = directory.path().join("collection.toml");
        fs::write(&control, b"control").unwrap();
        let resolver = CollectionPathResolver::new(&control, None).unwrap();
        let mut exact = Vec::new();
        for index in 0..4 {
            let name = format!("exact-{index}.toml");
            fs::File::create(directory.path().join(&name))
                .unwrap()
                .set_len(MAX_AGGREGATE_CONFIG_BYTES / 4)
                .unwrap();
            let key = DependencyResourceKeyV1::from_source_str(
                &name,
                ResourceKeySyntaxV1::ParserRelativePath,
            )
            .unwrap();
            exact.push(resolver.resolve_config(Some(&key)).unwrap());
        }
        exact.push(exact[0].clone());
        assert!(preflight_config_byte_budget(&exact).is_ok());

        fs::write(directory.path().join("excess.toml"), b"x").unwrap();
        let excess_key = DependencyResourceKeyV1::from_source_str(
            "excess.toml",
            ResourceKeySyntaxV1::ParserRelativePath,
        )
        .unwrap();
        exact.push(resolver.resolve_config(Some(&excess_key)).unwrap());
        assert_eq!(
            preflight_config_byte_budget(&exact).unwrap_err().kind(),
            FootCycleSourcePrepKind::SourceBudget
        );
    }

    #[test]
    fn actual_preparation_admits_all_member_metric_work_before_first_grid() {
        let half = MAX_AGGREGATE_METRIC_GRID_WORK / 2;
        let exact_members = VecDeque::from([
            MetricGridWork {
                pose_cells: half,
                sample_evaluations: half,
            },
            MetricGridWork {
                pose_cells: MAX_AGGREGATE_METRIC_GRID_WORK - half,
                sample_evaluations: MAX_AGGREGATE_METRIC_GRID_WORK - half,
            },
        ]);
        let fixture = Fixture::create(FixtureOptions::default());
        let before = snapshot(&fixture.root);
        let mut runtime = ObservedPreparationRuntime {
            metric_work: exact_members.clone(),
            ..ObservedPreparationRuntime::default()
        };
        let prepared = fixture.prepare_with_runtime(&mut runtime).unwrap();
        assert_eq!(
            prepared.source_metric_pose_cells(),
            MAX_AGGREGATE_METRIC_GRID_WORK
        );
        assert_eq!(
            prepared.source_metric_sample_evaluations(),
            MAX_AGGREGATE_METRIC_GRID_WORK
        );
        assert_eq!(runtime.grids_built, 2);
        assert_eq!(runtime.candidates_built, 2);
        assert_eq!(snapshot(&fixture.root), before);

        for excess_members in [
            VecDeque::from([
                exact_members[0],
                MetricGridWork {
                    pose_cells: exact_members[1].pose_cells + 1,
                    sample_evaluations: exact_members[1].sample_evaluations,
                },
            ]),
            VecDeque::from([
                exact_members[0],
                MetricGridWork {
                    pose_cells: exact_members[1].pose_cells,
                    sample_evaluations: exact_members[1].sample_evaluations + 1,
                },
            ]),
        ] {
            let fixture = Fixture::create(FixtureOptions::default());
            let before = snapshot(&fixture.root);
            let mut runtime = ObservedPreparationRuntime {
                metric_work: excess_members,
                ..ObservedPreparationRuntime::default()
            };
            let error = match fixture.prepare_with_runtime(&mut runtime) {
                Ok(_) => panic!("aggregate metric-grid excess must be refused"),
                Err(error) => error,
            };
            assert_eq!(
                error.kind(),
                FootCycleSourcePrepKind::RootEvidenceUnavailable
            );
            assert_eq!(runtime.grids_built, 0);
            assert_eq!(runtime.candidates_built, 0);
            assert_eq!(snapshot(&fixture.root), before);
        }
    }

    #[test]
    fn actual_preparation_retains_distinct_metric_work_dimensions() {
        let fixture = Fixture::create(FixtureOptions::default());
        let mut runtime = ObservedPreparationRuntime {
            metric_work: VecDeque::from([
                MetricGridWork {
                    pose_cells: 11,
                    sample_evaluations: 23,
                },
                MetricGridWork {
                    pose_cells: 31,
                    sample_evaluations: 47,
                },
            ]),
            ..ObservedPreparationRuntime::default()
        };

        let prepared = fixture.prepare_with_runtime(&mut runtime).unwrap();
        assert_eq!(prepared.source_metric_pose_cells(), 42);
        assert_eq!(prepared.source_metric_sample_evaluations(), 70);
    }

    #[test]
    fn actual_preparation_refuses_malformed_shape_before_grids_or_candidates() {
        for shape_failure in [
            InjectedShapeFailure::InvalidParent,
            InjectedShapeFailure::InvalidTrackBone,
        ] {
            let fixture = Fixture::create(FixtureOptions::default());
            let before = snapshot(&fixture.root);
            let mut runtime = ObservedPreparationRuntime {
                shape_failure: Some(shape_failure),
                ..ObservedPreparationRuntime::default()
            };
            let error = match fixture.prepare_with_runtime(&mut runtime) {
                Ok(_) => panic!("malformed document shape must be refused"),
                Err(error) => error,
            };
            assert_eq!(error.kind(), FootCycleSourcePrepKind::SourceLoad);
            assert_eq!(runtime.grids_built, 0);
            assert_eq!(runtime.candidates_built, 0);
            assert_eq!(snapshot(&fixture.root), before);
        }
    }

    #[test]
    fn actual_preparation_admits_all_candidate_preflights_before_first_candidate() {
        let half = MAX_AGGREGATE_CANDIDATE_BYTES / 2;
        let exact = VecDeque::from([
            CandidateBatchWork {
                bytes: half,
                ..CandidateBatchWork::default()
            },
            CandidateBatchWork {
                bytes: MAX_AGGREGATE_CANDIDATE_BYTES - half,
                ..CandidateBatchWork::default()
            },
        ]);
        let fixture = Fixture::create(FixtureOptions::default());
        let mut runtime = ObservedPreparationRuntime {
            candidate_preflights: exact.clone(),
            ..ObservedPreparationRuntime::default()
        };
        fixture.prepare_with_runtime(&mut runtime).unwrap();
        assert_eq!(runtime.candidates_built, 2);

        let fixture = Fixture::create(FixtureOptions::default());
        let before = snapshot(&fixture.root);
        let mut excess = exact;
        excess[1].bytes += 1;
        let mut runtime = ObservedPreparationRuntime {
            candidate_preflights: excess,
            ..ObservedPreparationRuntime::default()
        };
        let error = match fixture.prepare_with_runtime(&mut runtime) {
            Ok(_) => panic!("aggregate candidate excess must be refused"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), FootCycleSourcePrepKind::ClipTransformRefused);
        assert_eq!(runtime.grids_built, 2);
        assert_eq!(runtime.candidates_built, 0);
        assert_eq!(snapshot(&fixture.root), before);
    }

    #[test]
    fn aggregate_candidate_preflight_is_inclusive_and_precedes_construction() {
        let exact = CandidateBatchWork {
            keys: MAX_AGGREGATE_CANDIDATE_KEYS,
            values: MAX_AGGREGATE_CANDIDATE_VALUES,
            bytes: MAX_AGGREGATE_CANDIDATE_BYTES,
            work: MAX_AGGREGATE_CANDIDATE_WORK,
        };
        assert_eq!(
            checked_candidate_batch_work(CandidateBatchWork::default(), exact),
            Some(exact)
        );
        for excess in [
            CandidateBatchWork {
                keys: 1,
                values: 0,
                bytes: 0,
                work: 0,
            },
            CandidateBatchWork {
                keys: 0,
                values: 1,
                bytes: 0,
                work: 0,
            },
            CandidateBatchWork {
                keys: 0,
                values: 0,
                bytes: 1,
                work: 0,
            },
            CandidateBatchWork {
                keys: 0,
                values: 0,
                bytes: 0,
                work: 1,
            },
        ] {
            assert!(checked_candidate_batch_work(exact, excess).is_none());
            let mut built = false;
            assert_eq!(
                build_after_candidate_batch_preflight(&[exact, excess], || {
                    built = true;
                    Ok(())
                })
                .unwrap_err()
                .kind(),
                FootCycleSourcePrepKind::ClipTransformRefused
            );
            assert!(!built);
        }
        assert!(
            checked_candidate_batch_work(
                CandidateBatchWork {
                    keys: usize::MAX,
                    values: 0,
                    bytes: 0,
                    work: 0,
                },
                CandidateBatchWork {
                    keys: 1,
                    values: 0,
                    bytes: 0,
                    work: 0,
                },
            )
            .is_none()
        );
    }

    #[test]
    fn second_member_candidate_refusal_returns_no_partial_result_and_writes_nothing() {
        let fixture = Fixture::create(FixtureOptions {
            nonconstant_cubic_b: true,
            ..FixtureOptions::default()
        });
        let before = snapshot(&fixture.root);
        assert_eq!(
            fixture.error_kind(),
            FootCycleSourcePrepKind::ClipTransformRefused
        );
        assert_eq!(snapshot(&fixture.root), before);
        assert!(!fixture.root.join("generated/aligned").exists());
    }

    #[test]
    fn contact_transform_continuation_accepts_exact_copy_and_rejects_inconsistent_closure() {
        let fixture = Fixture::create(FixtureOptions::default());
        let prepared = fixture.prepare().unwrap();
        let continuation = prepared.members()[0].contact_transform();
        let output_artifact = InputIdentity::from_bytes(b"serialized-candidate-document");
        let output_closure = DependencyClosureBuilderV1::new(
            output_artifact.clone(),
            SourceSetCoverageV1::complete(),
            0,
        )
        .finish()
        .unwrap();
        let transformed = continuation
            .transform_after_serialization(
                output_artifact.clone(),
                output_closure,
                ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
            )
            .unwrap();
        assert_eq!(transformed.output().unwrap().artifact(), &output_artifact);
        assert_ne!(
            transformed.output().unwrap().artifact(),
            continuation.input_fragment().artifact()
        );

        // A freshly serialized identity-map candidate can truthfully have the
        // same content identity. Identity records carry no capture timestamp.
        let exact_copy = continuation
            .transform_after_serialization(
                continuation.input_fragment().artifact().clone(),
                prepared.sources()[0].dependency_closure().clone(),
                ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
            )
            .unwrap();
        assert_eq!(
            exact_copy.output().unwrap().artifact(),
            continuation.input_fragment().artifact()
        );

        let fresh_artifact = InputIdentity::from_bytes(b"another serialized candidate");
        assert_eq!(
            continuation
                .transform_after_serialization(
                    fresh_artifact,
                    prepared.sources()[0].dependency_closure().clone(),
                    ContactProducerV1::new("animsmith", "0.10.0").unwrap(),
                )
                .unwrap_err()
                .kind(),
            FootCycleSourcePrepKind::ContactTransformRefused
        );
    }
}
