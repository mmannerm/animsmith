//! The animsmith CLI binary.
//!
//! This crate publishes the `animsmith` command: inspect, measure, lint,
//! report, transform, fix, convert, assemble, scale, generate, and diff
//! skeletal animation clips. It
//! is not the Rust library API; use `animsmith-core` plus the loader
//! crates (`animsmith-gltf`, `animsmith-fbx`), `animsmith-engine`, and
//! `animsmith-report` from library code.
//!
//! Feature gates mirror the installed binary surface. The default build
//! includes FBX input and HTML reports; `--no-default-features` leaves a
//! pure-Rust glTF-only binary with report generation, FBX conversion, and
//! multi-source assembly omitted. `scale` is present in both: it is the
//! minimal binary's evidence-emitting producer.
//!
//! The GitHub [pipeline scenario guide] maps these commands to marketplace
//! intake, mocap cleanup, outsourced acceptance, CI, and artifact-storage
//! workflows.
//!
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md

#![warn(missing_docs)]

use animsmith_core::{
    Check, CheckCtx, CheckSelection, Config, DiffEnvelope, LintEnvelope, LintFileReport,
    MeasureEnvelope, MeasureFileReport, MeasurementContract, MeasurementFileError,
    MeasurementReportError, MeasurementReportInput, MeasurementReportReadError, MetricGrids,
    RigInfo, Severity, ToolInfo, ToolSource, all_checks, evaluate_checks, resolve_configured_roles,
};
use animsmith_core::{Document, InputIdentity};
use animsmith_engine::{
    BakeOrExtract, ENGINE_CHECK_IDS_V1, EngineAddressabilityCheck, EngineDeclaration,
    EngineImportAdviceStateV1, EngineImportAdviceV1, GltfAnimationAddressabilityInventoryV1,
    GltfAnimationAddressabilityV1, ProfileSelection, ResolvedProfile, SettingMap, SettingValue,
    StaticResolution, build_bevy_animation_addressability_adapter_v1,
};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
#[cfg(feature = "fbx")]
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(feature = "fbx")]
mod assembly;
mod collection_directional_speed;
mod collection_directional_speed_policy;
mod collection_lint;
mod collection_manifest;
mod collection_output;
mod contact_producer;
#[cfg(feature = "fbx")]
mod material_recipe;
mod producer;
mod publish;
mod render;
mod scale;
#[cfg(feature = "fbx")]
mod staged_selector;
#[cfg(feature = "fbx")]
mod texture_processing;

/// Exit codes, matching common asset-validation gate conventions:
/// 0 = no failing findings (warnings/notes allowed), 1 = error
/// findings, 2 = operator error.
const EXIT_FINDINGS: u8 = 1;
const EXIT_OPERATOR: u8 = 2;

#[derive(Parser)]
#[command(
    name = "animsmith",
    version = env!("ANIMSMITH_VERSION"),
    about = "Inspect, validate, and repair skeletal animation clips"
)]
struct Cli {
    /// Config for document-local commands (collection commands reject this option).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Summarize a file: skeleton, clips, mesh instances, materials, and rig profile.
    Inspect {
        /// Input .glb, .gltf, or .fbx file.
        file: PathBuf,
    },
    /// Emit per-clip measurements without judging them.
    Measure {
        /// Input .glb, .gltf, or .fbx files.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Run the check catalog and report findings.
    Lint {
        /// Input .glb, .gltf, or .fbx files.
        #[arg(required = true, value_name = "FILE")]
        files: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = PresentationFormat::Text)]
        format: PresentationFormat,
        /// Treat warnings as errors for the exit code.
        #[arg(long)]
        deny_warnings: bool,
        /// Run only these checks (comma-separated ids).
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
        /// Suppress findings from these checks (comma-separated ids).
        #[arg(long, value_delimiter = ',')]
        allow: Vec<String>,
    },
    /// Evaluate one strict multi-file collection manifest.
    Collection {
        #[command(subcommand)]
        operation: CollectionCmd,
    },
    /// Render a self-contained offline HTML report.
    #[command(
        long_about = "Render a self-contained offline HTML report: WebGL skeleton playback of the exact frames the checks judged, metric charts, and the findings list."
    )]
    #[cfg(feature = "report")]
    Report {
        /// Input .glb, .gltf, or .fbx file.
        file: PathBuf,
        /// Output HTML report path.
        #[arg(short, long)]
        output: PathBuf,
        /// Restrict the report to one clip.
        #[arg(long)]
        clip: Option<String>,
    },
    /// Apply mechanical clip transforms.
    #[command(
        long_about = "Apply pipeline-mechanical clip transforms and write the result as glTF, carrying through any scene assets the input brought (FBX or glTF meshes, skins, materials, and embedded base-color and normal textures). Operations apply to every clip, or one clip via --clip. Dropping a duplicate loop endpoint requires the clip to be declared loop = true in config and produces an open-cycle representation. Pruning constant tracks is opt-in and keeps tracks for bones declared animates_bones."
    )]
    Transform {
        /// Input .glb, .gltf, or .fbx file.
        input: PathBuf,
        /// Output .glb or .gltf path.
        #[arg(short, long)]
        output: PathBuf,
        /// Restrict to one clip.
        #[arg(long)]
        clip: Option<String>,
        /// Keep only `START:END` seconds, retimed to start at 0
        /// (half-frame epsilon at --fps).
        #[arg(long, value_name = "START:END")]
        slice: Option<String>,
        /// Extend the final pose by this many seconds (charge/block
        /// holds).
        #[arg(long, value_name = "SECONDS")]
        hold_extend: Option<f64>,
        /// Drop repeated closing keys from a declared loop when every
        /// authored track proves the endpoint is mechanically redundant.
        #[arg(long, conflicts_with = "hold_extend")]
        drop_duplicate_loop_endpoint: bool,
        /// Declare an in-place cyclic gait and rotate its measured stride
        /// anchor to t=0. Refuses accumulating root translation or yaw.
        #[arg(long)]
        gait_anchor: bool,
        /// Remove provably constant multi-key tracks after all other transforms.
        /// Tracks for bones declared `animates_bones` remain as motion evidence.
        #[arg(long)]
        prune_constant_tracks: bool,
        /// Frame rate used for epsilon and shift quantization.
        #[arg(long, default_value_t = 30.0)]
        fps: f64,
    },
    /// Repair safe mechanical glTF/GLB defects.
    #[command(
        long_about = "Repair mechanical clip defects in place, byte-surgically: only the offending animation bytes change; meshes, skins, materials, and textures pass through untouched. Currently fixes non-unit quaternions (the `quat-norm` check) and quaternion hemisphere flips (the `quat-flip` check) on glTF/GLB inputs."
    )]
    Fix {
        /// Input .glb or .gltf file.
        #[arg(value_name = "FILE")]
        input: PathBuf,
        /// Output path. Required unless --in-place or --dry-run is used.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Modify the input file in place.
        #[arg(long, conflicts_with = "output")]
        in_place: bool,
        /// Run exactly these repairs (comma-separated ids). Defaults to
        /// every available repair.
        #[arg(long = "repair", value_parser = repair_value_parser(), value_delimiter = ',')]
        repairs: Vec<Repair>,
        /// Report what would be repaired without writing anything.
        /// Exits 1 when repairs are pending, 0 when the file is clean.
        #[arg(long, conflicts_with_all = ["output", "in_place"])]
        dry_run: bool,
    },
    /// Convert FBX or glTF input to glTF.
    #[command(
        long_about = "Convert FBX or glTF input to glTF: skeleton, animation, triangulated meshes, skins, PBR materials, and embedded PNG/JPEG base-color, normal, metallic-roughness, and occlusion textures. A glTF input is re-emitted carrying its geometry; --animation-only drops it. --material-texture-recipe applies exact, declarative BaseColor, normal, metallic-roughness, and occlusion textures. --bake-static-mesh-transforms produces a strict canonical static scene whose mesh-local geometry includes accumulated rest transforms. Output format by extension: .glb binary, .gltf JSON with an embedded buffer. Asset-property refusals exit 1; under --format json they emit producer-refusal v1 on stdout. Invocation, recipe, path, and I/O errors exit 2 with stderr only."
    )]
    #[cfg(feature = "fbx")]
    Convert {
        /// Input .fbx, .glb, or .gltf file.
        input: PathBuf,
        /// Output .glb or .gltf path.
        #[arg(short, long)]
        output: PathBuf,
        /// Strip geometry: emit skeleton + animation only.
        #[arg(long, conflicts_with = "bake_static_mesh_transforms")]
        animation_only: bool,
        /// Apply an exact declarative PBR material-texture recipe.
        #[arg(long, value_name = "PATH", conflicts_with = "animation_only")]
        material_texture_recipe: Option<PathBuf>,
        /// Bake accumulated static node transforms into mesh-local geometry.
        #[arg(long)]
        bake_static_mesh_transforms: bool,
        /// Render a human write summary or versioned conversion evidence.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Assemble a multi-source skinned character from a versioned recipe.
    #[command(
        long_about = "Assemble one runtime GLB from an authoritative skinned base and animation takes supplied by FBX or glTF inputs. The versioned recipe owns exact mesh selection, skeleton remapping, clip windows and mechanical transforms. Recipe v7 can opt into rest/bind scale canonicalization for glTF/GLB and the narrow inventory-complete normalized/baked FBX subset by exact root name and expected factor. The base must resolve that name to exactly one governed non-empty skin. Every distinct FBX clip-only input receives the base-plan-owned animation-target rebase through an explicit projection that retains its normalized skeleton and takes but excludes its geometry, deformation, materials, and bind state. A glTF/GLB clip retains its existing successful full rest/bind or meshless track-only path; role-specific preflight additionally admits a track-only projection when only unused geometry, material, deformation, or bind obligations fail, while keeping framing, dependency, raw-coverage, named-skeleton, and animation accessor/layout checks strict. It composes canonicalization, grounding, and node removal, then proves one final raw GLB rewrite. Asset-property refusals exit 1; under --format json they emit producer-refusal v1 on stdout without publishing either output. Recipe, path, I/O, and publication errors exit 2 with stderr only. Source extraction, project policy, and publication remain consumer responsibilities."
    )]
    #[cfg(feature = "fbx")]
    Assemble {
        /// Versioned assembly recipe (.toml).
        recipe: PathBuf,
        /// Output .glb path.
        #[arg(short, long)]
        output: PathBuf,
        /// Versioned JSON evidence output path.
        #[arg(long)]
        evidence: PathBuf,
        /// Render a human publication summary or the versioned assembly
        /// evidence — the same bytes `--evidence` receives.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Rewrite declared linear scale and publish versioned evidence.
    #[command(
        long_about = "Rewrite one self-contained glTF/GLB asset's declared linear scale on its own raw bytes, or use the narrow rest-bind FBX path to stage a private normalized GLB, then publish the artifact and its versioned evidence as one atomic pair. `whole-document` converts every represented length by a declared factor; `rest-bind` removes one compensating inherited scale from a selected skinned hierarchy. Every factor and source selector is required: nothing is inferred from bounds, height, joint lengths, inverse-bind magnitude, filename, or asset category, there is no in-place mode, and the tolerance policy is fixed and recorded rather than exposed as a flag. Input, output, and evidence paths must be distinct. A refusal publishes nothing, leaves any prior pair byte-identical, and exits 1; an operator error exits 2."
    )]
    Scale {
        #[command(subcommand)]
        operation: ScaleCmd,
    },
    /// Generate bounded, versioned pipeline contracts from one source asset.
    Generate {
        #[command(subcommand)]
        operation: GenerateCmd,
    },
    /// Compare animation measurements.
    #[command(
        long_about = "Compare the measurements of two inputs (asset files or one-file output-v11 `measure` or `lint` JSON carrying measurements-v15) and report movement beyond significance thresholds. Exits 1 on significant movement."
    )]
    Diff {
        /// Before input: asset file or one-file output-v11 `measure`/`lint` JSON report.
        a: PathBuf,
        /// After input: asset file or one-file output-v11 `measure`/`lint` JSON report.
        b: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// Collection-scoped validation commands.
#[derive(Subcommand)]
enum CollectionCmd {
    /// Lint every declared source and retain logical clip/set completeness.
    Lint {
        /// Strict collection-manifest V1 TOML input.
        #[arg(value_name = "COLLECTION.toml")]
        manifest: PathBuf,
        /// Emit the collection-output V2 JSON contract.
        #[arg(long, value_enum, default_value_t = CollectionFormat::Json)]
        format: CollectionFormat,
    },
    /// Generate one strict contact fragment from an exactly declared collection clip.
    GenerateContactFragment {
        /// Strict collection-manifest V1 TOML input.
        #[arg(value_name = "MANIFEST.toml")]
        manifest: PathBuf,
        /// Exact logical clip id declared by the manifest.
        #[arg(long, value_name = "LOGICAL_ID")]
        clip: String,
        /// Destination contact-fragment JSON path.
        #[arg(short, long)]
        output: PathBuf,
        /// Render canonical JSON or a stable text summary/refusal.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Evaluate one declared directional-speed policy against collection-output V2 evidence.
    #[command(
        long_about = "Evaluate one strict manifest-bound directional-speed policy against one strict collection-output V2 document. The policy must declare every member of one directional-blend runtime set in manifest order; this command never infers members, filenames, controller behavior, or diagonal normalization. It writes only the immutable collection-directional-speed-evaluation V1 JSON result to stdout. Invalid, stale, wrong-kind, unreadable, malformed, or over-budget control inputs write no result and exit 2. Incomplete or not-evaluable evidence and declared-policy findings write a result and exit 1; only a complete passing policy exits 0."
    )]
    EvaluateDirectionalSpeed {
        /// Strict collection-directional-speed-policy V1 TOML input.
        #[arg(long, value_name = "POLICY.toml")]
        policy: PathBuf,
        /// Strict collection-output V2 JSON input.
        #[arg(long, value_name = "COLLECTION-OUTPUT.json")]
        evidence: PathBuf,
        /// Emit the immutable directional-speed evaluation JSON contract.
        #[arg(long, value_enum, default_value_t = CollectionFormat::Json)]
        format: CollectionFormat,
    },
}

