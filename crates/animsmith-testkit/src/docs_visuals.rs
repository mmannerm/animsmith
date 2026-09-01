//! The committed documentation visuals under `docs/visuals/`: which
//! report each one comes from, and how a standalone chart is cut out of
//! a rendered report.
//!
//! Every picture in the customer documentation is produced by the CLI
//! itself from a committed synthetic fixture. This module owns the
//! wiring — the fixture/argument manifest and the SVG extractor — so the
//! generator (`crates/animsmith/examples/gen_docs_visuals.rs`) and its
//! drift guard (`crates/animsmith/tests/docs_visuals.rs`) cannot
//! disagree about what the committed bytes are. Neither side renders a
//! report itself: both hand [`write_docs_visuals`] a closure that runs
//! the `report` command, so the committed documents come from the same
//! code path a reader's own `animsmith report` invocation takes.

use std::path::Path;

/// Directory every [`DocsReport::arguments`] list is relative to.
///
/// The `report` command records the input path it was given, so the
/// commands run from the fixture directory: the committed documents then
/// name `walk-dirty.glb` rather than one machine's absolute path.
pub const WORKING_DIR: &str = "examples/assets";

/// Directory the committed visuals live in, relative to the repository root.
pub const OUTPUT_DIR: &str = "docs/visuals";

/// Styling injected into every extracted chart.
///
/// A report styles its charts from `assets/tokens.css`, which a
/// standalone file loaded through `<img>` cannot reach: an `<img>`
/// document gets no parent stylesheet and no script. These are the same
/// token values (light first, dark under `prefers-color-scheme`), inlined
/// so one file reads correctly in both site themes and on GitHub.
pub const CHART_STYLE: &str = "\
.series-left,.series-right,.series-diff,.root-path{fill:none;stroke-width:1.5}\
.series-left{stroke:#3b67d6}.series-right{stroke:#946414}\
.series-diff{stroke:#5b6382;opacity:.6}.root-path{stroke:#287a3b}\
.pathdot{fill:#cf3f5b}text{fill:#5b6382;font:8.5px ui-monospace,monospace}\
@media (prefers-color-scheme:dark){\
.series-left{stroke:#7aa2f7}.series-right{stroke:#e0af68}\
.series-diff{stroke:#9099b2}.root-path{stroke:#9ece6a}\
.pathdot{fill:#f7768e}text{fill:#9099b2}}";

/// The SVG namespace a standalone chart needs and an embedded one does not.
const SVG_NAMESPACE: &str = " xmlns=\"http://www.w3.org/2000/svg\"";

/// One committed report document.
pub struct DocsReport {
    /// File name written under [`OUTPUT_DIR`].
    pub output: &'static str,
    /// `animsmith` arguments, resolved from [`WORKING_DIR`], without the
    /// `-o` pair: [`write_docs_visuals`] appends the output path.
    pub arguments: &'static [&'static str],
}

/// One standalone chart cut out of a committed report.
pub struct DocsChart {
    /// File name written under [`OUTPUT_DIR`].
    pub output: &'static str,
    /// [`DocsReport::output`] the figure is taken from.
    pub report: &'static str,
    /// The figure's `data-kind` attribute.
    pub kind: &'static str,
    /// Element classes the figure must carry. The extractor refuses a
    /// report whose chart markup changed rather than emitting a file
    /// whose injected styling no longer matches it.
    pub classes: &'static [&'static str],
}

/// Classes of the foot-height figure the injected styling colours.
const GAIT_CLASSES: &[&str] = &["series-left", "series-right", "series-diff"];

/// Every committed report, in write order.
///
/// `clip-dirty.glb` carries no rig roles, so its report has no charts;
/// it is committed whole for the mechanical-defect symptom page.
pub const REPORTS: &[DocsReport] = &[
    DocsReport {
        output: "walk-dirty.report.html",
        arguments: &[
            "--config",
            "../walk.animsmith.toml",
            "report",
            "walk-dirty.glb",
        ],
    },
    DocsReport {
        output: "walk.report.html",
        arguments: &["--config", "../walk.animsmith.toml", "report", "walk.glb"],
    },
    DocsReport {
        output: "clip-dirty.report.html",
        arguments: &["report", "clip-dirty.glb"],
    },
    DocsReport {
        output: "foot-slide-before.report.html",
        arguments: &[
            "--config",
            "../report-comparison.animsmith.toml",
            "report",
            "report-comparison-before.glb",
        ],
    },
    DocsReport {
        output: "foot-slide-after.report.html",
        arguments: &[
            "--config",
            "../report-comparison.animsmith.toml",
            "report",
            "report-comparison-after.glb",
        ],
    },
    DocsReport {
        output: "foot-slide.comparison.html",
        arguments: &[
            "--config",
            "../report-comparison.animsmith.toml",
            "report",
            "report-comparison-before.glb",
            "--compare-after",
            "report-comparison-after.glb",
            "--before-clip",
            "acceptance-matrix",
            "--after-clip",
            "acceptance-matrix",
        ],
    },
];

