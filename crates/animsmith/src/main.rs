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

#[cfg(feature = "report")]
use animsmith_core::evaluate_checks;
use animsmith_core::{
    Check, CheckCtx, CheckSelection, Config, DiffEnvelope, EngineRootMotionClipIntentInputV1,
    EngineRootMotionClipMappingStateV1, EngineRootMotionProjectIntentV1, LintEnvelopeV19,
    LintFileReportV19, MeasureEnvelope, MeasureFileReport, MeasurementContract,
    MeasurementFileError, MeasurementReportError, MeasurementReportInput,
    MeasurementReportReadError, MetricGrids, RigInfo, RootMotionProjectOwnerV1, Severity,
    TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES, ToolInfo, ToolSource,
    TransitionFamilyDeclarationInputV1, TransitionPoseDecisionV1, TransitionPoseStatusV1,
    all_checks, evaluate_checks_v2, evaluate_document_transition_poses_v1,
    resolve_configured_roles,
};
use animsmith_core::{Document, InputIdentity};
#[cfg(feature = "report")]
use animsmith_engine::EngineAddressabilityCheck;
use animsmith_engine::{
    BakeOrExtract, BevyGltfHandlerEnvironmentV2, BevyLoadMeshesStateV2, ENGINE_CHECK_IDS_V2,
    EngineAddressabilityCheckV3, EngineClipBoundaryCheck, EngineDeclaration, EngineDeclarationV2,
    EngineImportAdviceStateV1, EngineImportAdviceStateV2, EngineImportAdviceV1,
    EngineImportAdviceV2, EngineRootMotionCheck, EngineTrackSupportCheck, EngineUnitScaleCheck,
    GltfAddressabilityV2, GltfAnimationAddressabilityInventoryV1, GltfAnimationAddressabilityV1,
    ProfileSelection, ResolvedProfile, ResolvedProfileSettingsV2, ResolvedProfileV2, SettingMap,
    SettingMapV2, SettingValue, SettingValueV2, StaticResolution, StaticResolutionV2,
    TargetPointerWidth, UnityAnimationTypeV2, UnityAvatarSetupV2, UnrealSampleRateV2,
    build_bevy_addressability_adapter_v2, build_bevy_animation_addressability_adapter_v1,
    lookup_profile_v2,
};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
#[cfg(feature = "fbx")]
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
#[cfg(feature = "fbx")]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(feature = "fbx")]
mod assembly;
#[cfg(feature = "report")]
mod collection_dashboard;
mod collection_directional_speed;
mod collection_directional_speed_policy;
mod collection_lint;
mod collection_manifest;
mod collection_output;
mod collection_transition_pose;
mod contact_producer;
#[cfg(feature = "fbx")]
mod material_recipe;
mod producer;
mod publish;
mod render;
mod scale;
mod skeleton_compare;
#[cfg(feature = "fbx")]
mod staged_selector;
#[cfg(feature = "fbx")]
mod texture_processing;
mod transition_family;

/// Exit codes, matching common asset-validation gate conventions:
/// 0 = no failing findings (warnings/notes allowed), 1 = error
/// findings, 2 = operator error.
const EXIT_FINDINGS: u8 = 1;
const EXIT_OPERATOR: u8 = 2;
const COLLECTION_OUTPUT_V11_VALIDATION_HANDSHAKE: &[u8] =
    b"animsmith-internal collection-output-valid urn:animsmith:schema:collection-output:11 11\n";

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
    /// Compare declared entry and exit poses for one document's transition families.
    EvaluateTransitionPoses {
        /// Input .glb, .gltf, or .fbx file.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Emit the immutable transition-pose evaluation V1 JSON contract.
        #[arg(long, value_enum)]
        format: JsonOnlyFormat,
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
        /// Input .glb, .gltf, or .fbx file. In comparison mode this is the immutable before input.
        file: PathBuf,
        /// Output HTML report path.
        #[arg(short, long)]
        output: PathBuf,
        /// Restrict the report to one clip.
        #[arg(long)]
        clip: Option<String>,
        /// Immutable after input for a synchronized visual comparison.
        #[arg(long, value_name = "FILE")]
        compare_after: Option<PathBuf>,
        /// Exact before clip in comparison mode; no correspondence is inferred.
        #[arg(long, value_name = "CLIP")]
        before_clip: Option<String>,
        /// Exact after clip in comparison mode; no correspondence is inferred.
        #[arg(long, value_name = "CLIP")]
        after_clip: Option<String>,
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
        long_about = "Compare the measurements of two inputs (asset files, one-file current output-v19 `measure` or `lint` JSON carrying measurements-v18, historical output-v18 carrying measurements-v17, or output-v17/output-v16/output-v15/output-v14/output-v13 JSON carrying measurements-v16) and report movement beyond significance thresholds. Exits 1 on significant movement."
    )]
    Diff {
        /// Before input: asset file or one-file output-v19 through output-v13 `measure`/`lint` JSON report.
        a: PathBuf,
        /// After input: asset file or one-file output-v19 through output-v13 `measure`/`lint` JSON report.
        b: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Compare two declared skeleton authorities without retargeting either asset.
    Skeleton {
        #[command(subcommand)]
        operation: SkeletonCmd,
    },
}

/// Structural skeleton compatibility commands.
#[derive(Subcommand)]
enum SkeletonCmd {
    /// Compare one source skeleton against one target authority using strict correspondence TOML.
    #[command(
        long_about = "Compare selected source and target skeletons using a strict, identity-pinned correspondence TOML and emit skeleton-compatibility V1 evidence. This is structural evidence only: it never retargets, rewrites rest/bind data, infers aliases, or establishes runtime deformation, masking, contact, gameplay, or artistic acceptance."
    )]
    Compare {
        /// Source .glb, .gltf, or .fbx asset.
        source: PathBuf,
        /// Immutable target-skeleton authority asset.
        target: PathBuf,
        /// Strict skeleton-correspondence V1 TOML.
        #[arg(long)]
        correspondence: PathBuf,
        /// Render versioned JSON evidence rather than a stable text summary.
        #[arg(long, value_enum, default_value_t = Format::Json)]
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
        /// Emit the collection-output V3 JSON contract.
        #[arg(long, value_enum, default_value_t = CollectionFormat::Json)]
        format: CollectionFormat,
    },
    /// Render a bounded current-state dashboard from strict collection evidence.
    #[cfg(feature = "report")]
    #[command(
        long_about = "Render one self-contained offline current-state collection dashboard from strict current collection-output V11 evidence. The separately written collection-dashboard V1 JSON authority binds the exact collection-output bytes and any supplied compatible transition-pose evaluation. Filters only select displayed rows; they never alter incomplete or unavailable evidence. This command does not load source assets, infer clip semantics, score quality, or establish engine, retargeting, contacts, visual/artistic, or gameplay acceptance."
    )]
    Dashboard {
        /// Strict current collection-output V11 JSON input.
        #[arg(long, value_name = "COLLECTION-OUTPUT.json")]
        collection: PathBuf,
        /// Destination self-contained HTML dashboard.
        #[arg(short, long)]
        output: PathBuf,
        /// Destination collection-dashboard V1 JSON authority.
        #[arg(long)]
        authority: PathBuf,
        /// Optional compatible collection transition-pose evaluation V1 JSON.
        #[arg(long)]
        evaluation: Option<PathBuf>,
        /// Exact logical clip id and safe relative per-asset report reference, as ID=PATH.
        #[arg(long = "asset-report", value_name = "LOGICAL_ID=RELATIVE_PATH")]
        asset_reports: Vec<String>,
    },
    /// Strictly validate one collection-output document without publishing it.
    #[command(hide = true)]
    ValidateOutput,
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
    /// Evaluate one declared directional-speed policy against collection-output V3 evidence.
    #[command(
        long_about = "Evaluate one strict manifest-bound directional-speed policy against one strict collection-output V3 document. The policy must declare every member of one directional-blend runtime set in manifest order; this command never infers members, filenames, controller behavior, or diagonal normalization. It writes only the immutable collection-directional-speed-evaluation V1 JSON result to stdout. Invalid, stale, wrong-kind, unreadable, malformed, or over-budget control inputs write no result and exit 2. Incomplete or not-evaluable evidence and declared-policy findings write a result and exit 1; only a complete passing policy exits 0."
    )]
    EvaluateDirectionalSpeed {
        /// Strict collection-directional-speed-policy V1 TOML input.
        #[arg(long, value_name = "POLICY.toml")]
        policy: PathBuf,
        /// Strict collection-output V3 JSON input.
        #[arg(long, value_name = "COLLECTION-OUTPUT.json")]
        evidence: PathBuf,
        /// Emit the immutable directional-speed evaluation JSON contract.
        #[arg(long, value_enum, default_value_t = CollectionFormat::Json)]
        format: CollectionFormat,
    },
    /// Evaluate manifest-bound transition families over their declared raw sources.
    EvaluateTransitionPoses {
        /// Strict collection-manifest V1 TOML input.
        #[arg(value_name = "COLLECTION.toml")]
        manifest: PathBuf,
        /// Strict manifest-bound transition-family V1 TOML envelope.
        #[arg(long, value_name = "TRANSITION_FAMILIES.toml")]
        families: PathBuf,
        /// Emit the immutable transition-pose evaluation V1 JSON contract.
        #[arg(long, value_enum)]
        format: JsonOnlyFormat,
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
        long_about = "Generate one bounded glTF animation-addressability document from immutable source facts and dependency closure. Without the exact Bevy revision-3 profile, it emits the immutable V1 animation inventory and makes no scene, skin, target-path, UUID, or named-map claims. With that profile, it emits the separate V2 rich scene, skin, path, UUID, and bounded named-map projections beside the preserved V1 inventory, reusing the existing engine-addressability evaluation; --target-pointer-width is required for target UUIDs. Neither contract claims runtime loading, graph wiring, target survival, scene spawning, playback, or cross-file behavior."
    )]
    Addressability {
        /// Input .glb or .gltf file.
        #[arg(value_name = "INPUT")]
        input: PathBuf,
        /// Render canonical JSON or a presentation-only text/Markdown view.
        #[arg(long, value_enum, default_value_t = PresentationFormat::Json)]
        format: PresentationFormat,
        /// Explicit Bevy target pointer width required for exact target UUIDs.
        #[arg(long, value_enum, value_name = "32|64")]
        target_pointer_width: Option<TargetPointerWidthArg>,
    },
    /// Project bounded, versioned importer suggestions from one exact engine profile.
    #[command(
        long_about = "Generate bounded engine-import advice from one same-load source and its exact resolved engine profile/settings. Revision-1 Unity 6000.3 Generic/Humanoid emits its documented importer properties; revision-1 Unreal/Godot remains a typed refusal. Revision-2 Godot 4.7 emits only animation/fps and animation/trimming, while revision-2 Unreal 5.8 emits only the explicitly configured sample-rate fields. No frame ranges, per-animation slices, root-motion behavior, unit conversion, engine execution, or readback are inferred."
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

