//! The animsmith CLI: inspect, measure, lint, report, transform, fix,
//! convert, and diff skeletal animation clips.

mod diff;

use animsmith_core::model::Document;
use animsmith_core::profile::{ResolvedRoles, resolve_named};
use animsmith_core::{CheckCtx, Config, Finding, Severity, all_checks, run_checks};
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
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
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
        about = "Render a self-contained offline HTML report",
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
        about = "Apply mechanical clip transforms",
        long_about = "Apply pipeline-mechanical clip transforms and write the result as skeleton+animation glTF. Meshes are not carried; transform clips before splicing them into a full asset. Operations apply to every clip, or one clip via --clip."
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
        about = "Repair safe mechanical glTF/GLB defects",
        long_about = "Repair mechanical clip defects in place, byte-surgically: only the offending animation bytes change; meshes, skins, materials, and textures pass through untouched. Currently fixes quaternion hemisphere flips (the `quat-flip` check) on glTF/GLB inputs."
    )]
    Fix {
        /// Input .glb or .gltf file. Omit only with --list-repairs.
        #[arg(value_name = "FILE")]
        input: Option<PathBuf>,
        /// Output path. Required unless --in-place or --dry-run is used.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Modify the input file in place.
        #[arg(long, conflicts_with = "output")]
        in_place: bool,
        /// Run exactly these repairs (comma-separated ids).
        #[arg(long = "repair", value_enum, value_delimiter = ',')]
        repairs: Vec<Repair>,
        /// Run this repair group when --repair is not set.
        #[arg(long, value_enum, default_value = "default")]
        group: RepairGroup,
        /// Report what would be repaired without writing output.
        #[arg(long)]
        dry_run: bool,
        /// List known repairs and groups.
        #[arg(long)]
        list_repairs: bool,
    },
    /// Convert FBX input to glTF.
    #[command(
        about = "Convert FBX input to glTF",
        long_about = "Convert FBX input to glTF: skeleton, animation, triangulated meshes, skins, and factor-only materials. Texture wiring stays a downstream concern. Output format by extension: .glb binary, .gltf JSON with an embedded buffer."
    )]
    #[cfg(feature = "fbx")]
    Convert {
        /// Input .fbx file.
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
        about = "Compare animation measurements",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Repair {
    QuatFlip,
}

impl Repair {
    fn id(self) -> &'static str {
        match self {
            Repair::QuatFlip => "quat-flip",
        }
    }

    fn summary(self) -> &'static str {
        match self {
            Repair::QuatFlip => "lossless quaternion hemisphere normalization",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RepairGroup {
    Default,
    Lossless,
    Mechanical,
    All,
}

const ALL_REPAIRS: &[Repair] = &[Repair::QuatFlip];
const ALL_REPAIR_GROUPS: &[RepairGroup] = &[
    RepairGroup::Default,
    RepairGroup::Lossless,
    RepairGroup::Mechanical,
    RepairGroup::All,
];

impl RepairGroup {
    fn repairs(self) -> &'static [Repair] {
        // Invariant: `default` contains only safe, lossless,
        // idempotent repairs. Broader groups may grow faster, but must
        // remain explicit here so automation can pin the intended risk
        // profile by group name.
        match self {
            RepairGroup::Default
            | RepairGroup::Lossless
            | RepairGroup::Mechanical
            | RepairGroup::All => ALL_REPAIRS,
        }
    }

    fn description(self) -> &'static str {
        match self {
            RepairGroup::Default => "safe, lossless, idempotent repairs",
            RepairGroup::Lossless => "all mathematically lossless repairs",
            RepairGroup::Mechanical => "deterministic pipeline-mechanical repairs",
            RepairGroup::All => "every available repair",
        }
    }
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
            _ => self.note += 1,
        }
    }
}

#[derive(Serialize)]
struct ReportSummary {
    files: usize,
    findings: FindingSummary,
}

#[derive(Serialize)]
struct ReportEnvelope {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
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
            schema_version: SCHEMA_VERSION,
            schema: SCHEMA_URL,
            tool: ToolInfo::current(),
            command,
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
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    inputs: DiffInputs,
    summary: DiffSummary,
    deltas: Vec<diff::MetricDelta>,
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

fn print_repairs() {
    println!("repairs:");
    for repair in ALL_REPAIRS {
        println!("  {:<12} {}", repair.id(), repair.summary());
    }
    println!("groups:");
    for group in ALL_REPAIR_GROUPS {
        let repairs = group
            .repairs()
            .iter()
            .map(|repair| repair.id())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  {:<12} {} [{}]",
            group
                .to_possible_value()
                .expect("repair groups have clap values")
                .get_name(),
            group.description(),
            repairs
        );
    }
}

