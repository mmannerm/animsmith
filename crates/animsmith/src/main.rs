//! The animsmith CLI binary.
//!
//! This crate publishes the `animsmith` command: inspect, measure, lint,
//! report, transform, fix, convert, assemble, scale, and diff skeletal
//! animation clips. It
//! is not the Rust library API; use `animsmith-core` plus the loader
//! crates (`animsmith-gltf`, `animsmith-fbx`) and `animsmith-report` from
//! library code.
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
    CheckCtx, CheckSelection, Config, DiffEnvelope, LintEnvelope, LintFileReport, MeasureEnvelope,
    MeasureFileReport, MeasurementContract, MeasurementFileError, MeasurementReportError,
    MeasurementReportInput, MetricGrids, RigInfo, Severity, ToolInfo, ToolSource, all_checks,
    evaluate_checks, resolve_configured_roles,
};
use animsmith_core::{Document, InputIdentity};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "fbx")]
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(feature = "fbx")]
mod assembly;
#[cfg(feature = "fbx")]
mod material_recipe;
mod publish;
mod render;
mod scale;
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
    /// Config file (defaults to ./animsmith.toml when present).
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
        #[arg(long, value_enum, default_value_t = LintFormat::Text)]
        format: LintFormat,
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
        long_about = "Convert FBX or glTF input to glTF: skeleton, animation, triangulated meshes, skins, PBR materials, and embedded PNG/JPEG base-color, normal, metallic-roughness, and occlusion textures. A glTF input is re-emitted carrying its geometry; --animation-only drops it. --material-texture-recipe applies exact, declarative BaseColor, normal, metallic-roughness, and occlusion textures. --bake-static-mesh-transforms produces a strict canonical static scene whose mesh-local geometry includes accumulated rest transforms. Output format by extension: .glb binary, .gltf JSON with an embedded buffer."
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
        long_about = "Assemble one runtime GLB from an authoritative skinned base and animation takes supplied by FBX or glTF inputs. The versioned recipe owns exact mesh selection, skeleton remapping, clip windows and mechanical transforms. Recipe v4 can opt into glTF-only rest/bind scale canonicalization with explicit source selectors and expected factor; it validates the base and every clip basis before copying keys. Source extraction, project policy, and publication remain consumer responsibilities."
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
        long_about = "Rewrite one self-contained glTF/GLB asset's declared linear scale on its own raw bytes and publish the artifact and its versioned evidence as one atomic pair. `whole-document` converts every represented length by a declared factor; `rest-bind` removes one compensating inherited scale from a selected skinned hierarchy. Every factor and source selector is required: nothing is inferred from bounds, height, joint lengths, inverse-bind magnitude, filename, or asset category, there is no in-place mode, and the tolerance policy is fixed and recorded rather than exposed as a flag. Input, output, and evidence paths must be distinct. A refusal publishes nothing, leaves any prior pair byte-identical, and exits 1; an operator error exits 2."
    )]
    Scale {
        #[command(subcommand)]
        operation: ScaleCmd,
    },
    /// Compare animation measurements.
    #[command(
        long_about = "Compare the measurements of two inputs (asset files or one-file output-v7 `measure` or `lint` JSON carrying measurements-v13) and report movement beyond significance thresholds. Exits 1 on significant movement."
    )]
    Diff {
        /// Before input: asset file or one-file output-v7 `measure`/`lint` JSON report.
        a: PathBuf,
        /// After input: asset file or one-file output-v7 `measure`/`lint` JSON report.
        b: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
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
        long_about = "Reparameterize a restricted skinned rest/bind hierarchy so a compensating inherited scale is removed while world joint translations and orientations, sampled trajectories, and skinned vertex positions are preserved. Both source selectors are required raw source-array indices, and the expected common factor is declared and checked against the source rather than inferred."
    )]
    RestBind {
        /// Input .glb or .gltf file.
        input: PathBuf,
        /// Output path; must keep the input's container extension.
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

/// Output format for `lint`. Adds a presentation-only Markdown rendering
/// on top of the shared text/JSON surface, suitable for pasting into CI
/// comments and asset-review threads. JSON stays the machine-readable
/// source of truth; Markdown carries no schema or stability guarantees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LintFormat {
    Text,
    Json,
    Markdown,
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
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprint!("{}", render::render_operator_error(&message));
            ExitCode::from(EXIT_OPERATOR)
        }
    }
}

struct LoadedConfig {
    config: Config,
    #[cfg(feature = "fbx")]
    source: Option<LoadedConfigSource>,
}