/// The transition-pose evaluator has no presentation view: its one result is
/// the stable machine-readable V1 contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum JsonOnlyFormat {
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

/// Explicit target-platform pointer width used by Bevy's target-id preimage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TargetPointerWidthArg {
    #[value(name = "32")]
    /// A 32-bit target `usize` encoding.
    Bits32,
    #[value(name = "64")]
    /// A 64-bit target `usize` encoding.
    Bits64,
}

impl From<TargetPointerWidthArg> for TargetPointerWidth {
    fn from(value: TargetPointerWidthArg) -> Self {
        match value {
            TargetPointerWidthArg::Bits32 => Self::Bits32,
            TargetPointerWidthArg::Bits64 => Self::Bits64,
        }
    }
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

#[cfg(feature = "report")]
fn write_report(
    output: &Path,
    doc: &Document,
    finding_count: usize,
    html: String,
) -> Result<ExitCode, String> {
    std::fs::write(output, &html)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    publish::emit_text(&render::render_report_written(
        output,
        doc.clips.len(),
        finding_count,
        html.len(),
    ));
    Ok(ExitCode::SUCCESS)
}

#[cfg(feature = "report")]
fn require_comparison_output_distinct(
    output: &Path,
    inputs: &[(&str, &Path)],
) -> Result<(), String> {
    let destination = publish::PublicationDestination::new("comparison output", output)?;
    for &(label, input) in inputs {
        let input_identity = publish::input_identity(input)?;
        if input_identity == destination.identity()
            || same_file_entry(input, destination.identity())?
        {
            return Err(format!(
                "report comparison {label} and output must be different files"
            ));
        }
    }
    publish::require_writable_destination(destination.identity())
}

#[cfg(all(feature = "report", unix))]
fn same_file_entry(input: &Path, destination: &Path) -> Result<bool, String> {
    use std::os::unix::fs::MetadataExt;
    let input = std::fs::metadata(input)
        .map_err(|error| format!("cannot read {}: {error}", input.display()))?;
    let destination = match std::fs::metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("cannot inspect {}: {error}", destination.display())),
    };
    Ok(input.dev() == destination.dev() && input.ino() == destination.ino())
}

#[cfg(all(feature = "report", not(unix)))]
fn same_file_entry(_input: &Path, _destination: &Path) -> Result<bool, String> {
    Ok(false)
}

#[cfg(feature = "report")]
fn publish_comparison_report(
    output: &Path,
    inputs: &[(&str, &Path)],
    html: &str,
) -> Result<(), String> {
    require_comparison_output_distinct(output, inputs)?;
    let destination = publish::PublicationDestination::new("comparison output", output)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".animsmith-comparison-")
        .tempfile_in(publish::parent_or_current(destination.identity()))
        .map_err(|error| format!("cannot stage comparison output: {error}"))?;
    temp.write_all(html.as_bytes())
        .map_err(|error| format!("cannot stage comparison output: {error}"))?;
    temp.flush()
        .map_err(|error| format!("cannot stage comparison output: {error}"))?;
    publish::publish_single(temp.path(), destination.identity())
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
    engine_profile_v2: Option<StaticResolutionV2>,
    /// Strictly decoded document declaration bound to the exact complete
    /// config source. It is retained for the later transition-pose command,
    /// but this admission slice does not evaluate it.
    #[allow(
        dead_code,
        reason = "transition-pose evaluation and commands are intentionally deferred"
    )]
    transition_families: Option<TransitionFamilyDeclarationInputV1>,
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
    Integer(i64),
    Text(String),
}

fn engine_setting_value(key: &str, value: EngineSettingToml) -> Result<SettingValue, String> {
    match value {
        EngineSettingToml::Boolean(value) => Ok(SettingValue::Boolean(value)),
        EngineSettingToml::Text(value) if key == "root_motion_source" => {
            Ok(SettingValue::SourceTransformPath(value))
        }
        EngineSettingToml::Text(value) if value == "bake" => {
            Ok(SettingValue::BakeOrExtract(BakeOrExtract::Bake))
        }
        EngineSettingToml::Text(value) if value == "extract" => {
            Ok(SettingValue::BakeOrExtract(BakeOrExtract::Extract))
        }
        EngineSettingToml::Text(value) => Ok(SettingValue::SourceTransformPath(value)),
        EngineSettingToml::Integer(value) => Err(format!(
            "invalid revision-1 engine setting value {value} for {key:?}"
        )),
    }
}

fn engine_setting_map(values: BTreeMap<String, EngineSettingToml>) -> Result<SettingMap, String> {
    values
        .into_iter()
        .map(|(key, value)| {
            let value = engine_setting_value(&key, value)?;
            Ok((key, value))
        })
        .collect()
}

fn unreal_sample_rate(value: &str) -> Option<UnrealSampleRateV2> {
    match value {
        "default_30" => Some(UnrealSampleRateV2::Default30),
        "source_determined" => Some(UnrealSampleRateV2::SourceDetermined),
        _ => value
            .strip_prefix("custom_hz(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| {
                let parsed = value.parse::<u32>().ok()?;
                (value == parsed.to_string()).then_some(parsed)
            })
            .map(UnrealSampleRateV2::CustomHz),
    }
}

fn engine_setting_value_v2(key: &str, value: EngineSettingToml) -> Result<SettingValueV2, String> {
    match (key, value) {
        (_, EngineSettingToml::Boolean(value)) => Ok(SettingValueV2::Boolean(value)),
        ("animation_fps", EngineSettingToml::Integer(value)) => u32::try_from(value)
            .map(SettingValueV2::PositiveInteger)
            .map_err(|_| format!("invalid revision-2 engine setting value {value} for {key:?}")),
        ("sample_rate", EngineSettingToml::Text(value)) => unreal_sample_rate(&value)
            .map(SettingValueV2::SampleRate)
            .ok_or_else(|| {
                format!("invalid revision-2 engine setting value {value:?} for {key:?}")
            }),
        ("load_meshes", EngineSettingToml::Text(value)) if value == "empty" => Ok(
            SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Empty),
        ),
        ("load_meshes", EngineSettingToml::Text(value)) if value == "nonempty" => Ok(
            SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Nonempty),
        ),
        ("extension_handler_environment", EngineSettingToml::Text(value))
            if value == "bare_empty" =>
        {
            Ok(SettingValueV2::HandlerEnvironment(
                BevyGltfHandlerEnvironmentV2::BareEmpty,
            ))
        }
        ("extension_handler_environment", EngineSettingToml::Text(value))
            if value == "bevy_pbr_stock_0_19" =>
        {
            Ok(SettingValueV2::HandlerEnvironment(
                BevyGltfHandlerEnvironmentV2::BevyPbrStock019,
            ))
        }
        ("animation_type", EngineSettingToml::Text(value)) if value == "generic" => {
            Ok(SettingValueV2::AnimationType(UnityAnimationTypeV2::Generic))
        }
        ("animation_type", EngineSettingToml::Text(value)) if value == "humanoid" => Ok(
            SettingValueV2::AnimationType(UnityAnimationTypeV2::Humanoid),
        ),
        ("animation_type", EngineSettingToml::Text(value)) if value == "legacy" => {
            Ok(SettingValueV2::AnimationType(UnityAnimationTypeV2::Legacy))
        }
        ("avatar_setup", EngineSettingToml::Text(value)) if value == "create_from_this_model" => {
            Ok(SettingValueV2::AvatarSetup(
                UnityAvatarSetupV2::CreateFromThisModel,
            ))
        }
        ("avatar_setup", EngineSettingToml::Text(value)) if value == "copy_from_other_avatar" => {
            Ok(SettingValueV2::AvatarSetup(
                UnityAvatarSetupV2::CopyFromOtherAvatar,
            ))
        }
        ("root_motion_source", EngineSettingToml::Text(value)) => {
            Ok(SettingValueV2::SourceTransformPath(value))
        }
        (
            "root_rotation" | "root_position_y" | "root_position_xz",
            EngineSettingToml::Text(value),
        ) if value == "bake" => Ok(SettingValueV2::BakeOrExtract(BakeOrExtract::Bake)),
        (
            "root_rotation" | "root_position_y" | "root_position_xz",
            EngineSettingToml::Text(value),
        ) if value == "extract" => Ok(SettingValueV2::BakeOrExtract(BakeOrExtract::Extract)),
        (key, EngineSettingToml::Text(value)) => Err(format!(
            "invalid revision-2 engine setting value {value:?} for {key:?}"
        )),
        (key, EngineSettingToml::Integer(value)) => Err(format!(
            "invalid revision-2 engine setting value {value} for {key:?}"
        )),
    }
}

fn engine_setting_map_v2(
    values: BTreeMap<String, EngineSettingToml>,
) -> Result<SettingMapV2, String> {
    values
        .into_iter()
        .map(|(key, value)| {
            let value = engine_setting_value_v2(&key, value)?;
            Ok((key, value))
        })
        .collect()
}

