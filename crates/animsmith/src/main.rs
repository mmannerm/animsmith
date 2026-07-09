//! The animsmith CLI binary.
//!
//! This crate publishes the `animsmith` command: inspect, measure, lint,
//! report, transform, fix, convert, and diff skeletal animation clips. It
//! is not the Rust embedding API; use `animsmith-core` plus the loader
//! crates (`animsmith-gltf`, `animsmith-fbx`) and `animsmith-report` from
//! library code.
//!
//! Feature gates mirror the installed binary surface. The default build
//! includes FBX input and HTML reports; `--no-default-features` leaves a
//! pure-Rust glTF-only binary with report generation and FBX conversion
//! omitted.

#![warn(missing_docs)]

use animsmith_core::model::Document;
use animsmith_core::profile::{ResolvedRoles, resolve_named};
use animsmith_core::{CheckCtx, Config, Finding, MetricGrids, Severity, all_checks, run_checks};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Exit codes, matching common asset-validation gate conventions:
/// 0 = clean or warnings-only, 1 = error findings, 2 = operator error.
const EXIT_FINDINGS: u8 = 1;
const EXIT_OPERATOR: u8 = 2;

/// Version of the first-published machine-readable output schema,
/// bumped on breaking JSON changes after that contract is released.
const SCHEMA_VERSION: u32 = 1;
const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/mmannerm/animsmith/main/docs/schemas/output-v1.schema.json";

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
    /// Summarize a file: skeleton, clips, tracks, detected rig profile.
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
        long_about = "Apply pipeline-mechanical clip transforms and write the result as glTF, carrying through any geometry the input brought (FBX or glTF meshes/skins/materials). Operations apply to every clip, or one clip via --clip."
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
        /// Rotate cyclic clips so the measured stride anchor lands at
        /// t=0 (needs hips+feet rig roles).
        #[arg(long)]
        gait_anchor: bool,
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
        long_about = "Convert FBX or glTF input to glTF: skeleton, animation, triangulated meshes, skins, and factor-only materials. A glTF input is re-emitted carrying its geometry; --animation-only drops it. Texture wiring stays a downstream concern. Output format by extension: .glb binary, .gltf JSON with an embedded buffer."
    )]
    #[cfg(feature = "fbx")]
    Convert {
        /// Input .fbx, .glb, or .gltf file.
        input: PathBuf,
        /// Output .glb or .gltf path.
        #[arg(short, long)]
        output: PathBuf,
        /// Strip geometry: emit skeleton + animation only.
        #[arg(long)]
        animation_only: bool,
    },
    /// Compare animation measurements.
    #[command(
        long_about = "Compare the measurements of two inputs (asset files or prior `measure` JSON) and report movement beyond significance thresholds. Exits 1 on significant movement."
    )]
    Diff {
        /// Before input: asset file or single-file `measure --format json` report.
        a: PathBuf,
        /// After input: asset file or single-file `measure --format json` report.
        b: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
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

fn repair_action(repair: Repair) -> &'static str {
    match repair {
        Repair::QuatNorm => "unit-normalized",
        Repair::QuatFlip => "hemisphere-normalized",
        _ => "repaired",
    }
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

#[derive(Serialize)]
struct ToolInfo {
    name: &'static str,
    version: &'static str,
}

impl ToolInfo {
    fn current() -> Self {
        Self {
            name: "animsmith",
            version: env!("ANIMSMITH_VERSION"),
        }
    }
}

#[derive(Serialize)]
struct RigInfo {
    profile: String,
    resolved_roles: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
struct FileReport {
    path: String,
    rig: RigInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    findings: Option<Vec<Finding>>,
    measurements: BTreeMap<String, animsmith_core::measure::ClipMeasurements>,
    /// Static per-mesh geometry measurements; empty (and omitted) unless
    /// the input carried scene assets. Additive to the v1 schema.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    meshes: Vec<animsmith_core::measure::MeshMeasurements>,
}

#[derive(Default, Serialize)]
struct FindingSummary {
    error: usize,
    warning: usize,
    note: usize,
}

impl FindingSummary {
    fn add(&mut self, severity: Severity) {
        match severity {
            Severity::Error => self.error += 1,
            Severity::Warning => self.warning += 1,
            Severity::Note => self.note += 1,
        }
    }
}

#[derive(Serialize)]
struct ReportSummary {
    files: usize,
    findings: FindingSummary,
}

/// The common head of every JSON envelope — one definition for the
/// contract fields the schema requires of all commands.
#[derive(Serialize)]
struct EnvelopeHeader {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
}

impl EnvelopeHeader {
    fn new(command: &'static str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            schema: SCHEMA_URL,
            tool: ToolInfo::current(),
            command,
        }
    }
}

#[derive(Serialize)]
struct ReportEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    summary: ReportSummary,
    files: Vec<FileReport>,
}

