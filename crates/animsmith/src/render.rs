//! CLI output serializers.
//!
//! `docs/output.md` frames the JSON envelope as the machine-readable
//! source of truth, with text and Markdown as presentation-only views
//! over the same [`LintFileReport`] model. This module houses the shared JSON
//! serializer and every human-readable command renderer. Command dispatch
//! passes structured results here and retains execution, write, and exit
//! policy; this module owns presentation and escaping. Future serializers
//! (SARIF, GitLab Code Quality, JUnit, CSV) belong here alongside JSON.

use animsmith_core::diff::MetricDelta;
use animsmith_core::model::Clip;
use animsmith_core::{
    CoverageGap, CoverageGapCode, Document, Finding, LintFileReport, MeasureFileReport,
    ResolvedRoles, Severity,
};
use animsmith_gltf::fix::{FixReport, Repair};
use animsmith_gltf::write::WriteSummary;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Default)]
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

/// Serialize any envelope as pretty JSON — the machine-readable contract
/// shared by `measure`, `lint`, and `diff`.
pub(crate) fn print_json<T: Serialize>(value: &T) {
    let out = serde_json::to_string_pretty(value);
    println!("{}", out.expect("report serializes"));
}

/// Render one operator error for stderr.
pub(crate) fn render_operator_error(message: &str) -> String {
    format!("animsmith: {}\n", text_atom(message))
}

fn render_aabb_size(label: &str, bounds: &animsmith_core::measure::Aabb) -> String {
    let size = [
        bounds.max[0] - bounds.min[0],
        bounds.max[1] - bounds.min[1],
        bounds.max[2] - bounds.min[2],
    ];
    format!(" {label} {:.3}×{:.3}×{:.3}", size[0], size[1], size[2])
}

fn render_point(label: &str, point: &[f32; 3]) -> String {
    format!(
        " {label} ({:.3}, {:.3}, {:.3})",
        point[0], point[1], point[2]
    )
}

/// Yield measurement reports in the presentation-only text format, one
/// escaped line at a time without retaining the full transcript.
pub(crate) fn render_measure_text(
    reports: &[MeasureFileReport],
) -> impl Iterator<Item = String> + '_ {
    reports.iter().flat_map(|report| {
        std::iter::once(format!("{}:", text_atom(report.path())))
            .chain(
                report
                    .measurements()
                    .clips()
                    .iter()
                    .map(|(clip, measurement)| {
                        let seam = measurement
                            .loop_seam_ratio
                            .map(|ratio| format!(" seam×{ratio:.2}"))
                            .unwrap_or_default();
                        let continuity = measurement
                            .loop_continuity
                            .as_ref()
                            .filter(|continuity| !continuity.bones.is_empty())
                            .map(|continuity| {
                                let (position, rotation, velocity) = continuity.bones.iter().fold(
                                    (0.0f64, 0.0f64, 0.0f64),
                                    |(position, rotation, velocity), bone| {
                                        (
                                            position.max(bone.position_delta_m),
                                            rotation.max(bone.rotation_delta_deg),
                                            velocity.max(bone.seam_velocity_delta_mps),
                                        )
                                    },
                                );
                                format!(
                                    " loop Δp={:.2}cm Δr={rotation:.2}° Δv={velocity:.3}m/s",
                                    position * 100.0
                                )
                            })
                            .unwrap_or_default();
                        let gait = measurement
                            .gait
                            .as_ref()
                            .and_then(|gait| gait.phase.map(|phase| (phase, gait.lr_amplitude_m)))
                            .map(|(phase, amplitude)| {
                                format!(" gait φ={phase:.2} ({:.1}cm)", amplitude * 100.0)
                            })
                            .unwrap_or_default();
                        format!(
                            "  {}: {:.3}s, {} frames, {} animated bones{continuity}{seam}{gait}",
                            text_atom(clip),
                            measurement.duration_s,
                            measurement.frame_count,
                            measurement.animated_bones.len()
                        )
                    }),
            )
            .chain(std::iter::once({
                let assets = report.measurements().assets();
                let coverage = match assets.material_resource_coverage {
                    animsmith_core::model::MaterialResourceCoverage::Complete => "complete",
                    animsmith_core::model::MaterialResourceCoverage::Unavailable => "unavailable",
                };
                format!(
                    "  material resources: {} materials, {} textures, {} images ({coverage})",
                    assets.material_definitions.len(),
                    assets.textures.len(),
                    assets.images.len(),
                )
            }))
            .chain(report.measurements().assets().mesh_definitions.iter().map(|mesh| {
                let bbox = mesh
                    .geometry_aabb
                    .as_ref()
                    .map(|bounds| render_aabb_size("geometry bbox", bounds))
                    .unwrap_or_else(|| " geometry bbox unavailable".into());
                let centroid = mesh
                    .geometry_centroid
                    .as_ref()
                    .map(|point| render_point("geometry centroid", point))
                    .unwrap_or_else(|| " geometry centroid unavailable".into());
                let skin = match (mesh.weight_sum_min, mesh.weight_sum_max) {
                    (Some(min), Some(max)) => format!(
                        ", ≤{} joints/vtx, weight-sum {min:.3}–{max:.3}",
                        mesh.max_joints_per_vertex
                    ),
                    _ => String::new(),
                };
                let additional_influences = if mesh.additional_influence_sets.is_empty() {
                    String::new()
                } else {
                    let sets = mesh
                        .additional_influence_sets
                        .iter()
                        .map(|set| {
                            let presence = match (set.joints_present, set.weights_present) {
                                (true, true) => {
                                    format!("JOINTS_{} + WEIGHTS_{}", set.set_index, set.set_index)
                                }
                                (true, false) => format!("JOINTS_{}", set.set_index),
                                (false, true) => format!("WEIGHTS_{}", set.set_index),
                                (false, false) => format!("set {}", set.set_index),
                            };
                            let mismatch = match (
                                set.joints_without_weights_present,
                                set.weights_without_joints_present,
                            ) {
                                (true, true) => " (also JOINTS-only and WEIGHTS-only primitives)",
                                (true, false) => " (also JOINTS-only primitives)",
                                (false, true) => " (also WEIGHTS-only primitives)",
                                (false, false) => "",
                            };
                            format!("{presence}{mismatch}")
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(", additional influence sets: {sets}")
                };
                format!(
                    "  mesh definition #{} {}: {} verts{bbox}{centroid}{skin}{additional_influences}",
                    mesh.mesh_index,
                    text_atom(&mesh.name),
                    mesh.vertex_count
                )
            }))
            .chain(report.measurements().assets().node_instances.iter().map(|instance| {
                let bounds = instance
                    .static_node_world_aabb
                    .as_ref()
                    .map(|bounds| render_aabb_size("static node-world bbox", bounds))
                    .unwrap_or_else(|| {
                        let reason = match instance.static_node_world_aabb_unavailable_reason {
                            Some(animsmith_core::measure::StaticNodeAabbUnavailableReason::NoFinitePositions) => "no finite positions",
                            Some(animsmith_core::measure::StaticNodeAabbUnavailableReason::SkinnedDeformationExcluded) => "skinned deformation excluded",
                            Some(animsmith_core::measure::StaticNodeAabbUnavailableReason::NonFiniteTransform) => "non-finite transform",
                            Some(_) => "unsupported reason",
                            None => "unknown reason",
                        };
                        format!(" static node-world bbox unavailable ({reason})")
                    });
                format!(
                    "  node instance #{} {} -> mesh #{}:{bounds}",
                    instance.node_index,
                    text_atom(&instance.node_name),
                    instance.mesh_index
                )
            }))
            .chain(report.measurements().assets().scenes.iter().map(|scene| {
                let default = if report.measurements().assets().default_scene_index
                    == Some(scene.scene_index)
                {
                    " [default]"
                } else {
                    ""
                };
                let name = scene
                    .name
                    .as_deref()
                    .map(|name| format!(" {}", text_atom(name)))
                    .unwrap_or_default();
                let bounds = scene
                    .static_scene_world_aabb
                    .as_ref()
                    .map(|bounds| render_aabb_size("static scene-world bbox", bounds))
                    .unwrap_or_else(|| " static scene-world bbox unavailable".into());
                let excluded = if scene.excluded_instance_count > 0 {
                    format!(", {} excluded", scene.excluded_instance_count)
                } else {
                    String::new()
                };
                format!(
                    "  scene #{}{}{}: {} instances{bounds}{excluded}",
                    scene.scene_index, name, default, scene.instance_count
                )
            }))
    })
}

/// Yield significant measurement deltas in the presentation-only text format,
/// one escaped line at a time without retaining the full transcript.
pub(crate) fn render_diff_text(deltas: &[MetricDelta]) -> impl Iterator<Item = String> + '_ {
    deltas
        .is_empty()
        .then(|| "no significant movement".to_string())
        .into_iter()
        .chain(deltas.iter().map(|delta| {
            let values = match (delta.before, delta.after) {
                (Some(before), Some(after)) => format!(" {before:.4} -> {after:.4}"),
                (Some(before), None) => format!(" {before:.4} -> (gone)"),
                (None, Some(after)) => format!(" (none) -> {after:.4}"),
                (None, None) => String::new(),
            };
            format!(
                "  {} {}: {}{values}",
                text_atom(&delta.clip),
                text_atom(&delta.metric),
                text_atom(&delta.note)
            )
        }))
        .chain(std::iter::once(format!(
            "{} significant change(s)",
            deltas.len()
        )))
}