#[derive(Debug)]
enum ParsedEngineDeclaration {
    V1(EngineDeclaration),
    V2(EngineDeclarationV2),
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

fn parse_config(
    bytes: &[u8],
) -> Result<
    (
        Config,
        ParsedEngineDeclaration,
        TransitionFamilyDeclarationInputV1,
    ),
    String,
> {
    // This reader sees the exact, unmodified config bytes so its source
    // identity covers engine and unrelated config fields as well as the
    // declaration itself. Remove the subtree only after strict decoding so
    // the generic Config serde contract continues to own every other field.
    let transition_families = transition_family::parse_document_transition_families_bytes(bytes)
        .map_err(|error| error.to_string())?;
    // The strict reader above has already checked UTF-8 on these exact bytes.
    // Keep the generic TOML parser after it, so a source cap or encoding
    // refusal cannot be preempted by generic config decoding.
    let text = std::str::from_utf8(bytes).map_err(|_| {
        "transition-family declaration control error (transition-family-encoding)".to_owned()
    })?;
    let mut root: toml::Table = toml::from_str(text).map_err(|error| error.to_string())?;
    root.remove("transition_families");
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
            clip_settings.insert(selector.clone(), settings);
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
            (selection, engine.settings)
        }
        None => (None, None),
    };
    debug_assert_eq!(
        engine_declared,
        selection.is_some() || document_settings.is_some()
    );

    let uses_v2_contract = selection
        .as_ref()
        .is_some_and(|selection| lookup_profile_v2(selection).is_ok());
    let engine_declaration = if uses_v2_contract {
        ParsedEngineDeclaration::V2(EngineDeclarationV2 {
            selection,
            document_settings: document_settings.map(engine_setting_map_v2).transpose()?,
            clip_settings: clip_settings
                .into_iter()
                .map(|(selector, settings)| Ok((selector, engine_setting_map_v2(settings)?)))
                .collect::<Result<_, String>>()?,
        })
    } else {
        ParsedEngineDeclaration::V1(EngineDeclaration {
            selection,
            document_settings: document_settings.map(engine_setting_map).transpose()?,
            clip_settings: clip_settings
                .into_iter()
                .map(|(selector, settings)| Ok((selector, engine_setting_map(settings)?)))
                .collect::<Result<_, String>>()?,
        })
    };

    let config = toml::Value::Table(root)
        .try_into::<Config>()
        .map_err(|error| error.to_string())?;
    Ok((config, engine_declaration, transition_families))
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
        return Ok(LoadedConfig::without_file());
    };
    let bytes = read_document_config_bounded(&path)
        .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    let (config, declaration, transition_families) =
        parse_config(&bytes).map_err(|error| format!("bad config {}: {error}", path.display()))?;
    config
        .validate()
        .map_err(|e| format!("bad config {}: {e}", path.display()))?;
    let (engine, engine_profile_v2) = match declaration {
        ParsedEngineDeclaration::V1(declaration) => (
            animsmith_engine::resolve_static(declaration)
                .map_err(|error| format!("bad config {}: {error}", path.display()))?,
            None,
        ),
        ParsedEngineDeclaration::V2(declaration) => (
            None,
            animsmith_engine::resolve_static_v2(declaration)
                .map_err(|error| format!("bad config {}: {error}", path.display()))?,
        ),
    };
    Ok(LoadedConfig {
        config,
        engine,
        engine_profile_v2,
        transition_families: Some(transition_families),
        path: Some(path.clone()),
        control_input: Some(path.clone()),
        #[cfg(feature = "fbx")]
        source: Some(LoadedConfigSource { path, bytes }),
    })
}