/// Machine-only formats for collection-output and directional-speed evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum CollectionFormat {
    Json,
}

/// Versioned single-document pipeline contracts.
#[derive(Subcommand)]
enum GenerateCmd {
    /// Generate one strict contact-fragment V1 sidecar from sampled stance support.
    #[command(
        long_about = "Generate a source-bound contact-fragment V1 for one exactly named clip. The fragment reports sampled model-space stance support only; it does not infer physical contact, gameplay, footfalls, IK, or engine behavior. Both sides require complete finite evidence, and a refusal never changes the output path."
    )]
    ContactFragment {
        /// Input .glb, .gltf, or .fbx file.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Exact unique embedded clip name.
        #[arg(long, value_name = "TAKE_NAME")]
        clip: String,
        /// Destination contact-fragment JSON path.
        #[arg(short, long)]
        output: PathBuf,
        /// Render canonical JSON or a stable text summary/refusal.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Inventory glTF animation addressability with an optional exact Bevy adapter.
    #[command(
        long_about = "Generate one bounded glTF animation-addressability document from the immutable source facts and dependency closure. With the exact supported Bevy profile selected, the same document embeds the existing engine-addressability evaluation; without it, the neutral inventory remains available and the Bevy adapter is null. This command does not claim runtime loading, graph wiring, target survival, or named-map behavior."
    )]
    Addressability {
        /// Input .glb or .gltf file.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Render canonical JSON or a presentation-only text/Markdown view.
        #[arg(long, value_enum, default_value_t = PresentationFormat::Json)]
        format: PresentationFormat,
    },
    /// Project bounded, versioned importer suggestions from one exact engine profile.
    #[command(
        long_about = "Generate bounded engine-import advice from one same-load source, its exact resolved engine profile/settings, explicit clip intent, and normalized measurements. Unity 6000.3 Generic/Humanoid emits documented importer properties. Frozen Unreal 5.8 and Godot 4.7 V1 profiles emit a typed refusal because their setting vocabulary is not yet modeled. No frame coordinates, sample rates, root-motion behavior, or unit conversion are guessed."
    )]
    ImportAdvice {
        /// Input .fbx for Unity/Unreal, or .glb/.gltf for Godot.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Render canonical JSON or a presentation-only text/Markdown view.
        #[arg(long, value_enum, default_value_t = PresentationFormat::Json)]
        format: PresentationFormat,
    },
}

/// The two accepted scale operations of DESIGN.md Appendix D §D.7, as
/// distinct subcommands rather than one flag whose meaning depends on the
/// input. They rewrite different domains, so a factor alone does not
/// identify the operation.
#[derive(Subcommand)]
enum ScaleCmd {
    /// Convert every represented length by a declared factor.
    #[command(
        long_about = "Convert every represented length in a self-contained glTF/GLB document by the declared finite positive factor: node translations, animated translation tracks, inverse-bind translations, base mesh positions, and raw glTF POSITION morph-target deltas. Physical size changes; this is appropriate only when the source was authored in a different linear unit. The factor is never inferred."
    )]
    WholeDocument {
        /// Input .glb or .gltf file.
        input: PathBuf,
        /// Output path; must keep the input's container extension.
        #[arg(short, long)]
        output: PathBuf,
        /// Declared linear-unit conversion factor.
        #[arg(long)]
        factor: f64,
        /// Versioned JSON evidence output path.
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Remove one compensating inherited scale from a skinned hierarchy.
    #[command(
        long_about = "Reparameterize a restricted skinned rest/bind hierarchy so a compensating inherited scale is removed while world joint translations and orientations, sampled trajectories, and skinned vertex positions are preserved. Both source selectors are required raw source-array indices, and the expected common factor is declared and checked against the source rather than inferred. glTF/GLB preserves and rewrites its own raw representation; the narrow FBX path accepts only a complete normalized ufbx inventory and emits a newly serialized .glb with v5 evidence."
    )]
    RestBind {
        /// Input .glb/.gltf file, or (with the FBX feature) a complete-inventory .fbx file.
        input: PathBuf,
        /// Output path; glTF/GLB keeps its container, while FBX emits .glb.
        #[arg(short, long)]
        output: PathBuf,
        /// Source-skin array index whose joints anchor the affected domain.
        #[arg(long)]
        source_skin_index: usize,
        /// Source-node array index of the scaled ancestor root.
        #[arg(long)]
        source_root_node_index: usize,
        /// Declared expected common factor, checked against the source.
        #[arg(long)]
        expected_factor: f64,
        /// Versioned JSON evidence output path.
        #[arg(long)]
        evidence: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

/// JSON machine output plus the two presentation-only renderings shared by
/// `lint` and `generate`. Each command pins its own default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PresentationFormat {
    Json,
    Text,
    Markdown,
}