#[cfg(feature = "fbx")]
struct LoadedConfigSource {
    path: PathBuf,
    bytes: Vec<u8>,
}

fn load_config(explicit: Option<&Path>) -> Result<Config, String> {
    Ok(load_config_with_source(explicit)?.config)
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
            #[cfg(feature = "fbx")]
            source: None,
        });
    };
    let bytes =
        std::fs::read(&path).map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|e| format!("bad config {}: config is not UTF-8: {e}", path.display()))?;
    let config = toml::from_str(text).map_err(|e| format!("bad config {}: {e}", path.display()))?;
    Ok(LoadedConfig {
        config,
        #[cfg(feature = "fbx")]
        source: Some(LoadedConfigSource { path, bytes }),
    })
}

fn validate_check_selection(
    checks: &[Box<dyn animsmith_core::Check>],
    select: &[String],
) -> Result<(), String> {
    // Frontend validation intentionally runs before loading any input file, so
    // a bad CLI selection has one deterministic operator error. Core repeats
    // the invariant for embedded callers that invoke `evaluate_checks`
    // directly; the two boundaries serve different consumers.
    let known: Vec<&str> = checks.iter().map(|check| check.id()).collect();
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

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.cmd {
        Cmd::Inspect { file } => {
            let config = load_config(cli.config.as_deref())?;
            let doc = load(&file)?;
            let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
            publish::emit_text_lines(render::render_inspect(&doc, &roles));
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            let config = load_config(cli.config.as_deref())?;
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let (doc, input) = load_with_identity(file)?;
                let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                let grids = MetricGrids::new(&doc);
                reports.push(MeasureFileReport::new(
                    file.display().to_string(),
                    input,
                    RigInfo::from_resolved(&doc, &roles).map_err(|error| error.to_string())?,
                    MeasurementContract::new(
                        animsmith_core::measure::measure_document(&grids, &roles, &config),
                        animsmith_core::measure::measure_assets(&doc),
                    )
                    .map_err(|error| error.to_string())?,
                ));
            }
            match format {
                Format::Json => {
                    let envelope = MeasureEnvelope::new(current_tool(), reports);
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
            let config = load_config(cli.config.as_deref())?;
            require_files(&files)?;
            let checks = all_checks();
            validate_check_selection(&checks, &select)?;
            if format == LintFormat::Json && !allow.is_empty() {
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
            let mut reports = Vec::new();
            let mut worst = Severity::Note;
            for file in &files {
                let (doc, input) = load_with_identity(file)?;
                let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                let grids = MetricGrids::new(&doc);
                let ctx = CheckCtx::new(&grids, &roles, &config);
                let evaluations =
                    evaluate_checks(&ctx, &checks, selection).map_err(|error| error.to_string())?;
                for finding in evaluations
                    .iter()
                    .flat_map(|check| check.findings())
                    .filter(|finding| !allow.iter().any(|id| id == finding.check_id))
                {
                    worst = worst.max(finding.severity);
                }
                reports.push(LintFileReport::new(
                    file.display().to_string(),
                    input,
                    RigInfo::from_resolved(&doc, &roles).map_err(|error| error.to_string())?,
                    evaluations,
                    MeasurementContract::new(
                        animsmith_core::measure::measure_document(&grids, &roles, &config),
                        animsmith_core::measure::measure_assets(&doc),
                    )
                    .map_err(|error| error.to_string())?,
                ));
            }
            match format {
                LintFormat::Json => {
                    let envelope = LintEnvelope::new(current_tool(), reports);
                    render::print_json(&envelope)?;
                }
                LintFormat::Text => publish::emit_text(&render::render_text(&reports, &allow)),
                LintFormat::Markdown => {
                    publish::emit_text(&render::render_markdown(&reports, &allow));
                }
            }
            let fail_at = if deny_warnings {
                Severity::Warning
            } else {
                Severity::Error
            };
            Ok(if worst >= fail_at {
                ExitCode::from(EXIT_FINDINGS)
            } else {
                ExitCode::SUCCESS
            })
        }
        #[cfg(feature = "report")]
        Cmd::Report { file, output, clip } => {
            let config = load_config(cli.config.as_deref())?;
            let doc = load(&file)?;
            let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
            let grids = MetricGrids::new(&doc);
            let ctx = CheckCtx::new(&grids, &roles, &config);
            let findings: Vec<_> = evaluate_checks(&ctx, &all_checks(), CheckSelection::All)
                .map_err(|error| error.to_string())?
                .into_iter()
                .flat_map(|check| check.findings().to_vec())
                .collect();
            let html = animsmith_report::render(&grids, &roles, &findings, clip.as_deref());
            std::fs::write(&output, &html)
                .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
            publish::emit_text(&render::render_report_written(
                &output,
                doc.clips.len(),
                findings.len(),
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
            let config = load_config(cli.config.as_deref())?;
            let mut doc = load(&input)?;
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
        } => {
            let mut doc = load(&input)?;
            // `--animation-only` clears assets uniformly across formats:
            // this is where a conversion drops its geometry on request.
            if animation_only {
                doc.assets = animsmith_core::model::SceneAssets::default();
            }
            let recipe_application = material_texture_recipe
                .as_deref()
                .map(|path| material_recipe::apply_material_texture_recipe(path, &doc))
                .transpose()
                .map_err(|error| error.to_string())?;
            let recipe_doc = recipe_application
                .as_ref()
                .map_or(&doc, |application| &application.document);
            let static_mesh_bake = if bake_static_mesh_transforms {
                Some(
                    animsmith_core::bake_static_mesh_transforms(recipe_doc)
                        .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            let output_doc = static_mesh_bake
                .as_ref()
                .map_or(recipe_doc, |bake| &bake.document);
            let summary =
                animsmith_gltf::write::write(output_doc, &output).map_err(|e| e.to_string())?;
            match format {
                Format::Text => {
                    let transcript = std::iter::once(render::render_write_summary(
                        &output, &summary,
                    ))
                    .chain(static_mesh_bake.as_ref().map(|bake| {
                        format!(
                            "baked {} static mesh instance(s) into identity-root geometry\n",
                            bake.evidence.entries.len(),
                        )
                    }))
                    .chain(recipe_application.as_ref().map(|application| {
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
                    tool: current_tool(),
                    command: "convert",
                    input: input.display().to_string(),
                    output: output.display().to_string(),
                    options: ConversionOptions {
                        animation_only,
                        bake_static_mesh_transforms,
                        material_texture_recipe: material_texture_recipe
                            .as_ref()
                            .map(|path| path.display().to_string()),
                    },
                    artifact: summary.into(),
                    static_mesh_bake: static_mesh_bake.as_ref().map(|bake| &bake.evidence),
                    material_texture_recipe: recipe_application
                        .as_ref()
                        .map(|application| &application.evidence),
                })?,
            }
            Ok(ExitCode::SUCCESS)
        }
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
/// output-v7 `measure`/`lint` JSON report carrying measurements-v13.
fn load_measurements(
    path: &Path,
    config: &Config,
) -> Result<BTreeMap<String, animsmith_core::measure::ClipMeasurements>, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "json" {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        // Correctly rounded parsing is required by glTF raw-value proof. Keep
        // the released measurement diagnostic by first retaining any finite
        // JSON number in the generic representation, then decoding typed
        // fields; a value outside `f32` reaches contract validation as
        // non-finite evidence instead of becoming a parser error.
        let value = serde_json::from_str(&text)
            .map_err(|e| format!("bad JSON in {}: {e}", path.display()))?;
        let report: MeasurementReportInput = serde_json::from_value(value)
            .map_err(|e| format!("bad JSON in {}: {e}", path.display()))?;
        // Only the current output-v7 envelope with measurement contract v13 is
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
    let doc = load(path)?;
    let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
    let grids = MetricGrids::new(&doc);
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

fn load_bytes(path: &Path, format: InputFormat, bytes: &[u8]) -> Result<Document, String> {
    match format {
        InputFormat::Gltf => {
            animsmith_gltf::load_bytes(path, bytes).map_err(|error| error.to_string())
        }
        #[cfg(feature = "fbx")]
        InputFormat::Fbx => {
            animsmith_fbx::load_bytes(path, bytes).map_err(|error| error.to_string())
        }
    }
}

fn load(path: &Path) -> Result<Document, String> {
    let (format, bytes) = capture_input(path)?;
    load_bytes(path, format, &bytes)
}

/// Read one primary input once, derive its retained-evidence identity from
/// those exact bytes, and parse that same byte slice. This deliberately does
/// not identify a reopened path: a report must describe the bytes judged.
fn load_with_identity(path: &Path) -> Result<(Document, InputIdentity), String> {
    let (format, bytes) = capture_input(path)?;
    let input = InputIdentity::from_bytes(&bytes);
    let doc = load_bytes(path, format, &bytes)?;
    Ok((doc, input))
}

#[cfg(test)]
mod tests {
    use super::*;

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
