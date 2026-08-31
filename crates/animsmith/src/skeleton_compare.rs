//! Strict structural skeleton-compatibility evidence.
//!
//! This frontend owns the correspondence TOML and CLI presentation.  It uses
//! existing format-neutral source-skeleton measurements rather than creating a
//! second, hand-copied rest-pose authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use animsmith_core::measure::{
    AssetMeasurements, LinearTransformMeasurements, SkeletonNodeLocalRestMeasurements,
    SkeletonNodeMeasurements, SkeletonSourceCoverage,
};
use animsmith_core::{
    DependencyClosureIdentityV1, InputIdentity, SourceInverseBindAccessorStatus, ToolInfo,
};
use serde::{Deserialize, Serialize};

use super::{EXIT_FINDINGS, LoadedInput, input_format, load_source_bytes_typed};

/// Immutable correspondence identity.
pub(crate) const SKELETON_CORRESPONDENCE_V1_ID: &str = "urn:animsmith:skeleton-correspondence:1";
/// Immutable result identity.
pub(crate) const SKELETON_COMPATIBILITY_V1_ID: &str =
    "urn:animsmith:schema:skeleton-compatibility:1";
const SCHEMA_VERSION: u32 = 1;
const MAX_CONTROL_BYTES: u64 = 64 * 1024;
const MAX_SELECTED_NODES: usize = 16 * 1024;
const MAX_PRIMARY_SOURCE_BYTES: u64 = animsmith_core::DEPENDENCY_CLOSURE_V1_MAX_RESOURCE_BYTES;

/// Run one comparison and write its immutable JSON result to stdout.
pub(crate) fn run(
    source_path: &Path,
    target_path: &Path,
    correspondence_path: &Path,
    tool: ToolInfo,
    json: bool,
) -> Result<ExitCode, String> {
    let correspondence_bytes = read_bounded(correspondence_path)?;
    let correspondence = parse(&correspondence_bytes)?;
    let source = load_bounded_input(source_path)?;
    let target = load_bounded_input(target_path)?;
    let result = compare(
        &source,
        &target,
        &correspondence,
        InputIdentity::from_bytes(&correspondence_bytes),
        tool,
    )?;
    if json {
        super::render::print_json(&result)?;
    } else {
        super::publish::emit_text(&render_text(&result));
    }
    Ok(if result.outcome == CompatibilityOutcome::Compatible {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FINDINGS)
    })
}

fn load_bounded_input(path: &Path) -> Result<LoadedInput, String> {
    let format = input_format(path)?;
    let file =
        File::open(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_PRIMARY_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_PRIMARY_SOURCE_BYTES {
        return Err(format!(
            "skeleton comparison input exceeds its {} byte limit: {}",
            MAX_PRIMARY_SOURCE_BYTES,
            path.display()
        ));
    }
    let source =
        load_source_bytes_typed(path, format, &bytes).map_err(|error| error.to_string())?;
    Ok(LoadedInput {
        source,
        engine: None,
        engine_v2: None,
        engine_v4: None,
    })
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| "cannot read skeleton correspondence".to_owned())?;
    let mut bytes = Vec::new();
    file.take(MAX_CONTROL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read skeleton correspondence".to_owned())?;
    if bytes.len() as u64 > MAX_CONTROL_BYTES {
        return Err("skeleton correspondence exceeds its bounded reader limit".to_owned());
    }
    Ok(bytes)
}

fn parse(bytes: &[u8]) -> Result<Correspondence, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "skeleton correspondence control error (encoding)".to_owned())?;
    let wire: CorrespondenceWire = toml::from_str(text)
        .map_err(|_| "skeleton correspondence control error (malformed)".to_owned())?;
    if wire.schema != SKELETON_CORRESPONDENCE_V1_ID {
        return Err("skeleton correspondence control error (unsupported-schema)".to_owned());
    }
    if wire.schema_version != SCHEMA_VERSION {
        return Err(
            "skeleton correspondence control error (unsupported-schema-version)".to_owned(),
        );
    }
    let source = decode_subject(wire.source, "source")?;
    let target = decode_subject(wire.target, "target")?;
    let tolerances = Tolerances {
        translation_m: finite_nonnegative(wire.tolerances.translation_m, "translation_m")?,
        rotation_deg: finite_nonnegative(wire.tolerances.rotation_deg, "rotation_deg")?,
        scale_delta: finite_nonnegative(wire.tolerances.scale_delta, "scale_delta")?,
        normalized_bone_length_ratio_delta: finite_nonnegative(
            wire.tolerances.normalized_bone_length_ratio_delta,
            "normalized_bone_length_ratio_delta",
        )?,
    };
    let mapping = match wire.correspondence {
        MatchingWire::ExactName {} => Matching::ExactName,
        MatchingWire::Explicit { map } => {
            if map.is_empty() || map.len() > MAX_SELECTED_NODES {
                return Err(
                    "skeleton correspondence control error (invalid-explicit-map)".to_owned(),
                );
            }
            let mut targets = BTreeSet::new();
            for (source, target) in &map {
                valid_name(source, "source mapping name")?;
                valid_name(target, "target mapping name")?;
                if !targets.insert(target.clone()) {
                    return Err(
                        "skeleton correspondence control error (duplicate-target)".to_owned()
                    );
                }
            }
            Matching::Explicit(map)
        }
    };
    Ok(Correspondence {
        source,
        target,
        mapping,
        tolerances,
    })
}