#[cfg(feature = "fbx")]
const CONVERSION_EVIDENCE_SCHEMA_VERSION: u32 = 2;
#[cfg(feature = "fbx")]
const CONVERSION_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:conversion-evidence:2";

#[cfg(feature = "fbx")]
#[derive(Serialize)]
struct ConversionOptions {
    animation_only: bool,
    bake_static_mesh_transforms: bool,
    material_texture_recipe: Option<String>,
}

#[cfg(feature = "fbx")]
#[derive(Serialize)]
struct ConversionArtifact {
    nodes: usize,
    animations: usize,
    meshes: usize,
    primitive_positions: usize,
    materials: usize,
    clips_without_writable_tracks: usize,
}

#[cfg(feature = "fbx")]
impl From<animsmith_gltf::write::WriteSummary> for ConversionArtifact {
    fn from(summary: animsmith_gltf::write::WriteSummary) -> Self {
        Self {
            nodes: summary.nodes,
            animations: summary.animations,
            meshes: summary.meshes,
            primitive_positions: summary.primitive_positions,
            materials: summary.materials,
            clips_without_writable_tracks: summary.clips_without_writable_tracks,
        }
    }
}

#[cfg(feature = "fbx")]
#[derive(Serialize)]
struct ConversionEnvelope<'a> {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    input: String,
    output: String,
    options: ConversionOptions,
    artifact: ConversionArtifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    static_mesh_bake: Option<&'a animsmith_core::StaticMeshBakeEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material_texture_recipe: Option<&'a material_recipe::MaterialTextureRecipeEvidence>,
}

#[cfg(feature = "fbx")]
struct ConversionRequest<'a> {
    input: &'a Path,
    output: &'a Path,
    animation_only: bool,
    material_texture_recipe: Option<&'a Path>,
    bake_static_mesh_transforms: bool,
    format: Format,
}

#[cfg(feature = "fbx")]
struct Converted {
    summary: animsmith_gltf::write::WriteSummary,
    static_mesh_bake: Option<animsmith_core::StaticMeshBake>,
    recipe_application: Option<material_recipe::MaterialTextureRecipeApplication>,
}

#[cfg(feature = "fbx")]
fn material_recipe_failure(
    error: material_recipe::MaterialTextureRecipeError,
) -> producer::Failure {
    use material_recipe::MaterialTextureRecipeError as Error;
    use producer::{Failure, Kind, Stage};

    match error {
        Error::Recipe(_)
        | Error::UnsafePath { .. }
        | Error::InvalidRoot { .. }
        | Error::TextureFile { .. }
        | Error::DuplicateRecipeMaterial { .. } => Failure::operator(error),
        Error::AmbiguousSourceMaterial { .. }
        | Error::UnusedRecipeMaterial { .. }
        | Error::MissingMaterialMapping { .. } => {
            Failure::refusal(Stage::Selection, Kind::AssetRecipeMismatch, error)
        }
        Error::TextureTooLarge { .. } | Error::Image { .. } => {
            Failure::refusal(Stage::Transform, Kind::TransformRefused, error)
        }
    }
}

#[cfg(feature = "fbx")]
fn conversion_write_failure(error: animsmith_gltf::WriteError) -> producer::Failure {
    use animsmith_gltf::WriteError;
    use producer::{Failure, Kind, Stage};

    match error {
        WriteError::Io { .. } => Failure::operator(error),
        WriteError::Serialize(_) | WriteError::TooLarge { .. } => {
            Failure::refusal(Stage::Encode, Kind::UnrepresentableArtifact, error)
        }
        // `WriteError` is non-exhaustive. A future document-driven writer
        // rejection must fail closed as an asset refusal until it receives an
        // explicit classification; it can never silently become exit 2 by
        // falling through a string boundary.
        _ => Failure::refusal(Stage::Encode, Kind::UnrepresentableArtifact, error),
    }
}

#[cfg(feature = "fbx")]
fn produce_conversion_inner(
    request: &ConversionRequest<'_>,
) -> Result<Converted, producer::Failure> {
    use producer::{Failure, Kind, Stage};

    // Extension and primary-file I/O are invocation/filesystem facts. Keep
    // the typed loader error after capturing the primary bytes: parse and
    // structure defects are asset facts, while external-resource I/O remains
    // an operator failure.
    let (input_format, input_bytes) = capture_input(request.input).map_err(Failure::operator)?;
    let mut doc = load_bytes_typed(request.input, input_format, &input_bytes)
        .map_err(producer_load_failure)?;
    if request.animation_only {
        doc.assets = animsmith_core::model::SceneAssets::default();
    }
    let recipe_application = request
        .material_texture_recipe
        .map(|path| material_recipe::apply_material_texture_recipe(path, &doc))
        .transpose()
        .map_err(material_recipe_failure)?;
    let recipe_doc = recipe_application
        .as_ref()
        .map_or(&doc, |application| &application.document);
    let static_mesh_bake = if request.bake_static_mesh_transforms {
        Some(
            animsmith_core::bake_static_mesh_transforms(recipe_doc).map_err(|error| {
                Failure::refusal(Stage::Transform, Kind::TransformRefused, error)
            })?,
        )
    } else {
        None
    };
    let output_doc = static_mesh_bake
        .as_ref()
        .map_or(recipe_doc, |bake| &bake.document);
    let summary = animsmith_gltf::write::write(output_doc, request.output)
        .map_err(conversion_write_failure)?;
    Ok(Converted {
        summary,
        static_mesh_bake,
        recipe_application,
    })
}

#[cfg(feature = "fbx")]
fn produce_conversion(
    request: &ConversionRequest<'_>,
) -> Result<producer::Outcome<Converted>, String> {
    match produce_conversion_inner(request) {
        Ok(converted) => Ok(producer::Outcome::Published(converted)),
        Err(producer::Failure::Refusal(rejection)) => Ok(producer::Outcome::Rejected(rejection)),
        Err(producer::Failure::Operator(message)) => Err(message),
    }
}

#[cfg(feature = "fbx")]
fn run_conversion(request: &ConversionRequest<'_>, tool: ToolInfo) -> Result<ExitCode, String> {
    let converted = match produce_conversion(request) {
        Ok(producer::Outcome::Published(converted)) => converted,
        Ok(producer::Outcome::Rejected(rejection)) => {
            let mut delivery = producer::ProcessRefusalDelivery;
            return producer::emit_rejection(
                producer::Command::Convert,
                request.format,
                tool,
                rejection,
                &mut delivery,
            );
        }
        Err(message) => return Err(message),
    };
    match request.format {
        Format::Text => {
            let transcript = std::iter::once(render::render_write_summary(
                request.output,
                &converted.summary,
            ))
            .chain(converted.static_mesh_bake.as_ref().map(|bake| {
                format!(
                    "baked {} static mesh instance(s) into identity-root geometry\n",
                    bake.evidence.entries.len(),
                )
            }))
            .chain(converted.recipe_application.as_ref().map(|application| {
                format!(
                    "applied material texture recipe; emitted {} texture(s)\n",
                    application.evidence.emitted_textures.len(),
                )
            }));
            publish::emit_text_chunks(transcript);
        }
        Format::Json => render::print_json(&ConversionEnvelope {
            schema_version: CONVERSION_EVIDENCE_SCHEMA_VERSION,
            schema: CONVERSION_EVIDENCE_SCHEMA_ID,
            tool,
            command: "convert",
            input: request.input.display().to_string(),
            output: request.output.display().to_string(),
            options: ConversionOptions {
                animation_only: request.animation_only,
                bake_static_mesh_transforms: request.bake_static_mesh_transforms,
                material_texture_recipe: request
                    .material_texture_recipe
                    .map(|path| path.display().to_string()),
            },
            artifact: converted.summary.into(),
            static_mesh_bake: converted
                .static_mesh_bake
                .as_ref()
                .map(|bake| &bake.evidence),
            material_texture_recipe: converted
                .recipe_application
                .as_ref()
                .map(|application| &application.evidence),
        })?,
    }
    Ok(ExitCode::SUCCESS)
}

fn select_repairs(repairs: Vec<Repair>) -> Vec<Repair> {
    let repairs = if repairs.is_empty() {
        Repair::ALL.to_vec()
    } else {
        repairs
    };
    dedup_preserving_order(repairs)
}

fn repair_value_parser() -> impl TypedValueParser<Value = Repair> {
    let values = Repair::ALL
        .iter()
        .map(|repair| PossibleValue::new(repair.id()))
        .collect::<Vec<_>>();
    PossibleValuesParser::new(values)
        .map(|id| Repair::from_id(&id).expect("possible-values parser returned a known repair id"))
}

fn dedup_preserving_order<T: Copy + Eq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut selected = Vec::new();
    for item in items {
        if !selected.contains(&item) {
            selected.push(item);
        }
    }
    selected
}

