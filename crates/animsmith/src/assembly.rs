//! Versioned, recipe-driven multi-source character assembly.
//!
//! Assembly is deliberately a generic producer boundary: it combines already
//! extracted asset files, but it does not own archive extraction, consuming
//! project policy, acceptance contracts, or publication.

use crate::material_recipe::{MaterialTextureRecipeEvidence, apply_material_texture_recipe};
use animsmith_core::glam::{Quat, Vec3};
use animsmith_core::model::{
    Clip, Document, Interpolation, MaterialAsset, MeshAsset, MeshInstance, Property, Skeleton,
    Track, TrackValues,
};
use animsmith_core::{Config, ToolInfo, resolve_configured_roles};
use animsmith_gltf::write::WriteSummary;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const RECIPE_SCHEMA_VERSION: u32 = 1;
const RECIPE_SCHEMA_ID: &str = "urn:animsmith:schema:character-assembly-recipe:1";
const EVIDENCE_SCHEMA_VERSION: u32 = 1;
const EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:character-assembly-evidence:1";

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
    canonicalize_skin: bool,
    #[serde(default)]
    ground_and_center: bool,
    #[serde(default = "default_fps")]
    fps: f64,
    clips: Vec<AssemblyClipRecipe>,
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
    completed_tracks: usize,
    stripped_tracks: usize,
    duration_s: f64,
    frame_window: Option<[u32; 2]>,
    time_window: Option<[f64; 2]>,
    dropped_closing_endpoint: bool,
    hold_frames: u32,
    gait_anchor_frame_offset: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct AssemblyTransformEvidence {
    retained_mesh_instances: Vec<String>,
    removed_mesh_instances: usize,
    canonicalized_skin: bool,
    ground_and_center: bool,
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

#[derive(Debug, Clone, Serialize)]
struct AssemblyEvidence {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    recipe: AssemblyRecipeEvidence,
    inputs: Vec<AssemblyInputEvidence>,
    clips: Vec<AssemblyClipEvidence>,
    transforms: AssemblyTransformEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    material_texture_recipe: Option<MaterialTextureRecipeEvidence>,
    artifact: AssemblyArtifactEvidence,
}

/// Summary returned to the CLI's human-readable completion line.
pub(crate) struct AssemblyResult {
    pub(crate) animations: usize,
    pub(crate) meshes: usize,
    pub(crate) materials: usize,
}

struct InputResolver {
    root: PathBuf,
}

impl InputResolver {
    fn new(recipe_path: &Path, declared_root: Option<&Path>) -> Result<Self, String> {
        let recipe_parent = recipe_path.parent().unwrap_or_else(|| Path::new("."));
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
    if recipe.schema_version != RECIPE_SCHEMA_VERSION || recipe.schema != RECIPE_SCHEMA_ID {
        return Err(format!(
            "unsupported assembly recipe identity; expected schema_version {RECIPE_SCHEMA_VERSION} and schema {RECIPE_SCHEMA_ID:?}"
        ));
    }
    if !recipe.fps.is_finite() || recipe.fps <= 0.0 || recipe.fps > 1000.0 {
        return Err("fps must be finite and in 0..=1000".into());
    }
    if recipe.clips.is_empty() {
        return Err("clips must contain at least one entry".into());
    }
    unique_nonempty(&recipe.mesh_instances, "mesh_instances")?;
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
        if let Some([start, end]) = clip.frame_window {
            if start == 0 || end < start {
                return Err(format!(
                    "clip {:?} frame_window must be one-based and increasing",
                    clip.name
                ));
            }
        }
        if let Some([start, end]) = clip.time_window {
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
                return Err(format!(
                    "clip {:?} time_window must be finite, non-negative, and increasing",
                    clip.name
                ));
            }
        }
    }
    if recipe.ground_and_center && !recipe.canonicalize_skin {
        return Err("ground_and_center requires canonicalize_skin = true".into());
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

fn read_digest(path: &Path) -> Result<(String, u64), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let size = u64::try_from(bytes.len()).map_err(|_| "input size exceeds u64".to_owned())?;
    Ok((format!("{:x}", Sha256::digest(&bytes)), size))
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

/// Execute one complete assembly and atomically publish its artifact/evidence pair.
pub(crate) fn assemble(
    recipe_path: &Path,
    output: &Path,
    evidence_output: &Path,
    config: &Config,
    tool: ToolInfo,
) -> Result<AssemblyResult, String> {
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
    let recipe_bytes = fs::read(recipe_path)
        .map_err(|error| format!("cannot read recipe {}: {error}", recipe_path.display()))?;
    let recipe_text = std::str::from_utf8(&recipe_bytes)
        .map_err(|error| format!("recipe {} is not UTF-8: {error}", recipe_path.display()))?;
    let recipe: AssemblyRecipe =
        toml::from_str(recipe_text).map_err(|error| format!("invalid assembly recipe: {error}"))?;
    validate_recipe(&recipe)?;
    let resolver = InputResolver::new(recipe_path, recipe.input_root.as_deref())?;

    let base_path = resolver.resolve(&recipe.base_input)?;
    let mut inputs = vec![input_evidence("base", &recipe.base_input, &base_path)?];
    let mut base = load_input(&base_path)?;
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
            apply_material_texture_recipe(&resolved, &base).map_err(|error| error.to_string())
        })
        .transpose()?;
    if let Some(application) = &material_application {
        base = application.document.clone();
    }

    let mut loaded = BTreeMap::<PathBuf, Document>::new();
    let mut clip_evidence = Vec::with_capacity(recipe.clips.len());
    let mut output_clips = Vec::with_capacity(recipe.clips.len());
    for clip_recipe in &recipe.clips {
        let resolved = resolver.resolve(&clip_recipe.input)?;
        if !loaded.contains_key(&resolved) {
            inputs.push(input_evidence("clip", &clip_recipe.input, &resolved)?);
            loaded.insert(resolved.clone(), load_input(&resolved)?);
        }
        let source = &loaded[&resolved];
        ensure_unique_bones(
            &source.skeleton,
            &format!("clip input {}", clip_recipe.input.display()),
        )?;
        let source_clip = exact_take(source, &clip_recipe.take, &clip_recipe.input)?;
        let source_tracks = source_clip.tracks.len();
        let mut clip = source_clip.clone();
        clip.name.clone_from(&clip_recipe.name);
        apply_window(&mut clip, clip_recipe, recipe.fps)?;
        if clip_recipe.drop_closing_endpoint {
            drop_closing_endpoint(&mut clip, recipe.fps)?;
        }
        if clip_recipe.hold_frames > 0 {
            animsmith_core::transform::hold_extend(
                &mut clip,
                f64::from(clip_recipe.hold_frames) / recipe.fps,
            );
        }
        let remapped_tracks = remap_clip(&mut clip, &source.skeleton, &base.skeleton)?;
        let stripped_tracks = strip_tracks(&mut clip, &base.skeleton, &clip_recipe.strip_bones)?;
        let completed_tracks = if recipe.complete_tracks {
            complete_tracks(&mut clip, &base.skeleton, &clip_recipe.strip_bones)?
        } else {
            0
        };
        normalize_quaternions(&mut clip)?;
        let gait_anchor_frame_offset = if clip_recipe.gait_anchor {
            let roles = resolve_configured_roles(&base.skeleton, &config.rig);
            Some(
                animsmith_core::transform::align_gait_anchor(
                    &base.skeleton,
                    &mut clip,
                    &roles,
                    recipe.fps,
                )
                .map_err(|reason| format!("clip {:?}: {reason}", clip.name))?
                .frame_offset,
            )
        } else {
            None
        };
        clip_evidence.push(AssemblyClipEvidence {
            name: clip.name.clone(),
            declared_input: clip_recipe.input.display().to_string(),
            source_take: clip_recipe.take.clone(),
            source_tracks,
            emitted_tracks: clip.tracks.len(),
            remapped_tracks,
            completed_tracks,
            stripped_tracks,
            duration_s: clip.duration_s,
            frame_window: clip_recipe.frame_window,
            time_window: clip_recipe.time_window,
            dropped_closing_endpoint: clip_recipe.drop_closing_endpoint,
            hold_frames: clip_recipe.hold_frames,
            gait_anchor_frame_offset,
        });
        output_clips.push(clip);
    }
    base.clips = output_clips;

    if recipe.canonicalize_skin {
        // The verified core operation is intentionally the only place that
        // may rewrite bind-pose geometry. Its evidence is represented by the
        // recipe flags until the pre-1.0 core evidence type stabilizes.
        animsmith_core::canonicalize_skinned_geometry(&mut base, recipe.ground_and_center)
            .map_err(|error| error.to_string())?;
    }

    let output_parent = output.parent().unwrap_or_else(|| Path::new("."));
    let evidence_parent = evidence_output.parent().unwrap_or_else(|| Path::new("."));
    require_output_parent(output_parent, output)?;
    require_output_parent(evidence_parent, evidence_output)?;
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
    let (artifact_sha256, artifact_bytes) = read_digest(&artifact_temp)?;
    let evidence = AssemblyEvidence {
        schema_version: EVIDENCE_SCHEMA_VERSION,
        schema: EVIDENCE_SCHEMA_ID,
        tool,
        command: "assemble",
        recipe: AssemblyRecipeEvidence {
            path: recipe_path.display().to_string(),
            sha256: format!("{:x}", Sha256::digest(&recipe_bytes)),
            effective: recipe.clone(),
        },
        inputs,
        clips: clip_evidence,
        transforms: AssemblyTransformEvidence {
            retained_mesh_instances,
            removed_mesh_instances,
            canonicalized_skin: recipe.canonicalize_skin,
            ground_and_center: recipe.ground_and_center,
        },
        material_texture_recipe: material_application.map(|application| application.evidence),
        artifact: artifact_evidence(output, artifact_sha256, artifact_bytes, summary),
    };
    let mut evidence_bytes =
        serde_json::to_vec_pretty(&evidence).map_err(|error| error.to_string())?;
    evidence_bytes.push(b'\n');
    fs::write(&evidence_temp, evidence_bytes)
        .map_err(|error| format!("cannot write temporary evidence: {error}"))?;
    publish_pair(
        &artifact_temp,
        output,
        &evidence_temp,
        evidence_output,
        false,
    )?;
    Ok(AssemblyResult {
        animations: summary.animations,
        meshes: summary.meshes,
        materials: summary.materials,
    })
}