/// Read only the strict transition-family source budget plus its first excess
/// byte. The declaration reader classifies the excess against the same exact
/// bytes before generic TOML retains any config value.
fn read_document_config_bounded(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

impl LoadedConfig {
    fn without_file() -> Self {
        Self {
            config: Config::default(),
            engine: None,
            engine_profile_v2: None,
            transition_families: None,
            path: None,
            control_input: None,
            #[cfg(feature = "fbx")]
            source: None,
        }
    }

    /// Return the transition-family declaration for one document invocation.
    ///
    /// Omitted `--config` has one defined V1 declaration source: the exact
    /// zero-byte TOML sequence. Passing it through the same strict reader as
    /// a file makes omitted configuration and an explicitly empty file bind
    /// the same factual source and normalized declaration identities.
    fn transition_pose_declaration(&self) -> Result<TransitionFamilyDeclarationInputV1, String> {
        match &self.transition_families {
            Some(declaration) => Ok(declaration.clone()),
            None => transition_family::parse_document_transition_families_bytes(b"")
                .map_err(|error| error.to_string()),
        }
    }

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

    fn resolve_engine_input_v2(
        &self,
        source_format: animsmith_core::SourceFormatV1,
        document: &Document,
    ) -> Result<Option<ResolvedProfileV2>, String> {
        let Some(engine) = &self.engine else {
            return Ok(None);
        };
        engine
            .resolve_input_v2_iter(
                source_format,
                document.clips.iter().map(|clip| clip.name.as_str()),
            )
            .map(Some)
            .map_err(|error| match &self.path {
                Some(path) => format!("bad config {}: {error}", path.display()),
                None => format!("bad config: {error}"),
            })
    }

    fn resolve_engine_profile_v2_input(
        &self,
        source_format: animsmith_core::SourceFormatV1,
        document: &Document,
    ) -> Result<Option<ResolvedProfileSettingsV2>, String> {
        let Some(engine) = &self.engine_profile_v2 else {
            return Ok(None);
        };
        engine
            .resolve_input_with_clips_iter(
                source_format,
                document.clips.iter().map(|clip| clip.name.as_str()),
            )
            .map(Some)
            .map_err(|error| match &self.path {
                Some(path) => format!("bad config {}: {error}", path.display()),
                None => format!("bad config: {error}"),
            })
    }

    /// Resolve a V2 profile for lint after configuration admission.
    ///
    /// The exact Unity Generic V2 root-motion slice is FBX-only. Its lifecycle
    /// defines any non-FBX source as no work for the check, so lint must retain
    /// the static configuration validation but omit source-bound V2 evidence
    /// instead of surfacing the resolver's unsupported-format operator error.
    /// Other V2 consumers retain the resolver's strict source-format boundary.
    fn resolve_engine_profile_v2_lint_input(
        &self,
        source_format: animsmith_core::SourceFormatV1,
        document: &Document,
    ) -> Result<Option<ResolvedProfileSettingsV2>, String> {
        if self.engine_profile_v2.as_ref().is_some_and(|engine| {
            is_unity_generic_root_motion_selection(engine.profile().selection())
                && source_format != animsmith_core::SourceFormatV1::Fbx
        }) {
            return Ok(None);
        }
        self.resolve_engine_profile_v2_input(source_format, document)
    }
}

fn full_check_ids() -> Result<Vec<&'static str>, String> {
    let mut known = all_checks()
        .into_iter()
        .map(|check| check.id())
        .collect::<Vec<_>>();
    known.extend_from_slice(ENGINE_CHECK_IDS_V2);

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

fn root_motion_owner(
    owner: Option<animsmith_core::config::MovementOwner>,
) -> Option<RootMotionProjectOwnerV1> {
    owner.map(|owner| match owner {
        animsmith_core::config::MovementOwner::Gameplay => RootMotionProjectOwnerV1::Gameplay,
        animsmith_core::config::MovementOwner::Animation => RootMotionProjectOwnerV1::Animation,
    })
}

fn root_motion_project_intent(
    source: &animsmith_core::LoadedSource,
    config: &Config,
    resolved_root_bone_index: Option<u64>,
) -> Result<EngineRootMotionProjectIntentV1, String> {
    let document = source.document();
    let mut mapped_clip_indices = BTreeSet::new();
    let source_intent = source
        .source_facts()
        .clips()
        .rows()
        .iter()
        // Keep this producer bounded even when a future loader supplies a
        // lazy source-row view. The project-intent builder retains the same
        // prefix plus one overflow witness and never needs the tail.
        .take(animsmith_core::ENGINE_ROOT_MOTION_PROJECT_INTENT_V1_MAX_CLIPS.saturating_add(1))
        .map(|source_clip| {
            let normalized_clip_index = match source_clip.normalized_clip_index().state() {
                animsmith_core::SourceObservationStateV1::Observed(index) => *index,
                animsmith_core::SourceObservationStateV1::ProvenAbsent => {
                    return Ok(EngineRootMotionClipIntentInputV1::new(
                        EngineRootMotionClipMappingStateV1::ProvenAbsent,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ));
                }
                animsmith_core::SourceObservationStateV1::Unavailable(_) => {
                    return Ok(EngineRootMotionClipIntentInputV1::new(
                        EngineRootMotionClipMappingStateV1::Unavailable,
                        None,
                        None,
                        None,
                        None,
                        None,
                    ));
                }
            };
            let clip = document
                .clips
                .get(normalized_clip_index)
                .ok_or_else(|| "raw source clip maps outside the normalized document".to_owned())?;
            mapped_clip_indices.insert(normalized_clip_index);
            let expectations = config.expectations_for(&clip.name);
            Ok(EngineRootMotionClipIntentInputV1::new(
                EngineRootMotionClipMappingStateV1::Observed,
                Some(u64::try_from(normalized_clip_index).map_err(|_| {
                    "normalized clip index exceeds the root-motion contract".to_owned()
                })?),
                Some(clip.name.clone()),
                root_motion_owner(expectations.normalized_movement_owner_xz()),
                root_motion_owner(expectations.movement_owner_y),
                root_motion_owner(expectations.movement_owner_yaw),
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let unmapped_declarations = document
        .clips
        .iter()
        .enumerate()
        .filter(|(index, _)| !mapped_clip_indices.contains(index))
        .map(|(_, clip)| {
            let expectations = config.expectations_for(&clip.name);
            [
                root_motion_owner(expectations.normalized_movement_owner_xz()),
                root_motion_owner(expectations.movement_owner_y),
                root_motion_owner(expectations.movement_owner_yaw),
            ]
        });
    EngineRootMotionProjectIntentV1::from_clips_with_root_and_unmapped(
        resolved_root_bone_index,
        source_intent,
        unmapped_declarations,
    )
    .map_err(|error| error.to_string())
}

fn is_unity_generic_root_motion_selection(selection: &ProfileSelection) -> bool {
    selection.family() == "unity-generic"
        && selection.profile_revision() == 2
        && selection.engine_version() == "6000.3"
        && selection.importer() == "fbx-model-importer"
}

fn is_bevy_gltf_addressability_v2_selection(selection: &ProfileSelection) -> bool {
    selection.family() == "bevy"
        && selection.profile_revision() == 3
        && selection.engine_version() == "0.19.0"
        && selection.importer() == "gltf-asset-loader"
}

fn is_unity_generic_root_motion_profile(profile: &ResolvedProfileSettingsV2) -> bool {
    is_unity_generic_root_motion_selection(profile.profile().selection())
        && profile.source_format() == animsmith_core::SourceFormatV1::Fbx
}

struct LintAnalysis {
    report: LintFileReportV19,
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
    let doc = loaded.document();
    let roles = resolve_configured_roles(&doc.skeleton, &config.config.rig);
    let resolved_root_bone_index = roles
        .get(animsmith_core::profile::Role::Root)
        .map(u64::try_from)
        .transpose()
        .map_err(|_| "resolved Root bone index exceeds the root-motion contract".to_owned())?;
    let prediction_provenance_v3 = loaded
        .engine_v2
        .as_ref()
        .map(|profile| animsmith_engine::project_prediction_provenance_v3(profile, &loaded.source))
        .transpose()
        .map_err(|error| error.to_string())?;
    let runtime_node_selectors = config
        .config
        .runtime_node_selectors()
        .map(|selectors| selectors.selectors().to_vec())
        .unwrap_or_default();
    let prediction_provenance_v6 = loaded
        .engine_v4
        .as_ref()
        .filter(|profile| is_unity_generic_root_motion_profile(profile))
        .map(|profile| {
            animsmith_engine::project_prediction_provenance_v6(
                profile,
                &loaded.source,
                runtime_node_selectors.clone(),
                root_motion_project_intent(
                    &loaded.source,
                    &config.config,
                    resolved_root_bone_index,
                )?,
            )
            .map_err(|error| error.to_string())
        })
        .transpose()?;
    let prediction_provenance_v5 = loaded
        .engine_v4
        .as_ref()
        .filter(|profile| !is_unity_generic_root_motion_profile(profile))
        .map(|profile| {
            animsmith_engine::project_prediction_provenance_v5(
                profile,
                &loaded.source,
                runtime_node_selectors.clone(),
            )
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    debug_assert!(
        [
            prediction_provenance_v3.is_some(),
            prediction_provenance_v5.is_some(),
            prediction_provenance_v6.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            <= 1
    );
    let grids = MetricGrids::new(doc);
    let indexed_measurements =
        animsmith_core::measure::measure_document_indexed(&grids, &roles, &config.config);
    let ctx = CheckCtx::new(&grids, &roles, &config.config);
    let evaluations = {
        let mut checks: Vec<Box<dyn Check + '_>> = all_checks();
        checks.push(Box::new(
            EngineAddressabilityCheckV3::new(&loaded.source, prediction_provenance_v3.as_ref())
                .map_err(|error| error.to_string())?,
        ));
        checks.push(Box::new(
            EngineClipBoundaryCheck::new(&loaded.source, prediction_provenance_v3.as_ref())
                .map_err(|error| error.to_string())?,
        ));
        checks.push(Box::new(
            EngineUnitScaleCheck::new_v5(&loaded.source, prediction_provenance_v5.as_ref())
                .map_err(|error| error.to_string())?,
        ));
        checks.push(Box::new(
            EngineTrackSupportCheck::new(&loaded.source, prediction_provenance_v5.as_ref())
                .map_err(|error| error.to_string())?,
        ));
        checks.push(Box::new(
            EngineRootMotionCheck::new(
                &loaded.source,
                prediction_provenance_v6.as_ref(),
                &roles,
                &indexed_measurements,
            )
            .map_err(|error| error.to_string())?,
        ));
        evaluate_checks_v2(&ctx, &checks, selection).map_err(|error| error.to_string())?
    };
    let requires_failure =
        animsmith_core::evaluation::lint_requires_failure(&evaluations, fail_at, allowed);
    let measurements = doc
        .clips
        .iter()
        .map(|clip| clip.name.clone())
        .zip(indexed_measurements.iter().cloned())
        .collect();
    let rig = RigInfo::from_resolved(doc, &roles).map_err(|error| error.to_string())?;
    let measurements =
        MeasurementContract::new(measurements, animsmith_core::measure::measure_assets(doc))
            .map_err(|error| error.to_string())?;
    let report = match (prediction_provenance_v6, prediction_provenance_v5) {
        (Some(provenance), None) => LintFileReportV19::new_v6(
            path_label,
            input,
            rig,
            Some(provenance),
            evaluations,
            measurements,
        ),
        (None, Some(provenance)) => LintFileReportV19::new_v5(
            path_label,
            input,
            rig,
            Some(provenance),
            evaluations,
            measurements,
        ),
        (None, None) => LintFileReportV19::new(
            path_label,
            input,
            rig,
            prediction_provenance_v3,
            evaluations,
            measurements,
        ),
        (Some(_), Some(_)) => unreachable!("prediction provenance revisions are exclusive"),
    }
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
                reports.push(
                    MeasureFileReport::new(
                        file.display().to_string(),
                        input,
                        RigInfo::from_resolved(doc, &roles).map_err(|error| error.to_string())?,
                        MeasurementContract::new(
                            animsmith_core::measure::measure_document(&grids, &roles, config),
                            animsmith_core::measure::measure_assets(doc),
                        )
                        .map_err(|error| error.to_string())?,
                    )
                    .map_err(|error| error.to_string())?,
                );
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
                let loaded = load_with_config_v2(file, &loaded_config)?;
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
                    let envelope = LintEnvelopeV19::new(current_tool(), reports)
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
        Cmd::EvaluateTransitionPoses { input, format } => {
            debug_assert_eq!(format, JsonOnlyFormat::Json);
            let loaded_config = load_config(cli.config.as_deref())?;
            let declaration = loaded_config.transition_pose_declaration()?;
            let loaded = load_with_config(&input, &loaded_config)?;
            let result = evaluate_document_transition_poses_v1(
                &declaration,
                loaded.dependency_closure(),
                loaded.document(),
            )
            .map_err(|error| error.to_string())?;
            let complete_pass = result.status() == TransitionPoseStatusV1::Complete
                && result.decision() == TransitionPoseDecisionV1::Pass;
            // This command's immutable result is its only publication. Unlike
            // ordinary check/producer streams, a failed delivery is therefore
            // an operator error rather than a completed outcome with a
            // best-effort diagnostic.
            let bytes = publish::serialize_record(&result)?;
            publish::emit_required_json(&bytes)?;
            Ok(if complete_pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_FINDINGS)
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
                #[cfg(feature = "report")]
                CollectionCmd::Dashboard {
                    collection,
                    output,
                    authority,
                    evaluation,
                    asset_reports,
                } => collection_dashboard::run(
                    &collection,
                    &output,
                    &authority,
                    evaluation.as_deref(),
                    &asset_reports,
                ),
                CollectionCmd::ValidateOutput => {
                    let stdin = std::io::stdin();
                    collection_output::read_current_collection_output(stdin.lock()).map_err(
                        |error| format!("invalid collection output from stdin: {error}"),
                    )?;
                    publish::emit_required_text(COLLECTION_OUTPUT_V11_VALIDATION_HANDSHAKE)?;
                    Ok(ExitCode::SUCCESS)
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
                CollectionCmd::EvaluateTransitionPoses {
                    manifest,
                    families,
                    format,
                } => {
                    debug_assert_eq!(format, JsonOnlyFormat::Json);
                    collection_transition_pose::run(&manifest, &families)
                }
            }
        }
        #[cfg(feature = "report")]
        Cmd::Report {
            file,
            output,
            clip,
            compare_after,
            before_clip,
            after_clip,
        } => {
            let loaded_config = load_config(cli.config.as_deref())?;
            full_check_ids()?;
            let loaded = load_with_config(&file, &loaded_config)?;
            let comparison_after = if let Some(after_path) = compare_after.as_ref() {
                if clip.is_some() {
                    return Err("--clip cannot be used with --compare-after; declare both --before-clip and --after-clip".into());
                }
                let before_name = before_clip
                    .as_deref()
                    .ok_or_else(|| "--compare-after requires --before-clip".to_string())?;
                let after_name = after_clip
                    .as_deref()
                    .ok_or_else(|| "--compare-after requires --after-clip".to_string())?;
                let after_loaded = load_with_config(after_path, &loaded_config)?;
                animsmith_report::preflight_comparison_sources(
                    &loaded.source,
                    before_name,
                    &after_loaded.source,
                    after_name,
                )
                .map_err(|error| error.to_string())?;
                let mut comparison_inputs = vec![
                    ("before input", file.as_path()),
                    ("after input", after_path.as_path()),
                ];
                if let Some(config_path) = loaded_config.control_input() {
                    comparison_inputs.push(("configuration input", config_path));
                }
                require_comparison_output_distinct(&output, &comparison_inputs)?;
                let destinations = [("comparison output", output.as_path())];
                publish::require_external_dependencies_safe_for_publication(
                    "report comparison before input",
                    input_resource_root(&file),
                    loaded.dependency_closure(),
                    &destinations,
                )?;
                publish::require_external_dependencies_safe_for_publication(
                    "report comparison after input",
                    input_resource_root(after_path),
                    after_loaded.dependency_closure(),
                    &destinations,
                )?;
                Some(after_loaded)
            } else if before_clip.is_some() || after_clip.is_some() {
                return Err("--before-clip and --after-clip require --compare-after".into());
            } else {
                None
            };
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
            let finding_count: usize = evaluations.iter().map(|check| check.findings().len()).sum();
            let html = match comparison_after {
                None => animsmith_report::render(
                    &grids,
                    &roles,
                    &evaluations,
                    prediction_provenance.as_ref(),
                    clip.as_deref(),
                ),
                Some(after_loaded) => {
                    let before_clip = before_clip
                        .ok_or_else(|| "--compare-after requires --before-clip".to_string())?;
                    let after_clip = after_clip
                        .ok_or_else(|| "--compare-after requires --after-clip".to_string())?;
                    let after_path = compare_after
                        .as_ref()
                        .expect("comparison load is paired with its path");
                    let after_prediction_provenance = after_loaded
                        .engine
                        .as_ref()
                        .map(|profile| {
                            animsmith_engine::project_prediction_provenance_v1(
                                profile,
                                &after_loaded.source,
                            )
                        })
                        .transpose()
                        .map_err(|error| error.to_string())?;
                    let after_doc = after_loaded.document();
                    let after_roles = resolve_configured_roles(&after_doc.skeleton, &config.rig);
                    let after_grids = MetricGrids::new(after_doc);
                    let after_ctx = CheckCtx::new(&after_grids, &after_roles, config);
                    let after_evaluations = {
                        let mut checks: Vec<Box<dyn Check + '_>> = all_checks();
                        checks.push(Box::new(
                            EngineAddressabilityCheck::new(
                                &after_loaded.source,
                                after_prediction_provenance.as_ref(),
                            )
                            .map_err(|error| error.to_string())?,
                        ));
                        evaluate_checks(&after_ctx, &checks, CheckSelection::All)
                            .map_err(|error| error.to_string())?
                    };
                    let html = animsmith_report::render_comparison(
                        animsmith_report::ComparisonSide {
                            source: &loaded.source,
                            grids: &grids,
                            roles: &roles,
                            checks: &evaluations,
                            config,
                            prediction_provenance: prediction_provenance.as_ref(),
                            clip: &before_clip,
                        },
                        animsmith_report::ComparisonSide {
                            source: &after_loaded.source,
                            grids: &after_grids,
                            roles: &after_roles,
                            checks: &after_evaluations,
                            config,
                            prediction_provenance: after_prediction_provenance.as_ref(),
                            clip: &after_clip,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    let finding_count = finding_count
                        + after_evaluations
                            .iter()
                            .map(|check| check.findings().len())
                            .sum::<usize>();
                    let mut comparison_inputs = vec![
                        ("before input", file.as_path()),
                        ("after input", after_path.as_path()),
                    ];
                    if let Some(config_path) = loaded_config.control_input() {
                        comparison_inputs.push(("configuration input", config_path));
                    }
                    publish_comparison_report(&output, &comparison_inputs, &html)?;
                    publish::emit_text(&render::render_report_written(
                        &output,
                        doc.clips.len(),
                        finding_count,
                        html.len(),
                    ));
                    return Ok(ExitCode::SUCCESS);
                }
            };
            write_report(&output, doc, finding_count, html)
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
            GenerateCmd::Addressability {
                input,
                format,
                target_pointer_width,
            } => {
                // Static profile/configuration validation deliberately precedes
                // input I/O, matching lint and preserving #464's typed error
                // boundary for unknown or malformed tuples.
                let loaded_config = load_config(cli.config.as_deref())?;
                let rich_bevy = loaded_config
                    .engine_profile_v2
                    .as_ref()
                    .is_some_and(|engine| {
                        is_bevy_gltf_addressability_v2_selection(engine.profile().selection())
                    });
                if target_pointer_width.is_some() && !rich_bevy {
                    return Err(
                        "--target-pointer-width is valid only with the exact Bevy revision-3 0.19.0 gltf-asset-loader profile"
                            .to_owned(),
                    );
                }
                if rich_bevy {
                    let loaded = load_with_config_v2(&input, &loaded_config)?;
                    let profile = loaded.engine_v4.as_ref().ok_or_else(|| {
                        "generate addressability requires the exact supported Bevy revision-3 profile"
                            .to_owned()
                    })?;
                    let raw = loaded
                        .source
                        .raw_gltf_addressability_inventory()
                        .cloned()
                        .ok_or_else(|| {
                            "glTF loader did not retain raw addressability inventory".to_owned()
                        })?;
                    let animations =
                        GltfAnimationAddressabilityInventoryV1::from_source(&loaded.source)
                            .map_err(|error| error.to_string())?;
                    let provenance = animsmith_engine::project_prediction_provenance_v4(
                        profile,
                        &loaded.source,
                        Vec::new(),
                    )
                    .map_err(|error| error.to_string())?;
                    let config = &loaded_config.config;
                    let doc = loaded.document();
                    let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                    let grids = MetricGrids::new(doc);
                    let ctx = CheckCtx::new(&grids, &roles, config);
                    let bevy = build_bevy_addressability_adapter_v2(
                        &loaded.source,
                        &raw,
                        &animations,
                        profile,
                        provenance,
                        target_pointer_width.map(Into::into),
                        &ctx,
                    )
                    .map_err(|error| error.to_string())?;
                    let report = GltfAddressabilityV2::new(current_tool(), raw, animations, bevy)
                        .map_err(|error| error.to_string())?;
                    let requires_failure = report.has_required_prediction_unavailable();
                    match format {
                        PresentationFormat::Json => render::print_json(&report)?,
                        PresentationFormat::Text => {
                            publish::emit_text(&render::render_addressability_v2_text(&report));
                        }
                        PresentationFormat::Markdown => {
                            publish::emit_text(&render::render_addressability_v2_markdown(&report));
                        }
                    }
                    return Ok(if requires_failure {
                        ExitCode::from(EXIT_FINDINGS)
                    } else {
                        ExitCode::SUCCESS
                    });
                }
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
                if let Some(static_profile) = loaded_config.engine.as_ref() {
                    if !EngineImportAdviceV1::supports_profile(static_profile.profile()) {
                        return Err(
                            animsmith_engine::EngineImportAdviceError::UnsupportedProfile
                                .to_string(),
                        );
                    }
                    let loaded = load_with_config(&input, &loaded_config)?;
                    let profile = loaded.engine.as_ref().ok_or_else(|| {
                        "generate import-advice requires a complete [engine] selection and settings"
                            .to_owned()
                    })?;
                    let report = EngineImportAdviceV1::from_source(
                        current_tool(),
                        &loaded.source,
                        profile,
                        &loaded_config.config,
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
                    return Ok(if refused {
                        ExitCode::from(EXIT_FINDINGS)
                    } else {
                        ExitCode::SUCCESS
                    });
                }

                let static_profile = loaded_config.engine_profile_v2.as_ref().ok_or_else(|| {
                    "generate import-advice requires a complete [engine] selection and settings"
                        .to_owned()
                })?;
                if !EngineImportAdviceV2::supports_profile(static_profile.profile()) {
                    return Err(
                        animsmith_engine::EngineImportAdviceError::UnsupportedProfileV2.to_string(),
                    );
                }
                let loaded = load_with_config_v2(&input, &loaded_config)?;
                let profile = loaded.engine_v4.as_ref().ok_or_else(|| {
                    "generate import-advice requires a supported revision-2 [engine] selection"
                        .to_owned()
                })?;
                let report =
                    EngineImportAdviceV2::from_source(current_tool(), &loaded.source, profile)
                        .map_err(|error| error.to_string())?;
                let refused = report.state() == EngineImportAdviceStateV2::Refused;
                match format {
                    PresentationFormat::Json => render::print_json(&report)?,
                    PresentationFormat::Text => {
                        publish::emit_text(&render::render_import_advice_v2_text(&report));
                    }
                    PresentationFormat::Markdown => {
                        publish::emit_text(&render::render_import_advice_v2_markdown(&report));
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
        Cmd::Skeleton { operation } => match operation {
            SkeletonCmd::Compare {
                source,
                target,
                correspondence,
                format,
            } => {
                if cli.config.is_some() {
                    return Err("--config is not supported with skeleton compare; correspondence TOML is its complete control input".to_owned());
                }
                skeleton_compare::run(
                    &source,
                    &target,
                    &correspondence,
                    current_tool(),
                    format == Format::Json,
                )
            }
        },
    }
}

/// Measurements for `diff`: an asset file (measured now) or a one-file
/// current output-v19 carrying measurements-v18, historical output-v18
/// carrying measurements-v17, or historical
/// output-v17/output-v16/output-v15/output-v14/output-v13 `measure`/`lint` JSON
/// carrying measurements-v16.
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
        // Current output-v19 and historical output-v18 through output-v11 envelopes
        // are accepted only with their version-matched measurements contract.
        // The V11 route retains its original V1 evidence validation; producers
        // emit V16.
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
            MeasurementFileError::UnsupportedMeasurementVersion { found, expected } => format!(
                "has measurement schema_version {found}; this reader expects measurement schema_version {expected}; {REMEDIATION}"
            ),
            MeasurementFileError::WrongMeasurementIdentity { expected } => {
                format!("does not identify measurement contract {expected}; {REMEDIATION}")
            }
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
    engine_v2: Option<ResolvedProfileV2>,
    engine_v4: Option<ResolvedProfileSettingsV2>,
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
        engine_v2: None,
        engine_v4: None,
    })
}

fn load_with_config(path: &Path, config: &LoadedConfig) -> Result<LoadedInput, String> {
    let mut loaded = load_with_identity(path)?;
    let facts = loaded.source.source_facts();
    loaded.engine = config.resolve_engine_input(facts.format(), loaded.source.document())?;
    Ok(loaded)
}

/// Load an input for the current V2 lint path.  V1-only commands retain their
/// historical strict resolver and therefore cannot accidentally serialize a
/// bounded partial V2 setting prefix as V1 evidence.
fn load_with_config_v2(path: &Path, config: &LoadedConfig) -> Result<LoadedInput, String> {
    let mut loaded = load_with_identity(path)?;
    let facts = loaded.source.source_facts();
    loaded.engine_v2 = config.resolve_engine_input_v2(facts.format(), loaded.source.document())?;
    loaded.engine_v4 =
        config.resolve_engine_profile_v2_lint_input(facts.format(), loaded.source.document())?;
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
    Ok(LoadedInput {
        source,
        engine,
        engine_v2: None,
        engine_v4: None,
    })
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
    use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

    struct ConfigurationReferenceEntry {
        path: &'static str,
        authority: &'static str,
    }

    // Keep this inventory explicit and source-bound.  The Markdown parser
    // below checks rendered code spans, while the authority labels make a
    // newly added serde field or CLI setting require an intentional review of
    // this list instead of allowing a loose substring to satisfy the gate.
    const CONFIGURATION_REFERENCE_INVENTORY: &[ConfigurationReferenceEntry] = &[
        ConfigurationReferenceEntry {
            path: "Config::rig",
            authority: "Config::rig",
        },
        ConfigurationReferenceEntry {
            path: "Config::checks",
            authority: "Config::checks",
        },
        ConfigurationReferenceEntry {
            path: "Config::runtime_nodes",
            authority: "Config::runtime_nodes",
        },
        ConfigurationReferenceEntry {
            path: "Config::clips",
            authority: "Config::clips",
        },
        ConfigurationReferenceEntry {
            path: "Config::gait_groups",
            authority: "Config::gait_groups",
        },
        ConfigurationReferenceEntry {
            path: "Config::sync_groups",
            authority: "Config::sync_groups",
        },
        ConfigurationReferenceEntry {
            path: "rig.profile",
            authority: "RigConfig::profile",
        },
        ConfigurationReferenceEntry {
            path: "rig.roles.<role>",
            authority: "RigConfig::roles",
        },
        ConfigurationReferenceEntry {
            path: "rig.required_bones",
            authority: "RigConfig::required_bones",
        },
        ConfigurationReferenceEntry {
            path: "checks.<id>.severity",
            authority: "CheckSettings::severity",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-seam.max_ratio",
            authority: "CheckSettings::max_ratio",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-seam.min_stride_step_m",
            authority: "CheckSettings::min_stride_step_m",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-closure.max_position_delta_m",
            authority: "CheckSettings::max_position_delta_m",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-closure.max_rotation_delta_deg",
            authority: "CheckSettings::max_rotation_delta_deg",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-seam-vel.max_velocity_delta_mps",
            authority: "CheckSettings::max_velocity_delta_mps",
        },
        ConfigurationReferenceEntry {
            path: "checks.loop-seam-rot.max_angular_velocity_delta_degps",
            authority: "CheckSettings::max_angular_velocity_delta_degps",
        },
        ConfigurationReferenceEntry {
            path: "checks.frozen-bone.min_rotation_deg",
            authority: "CheckSettings::min_rotation_deg",
        },
        ConfigurationReferenceEntry {
            path: "checks.bind-pose.max_mean_rest_delta_deg",
            authority: "CheckSettings::max_mean_rest_delta_deg",
        },
        ConfigurationReferenceEntry {
            path: "checks.foot-slide.contact_height_m",
            authority: "CheckSettings::contact_height_m",
        },
        ConfigurationReferenceEntry {
            path: "checks.foot-slide.max_slide_mps",
            authority: "CheckSettings::max_slide_mps",
        },
        ConfigurationReferenceEntry {
            path: "checks.rest-world-scale.expected_uniform_scale",
            authority: "CheckSettings::expected_uniform_scale",
        },
        ConfigurationReferenceEntry {
            path: "checks.rest-world-scale.uniform_scale_tolerance",
            authority: "CheckSettings::uniform_scale_tolerance",
        },
        ConfigurationReferenceEntry {
            path: "checks.rest-world-scale.node_selectors",
            authority: "CheckSettings::node_selectors",
        },
        ConfigurationReferenceEntry {
            path: "runtime_nodes.selectors",
            authority: "RuntimeNodesConfig::selectors",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.loop",
            authority: "ClipExpectations::looping",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.max_loop_position_delta_m",
            authority: "ClipExpectations::max_loop_position_delta_m",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.max_loop_rotation_delta_deg",
            authority: "ClipExpectations::max_loop_rotation_delta_deg",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.max_loop_velocity_delta_mps",
            authority: "ClipExpectations::max_loop_velocity_delta_mps",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.max_loop_angular_velocity_delta_degps",
            authority: "ClipExpectations::max_loop_angular_velocity_delta_degps",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.duration_s",
            authority: "ClipExpectations::duration_s",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.speed_mps",
            authority: "ClipExpectations::speed_mps",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.movement_owner_xz",
            authority: "ClipExpectations::movement_owner_xz",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.movement_owner_y",
            authority: "ClipExpectations::movement_owner_y",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.movement_owner_yaw",
            authority: "ClipExpectations::movement_owner_yaw",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.in_place",
            authority: "ClipExpectations::in_place",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.fps",
            authority: "ClipExpectations::fps",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.animates_bones",
            authority: "ClipExpectations::animates_bones",
        },
        ConfigurationReferenceEntry {
            path: "gait_groups.<name>.clips",
            authority: "GaitGroup::clips",
        },
        ConfigurationReferenceEntry {
            path: "gait_groups.<name>.max_gait_phase_spread",
            authority: "GaitGroup::max_gait_phase_spread",
        },
        ConfigurationReferenceEntry {
            path: "gait_groups.<name>.min_lr_amplitude_m",
            authority: "GaitGroup::min_lr_amplitude_m",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.clips",
            authority: "SyncGroup::clips",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.max_duration_delta_s",
            authority: "SyncGroup::max_duration_delta_s",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.max_frame_count_delta",
            authority: "SyncGroup::max_frame_count_delta",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.max_fps_delta",
            authority: "SyncGroup::max_fps_delta",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.time_complement",
            authority: "SyncGroup::time_complement",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.time_complement.min_reflected_time_advantage",
            authority: "TimeComplementSettings::min_reflected_time_advantage",
        },
        ConfigurationReferenceEntry {
            path: "sync_groups.<name>.time_complement.min_lr_amplitude_m",
            authority: "TimeComplementSettings::min_lr_amplitude_m",
        },
        ConfigurationReferenceEntry {
            path: "engine.profile",
            authority: "EngineToml::profile",
        },
        ConfigurationReferenceEntry {
            path: "engine.profile_revision",
            authority: "EngineToml::profile_revision",
        },
        ConfigurationReferenceEntry {
            path: "engine.engine_version",
            authority: "EngineToml::engine_version",
        },
        ConfigurationReferenceEntry {
            path: "engine.importer",
            authority: "EngineToml::importer",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings",
            authority: "EngineToml::settings",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.convert_units",
            authority: "SettingId::ConvertUnits",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.bake_axis_conversion",
            authority: "SettingId::BakeAxisConversion",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.root_motion_source",
            authority: "SettingId(V2)::RootMotionSource",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.animation_type",
            authority: "SettingIdV2::AnimationType",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.avatar_setup",
            authority: "SettingIdV2::AvatarSetup",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.import_animation",
            authority: "SettingIdV2::ImportAnimation",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.root_rotation",
            authority: "SettingIdV2::RootRotation",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.root_position_y",
            authority: "SettingIdV2::RootPositionY",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.root_position_xz",
            authority: "SettingIdV2::RootPositionXz",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.rotate_scene_entity",
            authority: "SettingIdV2::RotateSceneEntity",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.rotate_meshes",
            authority: "SettingIdV2::RotateMeshes",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.load_meshes",
            authority: "SettingIdV2::LoadMeshes",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.extension_handler_environment",
            authority: "SettingIdV2::ExtensionHandlerEnvironment",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.bevy_animation_feature",
            authority: "SettingIdV2::BevyAnimationFeature",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.load_animations",
            authority: "SettingIdV2::LoadAnimations",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.animation_fps",
            authority: "SettingIdV2::AnimationFps",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.animation_trimming",
            authority: "SettingIdV2::AnimationTrimming",
        },
        ConfigurationReferenceEntry {
            path: "engine.settings.sample_rate",
            authority: "SettingIdV2::SampleRate",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.engine_settings.root_rotation",
            authority: "engine_setting_map(_v2)",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.engine_settings.root_position_y",
            authority: "engine_setting_map(_v2)",
        },
        ConfigurationReferenceEntry {
            path: "clips.<selector>.engine_settings.root_position_xz",
            authority: "engine_setting_map(_v2)",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>",
            authority: "DocumentConfigWire::transition_families",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.schema",
            authority: "DocumentFamilyWire::schema",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.schema_version",
            authority: "DocumentFamilyWire::schema_version",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.scope",
            authority: "DocumentFamilyWire::scope",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.boundary",
            authority: "DocumentFamilyWire::boundary",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.basis",
            authority: "DocumentFamilyWire::basis",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.basis.translation",
            authority: "BasisWire::translation",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.basis.rotation",
            authority: "BasisWire::rotation",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.basis.time",
            authority: "BasisWire::time",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.tolerances",
            authority: "DocumentFamilyWire::tolerances",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.tolerances.translation_m",
            authority: "TolerancesWire::translation_m",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.tolerances.rotation_deg",
            authority: "TolerancesWire::rotation_deg",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.tolerances.time_normalized",
            authority: "TolerancesWire::time_normalized",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.members",
            authority: "DocumentFamilyWire::members",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.members.take_index",
            authority: "DocumentMemberWire::take_index",
        },
        ConfigurationReferenceEntry {
            path: "transition_families.<id>.members.take_name",
            authority: "DocumentMemberWire::take_name",
        },
    ];

    fn configuration_reference_code_tokens(markdown: &str) -> BTreeSet<String> {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        Parser::new_ext(markdown, options)
            .filter_map(|event| match event {
                Event::Code(code) => Some(code.to_string()),
                _ => None,
            })
            .collect()
    }

    fn missing_configuration_reference_entries(
        markdown: &str,
    ) -> Vec<&'static ConfigurationReferenceEntry> {
        let tokens = configuration_reference_code_tokens(markdown);
        CONFIGURATION_REFERENCE_INVENTORY
            .iter()
            .filter(|entry| !tokens.contains(entry.path))
            .collect()
    }

    fn document_transition_family() -> &'static str {
        r#"
[transition_families."walk_to_run"]
schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "document"
boundary = "both"

[transition_families."walk_to_run".basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"

[transition_families."walk_to_run".tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.0

[[transition_families."walk_to_run".members]]
take_index = 0
take_name = "Walk"

[[transition_families."walk_to_run".members]]
take_index = 1
take_name = "Run"
"#
    }

    #[test]
    fn config_admits_transition_families_with_engine_clips_and_other_fields() {
        let source = format!(
            r#"
[engine]
profile = "unity-generic"
profile_revision = 1
engine_version = "6000.3"
importer = "fbx-model-importer"

[clips.walk]
loop = true

[runtime_nodes]
selectors = ["root"]
{}
"#,
            document_transition_family()
        );
        let (config, engine, transition_families) = parse_config(source.as_bytes()).unwrap();
        assert!(config.clips.contains_key("walk"));
        assert!(config.runtime_node_selectors().is_some());
        assert!(matches!(
            engine,
            ParsedEngineDeclaration::V1(EngineDeclaration {
                selection: Some(_),
                ..
            })
        ));
        assert_eq!(
            transition_families.source_identity(),
            &InputIdentity::from_bytes(source.as_bytes())
        );
        assert_eq!(
            transition_families
                .declaration()
                .document_families()
                .expect("document declaration")
                .len(),
            1
        );
    }

    #[test]
    fn config_retains_empty_document_declarations_but_no_file_has_none() {
        assert!(LoadedConfig::without_file().transition_families.is_none());
        let directory = tempfile::tempdir().unwrap();
        for (index, source) in ["[rig]\nprofile = \"auto\"\n", "[transition_families]\n"]
            .into_iter()
            .enumerate()
        {
            let path = directory.path().join(format!("config-{index}.toml"));
            std::fs::write(&path, source).unwrap();
            let loaded = load_config_with_source(Some(&path)).unwrap();
            let transition_families = loaded.transition_families.expect("config input retained");
            assert_eq!(
                transition_families.source_identity(),
                &InputIdentity::from_bytes(source.as_bytes())
            );
            assert!(
                transition_families
                    .declaration()
                    .document_families()
                    .expect("document declaration")
                    .is_empty()
            );
        }
    }

    #[test]
    fn config_loader_applies_the_exact_raw_source_bound_before_utf8_or_toml() {
        let directory = tempfile::tempdir().unwrap();
        let exact_path = directory.path().join("exact.toml");
        let exact = vec![b' '; TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize];
        std::fs::write(&exact_path, &exact).unwrap();
        let exact_loaded = load_config_with_source(Some(&exact_path)).unwrap();
        assert_eq!(
            exact_loaded
                .transition_families
                .expect("config input retained")
                .source_identity(),
            &InputIdentity::from_bytes(&exact)
        );

        let over_path = directory.path().join("over.toml");
        let over = vec![0xff; TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES as usize + 1];
        std::fs::write(&over_path, over).unwrap();
        let over_error = match load_config_with_source(Some(&over_path)) {
            Ok(_) => panic!("N+1 config source must be refused"),
            Err(error) => error,
        };
        assert!(over_error.contains("transition-family-too-large"));
        assert!(!over_error.contains("transition-family-encoding"));

        let invalid_utf8_path = directory.path().join("invalid-utf8.toml");
        std::fs::write(&invalid_utf8_path, b"[rig]\nprofile = \"auto\"\n\xff").unwrap();
        let invalid_utf8_error = match load_config_with_source(Some(&invalid_utf8_path)) {
            Ok(_) => panic!("invalid UTF-8 config source must be refused"),
            Err(error) => error,
        };
        assert!(invalid_utf8_error.contains("transition-family-encoding"));
        assert!(!invalid_utf8_error.contains("config is not UTF-8"));
    }

    #[test]
    fn config_refuses_invalid_transition_family_tables_before_generic_config_decode() {
        let unknown = document_transition_family().replace("boundary = \"both\"", "extra = true");
        assert!(
            parse_config(unknown.as_bytes())
                .unwrap_err()
                .starts_with("transition-family declaration control error")
        );
        let duplicate = format!("{}\nboundary = \"both\"\n", document_transition_family());
        assert!(
            parse_config(duplicate.as_bytes())
                .unwrap_err()
                .starts_with("transition-family declaration control error")
        );
        let malformed =
            document_transition_family().replace("scope = \"document\"", "scope = \"collection\"");
        assert!(
            parse_config(malformed.as_bytes())
                .unwrap_err()
                .starts_with("transition-family declaration control error")
        );
    }

    #[test]
    fn generic_config_deserialization_still_rejects_transition_family_tables() {
        assert!(toml::from_str::<Config>(document_transition_family()).is_err());
    }

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
        let (core, declaration, _) = parse_config(text.as_bytes()).unwrap();
        assert_eq!(
            core.clips.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["*", "walk*", "walk_forward"]
        );
        let ParsedEngineDeclaration::V1(declaration) = declaration else {
            panic!("revision 1 config must use the V1 declaration")
        };
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
    fn cli_engine_toml_maps_revision_2_values_to_the_closed_public_resolver() {
        let text = r#"
[engine]
profile = "bevy"
profile_revision = 2
engine_version = "0.19.0"
importer = "gltf-asset-loader"

[engine.settings]
extension_handler_environment = "bevy_pbr_stock_0_19"
bevy_animation_feature = true
load_meshes = "empty"
"#;
        let (_, declaration, _) = parse_config(text.as_bytes()).unwrap();
        let ParsedEngineDeclaration::V2(declaration) = declaration else {
            panic!("revision 2 config must use the V2 declaration")
        };
        let resolved = animsmith_engine::resolve_static_v2(declaration)
            .unwrap()
            .unwrap()
            .resolve_input(animsmith_core::SourceFormatV1::Glb)
            .unwrap();
        assert_eq!(resolved.profile().selection().profile_revision(), 2);
        assert!(resolved.document_settings().values().any(|setting| {
            setting.value()
                == &SettingValueV2::HandlerEnvironment(
                    BevyGltfHandlerEnvironmentV2::BevyPbrStock019,
                )
        }));
        assert!(resolved.document_settings().values().any(|setting| {
            setting.value() == &SettingValueV2::LoadMeshesState(BevyLoadMeshesStateV2::Empty)
        }));

        let bad = text.replace(
            "bevy_pbr_stock_0_19",
            "unbounded_application_handler_registry",
        );
        assert!(
            parse_config(bad.as_bytes())
                .unwrap_err()
                .contains("invalid revision-2 engine setting value")
        );
    }

    #[test]
    fn cli_engine_toml_maps_godot_and_unreal_advice_values_without_aliases() {
        let godot = r#"
[engine]
profile = "godot"
profile_revision = 2
engine_version = "4.7"
importer = "resource-importer-scene"

[engine.settings]
animation_fps = 120
animation_trimming = true
"#;
        let (_, declaration, _) = parse_config(godot.as_bytes()).unwrap();
        let ParsedEngineDeclaration::V2(declaration) = declaration else {
            panic!("Godot revision 2 must use the V2 declaration")
        };
        let resolved = animsmith_engine::resolve_static_v2(declaration)
            .unwrap()
            .unwrap();
        assert!(
            resolved
                .document_settings()
                .values()
                .any(|setting| { setting.value() == &SettingValueV2::PositiveInteger(120) })
        );

        for (spelling, expected) in [
            ("default_30", UnrealSampleRateV2::Default30),
            ("source_determined", UnrealSampleRateV2::SourceDetermined),
            ("custom_hz(48000)", UnrealSampleRateV2::CustomHz(48_000)),
        ] {
            let unreal = format!(
                r#"
[engine]
profile = "unreal"
profile_revision = 2
engine_version = "5.8"
importer = "fbx-importer"

[engine.settings]
sample_rate = "{spelling}"
"#
            );
            let (_, declaration, _) = parse_config(unreal.as_bytes()).unwrap();
            let ParsedEngineDeclaration::V2(declaration) = declaration else {
                panic!("Unreal revision 2 must use the V2 declaration")
            };
            let resolved = animsmith_engine::resolve_static_v2(declaration)
                .unwrap()
                .unwrap();
            assert!(
                resolved
                    .document_settings()
                    .values()
                    .any(|setting| { setting.value() == &SettingValueV2::SampleRate(expected) })
            );
        }

        for rejected in [
            "default30",
            "custom_hz(0",
            "custom_hz(01)",
            "custom_hz(+1)",
            "custom_hz(1)extra",
            "CUSTOM_HZ(60)",
        ] {
            assert!(
                unreal_sample_rate(rejected).is_none(),
                "accepted {rejected}"
            );
        }
    }

    #[test]
    fn cli_engine_toml_rejects_incomplete_selection_and_source_unit_escape_hatch() {
        let incomplete = parse_config(
            r#"
[engine]
profile = "bevy"
"#
            .as_bytes(),
        )
        .unwrap_err();
        assert!(incomplete.contains("requires profile, profile_revision"));

        let source_unit = parse_config(b"source_unit = \"metre\"").unwrap_err();
        assert!(source_unit.contains("unknown field `source_unit`"));
    }

    #[test]
    fn cli_clip_engine_settings_without_selection_reach_the_typed_error() {
        let (_, declaration, _) = parse_config(
            r#"
[clips.walk]
[clips.walk.engine_settings]
root_rotation = "bake"
"#
            .as_bytes(),
        )
        .unwrap();
        let ParsedEngineDeclaration::V1(declaration) = declaration else {
            panic!("settings without a selection retain the V1 error boundary")
        };
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

    #[test]
    fn configuration_reference_examples_and_inventory_are_parser_checked() {
        let source = r#"
[rig]
profile = "mixamo"
roles = { hips = "Hips" }
required_bones = ["weapon_socket"]
[checks.loop-seam]
severity = "warn"
max_ratio = 1.5
min_stride_step_m = 0.02
[checks.loop-closure]
max_position_delta_m = 0.01
max_rotation_delta_deg = 1.0
[checks.loop-seam-vel]
max_velocity_delta_mps = 0.1
[checks.loop-seam-rot]
max_angular_velocity_delta_degps = 5.0
[checks.frozen-bone]
min_rotation_deg = 1.0
[checks.bind-pose]
max_mean_rest_delta_deg = 45.0
[checks.foot-slide]
contact_height_m = 0.03
max_slide_mps = 0.3
[checks.rest-world-scale]
expected_uniform_scale = 1.0
uniform_scale_tolerance = 0.0001
[runtime_nodes]
selectors = ["weapon_socket", "ik_*"]
[clips."run_*"]
loop = true
max_loop_position_delta_m = 0.04
max_loop_rotation_delta_deg = 2.0
max_loop_velocity_delta_mps = 0.2
max_loop_angular_velocity_delta_degps = 200.0
duration_s = { value = 1.0, tolerance = 0.02 }
speed_mps = { value = 2.0, tolerance = 0.2 }
movement_owner_xz = "animation"
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"
fps = 30.0
animates_bones = ["Hips"]
[gait_groups.ring]
clips = ["run_forward", "run_back"]
max_gait_phase_spread = 0.15
min_lr_amplitude_m = 0.03
[sync_groups.ring]
clips = ["run_forward", "run_back"]
max_duration_delta_s = 0.001
max_frame_count_delta = 0
max_fps_delta = 0.01
[sync_groups.ring.time_complement]
min_reflected_time_advantage = 0.25
min_lr_amplitude_m = 0.03
"#;
        let config: Config = toml::from_str(source).expect("complete core config parses");
        config.validate().expect("complete core config validates");
        assert_eq!(config.rig.profile, "mixamo");
        assert_eq!(
            config.rig.roles.get(&animsmith_core::Role::Hips),
            Some(&"Hips".to_owned())
        );
        assert_eq!(
            config.runtime_node_selectors().unwrap().selectors(),
            ["weapon_socket", "ik_*"]
        );
        assert_eq!(config.expectations_for("run_forward").fps, Some(30.0));
        assert_eq!(config.gait_groups["ring"].max_gait_phase_spread, 0.15);
        assert_eq!(config.sync_groups["ring"].max_frame_count_delta, 0);
        assert!(toml::from_str::<Config>("[clips.walk]\nunknown = true\n").is_err());
        parse_config(document_transition_family().as_bytes())
            .expect("transition-family reference shape parses");

        let Some((workspace_root, docs)) = read_source_configuration_reference() else {
            // Repository documentation and examples are intentionally absent
            // from published package sources.
            return;
        };
        let missing = missing_configuration_reference_entries(&docs);
        assert!(
            missing.is_empty(),
            "configuration reference is missing exact documented paths: {}",
            missing
                .iter()
                .map(|entry| format!("{} ({})", entry.path, entry.authority))
                .collect::<Vec<_>>()
                .join(", ")
        );
        for entry in std::fs::read_dir(workspace_root.join("examples"))
            .expect("examples directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().to_string_lossy().ends_with(".animsmith.toml"))
        {
            let path = entry.path();
            let bytes = std::fs::read(&path).expect("example readable");
            parse_config(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        }
    }

    #[test]
    fn configuration_reference_inventory_detects_removed_nested_paths() {
        let Some((_workspace_root, docs)) = read_source_configuration_reference() else {
            return;
        };
        let mutated = docs.replace("`runtime_nodes.selectors`", "runtime_nodes.selectors");
        let missing = missing_configuration_reference_entries(&mutated);
        assert_eq!(
            missing.iter().map(|entry| entry.path).collect::<Vec<_>>(),
            vec!["runtime_nodes.selectors"],
            "removing one exact nested code span must fail the maintenance inventory"
        );

        let mutated = docs.replace(
            "`transition_families.<id>.tolerances.translation_m`",
            "transition_families.<id>.tolerances.translation_m",
        );
        let missing = missing_configuration_reference_entries(&mutated);
        assert_eq!(
            missing.iter().map(|entry| entry.path).collect::<Vec<_>>(),
            vec!["transition_families.<id>.tolerances.translation_m"],
            "removing a transition-family nested path must fail the maintenance inventory"
        );
    }

    #[test]
    fn configuration_reference_enablement_wording_matches_check_authority() {
        let Some((_workspace_root, docs)) = read_source_configuration_reference() else {
            return;
        };
        assert_enablement_wording(&docs);
    }

    #[test]
    fn configuration_reference_rejects_opt_in_time_complement_wording() {
        let Some((_workspace_root, docs)) = read_source_configuration_reference() else {
            return;
        };
        let mutated = docs.replace(
            "`time-complement` is enabled by default",
            "opt-in `time-complement`",
        );
        assert!(
            std::panic::catch_unwind(|| assert_enablement_wording(&mutated)).is_err(),
            "opt-in time-complement wording must fail the maintenance test"
        );
    }

    #[test]
    fn source_workspace_detection_distinguishes_a_packaged_layout() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        if manifest_dir.join(".cargo_vcs_info.json").is_file() {
            assert_eq!(source_workspace_root(manifest_dir), None);
            assert_eq!(read_source_configuration_reference_from(manifest_dir), None);
            return;
        }
        assert!(
            source_workspace_root(manifest_dir).is_some(),
            "the source checkout must be recognized as the documentation authority"
        );

        let fixture = tempfile::tempdir().expect("create package fixture");
        let package_root = fixture.path().join("crates/animsmith");
        std::fs::create_dir_all(&package_root).expect("create package source layout");
        std::fs::write(
            package_root.join("Cargo.toml"),
            "[package]\nname = \"animsmith\"\n",
        )
        .expect("write package manifest");
        assert!(
            source_workspace_root(&package_root).is_some(),
            "the synthetic layout must satisfy every source-checkout condition before marking it packaged"
        );
        std::fs::write(package_root.join(".cargo_vcs_info.json"), "{}\n")
            .expect("write package marker");
        assert_eq!(source_workspace_root(&package_root), None);
        assert_eq!(
            read_source_configuration_reference_from(&package_root),
            None
        );
    }

    fn read_workspace_doc(workspace_root: &Path, relative_path: &str) -> String {
        let path = workspace_root.join(relative_path);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    }

    fn read_source_configuration_reference() -> Option<(PathBuf, String)> {
        read_source_configuration_reference_from(Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    fn read_source_configuration_reference_from(manifest_dir: &Path) -> Option<(PathBuf, String)> {
        let workspace_root = source_workspace_root(manifest_dir)?;
        let docs = read_workspace_doc(&workspace_root, "docs/configuration-reference.md");
        Some((workspace_root, docs))
    }

    fn source_workspace_root(manifest_dir: &Path) -> Option<PathBuf> {
        if manifest_dir.join(".cargo_vcs_info.json").is_file() {
            return None;
        }
        let workspace_root = manifest_dir.join("../..");
        let current_manifest = manifest_dir.join("Cargo.toml").canonicalize().ok()?;
        let workspace_manifest = workspace_root
            .join("crates/animsmith/Cargo.toml")
            .canonicalize()
            .ok()?;
        (current_manifest == workspace_manifest).then_some(workspace_root)
    }

    fn assert_enablement_wording(markdown: &str) {
        let mut options = Options::empty();
        options.insert(pulldown_cmark::Options::ENABLE_TABLES);
        let mut in_checks_section = false;
        let mut in_heading = false;
        let mut in_paragraph = false;
        let mut heading = String::new();
        let mut paragraph = String::new();
        let mut paragraphs = Vec::new();
        for event in Parser::new_ext(markdown, options) {
            match event {
                Event::Start(Tag::Heading {
                    level: HeadingLevel::H2,
                    ..
                }) => {
                    in_heading = true;
                    heading.clear();
                }
                Event::End(TagEnd::Heading(HeadingLevel::H2)) => {
                    in_heading = false;
                    if in_checks_section {
                        break;
                    }
                    in_checks_section = heading == "Checks and severity";
                }
                Event::Start(Tag::Paragraph) if in_checks_section => {
                    in_paragraph = true;
                    paragraph.clear();
                }
                Event::End(TagEnd::Paragraph) if in_paragraph => {
                    paragraphs.push(paragraph.split_whitespace().collect::<Vec<_>>().join(" "));
                    in_paragraph = false;
                }
                Event::Text(text) | Event::Code(text) => {
                    if in_heading {
                        heading.push_str(&text);
                    } else if in_paragraph {
                        if !paragraph.is_empty() {
                            paragraph.push(' ');
                        }
                        paragraph.push_str(&text);
                    }
                }
                _ => {}
            }
        }
        let paragraph = paragraphs
            .iter()
            .find(|paragraph| paragraph.contains("time-complement"))
            .expect("checks section must explain time-complement");
        let opt_in_ids = all_checks()
            .into_iter()
            .filter(|check| !check.enabled_by_default())
            .map(|check| check.id().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            opt_in_ids,
            vec!["constant-nonunit-scale"],
            "built-in opt-in authority changed; update the reference wording deliberately"
        );
        let expected_opt_in = format!(
            "all built-ins are enabled by default except opt-in {}",
            opt_in_ids.join(", ")
        );
        assert!(
            paragraph.contains(&expected_opt_in),
            "checks section must name exactly the implementation-authoritative opt-in set"
        );
        assert!(paragraph.contains("time-complement is enabled by default"));
        assert!(paragraph.contains("NotApplicable") || paragraph.contains("not applicable"));
        assert!(
            !paragraph.contains("opt-in time-complement")
                && !paragraph.contains("time-complement is opt-in"),
            "time-complement must not be opt-in"
        );
    }
}
