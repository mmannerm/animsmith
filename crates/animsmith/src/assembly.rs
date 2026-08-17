//! Versioned, recipe-driven multi-source character assembly.
//!
//! Assembly is deliberately a generic producer boundary: it combines already
//! extracted asset files, but it does not own archive extraction, consuming
//! project policy, acceptance contracts, or publication.

use crate::material_recipe::{
    MaterialTextureRecipeEvidence, apply_material_texture_recipe_in_root,
};
use crate::publish::{
    destination_identity, emit, emit_text, parent_or_current, publish_pair, read_digest,
    require_writable_destination, serialize_record,
};
use crate::{Format, render};
use animsmith_core::model::{
    Clip, Document, Interpolation, MaterialAsset, MeshAsset, Property, Skeleton, TrackValues,
};
use animsmith_core::scale::{
    AssemblyScaleBasis, ScaleOperation, ScaleRequest, assembly_scale_basis, plan_scale,
    require_assembly_scale_compatibility,
};
use animsmith_core::{Config, ToolInfo, resolve_configured_roles};
use animsmith_gltf::write::WriteSummary;
use animsmith_gltf::{
    operation_capability_facts, preflight_scale_source_bytes, prove_rewritten_rest_bind,
    rewrite_scale_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const RECIPE_SCHEMA_VERSION_V3: u32 = 3;
const RECIPE_SCHEMA_ID_V3: &str = "urn:animsmith:schema:character-assembly-recipe:3";
const RECIPE_SCHEMA_VERSION_V4: u32 = 4;
const RECIPE_SCHEMA_ID_V4: &str = "urn:animsmith:schema:character-assembly-recipe:4";
const EVIDENCE_SCHEMA_VERSION_V3: u32 = 3;
const EVIDENCE_SCHEMA_ID_V3: &str = "urn:animsmith:schema:character-assembly-evidence:3";
const EVIDENCE_SCHEMA_VERSION_V4: u32 = 4;
const EVIDENCE_SCHEMA_ID_V4: &str = "urn:animsmith:schema:character-assembly-evidence:4";

fn default_fps() -> f64 {
    30.0
}

/// The stable recipe consumed by `animsmith assemble`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssemblyRecipe {
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
    rest_bind_scale: Option<AssemblyRestBindScaleRecipe>,
    clips: Vec<AssemblyClipRecipe>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AssemblyRestBindScaleRecipe {
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
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
}

#[derive(Debug, Serialize)]
struct AssemblyRestBindScaleEvidence {
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
    inputs: Vec<AssemblyRestBindScaleInputEvidence>,
    staged_source_sha256: String,
    read_back_sha256: String,
    residual_comparison_counts: crate::scale::ResidualComparisonCounts,
    proof: crate::scale::SharedScaleEvidence,
}

struct PreparedScaleInput {
    document: Document,
    rebased_document: Document,
    basis: AssemblyScaleBasis,
    evidence: AssemblyRestBindScaleInputEvidence,
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
    );
    if !identity_supported {
        return Err(
            "unsupported assembly recipe identity; expected schema_version 3/4 with its matching character-assembly-recipe URN"
                .into(),
        );
    }
    if recipe.schema_version == RECIPE_SCHEMA_VERSION_V3 && recipe.rest_bind_scale.is_some() {
        return Err("assembly recipe v3 does not admit rest_bind_scale; use recipe v4".into());
    }
    if let Some(scale) = recipe.rest_bind_scale {
        if !scale.expected_factor.is_finite() || scale.expected_factor <= 0.0 {
            return Err(
                "rest_bind_scale.expected_factor must be finite and greater than zero".into(),
            );
        }
        if recipe.canonicalize_skin || recipe.ground_and_center || !recipe.remove_nodes.is_empty() {
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
    let value: toml::Value =
        toml::from_str(text).map_err(|error| format!("invalid assembly recipe: {error}"))?;
    let version = value
        .get("schema_version")
        .and_then(toml::Value::as_integer);
    if version == Some(i64::from(RECIPE_SCHEMA_VERSION_V3))
        && value.get("rest_bind_scale").is_some()
    {
        return Err(
            "invalid assembly recipe: unknown field `rest_bind_scale` in character-assembly-recipe v3"
                .into(),
        );
    }
    value
        .try_into()
        .map_err(|error| format!("invalid assembly recipe: {error}"))
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

fn load_input(path: &Path) -> Result<Document, String> {
    crate::load(path)
}

fn rest_bind_operation(recipe: AssemblyRestBindScaleRecipe) -> ScaleOperation {
    ScaleOperation::RestBindUniformScale {
        source_skin_index: recipe.source_skin_index,
        source_root_node_index: recipe.source_root_node_index,
        expected_factor: recipe.expected_factor,
    }
}

fn prepare_scale_input(
    role: String,
    declared: &Path,
    resolved: &Path,
    scale: AssemblyRestBindScaleRecipe,
    tool: &ToolInfo,
) -> Result<PreparedScaleInput, String> {
    let extension = resolved
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("gltf") && !extension.eq_ignore_ascii_case("glb") {
        return Err(format!(
            "rest_bind_scale input {} is not glTF/GLB; assembly scale integration is glTF-only",
            declared.display()
        ));
    }
    let bytes = fs::read(resolved)
        .map_err(|error| format!("cannot read input {}: {error}", declared.display()))?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let byte_count = u64::try_from(bytes.len())
        .map_err(|_| format!("input {} size exceeds u64", declared.display()))?;
    let source = preflight_scale_source_bytes(resolved, &bytes).map_err(|error| {
        format!(
            "rest_bind_scale preflight rejected input {}: {error}",
            declared.display()
        )
    })?;
    let operation = rest_bind_operation(scale);
    let facts = operation_capability_facts(source.manifest(), operation).map_err(|error| {
        format!(
            "rest_bind_scale capability rejected input {}: {error}",
            declared.display()
        )
    })?;
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
    })?;
    let basis = assembly_scale_basis(source.document(), &plan).map_err(|error| {
        format!(
            "rest_bind_scale basis rejected input {}: {error}",
            declared.display()
        )
    })?;
    let artifact = rewrite_scale_plan(&source, &plan).map_err(|error| {
        format!(
            "rest_bind_scale rewrite rejected input {}: {error}",
            declared.display()
        )
    })?;
    let rebased_document =
        animsmith_gltf::load_bytes(resolved, artifact.bytes()).map_err(|error| {
            format!(
                "cannot reload rest_bind_scale rewrite for input {}: {error}",
                declared.display()
            )
        })?;
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        schema: &'static str,
        tool: &'a ToolInfo,
        input_sha256: &'a str,
        basis: &'a AssemblyScaleBasis,
    }
    let fingerprint_bytes = serde_json::to_vec(&Fingerprint {
        schema: "urn:animsmith:character-assembly-scale-basis:1",
        tool,
        input_sha256: &sha256,
        basis: &basis,
    })
    .map_err(|error| format!("cannot serialize assembly scale basis: {error}"))?;
    Ok(PreparedScaleInput {
        document: source.document().clone(),
        rebased_document,
        basis,
        evidence: AssemblyRestBindScaleInputEvidence {
            role,
            declared_path: declared.display().to_string(),
            sha256,
            bytes: byte_count,
            basis_schema: "urn:animsmith:character-assembly-scale-basis:1",
            basis_fingerprint: format!("{:x}", Sha256::digest(fingerprint_bytes)),
            compatible: true,
            compatibility: "compatible",
        },
    })
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
/// Returns an operator error (exit `2`) for every failure — a bad recipe, an
/// unreadable input, an asset the recipe does not fit, or a publication
/// failure alike. Splitting asset-property refusals out is issue #338's job,
/// not this dispatch's.
pub(crate) fn run(request: &Request, tool: ToolInfo) -> Result<ExitCode, String> {
    let loaded_config = crate::load_config_with_source(request.config.as_deref())?;
    let published = assemble(
        &request.recipe,
        &request.output,
        &request.evidence,
        &loaded_config.config,
        loaded_config
            .source
            .as_ref()
            .map(|source| (source.path.as_path(), source.bytes.as_slice())),
        tool,
    )?;
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
) -> Result<Published, String> {
    if !output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
    {
        return Err("assemble output must use the .glb extension".into());
    }
    if output == evidence_output {
        return Err("artifact and evidence outputs must be different paths".into());
    }
    let output_parent = parent_or_current(output);
    let evidence_parent = parent_or_current(evidence_output);
    require_writable_destination(output)?;
    require_writable_destination(evidence_output)?;
    let output_identity = destination_identity(output)?;
    let evidence_identity = destination_identity(evidence_output)?;
    if output_identity == evidence_identity {
        return Err("artifact and evidence outputs must resolve to different paths".into());
    }
    let recipe_bytes = fs::read(recipe_path)
        .map_err(|error| format!("cannot read recipe {}: {error}", recipe_path.display()))?;
    let recipe_text = std::str::from_utf8(&recipe_bytes)
        .map_err(|error| format!("recipe {} is not UTF-8: {error}", recipe_path.display()))?;
    let recipe = parse_recipe(recipe_text)?;
    validate_recipe(&recipe)?;
    let resolver = InputResolver::new(recipe_path, recipe.input_root.as_deref())?;
    let config_evidence = match config_source {
        Some((path, contents)) => AssemblyConfigEvidence {
            source: "file",
            path: Some(path.display().to_string()),
            sha256: Some(format!("{:x}", Sha256::digest(contents))),
            bytes: Some(
                u64::try_from(contents.len()).map_err(|_| "config size exceeds u64".to_owned())?,
            ),
        },
        None => AssemblyConfigEvidence {
            source: "built-in-defaults",
            path: None,
            sha256: None,
            bytes: None,
        },
    };

    let base_path = resolver.resolve(&recipe.base_input)?;
    // The v4 scale path captures and validates every source before any
    // assembly transform, remap, or copy. The same captured normalized
    // documents feed assembly; no later reopen can race validation.
    let mut prepared_scale_inputs = BTreeMap::<PathBuf, PreparedScaleInput>::new();
    let mut rest_bind_input_evidence = Vec::new();
    if let Some(scale) = recipe.rest_bind_scale {
        let prepared = prepare_scale_input(
            "base".to_owned(),
            &recipe.base_input,
            &base_path,
            scale,
            &tool,
        )?;
        let base_basis = prepared.basis.clone();
        rest_bind_input_evidence.push(prepared.evidence.clone());
        prepared_scale_inputs.insert(base_path.clone(), prepared);
        for clip_recipe in &recipe.clips {
            let resolved = resolver.resolve(&clip_recipe.input)?;
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
                &tool,
            )?;
            require_assembly_scale_compatibility(&base_basis, &prepared.basis).map_err(
                |error| {
                    format!(
                        "rest_bind_scale input {} is incompatible with base: {error}",
                        clip_recipe.input.display()
                    )
                },
            )?;
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
        vec![input_evidence("base", &recipe.base_input, &base_path)?]
    };
    let mut base = prepared_scale_inputs.get(&base_path).map_or_else(
        || load_input(&base_path),
        |prepared| Ok(prepared.document.clone()),
    )?;
    ensure_unique_bones(&base.skeleton, "base input")?;
    let (retained_mesh_instances, removed_mesh_instances) =
        select_mesh_instances(&mut base, &recipe.mesh_instances)?;

    let material_application = recipe
        .material_texture_recipe
        .as_deref()
        .map(|declared| {
            let resolved = resolver.resolve(declared)?;
            inputs.push(input_evidence(
                "material_texture_recipe",
                declared,
                &resolved,
            )?);
            let mut application =
                apply_material_texture_recipe_in_root(&resolved, &base, &resolver.root)
                    .map_err(|error| error.to_string())?;
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
                    })?;
                inputs.push(input_evidence(
                    "texture",
                    Path::new(&consumed.declared_path),
                    &texture_path,
                )?);
            }
            // The material helper saw the canonical path needed for its read;
            // assembly evidence retains only the recipe-declared path.
            application.evidence.path = declared.display().to_string();
            Ok::<_, String>(application)
        })
        .transpose()?;
    if let Some(application) = &material_application {
        base = application.document.clone();
    }

    // A base file may contain a take, but only recipe-selected clips belong in
    // the product. Canonicalization intentionally accepts a base scene only,
    // then clip remapping targets the canonical skeleton it returns.
    base.clips.clear();
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
            .map_err(|error| error.to_string())?;
        base = canonical.document.clone();
        Some(canonical)
    } else {
        None
    };
    ensure_unique_bones(&base.skeleton, "post-canonicalization base input")?;
    let node_removal =
        animsmith_core::assembly::plan_node_subtree_removal(&base, &recipe.remove_nodes)
            .map_err(|error| format!("cannot plan node removal: {error}"))?;

    let mut loaded = BTreeMap::<PathBuf, Document>::new();
    let mut clip_evidence = Vec::with_capacity(recipe.clips.len());
    let mut output_clips = Vec::with_capacity(recipe.clips.len());
    let mut expected_rebased_clips = Vec::with_capacity(recipe.clips.len());
    let rebased_base = prepared_scale_inputs
        .get(&base_path)
        .map(|prepared| &prepared.rebased_document);
    for clip_recipe in &recipe.clips {
        let resolved = resolver.resolve(&clip_recipe.input)?;
        if !loaded.contains_key(&resolved) {
            if let Some(prepared) = prepared_scale_inputs.get(&resolved) {
                inputs.push(AssemblyInputEvidence {
                    role: "clip",
                    declared_path: clip_recipe.input.display().to_string(),
                    sha256: prepared.evidence.sha256.clone(),
                    bytes: prepared.evidence.bytes,
                });
                loaded.insert(resolved.clone(), prepared.document.clone());
            } else {
                inputs.push(input_evidence("clip", &clip_recipe.input, &resolved)?);
                loaded.insert(resolved.clone(), load_input(&resolved)?);
            }
        }
        let source = &loaded[&resolved];
        let staged =
            process_clip_before_copy(source, &base, clip_recipe, recipe.fps, config, false)?;
        let rebased = if let (Some(scale_source), Some(scale_base)) = (
            prepared_scale_inputs
                .get(&resolved)
                .map(|prepared| &prepared.rebased_document),
            rebased_base,
        ) {
            Some(process_clip_before_copy(
                scale_source,
                scale_base,
                clip_recipe,
                recipe.fps,
                config,
                true,
            )?)
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
    let mut completion_targets = base
        .assets
        .instances
        .iter()
        .flat_map(|instance| instance.skin_joints.iter().copied())
        .collect::<BTreeSet<_>>();
    completion_targets.extend(
        output_clips
            .iter()
            .flat_map(|clip| clip.tracks.iter().map(|track| track.bone)),
    );
    completion_targets.retain(|bone| !node_removal.removes(*bone));
    for (index, clip_recipe) in recipe.clips.iter().enumerate() {
        let staged_clip = &mut output_clips[index];
        let staged_completed = complete_and_normalize_clip(
            staged_clip,
            &base.skeleton,
            &completion_targets,
            clip_recipe,
            recipe.complete_tracks,
            false,
        )?;
        let evidence = &mut clip_evidence[index];
        evidence.completed_tracks = staged_completed;

        if let Some(scale_base) = rebased_base {
            let rebased_clip = &mut expected_rebased_clips[index];
            let rebased_completed = complete_and_normalize_clip(
                rebased_clip,
                &scale_base.skeleton,
                &completion_targets,
                clip_recipe,
                recipe.complete_tracks,
                true,
            )?;
            evidence.completed_tracks = rebased_completed;
            if recipe.prune_constant_tracks {
                let protected_bones =
                    protected_clip_bones(&scale_base.skeleton, config, &rebased_clip.name);
                let outcome = animsmith_core::transform::prune_constant_tracks(
                    &scale_base.skeleton,
                    rebased_clip,
                    &protected_bones,
                );
                apply_authoritative_pruning(staged_clip, &outcome.removed)?;
                evidence.pruned_constant_tracks = pruned_track_evidence(
                    &scale_base.skeleton,
                    &rebased_clip.name,
                    outcome.removed,
                )?;
            }
        } else if recipe.prune_constant_tracks {
            let protected_bones = protected_clip_bones(&base.skeleton, config, &staged_clip.name);
            let outcome = animsmith_core::transform::prune_constant_tracks(
                &base.skeleton,
                staged_clip,
                &protected_bones,
            );
            evidence.pruned_constant_tracks =
                pruned_track_evidence(&base.skeleton, &staged_clip.name, outcome.removed)?;
        }
        evidence.emitted_tracks = staged_clip.tracks.len();
    }
    base.clips = output_clips;
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
        .map_err(|error| format!("cannot remove selected nodes: {error}"))?;

    let artifact_temp = tempfile::Builder::new()
        .prefix(".animsmith-assemble-")
        .suffix(".glb")
        .tempfile_in(output_parent)
        .map_err(|error| format!("cannot create temporary output: {error}"))?
        .into_temp_path();
    let evidence_temp = tempfile::Builder::new()
        .prefix(".animsmith-assemble-evidence-")
        .suffix(".json")
        .tempfile_in(evidence_parent)
        .map_err(|error| format!("cannot create temporary evidence: {error}"))?
        .into_temp_path();
    let summary =
        animsmith_gltf::write::write(&base, &artifact_temp).map_err(|error| error.to_string())?;
    let mut rest_bind_scale_evidence = None;
    if let Some(scale) = recipe.rest_bind_scale {
        let staged_bytes = fs::read(&artifact_temp)
            .map_err(|error| format!("cannot read staged assembly source: {error}"))?;
        let staged_source_sha256 = format!("{:x}", Sha256::digest(&staged_bytes));
        let staged_source = preflight_scale_source_bytes(&artifact_temp, &staged_bytes)
            .map_err(|error| format!("staged assembly scale preflight failed: {error}"))?;
        let original_base = &prepared_scale_inputs
            .get(&base_path)
            .ok_or_else(|| "missing captured base scale input".to_owned())?
            .document;
        let staged_operation =
            map_staged_rest_bind_operation(original_base, staged_source.document(), scale)?;
        let facts = operation_capability_facts(staged_source.manifest(), staged_operation)
            .map_err(|error| format!("staged assembly scale capability failed: {error}"))?;
        let plan = plan_scale(&ScaleRequest {
            operation: staged_operation,
            document: staged_source.document(),
            capability: &facts,
        })
        .map_err(|error| format!("staged assembly scale plan failed: {error}"))?;
        let artifact = rewrite_scale_plan(&staged_source, &plan)
            .map_err(|error| format!("staged assembly scale rewrite failed: {error}"))?;
        let proof = prove_rewritten_rest_bind(&staged_source, &artifact, &plan)
            .map_err(|error| format!("staged assembly scale proof failed: {error}"))?;
        fs::write(&artifact_temp, artifact.bytes())
            .map_err(|error| format!("cannot write proved assembly artifact: {error}"))?;
        let read_back_bytes = fs::read(&artifact_temp)
            .map_err(|error| format!("cannot read proved assembly artifact: {error}"))?;
        let read_back_sha256 = format!("{:x}", Sha256::digest(&read_back_bytes));
        let proved_sha256 = format!("{:x}", Sha256::digest(artifact.bytes()));
        require_assembly_read_back_match(&read_back_sha256, &proved_sha256)?;
        let reloaded = animsmith_gltf::load_bytes(&artifact_temp, &read_back_bytes)
            .map_err(|error| format!("cannot reload proved assembly artifact: {error}"))?;
        require_rebased_clips_match(&expected_rebased_clips, &reloaded.clips)?;
        rest_bind_scale_evidence = Some(AssemblyRestBindScaleEvidence {
            source_skin_index: scale.source_skin_index,
            source_root_node_index: scale.source_root_node_index,
            expected_factor: scale.expected_factor,
            inputs: rest_bind_input_evidence,
            staged_source_sha256,
            read_back_sha256,
            residual_comparison_counts: crate::scale::residual_comparison_counts(&proof.core),
            proof: crate::scale::shared_scale_evidence(&plan, &artifact, &proof)?,
        });
    }
    let (artifact_sha256, artifact_bytes) = read_digest(&artifact_temp)?;
    let (evidence_schema_version, evidence_schema) =
        if recipe.schema_version == RECIPE_SCHEMA_VERSION_V4 {
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
            sha256: format!("{:x}", Sha256::digest(&recipe_bytes)),
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
    let evidence_bytes = serialize_record(&evidence)?;
    fs::write(&evidence_temp, &evidence_bytes)
        .map_err(|error| format!("cannot write temporary evidence: {error}"))?;
    publish_pair(
        &artifact_temp,
        output,
        &evidence_temp,
        evidence_output,
        false,
    )?;
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
/// The v4 scale path invokes this same pipeline for the staged source and its
/// raw-rebased counterpart. The staged clip remains the source for the final
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
    let bone_remaps = bone_remap_evidence(&clip, &source.skeleton, &base.skeleton)?;
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
    removed: &[animsmith_core::transform::ConstantTrackPruneRecord],
) -> Result<(), String> {
    let removed_indices = removed
        .iter()
        .map(|record| {
            let track = staged
                .tracks
                .get(record.original_track_index)
                .ok_or_else(|| {
                    format!(
                        "rebased pruning selected missing staged track {} for clip {:?}",
                        record.original_track_index, staged.name
                    )
                })?;
            if track.bone != record.bone
                || track.property != record.property
                || track.interpolation != record.interpolation
                || track.key_count() != record.key_count
            {
                return Err(format!(
                    "rebased pruning track {} does not match staged clip {:?}",
                    record.original_track_index, staged.name
                ));
            }
            Ok(record.original_track_index)
        })
        .collect::<Result<BTreeSet<_>, String>>()?;
    let tracks = std::mem::take(&mut staged.tracks);
    staged.tracks = tracks
        .into_iter()
        .enumerate()
        .filter_map(|(index, track)| (!removed_indices.contains(&index)).then_some(track))
        .collect();
    Ok(())
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
    recipe: AssemblyRestBindScaleRecipe,
) -> Result<ScaleOperation, String> {
    let original_root = original
        .assets
        .source_skeleton
        .nodes
        .iter()
        .find(|node| node.source_node_index == recipe.source_root_node_index)
        .and_then(|node| node.bone)
        .and_then(|bone| original.skeleton.bones.get(bone))
        .ok_or_else(|| {
            format!(
                "source_root_node_index {} has no named normalized base node",
                recipe.source_root_node_index
            )
        })?;
    let staged_root_bone = staged
        .skeleton
        .bones
        .iter()
        .position(|bone| bone.name == original_root.name)
        .ok_or_else(|| {
            format!(
                "assembled artifact has no root node named {:?}",
                original_root.name
            )
        })?;
    let staged_root_matches = staged
        .assets
        .source_skeleton
        .nodes
        .iter()
        .filter(|node| node.bone == Some(staged_root_bone))
        .map(|node| node.source_node_index)
        .collect::<Vec<_>>();
    let [staged_root_node_index] = staged_root_matches.as_slice() else {
        return Err(format!(
            "assembled artifact does not map root {:?} to exactly one raw node",
            original_root.name
        ));
    };
    let original_skin = original
        .assets
        .source_skeleton
        .skins
        .iter()
        .find(|skin| skin.source_skin_index == recipe.source_skin_index)
        .ok_or_else(|| {
            format!(
                "source_skin_index {} is absent from base input",
                recipe.source_skin_index
            )
        })?;
    let joint_names = original_skin
        .joint_source_node_indices
        .iter()
        .map(|source_index| {
            original
                .assets
                .source_skeleton
                .nodes
                .iter()
                .find(|node| node.source_node_index == *source_index)
                .and_then(|node| node.bone)
                .and_then(|bone| original.skeleton.bones.get(bone))
                .map(|bone| bone.name.as_str())
                .ok_or_else(|| format!("selected base skin joint {source_index} is not named"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let staged_skin_matches = staged
        .assets
        .source_skeleton
        .skins
        .iter()
        .filter(|skin| {
            let names = skin
                .joint_source_node_indices
                .iter()
                .filter_map(|source_index| {
                    staged
                        .assets
                        .source_skeleton
                        .nodes
                        .iter()
                        .find(|node| node.source_node_index == *source_index)
                        .and_then(|node| node.bone)
                        .and_then(|bone| staged.skeleton.bones.get(bone))
                        .map(|bone| bone.name.as_str())
                })
                .collect::<Vec<_>>();
            names == joint_names
        })
        .map(|skin| skin.source_skin_index)
        .collect::<Vec<_>>();
    let [staged_skin_index] = staged_skin_matches.as_slice() else {
        return Err("assembled artifact does not contain exactly one skin with the selected named joint topology".into());
    };
    Ok(ScaleOperation::RestBindUniformScale {
        source_skin_index: *staged_skin_index,
        source_root_node_index: *staged_root_node_index,
        expected_factor: recipe.expected_factor,
    })
}

fn require_rebased_clips_match(expected: &[Clip], actual: &[Clip]) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err(format!(
            "proved artifact has {} clips but pre-remap rebase expected {}",
            actual.len(),
            expected.len()
        ));
    }
    for (clip_index, (expected_clip, actual_clip)) in expected.iter().zip(actual).enumerate() {
        if expected_clip.name != actual_clip.name
            || expected_clip.tracks.len() != actual_clip.tracks.len()
        {
            return Err(format!(
                "proved artifact clip {clip_index} structure differs from its pre-remap rebase"
            ));
        }
        for (track_index, (expected_track, actual_track)) in expected_clip
            .tracks
            .iter()
            .zip(&actual_clip.tracks)
            .enumerate()
        {
            if expected_track.bone != actual_track.bone
                || expected_track.property != actual_track.property
                || expected_track.interpolation != actual_track.interpolation
                || expected_track.times.len() != actual_track.times.len()
                || expected_track
                    .times
                    .iter()
                    .zip(&actual_track.times)
                    .any(|(left, right)| left.to_bits() != right.to_bits())
            {
                return Err(format!(
                    "proved artifact clip {clip_index} track {track_index} identity differs from its pre-remap rebase"
                ));
            }
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
                            if expected_component.to_bits() != actual_component.to_bits() {
                                return Err(format!(
                                    "proved artifact clip {clip_index} track {track_index} stored value {value_index} component {component} differs from its pre-remap rebase"
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
    source: &Skeleton,
    base: &Skeleton,
) -> Result<Vec<AssemblyBoneRemapEvidence>, String> {
    let referenced = clip
        .tracks
        .iter()
        .map(|track| track.bone)
        .collect::<BTreeSet<_>>();
    let base_by_name = base
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| (bone.name.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    referenced
        .into_iter()
        .map(|source_index| {
            let source_bone = source.bones.get(source_index).ok_or_else(|| {
                format!(
                    "clip {:?} references source bone {source_index}, but its skeleton has {} bones",
                    clip.name,
                    source.bones.len()
                )
            })?;
            let base_index = *base_by_name.get(source_bone.name.as_str()).ok_or_else(|| {
                format!(
                    "clip {:?} source bone {:?} is absent from the base skeleton",
                    clip.name, source_bone.name
                )
            })?;
            Ok(AssemblyBoneRemapEvidence {
                source_bone: source_bone.name.clone(),
                source_index,
                base_bone: base.bones[base_index].name.clone(),
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
    let mut matched = BTreeSet::new();
    let before = doc.assets.instances.len();
    doc.assets.instances.retain(|instance| {
        let keep = doc
            .skeleton
            .bones
            .get(instance.node)
            .is_some_and(|bone| requested.contains(bone.name.as_str()));
        if keep {
            matched.insert(doc.skeleton.bone_name(instance.node).to_owned());
        }
        keep
    });
    for name in &requested {
        if !matched.contains(*name) {
            return Err(format!(
                "mesh_instances entry {name:?} matches no base mesh instance node"
            ));
        }
    }
    prune_assets(doc)?;
    Ok((
        matched.into_iter().collect(),
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
    use animsmith_core::glam::Quat;
    use animsmith_core::model::{Bone, Property, Track, Transform};

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
    fn exact_final_clip_agreement_and_read_back_are_fail_closed() {
        let clip = Clip {
            name: "walk".into(),
            duration_s: 1.0,
            tracks: vec![Track {
                bone: 0,
                property: Property::Translation,
                interpolation: Interpolation::CubicSpline,
                times: vec![0.0, 1.0],
                values: TrackValues::Vec3s(vec![animsmith_core::glam::Vec3::ZERO; 6]),
            }],
        };
        require_rebased_clips_match(std::slice::from_ref(&clip), std::slice::from_ref(&clip))
            .unwrap();
        let mut changed = clip.clone();
        let TrackValues::Vec3s(values) = &mut changed.tracks[0].values else {
            panic!("translation fixture")
        };
        values[5].x = f32::from_bits(1);
        assert!(
            require_rebased_clips_match(std::slice::from_ref(&clip), &[changed])
                .unwrap_err()
                .contains("stored value 5 component 0 differs")
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
            AssemblyRestBindScaleRecipe {
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
    }

    #[test]
    fn explicit_mesh_selection_removes_prop_definitions_and_materials() {
        use animsmith_core::model::{MaterialAsset, MeshAsset, MeshInstance, SceneAssets};

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
