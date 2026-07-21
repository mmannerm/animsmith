//! CLI output serializers.
//!
//! `docs/output.md` frames the JSON envelope as the machine-readable
//! source of truth, with text and Markdown as presentation-only views
//! over the same [`LintFileReport`] model. This module houses the shared JSON
//! serializer and `lint`'s text and Markdown renderers, so they don't
//! accrete as free functions in `main`. `measure` and `diff` still format
//! their text inline at their call sites; future serializers (SARIF,
//! GitLab Code Quality, JUnit, CSV) belong here alongside the JSON one.

use animsmith_core::{CoverageGap, CoverageGapCode, Finding, LintFileReport, Severity};
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

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

/// Human-readable one-line-per-finding text output for `lint`.
pub(crate) fn print_text(reports: &[LintFileReport], suppressed: &[String]) {
    print!("{}", render_text(reports, suppressed));
}

fn render_text(reports: &[LintFileReport], suppressed: &[String]) -> String {
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

/// Escape terminal control characters while leaving ordinary Unicode text
/// readable. Each untrusted value therefore remains on one physical line and
/// cannot inject ANSI terminal commands.
pub(crate) fn text_atom(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_control() {
            escaped.extend(ch.escape_default());
        } else {
            escaped.push(ch);
        }
    }
    Cow::Owned(escaped)
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
/// machine-readable contract, and this layout carries no stability
/// guarantees. Mirrors the text output's information — severity, check
/// id, location, measured/expected values, per-clip grouping — as tables
/// inside per-file collapsible sections.
pub(crate) fn print_markdown(reports: &[LintFileReport], suppressed: &[String]) {
    print!("{}", render_markdown(reports, suppressed));
}

/// Pure Markdown renderer behind [`print_markdown`], returning the whole
/// document as a string. Keeping it side-effect free lets the per-clip
/// grouping, cell escaping, collapse threshold, and summary tallies be
/// unit-tested directly without spawning the CLI.
///
/// Findings are expected grouped by clip — the `lint` command sorts them
/// by clip before calling — and a new table is started each time the clip
/// changes; an unsorted slice would emit repeated per-clip headers.
fn render_markdown(reports: &[LintFileReport], suppressed: &[String]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        CheckEvaluation, CheckOutput, CoverageGap, CoverageGapCode, Document, EvaluationScope,
        EvaluationScopeCode, Finding, LintFileReport, MeasurementContract, ResolvedRoles, RigInfo,
    };
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
            RigInfo::from_resolved(&doc, &ResolvedRoles::default()),
            checks,
            MeasurementContract::new(BTreeMap::new(), Vec::new()),
        )
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
        // One string exercising all four `md_cell` transforms at once: a
        // bare delimiter, an authored `\|` that must not collapse back
        // into a delimiter, a code-span closer, an HTML tag, and a
        // newline. Routed through every asset-derived surface — path
        // heading, clip heading, bone location, message, and a textual
        // value — not just the bone.
        let hostile = "x|y\\|z`</details>\nq";
        let esc = md_cell(hostile);
        // The escaped form carries no live hazard: newline flattened and
        // the only code-span closer neutralized.
        assert!(!esc.contains('\n') && !esc.contains('`'), "{esc}");
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
    fn text_atom_escapes_every_ascii_control_character() {
        let raw: String = (0_u8..=31)
            .chain(std::iter::once(127))
            .map(char::from)
            .collect();
        let escaped = text_atom(&raw);
        assert!(
            !escaped.chars().any(char::is_control),
            "control survived sanitizer: {escaped:?}"
        );
    }

    #[test]
    fn repeated_coverage_gaps_are_grouped_without_losing_the_count() {
        let gaps = ["walk_a", "walk_b"]
            .into_iter()
            .map(|clip| {
                CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "feet unresolved")
                    .scope(EvaluationScope::new(EvaluationScopeCode::FOOT_STANCE).subject(clip))
            })
            .collect();
        let file = report_with_checks(
            "many.glb",
            vec![evaluation("foot-slide", Vec::new(), Vec::new(), gaps)],
        );

        let text = render_text(std::slice::from_ref(&file), &[]);
        assert_eq!(text.matches("coverage[foot-slide]").count(), 1, "{text}");
        assert!(text.contains("roles_unresolved ×2"), "{text}");
        assert!(text.contains("2 coverage gap(s)"), "{text}");

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
                        CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "roles"),
                        CoverageGap::new(CoverageGapCode::MEASUREMENT_UNAVAILABLE, "measurement"),
                    ],
                ),
                record(
                    "check-b",
                    vec![CoverageGap::new(CoverageGapCode::ROLES_UNRESOLVED, "roles")],
                ),
            ],
        );

        let text = render_text(std::slice::from_ref(&file), &[]);
        for row in [
            "coverage[check-a] roles_unresolved ×1",
            "coverage[check-a] measurement_unavailable ×1",
            "coverage[check-b] roles_unresolved ×1",
        ] {
            assert_eq!(text.matches(row).count(), 1, "{text}");
        }

        let markdown = render_markdown(&[file], &[]);
        for row in [
            "| `check-a` | `roles_unresolved` | 1 |",
            "| `check-a` | `measurement_unavailable` | 1 |",
            "| `check-b` | `roles_unresolved` | 1 |",
        ] {
            assert_eq!(markdown.matches(row).count(), 1, "{markdown}");
        }
    }
}