/// Yield an inspected document and resolved rig roles one escaped line at a
/// time. Bone depths are derived in one parent-before-child pass.
pub(crate) fn render_inspect<'a>(
    doc: &'a Document,
    roles: &'a ResolvedRoles,
) -> impl Iterator<Item = String> + 'a {
    let source = doc
        .source
        .path
        .iter()
        .map(|path| text_atom(path).into_owned());
    let profile = std::iter::once(if roles.is_empty() {
        "rig profile: none detected".to_string()
    } else {
        format!(
            "rig profile: {} ({} roles)",
            text_atom(&roles.profile),
            roles.len()
        )
    });
    let role_lines = roles.iter().map(|(role, bone)| {
        format!(
            "  {:<12} -> {}",
            role.as_str(),
            text_atom(&doc.skeleton.bones[bone].name)
        )
    });
    let skeleton = std::iter::once(format!("skeleton: {} bones", doc.skeleton.bones.len()));
    let bone_lines = doc.skeleton.bones.iter().scan(
        Vec::with_capacity(doc.skeleton.bones.len()),
        |depths, bone| {
            let depth = bone.parent.map_or(0, |parent| depths[parent] + 1);
            depths.push(depth);
            let skinned = if bone.inverse_bind.is_some() {
                " [skinned]"
            } else {
                ""
            };
            Some(format!(
                "  {}{}{}",
                "  ".repeat(depth),
                text_atom(&bone.name),
                skinned
            ))
        },
    );
    let material_name_counts =
        doc.assets
            .materials
            .iter()
            .fold(BTreeMap::<&str, usize>::new(), |mut counts, material| {
                *counts.entry(material.name.as_str()).or_default() += 1;
                counts
            });
    let materials = std::iter::once(format!("materials: {}", doc.assets.materials.len()));
    let material_lines = doc
        .assets
        .materials
        .iter()
        .enumerate()
        .map(move |(index, material)| {
            let ambiguity = match material_name_counts.get(material.name.as_str()) {
                Some(count) if *count > 1 => {
                    format!(" [ambiguous: {count} materials share this name]")
                }
                _ => String::new(),
            };
            format!("  #{index} {}{ambiguity}", quoted_text_atom(&material.name))
        });
    let node_name_counts =
        doc.skeleton
            .bones
            .iter()
            .fold(BTreeMap::<&str, usize>::new(), |mut counts, bone| {
                *counts.entry(bone.name.as_str()).or_default() += 1;
                counts
            });
    let mesh_instances = std::iter::once(format!("mesh instances: {}", doc.assets.instances.len()));
    let mesh_instance_lines = doc.assets.instances.iter().flat_map(move |instance| {
        let (node_name, ambiguity) = doc.skeleton.bones.get(instance.node).map_or_else(
            || (format!("<missing node #{}>", instance.node), String::new()),
            |bone| {
                let ambiguity = match node_name_counts.get(bone.name.as_str()) {
                    Some(count) if *count > 1 => {
                        format!(" [ambiguous: {count} skeleton nodes share this name]")
                    }
                    _ => String::new(),
                };
                (quoted_text_atom(&bone.name), ambiguity)
            },
        );
        let mut lines = vec![format!("  node {node_name}{ambiguity}")];
        lines.push(format!("    source node: #{}", instance.source_node_index));
        if let Some(mesh) = doc.assets.meshes.get(instance.mesh) {
            lines.push(format!(
                "    mesh: #{} {} (source mesh #{})",
                instance.mesh,
                quoted_text_atom(&mesh.name),
                mesh.source_mesh_index
            ));
            lines.push(format!(
                "    skin: {}",
                if instance.skin_joints.is_empty() {
                    "unskinned"
                } else {
                    "skinned"
                }
            ));
            if mesh.primitives.is_empty() {
                lines.push("    primitives: none".to_string());
            } else {
                for (primitive_index, primitive) in mesh.primitives.iter().enumerate() {
                    let material = primitive.material.map_or_else(
                        || "no material".to_string(),
                        |material_index| {
                            doc.assets.materials.get(material_index).map_or_else(
                                || format!("missing material #{material_index}"),
                                |material| {
                                    format!(
                                        "material #{material_index} {}",
                                        quoted_text_atom(&material.name)
                                    )
                                },
                            )
                        },
                    );
                    lines.push(format!("    primitive #{primitive_index}: {material}"));
                }
            }
        } else {
            lines.push(format!("    mesh: <missing mesh #{}>", instance.mesh));
            lines.push(format!(
                "    skin: {}",
                if instance.skin_joints.is_empty() {
                    "unskinned"
                } else {
                    "skinned"
                }
            ));
            lines.push("    primitives: unavailable".to_string());
        }
        lines
    });
    let clips = std::iter::once(format!("clips: {}", doc.clips.len()));
    let clip_lines = doc.clips.iter().map(|clip| {
        let keys = clip
            .tracks
            .iter()
            .map(|track| track.key_count())
            .max()
            .unwrap_or(0);
        format!(
            "  {}: {:.3}s, {} tracks, {} keys max",
            text_atom(&clip.name),
            clip.duration_s,
            clip.tracks.len(),
            keys
        )
    });

    source
        .chain(profile)
        .chain(role_lines)
        .chain(skeleton)
        .chain(bone_lines)
        .chain(materials)
        .chain(material_lines)
        .chain(mesh_instances)
        .chain(mesh_instance_lines)
        .chain(clips)
        .chain(clip_lines)
}

/// Render one report artifact write summary.
#[cfg(feature = "report")]
pub(crate) fn render_report_written(
    output: &Path,
    clip_count: usize,
    finding_count: usize,
    html_bytes: usize,
) -> String {
    format!(
        "wrote {} ({clip_count} clip(s), {finding_count} finding(s), {:.1} MB)\n",
        text_atom(&output.display().to_string()),
        html_bytes as f64 / 1e6
    )
}

/// Render the message emitted after slicing one clip.
pub(crate) fn render_transform_slice(clip: &Clip, start: f64, end: f64) -> String {
    let keys = clip
        .tracks
        .iter()
        .map(|track| track.key_count())
        .max()
        .unwrap_or(0);
    format!(
        "  sliced '{}' to [{start}:{end}]s ({keys} keys max)\n",
        text_atom(&clip.name)
    )
}

/// Render the message emitted after hold-extending one clip.
pub(crate) fn render_transform_hold_extend(clip: &Clip, hold_s: f64) -> String {
    format!("  hold-extended '{}' by {hold_s}s\n", text_atom(&clip.name))
}

/// Render the message emitted after successful gait-anchor alignment.
pub(crate) fn render_transform_gait_anchor(
    clip_name: &str,
    phase_before: f64,
    phase_after: f64,
    frame_offset: i32,
    seam_after: Option<f64>,
) -> String {
    let seam = seam_after
        .map(|seam| format!("{seam:.2}"))
        .unwrap_or_else(|| "n/a".into());
    format!(
        "  gait-anchored '{}': phase {phase_before:.3} -> {phase_after:.3} (offset {frame_offset}, seam {seam})\n",
        text_atom(clip_name)
    )
}

/// Render the message emitted when gait-anchor alignment is skipped.
pub(crate) fn render_transform_gait_anchor_skipped(clip_name: &str, reason: &str) -> String {
    format!(
        "  gait-anchor skipped '{}': {}\n",
        text_atom(clip_name),
        text_atom(reason)
    )
}

fn repair_action(repair: Repair) -> &'static str {
    match repair {
        Repair::QuatNorm => "unit-normalized",
        Repair::QuatFlip => "hemisphere-normalized",
        _ => "repaired",
    }
}