impl ReportEnvelope {
    fn new(command: &'static str, files: Vec<FileReport>) -> Self {
        let mut findings = FindingSummary::default();
        for file in &files {
            if let Some(file_findings) = &file.findings {
                for finding in file_findings {
                    findings.add(finding.severity);
                }
            }
        }
        Self {
            header: EnvelopeHeader::new(command),
            summary: ReportSummary {
                files: files.len(),
                findings,
            },
            files,
        }
    }
}

#[derive(Serialize)]
struct DiffInputs {
    before: String,
    after: String,
}

#[derive(Serialize)]
struct DiffSummary {
    deltas: usize,
}

#[derive(Serialize)]
struct DiffEnvelope {
    #[serde(flatten)]
    header: EnvelopeHeader,
    inputs: DiffInputs,
    summary: DiffSummary,
    deltas: Vec<animsmith_core::diff::MetricDelta>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("animsmith: {message}");
            ExitCode::from(EXIT_OPERATOR)
        }
    }
}

fn load_config(explicit: Option<&Path>) -> Result<Config, String> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let default = PathBuf::from("animsmith.toml");
            if !default.exists() {
                return Ok(Config::default());
            }
            default
        }
    };
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("bad config {}: {e}", path.display()))
}

/// Resolve rig roles per the config: inline role map entries override
/// the (named or auto-detected) profile.
fn resolve_roles(doc: &Document, config: &Config) -> ResolvedRoles {
    let base = resolve_named(&doc.skeleton, &config.rig.profile).unwrap_or_default();
    if config.rig.roles.is_empty() {
        let mut roles = base;
        if roles.profile.is_empty() {
            roles.profile = "unknown".into();
        }
        return roles;
    }
    let mut pairs: Vec<_> = base
        .iter()
        .map(|(role, bone)| (role, doc.skeleton.bones[bone].name.clone()))
        .collect();
    pairs.extend(
        config
            .rig
            .roles
            .iter()
            .map(|(role, name)| (*role, name.clone())),
    );
    let mut resolved = ResolvedRoles::from_names(&doc.skeleton, pairs);
    resolved.profile = if base.profile.is_empty() {
        "custom".into()
    } else {
        format!("{}+custom", base.profile)
    };
    resolved
}

fn rig_info(doc: &Document, roles: &ResolvedRoles) -> RigInfo {
    RigInfo {
        profile: roles.profile.clone(),
        resolved_roles: roles
            .iter()
            .map(|(role, bone)| (role.as_str(), doc.skeleton.bones[bone].name.clone()))
            .collect(),
    }
}

