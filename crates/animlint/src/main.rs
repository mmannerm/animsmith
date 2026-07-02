//! The animlint CLI. See DESIGN.md §3 for the surface; M0 ships
//! `inspect`, `measure`, and `lint` over glTF/GLB with the mechanical
//! check set. `convert`, `report`, `diff`, and FBX input arrive in
//! later milestones.

use animlint_core::model::Document;
use animlint_core::{Finding, Severity, mechanical_checks, run_checks};
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
const SCHEMA_VERSION: u32 = 0;

#[derive(Parser)]
#[command(
    name = "animlint",
    version,
    about = "A linter for skeletal animation clips"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Summarize a file: skeleton, clips, tracks.
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
struct FileReport {
    schema_version: u32,
    tool: ToolInfo,
    file: String,
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

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.cmd {
        Cmd::Inspect { file } => {
            let doc = load(&file)?;
            inspect(&doc);
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let doc = load(file)?;
                reports.push(FileReport {
                    schema_version: SCHEMA_VERSION,
                    tool: ToolInfo::current(),
                    file: file.display().to_string(),
                    findings: None,
                    measurements: animlint_core::measure::measure_document(&doc),
                });
            }
            match format {
                Format::Json => print_json(&reports),
                Format::Text => {
                    for report in &reports {
                        println!("{}:", report.file);
                        for (clip, m) in &report.measurements {
                            println!(
                                "  {clip}: {:.3}s, {} frames, {} animated bones",
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
        } => {
            require_files(&files)?;
            let checks = mechanical_checks();
            let mut reports = Vec::new();
            let mut worst = Severity::Note;
            for file in &files {
                let doc = load(file)?;
                let mut findings = run_checks(&doc, &checks);
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
                    findings: Some(findings),
                    measurements: animlint_core::measure::measure_document(&doc),
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
    }
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
        "fbx" => Err(format!(
            "{}: FBX input lands in M2 (via ufbx); convert to glTF for now",
            path.display()
        )),
        _ => Err(format!(
            "{}: unsupported input (expected .glb or .gltf)",
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

fn inspect(doc: &Document) {
    if let Some(path) = &doc.source.path {
        println!("{path}");
    }
    println!("skeleton: {} bones", doc.skeleton.bones.len());
    for (id, bone) in doc.skeleton.bones.iter().enumerate() {
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
        let _ = id;
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