fn current_tool() -> ToolInfo {
    ToolInfo::animsmith(ToolSource::new(
        option_env!("ANIMSMITH_GIT_REVISION").map(str::to_owned),
        option_env!("ANIMSMITH_GIT_DIRTY").and_then(|value| value.parse().ok()),
    ))
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if !error.use_stderr() => {
            // clap normally writes display-help/version itself and silently
            // swallows a broken pipe. Keep those successful parser outcomes,
            // but route their stdout through the same checked delivery rule as
            // every command result so a closed stream is diagnosed. Let clap
            // perform the write so its Auto/Always/Never color policy and
            // styled rendering remain byte-for-byte authoritative.
            publish::emit_clap_output(&error);
            return ExitCode::SUCCESS;
        }
        Err(error) => error.exit(),
    };
    finish_run(run(cli))
}

fn finish_run(result: Result<ExitCode, String>) -> ExitCode {
    finish_run_with(result, publish::emit_error_text)
}

fn finish_run_with(result: Result<ExitCode, String>, emit_error: impl FnOnce(&str)) -> ExitCode {
    match result {
        Ok(code) => code,
        Err(message) => {
            let rendered = render::render_operator_error(&message);
            emit_error(&rendered);
            ExitCode::from(EXIT_OPERATOR)
        }
    }
}

struct LoadedConfig {
    config: Config,
    engine: Option<StaticResolution>,
    path: Option<PathBuf>,
    /// Canonical control-file input used only for publication alias guards.
    /// Unlike `path`, this must never cross a collection's public boundary.
    control_input: Option<PathBuf>,
    #[cfg(feature = "fbx")]
    source: Option<LoadedConfigSource>,
}

#[cfg(feature = "fbx")]
struct LoadedConfigSource {
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineToml {
    profile: Option<String>,
    profile_revision: Option<u32>,
    engine_version: Option<String>,
    importer: Option<String>,
    settings: Option<BTreeMap<String, EngineSettingToml>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EngineSettingToml {
    Boolean(bool),
    Text(String),
}

fn engine_setting_value(key: &str, value: EngineSettingToml) -> SettingValue {
    match value {
        EngineSettingToml::Boolean(value) => SettingValue::Boolean(value),
        EngineSettingToml::Text(value) if key == "root_motion_source" => {
            SettingValue::SourceTransformPath(value)
        }
        EngineSettingToml::Text(value) if value == "bake" => {
            SettingValue::BakeOrExtract(BakeOrExtract::Bake)
        }
        EngineSettingToml::Text(value) if value == "extract" => {
            SettingValue::BakeOrExtract(BakeOrExtract::Extract)
        }
        EngineSettingToml::Text(value) => SettingValue::SourceTransformPath(value),
    }
}

fn engine_setting_map(values: BTreeMap<String, EngineSettingToml>) -> SettingMap {
    values
        .into_iter()
        .map(|(key, value)| {
            let value = engine_setting_value(&key, value);
            (key, value)
        })
        .collect()
}

fn engine_selection(config: &EngineToml) -> Result<Option<ProfileSelection>, String> {
    let fields_present = [
        config.profile.is_some(),
        config.profile_revision.is_some(),
        config.engine_version.is_some(),
        config.importer.is_some(),
    ];
    if fields_present.iter().all(|present| !present) {
        return Ok(None);
    }
    if !fields_present.iter().all(|present| *present) {
        return Err(
            "[engine] requires profile, profile_revision, engine_version, and importer".into(),
        );
    }
    Ok(Some(ProfileSelection::new(
        config.profile.clone().expect("presence checked"),
        config.profile_revision.expect("presence checked"),
        config.engine_version.clone().expect("presence checked"),
        config.importer.clone().expect("presence checked"),
    )))
}

fn parse_config(text: &str) -> Result<(Config, EngineDeclaration), String> {
    let mut root: toml::Table = toml::from_str(text).map_err(|error| error.to_string())?;
    let engine_value = root.remove("engine");
    let engine_declared = engine_value.is_some();
    let engine = engine_value
        .map(|value| {
            value
                .try_into::<EngineToml>()
                .map_err(|error| error.to_string())
        })
        .transpose()?;

    let mut clip_settings = BTreeMap::new();
    if let Some(toml::Value::Table(clips)) = root.get_mut("clips") {
        for (selector, clip) in clips {
            let toml::Value::Table(clip) = clip else {
                continue;
            };
            let Some(settings) = clip.remove("engine_settings") else {
                continue;
            };
            let settings = settings
                .try_into::<BTreeMap<String, EngineSettingToml>>()
                .map_err(|error| error.to_string())?;
            clip_settings.insert(selector.clone(), engine_setting_map(settings));
        }
    }

    let (selection, document_settings) = match engine {
        Some(engine) => {
            let selection = engine_selection(&engine)?;
            if selection.is_none() && engine.settings.is_none() {
                return Err(
                    "[engine] requires profile, profile_revision, engine_version, and importer"
                        .into(),
                );
            }
            (selection, engine.settings.map(engine_setting_map))
        }
        None => (None, None),
    };
    debug_assert_eq!(
        engine_declared,
        selection.is_some() || document_settings.is_some()
    );

    let config = toml::Value::Table(root)
        .try_into::<Config>()
        .map_err(|error| error.to_string())?;
    Ok((
        config,
        EngineDeclaration {
            selection,
            document_settings,
            clip_settings,
        },
    ))
}

fn load_config(explicit: Option<&Path>) -> Result<LoadedConfig, String> {
    load_config_with_source(explicit)
}

fn config_source_path(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit.map(Path::to_path_buf).or_else(|| {
        let default = PathBuf::from("animsmith.toml");
        default.exists().then_some(default)
    })
}

fn load_config_with_source(explicit: Option<&Path>) -> Result<LoadedConfig, String> {
    let Some(path) = config_source_path(explicit) else {
        return Ok(LoadedConfig {
            config: Config::default(),
            engine: None,
            path: None,
            control_input: None,
            #[cfg(feature = "fbx")]
            source: None,
        });
    };
    let bytes =
        std::fs::read(&path).map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| format!("bad config {}: config is not UTF-8: {e}", path.display()))?;
    let (config, declaration) =
        parse_config(text).map_err(|error| format!("bad config {}: {error}", path.display()))?;
    config
        .validate()
        .map_err(|e| format!("bad config {}: {e}", path.display()))?;
    let engine = animsmith_engine::resolve_static(declaration)
        .map_err(|error| format!("bad config {}: {error}", path.display()))?;
    Ok(LoadedConfig {
        config,
        engine,
        path: Some(path.clone()),
        control_input: Some(path.clone()),
        #[cfg(feature = "fbx")]
        source: Some(LoadedConfigSource { path, bytes }),
    })
}

impl LoadedConfig {
    /// The configuration file consumed for this invocation, when one was
    /// explicitly selected or found at the default location.
    ///
    /// Strict producers use this only to ensure a publication destination
    /// cannot replace a control input they have already consumed.
    pub(crate) fn control_input(&self) -> Option<&Path> {
        self.control_input.as_deref()
    }

    fn resolve_engine_input(
        &self,
        source_format: animsmith_core::SourceFormatV1,
        document: &Document,
    ) -> Result<Option<ResolvedProfile>, String> {
        let Some(engine) = &self.engine else {
            return Ok(None);
        };
        engine
            .resolve_input_iter(
                source_format,
                document.clips.iter().map(|clip| clip.name.as_str()),
            )
            .map(Some)
            .map_err(|error| match &self.path {
                Some(path) => format!("bad config {}: {error}", path.display()),
                None => format!("bad config: {error}"),
            })
    }
}

fn full_check_ids() -> Result<Vec<&'static str>, String> {
    let mut known = all_checks()
        .into_iter()
        .map(|check| check.id())
        .collect::<Vec<_>>();
    known.extend_from_slice(ENGINE_CHECK_IDS_V1);

    let mut unique = BTreeSet::new();
    for id in &known {
        if id.is_empty() {
            return Err("check catalog contains an empty id".into());
        }
        if !unique.insert(*id) {
            return Err(format!("check catalog contains duplicate id '{id}'"));
        }
    }
    Ok(known)
}

fn validate_check_selection(known: &[&str], select: &[String]) -> Result<(), String> {
    // Frontend validation intentionally runs before loading any input file, so
    // a bad CLI selection has one deterministic operator error. Core repeats
    // the invariant for embedded callers that invoke `evaluate_checks`
    // directly; the two boundaries serve different consumers.
    for id in select {
        if !known.contains(&id.as_str()) {
            return Err(format!(
                "--select: unknown check '{id}' (known: {})",
                known.join(", ")
            ));
        }
    }
    Ok(())
}

struct LintAnalysis {
    report: LintFileReport,
    requires_failure: bool,
    indexed_measurements: Vec<animsmith_core::measure::ClipMeasurements>,
}