/// Print one repair's report. `target` is the written path; `None`
/// means dry run (nothing written).
fn print_fix_report(
    repair: Repair,
    report: &animsmith_gltf::fix::FixReport,
    target: Option<&Path>,
) {
    let verb = if target.is_none() {
        "would fix"
    } else {
        "fixed"
    };
    for t in &report.tracks {
        println!(
            "  {verb}[{}] clip '{}' bone '{}': {} key(s) {}",
            repair.id(),
            t.clip,
            t.bone,
            t.fixed_keys,
            repair_action(repair)
        );
    }
    for s in &report.skipped {
        println!("  skipped[{}]: {s}", repair.id());
    }
    let destination = target
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "no output written".into());
    println!(
        "{} key(s) {} across {} track(s) -> {destination}",
        report.total_fixed(),
        if target.is_none() {
            "would be fixed"
        } else {
            "fixed"
        },
        report.tracks.len(),
    );
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.cmd {
        Cmd::Inspect { file } => {
            let config = load_config(cli.config.as_deref())?;
            let doc = load(&file)?;
            let roles = resolve_roles(&doc, &config);
            inspect(&doc, &roles);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            let config = load_config(cli.config.as_deref())?;
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let doc = load(file)?;
                let roles = resolve_roles(&doc, &config);
                let grids = MetricGrids::new(&doc);
                reports.push(FileReport {
                    path: file.display().to_string(),
                    rig: rig_info(&doc, &roles),
                    findings: None,
                    measurements: animsmith_core::measure::measure_document(
                        &grids, &roles, &config,
                    ),
                    meshes: animsmith_core::measure::measure_meshes(&doc.assets),
                });
            }
            match format {
                Format::Json => print_json(&ReportEnvelope::new("measure", reports)),
                Format::Text => {
                    for report in &reports {
                        println!("{}:", report.path);
                        for (clip, m) in &report.measurements {
                            let seam = m
                                .loop_seam_ratio
                                .map(|r| format!(" seam×{r:.2}"))
                                .unwrap_or_default();
                            let gait = m
                                .gait
                                .as_ref()
                                .and_then(|g| g.phase.map(|p| (p, g.lr_amplitude_m)))
                                .map(|(p, a)| format!(" gait φ={p:.2} ({:.1}cm)", a * 100.0))
                                .unwrap_or_default();
                            println!(
                                "  {clip}: {:.3}s, {} frames, {} animated bones{seam}{gait}",
                                m.duration_s,
                                m.frame_count,
                                m.animated_bones.len()
                            );
                        }
                        for mesh in &report.meshes {
                            let bbox = mesh
                                .aabb
                                .as_ref()
                                .map(|b| {
                                    let s = [
                                        b.max[0] - b.min[0],
                                        b.max[1] - b.min[1],
                                        b.max[2] - b.min[2],
                                    ];
                                    format!(" bbox {:.3}×{:.3}×{:.3}", s[0], s[1], s[2])
                                })
                                .unwrap_or_default();
                            let skin = match (mesh.weight_sum_min, mesh.weight_sum_max) {
                                (Some(lo), Some(hi)) => format!(
                                    ", ≤{} joints/vtx, weight-sum {lo:.3}–{hi:.3}",
                                    mesh.max_joints_per_vertex
                                ),
                                _ => String::new(),
                            };
                            println!(
                                "  mesh {}: {} verts{bbox}{skin}",
                                mesh.name, mesh.vertex_count
                            );
                        }
                    }
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
            let mut checks = all_checks();
            if !select.is_empty() {
                let known: Vec<&str> = checks.iter().map(|c| c.id()).collect();
                for id in &select {
                    if !known.contains(&id.as_str()) {
                        return Err(format!(
                            "--select: unknown check '{id}' (known: {})",
                            known.join(", ")
                        ));
                    }
                }
                checks.retain(|c| select.iter().any(|id| id == c.id()));
            }
            let mut reports = Vec::new();
            let mut worst = Severity::Note;
            for file in &files {
                let doc = load(file)?;
                let roles = resolve_roles(&doc, &config);
                let grids = MetricGrids::new(&doc);
                let ctx = CheckCtx::new(&grids, &roles, &config);
                let mut findings = run_checks(&ctx, &checks);
                findings.retain(|f| !allow.iter().any(|id| id == f.check_id));
                findings.sort_by(|a, b| {
                    (a.clip.as_deref(), std::cmp::Reverse(a.severity))
                        .cmp(&(b.clip.as_deref(), std::cmp::Reverse(b.severity)))
                });
                for finding in &findings {
                    worst = worst.max(finding.severity);
                }
                reports.push(FileReport {
                    path: file.display().to_string(),
                    rig: rig_info(&doc, &roles),
                    findings: Some(findings),
                    measurements: animsmith_core::measure::measure_document(
                        &grids, &roles, &config,
                    ),
                    // `lint` judges animation, not geometry — no meshes.
                    meshes: Vec::new(),
                });
            }
            match format {
                LintFormat::Json => print_json(&ReportEnvelope::new("lint", reports)),
                LintFormat::Text => print_text(&reports),
                LintFormat::Markdown => print_markdown(&reports),
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
            let roles = resolve_roles(&doc, &config);
            let grids = MetricGrids::new(&doc);
            let ctx = CheckCtx::new(&grids, &roles, &config);
            let findings = run_checks(&ctx, &all_checks());
            let html = animsmith_report::render(&grids, &roles, &findings, clip.as_deref());
            std::fs::write(&output, &html)
                .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
            println!(
                "wrote {} ({} clip(s), {} finding(s), {:.1} MB)",
                output.display(),
                doc.clips.len(),
                findings.len(),
                html.len() as f64 / 1e6
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Transform {
            input,
            output,
            clip,
            slice,
            hold_extend,
            gait_anchor,
            fps,
        } => {
            let config = load_config(cli.config.as_deref())?;
            let mut doc = load(&input)?;
            let roles = resolve_roles(&doc, &config);
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
            for c in doc.clips.iter_mut() {
                if clip.as_deref().is_some_and(|name| name != c.name) {
                    continue;
                }
                touched += 1;
                if let Some((a, b)) = window {
                    animsmith_core::transform::slice(c, a, b, fps);
                    println!(
                        "  sliced '{}' to [{a}:{b}]s ({} keys max)",
                        c.name,
                        c.tracks.iter().map(|t| t.key_count()).max().unwrap_or(0)
                    );
                }
                if let Some(hold) = hold_extend {
                    animsmith_core::transform::hold_extend(c, hold);
                    println!("  hold-extended '{}' by {hold}s", c.name);
                }
                if gait_anchor {
                    match animsmith_core::transform::align_gait_anchor(&skeleton, c, &roles, fps) {
                        Ok(o) => println!(
                            "  gait-anchored '{}': phase {:.3} -> {:.3} (offset {}, seam {})",
                            c.name,
                            o.phase_before,
                            o.phase_after,
                            o.frame_offset,
                            o.seam_after
                                .map(|s| format!("{s:.2}"))
                                .unwrap_or_else(|| "n/a".into()),
                        ),
                        Err(reason) => println!("  gait-anchor skipped '{}': {reason}", c.name),
                    }
                }
            }
            if touched == 0 {
                return Err(match clip {
                    Some(name) => format!("clip '{name}' not found in {}", input.display()),
                    None => format!("{} has no clips", input.display()),
                });
            }
            animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            println!("wrote {} ({touched} clip(s) transformed)", output.display());
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
            for (repair, report) in &reports {
                // clap rejects --dry-run with a write target, so
                // `output` is None exactly when this is a dry run.
                print_fix_report(*repair, report, output.as_deref());
            }
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
        } => {
            let mut doc = load(&input)?;
            // `--animation-only` clears assets uniformly across formats:
            // this is where a conversion drops its geometry on request.
            if animation_only {
                doc.assets = animsmith_core::model::SceneAssets::default();
            }
            animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            let vertices: usize = doc
                .assets
                .meshes
                .iter()
                .flat_map(|m| m.primitives.iter().map(|p| p.positions.len()))
                .sum();
            println!(
                "wrote {} ({} bones, {} clip(s), {} mesh(es) / {vertices} corners, {} material(s))",
                output.display(),
                doc.skeleton.bones.len(),
                doc.clips.len(),
                doc.assets.meshes.len(),
                doc.assets.materials.len(),
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Diff { a, b, format } => {
            let config = load_config(cli.config.as_deref())?;
            let ma = load_measurements(&a, &config)?;
            let mb = load_measurements(&b, &config)?;
            let deltas = animsmith_core::diff::diff_measurements(&ma, &mb);
            let has_deltas = !deltas.is_empty();
            match format {
                Format::Json => print_json(&DiffEnvelope {
                    header: EnvelopeHeader::new("diff"),
                    inputs: DiffInputs {
                        before: a.display().to_string(),
                        after: b.display().to_string(),
                    },
                    summary: DiffSummary {
                        deltas: deltas.len(),
                    },
                    deltas,
                }),
                Format::Text => {
                    if deltas.is_empty() {
                        println!("no significant movement");
                    }
                    for d in &deltas {
                        let values = match (d.before, d.after) {
                            (Some(x), Some(y)) => format!(" {x:.4} -> {y:.4}"),
                            (Some(x), None) => format!(" {x:.4} -> (gone)"),
                            (None, Some(y)) => format!(" (none) -> {y:.4}"),
                            (None, None) => String::new(),
                        };
                        println!("  {} {}: {}{values}", d.clip, d.metric, d.note);
                    }
                    println!("{} significant change(s)", deltas.len());
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

/// Measurements for `diff`: an asset file (measured now) or a prior
/// single-file `measure`/`lint` JSON report.
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
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("bad JSON in {}: {e}", path.display()))?;
        // Only the versioned v1 envelope is accepted — no pre-publish
        // legacy shapes, and no silently misreading a future version.
        match value.get("schema_version").and_then(|v| v.as_u64()) {
            Some(v) if v == u64::from(SCHEMA_VERSION) => {}
            Some(v) => {
                return Err(format!(
                    "{} has schema_version {v}; this build reads schema_version {SCHEMA_VERSION}",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{} is not an animsmith report envelope (no `schema_version`); \
                     regenerate it with `animsmith measure --format json`",
                    path.display()
                ));
            }
        }
        let Some(files) = value.get("files").and_then(|v| v.as_array()) else {
            return Err(format!(
                "{} is not an animsmith report envelope (no `files` array); \
                 regenerate it with `animsmith measure --format json`",
                path.display()
            ));
        };
        if files.len() != 1 {
            return Err(format!(
                "{} is a multi-file report; diff expects a single-file measurement report",
                path.display()
            ));
        }
        let map = files[0]
            .get("measurements")
            .cloned()
            .ok_or_else(|| format!("{} report has no measurements", path.display()))?;
        return serde_json::from_value(map)
            .map_err(|e| format!("{} is not a measurements report: {e}", path.display()));
    }
    let doc = load(path)?;
    let roles = resolve_roles(&doc, config);
    let grids = MetricGrids::new(&doc);
    Ok(animsmith_core::measure::measure_document(
        &grids, &roles, config,
    ))
}

fn require_files(files: &[PathBuf]) -> Result<(), String> {
    if files.is_empty() {
        Err("no input files given".into())
    } else {
        Ok(())
    }
}

fn load(path: &Path) -> Result<Document, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "glb" | "gltf" => animsmith_gltf::load(path).map_err(|e| e.to_string()),
        #[cfg(feature = "fbx")]
        "fbx" => animsmith_fbx::load(path).map_err(|e| e.to_string()),
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

fn print_json<T: Serialize>(value: &T) {
    let out = serde_json::to_string_pretty(value);
    println!("{}", out.expect("report serializes"));
}

fn print_text(reports: &[FileReport]) {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut notes = 0usize;
    for report in reports {
        let findings = report.findings.as_deref().unwrap_or_default();
        if findings.is_empty() {
            println!("{}: clean", report.path);
            continue;
        }
        println!("{}:", report.path);
        for f in findings {
            match f.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Note => notes += 1,
            }
            let mut location = String::new();
            if let Some(clip) = &f.clip {
                location.push_str(&format!(" clip '{clip}'"));
            }
            if let Some(bone) = &f.bone {
                location.push_str(&format!(" bone '{bone}'"));
            }
            if let Some(t) = f.time_s {
                location.push_str(&format!(" @{t:.3}s"));
            }
            let mut detail = String::new();
            if let (Some(measured), Some(expected)) = (&f.measured, &f.expected) {
                detail = format!(" (measured {measured}, expected {expected})");
            } else if let Some(measured) = &f.measured {
                detail = format!(" (measured {measured})");
            }
            println!(
                "  {}[{}]{}: {}{}",
                f.severity, f.check_id, location, f.message, detail
            );
        }
    }
    println!("{errors} error(s), {warnings} warning(s), {notes} note(s)");
}

/// The severity threshold at which a file's finding list is collapsed
/// behind a closed `<details>` element rather than shown expanded. Short
/// lists stay open so a reviewer sees them without a click; long lists
/// collapse so one noisy asset does not bury the rest of a CI comment.
const MARKDOWN_COLLAPSE_AT: usize = 10;

/// Render findings as GitHub/GitLab-flavored Markdown for CI comments and
/// asset-review threads. Presentation-only: the JSON output is the
/// machine-readable contract, and this layout carries no stability
/// guarantees. Mirrors the text output's information — severity, check
/// id, location, measured/expected values, per-clip grouping — as tables
/// inside per-file collapsible sections.
fn print_markdown(reports: &[FileReport]) {
    print!("{}", render_markdown(reports));
}

/// Pure Markdown renderer behind [`print_markdown`], returning the whole
/// document as a string. Keeping it side-effect free lets the per-clip
/// grouping, cell escaping, collapse threshold, and summary tallies be
/// unit-tested directly without spawning the CLI.
fn render_markdown(reports: &[FileReport]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut total = FindingSummary::default();

    let _ = writeln!(out, "## animsmith lint\n");

    for report in reports {
        let findings = report.findings.as_deref().unwrap_or_default();
        if findings.is_empty() {
            let _ = writeln!(out, "### `{}`\n", md_cell(&report.path));
            let _ = writeln!(out, "✅ Clean — no findings.\n");
            continue;
        }

        let mut file = FindingSummary::default();
        for f in findings {
            file.add(f.severity);
        }
        total.error += file.error;
        total.warning += file.warning;
        total.note += file.note;

        let _ = writeln!(out, "### `{}`\n", md_cell(&report.path));
        let _ = writeln!(out, "{}\n", severity_line(&file));

        let open = if findings.len() <= MARKDOWN_COLLAPSE_AT {
            " open"
        } else {
            ""
        };
        let count = findings.len();
        let plural = if count == 1 { "finding" } else { "findings" };
        let _ = writeln!(out, "<details{open}>");
        let _ = writeln!(
            out,
            "<summary><strong>{count} {plural}</strong></summary>\n"
        );

        let mut current_clip: Option<Option<&str>> = None;
        for f in findings {
            let clip = f.clip.as_deref();
            if current_clip != Some(clip) {
                current_clip = Some(clip);
                match clip {
                    Some(name) => {
                        let _ = writeln!(out, "\n#### clip `{}`\n", md_cell(name));
                    }
                    None => {
                        let _ = writeln!(out, "\n#### file-level\n");
                    }
                }
                let _ = writeln!(
                    out,
                    "| Severity | Check | Location | Measured | Expected | Message |"
                );
                let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- |");
            }
            let mut location = String::new();
            if let Some(bone) = &f.bone {
                let _ = write!(location, "bone `{}`", md_cell(bone));
            }
            if let Some(t) = f.time_s {
                if !location.is_empty() {
                    location.push(' ');
                }
                let _ = write!(location, "@{t:.3}s");
            }
            if location.is_empty() {
                location.push('—');
            }
            // Every asset-derived cell (check id excepted — it is
            // `'static`) is code-wrapped and escaped so an untrusted clip,
            // bone, value, or message cannot break the table or inject
            // Markdown/HTML into a CI comment. See `md_cell`.
            let _ = writeln!(
                out,
                "| {} {} | `{}` | {} | {} | {} | `{}` |",
                severity_badge(f.severity),
                f.severity,
                f.check_id,
                location,
                md_value_cell(f.measured.as_ref()),
                md_value_cell(f.expected.as_ref()),
                md_cell(&f.message),
            );
        }
        let _ = writeln!(out, "\n</details>\n");
    }

    let files = reports.len();
    let file_word = if files == 1 { "file" } else { "files" };
    let _ = writeln!(out, "---\n");
    let _ = writeln!(out, "**{files} {file_word}** — {}", severity_line(&total));
    out
}

/// A one-line severity tally for a Markdown header or footer, mirroring
/// the text summary's error/warning/note counts.
fn severity_line(summary: &FindingSummary) -> String {
    format!(
        "{} {} error(s) · {} {} warning(s) · {} {} note(s)",
        severity_badge(Severity::Error),
        summary.error,
        severity_badge(Severity::Warning),
        summary.warning,
        severity_badge(Severity::Note),
        summary.note,
    )
}

/// Emoji badge for a severity, chosen to render in a GitHub/GitLab
/// comment without a color-only cue.
fn severity_badge(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "❌",
        Severity::Warning => "⚠️",
        Severity::Note => "ℹ️",
    }
}

/// Render an optional measured/expected value as a Markdown table cell,
/// wrapping present values in backticks and using an em dash for absent
/// ones.
fn md_value_cell(value: Option<&animsmith_core::finding::Value>) -> String {
    match value {
        Some(v) => format!("`{}`", md_cell(&v.to_string())),
        None => "—".to_string(),
    }
}

/// Escape asset-derived text for a Markdown table cell that the renderer
/// wraps in a `` ` `` code span.
///
/// The finding fields fed here (clip, bone, message, textual measured /
/// expected values, and the input path) come from files a user
/// downloaded from anywhere, and this output is meant to be pasted into a
/// trusted GitHub/GitLab CI comment — so a hostile name must not be able
/// to break out and forge content. Two escapes cover that:
///
/// - Backslash-escape the pipe (and pre-double backslashes so an authored
///   `\|` cannot re-form an unescaped delimiter) and flatten newlines, so
///   the value stays inside its table cell.
/// - Replace the backtick, the only character that can close the
///   surrounding code span. Inside a code span every other Markdown/HTML
///   metacharacter (`<`, `>`, `[`, `*`, `!`, …) already renders literally,
///   so neutralizing the backtick is what blocks `</details>` breakout,
///   forged rows, and injected `<img>`/`<a>` tags.
///
/// A stray backslash may therefore render doubled inside the span; that
/// is a cosmetic loss on pathological names, acceptable for a
/// presentation-only format with no stability guarantee.
fn md_cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('`', "'")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn inspect(doc: &Document, roles: &ResolvedRoles) {
    if let Some(path) = &doc.source.path {
        println!("{path}");
    }
    if roles.is_empty() {
        println!("rig profile: none detected");
    } else {
        println!("rig profile: {} ({} roles)", roles.profile, roles.len());
        for (role, bone) in roles.iter() {
            println!(
                "  {:<12} -> {}",
                role.as_str(),
                doc.skeleton.bones[bone].name
            );
        }
    }
    println!("skeleton: {} bones", doc.skeleton.bones.len());
    for bone in &doc.skeleton.bones {
        let mut depth = 0;
        let mut parent = bone.parent;
        while let Some(p) = parent {
            depth += 1;
            parent = doc.skeleton.bones[p].parent;
        }
        let skinned = if bone.inverse_bind.is_some() {
            " [skinned]"
        } else {
            ""
        };
        println!("  {}{}{}", "  ".repeat(depth), bone.name, skinned);
    }
    println!("clips: {}", doc.clips.len());
    for clip in &doc.clips {
        let keys = clip
            .tracks
            .iter()
            .map(animsmith_core::model::Track::key_count)
            .max()
            .unwrap_or(0);
        println!(
            "  {}: {:.3}s, {} tracks, {} keys max",
            clip.name,
            clip.duration_s,
            clip.tracks.len(),
            keys
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal lint `FileReport`; only `path` and `findings` drive the
    /// Markdown renderer, so the rig/measurements/meshes are left empty.
    fn report(path: &str, findings: Vec<Finding>) -> FileReport {
        FileReport {
            path: path.to_string(),
            rig: RigInfo {
                profile: "unknown".to_string(),
                resolved_roles: BTreeMap::new(),
            },
            findings: Some(findings),
            measurements: BTreeMap::new(),
            meshes: Vec::new(),
        }
    }

    #[test]
    fn markdown_clean_file_renders_summary_without_a_table() {
        let md = render_markdown(&[report("clean.glb", vec![])]);
        assert!(md.contains("### `clean.glb`"), "{md}");
        assert!(md.contains("✅ Clean — no findings."), "{md}");
        assert!(!md.contains("<details"), "{md}");
        assert!(!md.contains("| Severity |"), "{md}");
        // Footer: singular "file" and a zeroed total.
        assert!(md.contains("**1 file** — ❌ 0 error(s)"), "{md}");
    }

    #[test]
    fn markdown_renders_location_and_measured_expected_cells() {
        let f = Finding::new("quat-norm", Severity::Error, "non-unit key")
            .clip("walk")
            .bone("spine")
            .time(0.5)
            .measured(1.05_f64)
            .expected(1.0_f64);
        let md = render_markdown(&[report("a.glb", vec![f])]);
        // The Location cell carries the bone and the formatted time, and
        // the measured/expected values render as their own cells — a
        // renderer that dropped either would fail here.
        assert!(md.contains("bone `spine` @0.500s"), "{md}");
        assert!(
            md.contains("| `1.0500` | `1.0000` | `non-unit key` |"),
            "{md}"
        );
    }

    #[test]
    fn markdown_file_level_findings_use_em_dash_and_heading() {
        // No clip, bone, time, or values: file-level heading plus the
        // em-dash placeholder in every optional cell.
        let f = Finding::new("nan", Severity::Error, "bad");
        let md = render_markdown(&[report("a.glb", vec![f])]);
        assert!(md.contains("#### file-level"), "{md}");
        assert!(
            md.contains("| ❌ error | `nan` | — | — | — | `bad` |"),
            "{md}"
        );
    }

    #[test]
    fn markdown_starts_a_fresh_table_per_clip() {
        // Fed contiguous-by-clip, as the lint command sorts before
        // rendering; two clips must yield two headers and two tables.
        let findings = vec![
            Finding::new("a", Severity::Error, "m1").clip("walk"),
            Finding::new("b", Severity::Warning, "m2").clip("walk"),
            Finding::new("c", Severity::Error, "m3").clip("run"),
        ];
        let md = render_markdown(&[report("a.glb", findings)]);
        assert!(md.contains("#### clip `walk`"), "{md}");
        assert!(md.contains("#### clip `run`"), "{md}");
        assert_eq!(md.matches("| Severity | Check |").count(), 2, "{md}");
    }

    #[test]
    fn markdown_collapses_only_long_finding_lists() {
        let make = |n: usize| {
            let findings = (0..n)
                .map(|_| Finding::new("a", Severity::Note, "m").clip("walk"))
                .collect();
            render_markdown(&[report("a.glb", findings)])
        };
        // The boundary: ten stay expanded, eleven collapse.
        assert!(make(MARKDOWN_COLLAPSE_AT).contains("<details open>"));
        let collapsed = make(MARKDOWN_COLLAPSE_AT + 1);
        assert!(collapsed.contains("<details>"), "{collapsed}");
        assert!(!collapsed.contains("<details open>"), "{collapsed}");
    }

    #[test]
    fn markdown_footer_sums_severities_across_files() {
        let a = report(
            "a.glb",
            vec![Finding::new("x", Severity::Error, "m").clip("c")],
        );
        let b = report(
            "b.glb",
            vec![
                Finding::new("y", Severity::Warning, "m").clip("c"),
                Finding::new("z", Severity::Note, "m").clip("c"),
            ],
        );
        let md = render_markdown(&[a, b]);
        // Plural "files" and the total summed across both inputs — not the
        // last file's counts alone.
        assert!(
            md.contains("**2 files** — ❌ 1 error(s) · ⚠️ 1 warning(s) · ℹ️ 1 note(s)"),
            "{md}"
        );
    }

    #[test]
    fn markdown_escapes_hostile_cell_text() {
        // A malicious asset name carrying the table delimiter, a code-span
        // closer, an HTML tag, and a newline must be neutralized so it can
        // neither break the table nor inject Markdown/HTML into a comment.
        let f = Finding::new("x", Severity::Error, "msg")
            .clip("walk")
            .bone("evil|`</details>\nrow");
        let md = render_markdown(&[report("a.glb", vec![f])]);
        // Pipe backslash-escaped, backtick replaced, newline flattened.
        assert!(md.contains("bone `evil\\|'</details> row`"), "{md}");
        // The raw hostile prefix never survives verbatim.
        assert!(!md.contains("evil|`"), "{md}");
    }
}