/// Yield one byte-surgical repair pass one escaped line at a time.
/// `target = None` means dry-run.
pub(crate) fn render_fix_report<'a>(
    repair: Repair,
    report: &'a FixReport,
    target: Option<&'a Path>,
) -> impl Iterator<Item = String> + 'a {
    let verb = if target.is_none() {
        "would fix"
    } else {
        "fixed"
    };
    let tracks = report.tracks.iter().map(move |track| {
        format!(
            "  {verb}[{}] clip '{}' bone '{}': {} key(s) {}",
            repair.id(),
            text_atom(&track.clip),
            text_atom(&track.bone),
            track.fixed_keys,
            repair_action(repair)
        )
    });
    let skipped = report
        .skipped
        .iter()
        .map(move |skipped| format!("  skipped[{}]: {}", repair.id(), text_atom(skipped)));
    let summary = std::iter::once_with(move || {
        let destination = target
            .map(|path| text_atom(&path.display().to_string()).into_owned())
            .unwrap_or_else(|| "no output written".into());
        format!(
            "{} key(s) {} across {} track(s) -> {destination}",
            report.total_fixed(),
            if target.is_none() {
                "would be fixed"
            } else {
                "fixed"
            },
            report.tracks.len(),
        )
    });
    tracks.chain(skipped).chain(summary)
}

/// Render the shared `transform`/`convert` artifact write summary.
pub(crate) fn render_write_summary(output: &Path, summary: &WriteSummary) -> String {
    use std::fmt::Write as _;

    let mut out = format!(
        "wrote {} ({} node(s), {} clip(s), {} mesh(es) / {} position(s), {} material(s))",
        text_atom(&output.display().to_string()),
        summary.nodes,
        summary.animations,
        summary.meshes,
        summary.primitive_positions,
        summary.materials,
    );
    if summary.clips_without_writable_tracks > 0 {
        let _ = write!(
            out,
            "; dropped {} clip(s) with no writable tracks",
            summary.clips_without_writable_tracks
        );
    }
    out.push('\n');
    out
}

/// Render human-readable one-line-per-finding text output for `lint`.
pub(crate) fn render_text(reports: &[LintFileReport], suppressed: &[String]) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut notes = 0usize;
    let mut gaps = 0usize;
    for report in reports {
        let findings = sorted_findings(report, suppressed);
        let coverage_groups = coverage_gap_groups(report);
        let file_gap_count: usize = coverage_groups.iter().map(|group| group.gaps.len()).sum();
        if findings.is_empty() && coverage_groups.is_empty() {
            let _ = writeln!(out, "{}: clean", text_atom(report.path()));
            continue;
        }
        let _ = writeln!(out, "{}:", text_atom(report.path()));
        for f in &findings {
            match f.severity {
                Severity::Error => errors += 1,
                Severity::Warning => warnings += 1,
                Severity::Note => notes += 1,
            }
            let mut location = String::new();
            if let Some(clip) = &f.clip {
                location.push_str(&format!(" clip '{}'", text_atom(clip)));
            }
            if let Some(bone) = &f.bone {
                location.push_str(&format!(" bone '{}'", text_atom(bone)));
            }
            if let Some(t) = f.time_s {
                location.push_str(&format!(" @{t:.3}s"));
            }
            let mut detail = String::new();
            if let (Some(measured), Some(expected)) = (&f.measured, &f.expected) {
                detail = format!(
                    " (measured {}, expected {})",
                    text_atom(&measured.to_string()),
                    text_atom(&expected.to_string())
                );
            } else if let Some(measured) = &f.measured {
                detail = format!(" (measured {})", text_atom(&measured.to_string()));
            }
            let _ = writeln!(
                out,
                "  {}[{}]{}: {}{}",
                f.severity,
                f.check_id,
                location,
                text_atom(&f.message),
                detail
            );
        }
        gaps += file_gap_count;
        for group in coverage_groups {
            let summary = group.summary();
            let mut context = Vec::new();
            if !summary.scopes.is_empty() {
                context.push(format!("scopes: {}", text_atom(&summary.scopes)));
            }
            if !summary.subjects.is_empty() {
                context.push(format!("subjects: {}", text_atom(&summary.subjects)));
            }
            let context = if context.is_empty() {
                String::new()
            } else {
                format!(" ({})", context.join("; "))
            };
            let _ = writeln!(
                out,
                "  coverage[{}] {} ×{}{}: {}",
                group.check_id,
                group.code,
                group.gaps.len(),
                context,
                text_atom(&summary.messages),
            );
        }
    }
    let _ = writeln!(
        out,
        "{errors} error(s), {warnings} warning(s), {notes} note(s), {gaps} coverage gap(s)"
    );
    out
}

fn sorted_findings<'a>(report: &'a LintFileReport, suppressed: &[String]) -> Vec<&'a Finding> {
    let mut findings: Vec<_> = report
        .checks()
        .iter()
        .flat_map(|check| check.findings())
        .filter(|finding| !suppressed.iter().any(|id| id == finding.check_id))
        .collect();
    findings.sort_by(|a, b| {
        (a.clip.as_deref(), std::cmp::Reverse(a.severity))
            .cmp(&(b.clip.as_deref(), std::cmp::Reverse(b.severity)))
    });
    findings
}

struct CoverageGapGroup<'a> {
    check_id: &'a str,
    code: CoverageGapCode,
    gaps: Vec<&'a CoverageGap>,
}

struct CoverageGapSummary {
    scopes: String,
    subjects: String,
    messages: String,
}

impl CoverageGapGroup<'_> {
    fn summary(&self) -> CoverageGapSummary {
        CoverageGapSummary {
            scopes: summarized_group_values(
                self.gaps
                    .iter()
                    .filter_map(|gap| gap.scope.as_ref().map(|scope| scope.code.as_str())),
            ),
            subjects: summarized_group_values(self.gaps.iter().filter_map(|gap| {
                gap.scope
                    .as_ref()
                    .and_then(|scope| scope.subject.as_deref())
            })),
            messages: summarized_group_values(self.gaps.iter().map(|gap| gap.message.as_str())),
        }
    }
}

fn coverage_gap_groups(report: &LintFileReport) -> Vec<CoverageGapGroup<'_>> {
    let mut groups: BTreeMap<(&str, CoverageGapCode), Vec<&CoverageGap>> = BTreeMap::new();
    for check in report.checks() {
        for gap in check.gaps() {
            groups
                .entry((check.check_id(), gap.code))
                .or_default()
                .push(gap);
        }
    }
    groups
        .into_iter()
        .map(|((check_id, code), gaps)| CoverageGapGroup {
            check_id,
            code,
            gaps,
        })
        .collect()
}

fn summarized_group_values<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    const DISPLAY_LIMIT: usize = 5;
    let values: BTreeSet<_> = values.into_iter().collect();
    let shown = values
        .iter()
        .take(DISPLAY_LIMIT)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    if values.len() > DISPLAY_LIMIT {
        format!("{shown}, … +{} more", values.len() - DISPLAY_LIMIT)
    } else {
        shown
    }
}

/// Return whether a character can alter terminal or visual text presentation
/// without occupying an ordinary printable cell.
fn is_presentation_control(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{00ad}'
                | '\u{034f}'
                | '\u{061c}'
                | '\u{180b}'..='\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{fe00}'..='\u{fe0f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{1bca0}'..='\u{1bca3}'
                | '\u{1d173}'..='\u{1d17a}'
                | '\u{e0000}'..='\u{e007f}'
                | '\u{e0100}'..='\u{e01ef}'
        )
}

/// Escape terminal controls, Unicode line separators, and bidirectional
/// formatting characters while leaving ordinary Unicode text readable. Each
/// untrusted value therefore remains one visibly ordered physical line and
/// cannot inject ANSI terminal commands.
fn text_atom(text: &str) -> Cow<'_, str> {
    if !text.chars().any(is_presentation_control) {
        return Cow::Borrowed(text);
    }
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if is_presentation_control(ch) {
            escaped.extend(ch.escape_default());
        } else {
            escaped.push(ch);
        }
    }
    Cow::Owned(escaped)
}

