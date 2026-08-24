//! Versioned, recipe-driven multi-source character assembly.
//!
//! Assembly is deliberately a generic producer boundary: it combines already
//! extracted asset files, but it does not own archive extraction, consuming
//! project policy, acceptance contracts, or publication.

use crate::material_recipe::{
    MaterialTextureRecipeEvidence, apply_material_texture_recipe_in_root,
};
use crate::publish::{
    PublicationDestination, emit, emit_text, parent_or_current, publish_pair, read_digest,
    require_external_dependencies_safe_for_publication, require_writable_destination,
    serialize_record,
};
use crate::{Format, render};
use animsmith_core::InputIdentity;
use animsmith_core::model::{
    Clip, Document, Interpolation, MaterialAsset, MeshAsset, Property, Skeleton, SourceNodeAsset,
    SourceSkinAsset, TrackValues,
};
use animsmith_core::scale::{
    AssemblyScaleBasis, AssemblyScaleCompatibilityBasis, AssemblyScaleNamedSelectorResolutionError,
    AssemblyScaleSelectorRequest, AssemblyScaleSkinlessClipBasis, ScaleOperation, ScaleRequest,
    assembly_scale_compatibility_basis, plan_scale, rebase_assembly_scale_skinless_clip,
    require_assembly_scale_compatibility_with_selectors, resolve_assembly_scale_named_selector,
};
use animsmith_core::{Config, ToolInfo, resolve_configured_roles, sha256_hex};
use animsmith_fbx::FbxScaleCapabilityInventory;
use animsmith_gltf::write::WriteSummary;
use animsmith_gltf::{
    operation_capability_facts_for_source, preflight_clip_track_source_bytes,
    preflight_scale_source_bytes, prove_rewritten_rest_bind, rewrite_scale_plan,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const RECIPE_SCHEMA_VERSION_V3: u32 = 3;
const RECIPE_SCHEMA_ID_V3: &str = "urn:animsmith:schema:character-assembly-recipe:3";
const RECIPE_SCHEMA_VERSION_V4: u32 = 4;
const RECIPE_SCHEMA_ID_V4: &str = "urn:animsmith:schema:character-assembly-recipe:4";
const RECIPE_SCHEMA_VERSION_V5: u32 = 5;
const RECIPE_SCHEMA_ID_V5: &str = "urn:animsmith:schema:character-assembly-recipe:5";
const RECIPE_SCHEMA_VERSION_V6: u32 = 6;
const RECIPE_SCHEMA_ID_V6: &str = "urn:animsmith:schema:character-assembly-recipe:6";
const RECIPE_SCHEMA_VERSION_V7: u32 = 7;
const RECIPE_SCHEMA_ID_V7: &str = "urn:animsmith:schema:character-assembly-recipe:7";
const EVIDENCE_SCHEMA_VERSION_V3: u32 = 3;
const EVIDENCE_SCHEMA_ID_V3: &str = "urn:animsmith:schema:character-assembly-evidence:3";
const EVIDENCE_SCHEMA_VERSION_V4: u32 = 4;
const EVIDENCE_SCHEMA_ID_V4: &str = "urn:animsmith:schema:character-assembly-evidence:4";
const EVIDENCE_SCHEMA_VERSION_V5: u32 = 5;
const EVIDENCE_SCHEMA_ID_V5: &str = "urn:animsmith:schema:character-assembly-evidence:5";
const EVIDENCE_SCHEMA_VERSION_V6: u32 = 6;
const EVIDENCE_SCHEMA_ID_V6: &str = "urn:animsmith:schema:character-assembly-evidence:6";
const EVIDENCE_SCHEMA_VERSION_V7: u32 = 7;
const EVIDENCE_SCHEMA_ID_V7: &str = "urn:animsmith:schema:character-assembly-evidence:7";

fn default_fps() -> f64 {
    30.0
}

/// The stable recipe consumed by `animsmith assemble`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "R: Deserialize<'de>", serialize = "R: Serialize")
)]
struct AssemblyRecipe<R = AssemblyRestBindScaleRecipe> {
    schema_version: u32,
    schema: String,
    #[serde(default)]
    input_root: Option<PathBuf>,
    base_input: PathBuf,
    #[serde(default)]
    mesh_instances: Vec<String>,
    #[serde(default)]
    material_texture_recipe: Option<PathBuf>,
    #[serde(default)]
    complete_tracks: bool,
    #[serde(default)]
    prune_constant_tracks: bool,
    #[serde(default)]
    remove_nodes: Vec<String>,
    #[serde(default)]
    canonicalize_skin: bool,
    #[serde(default)]
    ground_and_center: bool,
    #[serde(default = "default_fps")]
    fps: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rest_bind_scale: Option<R>,
    clips: Vec<AssemblyClipRecipe>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum AssemblyRestBindScaleRecipe {
    Indexed(IndexedRestBindScaleRecipe),
    Named(NamedRestBindScaleRecipe),
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct IndexedRestBindScaleRecipe {
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct NamedRestBindScaleRecipe {
    root_node_name: String,
    expected_factor: f64,
}

impl AssemblyRestBindScaleRecipe {
    fn indexed_selector(&self) -> Option<(usize, usize)> {
        match self {
            Self::Indexed(IndexedRestBindScaleRecipe {
                source_skin_index,
                source_root_node_index,
                ..
            }) => Some((*source_skin_index, *source_root_node_index)),
            Self::Named(_) => None,
        }
    }

    fn root_node_name(&self) -> Option<&str> {
        match self {
            Self::Indexed(_) => None,
            Self::Named(NamedRestBindScaleRecipe { root_node_name, .. }) => Some(root_node_name),
        }
    }

    fn expected_factor(&self) -> f64 {
        match self {
            Self::Indexed(recipe) => recipe.expected_factor,
            Self::Named(recipe) => recipe.expected_factor,
        }
    }

    fn compatibility_selector(&self) -> AssemblyScaleSelectorRequest<'_> {
        match self {
            Self::Indexed(_) => AssemblyScaleSelectorRequest::Indexed,
            Self::Named(recipe) => AssemblyScaleSelectorRequest::Named {
                root_node_name: &recipe.root_node_name,
            },
        }
    }
}

impl<R> AssemblyRecipe<R> {
    fn with_rest_bind_scale<S>(self, rest_bind_scale: Option<S>) -> AssemblyRecipe<S> {
        AssemblyRecipe {
            schema_version: self.schema_version,
            schema: self.schema,
            input_root: self.input_root,
            base_input: self.base_input,
            mesh_instances: self.mesh_instances,
            material_texture_recipe: self.material_texture_recipe,
            complete_tracks: self.complete_tracks,
            prune_constant_tracks: self.prune_constant_tracks,
            remove_nodes: self.remove_nodes,
            canonicalize_skin: self.canonicalize_skin,
            ground_and_center: self.ground_and_center,
            fps: self.fps,
            rest_bind_scale,
            clips: self.clips,
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedRestBindScaleSelector {
    source_skin_index: usize,
    source_root_node_index: usize,
    root_node_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssemblyClipRecipe {
    name: String,
    input: PathBuf,
    take: String,
    #[serde(default)]
    frame_window: Option<[u32; 2]>,
    #[serde(default)]
    time_window: Option<[f64; 2]>,
    #[serde(default)]
    drop_closing_endpoint: bool,
    #[serde(default)]
    hold_frames: u32,
    #[serde(default)]
    gait_anchor: bool,
    #[serde(default)]
    strip_bones: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyRecipeEvidence {
    path: String,
    sha256: String,
    effective: AssemblyRecipe,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyConfigEvidence {
    source: &'static str,
    path: Option<String>,
    sha256: Option<String>,
    bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyInputEvidence {
    role: &'static str,
    declared_path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyClipEvidence {
    name: String,
    declared_input: String,
    source_take: String,
    source_tracks: usize,
    emitted_tracks: usize,
    remapped_tracks: usize,
    bone_remaps: Vec<AssemblyBoneRemapEvidence>,
    completed_tracks: usize,
    stripped_tracks: usize,
    stripped_bone_motion: Vec<StrippedBoneMotionEvidence>,
    pruned_constant_tracks: Vec<PrunedConstantTrackEvidence>,
    duration_s: f64,
    frame_window: Option<[u32; 2]>,
    time_window: Option<[f64; 2]>,
    dropped_closing_endpoint: bool,
    hold_frames: u32,
    gait_anchor_frame_offset: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyBoneRemapEvidence {
    source_bone: String,
    source_index: usize,
    base_bone: String,
    base_index: usize,
}

#[derive(Debug, Clone, Serialize)]
struct StrippedBoneMotionEvidence {
    bone: String,
    translation_start: Option<[f32; 3]>,
    translation_end: Option<[f32; 3]>,
    translation_delta: Option<[f32; 3]>,
    duration_s: Option<f64>,
}

/// One constant track removed from a completed and normalized output clip.
#[derive(Debug, Clone, Serialize)]
struct PrunedConstantTrackEvidence {
    original_track_index: usize,
    bone: String,
    bone_index: usize,
    property: &'static str,
    interpolation: &'static str,
    key_count: usize,
}

/// One node removed by the final structural projection.
#[derive(Debug, Clone, Serialize)]
struct RemovedNodeEvidence {
    name: String,
    original_node_index: usize,
    original_parent_node_index: Option<usize>,
    selected: bool,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyTransformEvidence {
    retained_mesh_instances: Vec<String>,
    removed_mesh_instances: usize,
    removed_nodes: Vec<RemovedNodeEvidence>,
    canonicalized_skin: bool,
    ground_and_center: bool,
    source_world_to_canonical: Option<[f32; 16]>,
    converted_bounds_min: Option<[f32; 3]>,
    converted_bounds_max: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyArtifactEvidence {
    path: String,
    sha256: String,
    bytes: u64,
    nodes: usize,
    animations: usize,
    meshes: usize,
    primitive_positions: usize,
    materials: usize,
    clips_without_writable_tracks: usize,
}

#[derive(Debug, Serialize)]
struct AssemblyEvidence {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    recipe: AssemblyRecipeEvidence,
    config: AssemblyConfigEvidence,
    inputs: Vec<AssemblyInputEvidence>,
    clips: Vec<AssemblyClipEvidence>,
    transforms: AssemblyTransformEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    material_texture_recipe: Option<MaterialTextureRecipeEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rest_bind_scale: Option<AssemblyRestBindScaleEvidence>,
    artifact: AssemblyArtifactEvidence,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyRestBindScaleInputEvidence {
    role: String,
    declared_path: String,
    sha256: String,
    bytes: u64,
    basis_schema: &'static str,
    basis_fingerprint: String,
    compatible: bool,
    compatibility: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    application: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_projection: Option<AssemblyScaleInputProjectionEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_root_node_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_source_skin_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_source_root_node_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum AssemblyScaleInputProjectionEvidence {
    RawGltf {
        authored_curve_keys_preserved: bool,
        raw_source_spans_preserved: bool,
    },
    NormalizedBakedFbx {
        authored_curve_keys_preserved: bool,
        raw_source_spans_preserved: bool,
        staged_source: InputIdentity,
        capability: Box<FbxScaleCapabilityInventory>,
    },
}

#[derive(Debug, Serialize)]
struct AssemblyRestBindScaleEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_skin_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_root_node_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_root_node_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_source_skin_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_source_root_node_index: Option<usize>,
    expected_factor: f64,
    inputs: Vec<AssemblyRestBindScaleInputEvidence>,
    staged_source_sha256: String,
    read_back_sha256: String,
    residual_comparison_counts: crate::scale::ResidualComparisonCounts,
    proof: crate::scale::SharedScaleEvidence,
}

/// One role-admitted input projection shared by later assembly orchestration.
/// The authoritative source domain and independently rebased reference stay
/// separate through publication.
struct PreparedScaleProjection {
    authoritative_document: Document,
    rebased_reference_document: Document,
    application: PreparedScaleApplication,
    evidence: AssemblyRestBindScaleInputEvidence,
    mesh_selection: Option<PreparedMeshInstanceSelection>,
}

#[derive(Clone)]
struct PreparedMeshInstanceSelection {
    retained: Vec<String>,
    removed: usize,
}

enum PreparedScaleApplication {
    RestBind {
        compatibility_basis: Box<AssemblyScaleCompatibilityBasis>,
        selector: ResolvedRestBindScaleSelector,
    },
    ClipTracks,
}

struct AssemblyScalePreparationContext<'a> {
    staging_parent: &'a Path,
    output: &'a Path,
    evidence: &'a Path,
    recipe_version: u32,
    remove_nodes: &'a [String],
    retained_mesh_instances: &'a [String],
    tool: &'a ToolInfo,
}

#[derive(Clone, Copy)]
enum AssemblyScaleInputRole<'a> {
    BaseRestBind,
    ClipTracks {
        base_basis: &'a AssemblyScaleCompatibilityBasis,
    },
}

/// The typed domain projection a clip-track input has already earned from its
/// format adapter. Its admitted normalized skeleton and clips remain
/// authoritative to assembly; only `operation_document` can enter rebasing.
enum PreparedClipTrackProjection {
    /// Raw glTF/GLB has passed the clip-track capability policy. The exact raw
    /// identity has already been captured for evidence, while only the
    /// privately projected normalized document can enter rebasing.
    RawGltfClipTracks {
        authoritative_document: Document,
        staged_operation_document: Box<Document>,
        input_format: &'static str,
        source_projection: AssemblyScaleInputProjectionEvidence,
    },
    /// FBX has been normalized through its bounded, animation-only private
    /// stage. The raw authoritative document and staged operation source are
    /// intentionally distinct.
    NormalizedBakedFbx {
        authoritative_document: Document,
        staged_operation_document: Box<Document>,
        source_projection: AssemblyScaleInputProjectionEvidence,
    },
}

impl PreparedClipTrackProjection {
    fn raw_gltf_clip_tracks(
        authoritative_document: Document,
        input_format: &'static str,
        source_projection: AssemblyScaleInputProjectionEvidence,
    ) -> Self {
        let staged_operation_document = clip_scale_stage_document(&authoritative_document);
        Self::RawGltfClipTracks {
            authoritative_document,
            staged_operation_document: Box::new(staged_operation_document),
            input_format,
            source_projection,
        }
    }

    fn normalized_baked_fbx(
        authoritative_document: Document,
        staged_operation_document: Document,
        source_projection: AssemblyScaleInputProjectionEvidence,
    ) -> Self {
        Self::NormalizedBakedFbx {
            authoritative_document,
            staged_operation_document: Box::new(staged_operation_document),
            source_projection,
        }
    }

    fn into_documents_and_evidence(
        self,
    ) -> (
        Document,
        Document,
        &'static str,
        AssemblyScaleInputProjectionEvidence,
    ) {
        match self {
            Self::RawGltfClipTracks {
                authoritative_document,
                staged_operation_document,
                input_format,
                source_projection,
            } => (
                authoritative_document,
                *staged_operation_document,
                input_format,
                source_projection,
            ),
            Self::NormalizedBakedFbx {
                authoritative_document,
                staged_operation_document,
                source_projection,
            } => (
                authoritative_document,
                *staged_operation_document,
                "fbx",
                source_projection,
            ),
        }
    }
}

struct AdmittedRestBindProjection {
    authoritative_document: Document,
    rebased_reference_document: Document,
    compatibility_basis: AssemblyScaleCompatibilityBasis,
    input_format: &'static str,
    source_projection: AssemblyScaleInputProjectionEvidence,
    selector: ResolvedRestBindScaleSelector,
    mesh_selection: Option<PreparedMeshInstanceSelection>,
}

enum AssemblyScaleInputFormat {
    Gltf(&'static str),
    Fbx,
}

struct AssemblyScaleInputRequest<'a> {
    role: String,
    declared: &'a Path,
    resolved: &'a Path,
    scale: &'a AssemblyRestBindScaleRecipe,
    context: &'a AssemblyScalePreparationContext<'a>,
    input_role: AssemblyScaleInputRole<'a>,
}

struct PreparedAssemblyClip {
    clip: Clip,
    source_tracks: usize,
    remapped_tracks: usize,
    bone_remaps: Vec<AssemblyBoneRemapEvidence>,
    stripped_tracks: usize,
    stripped_bone_motion: Vec<StrippedBoneMotionEvidence>,
    gait_anchor_frame_offset: Option<i32>,
}

#[derive(Clone)]
struct AssemblyBoneIndex {
    by_name: BTreeMap<String, usize>,
    names_by_index: Vec<String>,
    parent_by_index: Vec<Option<usize>>,
    ambiguous: BTreeSet<String>,
}

impl AssemblyBoneIndex {
    fn new(skeleton: &Skeleton, _context: &str) -> Result<Self, String> {
        let mut by_name = BTreeMap::new();
        let mut names_by_index = Vec::with_capacity(skeleton.bones.len());
        let parent_by_index = skeleton.bones.iter().map(|bone| bone.parent).collect();
        let mut ambiguous = BTreeSet::new();
        for (index, bone) in skeleton.bones.iter().enumerate() {
            if bone.name.is_empty() {
                return Err(format!("{_context} contains an empty stable bone identity"));
            }
            if by_name.insert(bone.name.clone(), index).is_some() {
                ambiguous.insert(bone.name.clone());
            }
            names_by_index.push(bone.name.clone());
        }
        Ok(Self {
            by_name,
            names_by_index,
            parent_by_index,
            ambiguous,
        })
    }

    fn resolve(&self, name: &str, context: &str) -> Result<usize, String> {
        if self.ambiguous.contains(name) {
            return Err(format!(
                "{context} found ambiguous stable bone identity {name:?}"
            ));
        }
        self.by_name
            .get(name)
            .copied()
            .ok_or_else(|| format!("{context} cannot resolve stable bone identity {name:?}"))
    }

    fn name(&self, index: usize, context: &str) -> Result<&str, String> {
        let name = self
            .names_by_index
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| format!("{context} references missing bone index {index}"))?;
        if name.is_empty() {
            return Err(format!("{context} contains an empty stable bone identity"));
        }
        Ok(name)
    }

    fn ancestry(&self, index: usize, context: &str) -> Result<Vec<&str>, String> {
        let mut ancestry = Vec::new();
        let mut seen = BTreeSet::from([index]);
        let mut parent = *self
            .parent_by_index
            .get(index)
            .ok_or_else(|| format!("{context} references missing bone index {index}"))?;
        while let Some(parent_index) = parent {
            if !seen.insert(parent_index) {
                return Err(format!(
                    "{context} contains a cyclic parent chain at bone index {parent_index}"
                ));
            }
            let parent_name = self.name(parent_index, context)?;
            self.resolve(parent_name, context)?;
            ancestry.push(parent_name);
            parent = *self.parent_by_index.get(parent_index).ok_or_else(|| {
                format!("{context} references missing parent bone index {parent_index}")
            })?;
        }
        Ok(ancestry)
    }
}

#[derive(Clone)]
struct AssemblyBoneCorrespondence {
    left: AssemblyBoneIndex,
    right: AssemblyBoneIndex,
}

impl AssemblyBoneCorrespondence {
    fn new(left: &Skeleton, right: &Skeleton, context: &str) -> Result<Self, String> {
        Ok(Self {
            left: AssemblyBoneIndex::new(left, &format!("{context} left space"))?,
            right: AssemblyBoneIndex::new(right, &format!("{context} right space"))?,
        })
    }

    fn left(&self, name: &str, context: &str) -> Result<usize, String> {
        self.left.resolve(name, context)
    }

    fn right(&self, name: &str, context: &str) -> Result<usize, String> {
        self.right.resolve(name, context)
    }

    fn map_left_name(&self, name: &str, context: &str) -> Result<usize, String> {
        let left = self.left(name, context)?;
        let left_ancestry = self.left.ancestry(left, context)?;
        let right = self.right(name, context)?;
        let right_ancestry = self.right.ancestry(right, context)?;
        if left_ancestry != right_ancestry {
            return Err(format!(
                "{context} ancestor identity for bone {name:?} differs between correspondence spaces"
            ));
        }
        Ok(right)
    }

    fn map_right_name(&self, name: &str, context: &str) -> Result<usize, String> {
        let right = self.right(name, context)?;
        let right_ancestry = self.right.ancestry(right, context)?;
        let left = self.left(name, context)?;
        let left_ancestry = self.left.ancestry(left, context)?;
        if left_ancestry != right_ancestry {
            return Err(format!(
                "{context} ancestor identity for bone {name:?} differs between correspondence spaces"
            ));
        }
        Ok(left)
    }

    fn map_staged_selector_name(&self, name: &str, context: &str) -> Result<usize, String> {
        let left = self.left(name, context)?;
        let mut left_ancestry = self.left.ancestry(left, context)?;
        let right = self.right(name, context)?;
        let mut right_ancestry = self.right.ancestry(right, context)?;
        left_ancestry.retain(|ancestor| *ancestor != "animsmith-canonical-root");
        right_ancestry.retain(|ancestor| *ancestor != "animsmith-canonical-root");
        if left_ancestry != right_ancestry {
            return Err(format!(
                "{context} ancestor identity for bone {name:?} differs between selector spaces"
            ));
        }
        Ok(right)
    }

    fn require_names(&self, names: &[String], context: &str) -> Result<(), String> {
        for name in names {
            self.map_left_name(name, context)?;
        }
        Ok(())
    }

    fn channels(
        &self,
        expected: &Clip,
        actual: &Clip,
        clip_index: usize,
    ) -> Result<Vec<(usize, usize)>, String> {
        let expected_channels = indexed_clip_channels(
            expected,
            &self.left,
            &format!("pre-remap rebase clip {clip_index}"),
        )?;
        let actual_channels = indexed_clip_channels(
            actual,
            &self.right,
            &format!("proved artifact clip {clip_index}"),
        )?;
        let mut matches = Vec::with_capacity(expected_channels.len());
        for ((name, property), expected_channel) in expected_channels {
            let actual_bone = self.map_left_name(&name, "final clip channel correspondence")?;
            let actual_name = self
                .right
                .name(actual_bone, "final clip channel correspondence")?;
            let actual_channel = actual_channels.get(&(actual_name.to_owned(), property)).ok_or_else(|| {
                format!(
                    "proved artifact clip {clip_index} is missing the {property:?} track for bone {name:?}"
                )
            })?;
            if expected_channel.interpolation != actual_channel.interpolation
                || expected_channel.key_count != actual_channel.key_count
                || expected_channel.times != actual_channel.times
            {
                return Err(format!(
                    "proved artifact clip {clip_index} track {} shape differs from its pre-remap rebase",
                    expected_channel.track_index
                ));
            }
            matches.push((expected_channel.track_index, actual_channel.track_index));
        }
        Ok(matches)
    }
}

struct AssemblyClipChannel {
    track_index: usize,
    interpolation: Interpolation,
    key_count: usize,
    times: Vec<u32>,
}

fn indexed_clip_channels(
    clip: &Clip,
    bones: &AssemblyBoneIndex,
    context: &str,
) -> Result<BTreeMap<(String, Property), AssemblyClipChannel>, String> {
    let mut channels = BTreeMap::new();
    for (track_index, track) in clip.tracks.iter().enumerate() {
        let name = bones.name(track.bone, &format!("{context} track {track_index}"))?;
        bones.ancestry(track.bone, &format!("{context} track {track_index}"))?;
        let key = (name.to_owned(), track.property);
        if channels
            .insert(
                key.clone(),
                AssemblyClipChannel {
                    track_index,
                    interpolation: track.interpolation,
                    key_count: track.key_count(),
                    times: track.times.iter().map(|time| time.to_bits()).collect(),
                },
            )
            .is_some()
        {
            return Err(format!(
                "{context} has ambiguous {:?} tracks for bone {:?}",
                key.1, key.0
            ));
        }
    }
    Ok(channels)
}

/// What one published assembly leaves for the caller to report: the counts
/// the text summary names, and the **exact** evidence bytes the pair's
/// evidence member received.
///
/// The bytes travel out rather than the record because stdout must not
/// re-serialize: `--format json` writes this same `Vec<u8>`, so the two
/// destinations are identical by construction.
struct Published {
    animations: usize,
    meshes: usize,
    materials: usize,
    evidence_bytes: Vec<u8>,
}

struct InputResolver {
    root: PathBuf,
}

impl InputResolver {
    fn new(recipe_path: &Path, declared_root: Option<&Path>) -> Result<Self, String> {
        let recipe_parent = parent_or_current(recipe_path);
        let recipe_parent = fs::canonicalize(recipe_parent).map_err(|error| {
            format!(
                "cannot resolve recipe directory {}: {error}",
                recipe_parent.display()
            )
        })?;
        let root = match declared_root {
            Some(path) => {
                validate_relative_path(path, "input_root")?;
                let joined = recipe_parent.join(path);
                reject_symlink_path(&recipe_parent, path, "input_root")?;
                fs::canonicalize(&joined).map_err(|error| {
                    format!("cannot resolve input_root {}: {error}", path.display())
                })?
            }
            None => recipe_parent,
        };
        if !root.is_dir() {
            return Err(format!("input_root {} is not a directory", root.display()));
        }
        Ok(Self { root })
    }

    fn resolve(&self, declared: &Path) -> Result<PathBuf, String> {
        validate_relative_path(declared, "input")?;
        reject_symlink_path(&self.root, declared, "input")?;
        let resolved = fs::canonicalize(self.root.join(declared))
            .map_err(|error| format!("cannot resolve input {}: {error}", declared.display()))?;
        if !resolved.starts_with(&self.root) {
            return Err(format!(
                "input {} escapes declared input_root",
                declared.display()
            ));
        }
        if !resolved.is_file() {
            return Err(format!(
                "input {} is not a regular file",
                declared.display()
            ));
        }
        Ok(resolved)
    }
}

fn validate_relative_path(path: &Path, label: &str) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(format!("{label} must be a non-empty relative path"));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(format!(
            "{label} {} must not contain a parent or root component",
            path.display()
        ));
    }
    Ok(())
}

fn reject_symlink_path(base: &Path, declared: &Path, label: &str) -> Result<(), String> {
    let mut current = base.to_path_buf();
    for component in declared.components() {
        if component == Component::CurDir {
            continue;
        }
        current.push(component.as_os_str());
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("cannot inspect {label} {}: {error}", declared.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{label} {} traverses a symbolic link",
                declared.display()
            ));
        }
    }
    Ok(())
}

fn validate_recipe(recipe: &AssemblyRecipe) -> Result<(), String> {
    let identity_supported = matches!(
        (recipe.schema_version, recipe.schema.as_str()),
        (RECIPE_SCHEMA_VERSION_V3, RECIPE_SCHEMA_ID_V3)
            | (RECIPE_SCHEMA_VERSION_V4, RECIPE_SCHEMA_ID_V4)
            | (RECIPE_SCHEMA_VERSION_V5, RECIPE_SCHEMA_ID_V5)
            | (RECIPE_SCHEMA_VERSION_V6, RECIPE_SCHEMA_ID_V6)
            | (RECIPE_SCHEMA_VERSION_V7, RECIPE_SCHEMA_ID_V7)
    );
    if !identity_supported {
        return Err(
            "unsupported assembly recipe identity; expected schema_version 3/4/5/6/7 with its matching character-assembly-recipe URN"
                .into(),
        );
    }
    if recipe.schema_version == RECIPE_SCHEMA_VERSION_V3 && recipe.rest_bind_scale.is_some() {
        return Err("assembly recipe v3 does not admit rest_bind_scale; use recipe v4".into());
    }
    if let Some(scale) = &recipe.rest_bind_scale {
        if !scale.expected_factor().is_finite() || scale.expected_factor() <= 0.0 {
            return Err(
                "rest_bind_scale.expected_factor must be finite and greater than zero".into(),
            );
        }
        if recipe.schema_version == RECIPE_SCHEMA_VERSION_V7 {
            let Some(root_node_name) = scale.root_node_name() else {
                return Err(
                    "character-assembly-recipe v7 rest_bind_scale requires root_node_name and does not admit source indices"
                        .into(),
                );
            };
            if root_node_name.is_empty() || root_node_name.trim() != root_node_name {
                return Err(
                    "rest_bind_scale.root_node_name must be non-empty and contain no leading or trailing whitespace"
                        .into(),
                );
            }
        } else {
            if scale.root_node_name().is_some() {
                return Err(format!(
                    "character-assembly-recipe v{} rest_bind_scale does not admit root_node_name",
                    recipe.schema_version
                ));
            }
        }
        if recipe.schema_version == RECIPE_SCHEMA_VERSION_V4
            && (recipe.canonicalize_skin
                || recipe.ground_and_center
                || !recipe.remove_nodes.is_empty())
        {
            return Err(
                "rest_bind_scale cannot be combined with canonicalize_skin, ground_and_center, or remove_nodes because those operations change its proved basis"
                    .into(),
            );
        }
    }
    if !recipe.fps.is_finite() || recipe.fps <= 0.0 || recipe.fps > 1000.0 {
        return Err("fps must be finite and in 0..=1000".into());
    }
    if recipe.clips.is_empty() {
        return Err("clips must contain at least one entry".into());
    }
    unique_nonempty(&recipe.mesh_instances, "mesh_instances")?;
    unique_nonempty(&recipe.remove_nodes, "remove_nodes")?;
    let mut names = BTreeSet::new();
    for clip in &recipe.clips {
        if clip.name.is_empty() || clip.take.is_empty() {
            return Err("clip name and take must not be empty".into());
        }
        if !names.insert(&clip.name) {
            return Err(format!("duplicate output clip name {:?}", clip.name));
        }
        unique_nonempty(&clip.strip_bones, "strip_bones")?;
        if clip.frame_window.is_some() && clip.time_window.is_some() {
            return Err(format!(
                "clip {:?} declares both frame_window and time_window",
                clip.name
            ));
        }
        if let Some([start, end]) = clip.frame_window
            && (start == 0 || end < start)
        {
            return Err(format!(
                "clip {:?} frame_window must be one-based and increasing",
                clip.name
            ));
        }
        if let Some([start, end]) = clip.time_window
            && (!start.is_finite() || !end.is_finite() || start < 0.0 || end <= start)
        {
            return Err(format!(
                "clip {:?} time_window must be finite, non-negative, and increasing",
                clip.name
            ));
        }
    }
    if recipe.ground_and_center && !recipe.canonicalize_skin {
        return Err("ground_and_center requires canonicalize_skin = true".into());
    }
    Ok(())
}

fn parse_recipe(text: &str) -> Result<AssemblyRecipe, String> {
    let wire: AssemblyRecipe<toml::Value> =
        toml::from_str(text).map_err(|error| format!("invalid assembly recipe: {error}"))?;
    if !matches!(
        (wire.schema_version, wire.schema.as_str()),
        (RECIPE_SCHEMA_VERSION_V3, RECIPE_SCHEMA_ID_V3)
            | (RECIPE_SCHEMA_VERSION_V4, RECIPE_SCHEMA_ID_V4)
            | (RECIPE_SCHEMA_VERSION_V5, RECIPE_SCHEMA_ID_V5)
            | (RECIPE_SCHEMA_VERSION_V6, RECIPE_SCHEMA_ID_V6)
            | (RECIPE_SCHEMA_VERSION_V7, RECIPE_SCHEMA_ID_V7)
    ) {
        return Err(
            "invalid assembly recipe: unsupported assembly recipe identity; expected schema_version 3/4/5/6/7 with its matching character-assembly-recipe URN"
                .into(),
        );
    }
    let rest_bind_scale = wire
        .rest_bind_scale
        .as_ref()
        .map(|scale| parse_rest_bind_scale_wire(scale, wire.schema_version))
        .transpose()
        .map_err(|error| format!("invalid assembly recipe: {error}"))?;
    Ok(wire.with_rest_bind_scale(rest_bind_scale))
}

fn parse_rest_bind_scale_wire(
    scale: &toml::Value,
    version: u32,
) -> Result<AssemblyRestBindScaleRecipe, String> {
    if version == RECIPE_SCHEMA_VERSION_V3 {
        return Err("unknown field `rest_bind_scale` in character-assembly-recipe v3".into());
    }
    let table = scale.as_table().ok_or_else(|| {
        format!("character-assembly-recipe v{version} rest_bind_scale must be a table")
    })?;
    let identity = format!("character-assembly-recipe v{version} rest_bind_scale");
    if version == RECIPE_SCHEMA_VERSION_V7 {
        if table.contains_key("source_skin_index") || table.contains_key("source_root_node_index") {
            return Err(format!("{identity} does not admit source indices"));
        }
        reject_unknown_scale_fields(table, &["root_node_name", "expected_factor"], &identity)?;
        require_scale_field(table, "root_node_name", &identity)?;
        require_scale_field(table, "expected_factor", &identity)?;
        let root_node_name = table
            .get("root_node_name")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| "rest_bind_scale.root_node_name must be a string".to_owned())?;
        Ok(AssemblyRestBindScaleRecipe::Named(
            NamedRestBindScaleRecipe {
                root_node_name: root_node_name.to_owned(),
                expected_factor: scale_expected_factor(table)?,
            },
        ))
    } else {
        if table.contains_key("root_node_name") {
            return Err(format!("{identity} does not admit root_node_name"));
        }
        reject_unknown_scale_fields(
            table,
            &[
                "source_skin_index",
                "source_root_node_index",
                "expected_factor",
            ],
            &identity,
        )?;
        for field in [
            "source_skin_index",
            "source_root_node_index",
            "expected_factor",
        ] {
            require_scale_field(table, field, &identity)?;
        }
        Ok(AssemblyRestBindScaleRecipe::Indexed(
            IndexedRestBindScaleRecipe {
                source_skin_index: scale_source_index(table, "source_skin_index")?,
                source_root_node_index: scale_source_index(table, "source_root_node_index")?,
                expected_factor: scale_expected_factor(table)?,
            },
        ))
    }
}

fn scale_source_index(table: &toml::Table, field: &str) -> Result<usize, String> {
    table
        .get(field)
        .and_then(toml::Value::as_integer)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("rest_bind_scale.{field} must be a non-negative integer"))
}

fn scale_expected_factor(table: &toml::Table) -> Result<f64, String> {
    match table.get("expected_factor") {
        Some(toml::Value::Float(value)) => Ok(*value),
        Some(toml::Value::Integer(value)) => Ok(*value as f64),
        _ => Err("rest_bind_scale.expected_factor must be a number".into()),
    }
}

fn reject_unknown_scale_fields(
    table: &toml::Table,
    allowed: &[&str],
    identity: &str,
) -> Result<(), String> {
    if let Some(field) = table
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(format!("unknown field `{field}` in {identity}"));
    }
    Ok(())
}

fn require_scale_field(table: &toml::Table, field: &str, identity: &str) -> Result<(), String> {
    if !table.contains_key(field) {
        return Err(format!("missing field `{field}` in {identity}"));
    }
    Ok(())
}

fn unique_nonempty(values: &[String], label: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.is_empty() {
            return Err(format!("{label} entries must not be empty"));
        }
        if !seen.insert(value) {
            return Err(format!("{label} contains duplicate entry {value:?}"));
        }
    }
    Ok(())
}

fn input_evidence(
    role: &'static str,
    declared: &Path,
    resolved: &Path,
) -> Result<AssemblyInputEvidence, String> {
    let (sha256, bytes) = read_digest(resolved)?;
    Ok(AssemblyInputEvidence {
        role,
        declared_path: declared.display().to_string(),
        sha256,
        bytes,
    })
}

fn load_input(path: &Path) -> Result<Document, crate::producer::Failure> {
    use crate::producer::Failure;
    let (format, bytes) = crate::capture_input(path).map_err(Failure::operator)?;
    crate::load_bytes_typed(path, format, &bytes).map_err(crate::producer_load_failure)
}

fn rest_bind_operation(
    selector: &ResolvedRestBindScaleSelector,
    expected_factor: f64,
) -> ScaleOperation {
    ScaleOperation::RestBindUniformScale {
        source_skin_index: selector.source_skin_index,
        source_root_node_index: selector.source_root_node_index,
        expected_factor,
    }
}

fn resolve_rest_bind_scale_selector(
    document: &Document,
    scale: &AssemblyRestBindScaleRecipe,
) -> Result<ResolvedRestBindScaleSelector, String> {
    if let Some((source_skin_index, source_root_node_index)) = scale.indexed_selector() {
        let root_node_name = document
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == source_root_node_index)
            .and_then(|node| node.bone)
            .and_then(|bone| document.skeleton.bones.get(bone))
            .map(|bone| bone.name.clone())
            .unwrap_or_default();
        return Ok(ResolvedRestBindScaleSelector {
            source_skin_index,
            source_root_node_index,
            root_node_name,
        });
    }
    let root_node_name = scale
        .root_node_name()
        .ok_or_else(|| "rest_bind_scale has no selector".to_owned())?;
    let resolved = resolve_assembly_scale_named_selector(document, root_node_name).map_err(
        |error| match &error {
            AssemblyScaleNamedSelectorResolutionError::RootNotUnique { matches } => format!(
                "rest_bind_scale root_node_name {root_node_name:?} resolves to {matches} source nodes; expected exactly one"
            ),
            AssemblyScaleNamedSelectorResolutionError::SkinNotUnique { matches } => format!(
                "rest_bind_scale root_node_name {root_node_name:?} fully governs {matches} source skins; expected exactly one"
            ),
            _ => format!("rest_bind_scale root_node_name {root_node_name:?} is invalid: {error}"),
        },
    )?;
    Ok(ResolvedRestBindScaleSelector {
        source_skin_index: resolved.source_skin_index,
        source_root_node_index: resolved.source_root_node_index,
        root_node_name: root_node_name.to_owned(),
    })
}

fn remove_unskinned_instances_in_subtree(
    document: &mut Document,
    removal: &animsmith_core::assembly::NodeSubtreeRemovalPlan,
    explicitly_retained: &[String],
) -> usize {
    let explicitly_retained = explicitly_retained
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let skeleton = &document.skeleton;
    let before = document.assets.instances.len();
    document.assets.instances.retain(|instance| {
        !instance.skin_joints.is_empty()
            || !removal.removes(instance.node)
            || skeleton
                .bones
                .get(instance.node)
                .is_some_and(|bone| explicitly_retained.contains(bone.name.as_str()))
    });
    before - document.assets.instances.len()
}

fn remove_declared_unskinned_instances(
    document: &mut Document,
    remove_nodes: &[String],
    explicitly_retained: &[String],
) -> Result<usize, String> {
    let applicable = remove_nodes
        .iter()
        .filter(|name| {
            document
                .skeleton
                .bones
                .iter()
                .any(|bone| bone.name == name.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    if applicable.is_empty() {
        return Ok(0);
    }
    let removal = animsmith_core::assembly::plan_node_subtree_removal(document, &applicable)
        .map_err(|error| format!("cannot plan declared node removal: {error}"))?;
    Ok(remove_unskinned_instances_in_subtree(
        document,
        &removal,
        explicitly_retained,
    ))
}

fn fbx_scale_stage_document(
    document: &Document,
    remove_nodes: &[String],
    explicitly_retained: &[String],
) -> Result<Document, String> {
    let mut projected = document.clone();
    remove_declared_unskinned_instances(&mut projected, remove_nodes, explicitly_retained)?;
    Ok(projected)
}

fn selected_fbx_base_scale_stage_document(
    document: &Document,
    remove_nodes: &[String],
    requested_mesh_instances: &[String],
) -> Result<(Document, PreparedMeshInstanceSelection), String> {
    let mut projected = document.clone();
    let (mut retained, mut removed) =
        select_mesh_instances(&mut projected, requested_mesh_instances)?;
    removed += remove_declared_unskinned_instances(
        &mut projected,
        remove_nodes,
        requested_mesh_instances,
    )?;
    if !requested_mesh_instances.is_empty()
        && projected
            .assets
            .instances
            .iter()
            .all(|instance| instance.skin_joints.is_empty())
    {
        return Err(
            "mesh_instances selection retains no skinned base mesh instance for rest_bind_scale"
                .into(),
        );
    }
    retain_surviving_mesh_instance_names(&mut retained, &projected);
    Ok((
        projected,
        PreparedMeshInstanceSelection { retained, removed },
    ))
}

/// Project one clip-only input to the domains assembly can actually consume.
///
/// Clip assembly reads the normalized skeleton and selected take tracks, then
/// remaps those tracks onto the authoritative base. Geometry, deformation,
/// materials, and bind state from this input cannot reach the output. Removing
/// them before the private GLB bridge keeps unsupported source deformation from
/// being silently approximated while retaining the named rest basis needed to
/// interpret missing animation channels.
fn clip_scale_stage_document(document: &Document) -> Document {
    let mut projected = document.clone();
    projected.assets.meshes.clear();
    projected.assets.instances.clear();
    projected.assets.materials.clear();
    projected.assets.material_resources = Default::default();
    projected.assets.source_skeleton.skins.clear();
    for bone in &mut projected.skeleton.bones {
        bone.inverse_bind = None;
    }
    projected
}

fn retain_surviving_mesh_instance_names(retained: &mut Vec<String>, document: &Document) {
    let surviving = document
        .assets
        .instances
        .iter()
        .filter_map(|instance| document.skeleton.bones.get(instance.node))
        .map(|bone| bone.name.as_str())
        .collect::<BTreeSet<_>>();
    retained.retain(|name| surviving.contains(name.as_str()));
}

#[allow(clippy::too_many_arguments)]
fn prepare_clip_scale_input(
    role: String,
    declared: &Path,
    sha256: String,
    bytes: u64,
    projection: PreparedClipTrackProjection,
    base_basis: &AssemblyScaleCompatibilityBasis,
    scale: &AssemblyRestBindScaleRecipe,
    context: &AssemblyScalePreparationContext<'_>,
) -> Result<PreparedScaleProjection, crate::producer::Failure> {
    use crate::producer::{Classify as _, Kind, Stage};
    let (document, operation_document, input_format, source_projection) =
        projection.into_documents_and_evidence();
    let root_node_name = scale
        .root_node_name()
        .ok_or_else(|| {
            "clip track rebasing requires the v7 named rest_bind_scale selector".to_owned()
        })
        .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    let (rebased_projection, basis) =
        rebase_assembly_scale_skinless_clip(base_basis, &operation_document, root_node_name)
            .map_err(|error| {
                format!(
                    "rest_bind_scale clip projection rejected input {}: {error}",
                    declared.display()
                )
            })
            .refusal(Stage::Proof, Kind::ProofFailed)?;
    let rebased_stage =
        crate::scale::serialize_fbx_rest_bind_stage(&rebased_projection, context.staging_parent)
            .operator()?;
    let rebased_document = animsmith_gltf::load_bytes(rebased_stage.path(), rebased_stage.bytes())
        .map_err(|error| {
            format!(
                "cannot reload rest_bind_scale clip projection rewrite for input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Proof, Kind::ProofFailed)?;
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        schema: &'static str,
        tool: &'a ToolInfo,
        input_sha256: &'a str,
        basis: &'a AssemblyScaleSkinlessClipBasis,
    }
    let basis_schema = "urn:animsmith:character-assembly-skinless-clip-scale-basis:1";
    let fingerprint_bytes = serde_json::to_vec(&Fingerprint {
        schema: basis_schema,
        tool: context.tool,
        input_sha256: &sha256,
        basis: &basis,
    })
    .map_err(|error| format!("cannot serialize assembly skinless clip scale basis: {error}"))
    .operator()?;
    Ok(PreparedScaleProjection {
        authoritative_document: document,
        rebased_reference_document: rebased_document,
        application: PreparedScaleApplication::ClipTracks,
        mesh_selection: None,
        evidence: AssemblyRestBindScaleInputEvidence {
            role,
            declared_path: declared.display().to_string(),
            sha256,
            bytes,
            basis_schema,
            basis_fingerprint: sha256_hex(&fingerprint_bytes),
            compatible: true,
            compatibility: "compatible",
            application: Some("skinless-clip-tracks"),
            input_format: Some(input_format),
            source_projection: Some(source_projection),
            resolved_root_node_name: Some(root_node_name.to_owned()),
            resolved_source_skin_index: None,
            resolved_source_root_node_index: Some(basis.source_root_node_index),
        },
    })
}

fn prepare_scale_input(
    role: String,
    declared: &Path,
    resolved: &Path,
    scale: &AssemblyRestBindScaleRecipe,
    context: &AssemblyScalePreparationContext<'_>,
    input_role: AssemblyScaleInputRole<'_>,
) -> Result<PreparedScaleProjection, crate::producer::Failure> {
    use crate::producer::{Classify as _, Failure};
    let extension = resolved
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let format = if extension.eq_ignore_ascii_case("gltf") {
        AssemblyScaleInputFormat::Gltf("gltf")
    } else if extension.eq_ignore_ascii_case("glb") {
        AssemblyScaleInputFormat::Gltf("glb")
    } else if context.recipe_version >= RECIPE_SCHEMA_VERSION_V6
        && extension.eq_ignore_ascii_case("fbx")
    {
        AssemblyScaleInputFormat::Fbx
    } else if context.recipe_version < RECIPE_SCHEMA_VERSION_V6 {
        return Err(Failure::operator(format!(
            "rest_bind_scale input {} is not glTF/GLB; assembly scale integration is glTF-only",
            declared.display()
        )));
    } else {
        return Err(Failure::operator(format!(
            "rest_bind_scale input {} is not glTF/GLB or FBX",
            declared.display()
        )));
    };
    let bytes = fs::read(resolved)
        .map_err(|error| format!("cannot read input {}: {error}", declared.display()))
        .operator()?;
    let request = AssemblyScaleInputRequest {
        role,
        declared,
        resolved,
        scale,
        context,
        input_role,
    };
    match format {
        AssemblyScaleInputFormat::Gltf(input_format) => {
            prepare_gltf_scale_input(request, &bytes, input_format)
        }
        AssemblyScaleInputFormat::Fbx => prepare_fbx_scale_input(request, &bytes),
    }
}

fn finish_rest_bind_scale_input(
    role: String,
    declared: &Path,
    sha256: String,
    byte_count: u64,
    context: &AssemblyScalePreparationContext<'_>,
    admitted: AdmittedRestBindProjection,
) -> Result<PreparedScaleProjection, crate::producer::Failure> {
    use crate::producer::Classify as _;
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        schema: &'static str,
        tool: &'a ToolInfo,
        input_sha256: &'a str,
        basis: &'a AssemblyScaleBasis,
    }
    let fingerprint_bytes = serde_json::to_vec(&Fingerprint {
        schema: "urn:animsmith:character-assembly-scale-basis:1",
        tool: context.tool,
        input_sha256: &sha256,
        basis: admitted.compatibility_basis.basis(),
    })
    .map_err(|error| format!("cannot serialize assembly scale basis: {error}"))
    .operator()?;
    Ok(PreparedScaleProjection {
        authoritative_document: admitted.authoritative_document,
        rebased_reference_document: admitted.rebased_reference_document,
        evidence: AssemblyRestBindScaleInputEvidence {
            role,
            declared_path: declared.display().to_string(),
            sha256,
            bytes: byte_count,
            basis_schema: "urn:animsmith:character-assembly-scale-basis:1",
            basis_fingerprint: sha256_hex(&fingerprint_bytes),
            compatible: true,
            compatibility: "compatible",
            application: (context.recipe_version == RECIPE_SCHEMA_VERSION_V7)
                .then_some("rest-bind"),
            input_format: (context.recipe_version >= RECIPE_SCHEMA_VERSION_V6)
                .then_some(admitted.input_format),
            source_projection: (context.recipe_version >= RECIPE_SCHEMA_VERSION_V6)
                .then_some(admitted.source_projection),
            resolved_root_node_name: (context.recipe_version == RECIPE_SCHEMA_VERSION_V7)
                .then(|| admitted.selector.root_node_name.clone()),
            resolved_source_skin_index: (context.recipe_version == RECIPE_SCHEMA_VERSION_V7)
                .then_some(admitted.selector.source_skin_index),
            resolved_source_root_node_index: (context.recipe_version == RECIPE_SCHEMA_VERSION_V7)
                .then_some(admitted.selector.source_root_node_index),
        },
        application: PreparedScaleApplication::RestBind {
            compatibility_basis: Box::new(admitted.compatibility_basis),
            selector: admitted.selector,
        },
        mesh_selection: admitted.mesh_selection,
    })
}

/// Owns FBX raw admission and its normalized/baked role projections.
fn prepare_fbx_scale_input(
    request: AssemblyScaleInputRequest<'_>,
    bytes: &[u8],
) -> Result<PreparedScaleProjection, crate::producer::Failure> {
    use crate::producer::{Classify as _, Kind, Stage};

    let AssemblyScaleInputRequest {
        role,
        declared,
        resolved,
        scale,
        context,
        input_role,
    } = request;

    let resource_root = parent_or_current(resolved);
    let fbx_source =
        animsmith_fbx::load_scale_source_bytes_with_resource_root(resolved, bytes, resource_root)
            .map_err(|error| {
                format!(
                    "rest_bind_scale FBX load rejected input {}: {error}",
                    declared.display()
                )
            })
            .refusal(Stage::Load, Kind::UnreadableSource)?;
    require_external_dependencies_safe_for_publication(
        "assemble",
        resource_root,
        fbx_source.dependency_closure(),
        &[("output", context.output), ("evidence", context.evidence)],
    )
    .operator()?;
    let primary_identity = fbx_source.source_facts().primary_identity();
    let sha256 = primary_identity.sha256().to_owned();
    let byte_count = primary_identity.bytes();
    if context.recipe_version == RECIPE_SCHEMA_VERSION_V7
        && let AssemblyScaleInputRole::ClipTracks { base_basis } = input_role
    {
        animsmith_fbx::require_clip_track_capability_for_source(&fbx_source)
            .map_err(|error| {
                format!(
                    "rest_bind_scale FBX clip-track capability rejected input {}: {error}",
                    declared.display()
                )
            })
            .refusal(Stage::Transform, Kind::UnsupportedSourceDomain)?;
        let clip_projection = clip_scale_stage_document(fbx_source.document());
        let staged =
            crate::scale::serialize_fbx_rest_bind_stage(&clip_projection, context.staging_parent)
                .operator()?;
        let staged_source = preflight_scale_source_bytes(staged.path(), staged.bytes())
            .map_err(|error| {
                format!(
                    "rest_bind_scale staged FBX clip projection preflight rejected input {}: {error}",
                    declared.display()
                )
            })
            .refusal(Stage::Transform, Kind::UnsupportedSourceDomain)?;
        let source_projection = AssemblyScaleInputProjectionEvidence::NormalizedBakedFbx {
            authored_curve_keys_preserved: false,
            raw_source_spans_preserved: false,
            staged_source: staged_source.source_facts().primary_identity().clone(),
            capability: Box::new(fbx_source.inventory().clone()),
        };
        return prepare_clip_scale_input(
            role,
            declared,
            sha256,
            byte_count,
            PreparedClipTrackProjection::normalized_baked_fbx(
                fbx_source.document().clone(),
                staged_source.document().clone(),
                source_projection,
            ),
            base_basis,
            scale,
            context,
        );
    }
    animsmith_fbx::rest_bind_capability_facts_for_source(&fbx_source)
        .map_err(|error| {
            format!(
                "rest_bind_scale FBX capability rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Transform, Kind::UnsupportedSourceDomain)?;
    let (scale_stage_document, mesh_selection) = if context.recipe_version
        == RECIPE_SCHEMA_VERSION_V7
        && matches!(input_role, AssemblyScaleInputRole::BaseRestBind)
    {
        let (document, selection) = selected_fbx_base_scale_stage_document(
            fbx_source.document(),
            context.remove_nodes,
            context.retained_mesh_instances,
        )
        .map_err(|error| {
            format!(
                "rest_bind_scale FBX base projection rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
        (document, Some(selection))
    } else {
        let document = fbx_scale_stage_document(
            fbx_source.document(),
            context.remove_nodes,
            context.retained_mesh_instances,
        )
        .map_err(|error| {
            format!(
                "rest_bind_scale removal projection rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
        (document, None)
    };
    let staged =
        crate::scale::serialize_fbx_rest_bind_stage(&scale_stage_document, context.staging_parent)
            .operator()?;
    let staged_source = preflight_scale_source_bytes(staged.path(), staged.bytes())
        .map_err(|error| {
            format!(
                "rest_bind_scale staged FBX preflight rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Transform, Kind::UnsupportedSourceDomain)?;
    let selector = resolve_rest_bind_scale_selector(fbx_source.document(), scale)
        .map_err(|error| {
            format!(
                "rest_bind_scale selector rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    let staged_operation = map_staged_rest_bind_operation(
        fbx_source.document(),
        staged_source.document(),
        rest_bind_operation(&selector, scale.expected_factor()),
    )
    .map_err(|error| {
        format!(
            "rest_bind_scale FBX selector mapping rejected input {}: {error}",
            declared.display()
        )
    })
    .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    let (rebased_reference_document, compatibility_basis) = prepare_gltf_scale_projection(
        declared,
        staged.path(),
        &staged_source,
        staged_operation,
        scale.compatibility_selector(),
    )?;
    finish_rest_bind_scale_input(
        role,
        declared,
        sha256,
        byte_count,
        context,
        AdmittedRestBindProjection {
            authoritative_document: if mesh_selection.is_some() {
                scale_stage_document
            } else {
                fbx_source.document().clone()
            },
            rebased_reference_document,
            compatibility_basis,
            input_format: "fbx",
            source_projection: AssemblyScaleInputProjectionEvidence::NormalizedBakedFbx {
                authored_curve_keys_preserved: false,
                raw_source_spans_preserved: false,
                staged_source: staged_source.source_facts().primary_identity().clone(),
                capability: Box::new(fbx_source.inventory().clone()),
            },
            selector,
            mesh_selection,
        },
    )
}

/// Owns raw glTF/GLB admission and its existing role-dependent projection.
fn prepare_gltf_scale_input(
    request: AssemblyScaleInputRequest<'_>,
    bytes: &[u8],
    input_format: &'static str,
) -> Result<PreparedScaleProjection, crate::producer::Failure> {
    use crate::producer::{Classify as _, Kind, Stage};

    let AssemblyScaleInputRequest {
        role,
        declared,
        resolved,
        scale,
        context,
        input_role,
    } = request;

    let clip_track_application = context.recipe_version == RECIPE_SCHEMA_VERSION_V7
        && matches!(input_role, AssemblyScaleInputRole::ClipTracks { .. });
    let source = if clip_track_application {
        preflight_clip_track_source_bytes(resolved, bytes).map_err(|error| {
            format!(
                "rest_bind_scale clip-track preflight rejected input {}: {error}",
                declared.display()
            )
        })
    } else {
        preflight_scale_source_bytes(resolved, bytes).map_err(|error| {
            format!(
                "rest_bind_scale preflight rejected input {}: {error}",
                declared.display()
            )
        })
    }
    .refusal(Stage::Load, Kind::UnreadableSource)?;
    let primary_identity = source.source_facts().primary_identity();
    let sha256 = primary_identity.sha256().to_owned();
    let byte_count = primary_identity.bytes();
    let use_clip_track_projection = clip_track_application
        && (source.manifest().skins.is_empty() || source.requires_clip_track_projection());
    if use_clip_track_projection
        && let AssemblyScaleInputRole::ClipTracks { base_basis } = input_role
    {
        let projection = PreparedClipTrackProjection::raw_gltf_clip_tracks(
            source.document().clone(),
            input_format,
            AssemblyScaleInputProjectionEvidence::RawGltf {
                authored_curve_keys_preserved: true,
                raw_source_spans_preserved: true,
            },
        );
        return prepare_clip_scale_input(
            role, declared, sha256, byte_count, projection, base_basis, scale, context,
        );
    }
    let selector = resolve_rest_bind_scale_selector(source.document(), scale)
        .map_err(|error| {
            format!(
                "rest_bind_scale selector rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    let (rebased_reference_document, compatibility_basis) = prepare_gltf_scale_projection(
        declared,
        resolved,
        &source,
        rest_bind_operation(&selector, scale.expected_factor()),
        scale.compatibility_selector(),
    )?;
    finish_rest_bind_scale_input(
        role,
        declared,
        sha256,
        byte_count,
        context,
        AdmittedRestBindProjection {
            authoritative_document: source.document().clone(),
            rebased_reference_document,
            compatibility_basis,
            input_format,
            source_projection: AssemblyScaleInputProjectionEvidence::RawGltf {
                authored_curve_keys_preserved: true,
                raw_source_spans_preserved: true,
            },
            selector,
            mesh_selection: None,
        },
    )
}

fn prepare_gltf_scale_projection(
    declared: &Path,
    source_path: &Path,
    source: &animsmith_gltf::GltfScaleSource,
    operation: ScaleOperation,
    selector: AssemblyScaleSelectorRequest<'_>,
) -> Result<(Document, AssemblyScaleCompatibilityBasis), crate::producer::Failure> {
    use crate::producer::{Classify as _, Kind, Stage};
    let facts = operation_capability_facts_for_source(source, operation)
        .map_err(|error| {
            format!(
                "rest_bind_scale capability rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Transform, Kind::UnsupportedSourceDomain)?;
    let plan = plan_scale(&ScaleRequest {
        operation,
        document: source.document(),
        capability: &facts,
    })
    .map_err(|error| {
        format!(
            "rest_bind_scale plan rejected input {}: {error}",
            declared.display()
        )
    })
    .refusal(Stage::Transform, Kind::TransformRefused)?;
    let compatibility_basis =
        assembly_scale_compatibility_basis(source.document(), &plan, selector)
            .map_err(|error| {
                format!(
                    "rest_bind_scale compatibility basis rejected input {}: {error}",
                    declared.display()
                )
            })
            .refusal(Stage::Proof, Kind::ProofFailed)?;
    let artifact = rewrite_scale_plan(source, &plan)
        .map_err(|error| {
            format!(
                "rest_bind_scale rewrite rejected input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Transform, Kind::TransformRefused)?;
    let rebased_document = animsmith_gltf::load_bytes(source_path, artifact.bytes())
        .map_err(|error| {
            format!(
                "cannot reload rest_bind_scale rewrite for input {}: {error}",
                declared.display()
            )
        })
        .refusal(Stage::Proof, Kind::ProofFailed)?;
    Ok((rebased_document, compatibility_basis))
}

fn require_input_scale_compatibility(
    base: &AssemblyScaleCompatibilityBasis,
    input: &AssemblyScaleCompatibilityBasis,
) -> Result<(), String> {
    require_assembly_scale_compatibility_with_selectors(base, input)
        .map_err(|error| error.to_string())
}

/// One parsed `assemble` invocation, including the global `--config` this
/// command resolves for itself.
pub(crate) struct Request {
    /// Versioned assembly recipe.
    pub(crate) recipe: PathBuf,
    /// Artifact destination.
    pub(crate) output: PathBuf,
    /// Evidence destination; required, and never a substitute for the pair.
    pub(crate) evidence: PathBuf,
    /// Explicit config path, or `None` to auto-load `./animsmith.toml`.
    pub(crate) config: Option<PathBuf>,
    /// Which rendering stdout receives.
    pub(crate) format: Format,
}

/// Run one complete `assemble` invocation.
///
/// Mirrors [`crate::scale::run`]: the command owns its own format dispatch,
/// so the CLI's match arm is one call.
///
/// # Errors
///
/// Returns an operator error (exit `2`) for invalid recipe/config/path/I/O or
/// publication failures. Asset-property failures are typed refusals: this
/// function renders them and returns exit `1` without publishing either
/// destination.
pub(crate) fn run(request: &Request, tool: ToolInfo) -> Result<ExitCode, String> {
    let loaded_config = crate::load_config_with_source(request.config.as_deref())?;
    let published = match assemble(
        &request.recipe,
        &request.output,
        &request.evidence,
        &loaded_config.config,
        loaded_config
            .source
            .as_ref()
            .map(|source| (source.path.as_path(), source.bytes.as_slice())),
        tool.clone(),
    ) {
        Ok(crate::producer::Outcome::Published(published)) => published,
        Ok(crate::producer::Outcome::Rejected(rejection)) => {
            let mut delivery = crate::producer::ProcessRefusalDelivery;
            return crate::producer::emit_rejection(
                crate::producer::Command::Assemble,
                request.format,
                tool,
                rejection,
                &mut delivery,
            );
        }
        Err(message) => return Err(message),
    };
    match request.format {
        // The very bytes the evidence file received, not a second rendering
        // of the same record. A stdout that cannot take them is diagnosed
        // rather than raised: the pair is on disk, and a run that published
        // it does not report an operator error.
        Format::Json => emit(&published.evidence_bytes),
        Format::Text => emit_text(&render::render_assemble_published(
            &request.output,
            &request.evidence,
            published.animations,
            published.meshes,
            published.materials,
        )),
    }
    Ok(ExitCode::SUCCESS)
}

/// Execute one complete assembly and atomically publish its artifact/evidence pair.
fn assemble(
    recipe_path: &Path,
    output: &Path,
    evidence_output: &Path,
    config: &Config,
    config_source: Option<(&Path, &[u8])>,
    tool: ToolInfo,
) -> Result<crate::producer::Outcome<Published>, String> {
    match assemble_inner(
        recipe_path,
        output,
        evidence_output,
        config,
        config_source,
        tool,
    ) {
        Ok(published) => Ok(crate::producer::Outcome::Published(published)),
        Err(crate::producer::Failure::Refusal(rejection)) => {
            Ok(crate::producer::Outcome::Rejected(rejection))
        }
        Err(crate::producer::Failure::Operator(message)) => Err(message),
    }
}

fn assemble_inner(
    recipe_path: &Path,
    output: &Path,
    evidence_output: &Path,
    config: &Config,
    config_source: Option<(&Path, &[u8])>,
    tool: ToolInfo,
) -> Result<Published, crate::producer::Failure> {
    use crate::producer::{Classify as _, Failure, Kind, Stage};
    if !output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
    {
        return Err(Failure::operator(
            "assemble output must use the .glb extension",
        ));
    }
    let output_parent = parent_or_current(output);
    let evidence_parent = parent_or_current(evidence_output);
    require_writable_destination(output).operator()?;
    require_writable_destination(evidence_output).operator()?;
    let output_destination = PublicationDestination::new("artifact", output).operator()?;
    let evidence_destination =
        PublicationDestination::new("evidence", evidence_output).operator()?;
    if output_destination
        .aliases_destination(&evidence_destination)
        .operator()?
    {
        return Err(Failure::operator(
            "artifact and evidence outputs must resolve to different paths",
        ));
    }
    let recipe_bytes = fs::read(recipe_path)
        .map_err(|error| format!("cannot read recipe {}: {error}", recipe_path.display()))
        .operator()?;
    let recipe_text = std::str::from_utf8(&recipe_bytes)
        .map_err(|error| format!("recipe {} is not UTF-8: {error}", recipe_path.display()))
        .operator()?;
    let recipe = parse_recipe(recipe_text).operator()?;
    validate_recipe(&recipe).operator()?;
    let resolver = InputResolver::new(recipe_path, recipe.input_root.as_deref()).operator()?;
    let config_evidence = match config_source {
        Some((path, contents)) => AssemblyConfigEvidence {
            source: "file",
            path: Some(path.display().to_string()),
            sha256: Some(sha256_hex(contents)),
            bytes: Some(
                u64::try_from(contents.len())
                    .map_err(|_| "config size exceeds u64".to_owned())
                    .operator()?,
            ),
        },
        None => AssemblyConfigEvidence {
            source: "built-in-defaults",
            path: None,
            sha256: None,
            bytes: None,
        },
    };

    let base_path = resolver.resolve(&recipe.base_input).operator()?;
    // Every versioned scale path captures every source before any remap or
    // copy. V7's private FBX scale stage may exclude unskinned instances in a
    // declared removal closure, but the captured raw document still feeds
    // assembly and no later reopen can race validation.
    let mut prepared_scale_inputs = BTreeMap::<PathBuf, PreparedScaleProjection>::new();
    let mut rest_bind_input_evidence = Vec::new();
    if let Some(scale) = &recipe.rest_bind_scale {
        let scale_remove_nodes = if recipe.schema_version == RECIPE_SCHEMA_VERSION_V7 {
            recipe.remove_nodes.as_slice()
        } else {
            &[]
        };
        let scale_context = AssemblyScalePreparationContext {
            staging_parent: output_parent,
            output,
            evidence: evidence_output,
            recipe_version: recipe.schema_version,
            remove_nodes: scale_remove_nodes,
            retained_mesh_instances: &recipe.mesh_instances,
            tool: &tool,
        };
        let prepared = prepare_scale_input(
            "base".to_owned(),
            &recipe.base_input,
            &base_path,
            scale,
            &scale_context,
            AssemblyScaleInputRole::BaseRestBind,
        )?;
        let base_basis = match &prepared.application {
            PreparedScaleApplication::RestBind {
                compatibility_basis,
                ..
            } => compatibility_basis.as_ref().clone(),
            PreparedScaleApplication::ClipTracks => {
                return Err("base rest_bind_scale input did not produce a skinned basis".to_owned())
                    .refusal(Stage::Proof, Kind::ProofFailed);
            }
        };
        rest_bind_input_evidence.push(prepared.evidence.clone());
        prepared_scale_inputs.insert(base_path.clone(), prepared);
        for clip_recipe in &recipe.clips {
            let resolved = resolver.resolve(&clip_recipe.input).operator()?;
            if let Some(existing) = prepared_scale_inputs.get(&resolved) {
                let mut evidence = existing.evidence.clone();
                evidence.role = format!("clip:{}", clip_recipe.name);
                evidence.declared_path = clip_recipe.input.display().to_string();
                rest_bind_input_evidence.push(evidence);
                continue;
            }
            let prepared = prepare_scale_input(
                format!("clip:{}", clip_recipe.name),
                &clip_recipe.input,
                &resolved,
                scale,
                &scale_context,
                AssemblyScaleInputRole::ClipTracks {
                    base_basis: &base_basis,
                },
            )?;
            if let PreparedScaleApplication::RestBind {
                compatibility_basis,
                ..
            } = &prepared.application
            {
                require_input_scale_compatibility(&base_basis, compatibility_basis)
                    .map_err(|error| {
                        format!(
                            "rest_bind_scale input {} is incompatible with base: {error}",
                            clip_recipe.input.display()
                        )
                    })
                    .refusal(Stage::Proof, Kind::ProofFailed)?;
            }
            rest_bind_input_evidence.push(prepared.evidence.clone());
            prepared_scale_inputs.insert(resolved, prepared);
        }
    }
    let mut inputs = if let Some(prepared) = prepared_scale_inputs.get(&base_path) {
        vec![AssemblyInputEvidence {
            role: "base",
            declared_path: recipe.base_input.display().to_string(),
            sha256: prepared.evidence.sha256.clone(),
            bytes: prepared.evidence.bytes,
        }]
    } else {
        vec![input_evidence("base", &recipe.base_input, &base_path).operator()?]
    };
    let mut base = prepared_scale_inputs.get(&base_path).map_or_else(
        || load_input(&base_path),
        |prepared| Ok(prepared.authoritative_document.clone()),
    )?;
    // When v5/v6 composes rest/bind reparameterization with assembly transforms,
    // this independently source-rebased branch is the exact final-clip oracle.
    // It deliberately follows the same normalized transforms as the staged
    // branch; only the eventual raw staged-GLB rewrite remains staged-only.
    let mut rebased_base = prepared_scale_inputs
        .get(&base_path)
        .map(|prepared| prepared.rebased_reference_document.clone());
    let prepared_mesh_selection = prepared_scale_inputs
        .get(&base_path)
        .and_then(|prepared| prepared.mesh_selection.clone());
    ensure_unique_bones(&base.skeleton, "base input")
        .refusal(Stage::Load, Kind::InvalidAssetStructure)?;
    let (retained_mesh_instances, removed_mesh_instances) =
        if let Some(selection) = &prepared_mesh_selection {
            (selection.retained.clone(), selection.removed)
        } else {
            select_mesh_instances(&mut base, &recipe.mesh_instances)
                .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?
        };
    if let Some(reference) = &mut rebased_base
        && prepared_mesh_selection.is_none()
    {
        select_mesh_instances(reference, &recipe.mesh_instances)
            .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    }

    let material_application = recipe
        .material_texture_recipe
        .as_deref()
        .map(|declared| {
            let resolved = resolver.resolve(declared).operator()?;
            inputs.push(input_evidence("material_texture_recipe", declared, &resolved).operator()?);
            let mut application =
                apply_material_texture_recipe_in_root(&resolved, &base, &resolver.root)
                    .map_err(crate::material_recipe_failure)?;
            let recipe_base = parent_or_current(&resolved);
            let texture_base = application
                .evidence
                .texture_root
                .as_deref()
                .map_or_else(|| recipe_base.to_path_buf(), |root| recipe_base.join(root));
            for consumed in &application.evidence.consumed_inputs {
                let texture_path = fs::canonicalize(texture_base.join(&consumed.declared_path))
                    .map_err(|error| {
                        format!(
                            "cannot resolve consumed texture {}: {error}",
                            consumed.declared_path
                        )
                    })
                    .operator()?;
                inputs.push(
                    input_evidence("texture", Path::new(&consumed.declared_path), &texture_path)
                        .operator()?,
                );
            }
            // The material helper saw the canonical path needed for its read;
            // assembly evidence retains only the recipe-declared path.
            application.evidence.path = declared.display().to_string();
            Ok::<_, Failure>(application)
        })
        .transpose()?;
    if let Some(application) = &material_application {
        base = application.document.clone();
    }

    // A base file may contain a take, but only recipe-selected clips belong in
    // the product. Canonicalization intentionally accepts a base scene only,
    // then clip remapping targets the canonical skeleton it returns.
    base.clips.clear();
    if let Some(reference) = &mut rebased_base {
        reference.clips.clear();
    }
    let canonicalization = if recipe.canonicalize_skin {
        let options = animsmith_core::SkinnedBindPoseCanonicalizationOptions {
            // Both maintained loaders expose their Document world in
            // right-handed Y-up metres. Raw skinned vertex coordinates can
            // still be source-local; the core operation derives and bakes
            // their geometry-bind transform from the validated scene/IBMs.
            source_to_meters_y_up: animsmith_core::glam::Mat4::IDENTITY,
            placement: if recipe.ground_and_center {
                animsmith_core::SkinnedBindPosePlacement::GroundAndCenter
            } else {
                animsmith_core::SkinnedBindPosePlacement::Preserve
            },
        };
        let canonical = animsmith_core::canonicalize_skinned_bind_pose(&base, options)
            .refusal(Stage::Transform, Kind::TransformRefused)?;
        base = canonical.document.clone();
        Some(canonical)
    } else {
        None
    };
    if let Some(reference) = &mut rebased_base
        && recipe.canonicalize_skin
    {
        let options = animsmith_core::SkinnedBindPoseCanonicalizationOptions {
            source_to_meters_y_up: animsmith_core::glam::Mat4::IDENTITY,
            placement: if recipe.ground_and_center {
                animsmith_core::SkinnedBindPosePlacement::GroundAndCenter
            } else {
                animsmith_core::SkinnedBindPosePlacement::Preserve
            },
        };
        *reference = animsmith_core::canonicalize_skinned_bind_pose(reference, options)
            .refusal(Stage::Transform, Kind::TransformRefused)?
            .document;
    }
    ensure_unique_bones(&base.skeleton, "post-canonicalization base input")
        .refusal(Stage::Transform, Kind::InvalidAssetStructure)?;
    let reference_correspondence = rebased_base
        .as_ref()
        .map(|reference| {
            AssemblyBoneCorrespondence::new(
                &base.skeleton,
                &reference.skeleton,
                "base/reference correspondence",
            )
            .refusal(Stage::Transform, Kind::TransformRefused)
        })
        .transpose()?;
    if let Some(correspondence) = &reference_correspondence {
        correspondence
            .require_names(&recipe.remove_nodes, "node-removal correspondence")
            .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    }
    let node_removal =
        animsmith_core::assembly::plan_node_subtree_removal(&base, &recipe.remove_nodes)
            .map_err(|error| format!("cannot plan node removal: {error}"))
            .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;

    let reference_node_removal = rebased_base
        .as_ref()
        .map(|reference| {
            animsmith_core::assembly::plan_node_subtree_removal(reference, &recipe.remove_nodes)
                .map_err(|error| format!("cannot plan reference node removal: {error}"))
                .refusal(Stage::Selection, Kind::AssetRecipeMismatch)
        })
        .transpose()?;
    if let (Some(reference), Some(reference_removal)) =
        (rebased_base.as_ref(), reference_node_removal.as_ref())
    {
        require_matching_removal_closure(&base, &node_removal, reference, reference_removal)
            .refusal(Stage::Selection, Kind::AssetRecipeMismatch)?;
    }
    let base_index = AssemblyBoneIndex::new(&base.skeleton, "base completion")
        .refusal(Stage::Transform, Kind::TransformRefused)?;
    let mut loaded = BTreeMap::<PathBuf, Document>::new();
    let mut clip_evidence = Vec::with_capacity(recipe.clips.len());
    let mut output_clips = Vec::with_capacity(recipe.clips.len());
    let mut expected_rebased_clips = Vec::with_capacity(recipe.clips.len());
    let mut projected_rebased_clip_names = BTreeSet::new();
    for clip_recipe in &recipe.clips {
        let resolved = resolver.resolve(&clip_recipe.input).operator()?;
        if prepared_scale_inputs
            .get(&resolved)
            .is_some_and(|prepared| {
                matches!(&prepared.application, PreparedScaleApplication::ClipTracks)
            })
        {
            projected_rebased_clip_names.insert(clip_recipe.name.clone());
        }
        if !loaded.contains_key(&resolved) {
            if let Some(prepared) = prepared_scale_inputs.get(&resolved) {
                inputs.push(AssemblyInputEvidence {
                    role: "clip",
                    declared_path: clip_recipe.input.display().to_string(),
                    sha256: prepared.evidence.sha256.clone(),
                    bytes: prepared.evidence.bytes,
                });
                loaded.insert(resolved.clone(), prepared.authoritative_document.clone());
            } else {
                inputs.push(input_evidence("clip", &clip_recipe.input, &resolved).operator()?);
                loaded.insert(resolved.clone(), load_input(&resolved)?);
            }
        }
        let source = &loaded[&resolved];
        let staged =
            process_clip_before_copy(source, &base, clip_recipe, recipe.fps, config, false)
                .refusal(Stage::Transform, Kind::AssetRecipeMismatch)?;
        let rebased = if let (Some(scale_source), Some(scale_base)) = (
            prepared_scale_inputs
                .get(&resolved)
                .map(|prepared| &prepared.rebased_reference_document),
            rebased_base.as_ref(),
        ) {
            Some(
                process_clip_before_copy(
                    scale_source,
                    scale_base,
                    clip_recipe,
                    recipe.fps,
                    config,
                    true,
                )
                .refusal(Stage::Transform, Kind::AssetRecipeMismatch)?,
            )
        } else {
            None
        };
        let authoritative = rebased.as_ref().unwrap_or(&staged);
        clip_evidence.push(AssemblyClipEvidence {
            name: staged.clip.name.clone(),
            declared_input: clip_recipe.input.display().to_string(),
            source_take: clip_recipe.take.clone(),
            source_tracks: staged.source_tracks,
            emitted_tracks: 0,
            remapped_tracks: staged.remapped_tracks,
            bone_remaps: staged.bone_remaps.clone(),
            completed_tracks: 0,
            stripped_tracks: authoritative.stripped_tracks,
            stripped_bone_motion: authoritative.stripped_bone_motion.clone(),
            pruned_constant_tracks: Vec::new(),
            duration_s: authoritative.clip.duration_s,
            frame_window: clip_recipe.frame_window,
            time_window: clip_recipe.time_window,
            dropped_closing_endpoint: clip_recipe.drop_closing_endpoint,
            hold_frames: clip_recipe.hold_frames,
            gait_anchor_frame_offset: authoritative.gait_anchor_frame_offset,
        });
        output_clips.push(staged.clip);
        if let Some(rebased) = rebased {
            expected_rebased_clips.push(rebased.clip);
        }
    }
    let completion_identities = recipe
        .complete_tracks
        .then(|| completion_target_identities(&base, &output_clips))
        .transpose()
        .refusal(Stage::Transform, Kind::TransformRefused)?;
    let staged_completion_targets = completion_identities
        .as_ref()
        .map(|identities| {
            project_completion_targets(&base_index, identities, &node_removal, "base completion")
        })
        .transpose()
        .refusal(Stage::Transform, Kind::TransformRefused)?
        .unwrap_or_default();
    let reference_completion_targets =
        match (rebased_base.as_ref(), reference_node_removal.as_ref()) {
            (Some(_), Some(removal)) => Some(
                completion_identities
                    .as_ref()
                    .map(|identities| {
                        project_completion_targets(
                            &reference_correspondence
                                .as_ref()
                                .ok_or_else(|| "missing base/reference correspondence".to_owned())?
                                .right,
                            identities,
                            removal,
                            "reference completion",
                        )
                    })
                    .transpose()
                    .refusal(Stage::Transform, Kind::TransformRefused)?
                    .unwrap_or_default(),
            ),
            (None, None) => None,
            _ => {
                return Err(
                    "reference completion requires a matching node-removal plan".to_owned(),
                )
                .refusal(Stage::Proof, Kind::ProofFailed);
            }
        };
    for (index, clip_recipe) in recipe.clips.iter().enumerate() {
        let staged_clip = &mut output_clips[index];
        let staged_completed = complete_and_normalize_clip(
            staged_clip,
            &base.skeleton,
            &staged_completion_targets,
            clip_recipe,
            recipe.complete_tracks,
            false,
        )
        .refusal(Stage::Transform, Kind::TransformRefused)?;
        let evidence = &mut clip_evidence[index];
        evidence.completed_tracks = staged_completed;

        if let Some(scale_base) = rebased_base.as_ref() {
            let rebased_clip = &mut expected_rebased_clips[index];
            let completion_targets = reference_completion_targets
                .as_ref()
                .ok_or_else(|| {
                    "reference completion targets were not prepared with the reference skeleton"
                        .to_owned()
                })
                .refusal(Stage::Transform, Kind::TransformRefused)?;
            let rebased_completed = complete_and_normalize_clip(
                rebased_clip,
                &scale_base.skeleton,
                completion_targets,
                clip_recipe,
                recipe.complete_tracks,
                true,
            )
            .refusal(Stage::Transform, Kind::TransformRefused)?;
            evidence.completed_tracks = rebased_completed;
            if recipe.prune_constant_tracks {
                let protected_bones =
                    protected_clip_bones(&scale_base.skeleton, config, &rebased_clip.name);
                let outcome = animsmith_core::transform::prune_constant_tracks(
                    &scale_base.skeleton,
                    rebased_clip,
                    &protected_bones,
                );
                evidence.pruned_constant_tracks = apply_authoritative_pruning(
                    staged_clip,
                    reference_correspondence
                        .as_ref()
                        .ok_or_else(|| {
                            "missing reference/base correspondence for pruning".to_owned()
                        })
                        .refusal(Stage::Proof, Kind::ProofFailed)?,
                    &outcome.removed,
                )
                .refusal(Stage::Proof, Kind::ProofFailed)?;
            }
        } else if recipe.prune_constant_tracks {
            let protected_bones = protected_clip_bones(&base.skeleton, config, &staged_clip.name);
            let outcome = animsmith_core::transform::prune_constant_tracks(
                &base.skeleton,
                staged_clip,
                &protected_bones,
            );
            evidence.pruned_constant_tracks =
                pruned_track_evidence(&base.skeleton, &staged_clip.name, outcome.removed)
                    .refusal(Stage::Proof, Kind::ProofFailed)?;
        }
        evidence.emitted_tracks = staged_clip.tracks.len();
    }
    base.clips = output_clips;
    if let (Some(reference), Some(reference_removal)) =
        (&mut rebased_base, reference_node_removal.as_ref())
    {
        // The glTF writer deliberately omits clips that have no writable
        // tracks. Mirror that public artifact shape before the final exact
        // reload comparison; otherwise a fully stripped clip would make the
        // reference describe a clip the writer cannot emit.
        expected_rebased_clips.retain(|clip| !clip.tracks.is_empty());
        reference.clips = expected_rebased_clips;
        animsmith_core::assembly::apply_node_subtree_removal(reference, reference_removal)
            .map_err(|error| format!("cannot remove selected reference nodes: {error}"))
            .refusal(Stage::Transform, Kind::TransformRefused)?;
        expected_rebased_clips = reference.clips.clone();
    }
    let removed_nodes = node_removal
        .removed_nodes()
        .iter()
        .map(|node| RemovedNodeEvidence {
            name: node.name.clone(),
            original_node_index: node.original_node_index,
            original_parent_node_index: node.original_parent_node_index,
            selected: node.selected,
        })
        .collect();
    animsmith_core::assembly::apply_node_subtree_removal(&mut base, &node_removal)
        .map_err(|error| format!("cannot remove selected nodes: {error}"))
        .refusal(Stage::Transform, Kind::TransformRefused)?;

    let artifact_temp = tempfile::Builder::new()
        .prefix(".animsmith-assemble-")
        .suffix(".glb")
        .tempfile_in(output_parent)
        .map_err(|error| format!("cannot create temporary output: {error}"))
        .operator()?
        .into_temp_path();
    let evidence_temp = tempfile::Builder::new()
        .prefix(".animsmith-assemble-evidence-")
        .suffix(".json")
        .tempfile_in(evidence_parent)
        .map_err(|error| format!("cannot create temporary evidence: {error}"))
        .operator()?
        .into_temp_path();
    let summary = animsmith_gltf::write::write(&base, &artifact_temp)
        .map_err(crate::conversion_write_failure)?;
    let mut rest_bind_scale_evidence = None;
    if let Some(scale) = &recipe.rest_bind_scale {
        let staged_bytes = fs::read(&artifact_temp)
            .map_err(|error| format!("cannot read staged assembly source: {error}"))
            .operator()?;
        let staged_source = preflight_scale_source_bytes(&artifact_temp, &staged_bytes)
            .map_err(|error| format!("staged assembly scale preflight failed: {error}"))
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        let staged_source_sha256 = staged_source
            .source_facts()
            .primary_identity()
            .sha256()
            .to_owned();
        let original_base = &prepared_scale_inputs
            .get(&base_path)
            .ok_or_else(|| "missing captured base scale input".to_owned())
            .refusal(Stage::Proof, Kind::ProofFailed)?
            .authoritative_document;
        let base_application = &prepared_scale_inputs
            .get(&base_path)
            .ok_or_else(|| "missing captured base scale input".to_owned())
            .refusal(Stage::Proof, Kind::ProofFailed)?
            .application;
        let PreparedScaleApplication::RestBind {
            selector: base_selector,
            ..
        } = base_application
        else {
            return Err("captured base scale input has no rest/bind selector".to_owned())
                .refusal(Stage::Proof, Kind::ProofFailed);
        };
        let staged_operation = map_staged_rest_bind_operation(
            original_base,
            staged_source.document(),
            rest_bind_operation(base_selector, scale.expected_factor()),
        )
        .refusal(Stage::Proof, Kind::ProofFailed)?;
        let (effective_source_skin_index, effective_source_root_node_index) = match staged_operation
        {
            ScaleOperation::RestBindUniformScale {
                source_skin_index,
                source_root_node_index,
                ..
            } => (source_skin_index, source_root_node_index),
            _ => {
                return Err(
                    "staged assembly selector mapping did not produce a rest/bind operation"
                        .to_owned(),
                )
                .refusal(Stage::Proof, Kind::ProofFailed);
            }
        };
        let facts = operation_capability_facts_for_source(&staged_source, staged_operation)
            .map_err(|error| format!("staged assembly scale capability failed: {error}"))
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        let plan = plan_scale(&ScaleRequest {
            operation: staged_operation,
            document: staged_source.document(),
            capability: &facts,
        })
        .map_err(|error| format!("staged assembly scale plan failed: {error}"))
        .refusal(Stage::Proof, Kind::ProofFailed)?;
        let artifact = rewrite_scale_plan(&staged_source, &plan)
            .map_err(|error| format!("staged assembly scale rewrite failed: {error}"))
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        let proof = prove_rewritten_rest_bind(&staged_source, &artifact, &plan)
            .map_err(|error| format!("staged assembly scale proof failed: {error}"))
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        fs::write(&artifact_temp, artifact.bytes())
            .map_err(|error| format!("cannot write proved assembly artifact: {error}"))
            .operator()?;
        let read_back_bytes = fs::read(&artifact_temp)
            .map_err(|error| format!("cannot read proved assembly artifact: {error}"))
            .operator()?;
        let read_back_sha256 = sha256_hex(&read_back_bytes);
        let proved_sha256 = sha256_hex(artifact.bytes());
        require_assembly_read_back_match(&read_back_sha256, &proved_sha256)
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        let reloaded = animsmith_gltf::load_bytes(&artifact_temp, &read_back_bytes)
            .map_err(|error| format!("cannot reload proved assembly artifact: {error}"))
            .refusal(Stage::Proof, Kind::ProofFailed)?;
        let expected_rebased_skeleton = &rebased_base
            .as_ref()
            .ok_or_else(|| "missing rebased assembly reference".to_owned())
            .refusal(Stage::Proof, Kind::ProofFailed)?
            .skeleton;
        let final_correspondence = AssemblyBoneCorrespondence::new(
            expected_rebased_skeleton,
            &reloaded.skeleton,
            "final artifact correspondence",
        )
        .refusal(Stage::Proof, Kind::ProofFailed)?;
        require_rebased_clips_match_with_correspondence(
            &expected_rebased_clips,
            &reloaded.clips,
            &final_correspondence,
            &projected_rebased_clip_names,
        )
        .refusal(Stage::Proof, Kind::ProofFailed)?;
        rest_bind_scale_evidence = Some(AssemblyRestBindScaleEvidence {
            source_skin_index: scale.indexed_selector().map(|selector| selector.0),
            source_root_node_index: scale.indexed_selector().map(|selector| selector.1),
            declared_root_node_name: scale.root_node_name().map(str::to_owned),
            effective_source_skin_index: (recipe.schema_version >= RECIPE_SCHEMA_VERSION_V5)
                .then_some(effective_source_skin_index),
            effective_source_root_node_index: (recipe.schema_version >= RECIPE_SCHEMA_VERSION_V5)
                .then_some(effective_source_root_node_index),
            expected_factor: scale.expected_factor(),
            inputs: rest_bind_input_evidence,
            staged_source_sha256,
            read_back_sha256,
            residual_comparison_counts: crate::scale::residual_comparison_counts(&proof.core),
            proof: crate::scale::shared_scale_evidence(&plan, &artifact, &proof)
                .refusal(Stage::Proof, Kind::ProofFailed)?,
        });
    }
    let (artifact_sha256, artifact_bytes) = read_digest(&artifact_temp).operator()?;
    let (evidence_schema_version, evidence_schema) =
        if recipe.schema_version == RECIPE_SCHEMA_VERSION_V7 {
            (EVIDENCE_SCHEMA_VERSION_V7, EVIDENCE_SCHEMA_ID_V7)
        } else if recipe.schema_version == RECIPE_SCHEMA_VERSION_V6 {
            (EVIDENCE_SCHEMA_VERSION_V6, EVIDENCE_SCHEMA_ID_V6)
        } else if recipe.schema_version == RECIPE_SCHEMA_VERSION_V5 {
            (EVIDENCE_SCHEMA_VERSION_V5, EVIDENCE_SCHEMA_ID_V5)
        } else if recipe.schema_version == RECIPE_SCHEMA_VERSION_V4 {
            (EVIDENCE_SCHEMA_VERSION_V4, EVIDENCE_SCHEMA_ID_V4)
        } else {
            (EVIDENCE_SCHEMA_VERSION_V3, EVIDENCE_SCHEMA_ID_V3)
        };
    let evidence = AssemblyEvidence {
        schema_version: evidence_schema_version,
        schema: evidence_schema,
        tool,
        command: "assemble",
        recipe: AssemblyRecipeEvidence {
            path: recipe_path.display().to_string(),
            sha256: sha256_hex(&recipe_bytes),
            effective: recipe.clone(),
        },
        config: config_evidence,
        inputs,
        clips: clip_evidence,
        transforms: AssemblyTransformEvidence {
            retained_mesh_instances,
            removed_mesh_instances,
            removed_nodes,
            canonicalized_skin: recipe.canonicalize_skin,
            ground_and_center: recipe.ground_and_center,
            source_world_to_canonical: canonicalization
                .as_ref()
                .map(|result| result.source_world_to_canonical.to_cols_array()),
            converted_bounds_min: canonicalization
                .as_ref()
                .map(|result| result.converted_bounds_min.to_array()),
            converted_bounds_max: canonicalization
                .as_ref()
                .map(|result| result.converted_bounds_max.to_array()),
        },
        material_texture_recipe: material_application.map(|application| application.evidence),
        rest_bind_scale: rest_bind_scale_evidence,
        artifact: artifact_evidence(output, artifact_sha256, artifact_bytes, summary),
    };
    let evidence_bytes = serialize_record(&evidence).operator()?;
    fs::write(&evidence_temp, &evidence_bytes)
        .map_err(|error| format!("cannot write temporary evidence: {error}"))
        .operator()?;
    publish_pair(
        &artifact_temp,
        output,
        &evidence_temp,
        evidence_output,
        false,
    )
    .operator()?;
    Ok(Published {
        animations: summary.animations,
        meshes: summary.meshes,
        materials: summary.materials,
        evidence_bytes,
    })
}

fn interpolation_name(interpolation: Interpolation) -> &'static str {
    match interpolation {
        Interpolation::Linear => "linear",
        Interpolation::Step => "step",
        Interpolation::CubicSpline => "cubic_spline",
    }
}

fn artifact_evidence(
    path: &Path,
    sha256: String,
    bytes: u64,
    summary: WriteSummary,
) -> AssemblyArtifactEvidence {
    AssemblyArtifactEvidence {
        path: path.display().to_string(),
        sha256,
        bytes,
        nodes: summary.nodes,
        animations: summary.animations,
        meshes: summary.meshes,
        primitive_positions: summary.primitive_positions,
        materials: summary.materials,
        clips_without_writable_tracks: summary.clips_without_writable_tracks,
    }
}

fn exact_take<'a>(doc: &'a Document, take: &str, input: &Path) -> Result<&'a Clip, String> {
    let matches = doc
        .clips
        .iter()
        .filter(|clip| clip.name == take)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [clip] => Ok(clip),
        [] => Err(format!("input {} has no take {take:?}", input.display())),
        _ => Err(format!(
            "input {} has ambiguous duplicate take {take:?}",
            input.display()
        )),
    }
}

/// Apply every pre-copy clip transform in one declared source basis.
///
/// Every scale-enabled recipe invokes this same pipeline for the staged source
/// and its source-rebased counterpart. The staged clip remains the source for the final
/// shared raw rewrite, while scale-sensitive evidence and later membership
/// decisions come from the rebased result.
fn process_clip_before_copy(
    source: &Document,
    base: &Document,
    recipe: &AssemblyClipRecipe,
    fps: f64,
    config: &Config,
    rebased: bool,
) -> Result<PreparedAssemblyClip, String> {
    let context = if rebased { "rebased clip" } else { "clip" };
    ensure_unique_bones(
        &source.skeleton,
        &format!("{context} input {}", recipe.input.display()),
    )?;
    let source_clip = exact_take(source, &recipe.take, &recipe.input)?;
    let source_tracks = source_clip.tracks.len();
    let mut clip = source_clip.clone();
    clip.name.clone_from(&recipe.name);
    apply_window(&mut clip, recipe, fps)?;
    if recipe.drop_closing_endpoint {
        let removed = animsmith_core::assembly::remove_final_keys(&mut clip);
        if removed == 0 || clip.tracks.is_empty() {
            return Err(format!(
                "{context} {:?} has no retained animation after closing-endpoint removal",
                clip.name
            ));
        }
    }
    if recipe.hold_frames > 0 {
        animsmith_core::transform::hold_extend(&mut clip, f64::from(recipe.hold_frames) / fps);
    }
    let bone_correspondence = AssemblyBoneCorrespondence::new(
        &source.skeleton,
        &base.skeleton,
        &format!("{context} bone correspondence"),
    )?;
    let bone_remaps = bone_remap_evidence(&clip, &bone_correspondence)?;
    let remapped_tracks = clip.tracks.len();
    clip = animsmith_core::assembly::remap_clip_to_base(&clip, &source.skeleton, &base.skeleton)
        .map_err(|error| format!("{context} {:?}: {error}", clip.name))?;
    validate_unique_channels(&clip, &base.skeleton)?;
    require_named_bones(&base.skeleton, &recipe.strip_bones, "strip_bones")?;
    let stripped_bone_motion = stripped_bone_motion(&clip, &base.skeleton, &recipe.strip_bones)?;
    let stripped_tracks = animsmith_core::assembly::strip_named_bone_tracks(
        &mut clip,
        &base.skeleton,
        &recipe.strip_bones,
    )
    .map_err(|error| format!("{context} {:?}: {error}", clip.name))?;
    let gait_anchor_frame_offset = if recipe.gait_anchor {
        let roles = resolve_configured_roles(&base.skeleton, &config.rig);
        Some(
            animsmith_core::transform::align_gait_anchor(
                &base.skeleton,
                &mut clip,
                &roles,
                fps,
                animsmith_core::transform::GaitTrajectoryPolicy::InPlace,
            )
            .map_err(|reason| format!("{context} {:?}: {reason}", clip.name))?
            .frame_offset,
        )
    } else {
        None
    };
    Ok(PreparedAssemblyClip {
        clip,
        source_tracks,
        remapped_tracks,
        bone_remaps,
        stripped_tracks,
        stripped_bone_motion,
        gait_anchor_frame_offset,
    })
}

fn completion_target_identities(
    base: &Document,
    clips: &[Clip],
) -> Result<BTreeSet<String>, String> {
    let mut identities = BTreeSet::new();
    for (instance_index, instance) in base.assets.instances.iter().enumerate() {
        for bone in &instance.skin_joints {
            let target = base.skeleton.bones.get(*bone).ok_or_else(|| {
                format!(
                    "base mesh instance {instance_index} completion target {bone} is outside its skeleton"
                )
            })?;
            if target.name.is_empty() {
                return Err(format!(
                    "base mesh instance {instance_index} completion target {bone} has no stable bone name"
                ));
            }
            identities.insert(target.name.clone());
        }
    }
    for clip in clips {
        for (track_index, track) in clip.tracks.iter().enumerate() {
            let target = base.skeleton.bones.get(track.bone).ok_or_else(|| {
                format!(
                    "clip {:?} track {track_index} completion target {} is outside the base skeleton",
                    clip.name, track.bone
                )
            })?;
            if target.name.is_empty() {
                return Err(format!(
                    "clip {:?} track {track_index} completion target {} has no stable bone name",
                    clip.name, track.bone
                ));
            }
            identities.insert(target.name.clone());
        }
    }
    Ok(identities)
}

fn project_completion_targets(
    index: &AssemblyBoneIndex,
    identities: &BTreeSet<String>,
    removal: &animsmith_core::assembly::NodeSubtreeRemovalPlan,
    context: &str,
) -> Result<BTreeSet<usize>, String> {
    identities
        .iter()
        .map(|name| index.resolve(name, context))
        .filter_map(|result| match result {
            Ok(bone) if removal.removes(bone) => None,
            result => Some(result),
        })
        .collect()
}

fn complete_and_normalize_clip(
    clip: &mut Clip,
    skeleton: &Skeleton,
    completion_targets: &BTreeSet<usize>,
    recipe: &AssemblyClipRecipe,
    complete_tracks: bool,
    rebased: bool,
) -> Result<usize, String> {
    let context = if rebased { "rebased clip" } else { "clip" };
    let completed = if complete_tracks {
        let excluded = recipe
            .strip_bones
            .iter()
            .map(|name| {
                skeleton
                    .bones
                    .iter()
                    .position(|bone| bone.name == *name)
                    .ok_or_else(|| format!("strip_bones names missing bone {name:?}"))
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        animsmith_core::assembly::complete_rest_pose_tracks_for_bones(
            clip,
            skeleton,
            completion_targets
                .iter()
                .copied()
                .filter(|bone| !excluded.contains(bone)),
            animsmith_core::assembly::RestPoseTrackOptions::ALL,
        )
        .map_err(|error| format!("{context} {:?}: {error}", clip.name))?
    } else {
        0
    };
    validate_unique_channels(clip, skeleton)?;
    normalize_quaternion_magnitudes(clip)?;
    animsmith_core::assembly::normalize_quaternion_hemispheres(clip);
    Ok(completed)
}

fn protected_clip_bones(skeleton: &Skeleton, config: &Config, clip_name: &str) -> Vec<usize> {
    // `animates_bones` is a per-clip motion contract. Keep its exact-name
    // tracks even when they are mechanically constant, so a later lint can
    // still observe that authored channel. `required_bones` is only a
    // skeleton-presence contract and deliberately does not protect a track.
    config
        .expectations_for(clip_name)
        .animates_bones
        .as_deref()
        .map(|names| {
            skeleton
                .bones
                .iter()
                .enumerate()
                .filter_map(|(index, bone)| {
                    names.iter().any(|name| name == &bone.name).then_some(index)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn apply_authoritative_pruning(
    staged: &mut Clip,
    correspondence: &AssemblyBoneCorrespondence,
    removed: &[animsmith_core::transform::ConstantTrackPruneRecord],
) -> Result<Vec<PrunedConstantTrackEvidence>, String> {
    let staged_channels = indexed_clip_channels(staged, &correspondence.left, "rebased pruning")?;
    let mut removed_indices = BTreeSet::new();
    let mut projected_evidence = Vec::with_capacity(removed.len());
    for record in removed {
        let authoritative_name = correspondence.right.name(
            record.bone,
            &format!("rebased pruning clip {:?}", staged.name),
        )?;
        let staged_bone = correspondence.map_right_name(authoritative_name, "staged pruning")?;
        let staged_channel = staged_channels
            .get(&(authoritative_name.to_owned(), record.property))
            .ok_or_else(|| {
                format!(
                    "rebased pruning cannot resolve staged {:?} track for bone {:?} in clip {:?}",
                    record.property, authoritative_name, staged.name
                )
            })?;
        let staged_index = staged_channel.track_index;
        let track = staged.tracks.get(staged_index).ok_or_else(|| {
            format!(
                "rebased pruning track index {staged_index} is stale for clip {:?}",
                staged.name
            )
        })?;
        if track.interpolation != record.interpolation || track.key_count() != record.key_count {
            return Err(format!(
                "rebased pruning track for bone {:?} does not match authoritative shape in clip {:?}",
                authoritative_name, staged.name
            ));
        }
        if !removed_indices.insert(staged_index) {
            return Err(format!(
                "rebased pruning selected staged track {staged_index} more than once for clip {:?}",
                staged.name
            ));
        }
        projected_evidence.push(PrunedConstantTrackEvidence {
            original_track_index: staged_index,
            bone: authoritative_name.to_owned(),
            bone_index: staged_bone,
            property: record.property.as_str(),
            interpolation: interpolation_name(record.interpolation),
            key_count: record.key_count,
        });
    }
    let tracks = std::mem::take(&mut staged.tracks);
    staged.tracks = tracks
        .into_iter()
        .enumerate()
        .filter_map(|(index, track)| (!removed_indices.contains(&index)).then_some(track))
        .collect();
    projected_evidence.sort_by_key(|record| record.original_track_index);
    Ok(projected_evidence)
}

fn pruned_track_evidence(
    skeleton: &Skeleton,
    clip_name: &str,
    removed: Vec<animsmith_core::transform::ConstantTrackPruneRecord>,
) -> Result<Vec<PrunedConstantTrackEvidence>, String> {
    removed
        .into_iter()
        .map(|record| {
            let bone = skeleton.bones.get(record.bone).ok_or_else(|| {
                format!(
                    "constant-track pruning reported missing bone {} for clip {:?}",
                    record.bone, clip_name
                )
            })?;
            Ok(PrunedConstantTrackEvidence {
                original_track_index: record.original_track_index,
                bone: bone.name.clone(),
                bone_index: record.bone,
                property: record.property.as_str(),
                interpolation: interpolation_name(record.interpolation),
                key_count: record.key_count,
            })
        })
        .collect()
}

fn map_staged_rest_bind_operation(
    original: &Document,
    staged: &Document,
    operation: ScaleOperation,
) -> Result<ScaleOperation, String> {
    let ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    } = operation
    else {
        return Err("assembly staging only maps rest/bind operations".into());
    };
    let original_nodes = indexed_source_nodes(&original.assets.source_skeleton.nodes, "base")?;
    let staged_nodes = indexed_source_nodes(&staged.assets.source_skeleton.nodes, "staged")?;
    let original_skins = indexed_source_skins(&original.assets.source_skeleton.skins, "base")?;
    let staged_skins = indexed_source_skins(&staged.assets.source_skeleton.skins, "staged")?;
    validate_source_node_parents(&original_nodes, "base")?;
    validate_source_node_parents(&staged_nodes, "staged")?;
    let correspondence = AssemblyBoneCorrespondence::new(
        &original.skeleton,
        &staged.skeleton,
        "staged selector correspondence",
    )?;
    let original_root = original_nodes
        .get(&source_root_node_index)
        .ok_or_else(|| format!("base source root id {source_root_node_index} is absent"))?;
    let original_root_bone = original_root
        .bone
        .and_then(|bone| original.skeleton.bones.get(bone))
        .ok_or_else(|| {
            format!(
                "source_root_node_index {} has no named normalized base node",
                source_root_node_index
            )
        })?;
    let staged_root_bone = correspondence.map_staged_selector_name(
        &original_root_bone.name,
        "assembled artifact root correspondence",
    )?;
    let staged_root_matches = staged_nodes
        .values()
        .filter(|node| node.bone == Some(staged_root_bone))
        .map(|node| node.source_node_index)
        .collect::<Vec<_>>();
    let [staged_root_node_index] = staged_root_matches.as_slice() else {
        return Err(format!(
            "assembled artifact does not map root {:?} to exactly one raw node",
            original_root_bone.name
        ));
    };
    let staged_root = staged_nodes
        .get(staged_root_node_index)
        .ok_or_else(|| format!("staged source root id {staged_root_node_index} is absent"))?;
    require_source_parent_correspondence(
        original_root,
        staged_root,
        &original_nodes,
        &staged_nodes,
        &original.skeleton,
        &staged.skeleton,
        "assembled artifact root correspondence",
    )?;
    let original_skin = original_skins
        .get(&source_skin_index)
        .ok_or_else(|| format!("base source skin id {source_skin_index} is absent"))?;
    let joint_names = original_skin
        .joint_source_node_indices
        .iter()
        .map(|source_index| {
            original_nodes
                .get(source_index)
                .ok_or_else(|| format!("base skin joint source id {source_index} is absent"))
                .and_then(|node| {
                    node.bone
                        .and_then(|bone| original.skeleton.bones.get(bone))
                        .ok_or_else(|| {
                            format!("selected base skin joint {source_index} is not named")
                        })
                })
                .map(|bone| bone.name.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if joint_names.is_empty() {
        return Err("selected base skin has no stable joint topology".into());
    }
    if joint_names.iter().collect::<BTreeSet<_>>().len() != joint_names.len() {
        return Err("selected base skin has duplicate named joint identities".into());
    }
    let staged_skin_matches = staged_skins
        .values()
        .filter_map(|skin| {
            let names = skin
                .joint_source_node_indices
                .iter()
                .map(|source_index| {
                    staged_nodes
                        .get(source_index)
                        .ok_or_else(|| {
                            format!("staged skin joint source id {source_index} is absent")
                        })
                        .and_then(|node| {
                            node.bone
                                .and_then(|bone| staged.skeleton.bones.get(bone))
                                .map(|bone| bone.name.clone())
                                .ok_or_else(|| {
                                    format!("staged skin joint {source_index} is not named")
                                })
                        })
                })
                .collect::<Result<Vec<_>, _>>();
            match names {
                Ok(names) if names == joint_names => Some(Ok(skin.source_skin_index)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let [staged_skin_index] = staged_skin_matches.as_slice() else {
        return Err("assembled artifact does not contain exactly one skin with the selected named joint topology".into());
    };
    let selected_staged_skin = staged_skins
        .get(staged_skin_index)
        .ok_or_else(|| format!("staged source skin id {staged_skin_index} is absent"))?;
    for (original_joint, staged_joint) in original_skin
        .joint_source_node_indices
        .iter()
        .zip(&selected_staged_skin.joint_source_node_indices)
    {
        let original_node = original_nodes
            .get(original_joint)
            .ok_or_else(|| format!("base skin joint source id {original_joint} is absent"))?;
        let staged_node = staged_nodes
            .get(staged_joint)
            .ok_or_else(|| format!("staged skin joint source id {staged_joint} is absent"))?;
        let original_bone = original_node
            .bone
            .and_then(|bone| original.skeleton.bones.get(bone))
            .ok_or_else(|| format!("base skin joint source id {original_joint} is not named"))?;
        correspondence
            .map_staged_selector_name(&original_bone.name, "staged skin joint correspondence")?;
        require_source_parent_correspondence(
            original_node,
            staged_node,
            &original_nodes,
            &staged_nodes,
            &original.skeleton,
            &staged.skeleton,
            "staged skin joint correspondence",
        )?;
    }
    Ok(ScaleOperation::RestBindUniformScale {
        source_skin_index: *staged_skin_index,
        source_root_node_index: *staged_root_node_index,
        expected_factor,
    })
}

fn require_source_parent_correspondence(
    left: &SourceNodeAsset,
    right: &SourceNodeAsset,
    left_nodes: &BTreeMap<usize, &SourceNodeAsset>,
    right_nodes: &BTreeMap<usize, &SourceNodeAsset>,
    left_skeleton: &Skeleton,
    right_skeleton: &Skeleton,
    context: &str,
) -> Result<(), String> {
    let ancestry = |node: &SourceNodeAsset,
                    nodes: &BTreeMap<usize, &SourceNodeAsset>,
                    skeleton: &Skeleton|
     -> Result<Vec<(bool, String)>, String> {
        let mut ancestry = Vec::new();
        let mut seen = BTreeSet::from([node.source_node_index]);
        let mut parent = node.parent_source_node_index;
        while let Some(parent_index) = parent {
            if !seen.insert(parent_index) {
                return Err(format!(
                    "{context} source node {} has a cyclic parent chain at source id {parent_index}",
                    node.source_node_index
                ));
            }
            let parent_node = nodes.get(&parent_index).ok_or_else(|| {
                format!(
                    "{context} source node {} has stale parent id {parent_index}",
                    node.source_node_index
                )
            })?;
            let identity = if let Some(bone) = parent_node.bone {
                let bone = skeleton.bones.get(bone).ok_or_else(|| {
                    format!(
                        "{context} source parent {parent_index} references missing normalized bone {bone}"
                    )
                })?;
                if bone.name.is_empty() {
                    return Err(format!(
                        "{context} source parent {parent_index} has an empty normalized identity"
                    ));
                }
                if bone.name != "animsmith-canonical-root" {
                    Some((true, bone.name.clone()))
                } else {
                    None
                }
            } else {
                let name = parent_node.name.as_deref().ok_or_else(|| {
                    format!("{context} source parent {parent_index} has no stable source identity")
                })?;
                if name.is_empty() {
                    return Err(format!(
                        "{context} source parent {parent_index} has an empty source identity"
                    ));
                }
                Some((false, name.to_owned()))
            };
            if let Some(identity) = identity {
                if ancestry.contains(&identity) {
                    return Err(format!(
                        "{context} source node {} has duplicate ancestor identity {:?}",
                        node.source_node_index, identity.1
                    ));
                }
                ancestry.push(identity);
            }
            parent = parent_node.parent_source_node_index;
        }
        Ok(ancestry)
    };
    let left_ancestry = ancestry(left, left_nodes, left_skeleton)?;
    let right_ancestry = ancestry(right, right_nodes, right_skeleton)?;
    if left_ancestry != right_ancestry {
        return Err(format!(
            "{context} ancestor identity differs for consumed source node {}",
            left.source_node_index
        ));
    }
    Ok(())
}

fn indexed_source_nodes<'a>(
    nodes: &'a [SourceNodeAsset],
    context: &str,
) -> Result<BTreeMap<usize, &'a SourceNodeAsset>, String> {
    let mut indexed = BTreeMap::new();
    for node in nodes {
        if indexed.insert(node.source_node_index, node).is_some() {
            return Err(format!(
                "{context} source node id {} is duplicated",
                node.source_node_index
            ));
        }
    }
    Ok(indexed)
}

fn require_matching_removal_closure(
    left: &Document,
    left_plan: &animsmith_core::assembly::NodeSubtreeRemovalPlan,
    right: &Document,
    right_plan: &animsmith_core::assembly::NodeSubtreeRemovalPlan,
) -> Result<(), String> {
    let signature = |document: &Document,
                     plan: &animsmith_core::assembly::NodeSubtreeRemovalPlan,
                     context: &str|
     -> Result<BTreeMap<String, (Option<String>, bool)>, String> {
        let mut result = BTreeMap::new();
        for node in plan.removed_nodes() {
            let parent_name = node
                .original_parent_node_index
                .map(|parent| {
                    document
                        .skeleton
                        .bones
                        .get(parent)
                        .map(|bone| bone.name.clone())
                        .ok_or_else(|| {
                            format!(
                                "{context} removal node {:?} has stale parent index {parent}",
                                node.name
                            )
                        })
                })
                .transpose()?;
            if result
                .insert(node.name.clone(), (parent_name, node.selected))
                .is_some()
            {
                return Err(format!(
                    "{context} removal closure contains duplicate identity {:?}",
                    node.name
                ));
            }
        }
        Ok(result)
    };
    let left_signature = signature(left, left_plan, "base")?;
    let right_signature = signature(right, right_plan, "reference")?;
    if left_signature != right_signature {
        return Err(
            "base/reference node-removal closures differ by stable identity or parent topology"
                .into(),
        );
    }
    Ok(())
}

fn indexed_source_skins<'a>(
    skins: &'a [SourceSkinAsset],
    context: &str,
) -> Result<BTreeMap<usize, &'a SourceSkinAsset>, String> {
    let mut indexed = BTreeMap::new();
    for skin in skins {
        if indexed.insert(skin.source_skin_index, skin).is_some() {
            return Err(format!(
                "{context} source skin id {} is duplicated",
                skin.source_skin_index
            ));
        }
    }
    Ok(indexed)
}

fn validate_source_node_parents(
    nodes: &BTreeMap<usize, &SourceNodeAsset>,
    context: &str,
) -> Result<(), String> {
    for node in nodes.values() {
        if let Some(parent) = node.parent_source_node_index
            && !nodes.contains_key(&parent)
        {
            return Err(format!(
                "{context} source node {} references absent parent source id {parent}",
                node.source_node_index
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
fn require_rebased_clips_match(
    expected: &[Clip],
    expected_skeleton: &Skeleton,
    actual: &[Clip],
    actual_skeleton: &Skeleton,
    tolerant_clip_names: &BTreeSet<String>,
) -> Result<(), String> {
    let correspondence = AssemblyBoneCorrespondence::new(
        expected_skeleton,
        actual_skeleton,
        "final artifact correspondence",
    )?;
    require_rebased_clips_match_with_correspondence(
        expected,
        actual,
        &correspondence,
        tolerant_clip_names,
    )
}

fn require_rebased_clips_match_with_correspondence(
    expected: &[Clip],
    actual: &[Clip],
    correspondence: &AssemblyBoneCorrespondence,
    tolerant_clip_names: &BTreeSet<String>,
) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err(format!(
            "proved artifact has {} clips but pre-remap rebase expected {}",
            actual.len(),
            expected.len()
        ));
    }
    for (clip_index, (expected_clip, actual_clip)) in expected.iter().zip(actual).enumerate() {
        let tolerant = tolerant_clip_names.contains(&expected_clip.name);
        if expected_clip.name != actual_clip.name
            || expected_clip.tracks.len() != actual_clip.tracks.len()
        {
            return Err(format!(
                "proved artifact clip {clip_index} structure differs from its pre-remap rebase"
            ));
        }
        let channel_matches = correspondence.channels(expected_clip, actual_clip, clip_index)?;
        for (track_index, actual_index) in channel_matches {
            let expected_track = &expected_clip.tracks[track_index];
            let actual_track = &actual_clip.tracks[actual_index];
            match (&expected_track.values, &actual_track.values) {
                (TrackValues::Vec3s(expected_values), TrackValues::Vec3s(actual_values)) => {
                    if expected_values.len() != actual_values.len() {
                        return Err(format!(
                            "proved artifact clip {clip_index} track {track_index} value count differs"
                        ));
                    }
                    for (value_index, (expected_value, actual_value)) in
                        expected_values.iter().zip(actual_values).enumerate()
                    {
                        for (component, (expected_component, actual_component)) in expected_value
                            .to_array()
                            .into_iter()
                            .zip(actual_value.to_array())
                            .enumerate()
                        {
                            let tolerance =
                                animsmith_core::scale::ScaleTolerancePolicy::APPENDIX_D_V6;
                            // Only an independently serialized animation-only
                            // clip projection crosses this numeric oracle;
                            // full rest/bind rows remain bit-exact above this
                            // tolerant branch.
                            let close = expected_component.is_finite()
                                && actual_component.is_finite()
                                && (f64::from(expected_component) - f64::from(actual_component))
                                    .abs()
                                    <= tolerance.scalar_tolerance(
                                        f64::from(expected_component),
                                        f64::from(actual_component),
                                    );
                            if expected_component.to_bits() != actual_component.to_bits()
                                && !(tolerant && close)
                            {
                                return Err(format!(
                                    "proved artifact clip {clip_index} track {track_index} stored value {value_index} component {component} differs from its pre-remap rebase: expected {expected_component:?}, observed {actual_component:?}"
                                ));
                            }
                        }
                    }
                }
                (TrackValues::Quats(expected_values), TrackValues::Quats(actual_values)) => {
                    if expected_values.len() != actual_values.len()
                        || expected_values
                            .iter()
                            .zip(actual_values)
                            .any(|(left, right)| {
                                left.to_array()
                                    .into_iter()
                                    .zip(right.to_array())
                                    .any(|(left, right)| left.to_bits() != right.to_bits())
                            })
                    {
                        return Err(format!(
                            "proved artifact clip {clip_index} track {track_index} rotation values differ from its pre-remap rebase"
                        ));
                    }
                }
                _ => {
                    return Err(format!(
                        "proved artifact clip {clip_index} track {track_index} value type differs"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn require_assembly_read_back_match(observed: &str, expected: &str) -> Result<(), String> {
    if observed == expected {
        Ok(())
    } else {
        Err(format!(
            "proved assembly artifact read-back digest mismatch: expected {expected}, observed {observed}"
        ))
    }
}

fn bone_remap_evidence(
    clip: &Clip,
    correspondence: &AssemblyBoneCorrespondence,
) -> Result<Vec<AssemblyBoneRemapEvidence>, String> {
    let referenced = clip
        .tracks
        .iter()
        .map(|track| track.bone)
        .collect::<BTreeSet<_>>();
    referenced
        .into_iter()
        .map(|source_index| {
            let source_name = correspondence.left.name(
                source_index,
                &format!("clip {:?} source bone remap", clip.name),
            )?;
            correspondence.left(
                source_name,
                &format!("clip {:?} source bone remap", clip.name),
            )?;
            let base_index = correspondence.right(
                source_name,
                &format!("clip {:?} base bone remap", clip.name),
            )?;
            Ok(AssemblyBoneRemapEvidence {
                source_bone: source_name.to_owned(),
                source_index,
                base_bone: correspondence
                    .right
                    .name(base_index, "base bone remap")?
                    .to_owned(),
                base_index,
            })
        })
        .collect()
}

fn apply_window(clip: &mut Clip, recipe: &AssemblyClipRecipe, fps: f64) -> Result<(), String> {
    let window = if let Some([start, end]) = recipe.frame_window {
        Some((f64::from(start - 1) / fps, f64::from(end - 1) / fps))
    } else {
        recipe.time_window.map(|[start, end]| (start, end))
    };
    if let Some((start, end)) = window {
        if end > clip.duration_s + 0.5 / fps {
            return Err(format!(
                "clip {:?} window ends at {end:.6}s beyond source duration {:.6}s",
                recipe.name, clip.duration_s
            ));
        }
        animsmith_core::transform::slice(clip, start, end, fps);
    }
    Ok(())
}

fn ensure_unique_bones(skeleton: &Skeleton, label: &str) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for bone in &skeleton.bones {
        if bone.name.is_empty() {
            return Err(format!("{label} contains an unnamed bone"));
        }
        if !names.insert(&bone.name) {
            return Err(format!(
                "{label} contains ambiguous duplicate bone name {:?}",
                bone.name
            ));
        }
    }
    Ok(())
}

fn require_named_bones(skeleton: &Skeleton, names: &[String], label: &str) -> Result<(), String> {
    let available = skeleton
        .bones
        .iter()
        .map(|bone| bone.name.as_str())
        .collect::<BTreeSet<_>>();
    for name in names {
        if !available.contains(name.as_str()) {
            return Err(format!(
                "{label} target {name:?} is absent from the base skeleton"
            ));
        }
    }
    Ok(())
}

fn validate_unique_channels(clip: &Clip, skeleton: &Skeleton) -> Result<(), String> {
    let mut targets = BTreeSet::new();
    for track in &clip.tracks {
        if !targets.insert((track.bone, track.property.as_str())) {
            return Err(format!(
                "clip {:?} contains duplicate {} tracks for bone {:?}",
                clip.name,
                track.property.as_str(),
                skeleton.bone_name(track.bone)
            ));
        }
    }
    Ok(())
}

fn stripped_bone_motion(
    clip: &Clip,
    skeleton: &Skeleton,
    names: &[String],
) -> Result<Vec<StrippedBoneMotionEvidence>, String> {
    let by_name = skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| (bone.name.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    names
        .iter()
        .map(|name| {
            let bone = by_name[name.as_str()];
            let translation = clip
                .tracks
                .iter()
                .find(|track| track.bone == bone && track.property == Property::Translation);
            let (translation_start, translation_end, translation_delta, duration_s) =
                if let Some(track) = translation {
                    let start = track.key_vec3(0);
                    let end = track
                        .key_count()
                        .checked_sub(1)
                        .and_then(|key| track.key_vec3(key));
                    let delta = start.zip(end).map(|(start, end)| (end - start).to_array());
                    (
                        start.map(|value| value.to_array()),
                        end.map(|value| value.to_array()),
                        delta,
                        Some(f64::from(track.end_time() - track.start_time())),
                    )
                } else {
                    (None, None, None, None)
                };
            Ok(StrippedBoneMotionEvidence {
                bone: name.clone(),
                translation_start,
                translation_end,
                translation_delta,
                duration_s,
            })
        })
        .collect()
}

fn normalize_quaternion_magnitudes(clip: &mut Clip) -> Result<(), String> {
    for track in &mut clip.tracks {
        let key_count = track.key_count();
        let interpolation = track.interpolation;
        let TrackValues::Quats(values) = &mut track.values else {
            continue;
        };
        for key in 0..key_count {
            let index = match interpolation {
                Interpolation::CubicSpline => key * 3 + 1,
                _ => key,
            };
            let mut value = values[index];
            if !value.is_finite() || value.length_squared() <= f32::EPSILON {
                return Err(format!(
                    "clip {:?} has a non-finite or zero quaternion at bone {} key {key}",
                    clip.name, track.bone
                ));
            }
            value = value.normalize();
            values[index] = value;
        }
    }
    Ok(())
}

fn select_mesh_instances(
    doc: &mut Document,
    requested: &[String],
) -> Result<(Vec<String>, usize), String> {
    if requested.is_empty() {
        let names = doc
            .assets
            .instances
            .iter()
            .map(|instance| doc.skeleton.bone_name(instance.node).to_owned())
            .collect();
        return Ok((names, 0));
    }
    let requested = requested
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut matched_nodes = BTreeMap::<&str, BTreeSet<usize>>::new();
    for instance in &doc.assets.instances {
        if let Some(bone) = doc.skeleton.bones.get(instance.node)
            && requested.contains(bone.name.as_str())
        {
            matched_nodes
                .entry(bone.name.as_str())
                .or_default()
                .insert(instance.node);
        }
    }
    for name in &requested {
        match matched_nodes.get(name).map_or(0, BTreeSet::len) {
            0 => {
                return Err(format!(
                    "mesh_instances entry {name:?} matches no base mesh instance node"
                ));
            }
            1 => {}
            matches => {
                return Err(format!(
                    "mesh_instances entry {name:?} matches {matches} base mesh instance nodes; expected exactly one"
                ));
            }
        }
    }
    let before = doc.assets.instances.len();
    doc.assets.instances.retain(|instance| {
        doc.skeleton
            .bones
            .get(instance.node)
            .is_some_and(|bone| requested.contains(bone.name.as_str()))
    });
    prune_assets(doc)?;
    Ok((
        requested.into_iter().map(str::to_owned).collect(),
        before - doc.assets.instances.len(),
    ))
}

fn prune_assets(doc: &mut Document) -> Result<(), String> {
    let used_meshes = doc
        .assets
        .instances
        .iter()
        .map(|instance| instance.mesh)
        .collect::<BTreeSet<_>>();
    if used_meshes
        .iter()
        .any(|&index| index >= doc.assets.meshes.len())
    {
        return Err("base mesh instance references an absent mesh definition".into());
    }
    let mesh_map = used_meshes
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, new))
        .collect::<BTreeMap<_, _>>();
    let meshes = used_meshes
        .iter()
        .map(|&index| doc.assets.meshes[index].clone())
        .collect::<Vec<MeshAsset>>();
    for instance in &mut doc.assets.instances {
        instance.mesh = mesh_map[&instance.mesh];
    }
    doc.assets.meshes = meshes;

    let used_materials = doc
        .assets
        .meshes
        .iter()
        .flat_map(|mesh| &mesh.primitives)
        .filter_map(|primitive| primitive.material)
        .collect::<BTreeSet<_>>();
    if used_materials
        .iter()
        .any(|&index| index >= doc.assets.materials.len())
    {
        return Err("base mesh references an absent material".into());
    }
    let material_map = used_materials
        .iter()
        .enumerate()
        .map(|(new, &old)| (old, new))
        .collect::<BTreeMap<_, _>>();
    let materials = used_materials
        .iter()
        .map(|&index| doc.assets.materials[index].clone())
        .collect::<Vec<MaterialAsset>>();
    for primitive in doc
        .assets
        .meshes
        .iter_mut()
        .flat_map(|mesh| &mut mesh.primitives)
    {
        if let Some(material) = primitive.material {
            primitive.material = Some(material_map[&material]);
        }
    }
    doc.assets.materials = materials;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::glam::{Mat4, Quat};
    use animsmith_core::model::{
        Bone, MaterialAsset, MaterialResourceCoverage, Property, Track, Transform,
    };

    fn skeleton(names: &[&str]) -> Skeleton {
        Skeleton {
            bones: names
                .iter()
                .enumerate()
                .map(|(index, name)| Bone {
                    name: (*name).into(),
                    parent: index.checked_sub(1),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                })
                .collect(),
        }
    }

    #[test]
    fn remap_completion_stripping_and_quaternion_normalization_are_deterministic() {
        let source = skeleton(&["child", "root"]);
        let base = skeleton(&["root", "child"]);
        let mut clip = Clip {
            name: "motion".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Rotation,
                interpolation: Interpolation::Linear,
                times: vec![0.0, 1.0],
                values: TrackValues::Quats(vec![Quat::IDENTITY * 2.0, -Quat::IDENTITY]),
            }],
        };

        clip = animsmith_core::assembly::remap_clip_to_base(&clip, &source, &base).unwrap();
        assert_eq!(clip.tracks[0].bone, 1);
        assert_eq!(
            animsmith_core::assembly::strip_named_bone_tracks(&mut clip, &base, ["root"]).unwrap(),
            0
        );
        assert_eq!(
            animsmith_core::assembly::complete_rest_pose_tracks(
                &mut clip,
                &base,
                animsmith_core::assembly::RestPoseTrackOptions::ALL,
            )
            .unwrap(),
            5
        );
        normalize_quaternion_magnitudes(&mut clip).unwrap();
        animsmith_core::assembly::normalize_quaternion_hemispheres(&mut clip);
        let rotation = clip
            .tracks
            .iter()
            .find(|track| track.bone == 1 && track.property == Property::Rotation)
            .unwrap();
        let TrackValues::Quats(values) = &rotation.values else {
            panic!("rotation values");
        };
        assert!((values[0].length() - 1.0).abs() < 1e-6);
        assert!(values[0].dot(values[1]) >= 0.0);
    }

    #[test]
    fn authoritative_pruning_projects_bone_and_track_indices_by_stable_identity() {
        let mut authoritative_skeleton = skeleton(&["root", "retained", "other"]);
        let mut staged_skeleton = skeleton(&["root", "other", "retained"]);
        authoritative_skeleton.bones[2].parent = Some(0);
        staged_skeleton.bones[2].parent = Some(0);
        let translation = Track {
            bone: 0,
            property: Property::Translation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Vec3s(vec![
                animsmith_core::glam::Vec3::ZERO,
                animsmith_core::glam::Vec3::X,
            ]),
        };
        let rotation = Track {
            bone: 1,
            property: Property::Rotation,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            values: TrackValues::Quats(vec![Quat::IDENTITY; 2]),
        };
        let mut authoritative = Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![rotation.clone(), translation.clone()],
        };
        let outcome = animsmith_core::transform::prune_constant_tracks(
            &authoritative_skeleton,
            &mut authoritative,
            &[],
        );
        assert_eq!(outcome.removed.len(), 1);
        assert_eq!(outcome.removed[0].original_track_index, 0);
        assert_eq!(outcome.removed[0].bone, 1);
        let mut staged = Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                translation,
                Track {
                    bone: 2,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Quats(vec![Quat::IDENTITY; 2]),
                },
            ],
        };

        let correspondence = AssemblyBoneCorrespondence::new(
            &staged_skeleton,
            &authoritative_skeleton,
            "pruning test correspondence",
        )
        .unwrap();

        let projected =
            apply_authoritative_pruning(&mut staged, &correspondence, &outcome.removed).unwrap();

        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].original_track_index, 1);
        assert_eq!(projected[0].bone, "retained");
        assert_eq!(projected[0].bone_index, 2);
        assert_eq!(projected[0].property, "rotation");
        assert_eq!(staged.tracks.len(), 1);
        assert_eq!(staged.tracks[0].bone, 0);
        assert_eq!(staged.tracks[0].property, Property::Translation);
    }

    #[test]
    fn removal_correspondence_rejects_a_stale_reference_closure() {
        let base = Document {
            skeleton: skeleton(&["root", "cut", "leaf"]),
            ..Document::default()
        };
        let mut reference = base.clone();
        reference.skeleton.bones[2].parent = Some(0);
        let selected = vec!["cut".to_owned()];
        let base_plan =
            animsmith_core::assembly::plan_node_subtree_removal(&base, &selected).unwrap();
        let reference_plan =
            animsmith_core::assembly::plan_node_subtree_removal(&reference, &selected).unwrap();

        let error =
            require_matching_removal_closure(&base, &base_plan, &reference, &reference_plan)
                .unwrap_err();
        assert!(error.contains("node-removal closures differ"), "{error}");
    }

    #[test]
    fn exact_final_clip_agreement_and_read_back_are_fail_closed() {
        let clip = Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::CubicSpline,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![animsmith_core::glam::Vec3::ZERO; 6]),
                },
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Quats(vec![animsmith_core::glam::Quat::IDENTITY; 2]),
                },
            ],
        };
        let proof_skeleton = skeleton(&["root"]);
        require_rebased_clips_match(
            std::slice::from_ref(&clip),
            &proof_skeleton,
            std::slice::from_ref(&clip),
            &proof_skeleton,
            &BTreeSet::new(),
        )
        .unwrap();
        let independent_skeleton = |names: &[&str]| Skeleton {
            bones: names
                .iter()
                .map(|name| Bone {
                    name: (*name).into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                })
                .collect(),
        };
        let empty_identity = independent_skeleton(&["root", ""]);
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&clip),
                &empty_identity,
                std::slice::from_ref(&clip),
                &proof_skeleton,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("empty stable bone identity")
        );
        let reordered_expected_skeleton = independent_skeleton(&["root", "other"]);
        let reordered_actual_skeleton = independent_skeleton(&["other", "root"]);
        let mut reordered_actual = clip.clone();
        reordered_actual.tracks[0].bone = 1;
        reordered_actual.tracks[1].bone = 1;
        require_rebased_clips_match(
            std::slice::from_ref(&clip),
            &reordered_expected_skeleton,
            &[reordered_actual],
            &reordered_actual_skeleton,
            &BTreeSet::new(),
        )
        .unwrap();
        let drift_expected_skeleton = skeleton(&["root", "child"]);
        let drift_actual_skeleton = skeleton(&["child", "root"]);
        let mut drift_actual = clip.clone();
        drift_actual.tracks[0].bone = 1;
        drift_actual.tracks[1].bone = 1;
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&clip),
                &drift_expected_skeleton,
                &[drift_actual],
                &drift_actual_skeleton,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("ancestor identity")
        );
        let grandparent_expected = skeleton(&["root", "parent", "child"]);
        let grandparent_actual = skeleton(&["other", "parent", "child"]);
        let mut grandparent_clip = clip.clone();
        for track in &mut grandparent_clip.tracks {
            track.bone = 2;
        }
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&grandparent_clip),
                &grandparent_expected,
                std::slice::from_ref(&grandparent_clip),
                &grandparent_actual,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("ancestor identity")
        );
        let mut renamed = clip.clone();
        renamed.name = "run".into();
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&clip),
                &proof_skeleton,
                &[renamed],
                &proof_skeleton,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("clip 0 structure differs")
        );
        let mut second = clip.clone();
        second.name = "run".into();
        assert!(
            require_rebased_clips_match(
                &[clip.clone(), second.clone()],
                &proof_skeleton,
                &[second, clip.clone()],
                &proof_skeleton,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("clip 0 structure differs")
        );
        let mut wrong_channel = clip.clone();
        wrong_channel.tracks[1].property = Property::Translation;
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&clip),
                &proof_skeleton,
                &[wrong_channel],
                &proof_skeleton,
                &BTreeSet::new(),
            )
            .unwrap_err()
            .contains("has ambiguous")
        );
        for property in [Property::Translation, Property::Scale] {
            for component in 0..3 {
                let mut expected_clip = clip.clone();
                expected_clip.tracks[0].property = property;
                let mut changed = expected_clip.clone();
                let TrackValues::Vec3s(values) = &mut changed.tracks[0].values else {
                    panic!("vector fixture")
                };
                values[5].as_mut()[component] = f32::from_bits(1);
                assert!(
                    require_rebased_clips_match(
                        std::slice::from_ref(&expected_clip),
                        &proof_skeleton,
                        &[changed],
                        &proof_skeleton,
                        &BTreeSet::new(),
                    )
                    .unwrap_err()
                    .contains(&format!("stored value 5 component {component} differs"))
                );
            }
        }
        for component in 0..4 {
            let mut changed = clip.clone();
            let TrackValues::Quats(values) = &mut changed.tracks[1].values else {
                panic!("rotation fixture")
            };
            let mut components = values[1].to_array();
            components[component] = if component == 3 {
                1.0f32.next_up()
            } else {
                f32::from_bits(1)
            };
            values[1] = animsmith_core::glam::Quat::from_array(components);
            assert!(
                require_rebased_clips_match(
                    std::slice::from_ref(&clip),
                    &proof_skeleton,
                    &[changed],
                    &proof_skeleton,
                    &BTreeSet::new(),
                )
                .unwrap_err()
                .contains("rotation values differ")
            );
        }
        let mut skinless_names = BTreeSet::new();
        skinless_names.insert("walk".to_owned());
        let tolerance = animsmith_core::scale::ScaleTolerancePolicy::APPENDIX_D_V6;
        for expected in [0.0f32, 1_000.0] {
            let closes = |observed: f32| {
                (f64::from(observed) - f64::from(expected)).abs()
                    <= tolerance.scalar_absolute
                        + tolerance.scalar_relative
                            * f64::from(expected).abs().max(f64::from(observed).abs())
            };
            let limit =
                tolerance.scalar_absolute + tolerance.scalar_relative * f64::from(expected).abs();
            let mut accepted = expected + limit as f32;
            while !closes(accepted) {
                accepted = accepted.next_down();
            }
            let mut refused = accepted.next_up();
            while closes(refused) {
                refused = refused.next_up();
            }
            for (observed, should_pass) in [(accepted, true), (refused, false)] {
                let mut rounded = clip.clone();
                let TrackValues::Vec3s(values) = &mut rounded.tracks[0].values else {
                    panic!("translation fixture")
                };
                values[5].x = expected;
                let mut actual = clip.clone();
                let TrackValues::Vec3s(values) = &mut actual.tracks[0].values else {
                    panic!("translation fixture")
                };
                values[5].x = observed;
                assert_eq!(
                    require_rebased_clips_match(
                        &[rounded],
                        &proof_skeleton,
                        &[actual],
                        &proof_skeleton,
                        &skinless_names,
                    )
                    .is_ok(),
                    should_pass,
                    "Appendix-D boundary expected={expected} observed={observed}"
                );
            }
        }
        let mut rounded_rotation = clip.clone();
        let TrackValues::Quats(values) = &mut rounded_rotation.tracks[1].values else {
            panic!("rotation fixture")
        };
        values[1].x = f32::from_bits(1);
        assert!(
            require_rebased_clips_match(
                std::slice::from_ref(&clip),
                &proof_skeleton,
                &[rounded_rotation],
                &proof_skeleton,
                &skinless_names,
            )
            .unwrap_err()
            .contains("rotation values differ")
        );
        require_assembly_read_back_match("proved", "proved").unwrap();
        let mismatch = require_assembly_read_back_match("mutated", "proved").unwrap_err();
        assert!(mismatch.contains("expected proved"));
        assert!(mismatch.contains("observed mutated"));
    }

    #[test]
    fn staged_rest_bind_selectors_are_mapped_by_named_identity_not_raw_index() {
        let staged = animsmith_gltf::load_bytes(
            Path::new("fixture.glb"),
            &animsmith_testkit::rest_bind_scale_rig_glb(),
        )
        .unwrap();
        let mut original = staged.clone();
        for node in &mut original.assets.source_skeleton.nodes {
            node.source_node_index += 10;
            node.parent_source_node_index = node.parent_source_node_index.map(|index| index + 10);
        }
        original.assets.source_skeleton.skins[0].source_skin_index = 7;
        original.assets.source_skeleton.skins[0].joint_source_node_indices = vec![11];
        original.assets.source_skeleton.skins[0].skeleton_root_source_node_index = Some(10);
        let operation = map_staged_rest_bind_operation(
            &original,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap();
        assert_eq!(
            operation,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            }
        );

        let mut duplicate_root = staged.clone();
        duplicate_root
            .skeleton
            .bones
            .push(duplicate_root.skeleton.bones[0].clone());
        let error = map_staged_rest_bind_operation(
            &original,
            &duplicate_root,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(error.contains("ambiguous stable bone identity"), "{error}");

        let staged_joint_bone = staged
            .assets
            .source_skeleton
            .nodes
            .iter()
            .find(|node| node.source_node_index == 1)
            .and_then(|node| node.bone)
            .unwrap();
        let mut reparented_joint = staged.clone();
        reparented_joint.skeleton.bones[staged_joint_bone].parent = None;
        let reparented = map_staged_rest_bind_operation(
            &original,
            &reparented_joint,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(reparented.contains("ancestor identity"), "{reparented}");

        let mut cyclic_joint = staged.clone();
        cyclic_joint.skeleton.bones[staged_joint_bone].parent = Some(staged_joint_bone);
        let cyclic = map_staged_rest_bind_operation(
            &original,
            &cyclic_joint,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(cyclic.contains("cyclic parent chain"), "{cyclic}");

        let mut source_ancestor_drift = staged.clone();
        let mut alternate_parent = source_ancestor_drift.assets.source_skeleton.nodes[0].clone();
        alternate_parent.source_node_index = 99;
        alternate_parent.parent_source_node_index = None;
        alternate_parent.bone = None;
        alternate_parent.name = Some("other-parent".into());
        source_ancestor_drift
            .assets
            .source_skeleton
            .nodes
            .push(alternate_parent);
        source_ancestor_drift
            .assets
            .source_skeleton
            .nodes
            .iter_mut()
            .find(|node| node.source_node_index == 0)
            .unwrap()
            .parent_source_node_index = Some(99);
        let source_drift = map_staged_rest_bind_operation(
            &original,
            &source_ancestor_drift,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(source_drift.contains("ancestor identity"), "{source_drift}");

        let missing = map_staged_rest_bind_operation(
            &original,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 999,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(
            missing.contains("source root id 999 is absent"),
            "{missing}"
        );

        let mut duplicate_source = original.clone();
        duplicate_source
            .assets
            .source_skeleton
            .nodes
            .push(duplicate_source.assets.source_skeleton.nodes[0].clone());
        let duplicate = map_staged_rest_bind_operation(
            &duplicate_source,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(duplicate.contains("source node id"), "{duplicate}");

        let mut duplicate_skin = original.clone();
        duplicate_skin
            .assets
            .source_skeleton
            .skins
            .push(duplicate_skin.assets.source_skeleton.skins[0].clone());
        let duplicate_skin_error = map_staged_rest_bind_operation(
            &duplicate_skin,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(
            duplicate_skin_error.contains("base source skin id 7 is duplicated"),
            "{duplicate_skin_error}"
        );

        let mut stale_parent = original.clone();
        stale_parent.assets.source_skeleton.nodes[1].parent_source_node_index = Some(999);
        let stale = map_staged_rest_bind_operation(
            &stale_parent,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(stale.contains("absent parent source id 999"), "{stale}");

        let mut missing_joint = original.clone();
        missing_joint.assets.source_skeleton.skins[0].joint_source_node_indices = vec![999];
        let missing_joint_error = map_staged_rest_bind_operation(
            &missing_joint,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(missing_joint_error.contains("skin joint source id 999 is absent"));

        let mut empty_topology = original.clone();
        empty_topology.assets.source_skeleton.skins[0].joint_source_node_indices = Vec::new();
        let empty_topology_error = map_staged_rest_bind_operation(
            &empty_topology,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(
            empty_topology_error.contains("no stable joint topology"),
            "{empty_topology_error}"
        );

        let mut duplicate_topology = original.clone();
        duplicate_topology.assets.source_skeleton.skins[0].joint_source_node_indices = vec![11, 11];
        let duplicate_topology_error = map_staged_rest_bind_operation(
            &duplicate_topology,
            &staged,
            ScaleOperation::RestBindUniformScale {
                source_skin_index: 7,
                source_root_node_index: 10,
                expected_factor: 0.01,
            },
        )
        .unwrap_err();
        assert!(
            duplicate_topology_error.contains("duplicate named joint identities"),
            "{duplicate_topology_error}"
        );
    }

    #[test]
    fn clip_stage_projection_removes_every_unconsumed_domain() {
        let mut document = animsmith_gltf::load_bytes(
            Path::new("fixture.glb"),
            &animsmith_testkit::rest_bind_scale_rig_glb(),
        )
        .unwrap();
        document.assets.materials.push(MaterialAsset {
            name: "unused-clip-material".into(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        });
        document.assets.material_resources.coverage = MaterialResourceCoverage::Complete;
        for bone in &mut document.skeleton.bones {
            bone.inverse_bind = Some(Mat4::IDENTITY);
        }
        assert!(!document.assets.meshes.is_empty());
        assert!(!document.assets.instances.is_empty());
        assert!(!document.assets.source_skeleton.skins.is_empty());
        let skeleton_names = document
            .skeleton
            .bones
            .iter()
            .map(|bone| bone.name.clone())
            .collect::<Vec<_>>();
        let clip_names = document
            .clips
            .iter()
            .map(|clip| clip.name.clone())
            .collect::<Vec<_>>();

        let projected = clip_scale_stage_document(&document);

        assert!(projected.assets.meshes.is_empty());
        assert!(projected.assets.instances.is_empty());
        assert!(projected.assets.materials.is_empty());
        assert_eq!(
            projected.assets.material_resources.coverage,
            MaterialResourceCoverage::Unavailable
        );
        assert!(projected.assets.source_skeleton.skins.is_empty());
        assert!(
            projected
                .skeleton
                .bones
                .iter()
                .all(|bone| bone.inverse_bind.is_none())
        );
        assert_eq!(
            projected
                .skeleton
                .bones
                .iter()
                .map(|bone| bone.name.clone())
                .collect::<Vec<_>>(),
            skeleton_names
        );
        assert_eq!(
            projected
                .clips
                .iter()
                .map(|clip| clip.name.clone())
                .collect::<Vec<_>>(),
            clip_names
        );
    }

    #[test]
    fn explicit_mesh_selection_removes_prop_definitions_and_materials() {
        use animsmith_core::model::{MeshAsset, MeshInstance, SceneAssets};

        let mut document = Document {
            skeleton: skeleton(&["body", "prop"]),
            assets: SceneAssets {
                meshes: vec![
                    MeshAsset {
                        name: "body-mesh".into(),
                        primitives: vec![animsmith_core::model::Primitive {
                            material: Some(0),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    MeshAsset {
                        name: "prop-mesh".into(),
                        primitives: vec![animsmith_core::model::Primitive {
                            material: Some(1),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                instances: vec![
                    MeshInstance {
                        node: 0,
                        mesh: 0,
                        ..Default::default()
                    },
                    MeshInstance {
                        node: 1,
                        mesh: 1,
                        ..Default::default()
                    },
                ],
                materials: vec![
                    MaterialAsset {
                        name: "body-material".into(),
                        base_color: [1.0; 4],
                        metallic: 0.0,
                        roughness: 1.0,
                        base_color_texture: None,
                        normal_texture: None,
                        metallic_roughness_texture: None,
                        occlusion_texture: None,
                    },
                    MaterialAsset {
                        name: "prop-material".into(),
                        base_color: [1.0; 4],
                        metallic: 0.0,
                        roughness: 1.0,
                        base_color_texture: None,
                        normal_texture: None,
                        metallic_roughness_texture: None,
                        occlusion_texture: None,
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let (retained, removed) = select_mesh_instances(&mut document, &["body".into()]).unwrap();
        assert_eq!(retained, ["body"]);
        assert_eq!(removed, 1);
        assert_eq!(document.assets.instances.len(), 1);
        assert_eq!(document.assets.meshes.len(), 1);
        assert_eq!(document.assets.meshes[0].name, "body-mesh");
        assert_eq!(document.assets.materials.len(), 1);
        assert_eq!(document.assets.materials[0].name, "body-material");
        assert_eq!(document.assets.instances[0].mesh, 0);
        assert_eq!(document.assets.meshes[0].primitives[0].material, Some(0));
    }

    #[test]
    fn mesh_selection_refuses_one_name_on_distinct_instance_nodes() {
        use animsmith_core::model::{MeshInstance, SceneAssets};

        let mut document = Document {
            skeleton: skeleton(&["body", "body"]),
            assets: SceneAssets {
                instances: vec![
                    MeshInstance {
                        node: 0,
                        ..Default::default()
                    },
                    MeshInstance {
                        node: 1,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let error = select_mesh_instances(&mut document, &["body".into()]).unwrap_err();
        assert_eq!(
            error,
            "mesh_instances entry \"body\" matches 2 base mesh instance nodes; expected exactly one"
        );
        assert_eq!(document.assets.instances.len(), 2, "refusal is atomic");
    }

    #[test]
    fn surviving_mesh_evidence_preserves_canonical_order_and_uniqueness() {
        use animsmith_core::model::{MeshInstance, SceneAssets};

        let document = Document {
            skeleton: skeleton(&["z-body", "removed", "a-accessory"]),
            assets: SceneAssets {
                instances: vec![
                    MeshInstance {
                        node: 0,
                        ..Default::default()
                    },
                    MeshInstance {
                        node: 0,
                        ..Default::default()
                    },
                    MeshInstance {
                        node: 2,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let mut retained = vec!["a-accessory".into(), "removed".into(), "z-body".into()];

        retain_surviving_mesh_instance_names(&mut retained, &document);

        assert_eq!(retained, ["a-accessory", "z-body"]);
    }

    #[test]
    fn recipe_rejects_duplicate_outputs_and_conflicting_windows() {
        let recipe = AssemblyRecipe {
            schema_version: RECIPE_SCHEMA_VERSION_V3,
            schema: RECIPE_SCHEMA_ID_V3.into(),
            input_root: None,
            base_input: "base.glb".into(),
            mesh_instances: vec![],
            material_texture_recipe: None,
            complete_tracks: false,
            prune_constant_tracks: false,
            remove_nodes: vec![],
            canonicalize_skin: false,
            ground_and_center: false,
            fps: 30.0,
            rest_bind_scale: None,
            clips: vec![
                AssemblyClipRecipe {
                    name: "same".into(),
                    input: "a.glb".into(),
                    take: "take".into(),
                    frame_window: Some([1, 2]),
                    time_window: Some([0.0, 1.0]),
                    drop_closing_endpoint: false,
                    hold_frames: 0,
                    gait_anchor: false,
                    strip_bones: vec![],
                },
                AssemblyClipRecipe {
                    name: "same".into(),
                    input: "b.glb".into(),
                    take: "take".into(),
                    frame_window: None,
                    time_window: None,
                    drop_closing_endpoint: false,
                    hold_frames: 0,
                    gait_anchor: false,
                    strip_bones: vec![],
                },
            ],
        };
        let error = validate_recipe(&recipe).unwrap_err();
        assert!(error.contains("both frame_window and time_window"));
    }
}
