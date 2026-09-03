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

/// One painted element of a chart: the class the report gives it, the
/// property that carries its colour, the documentation-site token that
/// colour belongs to, and the values a file opened on its own falls back
/// to in each scheme.
pub struct ChartColour {
    /// The element or class the report's own markup uses.
    pub selector: &'static str,
    /// The SVG presentation property this colour paints.
    pub property: &'static str,
    /// The `--as-*` custom property the documentation theme defines.
    pub token: &'static str,
    /// The standalone value in a light context.
    pub light: &'static str,
    /// The standalone value in a dark context.
    pub dark: &'static str,
}

/// Every colour an extracted chart paints.
///
/// The values are the report's own design tokens, and each names the
/// documentation theme's equivalent: the two palettes were chosen to
/// match, so an embedded report and a chart cut out of it read as one
/// picture.
pub const CHART_COLOURS: &[ChartColour] = &[
    ChartColour {
        selector: ".series-left",
        property: "stroke",
        token: "--as-accent",
        light: "#3b67d6",
        dark: "#7aa2f7",
    },
    ChartColour {
        selector: ".series-right",
        property: "stroke",
        token: "--as-warning",
        light: "#946414",
        dark: "#e0af68",
    },
    ChartColour {
        selector: ".series-diff",
        property: "stroke",
        token: "--as-muted",
        light: "#5b6382",
        dark: "#9099b2",
    },
    ChartColour {
        selector: ".root-path",
        property: "stroke",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: ".pathdot",
        property: "fill",
        token: "--as-error",
        light: "#cf3f5b",
        dark: "#f7768e",
    },
    ChartColour {
        selector: ".pathstart",
        property: "stroke",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: ".pathend",
        property: "fill",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: ".pathend",
        property: "stroke",
        token: "--as-ground",
        light: "#f4f5f9",
        dark: "#17171f",
    },
    ChartColour {
        selector: ".pathleader",
        property: "stroke",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: "text",
        property: "fill",
        token: "--as-muted",
        light: "#5b6382",
        dark: "#9099b2",
    },
    // A gait group draws one series per member, each with a stride-anchor
    // mark in the same colour, plus the declared tolerance as a tinted band.
    ChartColour {
        selector: ".series-member-0",
        property: "stroke",
        token: "--as-accent",
        light: "#3b67d6",
        dark: "#7aa2f7",
    },
    ChartColour {
        selector: ".anchor-member-0",
        property: "stroke",
        token: "--as-accent",
        light: "#3b67d6",
        dark: "#7aa2f7",
    },
    ChartColour {
        selector: ".series-member-1",
        property: "stroke",
        token: "--as-warning",
        light: "#946414",
        dark: "#e0af68",
    },
    ChartColour {
        selector: ".anchor-member-1",
        property: "stroke",
        token: "--as-warning",
        light: "#946414",
        dark: "#e0af68",
    },
    ChartColour {
        selector: ".series-member-2",
        property: "stroke",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: ".anchor-member-2",
        property: "stroke",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
    ChartColour {
        selector: ".series-member-3",
        property: "stroke",
        token: "--as-note",
        light: "#6b7390",
        dark: "#bb9af7",
    },
    ChartColour {
        selector: ".anchor-member-3",
        property: "stroke",
        token: "--as-note",
        light: "#6b7390",
        dark: "#bb9af7",
    },
    ChartColour {
        selector: ".series-member-4",
        property: "stroke",
        token: "--as-error",
        light: "#cf3f5b",
        dark: "#f7768e",
    },
    ChartColour {
        selector: ".anchor-member-4",
        property: "stroke",
        token: "--as-error",
        light: "#cf3f5b",
        dark: "#f7768e",
    },
    ChartColour {
        selector: ".series-member-5",
        property: "stroke",
        token: "--as-muted",
        light: "#5b6382",
        dark: "#9099b2",
    },
    ChartColour {
        selector: ".anchor-member-5",
        property: "stroke",
        token: "--as-muted",
        light: "#5b6382",
        dark: "#9099b2",
    },
    ChartColour {
        selector: ".phase-band",
        property: "fill",
        token: "--as-pass",
        light: "#287a3b",
        dark: "#9ece6a",
    },
];

/// The id a standalone chart's root element carries, derived from its
/// file name: `walk.foot-height.svg` becomes `chart-walk-foot-height`.
///
/// Every rule the extractor injects is written under this id, so a page
/// that inlines several charts keeps each one's styling to itself.
pub fn chart_scope(output: &str) -> String {
    format!(
        "chart-{}",
        output.trim_end_matches(".svg").replace('.', "-")
    )
}

/// The styling injected into one extracted chart, scoped to its own id.
///
/// A report styles its charts from `assets/tokens.css`, which a
/// standalone file cannot reach: an `<img>` document gets no parent
/// stylesheet and no script. So every colour resolves through the
/// documentation theme's own token with the standalone value as its
/// fallback. Inlined into a page, the page's theme wins — including an
/// explicit light/dark choice that differs from the operating system,
/// which `prefers-color-scheme` alone would get wrong. Opened on its own,
/// on GitHub or in a browser tab, the fallbacks apply: light first, dark
/// under `prefers-color-scheme`.
pub fn chart_style(scope: &str) -> String {
    let colours = |dark: bool| -> String {
        CHART_COLOURS
            .iter()
            .map(|colour| {
                let value = if dark { colour.dark } else { colour.light };
                format!(
                    "#{scope} {}{{{}:var({},{value})}}",
                    colour.selector, colour.property, colour.token
                )
            })
            .collect()
    };
    // The dash patterns and the band tint are geometry rather than colour:
    // members of a coherent group draw the same curve onto the same
    // coordinates, and without them every member after the first would be
    // hidden under the last one drawn. The patterns reach the curves only,
    // the way the report's own stylesheet applies them.
    let members: String =
        (0..GROUP_MEMBERS)
            .map(|member| {
                format!(
                    "#{scope} .series-member-{member}{{fill:none;stroke-width:1.5}}\
                 #{scope} .anchor-member-{member}{{stroke-width:2.5}}{}",
                    match GROUP_MEMBER_DASHES[member] {
                        "" => String::new(),
                        dashes => format!(
                            "#{scope} path.series-member-{member}{{stroke-dasharray:{dashes}}}"
                        ),
                    }
                )
            })
            .collect();
    format!(
        "#{scope} .series-left,#{scope} .series-right,#{scope} .series-diff,\
         #{scope} .root-path,#{scope} .pathstart{{fill:none;stroke-width:1.5}}\
         #{scope} .pathend{{stroke-width:1}}\
         #{scope} .pathleader{{fill:none;stroke-width:1}}\
         #{scope} .series-diff{{opacity:.6;stroke-dasharray:4 3}}\
         {members}#{scope} .phase-band{{opacity:.16}}\
         #{scope} text{{font:8.5px ui-monospace,monospace}}{}\
         @media (prefers-color-scheme:dark){{{}}}",
        colours(false),
        colours(true),
    )
}

/// Members one gait-group figure can tell apart, which is the number of
/// series colours the report defines.
pub const GROUP_MEMBERS: usize = 6;

/// The dash pattern each member's curve carries.
///
/// The report's own stylesheet is the authority; this is the copy an
/// extracted chart is styled from, and `animsmith-report`'s render tests pin
/// the two together, so a pattern changed in one place fails rather than
/// producing a standalone picture that reads differently from the report it
/// was cut out of. The first member is solid.
pub const GROUP_MEMBER_DASHES: [&str; GROUP_MEMBERS] = ["", "7 4", "2 3", "12 5", "4 2 1 2", "1 4"];

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

/// Which figure of a report a chart is cut from, beyond its kind.
///
/// A report of one clip carries one figure of each kind, and a report with
/// declared gait groups carries one per group as well. Naming the subject is
/// what keeps an ambiguous selector an error rather than a silent first
/// match. A report of several clips carries one figure per clip, which no
/// committed chart is cut from: the group figure is what a multi-clip
/// document has to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartSubject {
    /// The report holds exactly one figure of this kind.
    Only,
    /// The figure of one named gait group (`data-group`).
    Group(&'static str),
}

/// One standalone chart cut out of a committed report.
pub struct DocsChart {
    /// File name written under [`OUTPUT_DIR`].
    pub output: &'static str,
    /// [`DocsReport::output`] the figure is taken from.
    pub report: &'static str,
    /// The figure's `data-kind` attribute.
    pub kind: &'static str,
    /// Which figure of that kind the report is cut at.
    pub subject: ChartSubject,
    /// Element classes the figure must carry. The extractor refuses a
    /// report whose chart markup changed rather than emitting a file
    /// whose injected styling no longer matches it.
    pub classes: &'static [&'static str],
}