/// Every committed standalone chart, in write order.
///
/// Only the foot-height figure is committed. Both walk fixtures declare
/// `movement_owner_xz = "gameplay"`, so their root-path figures are the
/// same stationary dot; a root-path chart is worth committing once a
/// fixture whose root actually travels needs one.
pub const CHARTS: &[DocsChart] = &[
    DocsChart {
        output: "walk-dirty.foot-height.svg",
        report: "walk-dirty.report.html",
        kind: "gait",
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "walk.foot-height.svg",
        report: "walk.report.html",
        kind: "gait",
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "foot-slide-before.foot-height.svg",
        report: "foot-slide-before.report.html",
        kind: "gait",
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "foot-slide-after.foot-height.svg",
        report: "foot-slide-after.report.html",
        kind: "gait",
        classes: GAIT_CLASSES,
    },
];

/// Write every committed visual into `out_dir`.
///
/// `render` runs one `report` invocation with the given arguments; the
/// caller decides which `animsmith` binary that is and runs it in
/// [`WORKING_DIR`]. Charts are then cut out of the documents this call
/// just wrote, so the two halves cannot describe different runs.
pub fn write_docs_visuals(
    out_dir: &Path,
    mut render: impl FnMut(&[&str]) -> Result<(), String>,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir)
        .map_err(|error| format!("creates {}: {error}", out_dir.display()))?;

    for report in REPORTS {
        let output = out_dir.join(report.output);
        let output = output
            .to_str()
            .ok_or_else(|| format!("report output path is not UTF-8: {}", output.display()))?
            .to_owned();
        let mut arguments = report.arguments.to_vec();
        arguments.extend(["-o", output.as_str()]);
        render(&arguments)?;
    }

    for chart in CHARTS {
        let report = out_dir.join(chart.report);
        let html = std::fs::read_to_string(&report)
            .map_err(|error| format!("reads {}: {error}", report.display()))?;
        let svg =
            standalone_chart(&html, chart).map_err(|error| format!("{}: {error}", chart.output))?;
        let output = out_dir.join(chart.output);
        std::fs::write(&output, svg)
            .map_err(|error| format!("writes {}: {error}", output.display()))?;
    }
    Ok(())
}

/// Cut one figure's `<svg>` out of a rendered report as a standalone file.
///
/// The element is kept verbatim — `viewBox`, `role` and `aria-label`
/// included — with three changes: the SVG namespace a standalone
/// document needs, [`CHART_STYLE`] as the first child, and the playhead
/// removed, because a still picture has no frame selection. An absent
/// figure or an absent expected class is an error rather than a silently
/// unstyled file.
pub fn standalone_chart(report_html: &str, chart: &DocsChart) -> Result<String, String> {
    let figure = figure_body(report_html, chart.kind)?;
    let svg = element(figure, "<svg", "</svg>")
        .map(|span| &figure[span])
        .ok_or_else(|| format!("figure data-kind=\"{}\" has no <svg> element", chart.kind))?;
    for class in chart.classes {
        if !svg.contains(&format!("class=\"{class}\"")) {
            return Err(format!(
                "figure data-kind=\"{}\" no longer carries class {class:?}",
                chart.kind
            ));
        }
    }

    let open_end = svg
        .find('>')
        .ok_or_else(|| "unterminated <svg> start tag".to_owned())?;
    if svg[..open_end].contains("xmlns") {
        return Err("report <svg> already declares a namespace".to_owned());
    }
    if svg.contains("xlink:") {
        return Err("report <svg> uses xlink, which this extractor does not declare".to_owned());
    }

    let mut standalone = String::with_capacity(svg.len() + CHART_STYLE.len() + 64);
    standalone.push_str("<svg");
    standalone.push_str(SVG_NAMESPACE);
    standalone.push_str(&svg["<svg".len()..=open_end]);
    standalone.push_str("<style>");
    standalone.push_str(CHART_STYLE);
    standalone.push_str("</style>");
    standalone.push_str(&remove_playheads(&svg[open_end + 1..]));
    standalone.push('\n');
    Ok(standalone)
}