fn print_fix_report(
    repair: Repair,
    report: &animsmith_gltf::fix::FixReport,
    output: Option<&Path>,
    dry_run: bool,
) {
    let verb = if dry_run { "would fix" } else { "fixed" };
    for t in &report.tracks {
        println!(
            "  {verb}[{}] clip '{}' bone '{}': {} key(s) hemisphere-normalized",
            repair.id(),
            t.clip,
            t.bone,
            t.flipped_keys
        );
    }
    for s in &report.skipped {
        println!("  skipped[{}]: {s}", repair.id());
    }
    let target = if dry_run {
        "no output written".into()
    } else {
        output
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "output".into())
    };
    println!(
        "{} key(s) {} across {} track(s) -> {target}",
        report.total_flipped(),
        if dry_run { "would be fixed" } else { "fixed" },
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
                reports.push(FileReport {
                    path: file.display().to_string(),
                    rig: rig_info(&doc, &roles),
                    findings: None,
                    measurements: animsmith_core::measure::measure_document(&doc, &roles),
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
                let ctx = CheckCtx::new(&doc, &roles, &config);
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
                    measurements: animsmith_core::measure::measure_document(&doc, &roles),
                });
            }
            match format {
                Format::Json => print_json(&ReportEnvelope::new("lint", reports)),
                Format::Text => print_text(&reports),
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
            let ctx = CheckCtx::new(&doc, &roles, &config);
            let findings = run_checks(&ctx, &all_checks());
            let html = animsmith_report::render(&doc, &roles, &findings, clip.as_deref());
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
            group,
            dry_run,
            list_repairs,
        } => {
            if list_repairs {
                print_repairs();
                return Ok(ExitCode::SUCCESS);
            }
            let input = input.ok_or_else(|| "fix requires an input file".to_string())?;
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
            let selected = if repairs.is_empty() {
                group.repairs().to_vec()
            } else {
                repairs
            };
            if selected.is_empty() {
                return Err("no repairs selected".into());
            }
            if !dry_run && selected.len() > 1 {
                return Err(
                    "running multiple repairs in one write is not supported yet; rerun one repair at a time"
                        .into(),
                );
            }
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
            for repair in selected {
                match repair {
                    Repair::QuatFlip => {
                        let report = if dry_run {
                            animsmith_gltf::fix::inspect_quat_hemisphere(&input)
                                .map_err(|e| e.to_string())?
                        } else {
                            let output = output.as_ref().expect("validated output target");
                            animsmith_gltf::fix::fix_quat_hemisphere(&input, output)
                                .map_err(|e| e.to_string())?
                        };
                        print_fix_report(repair, &report, output.as_deref(), dry_run);
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        #[cfg(feature = "fbx")]
        Cmd::Convert {
            input,
            output,
            animation_only,
        } => {
            let ext = input
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let (doc, assets) = match ext.as_str() {
                "fbx" if !animation_only => {
                    animsmith_fbx::load_with_assets(&input).map_err(|e| e.to_string())?
                }
                // glTF ingestion carries no scene assets, so the output
                // is animation-only regardless of the flag (which only
                // the fbx path reads).
                _ => {
                    let _ = animation_only;
                    (load(&input)?, animsmith_core::model::SceneAssets::default())
                }
            };
            animsmith_gltf::write::write_with_assets(&doc, &assets, &output)
                .map_err(|e| e.to_string())?;
            let vertices: usize = assets
                .meshes
                .iter()
                .flat_map(|m| m.primitives.iter().map(|p| p.positions.len()))
                .sum();
            println!(
                "wrote {} ({} bones, {} clip(s), {} mesh(es) / {vertices} corners, {} material(s))",
                output.display(),
                doc.skeleton.bones.len(),
                doc.clips.len(),
                assets.meshes.len(),
                assets.materials.len(),
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Diff { a, b, format } => {
            let config = load_config(cli.config.as_deref())?;
            let ma = load_measurements(&a, &config)?;
            let mb = load_measurements(&b, &config)?;
            let deltas = diff::diff_measurements(&ma, &mb);
            let has_deltas = !deltas.is_empty();
            match format {
                Format::Json => print_json(&DiffEnvelope {
                    schema_version: SCHEMA_VERSION,
                    schema: SCHEMA_URL,
                    tool: ToolInfo::current(),
                    command: "diff",
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
        let map = if let Some(files) = value.get("files").and_then(|v| v.as_array()) {
            if files.len() != 1 {
                return Err(format!(
                    "{} is a multi-file report; diff expects a single-file measurement report",
                    path.display()
                ));
            }
            files[0]
                .get("measurements")
                .cloned()
                .ok_or_else(|| format!("{} report has no measurements", path.display()))?
        } else {
            value.get("measurements").cloned().unwrap_or(value)
        };
        return serde_json::from_value(map)
            .map_err(|e| format!("{} is not a measurements report: {e}", path.display()));
    }
    let doc = load(path)?;
    let roles = resolve_roles(&doc, config);
    Ok(animsmith_core::measure::measure_document(&doc, &roles))
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
                _ => notes += 1,
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