fn decode_subject(wire: SubjectWire, label: &str) -> Result<Subject, String> {
    valid_name(&wire.selector.root_name, "skeleton selector")?;
    if wire.selector.node_names.is_empty() || wire.selector.node_names.len() > MAX_SELECTED_NODES {
        return Err(format!(
            "skeleton correspondence control error (invalid-{label}-selector-nodes)"
        ));
    }
    let mut names = BTreeSet::new();
    for name in &wire.selector.node_names {
        valid_name(name, "skeleton selector node")?;
        if !names.insert(name) {
            return Err(format!(
                "skeleton correspondence control error (duplicate-{label}-selector-node)"
            ));
        }
    }
    if !names.contains(&wire.selector.root_name) {
        return Err(format!(
            "skeleton correspondence control error (missing-{label}-selector-root)"
        ));
    }
    let sha256 = wire.input.sha256;
    if sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!(
            "skeleton correspondence control error (invalid-{label}-identity)"
        ));
    }
    if wire.input.bytes == 0 {
        return Err(format!(
            "skeleton correspondence control error (invalid-{label}-identity)"
        ));
    }
    Ok(Subject {
        expected_identity: IdentityPin {
            sha256,
            bytes: wire.input.bytes,
        },
        root_name: wire.selector.root_name,
        node_names: wire.selector.node_names,
    })
}

fn valid_name(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 1024 || value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(format!(
            "skeleton correspondence control error (invalid-{label})"
        ));
    }
    Ok(())
}