fn require_output_parent(parent: &Path, output: &Path) -> Result<(), String> {
    if !parent.is_dir() {
        return Err(format!(
            "output directory for {} does not exist",
            output.display()
        ));
    }
    if output.exists() && !output.is_file() {
        return Err(format!("output {} is not a regular file", output.display()));
    }
    Ok(())
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

fn drop_closing_endpoint(clip: &mut Clip, fps: f64) -> Result<(), String> {
    let end = clip.duration_s - 1.0 / fps;
    if end < 0.0 {
        return Err(format!(
            "clip {:?} is too short to drop a closing endpoint at {fps} fps",
            clip.name
        ));
    }
    animsmith_core::transform::slice(clip, 0.0, end, fps);
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

fn bone_indices(skeleton: &Skeleton) -> BTreeMap<&str, usize> {
    skeleton
        .bones
        .iter()
        .enumerate()
        .map(|(index, bone)| (bone.name.as_str(), index))
        .collect()
}

fn remap_clip(clip: &mut Clip, source: &Skeleton, base: &Skeleton) -> Result<usize, String> {
    let base_by_name = bone_indices(base);
    let mut targets = BTreeSet::new();
    for track in &mut clip.tracks {
        let source_bone = source.bones.get(track.bone).ok_or_else(|| {
            format!(
                "clip {:?} track targets source bone {} outside its skeleton",
                clip.name, track.bone
            )
        })?;
        track.bone = *base_by_name.get(source_bone.name.as_str()).ok_or_else(|| {
            format!(
                "clip {:?} targets bone {:?}, absent from the base skeleton",
                clip.name, source_bone.name
            )
        })?;
        if !targets.insert((track.bone, track.property.as_str())) {
            return Err(format!(
                "clip {:?} contains duplicate {} tracks for bone {:?}",
                clip.name,
                track.property.as_str(),
                base.bone_name(track.bone)
            ));
        }
    }
    Ok(clip.tracks.len())
}

fn strip_tracks(clip: &mut Clip, skeleton: &Skeleton, names: &[String]) -> Result<usize, String> {
    let indices = bone_indices(skeleton);
    let stripped = names
        .iter()
        .map(|name| {
            indices
                .get(name.as_str())
                .copied()
                .ok_or_else(|| format!("strip_bones target {name:?} is absent from base skeleton"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let before = clip.tracks.len();
    clip.tracks.retain(|track| !stripped.contains(&track.bone));
    Ok(before - clip.tracks.len())
}

fn complete_tracks(
    clip: &mut Clip,
    skeleton: &Skeleton,
    excluded_names: &[String],
) -> Result<usize, String> {
    let excluded = excluded_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut present = clip
        .tracks
        .iter()
        .map(|track| (track.bone, track.property))
        .collect::<Vec<_>>();
    let mut added = 0;
    for (bone, definition) in skeleton.bones.iter().enumerate() {
        if excluded.contains(definition.name.as_str()) {
            continue;
        }
        for (property, values) in [
            (
                Property::Translation,
                TrackValues::Vec3s(vec![definition.rest.translation]),
            ),
            (
                Property::Rotation,
                TrackValues::Quats(vec![definition.rest.rotation]),
            ),
            (
                Property::Scale,
                TrackValues::Vec3s(vec![definition.rest.scale]),
            ),
        ] {
            if present.contains(&(bone, property)) {
                continue;
            }
            clip.tracks.push(Track {
                bone,
                property,
                interpolation: Interpolation::Linear,
                times: vec![0.0],
                values,
            });
            present.push((bone, property));
            added += 1;
        }
    }
    clip.tracks
        .sort_by_key(|track| (track.bone, property_order(track.property)));
    if clip
        .tracks
        .iter()
        .any(|track| track.bone >= skeleton.bones.len())
    {
        return Err("completed clip contains a track outside the base skeleton".into());
    }
    Ok(added)
}

fn property_order(property: Property) -> u8 {
    match property {
        Property::Translation => 0,
        Property::Rotation => 1,
        Property::Scale => 2,
    }
}

fn normalize_quaternions(clip: &mut Clip) -> Result<(), String> {
    for track in &mut clip.tracks {
        let TrackValues::Quats(values) = &mut track.values else {
            continue;
        };
        let mut previous = None::<Quat>;
        for key in 0..track.key_count() {
            let index = track.value_index(key);
            let mut value = values[index];
            if !value.is_finite() || value.length_squared() <= f32::EPSILON {
                return Err(format!(
                    "clip {:?} has a non-finite or zero quaternion at bone {} key {key}",
                    clip.name, track.bone
                ));
            }
            value = value.normalize();
            let flip = previous.is_some_and(|prior| prior.dot(value) < 0.0);
            if flip {
                value = -value;
                if track.interpolation == Interpolation::CubicSpline {
                    values[index - 1] = -values[index - 1];
                    values[index + 1] = -values[index + 1];
                }
            }
            values[index] = value;
            previous = Some(value);
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

fn backup_destination(path: &Path) -> Result<Option<tempfile::TempPath>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let backup = tempfile::Builder::new()
        .prefix(".animsmith-assemble-backup-")
        .tempfile_in(parent)
        .map_err(|error| format!("cannot reserve backup for {}: {error}", path.display()))?
        .into_temp_path();
    fs::remove_file(&backup)
        .map_err(|error| format!("cannot prepare backup for {}: {error}", path.display()))?;
    fs::rename(path, &backup)
        .map_err(|error| format!("cannot back up {}: {error}", path.display()))?;
    Ok(Some(backup))
}

fn restore_destination(path: &Path, backup: Option<&tempfile::TempPath>) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot remove partial {}: {error}", path.display()))?;
    }
    if let Some(backup) = backup {
        fs::rename(backup, path)
            .map_err(|error| format!("cannot restore {}: {error}", path.display()))?;
    }
    Ok(())
}

fn publish_pair(
    artifact_temp: &Path,
    artifact: &Path,
    evidence_temp: &Path,
    evidence: &Path,
    fail_after_first_for_test: bool,
) -> Result<(), String> {
    let artifact_backup = backup_destination(artifact)?;
    let evidence_backup = match backup_destination(evidence) {
        Ok(backup) => backup,
        Err(error) => {
            restore_destination(artifact, artifact_backup.as_ref())?;
            return Err(error);
        }
    };
    let promote = || -> Result<(), String> {
        fs::rename(artifact_temp, artifact)
            .map_err(|error| format!("cannot publish {}: {error}", artifact.display()))?;
        if fail_after_first_for_test {
            return Err("injected evidence publication failure".into());
        }
        fs::rename(evidence_temp, evidence)
            .map_err(|error| format!("cannot publish {}: {error}", evidence.display()))?;
        Ok(())
    };
    if let Err(error) = promote() {
        let artifact_restore = restore_destination(artifact, artifact_backup.as_ref());
        let evidence_restore = restore_destination(evidence, evidence_backup.as_ref());
        let rollback_errors = [artifact_restore.err(), evidence_restore.err()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        return if rollback_errors.is_empty() {
            Err(error)
        } else {
            Err(format!(
                "{error}; rollback also failed: {}",
                rollback_errors.join("; ")
            ))
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::glam::Quat;
    use animsmith_core::model::{Bone, Transform};

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

        assert_eq!(remap_clip(&mut clip, &source, &base).unwrap(), 1);
        assert_eq!(clip.tracks[0].bone, 1);
        assert_eq!(strip_tracks(&mut clip, &base, &["root".into()]).unwrap(), 0);
        assert_eq!(
            complete_tracks(&mut clip, &base, &["root".into()]).unwrap(),
            2
        );
        normalize_quaternions(&mut clip).unwrap();
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
    fn failed_second_publish_restores_both_previous_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("out.glb");
        let evidence = dir.path().join("out.json");
        let artifact_temp = dir.path().join("new.glb");
        let evidence_temp = dir.path().join("new.json");
        fs::write(&artifact, b"old artifact").unwrap();
        fs::write(&evidence, b"old evidence").unwrap();
        fs::write(&artifact_temp, b"new artifact").unwrap();
        fs::write(&evidence_temp, b"new evidence").unwrap();

        let error =
            publish_pair(&artifact_temp, &artifact, &evidence_temp, &evidence, true).unwrap_err();
        assert!(error.contains("injected evidence publication failure"));
        assert_eq!(fs::read(&artifact).unwrap(), b"old artifact");
        assert_eq!(fs::read(&evidence).unwrap(), b"old evidence");
    }

    #[test]
    fn recipe_rejects_duplicate_outputs_and_conflicting_windows() {
        let recipe = AssemblyRecipe {
            schema_version: 1,
            schema: RECIPE_SCHEMA_ID.into(),
            input_root: None,
            base_input: "base.glb".into(),
            mesh_instances: vec![],
            material_texture_recipe: None,
            complete_tracks: false,
            canonicalize_skin: false,
            ground_and_center: false,
            fps: 30.0,
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