/// Quote an exact author-provided identifier so empty names and leading or
/// trailing whitespace remain visible. The result is also a valid TOML basic
/// string, so selectors can be copied directly into animsmith recipes.
fn quoted_text_atom(text: &str) -> String {
    use std::fmt::Write as _;

    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for ch in text.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{8}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{c}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            ch if is_presentation_control(ch) => {
                let scalar = u32::from(ch);
                if scalar <= u32::from(u16::MAX) {
                    let _ = write!(escaped, "\\u{scalar:04X}");
                } else {
                    let _ = write!(escaped, "\\U{scalar:08X}");
                }
            }
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

/// The finding-count threshold at or below which a file's list stays
/// expanded; a file carrying more than this many findings is collapsed
/// behind a closed `<details>` element instead. Short lists stay open so
/// a reviewer sees them without a click; long lists collapse so one noisy
/// asset does not bury the rest of a CI comment. Kept in sync with the
/// "more than ten" boundary documented in `docs/cli.md`.
const MARKDOWN_COLLAPSE_AT: usize = 10;

/// Render findings as GitHub/GitLab-flavored Markdown for CI comments and
/// asset-review threads. Presentation-only: the JSON output is the
/// machine-readable contract, and this layout carries no stability guarantees.
/// It mirrors text output's severity, check id, location, measured/expected
/// values, and per-clip grouping as tables inside per-file sections. Keeping
/// it side-effect free lets the per-clip
/// grouping, cell escaping, collapse threshold, and summary tallies be
/// unit-tested directly without spawning the CLI.
///
/// Findings are sorted here before grouping so callers cannot accidentally
/// emit repeated per-clip headers from an interleaved input slice.
pub(crate) fn render_markdown(reports: &[LintFileReport], suppressed: &[String]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut total = FindingSummary::default();
    let mut total_gaps = 0usize;

    let _ = writeln!(out, "## animsmith lint\n");

    for report in reports {
        let findings = sorted_findings(report, suppressed);
        let gap_groups = coverage_gap_groups(report);
        let gap_count: usize = gap_groups.iter().map(|group| group.gaps.len()).sum();
        if findings.is_empty() && gap_groups.is_empty() {
            let _ = writeln!(out, "### `{}`\n", md_cell(report.path()));
            let _ = writeln!(out, "✅ Clean — no findings or coverage gaps.\n");
            continue;
        }

        let mut file = FindingSummary::default();
        for f in &findings {
            file.add(f.severity);
        }
        total.error += file.error;
        total.warning += file.warning;
        total.note += file.note;
        total_gaps += gap_count;

        let _ = writeln!(out, "### `{}`\n", md_cell(report.path()));
        let _ = writeln!(out, "{}\n", severity_line(&file));

        if !findings.is_empty() {
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
            for f in &findings {
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

        if !gap_groups.is_empty() {
            let _ = writeln!(out, "<details open>");
            let _ = writeln!(
                out,
                "<summary><strong>{} coverage gap(s)</strong></summary>\n",
                gap_count
            );
            let _ = writeln!(
                out,
                "| Check | Code | Count | Scopes | Subjects | Messages |"
            );
            let _ = writeln!(out, "| --- | --- | ---: | --- | --- | --- |");
            for group in gap_groups {
                let summary = group.summary();
                let _ = writeln!(
                    out,
                    "| `{}` | `{}` | {} | `{}` | `{}` | `{}` |",
                    group.check_id,
                    group.code,
                    group.gaps.len(),
                    if summary.scopes.is_empty() {
                        "—".into()
                    } else {
                        md_cell(&summary.scopes)
                    },
                    if summary.subjects.is_empty() {
                        "—".into()
                    } else {
                        md_cell(&summary.subjects)
                    },
                    md_cell(&summary.messages),
                );
            }
            let _ = writeln!(out, "\n</details>\n");
        }
    }

    let files = reports.len();
    let file_word = if files == 1 { "file" } else { "files" };
    let _ = writeln!(out, "---\n");
    let _ = writeln!(
        out,
        "**{files} {file_word}** — {} · {total_gaps} coverage gap(s)",
        severity_line(&total)
    );
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
/// - Backslash-escape the pipe (and pre-double authored backslashes so an
///   authored `\|` cannot re-form an unescaped delimiter), flatten physical
///   and Unicode line separators, and visibly escape remaining presentation
///   controls so the value stays inside one ordered table cell.
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
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\r' | '\n' | '\u{2028}' | '\u{2029}' => escaped.push(' '),
            ch if is_presentation_control(ch) => escaped.extend(ch.escape_default()),
            '\\' => escaped.push_str("\\\\"),
            '`' => escaped.push('\''),
            '|' => escaped.push_str("\\|"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        Bone, CheckEvaluation, CheckOutput, Clip, CoverageGap, CoverageGapCode, Document,
        EvaluationScope, EvaluationScopeCode, Finding, LintFileReport, MeasureFileReport,
        MeasurementContract, ResolvedRoles, RigInfo, Role, SourceInfo, Track, TrackValues,
        Transform,
    };
    use animsmith_core::{Interpolation, Property};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn evaluation(
        check_id: &'static str,
        findings: Vec<Finding>,
        evaluated_scopes: Vec<EvaluationScope>,
        gaps: Vec<CoverageGap>,
    ) -> CheckEvaluation {
        CheckEvaluation::evaluated(
            check_id,
            CheckOutput::from_coverage(findings, evaluated_scopes, gaps),
        )
        .expect("test finding ids match their parent check")
    }

    fn report(path: &str, findings: Vec<Finding>) -> LintFileReport {
        let mut by_check = BTreeMap::<_, Vec<_>>::new();
        for finding in findings {
            by_check.entry(finding.check_id).or_default().push(finding);
        }
        let checks = if by_check.is_empty() {
            vec![evaluation("test", Vec::new(), Vec::new(), Vec::new())]
        } else {
            by_check
                .into_iter()
                .map(|(check_id, findings)| evaluation(check_id, findings, Vec::new(), Vec::new()))
                .collect()
        };
        report_with_checks(path, checks)
    }

    fn report_with_checks(path: &str, checks: Vec<CheckEvaluation>) -> LintFileReport {
        let doc = Document::default();
        LintFileReport::new(
            path,
            RigInfo::from_resolved(&doc, &ResolvedRoles::default())
                .expect("empty roles match an empty document"),
            checks,
            MeasurementContract::new(
                BTreeMap::new(),
                animsmith_core::measure::AssetMeasurements::default(),
            )
            .expect("empty measurements are valid"),
        )
    }

    fn empty_rig() -> RigInfo {
        let doc = Document::default();
        RigInfo::from_resolved(&doc, &ResolvedRoles::default())
            .expect("empty roles match an empty document")
    }

    #[test]
    fn measure_renderer_owns_layout_and_escaping() {
        let clips = serde_json::from_value(json!({
            "walk\nclip": {
                "duration_s": 1.0,
                "frame_count": 2,
                "animated_bones": ["hips"],
                "bone_rotation_range_deg": {},
                "loop_continuity": { "bones": [{
                    "bone_index": 0,
                    "bone_name": "hips",
                    "position_delta_m": 0.012,
                    "rotation_delta_deg": 2.5,
                    "seam_velocity_delta_mps": 0.3456
                }] },
                "loop_seam_ratio": 0.25,
                "gait": { "phase": 0.5, "lr_amplitude_m": 0.1 }
            }
        }))
        .expect("clip measurements deserialize");
        let assets = serde_json::from_value(json!({
            "material_resource_coverage": "complete",
            "material_definitions": [{
                "material_index": 0,
                "name": "body material",
                "texture_bindings": [{ "slot": "base_color", "texture_index": 0 }]
            }],
            "textures": [{ "texture_index": 0, "image_index": 0 }],
            "images": [{
                "image_index": 0,
                "source_kind": "embedded",
                "detected_container": "png",
                "width": 1,
                "height": 1,
                "channel_count": 4,
                "decoded_color_type": "rgba8"
            }],
            "mesh_definitions": [{
                "mesh_index": 7,
                "name": "body\nmesh",
                "vertex_count": 3,
                "geometry_aabb": { "min": [0.0, 0.0, 0.0], "max": [1.0, 2.0, 3.0] },
                "geometry_centroid": [0.25, 1.0, -0.5],
                "max_joints_per_vertex": 4,
                "weight_sum_min": 0.9,
                "weight_sum_max": 1.1,
                "additional_influence_sets": [
                    { "set_index": 1, "joints_present": true, "weights_present": true, "joints_without_weights_present": true, "weights_without_joints_present": true },
                    { "set_index": 2, "joints_present": true, "weights_present": false, "joints_without_weights_present": true, "weights_without_joints_present": false },
                    { "set_index": 3, "joints_present": false, "weights_present": true, "joints_without_weights_present": false, "weights_without_joints_present": true }
                ]
            }],
            "node_instances": [{
                "node_index": 9,
                "node_name": "body\nnode",
                "mesh_index": 7,
                "static_node_world_aabb": { "min": [2.0, 0.0, 0.0], "max": [3.0, 2.0, 3.0] }
            }],
            "scenes": [{
                "scene_index": 2,
                "name": "main\nscene",
                "instance_count": 1,
                "static_scene_world_aabb": { "min": [2.0, 0.0, 0.0], "max": [3.0, 2.0, 3.0] },
                "excluded_instance_count": 0
            }],
            "default_scene_index": 2
        }))
        .expect("asset measurements deserialize");
        let measurements = MeasurementContract::new(clips, assets).expect("finite measurements");
        let reports = [MeasureFileReport::new(
            "asset\npath.glb",
            empty_rig(),
            measurements,
        )];

        assert_eq!(
            render_measure_text(&reports).collect::<Vec<_>>(),
            vec![
                "asset\\npath.glb:",
                "  walk\\nclip: 1.000s, 2 frames, 1 animated bones loop Δp=1.20cm Δr=2.50° Δv=0.346m/s seam×0.25 gait φ=0.50 (10.0cm)",
                "  material resources: 1 materials, 1 textures, 1 images (complete)",
                "  mesh definition #7 body\\nmesh: 3 verts geometry bbox 1.000×2.000×3.000 geometry centroid (0.250, 1.000, -0.500), ≤4 joints/vtx, weight-sum 0.900–1.100, additional influence sets: JOINTS_1 + WEIGHTS_1 (also JOINTS-only and WEIGHTS-only primitives), JOINTS_2 (also JOINTS-only primitives), WEIGHTS_3 (also WEIGHTS-only primitives)",
                "  node instance #9 body\\nnode -> mesh #7: static node-world bbox 1.000×2.000×3.000",
                "  scene #2 main\\nscene [default]: 1 instances static scene-world bbox 1.000×2.000×3.000",
            ]
        );
    }

    #[test]
    fn measure_renderer_preserves_optional_absence_and_report_order() {
        let clips = serde_json::from_value(json!({
            "idle": {
                "duration_s": 2.0,
                "frame_count": 1,
                "animated_bones": [],
                "bone_rotation_range_deg": {}
            }
        }))
        .expect("clip measurements deserialize");
        let assets = serde_json::from_value(json!({
            "material_resource_coverage": "unavailable",
            "material_definitions": [],
            "textures": [],
            "images": [],
            "mesh_definitions": [{
                "mesh_index": 0,
                "name": "plain",
                "vertex_count": 0,
                "max_joints_per_vertex": 0,
                "additional_influence_sets": []
            }],
            "node_instances": [],
            "scenes": []
        }))
        .expect("asset measurements deserialize");
        let reports = [
            MeasureFileReport::new(
                "first.glb",
                empty_rig(),
                MeasurementContract::new(clips, assets).expect("finite measurements"),
            ),
            MeasureFileReport::new(
                "second.glb",
                empty_rig(),
                MeasurementContract::new(
                    BTreeMap::new(),
                    animsmith_core::measure::AssetMeasurements::default(),
                )
                .expect("empty measurements"),
            ),
        ];

        assert_eq!(
            render_measure_text(&reports).collect::<Vec<_>>(),
            vec![
                "first.glb:",
                "  idle: 2.000s, 1 frames, 0 animated bones",
                "  material resources: 0 materials, 0 textures, 0 images (unavailable)",
                "  mesh definition #0 plain: 0 verts geometry bbox unavailable geometry centroid unavailable",
                "second.glb:",
                "  material resources: 0 materials, 0 textures, 0 images (unavailable)",
            ]
        );
    }

    #[test]
    fn diff_renderer_owns_clean_dirty_layout_and_escaping() {
        assert_eq!(
            render_diff_text(&[]).collect::<Vec<_>>(),
            vec!["no significant movement", "0 significant change(s)"]
        );

        let before = serde_json::from_value(json!({
            "walk\nclip": {
                "duration_s": 1.0,
                "frame_count": 2,
                "animated_bones": [],
                "bone_rotation_range_deg": {}
            }
        }))
        .expect("before measurements deserialize");
        let after = serde_json::from_value(json!({
            "walk\nclip": {
                "duration_s": 1.1,
                "frame_count": 2,
                "animated_bones": [],
                "bone_rotation_range_deg": {}
            }
        }))
        .expect("after measurements deserialize");
        let deltas = animsmith_core::diff::diff_measurements(&before, &after);
        assert_eq!(
            render_diff_text(&deltas).collect::<Vec<_>>(),
            vec![
                "  walk\\nclip duration_s: moved 1.0000 -> 1.1000",
                "1 significant change(s)",
            ]
        );
    }

    #[test]
    fn diff_renderer_preserves_value_presence_branches_and_delta_order() {
        let before = serde_json::from_value(json!({
            "a_removed": {
                "duration_s": 1.0,
                "frame_count": 1,
                "animated_bones": [],
                "bone_rotation_range_deg": {}
            },
            "b_shared": {
                "duration_s": 1.0,
                "frame_count": 1,
                "animated_bones": [],
                "bone_rotation_range_deg": {},
                "loop_seam_ratio": 1.0
            }
        }))
        .expect("before measurements deserialize");
        let after = serde_json::from_value(json!({
            "b_shared": {
                "duration_s": 1.0,
                "frame_count": 1,
                "animated_bones": [],
                "bone_rotation_range_deg": {},
                "speed_mps": 2.0
            },
            "c_added": {
                "duration_s": 1.0,
                "frame_count": 1,
                "animated_bones": [],
                "bone_rotation_range_deg": {}
            }
        }))
        .expect("after measurements deserialize");
        let deltas = animsmith_core::diff::diff_measurements(&before, &after);

        assert_eq!(
            render_diff_text(&deltas).collect::<Vec<_>>(),
            vec![
                "  a_removed clip: clip removed",
                "  b_shared loop_seam_ratio: disappeared 1.0000 -> (gone)",
                "  b_shared speed_mps: appeared (none) -> 2.0000",
                "  c_added clip: clip added",
                "4 significant change(s)",
            ]
        );
    }

    #[test]
    fn inspect_renderer_owns_tree_layout_and_escaping() {
        let hostile = "asset\nname";
        let doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: vec![
                    Bone {
                        name: "root\nname".into(),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    },
                    Bone {
                        name: "child\nname".into(),
                        parent: Some(0),
                        rest: Transform::IDENTITY,
                        inverse_bind: Some(animsmith_core::glam::Mat4::IDENTITY),
                    },
                ],
            },
            clips: vec![Clip {
                name: "clip\nname".into(),
                duration_s: 1.0,
                tracks: Vec::new(),
            }],
            source: SourceInfo {
                path: Some(hostile.into()),
                format: Some("glb".into()),
            },
            ..Document::default()
        };

        assert_eq!(
            render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>(),
            vec![
                "asset\\nname",
                "rig profile: none detected",
                "skeleton: 2 bones",
                "  root\\nname",
                "    child\\nname [skinned]",
                "materials: 0",
                "mesh instances: 0",
                "clips: 1",
                "  clip\\nname: 1.000s, 0 tracks, 0 keys max",
            ]
        );
    }

    #[test]
    fn inspect_renderer_preserves_resolved_roles_and_deep_hierarchy() {
        let skeleton = animsmith_core::Skeleton {
            bones: vec![
                Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "hips".into(),
                    parent: Some(0),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
                Bone {
                    name: "spine".into(),
                    parent: Some(1),
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                },
            ],
        };
        let roles = ResolvedRoles::from_names(
            &skeleton,
            [
                (Role::Root, "root".to_string()),
                (Role::Hips, "hips".to_string()),
            ],
        );
        let doc = Document {
            skeleton,
            ..Document::default()
        };

        assert_eq!(
            render_inspect(&doc, &roles).collect::<Vec<_>>(),
            vec![
                "rig profile: custom (2 roles)",
                "  root         -> root",
                "  hips         -> hips",
                "skeleton: 3 bones",
                "  root",
                "    hips",
                "      spine",
                "materials: 0",
                "mesh instances: 0",
                "clips: 0",
            ]
        );
    }

    #[test]
    fn inspect_renderer_lists_exact_mesh_instance_and_material_context() {
        use animsmith_core::model::{
            MaterialAsset, MeshAsset, MeshInstance, Primitive, SceneAssets,
        };

        let material = |name: &str| MaterialAsset {
            name: name.into(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        };
        let bone = |name: &str| Bone {
            name: name.into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        };
        let doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: vec![
                    bone("body"),
                    bone("body"),
                    bone("body"),
                    bone("prop"),
                    bone("empty"),
                ],
            },
            assets: SceneAssets {
                materials: vec![material("paint"), material("paint"), material("paint")],
                meshes: vec![
                    MeshAsset {
                        name: "character".into(),
                        source_mesh_index: 7,
                        primitives: vec![
                            Primitive {
                                material: Some(0),
                                ..Primitive::default()
                            },
                            Primitive::default(),
                            Primitive {
                                material: Some(1),
                                ..Primitive::default()
                            },
                        ],
                    },
                    MeshAsset {
                        name: "weapon".into(),
                        source_mesh_index: 9,
                        primitives: vec![Primitive {
                            material: Some(2),
                            ..Primitive::default()
                        }],
                    },
                    MeshAsset {
                        name: "empty-mesh".into(),
                        source_mesh_index: 11,
                        primitives: vec![],
                    },
                ],
                instances: vec![
                    MeshInstance {
                        source_node_index: 4,
                        node: 0,
                        mesh: 0,
                        skin_joints: vec![0, 1],
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 5,
                        node: 1,
                        mesh: 0,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 6,
                        node: 2,
                        mesh: 0,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 7,
                        node: 3,
                        mesh: 1,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 8,
                        node: 4,
                        mesh: 2,
                        ..MeshInstance::default()
                    },
                ],
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let lines = render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>();
        assert_eq!(
            lines,
            [
                "rig profile: none detected",
                "skeleton: 5 bones",
                "  body",
                "  body",
                "  body",
                "  prop",
                "  empty",
                "materials: 3",
                "  #0 \"paint\" [ambiguous: 3 materials share this name]",
                "  #1 \"paint\" [ambiguous: 3 materials share this name]",
                "  #2 \"paint\" [ambiguous: 3 materials share this name]",
                "mesh instances: 5",
                "  node \"body\" [ambiguous: 3 skeleton nodes share this name]",
                "    source node: #4",
                "    mesh: #0 \"character\" (source mesh #7)",
                "    skin: skinned",
                "    primitive #0: material #0 \"paint\"",
                "    primitive #1: no material",
                "    primitive #2: material #1 \"paint\"",
                "  node \"body\" [ambiguous: 3 skeleton nodes share this name]",
                "    source node: #5",
                "    mesh: #0 \"character\" (source mesh #7)",
                "    skin: unskinned",
                "    primitive #0: material #0 \"paint\"",
                "    primitive #1: no material",
                "    primitive #2: material #1 \"paint\"",
                "  node \"body\" [ambiguous: 3 skeleton nodes share this name]",
                "    source node: #6",
                "    mesh: #0 \"character\" (source mesh #7)",
                "    skin: unskinned",
                "    primitive #0: material #0 \"paint\"",
                "    primitive #1: no material",
                "    primitive #2: material #1 \"paint\"",
                "  node \"prop\"",
                "    source node: #7",
                "    mesh: #1 \"weapon\" (source mesh #9)",
                "    skin: unskinned",
                "    primitive #0: material #2 \"paint\"",
                "  node \"empty\"",
                "    source node: #8",
                "    mesh: #2 \"empty-mesh\" (source mesh #11)",
                "    skin: unskinned",
                "    primitives: none",
                "clips: 0",
            ]
        );
    }

    #[test]
    fn quoted_text_atom_keeps_identifier_boundaries_visible_and_safe() {
        assert_eq!(quoted_text_atom("  spaced  "), "\"  spaced  \"");
        assert_eq!(quoted_text_atom("\ttabbed\t"), "\"\\ttabbed\\t\"");
        assert_eq!(
            quoted_text_atom("quote\"slash\\line\n"),
            "\"quote\\\"slash\\\\line\\n\""
        );
        assert_eq!(quoted_text_atom(""), "\"\"");
        let raw = "escape\u{1b}bidi\u{202e}zero\u{200b}join\u{200d}word\u{2060}bom\u{feff}astral\u{e0001}";
        let quoted = quoted_text_atom(raw);
        assert_eq!(
            quoted,
            "\"escape\\u001Bbidi\\u202Ezero\\u200Bjoin\\u200Dword\\u2060bom\\uFEFFastral\\U000E0001\""
        );
        let parsed: toml::Value =
            toml::from_str(&format!("name = {quoted}")).expect("quoted name is valid TOML");
        assert_eq!(parsed["name"].as_str(), Some(raw));
    }

    #[test]
    fn inspect_renderer_does_not_truncate_large_asset_inventories() {
        use animsmith_core::model::{MaterialAsset, MeshAsset, MeshInstance, SceneAssets};

        const COUNT: usize = 257;
        let material = |index| MaterialAsset {
            name: format!("material-{index}"),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        };
        let bones = (0..COUNT)
            .map(|index| Bone {
                name: format!("node-{index}"),
                parent: None,
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect();
        let meshes = (0..COUNT)
            .map(|index| MeshAsset {
                name: format!("mesh-{index}"),
                source_mesh_index: 1_000 + index,
                primitives: vec![],
            })
            .collect();
        let instances = (0..COUNT)
            .map(|index| MeshInstance {
                source_node_index: 2_000 + index,
                node: index,
                mesh: index,
                skin_joints: (index == COUNT - 1)
                    .then_some(vec![0, 1])
                    .unwrap_or_default(),
                ..MeshInstance::default()
            })
            .collect();
        let doc = Document {
            skeleton: animsmith_core::Skeleton { bones },
            assets: SceneAssets {
                materials: (0..COUNT).map(material).collect(),
                meshes,
                instances,
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let lines = render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>();
        assert!(lines.contains(&format!("materials: {COUNT}")));
        assert!(lines.contains(&"  #256 \"material-256\"".to_string()));
        assert!(lines.contains(&format!("mesh instances: {COUNT}")));
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("  node \""))
                .count(),
            COUNT
        );
        let last = lines
            .iter()
            .position(|line| line == "  node \"node-256\"")
            .expect("last instance is present");
        assert_eq!(
            &lines[last..last + 5],
            [
                "  node \"node-256\"",
                "    source node: #2256",
                "    mesh: #256 \"mesh-256\" (source mesh #1256)",
                "    skin: skinned",
                "    primitives: none",
            ]
        );
    }

    #[test]
    fn inspect_renderer_marks_two_and_four_way_name_ambiguity() {
        use animsmith_core::model::{MaterialAsset, MeshInstance, SceneAssets};

        let material = |name: &str| MaterialAsset {
            name: name.into(),
            base_color: [1.0; 4],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture: None,
            normal_texture: None,
            metallic_roughness_texture: None,
            occlusion_texture: None,
        };
        let names = ["pair", "pair", "quad", "quad", "quad", "quad"];
        let doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: names
                    .iter()
                    .map(|name| Bone {
                        name: (*name).into(),
                        parent: None,
                        rest: Transform::IDENTITY,
                        inverse_bind: None,
                    })
                    .collect(),
            },
            assets: SceneAssets {
                materials: names.iter().map(|name| material(name)).collect(),
                instances: (0..names.len())
                    .map(|node| MeshInstance {
                        node,
                        ..MeshInstance::default()
                    })
                    .collect(),
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let lines = render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>();
        for (name, count) in [("pair", 2), ("quad", 4)] {
            assert_eq!(
                lines
                    .iter()
                    .filter(|line| {
                        *line == &format!(
                            "  node \"{name}\" [ambiguous: {count} skeleton nodes share this name]"
                        )
                    })
                    .count(),
                count
            );
            assert_eq!(
                lines
                    .iter()
                    .filter(|line| {
                        line.contains(&format!(
                            "\"{name}\" [ambiguous: {count} materials share this name]"
                        ))
                    })
                    .count(),
                count
            );
        }
    }

    #[test]
    fn inspect_renderer_matches_assembly_node_name_ambiguity() {
        use animsmith_core::model::{MeshInstance, SceneAssets};

        let bone = |name: &str| Bone {
            name: name.into(),
            parent: None,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        };
        let doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: vec![bone("duplicate"), bone("duplicate"), bone("unique")],
            },
            assets: SceneAssets {
                instances: vec![
                    MeshInstance {
                        node: 0,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 1,
                        node: 2,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 2,
                        node: 2,
                        ..MeshInstance::default()
                    },
                ],
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let lines = render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>();
        assert!(lines.contains(
            &"  node \"duplicate\" [ambiguous: 2 skeleton nodes share this name]".to_string()
        ));
        assert_eq!(
            lines
                .iter()
                .filter(|line| *line == "  node \"unique\"")
                .count(),
            2,
            "multiple instances on one unique node remain selectable together"
        );
    }

    #[test]
    fn inspect_renderer_reports_dangling_asset_references_without_panicking() {
        use animsmith_core::model::{MeshAsset, MeshInstance, Primitive, SceneAssets};

        let doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: vec![Bone {
                    name: "valid".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "mesh".into(),
                    source_mesh_index: 4,
                    primitives: vec![Primitive {
                        material: Some(99),
                        ..Primitive::default()
                    }],
                }],
                instances: vec![
                    MeshInstance {
                        source_node_index: 7,
                        node: 99,
                        mesh: 0,
                        ..MeshInstance::default()
                    },
                    MeshInstance {
                        source_node_index: 8,
                        node: 0,
                        mesh: 99,
                        skin_joints: vec![0],
                        ..MeshInstance::default()
                    },
                ],
                ..SceneAssets::default()
            },
            ..Document::default()
        };

        let lines = render_inspect(&doc, &ResolvedRoles::default()).collect::<Vec<_>>();
        for expected in [
            "  node <missing node #99>",
            "    source node: #7",
            "    mesh: #0 \"mesh\" (source mesh #4)",
            "    primitive #0: missing material #99",
            "  node \"valid\"",
            "    source node: #8",
            "    mesh: <missing mesh #99>",
            "    skin: skinned",
            "    primitives: unavailable",
        ] {
            assert!(
                lines.iter().any(|line| line == expected),
                "missing {expected:?}"
            );
        }
    }

    #[test]
    fn fix_renderer_owns_multiline_layout_and_escaping() {
        let mut report = FixReport::default();
        report.skipped.push("bone\ntrack".into());

        assert_eq!(
            render_fix_report(Repair::QuatFlip, &report, None).collect::<Vec<_>>(),
            vec![
                "  skipped[quat-flip]: bone\\ntrack",
                "0 key(s) would be fixed across 0 track(s) -> no output written",
            ]
        );
    }

    #[test]
    fn fix_renderer_preserves_repaired_tracks_and_written_target_summary() {
        let mut quats = animsmith_testkit::quats_from_angles(&[0.0, 0.1, 0.2, 0.3, 0.4]);
        quats[3] = -quats[3];
        let mut doc = animsmith_testkit::two_bone_rotation_doc("clip\nname", quats, false);
        doc.skeleton.bones[1].name = "bone\nname".into();
        let dir = tempfile::tempdir().expect("creates temporary directory");
        let input = dir.path().join("input.glb");
        let safe_output = dir.path().join("fixed.glb");
        animsmith_gltf::write::write(&doc, &input).expect("writes repair fixture");
        let report =
            animsmith_gltf::fix::FixSession::apply_to_path(&input, &safe_output, Repair::QuatFlip)
                .expect("repairs generated fixture");
        let hostile_target = Path::new("fixed\npath\u{1b}[31m.glb");

        assert_eq!(report.total_fixed(), 1, "analytic fixture repair count");
        assert_eq!(
            render_fix_report(Repair::QuatFlip, &report, Some(hostile_target)).collect::<Vec<_>>(),
            vec![
                "  fixed[quat-flip] clip 'clip\\nname' bone 'bone\\nname': 1 key(s) hemisphere-normalized",
                "1 key(s) fixed across 1 track(s) -> fixed\\npath\\u{1b}[31m.glb",
            ]
        );
    }

    #[test]
    fn transform_renderers_own_operation_layout_and_escaping() {
        let clip = Clip {
            name: "walk\nclip".into(),
            duration_s: 1.0,
            tracks: Vec::new(),
        };
        assert_eq!(
            render_transform_slice(&clip, 0.25, 0.75),
            "  sliced 'walk\\nclip' to [0.25:0.75]s (0 keys max)\n"
        );
        assert_eq!(
            render_transform_hold_extend(&clip, 0.5),
            "  hold-extended 'walk\\nclip' by 0.5s\n"
        );
        assert_eq!(
            render_transform_gait_anchor("walk\nclip", 0.25, 0.0, -1, Some(1.234)),
            "  gait-anchored 'walk\\nclip': phase 0.250 -> 0.000 (offset -1, seam 1.23)\n"
        );
        assert_eq!(
            render_transform_gait_anchor_skipped("walk\nclip", "bad\nreason"),
            "  gait-anchor skipped 'walk\\nclip': bad\\nreason\n"
        );

        let clip_with_tracks = Clip {
            name: "multi".into(),
            duration_s: 1.0,
            tracks: vec![
                Track {
                    bone: 0,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 1.0],
                    values: TrackValues::Vec3s(vec![
                        animsmith_core::glam::Vec3::ZERO,
                        animsmith_core::glam::Vec3::ONE,
                    ]),
                },
                Track {
                    bone: 0,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0, 0.25, 0.5, 1.0],
                    values: TrackValues::Quats(vec![animsmith_core::glam::Quat::IDENTITY; 4]),
                },
            ],
        };
        assert_eq!(
            render_transform_slice(&clip_with_tracks, 0.0, 1.0),
            "  sliced 'multi' to [0:1]s (4 keys max)\n"
        );
        assert_eq!(
            render_transform_gait_anchor("walk", 0.25, 0.0, 0, None),
            "  gait-anchored 'walk': phase 0.250 -> 0.000 (offset 0, seam n/a)\n"
        );
    }

    #[cfg(feature = "report")]
    #[test]
    fn report_renderer_owns_write_summary_and_path_escaping() {
        assert_eq!(
            render_report_written(Path::new("report\nname.html"), 2, 3, 1_100_000),
            "wrote report\\nname.html (2 clip(s), 3 finding(s), 1.1 MB)\n"
        );
    }

    #[test]
    fn write_summary_renderer_preserves_optional_dropped_clip_suffix() {
        let dir = tempfile::tempdir().expect("creates temporary directory");
        let empty_path = dir.path().join("empty.glb");
        let empty_summary = animsmith_gltf::write::write(&Document::default(), &empty_path)
            .expect("writes empty document");
        assert_eq!(
            render_write_summary(&empty_path, &empty_summary),
            format!(
                "wrote {} (0 node(s), 0 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))\n",
                empty_path.display()
            )
        );

        let dropped_path = dir.path().join("dropped.glb");
        let dropped_doc = Document {
            skeleton: animsmith_core::Skeleton {
                bones: vec![Bone {
                    name: "root".into(),
                    parent: None,
                    rest: Transform::IDENTITY,
                    inverse_bind: None,
                }],
            },
            clips: vec![Clip {
                name: "empty".into(),
                duration_s: 1.0,
                tracks: Vec::new(),
            }],
            ..Document::default()
        };
        let dropped_summary = animsmith_gltf::write::write(&dropped_doc, &dropped_path)
            .expect("writes document with dropped clip");
        assert_eq!(
            render_write_summary(&dropped_path, &dropped_summary),
            format!(
                "wrote {} (1 node(s), 0 clip(s), 0 mesh(es) / 0 position(s), 0 material(s)); dropped 1 clip(s) with no writable tracks\n",
                dropped_path.display()
            )
        );
    }

    #[test]
    fn operator_error_renderer_owns_prefix_newline_and_escaping() {
        let hostile = "bad\npath\u{1b}[31m\u{2028}left\u{2029}right\u{202e}evil";
        assert_eq!(
            render_operator_error(hostile),
            "animsmith: bad\\npath\\u{1b}[31m\\u{2028}left\\u{2029}right\\u{202e}evil\n"
        );
    }

    #[test]
    fn markdown_clean_file_renders_summary_without_a_table() {
        let md = render_markdown(&[report("clean.glb", vec![])], &[]);
        assert!(md.contains("### `clean.glb`"), "{md}");
        assert!(
            md.contains("✅ Clean — no findings or coverage gaps."),
            "{md}"
        );
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
        let md = render_markdown(&[report("a.glb", vec![f])], &[]);
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
        let md = render_markdown(&[report("a.glb", vec![f])], &[]);
        assert!(md.contains("#### file-level"), "{md}");
        assert!(
            md.contains("| ❌ error | `nan` | — | — | — | `bad` |"),
            "{md}"
        );
    }

    #[test]
    fn markdown_starts_a_fresh_table_per_clip() {
        // Deliberately interleaved: the renderer must sort before grouping,
        // or the repeated walk clip would create a third table.
        let findings = vec![
            Finding::new("c", Severity::Note, "m3").clip("walk"),
            Finding::new("b", Severity::Warning, "m2").clip("run"),
            Finding::new("a", Severity::Error, "m1").clip("walk"),
        ];
        let md = render_markdown(&[report("a.glb", findings)], &[]);
        let run = md.find("#### clip `run`").expect("run heading");
        let walk = md.find("#### clip `walk`").expect("walk heading");
        let walk_error = md.find("| ❌ error | `a`").expect("walk error");
        let walk_note = md.find("| ℹ️ note | `c`").expect("walk note");
        assert!(run < walk, "clips sort ascending:\n{md}");
        assert!(
            walk < walk_error && walk_error < walk_note,
            "severity sort:\n{md}"
        );
        assert_eq!(md.matches("| Severity | Check |").count(), 2, "{md}");

        let text = render_text(
            &[report(
                "a.glb",
                vec![
                    Finding::new("c", Severity::Note, "m3").clip("walk"),
                    Finding::new("b", Severity::Warning, "m2").clip("run"),
                    Finding::new("a", Severity::Error, "m1").clip("walk"),
                ],
            )],
            &[],
        );
        let run = text.find("warning[b] clip 'run'").expect("run finding");
        let walk_error = text.find("error[a] clip 'walk'").expect("walk error");
        let walk_note = text.find("note[c] clip 'walk'").expect("walk note");
        assert!(
            run < walk_error && walk_error < walk_note,
            "text sort:\n{text}"
        );
    }

    #[test]
    fn markdown_collapses_only_long_finding_lists() {
        let make = |n: usize| {
            let findings = (0..n)
                .map(|_| Finding::new("a", Severity::Note, "m").clip("walk"))
                .collect();
            render_markdown(&[report("a.glb", findings)], &[])
        };
        // Assert the boundary documented in docs/cli.md as literals — ten
        // findings stay expanded, eleven collapse — so drifting the
        // internal constant away from the documented "more than ten"
        // fails here rather than silently tracking the constant.
        assert!(make(10).contains("<details open>"));
        let collapsed = make(11);
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
        let md = render_markdown(&[a, b], &[]);
        // Plural "files" and the total summed across both inputs — not the
        // last file's counts alone.
        assert!(
            md.contains("**2 files** — ❌ 1 error(s) · ⚠️ 1 warning(s) · ℹ️ 1 note(s)"),
            "{md}"
        );
    }

    #[test]
    fn markdown_escapes_hostile_text_in_every_asset_derived_cell() {
        // One string exercising delimiter, authored-backslash, code-span,
        // HTML, line-separator, bidi, and terminal-control handling. It is
        // routed through every asset-derived surface, not just the bone.
        let hostile = "x|y\\|z`</details>\nq\u{2028}r\u{2029}t\u{202e}s\u{1b}[31m";
        let esc = md_cell(hostile);
        assert!(
            !esc.chars().any(is_presentation_control) && !esc.contains('`'),
            "{esc}"
        );
        assert!(esc.contains("q r t\\u{202e}s\\u{1b}[31m"), "{esc}");
        // The authored `\|` is pinned: backslash pre-doubled and the pipe
        // escaped, so `y\|z` becomes `y\\\|z` — never a bare delimiter.
        assert!(esc.contains("y\\\\\\|z"), "{esc}");
        let f = Finding::new("x", Severity::Error, hostile)
            .clip(hostile)
            .bone(hostile)
            .measured(hostile);
        let md = render_markdown(&[report(hostile, vec![f])], &[]);
        // The raw hostile string never survives anywhere, and the escaped
        // form appears once per cell it was routed through (path, clip,
        // bone, message, value).
        assert!(!md.contains(hostile), "raw hostile text leaked:\n{md}");
        assert!(
            md.matches(esc.as_str()).count() >= 5,
            "escaped form missing from some cell:\n{md}"
        );
    }

    #[test]
    fn markdown_escapes_hostile_coverage_gap_subjects_and_messages() {
        let hostile = "x|y`</details>\nq";
        let escaped = md_cell(hostile);
        let file = report_with_checks(
            "gap.glb",
            vec![evaluation(
                "foot-slide",
                Vec::new(),
                Vec::new(),
                vec![
                    CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, hostile).scope(
                        EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject(hostile),
                    ),
                ],
            )],
        );

        let md = render_markdown(&[file], &[]);
        assert!(!md.contains(hostile), "raw hostile gap text leaked:\n{md}");
        assert_eq!(
            md.matches(&escaped).count(),
            2,
            "subject and message:\n{md}"
        );
    }

    #[test]
    fn text_escapes_controls_in_findings_and_coverage_gaps() {
        let hostile = "forged\nline\u{1b}[31m";
        let file = report_with_checks(
            hostile,
            vec![evaluation(
                "foot-slide",
                vec![
                    Finding::new("foot-slide", Severity::Warning, hostile)
                        .clip(hostile)
                        .bone(hostile)
                        .measured(hostile),
                ],
                vec![EvaluationScope::new(EvaluationScopeCode::LEFT_FOOT_STANCE)],
                vec![
                    CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, hostile).scope(
                        EvaluationScope::new(EvaluationScopeCode::RIGHT_FOOT_STANCE)
                            .subject(hostile),
                    ),
                ],
            )],
        );

        let text = render_text(&[file], &[]);
        assert!(!text.contains(hostile), "raw control text leaked:\n{text}");
        assert!(text.contains("\\n"), "newline was not escaped:\n{text}");
        assert!(text.contains("\\u{1b}"), "escape was not escaped:\n{text}");
    }

    #[test]
    fn text_atom_escapes_control_and_formatting_characters() {
        let mut raw: String = (0_u8..=31)
            .chain(std::iter::once(127))
            .map(char::from)
            .collect();
        raw.extend([
            '\u{00ad}',
            '\u{034f}',
            '\u{061c}',
            '\u{180e}',
            '\u{200b}',
            '\u{200d}',
            '\u{200e}',
            '\u{200f}',
            '\u{2028}',
            '\u{2029}',
            '\u{202a}',
            '\u{202e}',
            '\u{2060}',
            '\u{2066}',
            '\u{2067}',
            '\u{2068}',
            '\u{2069}',
            '\u{206f}',
            '\u{fe0f}',
            '\u{feff}',
            '\u{fff9}',
            '\u{1bca0}',
            '\u{1d173}',
            '\u{e0001}',
            '\u{e0100}',
        ]);
        let escaped = text_atom(&raw);
        assert!(
            !escaped.chars().any(is_presentation_control),
            "presentation control survived sanitizer: {escaped:?}"
        );
        for visible in [
            "\\u{ad}",
            "\\u{34f}",
            "\\u{61c}",
            "\\u{180e}",
            "\\u{200b}",
            "\\u{200d}",
            "\\u{200e}",
            "\\u{200f}",
            "\\u{2028}",
            "\\u{2029}",
            "\\u{202a}",
            "\\u{202e}",
            "\\u{2060}",
            "\\u{2066}",
            "\\u{2067}",
            "\\u{2068}",
            "\\u{2069}",
            "\\u{206f}",
            "\\u{fe0f}",
            "\\u{feff}",
            "\\u{fff9}",
            "\\u{1bca0}",
            "\\u{1d173}",
            "\\u{e0001}",
            "\\u{e0100}",
        ] {
            assert!(escaped.contains(visible), "missing {visible}: {escaped:?}");
        }
    }

    #[test]
    fn repeated_coverage_gaps_are_grouped_without_losing_the_count() {
        let gaps = vec![
            CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "feet unresolved")
                .scope(EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject("walk_a")),
            CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "toes unresolved").scope(
                EvaluationScope::new(EvaluationScopeCode::LEFT_FOOT_STANCE).subject("walk_b"),
            ),
        ];
        let file = report_with_checks(
            "many.glb",
            vec![evaluation("foot-slide", Vec::new(), Vec::new(), gaps)],
        );

        let text = render_text(std::slice::from_ref(&file), &[]);
        assert_eq!(text.matches("coverage[foot-slide]").count(), 1, "{text}");
        assert!(text.contains("roles_unresolved ×2"), "{text}");
        assert!(text.contains("2 coverage gap(s)"), "{text}");
        for value in [
            "foot_stance",
            "left_foot_stance",
            "walk_a",
            "walk_b",
            "feet unresolved",
            "toes unresolved",
        ] {
            assert!(text.contains(value), "missing {value:?}:\n{text}");
        }

        let markdown = render_markdown(&[file], &[]);
        assert_eq!(
            markdown
                .matches("| `foot-slide` | `roles_unresolved`")
                .count(),
            1
        );
        assert!(
            markdown.contains("| `roles_unresolved` | 2 |"),
            "{markdown}"
        );
        assert!(markdown.contains("2 coverage gap(s)"), "{markdown}");
        for value in [
            "foot_stance",
            "left_foot_stance",
            "walk_a",
            "walk_b",
            "feet unresolved",
            "toes unresolved",
        ] {
            assert!(markdown.contains(value), "missing {value:?}:\n{markdown}");
        }
    }

    #[test]
    fn grouped_value_summary_is_unique_sorted_and_truncated_after_five() {
        assert_eq!(
            summarized_group_values(["b", "a", "b"]),
            "a, b",
            "duplicate values collapse in stable order"
        );
        assert_eq!(
            summarized_group_values(["g", "f", "e", "d", "c", "b", "a"]),
            "a, b, c, d, e, … +2 more"
        );
    }

    #[test]
    fn coverage_gap_groups_keep_check_id_and_code_as_the_full_key() {
        let record = |check_id, gaps| evaluation(check_id, Vec::new(), Vec::new(), gaps);
        let file = report_with_checks(
            "mixed.glb",
            vec![
                record(
                    "check-a",
                    vec![
                        CoverageGap::new(CoverageGapCode::custom("test:roles"), "roles"),
                        CoverageGap::new(
                            CoverageGapCode::custom("test:measurement"),
                            "measurement",
                        ),
                    ],
                ),
                record(
                    "check-b",
                    vec![CoverageGap::new(
                        CoverageGapCode::custom("test:roles"),
                        "roles",
                    )],
                ),
            ],
        );

        let text = render_text(std::slice::from_ref(&file), &[]);
        for row in [
            "coverage[check-a] test:roles ×1",
            "coverage[check-a] test:measurement ×1",
            "coverage[check-b] test:roles ×1",
        ] {
            assert_eq!(text.matches(row).count(), 1, "{text}");
        }

        let markdown = render_markdown(&[file], &[]);
        for row in [
            "| `check-a` | `test:roles` | 1 |",
            "| `check-a` | `test:measurement` | 1 |",
            "| `check-b` | `test:roles` | 1 |",
        ] {
            assert_eq!(markdown.matches(row).count(), 1, "{markdown}");
        }
    }
}
