//! The animsmith CLI binary.
//!
//! This crate publishes the `animsmith` command: inspect, measure, lint,
//! report, transform, fix, convert, and diff skeletal animation clips. It
//! is not the Rust library API; use `animsmith-core` plus the loader
//! crates (`animsmith-gltf`, `animsmith-fbx`) and `animsmith-report` from
//! library code.
//!
//! Feature gates mirror the installed binary surface. The default build
//! includes FBX input and HTML reports; `--no-default-features` leaves a
//! pure-Rust glTF-only binary with report generation and FBX conversion
//! omitted.
//!
//! The GitHub [pipeline scenario guide] maps these commands to marketplace
//! intake, mocap cleanup, outsourced acceptance, CI, and artifact-storage
//! workflows.
//!
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md

#![warn(missing_docs)]

use animsmith_core::Document;
use animsmith_core::{
    CheckCtx, CheckSelection, Config, DiffEnvelope, LintEnvelope, LintFileReport, MeasureEnvelope,
    MeasureFileReport, MeasurementContract, MeasurementFileError, MeasurementReportError,
    MeasurementReportInput, MetricGrids, RigInfo, Severity, ToolInfo, ToolSource, all_checks,
    evaluate_checks, resolve_configured_roles,
};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod render;

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
        long_about = "Apply pipeline-mechanical clip transforms and write the result as glTF, carrying through any scene assets the input brought (FBX or glTF meshes, skins, materials, and embedded base-color textures). Operations apply to every clip, or one clip via --clip."
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
        long_about = "Convert FBX or glTF input to glTF: skeleton, animation, triangulated meshes, skins, factor-only materials, and embedded PNG/JPEG base-color textures. A glTF input is re-emitted carrying its geometry; --animation-only drops it. Output format by extension: .glb binary, .gltf JSON with an embedded buffer."
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
        long_about = "Compare the measurements of two inputs (asset files or prior single-file `measure` or `lint` JSON) and report movement beyond significance thresholds. Exits 1 on significant movement."
    )]
    Diff {
        /// Before input: asset file or single-file v2 `measure`/`lint` JSON report.
        a: PathBuf,
        /// After input: asset file or single-file v2 `measure`/`lint` JSON report.
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
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprint!("{}", render::render_operator_error(&message));
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
            for line in render::render_inspect(&doc, &roles) {
                println!("{line}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Measure { files, format } => {
            let config = load_config(cli.config.as_deref())?;
            require_files(&files)?;
            let mut reports = Vec::new();
            for file in &files {
                let doc = load(file)?;
                let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
                let grids = MetricGrids::new(&doc);
                reports.push(MeasureFileReport::new(
                    file.display().to_string(),
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
                    render::print_json(&envelope);
                }
                Format::Text => {
                    for line in render::render_measure_text(&reports) {
                        println!("{line}");
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
                let doc = load(file)?;
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
                    render::print_json(&envelope);
                }
                LintFormat::Text => print!("{}", render::render_text(&reports, &allow)),
                LintFormat::Markdown => print!("{}", render::render_markdown(&reports, &allow)),
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
            print!(
                "{}",
                render::render_report_written(&output, doc.clips.len(), findings.len(), html.len())
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
            for c in doc.clips.iter_mut() {
                if clip.as_deref().is_some_and(|name| name != c.name) {
                    continue;
                }
                touched += 1;
                if let Some((a, b)) = window {
                    animsmith_core::transform::slice(c, a, b, fps);
                    print!("{}", render::render_transform_slice(c, a, b));
                }
                if let Some(hold) = hold_extend {
                    animsmith_core::transform::hold_extend(c, hold);
                    print!("{}", render::render_transform_hold_extend(c, hold));
                }
                if gait_anchor {
                    match animsmith_core::transform::align_gait_anchor(&skeleton, c, &roles, fps) {
                        Ok(outcome) => print!(
                            "{}",
                            render::render_transform_gait_anchor(
                                &c.name,
                                outcome.phase_before,
                                outcome.phase_after,
                                outcome.frame_offset,
                                outcome.seam_after
                            )
                        ),
                        Err(reason) => print!(
                            "{}",
                            render::render_transform_gait_anchor_skipped(&c.name, &reason)
                        ),
                    }
                }
            }
            if touched == 0 {
                return Err(match clip {
                    Some(name) => format!("clip '{name}' not found in {}", input.display()),
                    None => format!("{} has no clips", input.display()),
                });
            }
            let summary = animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            print!("{}", render::render_write_summary(&output, &summary));
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
                for line in render::render_fix_report(*repair, report, output.as_deref()) {
                    println!("{line}");
                }
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
            let summary = animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            print!("{}", render::render_write_summary(&output, &summary));
            Ok(ExitCode::SUCCESS)
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
                )),
                Format::Text => {
                    for line in render::render_diff_text(&deltas) {
                        println!("{line}");
                    }
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
        let report: MeasurementReportInput = serde_json::from_str(&text)
            .map_err(|e| format!("bad JSON in {}: {e}", path.display()))?;
        // Only the final v2 envelope with measurement contract v2 is
        // accepted. Pre-finalization report shapes are intentionally not
        // retained while the project is alpha.
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
    const REMEDIATION: &str = "regenerate it with `animsmith measure --format json`";
    if let Some(found) = file_count.filter(|found| *found != 1)
        && error.file_index().is_some()
    {
        return diff_file_count_error(found);
    }
    match error {
        MeasurementReportError::MissingOutputVersion => {
            format!("is not an animsmith report envelope (no `schema_version`); {REMEDIATION}")
        }
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
                "has measurement schema_version {found}; this build reads measurement schema_version {}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_owns_remediation_for_invalid_measurements() {
        // Workspace test builds enable serde_json's `float_roundtrip` through
        // a dev dependency and reject f32 overflow while parsing. Shipped
        // binaries can instead reach this branch, so construct the public
        // typed error to pin CLI policy independently of feature unification.
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
            "has invalid measurements: measurement value meshes[0].aabb.min[0] must be finite; regenerate it with `animsmith measure --format json`"
        );
    }
}
