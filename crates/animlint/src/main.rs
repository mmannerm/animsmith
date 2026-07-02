//! The animlint CLI. See DESIGN.md §3 for the surface; M0+M1 ship
//! `inspect`, `measure`, and `lint` over glTF/GLB with the mechanical
//! and semantic check sets, rig profiles, and TOML config. `convert`,
//! `report`, `diff`, and FBX input arrive in later milestones.

mod diff;

use animlint_core::model::Document;
use animlint_core::profile::{ResolvedRoles, resolve_named};
use animlint_core::{CheckCtx, Config, Finding, Severity, all_checks, run_checks};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Exit codes, wire-compatible with rauta's validate-assets:
/// 0 = clean or warnings-only, 1 = error findings, 2 = operator error.
const EXIT_FINDINGS: u8 = 1;
const EXIT_OPERATOR: u8 = 2;

/// Version of the machine-readable output schema, bumped on breaking
/// changes to the JSON shape.
const SCHEMA_VERSION: u32 = 1;

#[derive(Parser)]
#[command(
    name = "animlint",
    version,
    about = "A linter for skeletal animation clips"
)]
struct Cli {
    /// Config file (defaults to ./animlint.toml when present).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Summarize a file: skeleton, clips, tracks, detected rig profile.
    Inspect { file: PathBuf },
    /// Emit per-clip measurements without judging them.
    Measure {
        files: Vec<PathBuf>,
        #[arg(long, value_enum, default_value_t = Format::Json)]
        format: Format,
    },
    /// Run the check catalog and report findings.
    Lint {
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
    /// Convert an input (typically FBX) to glTF: skeleton + animation
    /// tracks only — no meshes, skins, or materials. Output format by
    /// extension: .glb binary, .gltf JSON with an embedded buffer.
    Convert {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Compare the measurements of two inputs (asset files or prior
    /// `measure` JSON) and report movement beyond significance
    /// thresholds. Exits 1 on significant movement.
    Diff {
        a: PathBuf,
        b: PathBuf,
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Serialize)]
struct ToolInfo {
    name: &'static str,
    version: &'static str,
}

impl ToolInfo {
    fn current() -> Self {
        Self {
            name: "animlint",
            version: env!("CARGO_PKG_VERSION"),
        }
    }
}

#[derive(Serialize)]
struct RigInfo {
    profile: String,
    resolved_roles: std::collections::BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
struct FileReport {
    schema_version: u32,
    tool: ToolInfo,
    file: String,
    rig: RigInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    findings: Option<Vec<Finding>>,
    measurements: std::collections::BTreeMap<String, animlint_core::measure::ClipMeasurements>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("animlint: {message}");
            ExitCode::from(EXIT_OPERATOR)
        }
    }
}

fn load_config(explicit: Option<&Path>) -> Result<Config, String> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let default = PathBuf::from("animlint.toml");
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
        return base;
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

fn run(cli: Cli) -> Result<ExitCode, String> {
    let config = load_config(cli.config.as_deref())?;
    match cli.cmd {
        Cmd::Inspect { file } => {
            let doc = load(&file)?;
            let roles = resolve_roles(&doc, &config);
            inspect(&doc, &roles);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let doc = load(file)?;
                let roles = resolve_roles(&doc, &config);
                reports.push(FileReport {
                    schema_version: SCHEMA_VERSION,
                    tool: ToolInfo::current(),
                    file: file.display().to_string(),
                    rig: rig_info(&doc, &roles),
                    findings: None,
                    measurements: animlint_core::measure::measure_document(&doc, &roles),
                });
            }
            match format {
                Format::Json => print_json(&reports),
                Format::Text => {
                    for report in &reports {
                        println!("{}:", report.file);
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
                    schema_version: SCHEMA_VERSION,
                    tool: ToolInfo::current(),
                    file: file.display().to_string(),
                    rig: rig_info(&doc, &roles),
                    findings: Some(findings),
                    measurements: animlint_core::measure::measure_document(&doc, &roles),
                });
            }
            match format {
                Format::Json => print_json(&reports),
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
        Cmd::Convert { input, output } => {
            let doc = load(&input)?;
            animlint_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            let clips = doc.clips.len();
            let bones = doc.skeleton.bones.len();
            println!(
                "wrote {} ({bones} bones, {clips} clip(s); skeleton + animation only)",
                output.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Diff { a, b, format } => {
            let ma = load_measurements(&a, &config)?;
            let mb = load_measurements(&b, &config)?;
            let deltas = diff::diff_measurements(&ma, &mb);
            match format {
                Format::Json => print_json(&deltas),
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
            Ok(if deltas.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_FINDINGS)
            })
        }
    }
}

/// Measurements for `diff`: an asset file (measured now) or a prior
/// `measure`/`lint` JSON report (its `measurements` field, or the whole
/// object as a bare measurement map).
fn load_measurements(
    path: &Path,
    config: &Config,
) -> Result<std::collections::BTreeMap<String, animlint_core::measure::ClipMeasurements>, String> {
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
        let map = value.get("measurements").cloned().unwrap_or(value);
        return serde_json::from_value(map)
            .map_err(|e| format!("{} is not a measurements report: {e}", path.display()));
    }
    let doc = load(path)?;
    let roles = resolve_roles(&doc, config);
    Ok(animlint_core::measure::measure_document(&doc, &roles))
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
        "glb" | "gltf" => animlint_gltf::load(path).map_err(|e| e.to_string()),
        #[cfg(feature = "fbx")]
        "fbx" => animlint_fbx::load(path).map_err(|e| e.to_string()),
        #[cfg(not(feature = "fbx"))]
        "fbx" => Err(format!(
            "{}: this animlint build has no FBX support (rebuild with the default `fbx` feature)",
            path.display()
        )),
        _ => Err(format!(
            "{}: unsupported input (expected .glb, .gltf, or .fbx)",
            path.display()
        )),
    }
}

fn print_json<T: Serialize>(reports: &[T]) {
    let out = if reports.len() == 1 {
        serde_json::to_string_pretty(&reports[0])
    } else {
        serde_json::to_string_pretty(reports)
    };
    println!("{}", out.expect("report serializes"));
}

fn print_text(reports: &[FileReport]) {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut notes = 0usize;
    for report in reports {
        let findings = report.findings.as_deref().unwrap_or_default();
        if findings.is_empty() {
            println!("{}: clean", report.file);
            continue;
        }
        println!("{}:", report.file);
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
            .map(animlint_core::model::Track::key_count)
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