/// Classes of the foot-height figure the injected styling colours.
const GAIT_CLASSES: &[&str] = &["series-left", "series-right", "series-diff"];

/// Classes of the root-path figure the injected styling colours.
const ROOT_PATH_CLASSES: &[&str] = &["root-path", "pathdot", "pathstart", "pathend"];

/// Classes of the gait-group figure the injected styling colours: one
/// series and one stride-anchor mark per member of the committed
/// four-member ring, and the declared tolerance band.
const GAIT_GROUP_CLASSES: &[&str] = &[
    "series-member-0",
    "series-member-1",
    "series-member-2",
    "series-member-3",
    "anchor-member-0",
    "anchor-member-1",
    "anchor-member-2",
    "anchor-member-3",
    "phase-band",
];

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
        output: "walk-short-channel.report.html",
        arguments: &["report", "walk-short-channel.glb"],
    },
    DocsReport {
        output: "walk-travel.report.html",
        arguments: &[
            "--config",
            "../walk-travel-in-place.animsmith.toml",
            "report",
            "walk-travel.glb",
        ],
    },
    DocsReport {
        output: "run-ring.report.html",
        arguments: &[
            "--config",
            "../run-ring.animsmith.toml",
            "report",
            "run-ring.glb",
        ],
    },
    DocsReport {
        output: "walk-frozen-arm.report.html",
        arguments: &[
            "--config",
            "../walk-frozen-arm.animsmith.toml",
            "report",
            "walk-frozen-arm.glb",
        ],
    },
    DocsReport {
        output: "walk-scaled.report.html",
        arguments: &["report", "walk-scaled.glb"],
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
/// A chart earns a file when a still picture carries the symptom on its
/// own; the other reports are embedded whole, because what they show is
/// the findings list or the pose grid rather than one curve.
pub const CHARTS: &[DocsChart] = &[
    DocsChart {
        output: "walk-dirty.foot-height.svg",
        report: "walk-dirty.report.html",
        kind: "gait",
        subject: ChartSubject::Only,
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "walk.foot-height.svg",
        report: "walk.report.html",
        kind: "gait",
        subject: ChartSubject::Only,
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "walk.root-path.svg",
        report: "walk.report.html",
        kind: "rootpath",
        subject: ChartSubject::Only,
        classes: ROOT_PATH_CLASSES,
    },
    DocsChart {
        output: "walk-travel.root-path.svg",
        report: "walk-travel.report.html",
        kind: "rootpath",
        subject: ChartSubject::Only,
        classes: ROOT_PATH_CLASSES,
    },
    DocsChart {
        output: "run-ring.gait-group.svg",
        report: "run-ring.report.html",
        kind: "gait-group",
        subject: ChartSubject::Group("run-ring"),
        classes: GAIT_GROUP_CLASSES,
    },
    DocsChart {
        output: "foot-slide-before.foot-height.svg",
        report: "foot-slide-before.report.html",
        kind: "gait",
        subject: ChartSubject::Only,
        classes: GAIT_CLASSES,
    },
    DocsChart {
        output: "foot-slide-after.foot-height.svg",
        report: "foot-slide-after.report.html",
        kind: "gait",
        subject: ChartSubject::Only,
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
/// included — with four changes: the SVG namespace a standalone document
/// needs, the [`chart_scope`] id every injected rule is written under,
/// [`chart_style`] as the first child, and the playhead removed, because
/// a still picture has no frame selection. An absent figure or an absent
/// expected class is an error rather than a silently unstyled file.
pub fn standalone_chart(report_html: &str, chart: &DocsChart) -> Result<String, String> {
    let figure = figure_body(report_html, chart.kind, chart.subject)?;
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

    let scope = chart_scope(chart.output);
    let style = chart_style(&scope);
    let mut standalone = String::with_capacity(svg.len() + style.len() + 64);
    standalone.push_str("<svg");
    standalone.push_str(SVG_NAMESPACE);
    standalone.push_str(&format!(" id=\"{scope}\""));
    standalone.push_str(&svg["<svg".len()..=open_end]);
    standalone.push_str("<style>");
    standalone.push_str(&style);
    standalone.push_str("</style>");
    standalone.push_str(&remove_playheads(&svg[open_end + 1..]));
    standalone.push('\n');
    Ok(standalone)
}

/// The inner markup of the one `<figure class="chart">` selected by this
/// `data-kind` and, where the report carries several figures of that kind,
/// its subject attribute.
///
/// Attributes are matched independently, so the selector does not depend
/// on the order the renderer writes them in.
fn figure_body<'a>(
    report_html: &'a str,
    kind: &str,
    subject: ChartSubject,
) -> Result<&'a str, String> {
    let selector = figure_selector(kind, subject);
    let mut found: Option<&str> = None;
    let mut rest = report_html;
    while let Some(span) = element(rest, "<figure class=\"chart\"", "</figure>") {
        let figure = &rest[span.clone()];
        let start_tag_end = figure
            .find('>')
            .ok_or_else(|| "unterminated <figure> start tag".to_owned())?;
        let start_tag = &figure[..start_tag_end];
        if selector
            .iter()
            .all(|attribute| start_tag.contains(attribute.as_str()))
        {
            if found.is_some() {
                return Err(format!(
                    "report has more than one {} figure",
                    selector.join(" ")
                ));
            }
            found = Some(&figure[start_tag_end + 1..]);
        }
        rest = &rest[span.end..];
    }
    found.ok_or_else(|| {
        format!(
            "report has no <figure class=\"chart\" {}>",
            selector.join(" ")
        )
    })
}

/// The attributes a figure must carry to be this chart's source.
fn figure_selector(kind: &str, subject: ChartSubject) -> Vec<String> {
    let mut selector = Vec::with_capacity(2);
    match subject {
        ChartSubject::Only => {}
        ChartSubject::Group(group) => selector.push(format!("data-group=\"{group}\"")),
    }
    selector.push(format!("data-kind=\"{kind}\""));
    selector
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

    /// A second document whose two clips each carry a gait figure, so a
    /// kind-only selector is ambiguous and a clip selector is not. Its two
    /// declared gait groups make the same point for a group selector.
    const TWO_CLIP_FIXTURE: &str = concat!(
        "<figure class=\"chart\" data-clip=\"run_forward\" data-kind=\"gait\">",
        "<svg viewBox=\"0 0 2 2\"><path class=\"series-left\" d=\"M0,0\"/></svg></figure>",
        "<figure class=\"chart\" data-clip=\"run_left\" data-kind=\"gait\">",
        "<svg viewBox=\"0 0 3 3\"><path class=\"series-left\" d=\"M1,1\"/></svg></figure>",
        "<figure class=\"chart\" data-group=\"run-ring\" data-kind=\"gait-group\">",
        "<svg viewBox=\"0 0 4 4\"><path class=\"series-member-0\" d=\"M2,2\"/></svg></figure>",
        "<figure class=\"chart\" data-group=\"walk-ring\" data-kind=\"gait-group\">",
        "<svg viewBox=\"0 0 5 5\"><path class=\"series-member-0\" d=\"M3,3\"/></svg></figure>",
    );

    fn gait(classes: &'static [&'static str]) -> DocsChart {
        DocsChart {
            output: "fixture.svg",
            report: "fixture.html",
            kind: "gait",
            subject: ChartSubject::Only,
            classes,
        }
    }

    #[test]
    fn extraction_keeps_the_element_verbatim_minus_the_playhead() {
        let svg = standalone_chart(FIXTURE, &gait(&["series-left", "series-right"]))
            .expect("extracts the gait figure");
        let style = chart_style("chart-fixture");
        assert_eq!(
            svg,
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" id=\"chart-fixture\" \
                 viewBox=\"0 0 360 150\" width=\"100%\" role=\"img\" aria-label=\"walk\">\
                 <style>{style}</style>\
                 <title>walk</title><path class=\"series-left\" d=\"M0,0\"/>\
                 <path class=\"series-right\" d=\"M0,0\"/></svg>\n"
            )
        );
    }

    /// Every colour a chart paints resolves through the documentation
    /// theme's token, so an inlined chart follows the page's own light or
    /// dark choice; the standalone fallbacks stay for a file opened on its
    /// own, and every rule is scoped to that chart alone.
    #[test]
    fn chart_styles_prefer_the_page_token_and_stay_scoped_to_one_chart() {
        assert_eq!(
            chart_scope("walk-dirty.foot-height.svg"),
            "chart-walk-dirty-foot-height"
        );
        let style = chart_style("chart-one");
        for colour in CHART_COLOURS {
            assert!(
                style.contains(&format!(
                    "#chart-one {}{{{}:var({},{})}}",
                    colour.selector, colour.property, colour.token, colour.light
                )),
                "the light fallback is the standalone value: {style}"
            );
            assert!(
                style.contains(&format!(
                    "#chart-one {}{{{}:var({},{})}}",
                    colour.selector, colour.property, colour.token, colour.dark
                )),
                "the dark fallback stays under the media query: {style}"
            );
        }
        assert!(
            style.contains("@media (prefers-color-scheme:dark){#chart-one "),
            "the dark fallbacks are the media query's whole content: {style}"
        );
        for rule in style.split('}').filter(|rule| rule.contains('{')) {
            let selectors = rule.rsplit('{').next_back().expect("a rule has a selector");
            for selector in selectors.split(',') {
                let selector = selector.trim();
                assert!(
                    selector.starts_with("#chart-one ") || selector.starts_with("@media"),
                    "every rule is scoped to the chart: {selector:?} in {style}"
                );
            }
        }
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

    /// A gait-group figure is selected by the group it draws, and the two
    /// subjects never cross: a clip name does not select a group figure and
    /// a group name does not select a clip figure.
    #[test]
    fn a_group_selector_picks_one_figure_out_of_a_multi_group_report() {
        let group = |name| DocsChart {
            kind: "gait-group",
            subject: ChartSubject::Group(name),
            classes: &["series-member-0"],
            ..gait(&[])
        };
        for (name, view_box) in [("run-ring", "0 0 4 4"), ("walk-ring", "0 0 5 5")] {
            let svg = standalone_chart(TWO_CLIP_FIXTURE, &group(name))
                .unwrap_or_else(|error| panic!("extracts {name}: {error}"));
            assert!(svg.contains(&format!("viewBox=\"{view_box}\"")), "{svg}");
        }

        let ambiguous = DocsChart {
            subject: ChartSubject::Only,
            ..group("run-ring")
        };
        assert_eq!(
            standalone_chart(TWO_CLIP_FIXTURE, &ambiguous).unwrap_err(),
            "report has more than one data-kind=\"gait-group\" figure",
            "a report of two groups is not silently cut at its first figure"
        );
        assert_eq!(
            standalone_chart(TWO_CLIP_FIXTURE, &group("run_left")).unwrap_err(),
            "report has no <figure class=\"chart\" data-group=\"run_left\" \
             data-kind=\"gait-group\">",
            "a clip name does not reach a group figure"
        );
        let crossed = DocsChart {
            kind: "gait",
            ..group("run-ring")
        };
        assert_eq!(
            standalone_chart(TWO_CLIP_FIXTURE, &crossed).unwrap_err(),
            "report has no <figure class=\"chart\" data-group=\"run-ring\" \
             data-kind=\"gait\">",
            "a group name does not reach a clip figure"
        );
    }

    /// The selector matches attributes independently for a group as much as
    /// for a clip.
    #[test]
    fn a_group_selector_does_not_depend_on_the_attribute_order() {
        let swapped = TWO_CLIP_FIXTURE.replace(
            "data-group=\"walk-ring\" data-kind=\"gait-group\"",
            "data-kind=\"gait-group\" data-group=\"walk-ring\"",
        );
        assert!(
            swapped != TWO_CLIP_FIXTURE,
            "the fixture must really be rewritten"
        );
        let chart = DocsChart {
            kind: "gait-group",
            subject: ChartSubject::Group("walk-ring"),
            classes: &["series-member-0"],
            ..gait(&[])
        };
        let svg = standalone_chart(&swapped, &chart).expect("extracts walk-ring either way");
        assert!(svg.contains("viewBox=\"0 0 5 5\""), "{svg}");
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