/// The inner markup of the one `<figure class="chart">` with this `data-kind`.
fn figure_body<'a>(report_html: &'a str, kind: &str) -> Result<&'a str, String> {
    let attribute = format!("data-kind=\"{kind}\"");
    let mut found: Option<&str> = None;
    let mut rest = report_html;
    while let Some(span) = element(rest, "<figure class=\"chart\"", "</figure>") {
        let figure = &rest[span.clone()];
        let start_tag_end = figure
            .find('>')
            .ok_or_else(|| "unterminated <figure> start tag".to_owned())?;
        if figure[..start_tag_end].contains(&attribute) {
            if found.is_some() {
                return Err(format!("report has more than one {attribute} figure"));
            }
            found = Some(&figure[start_tag_end + 1..]);
        }
        rest = &rest[span.end..];
    }
    found.ok_or_else(|| format!("report has no <figure class=\"chart\" {attribute}>"))
}

/// The byte range of the first `open`…`close` element of `html`, start
/// and end tags included. The report's chart markup never nests an
/// element inside another of the same kind, so the first closing tag
/// after the opening one is this element's.
fn element(html: &str, open: &str, close: &str) -> Option<std::ops::Range<usize>> {
    let start = html.find(open)?;
    let end = html[start..].find(close)? + start + close.len();
    Some(start..end)
}

/// Drop every self-closing `<line class="playhead" …/>`.
fn remove_playheads(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(start) = rest.find("<line class=\"playhead\"") {
        let Some(end) = rest[start..].find('>') else {
            break;
        };
        out.push_str(&rest[..start]);
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = concat!(
        "<figure class=\"chart\" data-clip=\"walk\" data-kind=\"rootpath\">",
        "<figcaption>other</figcaption><svg viewBox=\"0 0 1 1\">",
        "<path class=\"root-path\" d=\"M0,0\"/><circle class=\"pathdot\" r=\"3\"/>",
        "</svg></figure>",
        "<figure class=\"chart\" data-clip=\"walk\" data-kind=\"gait\" data-pad=\"34\">",
        "<figcaption>caption</figcaption>",
        "<svg viewBox=\"0 0 360 150\" width=\"100%\" role=\"img\" aria-label=\"walk\">",
        "<title>walk</title><path class=\"series-left\" d=\"M0,0\"/>",
        "<line class=\"playhead\" x1=\"34\" x2=\"34\" y1=\"18\" y2=\"134.0\"/>",
        "<path class=\"series-right\" d=\"M0,0\"/></svg></figure>",
    );

    fn gait(classes: &'static [&'static str]) -> DocsChart {
        DocsChart {
            output: "fixture.svg",
            report: "fixture.html",
            kind: "gait",
            classes,
        }
    }

    #[test]
    fn extraction_keeps_the_element_verbatim_minus_the_playhead() {
        let svg = standalone_chart(FIXTURE, &gait(&["series-left", "series-right"]))
            .expect("extracts the gait figure");
        assert_eq!(
            svg,
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 360 150\" \
                 width=\"100%\" role=\"img\" aria-label=\"walk\"><style>{CHART_STYLE}</style>\
                 <title>walk</title><path class=\"series-left\" d=\"M0,0\"/>\
                 <path class=\"series-right\" d=\"M0,0\"/></svg>\n"
            )
        );
    }

    #[test]
    fn each_figure_is_selected_by_its_own_kind() {
        let chart = DocsChart {
            kind: "rootpath",
            classes: &["root-path", "pathdot"],
            ..gait(&[])
        };
        let svg = standalone_chart(FIXTURE, &chart).expect("extracts the root-path figure");
        assert!(svg.contains("viewBox=\"0 0 1 1\""), "{svg}");
        assert!(svg.contains("class=\"root-path\""), "{svg}");
        assert!(!svg.contains("class=\"series-left\""), "{svg}");
    }

    #[test]
    fn a_missing_figure_or_class_is_an_error_rather_than_an_unstyled_file() {
        assert_eq!(
            standalone_chart(FIXTURE, &gait(&["series-left", "series-gone"])).unwrap_err(),
            "figure data-kind=\"gait\" no longer carries class \"series-gone\""
        );
        let missing = DocsChart {
            kind: "absent",
            ..gait(&[])
        };
        assert_eq!(
            standalone_chart(FIXTURE, &missing).unwrap_err(),
            "report has no <figure class=\"chart\" data-kind=\"absent\">"
        );
    }

    #[test]
    fn every_committed_chart_names_a_committed_report() {
        for chart in CHARTS {
            assert!(
                REPORTS.iter().any(|report| report.output == chart.report),
                "{} is cut from an unwritten report {}",
                chart.output,
                chart.report
            );
        }
    }
}