fn finite_nonnegative(value: f64, label: &str) -> Result<f64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "skeleton correspondence control error (invalid-{label})"
        ));
    }
    Ok(value)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorrespondenceWire {
    schema: String,
    schema_version: u32,
    source: SubjectWire,
    target: SubjectWire,
    correspondence: MatchingWire,
    tolerances: TolerancesWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectWire {
    input: IdentityWire,
    selector: SelectorWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    sha256: String,
    bytes: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectorWire {
    root_name: String,
    node_names: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
enum MatchingWire {
    ExactName {},
    Explicit { map: BTreeMap<String, String> },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TolerancesWire {
    translation_m: f64,
    rotation_deg: f64,
    scale_delta: f64,
    normalized_bone_length_ratio_delta: f64,
}

struct Correspondence {
    source: Subject,
    target: Subject,
    mapping: Matching,
    tolerances: Tolerances,
}

struct Subject {
    expected_identity: IdentityPin,
    root_name: String,
    node_names: Vec<String>,
}

struct IdentityPin {
    sha256: String,
    bytes: u64,
}

impl IdentityPin {
    fn matches(&self, actual: &InputIdentity) -> bool {
        self.sha256 == actual.sha256() && self.bytes == actual.bytes()
    }
}

enum Matching {
    ExactName,
    Explicit(BTreeMap<String, String>),
}

struct Tolerances {
    translation_m: f64,
    rotation_deg: f64,
    scale_delta: f64,
    normalized_bone_length_ratio_delta: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CompatibilityOutcome {
    Compatible,
    Incompatible,
    Partial,
    NotEvaluated,
}

#[derive(Serialize)]
struct ResultEnvelope {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    outcome: CompatibilityOutcome,
    correspondence: CorrespondenceProvenance,
    source: SubjectProvenance,
    target: SubjectProvenance,
    rows: Vec<Row>,
    facets: Facets,
}

#[derive(Serialize)]
struct CorrespondenceProvenance {
    schema: &'static str,
    schema_version: u32,
    input: InputIdentity,
    matching_mode: &'static str,
    tolerances: OutputTolerances,
}

#[derive(Serialize)]
struct OutputTolerances {
    translation_m: f64,
    rotation_deg: f64,
    scale_delta: f64,
    normalized_bone_length_ratio_delta: f64,
}

#[derive(Serialize)]
struct SubjectProvenance {
    input: InputIdentity,
    selected_skeleton_identity: InputIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependency_closure_identity: Option<DependencyClosureIdentityV1>,
    dependency_closure_complete: bool,
    selector: SelectorOut,
    normalized_units_basis: NormalizedBasis,
}

#[derive(Serialize)]
struct SelectorOut {
    root_name: String,
    node_names: Vec<String>,
}

#[derive(Serialize)]
struct NormalizedBasis {
    translation: &'static str,
    rotation: &'static str,
}

#[derive(Serialize)]
struct Row {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_correspondence: Option<FacetState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_rest: Option<RestDeltas>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rest_world: Option<RestDeltas>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_child_bone_length_ratio: Option<Delta>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum FacetState {
    Pass,
    Mismatch,
    Unavailable,
}

#[derive(Serialize)]
struct RestDeltas {
    translation_m: Delta,
    rotation_deg: Delta,
    scale_delta: Delta,
}

#[derive(Serialize)]
struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f64>,
    tolerance: f64,
    state: FacetState,
}

#[derive(Serialize)]
struct Facets {
    topology_rest: FacetSummary,
    skin_membership: EvidenceSummary,
    inverse_bind: EvidenceSummary,
    deformation_model: EvidenceSummary,
}

#[derive(Serialize)]
struct FacetSummary {
    required: bool,
    state: FacetState,
}

#[derive(Serialize)]
struct EvidenceSummary {
    required: bool,
    state: FacetState,
    source: EvidenceSide,
    target: EvidenceSide,
}

#[derive(Serialize)]
struct EvidenceSide {
    state: FacetState,
    detail: String,
}

fn compare(
    source: &LoadedInput,
    target: &LoadedInput,
    correspondence: &Correspondence,
    correspondence_input: InputIdentity,
    tool: ToolInfo,
) -> Result<ResultEnvelope, String> {
    if !correspondence
        .source
        .expected_identity
        .matches(source.input())
    {
        return Err("skeleton correspondence control error (stale-source-identity)".to_owned());
    }
    if !correspondence
        .target
        .expected_identity
        .matches(target.input())
    {
        return Err("skeleton correspondence control error (stale-target-identity)".to_owned());
    }

    let source_assets = animsmith_core::measure::measure_assets(source.document());
    let target_assets = animsmith_core::measure::measure_assets(target.document());
    let source_nodes = select_nodes(&source_assets, &correspondence.source)?;
    let target_nodes = select_nodes(&target_assets, &correspondence.target)?;
    let source_subject = subject_provenance(
        source,
        &correspondence.source,
        selected_skeleton_identity(&source_nodes, &correspondence.source)?,
    );
    let target_subject = subject_provenance(
        target,
        &correspondence.target,
        selected_skeleton_identity(&target_nodes, &correspondence.target)?,
    );
    let mut partial =
        !source_subject.dependency_closure_complete || !target_subject.dependency_closure_complete;
    let (rows, has_mismatch, has_evaluable) = compare_rows(
        &source_nodes,
        &target_nodes,
        &correspondence.mapping,
        &correspondence.tolerances,
        &mut partial,
    );
    let topology_rest = if !has_evaluable {
        FacetSummary {
            required: true,
            state: FacetState::Unavailable,
        }
    } else if has_mismatch {
        FacetSummary {
            required: true,
            state: FacetState::Mismatch,
        }
    } else if partial {
        FacetSummary {
            required: true,
            state: FacetState::Unavailable,
        }
    } else {
        FacetSummary {
            required: true,
            state: FacetState::Pass,
        }
    };
    let facets = Facets {
        topology_rest,
        skin_membership: evidence_summary(&source_assets, &target_assets, EvidenceKind::Skin),
        inverse_bind: evidence_summary(&source_assets, &target_assets, EvidenceKind::Bind),
        // The format-neutral source table deliberately has no deformation model
        // vocabulary yet.  Recording that boundary prevents a structural result
        // from being read as a skinning-runtime verdict.
        deformation_model: evidence_summary(
            &source_assets,
            &target_assets,
            EvidenceKind::Deformation,
        ),
    };
    let outcome = if !has_evaluable {
        CompatibilityOutcome::NotEvaluated
    } else if partial {
        CompatibilityOutcome::Partial
    } else if has_mismatch {
        CompatibilityOutcome::Incompatible
    } else {
        CompatibilityOutcome::Compatible
    };
    Ok(ResultEnvelope {
        schema_version: SCHEMA_VERSION,
        schema: SKELETON_COMPATIBILITY_V1_ID,
        tool,
        command: "skeleton compare",
        outcome,
        correspondence: CorrespondenceProvenance {
            schema: SKELETON_CORRESPONDENCE_V1_ID,
            schema_version: SCHEMA_VERSION,
            input: correspondence_input,
            matching_mode: match correspondence.mapping {
                Matching::ExactName => "exact_name",
                Matching::Explicit(_) => "explicit",
            },
            tolerances: OutputTolerances {
                translation_m: correspondence.tolerances.translation_m,
                rotation_deg: correspondence.tolerances.rotation_deg,
                scale_delta: correspondence.tolerances.scale_delta,
                normalized_bone_length_ratio_delta: correspondence
                    .tolerances
                    .normalized_bone_length_ratio_delta,
            },
        },
        source: source_subject,
        target: target_subject,
        rows,
        facets,
    })
}

fn subject_provenance(
    input: &LoadedInput,
    subject: &Subject,
    selected_skeleton_identity: InputIdentity,
) -> SubjectProvenance {
    SubjectProvenance {
        input: input.input().clone(),
        selected_skeleton_identity,
        dependency_closure_identity: input.dependency_closure().identity().cloned(),
        dependency_closure_complete: input.dependency_closure().coverage().is_complete()
            && input.dependency_closure().identity().is_some(),
        selector: SelectorOut {
            root_name: subject.root_name.clone(),
            node_names: subject.node_names.clone(),
        },
        normalized_units_basis: NormalizedBasis {
            translation: "metres",
            rotation: "right-handed normalized coordinates",
        },
    }
}

#[derive(Clone)]
struct SelectedNode<'a> {
    node: &'a SkeletonNodeMeasurements,
    parent_name: Option<String>,
}

#[derive(Serialize)]
struct SelectedSkeletonIdentity<'a> {
    root_name: &'a str,
    node_names: &'a [String],
    nodes: BTreeMap<&'a str, &'a SkeletonNodeMeasurements>,
}

fn selected_skeleton_identity(
    nodes: &BTreeMap<String, SelectedNode<'_>>,
    subject: &Subject,
) -> Result<InputIdentity, String> {
    let projection = SelectedSkeletonIdentity {
        root_name: &subject.root_name,
        node_names: &subject.node_names,
        nodes: nodes
            .iter()
            .map(|(name, selected)| (name.as_str(), selected.node))
            .collect(),
    };
    let bytes = serde_json::to_vec(&projection)
        .map_err(|_| "cannot serialize selected skeleton identity".to_owned())?;
    Ok(InputIdentity::from_bytes(&bytes))
}

type TrsComponents = (Option<[f64; 3]>, Option<[f64; 4]>, Option<[f64; 3]>);

fn select_nodes<'a>(
    assets: &'a AssetMeasurements,
    subject: &Subject,
) -> Result<BTreeMap<String, SelectedNode<'a>>, String> {
    if assets.skeleton_source_coverage != SkeletonSourceCoverage::Complete {
        return Err(
            "skeleton comparison unavailable: source skeleton identity coverage is unavailable"
                .to_owned(),
        );
    }
    let mut by_index = BTreeMap::new();
    for node in &assets.skeleton_nodes {
        if by_index.insert(node.node_index, node).is_some() {
            return Err(
                "skeleton comparison unavailable: source skeleton has duplicate node identities"
                    .to_owned(),
            );
        }
    }
    let requested = subject
        .node_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut selected_by_name = BTreeMap::new();
    for node in &assets.skeleton_nodes {
        let Some(name) = node.name.as_deref() else {
            continue;
        };
        if requested.contains(name) && selected_by_name.insert(name, node).is_some() {
            return Err(
                "skeleton comparison unavailable: selector did not resolve exactly one declared node"
                    .to_owned(),
            );
        }
    }
    let mut selected_by_index = BTreeMap::new();
    for name in &subject.node_names {
        let node = selected_by_name.get(name.as_str()).ok_or_else(|| {
            "skeleton comparison unavailable: selector did not resolve exactly one declared node"
                .to_owned()
        })?;
        if selected_by_index.insert(node.node_index, *node).is_some() {
            return Err(
                "skeleton comparison unavailable: selector has duplicate node identities"
                    .to_owned(),
            );
        }
    }
    let mut selected = BTreeMap::new();
    for name in &subject.node_names {
        let node = selected_by_name[name.as_str()];
        let mut parent = node.parent_node_index;
        let mut remaining = by_index.len();
        let parent_name = loop {
            let Some(index) = parent else { break None };
            if remaining == 0 {
                return Err(
                    "skeleton comparison unavailable: source parent chain is cyclic".to_owned(),
                );
            }
            remaining -= 1;
            if let Some(selected_parent) = selected_by_index.get(&index) {
                break selected_parent.name.clone();
            }
            parent = by_index
                .get(&index)
                .ok_or_else(|| {
                    "skeleton comparison unavailable: source parent identity is unavailable"
                        .to_owned()
                })?
                .parent_node_index;
        };
        selected.insert(name.clone(), SelectedNode { node, parent_name });
    }
    Ok(selected)
}

fn compare_rows(
    source: &BTreeMap<String, SelectedNode<'_>>,
    target: &BTreeMap<String, SelectedNode<'_>>,
    matching: &Matching,
    tolerance: &Tolerances,
    partial: &mut bool,
) -> (Vec<Row>, bool, bool) {
    let map: BTreeMap<String, String> = match matching {
        Matching::ExactName => source
            .keys()
            .filter(|name| target.contains_key(*name))
            .map(|name| (name.clone(), name.clone()))
            .collect(),
        Matching::Explicit(map) => map.clone(),
    };
    let mut rows = Vec::new();
    let mut mismatches = false;
    let mut evaluable = false;
    let mapped_targets = map.values().cloned().collect::<BTreeSet<_>>();
    for (source_name, target_name) in &map {
        match (source.get(source_name), target.get(target_name)) {
            (Some(source_node), Some(target_node)) => {
                evaluable = true;
                let parent = parent_state(source_node, target_node, &map);
                let local_rest = rest_deltas(
                    source_node.node,
                    target_node.node,
                    tolerance,
                    false,
                    partial,
                );
                let rest_world =
                    rest_deltas(source_node.node, target_node.node, tolerance, true, partial);
                let length = normalized_length_delta(
                    source_node,
                    target_node,
                    source,
                    target,
                    &map,
                    tolerance,
                    partial,
                );
                let row_mismatch = matches!(parent, FacetState::Mismatch)
                    || rest_mismatch(local_rest.as_ref())
                    || rest_mismatch(rest_world.as_ref())
                    || delta_mismatch(length.as_ref());
                mismatches |= row_mismatch;
                rows.push(Row {
                    kind: if matches!(parent, FacetState::Mismatch) {
                        "parent_mismatch"
                    } else {
                        "matched"
                    },
                    source_name: Some(source_name.clone()),
                    target_name: Some(target_name.clone()),
                    parent_correspondence: Some(parent),
                    local_rest,
                    rest_world,
                    normalized_child_bone_length_ratio: length,
                });
            }
            (Some(_), None) => {
                mismatches = true;
                rows.push(Row {
                    kind: "missing_target",
                    source_name: Some(source_name.clone()),
                    target_name: Some(target_name.clone()),
                    parent_correspondence: None,
                    local_rest: None,
                    rest_world: None,
                    normalized_child_bone_length_ratio: None,
                });
            }
            (None, Some(_)) => {
                mismatches = true;
                rows.push(Row {
                    kind: "missing_source",
                    source_name: Some(source_name.clone()),
                    target_name: Some(target_name.clone()),
                    parent_correspondence: None,
                    local_rest: None,
                    rest_world: None,
                    normalized_child_bone_length_ratio: None,
                });
            }
            (None, None) => {
                *partial = true;
                rows.push(Row {
                    kind: "unavailable",
                    source_name: Some(source_name.clone()),
                    target_name: Some(target_name.clone()),
                    parent_correspondence: None,
                    local_rest: None,
                    rest_world: None,
                    normalized_child_bone_length_ratio: None,
                });
            }
        }
    }
    for name in source.keys().filter(|name| !map.contains_key(*name)) {
        mismatches = true;
        rows.push(Row {
            kind: "missing_target",
            source_name: Some(name.clone()),
            target_name: None,
            parent_correspondence: None,
            local_rest: None,
            rest_world: None,
            normalized_child_bone_length_ratio: None,
        });
    }
    for name in target.keys().filter(|name| !mapped_targets.contains(*name)) {
        mismatches = true;
        rows.push(Row {
            kind: "missing_source",
            source_name: None,
            target_name: Some(name.clone()),
            parent_correspondence: None,
            local_rest: None,
            rest_world: None,
            normalized_child_bone_length_ratio: None,
        });
    }
    (rows, mismatches, evaluable)
}

fn parent_state(
    source: &SelectedNode<'_>,
    target: &SelectedNode<'_>,
    mapping: &BTreeMap<String, String>,
) -> FacetState {
    match (&source.parent_name, &target.parent_name) {
        (None, None) => FacetState::Pass,
        (Some(source_parent), Some(target_parent))
            if mapping.get(source_parent) == Some(target_parent) =>
        {
            FacetState::Pass
        }
        _ => FacetState::Mismatch,
    }
}

fn rest_deltas(
    source: &SkeletonNodeMeasurements,
    target: &SkeletonNodeMeasurements,
    tolerance: &Tolerances,
    world: bool,
    partial: &mut bool,
) -> Option<RestDeltas> {
    let (source_translation, source_rotation, source_scale) = if world {
        (
            source.rest_world_translation_m.map(|v| v.map(f64::from)),
            source
                .rest_world_linear
                .rotation_xyzw
                .map(|v| v.map(f64::from)),
            axis_lengths(&source.rest_world_linear),
        )
    } else {
        local_trs(source)
    };
    let (target_translation, target_rotation, target_scale) = if world {
        (
            target.rest_world_translation_m.map(|v| v.map(f64::from)),
            target
                .rest_world_linear
                .rotation_xyzw
                .map(|v| v.map(f64::from)),
            axis_lengths(&target.rest_world_linear),
        )
    } else {
        local_trs(target)
    };
    let translation = vector_delta(
        source_translation,
        target_translation,
        tolerance.translation_m,
    );
    let rotation = rotation_delta(source_rotation, target_rotation, tolerance.rotation_deg);
    let scale = vector_delta(source_scale, target_scale, tolerance.scale_delta);
    if matches!(translation.state, FacetState::Unavailable)
        || matches!(rotation.state, FacetState::Unavailable)
        || matches!(scale.state, FacetState::Unavailable)
    {
        *partial = true;
    }
    Some(RestDeltas {
        translation_m: translation,
        rotation_deg: rotation,
        scale_delta: scale,
    })
}

fn local_trs(node: &SkeletonNodeMeasurements) -> TrsComponents {
    match &node.local_rest {
        SkeletonNodeLocalRestMeasurements::Trs {
            translation_parent_space_m,
            rotation_xyzw,
            scale,
        } => (
            Some(translation_parent_space_m.map(f64::from)),
            Some(rotation_xyzw.map(f64::from)),
            Some(scale.map(f64::from)),
        ),
        SkeletonNodeLocalRestMeasurements::Matrix { .. }
        | SkeletonNodeLocalRestMeasurements::Unavailable { .. }
        | _ => (None, None, None),
    }
}

fn axis_lengths(linear: &LinearTransformMeasurements) -> Option<[f64; 3]> {
    linear.axis_lengths
}

fn vector_delta(a: Option<[f64; 3]>, b: Option<[f64; 3]>, tolerance: f64) -> Delta {
    match (a, b) {
        (Some(a), Some(b)) => {
            let value = a
                .into_iter()
                .zip(b)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f64>()
                .sqrt();
            Delta {
                value: Some(value),
                tolerance,
                state: if value <= tolerance {
                    FacetState::Pass
                } else {
                    FacetState::Mismatch
                },
            }
        }
        _ => Delta {
            value: None,
            tolerance,
            state: FacetState::Unavailable,
        },
    }
}

fn rotation_delta(a: Option<[f64; 4]>, b: Option<[f64; 4]>, tolerance: f64) -> Delta {
    match (a, b) {
        (Some(a), Some(b)) => {
            let dot = a
                .into_iter()
                .zip(b)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .abs()
                .clamp(-1.0, 1.0);
            let value = (2.0 * dot.acos()).to_degrees();
            Delta {
                value: Some(value),
                tolerance,
                state: if value <= tolerance {
                    FacetState::Pass
                } else {
                    FacetState::Mismatch
                },
            }
        }
        _ => Delta {
            value: None,
            tolerance,
            state: FacetState::Unavailable,
        },
    }
}

fn normalized_length_delta(
    source_node: &SelectedNode<'_>,
    target_node: &SelectedNode<'_>,
    source: &BTreeMap<String, SelectedNode<'_>>,
    target: &BTreeMap<String, SelectedNode<'_>>,
    mapping: &BTreeMap<String, String>,
    tolerance: &Tolerances,
    partial: &mut bool,
) -> Option<Delta> {
    let (Some(source_parent), Some(target_parent)) =
        (&source_node.parent_name, &target_node.parent_name)
    else {
        return None;
    };
    if mapping.get(source_parent) != Some(target_parent) {
        return None;
    }
    let (Some(source_parent), Some(target_parent)) =
        (source.get(source_parent), target.get(target_parent))
    else {
        *partial = true;
        return Some(Delta {
            value: None,
            tolerance: tolerance.normalized_bone_length_ratio_delta,
            state: FacetState::Unavailable,
        });
    };
    let source_length = world_distance(source_node.node, source_parent.node);
    let target_length = world_distance(target_node.node, target_parent.node);
    let value = match (source_length, target_length) {
        (Some(source), Some(target)) if source > 0.0 && target > 0.0 => {
            Some((source / target - 1.0).abs())
        }
        _ => None,
    };
    if value.is_none() {
        *partial = true;
    }
    Some(Delta {
        value,
        tolerance: tolerance.normalized_bone_length_ratio_delta,
        state: match value {
            Some(value) if value <= tolerance.normalized_bone_length_ratio_delta => {
                FacetState::Pass
            }
            Some(_) => FacetState::Mismatch,
            None => FacetState::Unavailable,
        },
    })
}

fn world_distance(
    node: &SkeletonNodeMeasurements,
    parent: &SkeletonNodeMeasurements,
) -> Option<f64> {
    let a = node.rest_world_translation_m?;
    let b = parent.rest_world_translation_m?;
    Some(
        a.into_iter()
            .zip(b)
            .map(|(a, b)| {
                let d = f64::from(a - b);
                d * d
            })
            .sum::<f64>()
            .sqrt(),
    )
}

fn rest_mismatch(value: Option<&RestDeltas>) -> bool {
    value.is_some_and(|value| {
        delta_mismatch(Some(&value.translation_m))
            || delta_mismatch(Some(&value.rotation_deg))
            || delta_mismatch(Some(&value.scale_delta))
    })
}
fn delta_mismatch(value: Option<&Delta>) -> bool {
    value.is_some_and(|value| matches!(value.state, FacetState::Mismatch))
}

enum EvidenceKind {
    Skin,
    Bind,
    Deformation,
}
fn evidence_summary(
    source: &AssetMeasurements,
    target: &AssetMeasurements,
    kind: EvidenceKind,
) -> EvidenceSummary {
    let side = |assets: &AssetMeasurements| -> EvidenceSide {
        match kind {
            EvidenceKind::Skin => match assets.skeleton_source_coverage {
                SkeletonSourceCoverage::Complete => EvidenceSide {
                    state: FacetState::Pass,
                    detail: format!("{} source skin declarations", assets.skins.len()),
                },
                SkeletonSourceCoverage::Unavailable => EvidenceSide {
                    state: FacetState::Unavailable,
                    detail: "source skeleton coverage unavailable".into(),
                },
            },
            EvidenceKind::Bind => match assets.skeleton_source_coverage {
                SkeletonSourceCoverage::Unavailable => EvidenceSide {
                    state: FacetState::Unavailable,
                    detail: "source skeleton coverage unavailable".into(),
                },
                SkeletonSourceCoverage::Complete if assets.skins.is_empty() => EvidenceSide {
                    state: FacetState::Unavailable,
                    detail: "no source skin declarations".into(),
                },
                SkeletonSourceCoverage::Complete
                    if assets.skins.iter().all(|skin| {
                        skin.inverse_bind_accessor.status
                            == SourceInverseBindAccessorStatus::Available
                    }) =>
                {
                    EvidenceSide {
                        state: FacetState::Pass,
                        detail: format!(
                            "{} source skin declarations have readable inverse-bind accessors",
                            assets.skins.len()
                        ),
                    }
                }
                SkeletonSourceCoverage::Complete => EvidenceSide {
                    state: FacetState::Unavailable,
                    detail: "one or more source skin inverse-bind accessors are unavailable".into(),
                },
            },
            EvidenceKind::Deformation => EvidenceSide {
                state: FacetState::Unavailable,
                detail: "the format-neutral source contract has no deformation-model evidence"
                    .into(),
            },
        }
    };
    let source_side = side(source);
    let target_side = side(target);
    let state = if matches!(source_side.state, FacetState::Unavailable)
        || matches!(target_side.state, FacetState::Unavailable)
    {
        FacetState::Unavailable
    } else {
        FacetState::Pass
    };
    EvidenceSummary {
        required: false,
        state,
        source: source_side,
        target: target_side,
    }
}

fn render_text(result: &ResultEnvelope) -> String {
    let mut text = format!("skeleton compatibility: {:?}\n", result.outcome).to_lowercase();
    for row in &result.rows {
        let source = row.source_name.as_deref().unwrap_or("-");
        let target = row.target_name.as_deref().unwrap_or("-");
        text.push_str(&format!("{:<18} {} -> {}\n", row.kind, source, target));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_reader_rejects_unknown_and_duplicate_explicit_targets() {
        let unknown = br#"schema = "urn:animsmith:skeleton-correspondence:1"
schema_version = 1
unknown = 1
[source]
input = { sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", bytes = 1 }
selector = { root_name = "Root", node_names = ["Root"] }
[target]
input = { sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", bytes = 1 }
selector = { root_name = "Root", node_names = ["Root"] }
[correspondence]
mode = "exact_name"
[tolerances]
translation_m = 0.0
rotation_deg = 0.0
scale_delta = 0.0
normalized_bone_length_ratio_delta = 0.0
"#;
        assert!(parse(unknown).is_err());
        let duplicate = std::str::from_utf8(unknown)
            .unwrap()
            .replace("unknown = 1\n", "")
            .replace(
                "mode = \"exact_name\"",
                "mode = \"explicit\"\nmap = { A = \"X\", B = \"X\" }",
            );
        assert!(parse(duplicate.as_bytes()).is_err());
        let nonfinite = std::str::from_utf8(unknown)
            .unwrap()
            .replace("unknown = 1\n", "")
            .replace("translation_m = 0.0", "translation_m = nan");
        assert!(parse(nonfinite.as_bytes()).is_err());
        let invalid_digest = std::str::from_utf8(unknown)
            .unwrap()
            .replace("unknown = 1\n", "")
            .replace('a', "h");
        assert!(parse(invalid_digest.as_bytes()).is_err());
        let duplicate_selector = std::str::from_utf8(unknown)
            .unwrap()
            .replace("unknown = 1\n", "")
            .replace(
                "node_names = [\"Root\"]",
                "node_names = [\"Root\", \"Root\"]",
            );
        assert!(parse(duplicate_selector.as_bytes()).is_err());
    }
}
