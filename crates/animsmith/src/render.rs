//! CLI output serializers over the report model.
//!
//! `docs/output.md` frames the JSON envelope as the machine-readable
//! source of truth, with text and Markdown as presentation-only views and
//! future serializers (SARIF, GitLab Code Quality, JUnit, CSV) layered
//! over the same [`FileReport`] model. This module keeps those renderers
//! in one place rather than letting them accrete as free functions in
//! `main`.

use crate::{FileReport, FindingSummary};
use animsmith_core::Severity;
use serde::Serialize;

/// Serialize any envelope as pretty JSON — the machine-readable contract
/// shared by `measure`, `lint`, and `diff`.
pub(crate) fn print_json<T: Serialize>(value: &T) {
    let out = serde_json::to_string_pretty(value);
    println!("{}", out.expect("report serializes"));
}

/// Human-readable one-line-per-finding text output for `lint`.
pub(crate) fn print_text(reports: &[FileReport]) {
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
pub(crate) fn print_markdown(reports: &[FileReport]) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RigInfo;
    use animsmith_core::Finding;
    use std::collections::BTreeMap;

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