fn analyze_loaded_lint(
    loaded: &LoadedInput,
    config: &LoadedConfig,
    path_label: impl Into<String>,
    selection: CheckSelection<'_>,
    fail_at: Severity,
    allowed: &BTreeSet<String>,
) -> Result<LintAnalysis, String> {
    let input = loaded.input().clone();
    let prediction_provenance = loaded
        .engine
        .as_ref()
        .map(|profile| animsmith_engine::project_prediction_provenance_v1(profile, &loaded.source))
        .transpose()
        .map_err(|error| error.to_string())?;
    let doc = loaded.document();
    let roles = resolve_configured_roles(&doc.skeleton, &config.config.rig);
    let grids = MetricGrids::new(doc);
    let ctx = CheckCtx::new(&grids, &roles, &config.config);
    let evaluations = {
        let mut checks: Vec<Box<dyn Check + '_>> = all_checks();
        checks.push(Box::new(
            EngineAddressabilityCheck::new(&loaded.source, prediction_provenance.as_ref())
                .map_err(|error| error.to_string())?,
        ));
        evaluate_checks(&ctx, &checks, selection).map_err(|error| error.to_string())?
    };
    let requires_failure =
        animsmith_core::evaluation::lint_requires_failure(&evaluations, fail_at, allowed);
    let indexed_measurements =
        animsmith_core::measure::measure_document_indexed(&grids, &roles, &config.config);
    let measurements = doc
        .clips
        .iter()
        .map(|clip| clip.name.clone())
        .zip(indexed_measurements.iter().cloned())
        .collect();
    let report = LintFileReport::new(
        path_label,
        input,
        RigInfo::from_resolved(doc, &roles).map_err(|error| error.to_string())?,
        prediction_provenance,
        evaluations,
        MeasurementContract::new(measurements, animsmith_core::measure::measure_assets(doc))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(LintAnalysis {
        report,
        requires_failure,
        indexed_measurements,
    })
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.cmd {
        Cmd::Inspect { file } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            let loaded = load_with_config(&file, &loaded_config)?;
            let config = &loaded_config.config;
            let doc = loaded.document();
            let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
            publish::emit_text_lines(render::render_inspect(doc, &roles));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            let config = &loaded_config.config;
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let loaded = load_with_config(file, &loaded_config)?;
                let input = loaded.input().clone();
                let doc = loaded.document();
                let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                let grids = MetricGrids::new(doc);
                reports.push(MeasureFileReport::new(
                    file.display().to_string(),
                    input,
                    RigInfo::from_resolved(doc, &roles).map_err(|error| error.to_string())?,
                    MeasurementContract::new(
                        animsmith_core::measure::measure_document(&grids, &roles, config),
                        animsmith_core::measure::measure_assets(doc),
                    )
                    .map_err(|error| error.to_string())?,
                ));
            }
            match format {
                Format::Json => {
                    let envelope = MeasureEnvelope::new(current_tool(), reports)
                        .map_err(|error| error.to_string())?;
                    render::print_json(&envelope)?;
                }
                Format::Text => {
                    publish::emit_text_lines(render::render_measure_text(&reports));
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Lint {
            files,
            format,
            deny_warnings,
            select,
            allow,
        } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            require_files(&files)?;
            let known_check_ids = full_check_ids()?;
            validate_check_selection(&known_check_ids, &select)?;
            if format == PresentationFormat::Json && !allow.is_empty() {
                return Err(
                    "--allow is not supported with --format json; machine-readable results retain every content finding"
                        .into(),
                );
            }
            let selected: BTreeSet<String> = select.iter().cloned().collect();
            let selection = if selected.is_empty() {
                CheckSelection::All
            } else {
                CheckSelection::Only(&selected)
            };
            let fail_at = if deny_warnings {
                Severity::Warning
            } else {
                Severity::Error
            };
            let allowed: BTreeSet<String> = allow.iter().cloned().collect();
            let mut reports = Vec::new();
            let mut requires_failure = false;
            for file in &files {
                let loaded = load_with_config(file, &loaded_config)?;
                let analysis = analyze_loaded_lint(
                    &loaded,
                    &loaded_config,
                    file.display().to_string(),
                    selection,
                    fail_at,
                    &allowed,
                )?;
                requires_failure |= analysis.requires_failure;
                reports.push(analysis.report);
            }
            match format {
                PresentationFormat::Json => {
                    let envelope = LintEnvelope::new(current_tool(), reports)
                        .map_err(|error| error.to_string())?;
                    render::print_json(&envelope)?;
                }
                PresentationFormat::Text => {
                    publish::emit_text(&render::render_text(&reports, &allow));
                }
                PresentationFormat::Markdown => {
                    publish::emit_text(&render::render_markdown(&reports, &allow));
                }
            }
            Ok(if requires_failure {
                ExitCode::from(EXIT_FINDINGS)
            } else {
                ExitCode::SUCCESS
            })
        }
        Cmd::Collection { operation } => {
            if cli.config.is_some() {
                return Err(
                    "--config is not accepted by collection commands; collection lint declares each source config in the collection manifest"
                        .into(),
                );
            }
            match operation {
                CollectionCmd::Lint { manifest, format } => {
                    debug_assert_eq!(format, CollectionFormat::Json);
                    collection_lint::run_collection_lint(&manifest)
                }
                CollectionCmd::GenerateContactFragment {
                    manifest,
                    clip,
                    output,
                    format,
                } => contact_producer::run_collection(
                    &manifest,
                    &clip,
                    &output,
                    format,
                    current_tool(),
                ),
                CollectionCmd::EvaluateDirectionalSpeed {
                    policy,
                    evidence,
                    format,
                } => {
                    debug_assert_eq!(format, CollectionFormat::Json);
                    collection_directional_speed::run(&policy, &evidence)
                }
            }
        }
        #[cfg(feature = "report")]
        Cmd::Report { file, output, clip } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            full_check_ids()?;
            let loaded = load_with_config(&file, &loaded_config)?;
            let config = &loaded_config.config;
            let prediction_provenance = loaded
                .engine
                .as_ref()
                .map(|profile| {
                    animsmith_engine::project_prediction_provenance_v1(profile, &loaded.source)
                })
                .transpose()
                .map_err(|error| error.to_string())?;
            let doc = loaded.document();
            let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
            let grids = MetricGrids::new(doc);
            let ctx = CheckCtx::new(&grids, &roles, config);
            let evaluations = {
                let mut checks: Vec<Box<dyn Check + '_>> = all_checks();
                checks.push(Box::new(
                    EngineAddressabilityCheck::new(&loaded.source, prediction_provenance.as_ref())
                        .map_err(|error| error.to_string())?,
                ));
                evaluate_checks(&ctx, &checks, CheckSelection::All)
                    .map_err(|error| error.to_string())?
            };
            let finding_count = evaluations.iter().map(|check| check.findings().len()).sum();
            let html = animsmith_report::render(
                &grids,
                &roles,
                &evaluations,
                prediction_provenance.as_ref(),
                clip.as_deref(),
            );
            std::fs::write(&output, &html)
                .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
            publish::emit_text(&render::render_report_written(
                &output,
                doc.clips.len(),
                finding_count,
                html.len(),
            ));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Transform {
            input,
            output,
            clip,
            slice,
            hold_extend,
            drop_duplicate_loop_endpoint,
            gait_anchor,
            prune_constant_tracks,
            fps,
        } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            let loaded = load_with_config(&input, &loaded_config)?;
            let config = &loaded_config.config;
            let mut doc = loaded.into_document();
            let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
            let window = match &slice {
                None => None,
                Some(spec) => {
                    let (a, b) = spec
                        .split_once(':')
                        .ok_or_else(|| format!("--slice wants START:END, got {spec}"))?;
                    let a: f64 = a.parse().map_err(|e| format!("--slice start: {e}"))?;
                    let b: f64 = b.parse().map_err(|e| format!("--slice end: {e}"))?;
                    if b <= a {
                        return Err(format!("--slice end must be after start ({spec})"));
                    }
                    Some((a, b))
                }
            };
            let skeleton = doc.skeleton.clone();
            let mut touched = 0usize;
            // Transform reporting is transactional with the artifact: a later
            // clip refusal must not leave earlier per-clip success text on
            // stdout when the command publishes nothing.
            let mut messages = String::new();
            for c in doc.clips.iter_mut() {
                if clip.as_deref().is_some_and(|name| name != c.name) {
                    continue;
                }
                touched += 1;
                if let Some((a, b)) = window {
                    animsmith_core::transform::slice(c, a, b, fps);
                    messages.push_str(&render::render_transform_slice(c, a, b));
                }
                if let Some(hold) = hold_extend {
                    animsmith_core::transform::hold_extend(c, hold);
                    messages.push_str(&render::render_transform_hold_extend(c, hold));
                }
                if drop_duplicate_loop_endpoint {
                    if config.expectations_for(&c.name).looping != Some(true) {
                        messages.push_str(
                            &render::render_transform_duplicate_loop_endpoint_skipped(
                                &c.name,
                                "clip is not declared `loop = true` in config",
                            ),
                        );
                    } else {
                        match animsmith_core::transform::drop_duplicate_loop_endpoint(c) {
                            Ok(Some(outcome)) => messages.push_str(
                                &render::render_transform_duplicate_loop_endpoint(
                                    &c.name,
                                    outcome.removed_keys_per_track,
                                    outcome.duration_before_s,
                                    outcome.duration_after_s,
                                ),
                            ),
                            Ok(None) => messages.push_str(
                                &render::render_transform_duplicate_loop_endpoint_skipped(
                                    &c.name,
                                    "no mechanically removable repeated endpoint",
                                ),
                            ),
                            Err(reason) => messages.push_str(
                                &render::render_transform_duplicate_loop_endpoint_skipped(
                                    &c.name,
                                    &reason.to_string(),
                                ),
                            ),
                        }
                    }
                }
                if gait_anchor {
                    let outcome = animsmith_core::transform::align_gait_anchor(
                        &skeleton,
                        c,
                        &roles,
                        fps,
                        animsmith_core::transform::GaitTrajectoryPolicy::InPlace,
                    )
                    .map_err(|reason| format!("clip {:?}: {reason}", c.name))?;
                    messages.push_str(&render::render_transform_gait_anchor(
                        &c.name,
                        outcome.phase_before,
                        outcome.phase_after,
                        outcome.frame_offset,
                        outcome.seam_after,
                    ));
                }
                if prune_constant_tracks {
                    // `animates_bones` is an animation/motion contract.  Keep its
                    // exact-name tracks even if they are mechanically constant, so
                    // subsequent lint can still diagnose an unmet declaration.
                    // `[rig] required_bones` is deliberately not included: it is a
                    // skeleton-presence contract, not per-clip motion evidence.
                    let protected_bones = config
                        .expectations_for(&c.name)
                        .animates_bones
                        .as_deref()
                        .map(|names| {
                            skeleton
                                .bones
                                .iter()
                                .enumerate()
                                .filter_map(|(id, bone)| {
                                    names.iter().any(|name| name == &bone.name).then_some(id)
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let outcome = animsmith_core::transform::prune_constant_tracks(
                        &skeleton,
                        c,
                        &protected_bones,
                    );
                    messages.push_str(&render::render_transform_constant_track_pruning(
                        &c.name, &skeleton, &outcome,
                    ));
                }
            }
            if touched == 0 {
                return Err(match clip {
                    Some(name) => format!("clip '{name}' not found in {}", input.display()),
                    None => format!("{} has no clips", input.display()),
                });
            }
            let summary = animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            messages.push_str(&render::render_write_summary(&output, &summary));
            publish::emit_text(&messages);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Fix {
            input,
            output,
            in_place,
            repairs,
            dry_run,
        } => {
            let ext = input
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if ext != "glb" && ext != "gltf" {
                return Err(format!(
                    "{}: fix operates on .glb/.gltf (convert FBX first)",
                    input.display()
                ));
            }
            let selected = select_repairs(repairs);
            if !dry_run && output.is_none() && !in_place {
                return Err(
                    "fix requires --output <PATH> or --in-place (use --dry-run to inspect only)"
                        .into(),
                );
            }
            let output = if in_place {
                Some(input.clone())
            } else {
                output
            };
            let mut pending = false;
            let mut session =
                animsmith_gltf::fix::FixSession::read(&input).map_err(|e| e.to_string())?;
            let mut reports = Vec::new();
            for repair in selected {
                let report = session.apply(repair);
                pending |= report.total_fixed() > 0;
                reports.push((repair, report));
            }
            if let Some(output) = output.as_deref() {
                session.write(&input, output).map_err(|e| e.to_string())?;
            }
            // clap rejects --dry-run with a write target, so `output` is None
            // exactly when this is a dry run. Stream every selected repair's
            // lines through one checked attempt: one closed stdout must
            // produce one diagnosis, regardless of the repair count, without
            // retaining an asset-sized transcript.
            publish::emit_fix_reports(reports.iter(), output.as_deref());
            // Dry run doubles as a CI check mode: pending repairs are
            // findings, mirroring `lint`'s exit contract.
            Ok(if dry_run && pending {
                ExitCode::from(EXIT_FINDINGS)
            } else {
                ExitCode::SUCCESS
            })
        }
        #[cfg(feature = "fbx")]
        Cmd::Convert {
            input,
            output,
            animation_only,
            material_texture_recipe,
            bake_static_mesh_transforms,
            format,
        } => run_conversion(
            &ConversionRequest {
                input: &input,
                output: &output,
                animation_only,
                material_texture_recipe: material_texture_recipe.as_deref(),
                bake_static_mesh_transforms,
                format,
            },
            current_tool(),
        ),
        #[cfg(feature = "fbx")]
        Cmd::Assemble {
            recipe,
            output,
            evidence,
            format,
        } => assembly::run(
            &assembly::Request {
                recipe,
                output,
                evidence,
                config: cli.config,
                format,
            },
            current_tool(),
        ),
        Cmd::Scale { operation } => {
            let request = match operation {
                ScaleCmd::WholeDocument {
                    input,
                    output,
                    factor,
                    evidence,
                    format,
                } => scale::Request {
                    operation: scale::Operation::WholeDocument { factor },
                    input,
                    output,
                    evidence,
                    format,
                },
                ScaleCmd::RestBind {
                    input,
                    output,
                    source_skin_index,
                    source_root_node_index,
                    expected_factor,
                    evidence,
                    format,
                } => scale::Request {
                    operation: scale::Operation::RestBind {
                        source_skin_index,
                        source_root_node_index,
                        expected_factor,
                    },
                    input,
                    output,
                    evidence,
                    format,
                },
            };
            scale::run(&request, current_tool())
        }
        Cmd::Generate { operation } => match operation {
            GenerateCmd::ContactFragment {
                input,
                clip,
                output,
                format,
            } => {
                let loaded_config = load_config(cli.config.as_deref())?;
                contact_producer::run_direct(
                    &input,
                    &clip,
                    &output,
                    format,
                    current_tool(),
                    &loaded_config,
                )
            }
            GenerateCmd::Addressability { input, format } => {
                // Static profile/configuration validation deliberately precedes
                // input I/O, matching lint and preserving #464's typed error
                // boundary for unknown or malformed tuples.
                let loaded_config = load_config(cli.config.as_deref())?;
                let loaded = load_with_config(&input, &loaded_config)?;
                let prediction_provenance = loaded
                    .engine
                    .as_ref()
                    .map(|profile| {
                        animsmith_engine::project_prediction_provenance_v1(profile, &loaded.source)
                    })
                    .transpose()
                    .map_err(|error| error.to_string())?;
                let inventory = GltfAnimationAddressabilityInventoryV1::from_source(&loaded.source)
                    .map_err(|error| error.to_string())?;

                let config = &loaded_config.config;
                let doc = loaded.document();
                let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                let grids = MetricGrids::new(doc);
                let ctx = CheckCtx::new(&grids, &roles, config);
                let bevy = build_bevy_animation_addressability_adapter_v1(
                    &loaded.source,
                    &inventory,
                    prediction_provenance,
                    &ctx,
                )
                .map_err(|error| error.to_string())?;
                let report = GltfAnimationAddressabilityV1::new(current_tool(), inventory, bevy)
                    .map_err(|error| error.to_string())?;
                let requires_failure = report.bevy().is_some_and(|adapter| {
                    animsmith_core::evaluation::lint_requires_failure(
                        std::slice::from_ref(adapter.check()),
                        Severity::Error,
                        &BTreeSet::new(),
                    )
                });

                match format {
                    PresentationFormat::Json => render::print_json(&report)?,
                    PresentationFormat::Text => {
                        publish::emit_text(&render::render_addressability_text(&report));
                    }
                    PresentationFormat::Markdown => {
                        publish::emit_text(&render::render_addressability_markdown(&report));
                    }
                }
                Ok(if requires_failure {
                    ExitCode::from(EXIT_FINDINGS)
                } else {
                    ExitCode::SUCCESS
                })
            }
            GenerateCmd::ImportAdvice { input, format } => {
                // Configuration/profile resolution precedes input I/O so an
                // incomplete or unknown tuple remains an operator error at
                // the same boundary as lint and addressability.
                let loaded_config = load_config(cli.config.as_deref())?;
                let static_profile = loaded_config.engine.as_ref().ok_or_else(|| {
                    "generate import-advice requires a complete [engine] selection and settings"
                        .to_owned()
                })?;
                if !EngineImportAdviceV1::supports_profile(static_profile.profile()) {
                    return Err(
                        animsmith_engine::EngineImportAdviceError::UnsupportedProfile.to_string(),
                    );
                }
                let loaded = load_with_config(&input, &loaded_config)?;
                let profile = loaded.engine.as_ref().ok_or_else(|| {
                    "generate import-advice requires a complete [engine] selection and settings"
                        .to_owned()
                })?;
                let config = &loaded_config.config;
                let report = EngineImportAdviceV1::from_source(
                    current_tool(),
                    &loaded.source,
                    profile,
                    config,
                )
                .map_err(|error| error.to_string())?;
                let refused = report.state() == EngineImportAdviceStateV1::Refused;
                match format {
                    PresentationFormat::Json => render::print_json(&report)?,
                    PresentationFormat::Text => {
                        publish::emit_text(&render::render_import_advice_text(&report));
                    }
                    PresentationFormat::Markdown => {
                        publish::emit_text(&render::render_import_advice_markdown(&report));
                    }
                }
                Ok(if refused {
                    ExitCode::from(EXIT_FINDINGS)
                } else {
                    ExitCode::SUCCESS
                })
            }
        },
        Cmd::Diff { a, b, format } => {
            let config = load_config(cli.config.as_deref())?;
            let ma = load_measurements(&a, &config)?;
            let mb = load_measurements(&b, &config)?;
            let deltas = animsmith_core::diff::diff_measurements(&ma, &mb);
            let has_deltas = !deltas.is_empty();
            match format {
                Format::Json => render::print_json(&DiffEnvelope::new(
                    current_tool(),
                    a.display().to_string(),
                    b.display().to_string(),
                    deltas,
                ))?,
                Format::Text => {
                    publish::emit_text_lines(render::render_diff_text(&deltas));
                }
            }
            Ok(if has_deltas {
                ExitCode::from(EXIT_FINDINGS)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}

/// Measurements for `diff`: an asset file (measured now) or a one-file
/// output-v11 `measure`/`lint` JSON report carrying measurements-v15.
fn load_measurements(
    path: &Path,
    loaded_config: &LoadedConfig,
) -> Result<BTreeMap<String, animsmith_core::measure::ClipMeasurements>, String> {
    let config = &loaded_config.config;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "json" {
        let input = std::fs::File::open(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let report = MeasurementReportInput::read_from(input).map_err(|error| match error {
            MeasurementReportReadError::InvalidJson { source } => {
                format!("bad JSON in {}: {source}", path.display())
            }
            _ => format!("{} {error}", path.display()),
        })?;
        // Only the current output-v11 envelope with measurement contract v15 is
        // accepted. Older report shapes are intentionally not retained while
        // the project is alpha.
        let file_count = report.file_count();
        let files = report.into_files().map_err(|error| {
            format!(
                "{} {}",
                path.display(),
                diff_report_error(&error, file_count)
            )
        })?;
        let [file]: [animsmith_core::MeasurementReportFile; 1] =
            files.try_into().map_err(|files: Vec<_>| {
                format!("{} {}", path.display(), diff_file_count_error(files.len()))
            })?;
        return Ok(file.into_measurements().into_parts().0);
    }
    let loaded = load_with_config(path, loaded_config)?;
    let doc = loaded.document();
    let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
    let grids = MetricGrids::new(doc);
    Ok(animsmith_core::measure::measure_document(
        &grids, &roles, config,
    ))
}

/// Add `diff` consumer policy and operator remediation to neutral core errors.
fn diff_report_error(error: &MeasurementReportError, file_count: Option<usize>) -> String {
    const REMEDIATION: &str =
        "regenerate it from the original asset with `animsmith measure --format json <asset>`";
    if let Some(found) = file_count.filter(|found| *found != 1)
        && error.file_index().is_some()
    {
        return diff_file_count_error(found);
    }
    match error {
        MeasurementReportError::MissingOutputVersion => {
            format!("is not an animsmith report envelope (no `schema_version`); {REMEDIATION}")
        }
        MeasurementReportError::UnsupportedOutputVersion { found } => format!(
            "has schema_version {found}; this build reads schema_version {}; {REMEDIATION}",
            animsmith_core::OUTPUT_SCHEMA_VERSION
        ),
        MeasurementReportError::WrongOutputIdentity => format!(
            "does not identify output contract {}; {REMEDIATION}",
            animsmith_core::OUTPUT_SCHEMA_ID
        ),
        MeasurementReportError::MissingCommand => {
            format!("is not an animsmith measurement report (no `command`); {REMEDIATION}")
        }
        MeasurementReportError::UnsupportedCommand { command } => {
            format!("is a {command:?} report; diff reads only measure or lint reports")
        }
        MeasurementReportError::MissingFiles => {
            format!("is not an animsmith report envelope (no `files` array); {REMEDIATION}")
        }
        MeasurementReportError::File {
            file_index: 0,
            source,
        } => match source {
            MeasurementFileError::MissingPath => {
                format!("report file record has no `path`; {REMEDIATION}")
            }
            MeasurementFileError::MissingMeasurements => "report has no measurements".into(),
            MeasurementFileError::MissingMeasurementVersion => {
                format!("has no versioned measurement contract; {REMEDIATION}")
            }
            MeasurementFileError::UnsupportedMeasurementVersion { found } => format!(
                "has measurement schema_version {found}; this build reads measurement schema_version {}; {REMEDIATION}",
                animsmith_core::MEASUREMENTS_SCHEMA_VERSION
            ),
            MeasurementFileError::WrongMeasurementIdentity => format!(
                "does not identify measurement contract {}; {REMEDIATION}",
                animsmith_core::MEASUREMENTS_SCHEMA_ID
            ),
            MeasurementFileError::MissingClips => "measurement contract has no `clips` map".into(),
            MeasurementFileError::InvalidMeasurements { source } => {
                format!("has invalid measurements: {source}; {REMEDIATION}")
            }
            _ => source.to_string(),
        },
        _ => error.to_string(),
    }
}

fn diff_file_count_error(found: usize) -> String {
    format!("contains {found} file records; diff expects a single-file measurement report")
}

fn require_files(files: &[PathBuf]) -> Result<(), String> {
    if files.is_empty() {
        Err("no input files given".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum InputFormat {
    Gltf,
    #[cfg(feature = "fbx")]
    Fbx,
}

enum InputLoadError {
    Gltf(animsmith_gltf::LoadError),
    #[cfg(feature = "fbx")]
    Fbx(animsmith_fbx::LoadError),
}

impl std::fmt::Display for InputLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gltf(error) => error.fmt(formatter),
            #[cfg(feature = "fbx")]
            Self::Fbx(error) => error.fmt(formatter),
        }
    }
}

fn producer_load_failure(error: InputLoadError) -> producer::Failure {
    use producer::{Failure, Kind, Stage};

    match error {
        InputLoadError::Gltf(error @ animsmith_gltf::LoadError::Io { .. }) => {
            Failure::operator(error)
        }
        InputLoadError::Gltf(error @ animsmith_gltf::LoadError::ExternalResource(_)) => {
            Failure::operator(error)
        }
        #[cfg(feature = "fbx")]
        InputLoadError::Fbx(error @ animsmith_fbx::LoadError::Path(_)) => Failure::operator(error),
        // These are the only operator exceptions. Every other current or
        // future typed loader variant is a fact about bytes the producer was
        // able to read, so it follows the same stable refusal policy.
        error => Failure::refusal(Stage::Load, Kind::UnreadableSource, error),
    }
}

fn input_format(path: &Path) -> Result<InputFormat, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "glb" | "gltf" => Ok(InputFormat::Gltf),
        #[cfg(feature = "fbx")]
        "fbx" => {
            path.to_str()
                .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))?;
            Ok(InputFormat::Fbx)
        }
        #[cfg(not(feature = "fbx"))]
        "fbx" => Err(format!(
            "{}: this animsmith build has no FBX support (rebuild with the default `fbx` feature)",
            path.display()
        )),
        _ => Err(format!(
            "{}: unsupported input (expected .glb, .gltf, or .fbx)",
            path.display()
        )),
    }
}

fn capture_input(path: &Path) -> Result<(InputFormat, Vec<u8>), String> {
    let format = input_format(path)?;
    let bytes = std::fs::read(path).map_err(|error| match format {
        InputFormat::Gltf => format!("failed to read {}: {error}", path.display()),
        #[cfg(feature = "fbx")]
        InputFormat::Fbx => format!("FBX parse error: {error}"),
    })?;
    Ok((format, bytes))
}

fn input_resource_root(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(feature = "fbx")]
fn load_bytes_typed(
    path: &Path,
    format: InputFormat,
    bytes: &[u8],
) -> Result<Document, InputLoadError> {
    let resource_root = input_resource_root(path);
    match format {
        InputFormat::Gltf => {
            animsmith_gltf::load_bytes_with_resource_root(path, bytes, resource_root)
                .map_err(InputLoadError::Gltf)
        }
        InputFormat::Fbx => {
            animsmith_fbx::load_bytes_with_resource_root(path, bytes, resource_root)
                .map_err(InputLoadError::Fbx)
        }
    }
}

#[cfg(feature = "fbx")]
fn load_source_bytes_typed(
    path: &Path,
    format: InputFormat,
    bytes: &[u8],
) -> Result<animsmith_core::LoadedSource, InputLoadError> {
    let resource_root = input_resource_root(path);
    match format {
        InputFormat::Gltf => {
            animsmith_gltf::load_source_bytes_with_resource_root(path, bytes, resource_root)
                .map_err(InputLoadError::Gltf)
        }
        InputFormat::Fbx => {
            animsmith_fbx::load_source_bytes_with_resource_root(path, bytes, resource_root)
                .map_err(InputLoadError::Fbx)
        }
    }
}

#[cfg(not(feature = "fbx"))]
fn load_source_bytes_typed(
    path: &Path,
    format: InputFormat,
    bytes: &[u8],
) -> Result<animsmith_core::LoadedSource, InputLoadError> {
    let resource_root = input_resource_root(path);
    match format {
        InputFormat::Gltf => {
            animsmith_gltf::load_source_bytes_with_resource_root(path, bytes, resource_root)
                .map_err(InputLoadError::Gltf)
        }
    }
}

struct LoadedInput {
    source: animsmith_core::LoadedSource,
    engine: Option<ResolvedProfile>,
}

impl LoadedInput {
    fn document(&self) -> &Document {
        self.source.document()
    }

    fn input(&self) -> &InputIdentity {
        self.source.source_facts().primary_identity()
    }

    fn dependency_closure(&self) -> &animsmith_core::DependencyClosureV1 {
        self.source.dependency_closure()
    }

    fn source_facts(&self) -> animsmith_core::SourceFactsViewV1<'_> {
        self.source.source_facts()
    }

    fn into_document(self) -> Document {
        self.source.into_document()
    }
}

/// Read one primary input once, derive its retained-evidence identity from
/// those exact bytes, and parse that same byte slice. This deliberately does
/// not identify a reopened path: a report must describe the bytes judged.
fn load_with_identity(path: &Path) -> Result<LoadedInput, String> {
    let (format, bytes) = capture_input(path)?;
    let source =
        load_source_bytes_typed(path, format, &bytes).map_err(|error| error.to_string())?;
    Ok(LoadedInput {
        source,
        engine: None,
    })
}

fn load_with_config(path: &Path, config: &LoadedConfig) -> Result<LoadedInput, String> {
    let mut loaded = load_with_identity(path)?;
    let facts = loaded.source.source_facts();
    loaded.engine = config.resolve_engine_input(facts.format(), loaded.source.document())?;
    Ok(loaded)
}

/// Load an input for a strict producer while preserving its typed refusal
/// boundary. Ordinary inspection commands surface loader diagnostics as
/// operator errors; a sidecar producer has already committed to a result
/// contract, so malformed source facts are instead a refusal.
fn load_with_config_for_producer(
    path: &Path,
    config: &LoadedConfig,
) -> Result<LoadedInput, producer::Failure> {
    let (format, bytes) = capture_input(path).map_err(producer::Failure::operator)?;
    let source =
        load_source_bytes_typed(path, format, &bytes).map_err(contact_producer_load_failure)?;
    let engine = config
        .resolve_engine_input(source.source_facts().format(), source.document())
        .map_err(producer::Failure::operator)?;
    Ok(LoadedInput { source, engine })
}

fn contact_producer_load_failure(error: InputLoadError) -> producer::Failure {
    match error {
        InputLoadError::Gltf(error @ animsmith_gltf::LoadError::ExternalResource(_)) => {
            producer::Failure::refusal(
                producer::Stage::Load,
                producer::Kind::IncompleteEvidence,
                error,
            )
        }
        error => producer_load_failure(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_engine_toml_maps_to_the_public_resolver_without_new_core_authority() {
        let text = r#"
[rig]
profile = "auto"

[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[engine.settings]
convert_units = true
bake_axis_conversion = true
root_motion_source = "Reference/Root"

[clips."*"]
[clips."*".engine_settings]
root_rotation = "bake"
root_position_y = "bake"
root_position_xz = "bake"

[clips."walk*"]
[clips."walk*".engine_settings]
root_rotation = "extract"

[clips.walk_forward]
[clips.walk_forward.engine_settings]
root_position_y = "extract"
"#;
        let (core, declaration) = parse_config(text).unwrap();
        assert_eq!(
            core.clips.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["*", "walk*", "walk_forward"]
        );
        let from_cli = animsmith_engine::resolve_static(declaration)
            .unwrap()
            .unwrap()
            .resolve_input(
                animsmith_core::SourceFormatV1::Fbx,
                &["walk_forward".into()],
            )
            .unwrap();

        let direct = EngineDeclaration {
            selection: Some(ProfileSelection::new(
                "unity-generic",
                1,
                "6000.3",
                "fbx-model-importer",
            )),
            document_settings: Some(BTreeMap::from([
                ("convert_units".into(), SettingValue::Boolean(true)),
                ("bake_axis_conversion".into(), SettingValue::Boolean(true)),
                (
                    "root_motion_source".into(),
                    SettingValue::SourceTransformPath("Reference/Root".into()),
                ),
            ])),
            clip_settings: BTreeMap::from([
                (
                    "*".into(),
                    BTreeMap::from([
                        (
                            "root_rotation".into(),
                            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
                        ),
                        (
                            "root_position_y".into(),
                            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
                        ),
                        (
                            "root_position_xz".into(),
                            SettingValue::BakeOrExtract(BakeOrExtract::Bake),
                        ),
                    ]),
                ),
                (
                    "walk*".into(),
                    BTreeMap::from([(
                        "root_rotation".into(),
                        SettingValue::BakeOrExtract(BakeOrExtract::Extract),
                    )]),
                ),
                (
                    "walk_forward".into(),
                    BTreeMap::from([(
                        "root_position_y".into(),
                        SettingValue::BakeOrExtract(BakeOrExtract::Extract),
                    )]),
                ),
            ]),
        };
        let direct = animsmith_engine::resolve_static(direct)
            .unwrap()
            .unwrap()
            .resolve_input(
                animsmith_core::SourceFormatV1::Fbx,
                &["walk_forward".into()],
            )
            .unwrap();
        assert_eq!(from_cli, direct);
    }

    #[test]
    fn cli_engine_toml_rejects_incomplete_selection_and_source_unit_escape_hatch() {
        let incomplete = parse_config(
            r#"
[engine]
profile = "bevy"
"#,
        )
        .unwrap_err();
        assert!(incomplete.contains("requires profile, profile_revision"));

        let source_unit = parse_config("source_unit = \"metre\"").unwrap_err();
        assert!(source_unit.contains("unknown field `source_unit`"));
    }

    #[test]
    fn cli_clip_engine_settings_without_selection_reach_the_typed_error() {
        let (_, declaration) = parse_config(
            r#"
[clips.walk]
[clips.walk.engine_settings]
root_rotation = "bake"
"#,
        )
        .unwrap();
        assert_eq!(
            animsmith_engine::resolve_static(declaration),
            Err(animsmith_engine::ResolutionError::SettingsWithoutSelection)
        );
    }

    #[cfg(feature = "fbx")]
    struct SerializationFailureDelivery {
        stdout: Vec<u8>,
        text_stderr: Vec<u8>,
    }

    #[cfg(feature = "fbx")]
    impl producer::RefusalDelivery for SerializationFailureDelivery {
        fn serialize(&mut self, _record: &producer::RefusalRecord) -> Result<Vec<u8>, String> {
            Err("cannot serialize JSON output: injected refusal failure".into())
        }

        fn emit_json(&mut self, bytes: &[u8]) {
            self.stdout.extend_from_slice(bytes);
        }

        fn emit_text(&mut self, text: &str) {
            self.text_stderr.extend_from_slice(text.as_bytes());
        }
    }

    #[cfg(feature = "fbx")]
    #[test]
    fn refusal_serialization_failure_dispatches_as_operator_without_stdout() {
        let mut delivery = SerializationFailureDelivery {
            stdout: Vec::new(),
            text_stderr: Vec::new(),
        };
        let result = producer::emit_rejection(
            producer::Command::Convert,
            Format::Json,
            current_tool(),
            producer::Rejection::new(
                producer::Stage::Load,
                producer::Kind::UnreadableSource,
                "malformed source bytes",
            ),
            &mut delivery,
        );
        let mut operator_stderr = String::new();
        let code = finish_run_with(result, |rendered| operator_stderr.push_str(rendered));

        assert_eq!(code, ExitCode::from(EXIT_OPERATOR));
        assert!(delivery.stdout.is_empty(), "serialization wrote stdout");
        assert!(
            delivery.text_stderr.is_empty(),
            "JSON refusal used the text refusal channel"
        );
        assert!(
            operator_stderr.contains("cannot serialize JSON output"),
            "{operator_stderr}"
        );
    }

    #[cfg(feature = "fbx")]
    #[test]
    fn producer_loader_classifier_keeps_io_and_required_dependencies_operator_owned() {
        let cases = [
            (
                InputLoadError::Gltf(animsmith_gltf::LoadError::Io {
                    path: "missing.bin".into(),
                    source: std::io::Error::from(std::io::ErrorKind::NotFound),
                }),
                true,
            ),
            (
                InputLoadError::Fbx(animsmith_fbx::LoadError::Path("non-UTF-8".into())),
                true,
            ),
            (
                InputLoadError::Gltf(animsmith_gltf::LoadError::ExternalResource(
                    animsmith_gltf::ExternalResourceFailure::Unavailable,
                )),
                true,
            ),
            (
                InputLoadError::Gltf(animsmith_gltf::LoadError::Malformed(
                    "bad animation shape".into(),
                )),
                false,
            ),
            (
                InputLoadError::Fbx(animsmith_fbx::LoadError::Fbx("bad container".into())),
                false,
            ),
            (
                InputLoadError::Fbx(animsmith_fbx::LoadError::Bake {
                    take: "walk".into(),
                    message: "bad curve".into(),
                }),
                false,
            ),
        ];

        for (error, operator) in cases {
            let expected_detail = error.to_string();
            match (producer_load_failure(error), operator) {
                (producer::Failure::Operator(detail), true) => {
                    assert_eq!(detail, expected_detail);
                }
                (producer::Failure::Refusal(rejection), false) => {
                    assert_eq!(rejection.stage, producer::Stage::Load);
                    assert_eq!(rejection.kind, producer::Kind::UnreadableSource);
                    assert_eq!(rejection.detail, expected_detail);
                }
                (producer::Failure::Operator(detail), false) => {
                    panic!("unexpected operator classification: {detail}");
                }
                (producer::Failure::Refusal(rejection), true) => {
                    panic!("unexpected refusal classification: {}", rejection.detail);
                }
            }
        }
    }

    #[test]
    fn diff_owns_remediation_for_invalid_measurements() {
        // Construct the public typed error directly so this test pins CLI
        // policy independently of which malformed input first exposes it.
        let error = MeasurementReportError::File {
            file_index: 0,
            source: MeasurementFileError::InvalidMeasurements {
                source: animsmith_core::MeasurementContractError::NonFiniteValue {
                    path: "meshes[0].aabb.min[0]".into(),
                },
            },
        };

        assert_eq!(
            diff_report_error(&error, Some(1)),
            "has invalid measurements: measurement value meshes[0].aabb.min[0] must be finite; regenerate it from the original asset with `animsmith measure --format json <asset>`"
        );
    }
}
