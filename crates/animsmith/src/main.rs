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
    CheckCtx, CheckSelection, Config, DiffEnvelope, LintFileReport, MEASUREMENTS_SCHEMA_ID,
    MEASUREMENTS_SCHEMA_VERSION, MeasureFileReport, MeasurementContract, MetricGrids,
    OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION, ReportEnvelope, ResolvedRoles, RigInfo, Severity,
    ToolInfo, ToolSource, all_checks, evaluate_checks, resolve_configured_roles,
};
use animsmith_gltf::fix::Repair;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
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

fn current_tool() -> ToolInfo {
    ToolInfo::animsmith(
        env!("CARGO_PKG_VERSION"),
        ToolSource::new(
            option_env!("ANIMSMITH_GIT_REVISION").map(str::to_owned),
            option_env!("ANIMSMITH_GIT_DIRTY").and_then(|value| value.parse().ok()),
        ),
    )
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
            inspect(&doc, &roles);
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
                    RigInfo::from_resolved(&doc, &roles),
                    MeasurementContract::new(
                        animsmith_core::measure::measure_document(&grids, &roles, &config),
                        animsmith_core::measure::measure_meshes(&doc.assets),
                    ),
                ));
            }
            match format {
                Format::Json => {
                    let envelope = ReportEnvelope::measure(current_tool(), reports);
                    render::print_json(&envelope);
                }
                Format::Text => {
                    for report in &reports {
                        println!("{}:", render::text_atom(report.path()));
                        for (clip, m) in report.measurements().clips() {
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
                                "  {}: {:.3}s, {} frames, {} animated bones{seam}{gait}",
                                render::text_atom(clip),
                                m.duration_s,
                                m.frame_count,
                                m.animated_bones.len()
                            );
                        }
                        for mesh in report.measurements().meshes() {
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
                                render::text_atom(&mesh.name),
                                mesh.vertex_count
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
                    RigInfo::from_resolved(&doc, &roles),
                    evaluations,
                    MeasurementContract::new(
                        animsmith_core::measure::measure_document(&grids, &roles, &config),
                        animsmith_core::measure::measure_meshes(&doc.assets),
                    ),
                ));
            }
            match format {
                LintFormat::Json => {
                    let envelope = ReportEnvelope::lint(current_tool(), reports);
                    render::print_json(&envelope);
                }
                LintFormat::Text => render::print_text(&reports, &allow),
                LintFormat::Markdown => render::print_markdown(&reports, &allow),
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
            let summary = animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            println!("{}", format_write_summary(&output, &summary));
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
            let summary = animsmith_gltf::write::write(&doc, &output).map_err(|e| e.to_string())?;
            println!("{}", format_write_summary(&output, &summary));
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
                        println!(
                            "  {} {}: {}{values}",
                            render::text_atom(&d.clip),
                            render::text_atom(&d.metric),
                            render::text_atom(&d.note)
                        );
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

fn format_write_summary(output: &Path, summary: &animsmith_gltf::write::WriteSummary) -> String {
    let mut text = format!(
        "wrote {} ({} node(s), {} clip(s), {} mesh(es) / {} position(s), {} material(s))",
        output.display(),
        summary.nodes,
        summary.animations,
        summary.meshes,
        summary.primitive_positions,
        summary.materials,
    );
    if summary.clips_without_writable_tracks > 0 {
        text.push_str(&format!(
            "; dropped {} clip(s) with no writable tracks",
            summary.clips_without_writable_tracks
        ));
    }
    text
}

#[derive(Deserialize)]
struct MeasurementReportInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    command: Option<String>,
    files: Option<Vec<MeasurementFileInput>>,
}

#[derive(Deserialize)]
struct MeasurementFileInput {
    measurements: Option<MeasurementPayloadInput>,
}

#[derive(Deserialize)]
struct MeasurementPayloadInput {
    schema_version: Option<u32>,
    schema: Option<String>,
    clips: Option<BTreeMap<String, animsmith_core::measure::ClipMeasurements>>,
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
        // Only the final v2 envelope with measurement contract v1 is
        // accepted. Pre-finalization report shapes are intentionally not
        // retained while the project is alpha.
        match report.schema_version {
            Some(v) if v == OUTPUT_SCHEMA_VERSION => {}
            Some(v) => {
                return Err(format!(
                    "{} has schema_version {v}; this build reads schema_version {OUTPUT_SCHEMA_VERSION}",
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
        if report.schema.as_deref() != Some(OUTPUT_SCHEMA_ID) {
            return Err(format!(
                "{} does not identify output contract {OUTPUT_SCHEMA_ID}; regenerate it with `animsmith measure --format json`",
                path.display()
            ));
        }
        match report.command.as_deref() {
            Some("measure" | "lint") => {}
            Some(command) => {
                return Err(format!(
                    "{} is a {command:?} report; diff reads only measure or lint reports",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{} is not an animsmith measurement report (no `command`); regenerate it with `animsmith measure --format json`",
                    path.display()
                ));
            }
        }
        let Some(files) = report.files else {
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
        let measurements = files
            .into_iter()
            .next()
            .expect("one file established")
            .measurements
            .ok_or_else(|| format!("{} report has no measurements", path.display()))?;
        match measurements.schema_version {
            Some(v) if v == MEASUREMENTS_SCHEMA_VERSION => {}
            Some(v) => {
                return Err(format!(
                    "{} has measurement schema_version {v}; this build reads measurement schema_version {MEASUREMENTS_SCHEMA_VERSION}",
                    path.display()
                ));
            }
            None => {
                return Err(format!(
                    "{} has no versioned measurement contract; regenerate it with `animsmith measure --format json`",
                    path.display()
                ));
            }
        }
        if measurements.schema.as_deref() != Some(MEASUREMENTS_SCHEMA_ID) {
            return Err(format!(
                "{} does not identify measurement contract {MEASUREMENTS_SCHEMA_ID}; regenerate it with `animsmith measure --format json`",
                path.display()
            ));
        }
        return measurements
            .clips
            .ok_or_else(|| format!("{} measurement contract has no `clips` map", path.display()));
    }
    let doc = load(path)?;
    let roles = resolve_configured_roles(&doc.skeleton, &config.rig);
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
