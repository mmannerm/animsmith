//! Drift guard for the committed documentation visuals under
//! `docs/visuals/`.
//!
//! The generator (`crates/animsmith/examples/gen_docs_visuals.rs`) and
//! this test drive the same `animsmith-testkit` wiring, so a renamed
//! output, a dropped chart, a changed report argument, or a change in
//! the report renderer itself fails here rather than when a human next
//! remembers to rerun the generator. This test runs the built CLI
//! directly; the generator reaches the same binary through Cargo.
//!
//! `docs/visuals/README.md` and `docs/visuals/icons/` are hand-authored
//! and deliberately outside the generated set.

#![cfg(feature = "report")]

use animsmith_core::sha256_hex;
use animsmith_testkit::docs_visuals;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run one `animsmith` invocation from the fixture directory, the way
/// both the generator and a reader's own shell would.
fn run(arguments: &[String]) -> Result<(), String> {
    let working_dir = repo_root().join(docs_visuals::WORKING_DIR);
    let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
        .args(arguments)
        .current_dir(&working_dir)
        .output()
        .map_err(|error| format!("runs animsmith {}: {error}", arguments.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "animsmith {} exited with {}: {}",
            arguments.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// A throwaway directory for the intermediates the manifest writes, so
/// nothing a generator run produces lands in this checkout.
fn scratch_dir() -> tempfile::TempDir {
    scratch_dir_named(docs_visuals::SCRATCH_PREFIX)
}

fn scratch_dir_named(prefix: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .expect("creates scratch dir")
}

/// Regenerate every visual into `out_dir` with the built CLI.
fn regenerate(out_dir: &Path, scratch: &Path) {
    docs_visuals::write_docs_visuals(out_dir, scratch, run)
        .expect("regenerates the documentation visuals");
}

/// Every committed file name, in manifest order. A [`SCRATCH`] name is an
/// intermediate a later invocation reads, not a visual anyone commits.
///
/// [`SCRATCH`]: docs_visuals::SCRATCH
fn generated_names() -> Vec<&'static str> {
    docs_visuals::COMMANDS
        .iter()
        .map(|command| command.output)
        .filter(|output| !output.starts_with(docs_visuals::SCRATCH))
        .chain(docs_visuals::CHARTS.iter().map(|chart| chart.output))
        .collect()
}

/// The `.glb` arguments of one invocation: the after side a comparison
/// names with `--compare-after`, and everything else it reads.
fn inputs(arguments: &[&'static str]) -> (Vec<&'static str>, Option<&'static str>) {
    let after = arguments
        .windows(2)
        .find(|pair| pair[0] == "--compare-after")
        .map(|pair| pair[1]);
    let before = arguments
        .iter()
        .copied()
        .filter(|argument| argument.ends_with(".glb") && Some(*argument) != after)
        .collect();
    (before, after)
}

/// The bytes one manifest input names, read from the fixture directory or
/// from the scratch directory an earlier invocation wrote it into.
fn input_digest(input: &str, scratch: &Path) -> String {
    let source =
        docs_visuals::output_path(input, &repo_root().join(docs_visuals::WORKING_DIR), scratch);
    let bytes = std::fs::read(&source)
        .unwrap_or_else(|error| panic!("reads the {input} input at {}: {error}", source.display()));
    sha256_hex(&bytes)
}

/// The `application/json` payload a rendered document carries under `id`.
fn embedded_json(html: &str, id: &str, name: &str) -> serde_json::Value {
    let marker = format!("<script type=\"application/json\" id=\"{id}\">");
    let start = html
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} carries its {id} payload"))
        + marker.len();
    let end = html[start..]
        .find("</script>")
        .unwrap_or_else(|| panic!("{name} closes its {id} payload"))
        + start;
    serde_json::from_str(&html[start..end])
        .unwrap_or_else(|error| panic!("{name} {id} payload is JSON: {error}"))
}

#[test]
fn committed_visuals_match_the_generator_output() {
    let temporary = tempfile::Builder::new()
        .prefix("animsmith-docs-visuals-")
        .tempdir()
        .expect("creates temp dir");
    let scratch = scratch_dir();
    regenerate(temporary.path(), scratch.path());

    let committed = repo_root().join(docs_visuals::OUTPUT_DIR);
    for name in generated_names() {
        let expected = std::fs::read(temporary.path().join(name))
            .unwrap_or_else(|error| panic!("reads regenerated {name}: {error}"));
        let actual = std::fs::read(committed.join(name)).unwrap_or_else(|error| {
            panic!(
                "docs/visuals/{name} must be committed; run \
                 `cargo run -p animsmith --example gen_docs_visuals` ({error})"
            )
        });
        assert!(
            actual == expected,
            "docs/visuals/{name} does not match the generator output; \
             run `cargo run -p animsmith --example gen_docs_visuals`"
        );
    }
}

#[test]
fn the_committed_directory_holds_exactly_the_generated_set_plus_its_hand_authored_files() {
    let committed = repo_root().join(docs_visuals::OUTPUT_DIR);
    let present: BTreeSet<String> = std::fs::read_dir(&committed)
        .expect("lists docs/visuals/")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("utf-8 visual name")
        })
        .collect();
    let mut expected: BTreeSet<String> = generated_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    // The provenance page and the hand-drawn symptom icons are the only
    // files in this directory that no generator writes.
    expected.insert("README.md".to_owned());
    expected.insert("icons".to_owned());
    assert_eq!(
        present, expected,
        "docs/visuals/ must hold exactly the generated visuals plus README.md and icons/"
    );

    // No manifest can regenerate a drawing, so the provenance page beside
    // them is the icons' inventory: every icon is accounted for there, and
    // the directory holds nothing but drawings.
    let provenance = std::fs::read_to_string(committed.join("README.md"))
        .expect("reads the visuals provenance page");
    let icons: BTreeSet<String> = std::fs::read_dir(committed.join("icons"))
        .expect("lists docs/visuals/icons/")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("utf-8 icon name")
        })
        .collect();
    assert!(
        !icons.is_empty(),
        "docs/visuals/icons/ holds the hand-drawn symptom icons"
    );
    for icon in &icons {
        assert!(
            icon.ends_with(".svg"),
            "docs/visuals/icons/ holds drawings only, found {icon}"
        );
        assert!(
            provenance.contains(&format!("icons/{icon}")),
            "docs/visuals/README.md must account for the hand-authored icons/{icon}"
        );
    }
}

/// Every committed document identifies each input it was rendered from
/// by content, and none of them embeds a path from the checkout that
/// rendered it. A single-input report records the basename the command
/// was given; a comparison records each side's digest under its own
/// `identity`, so the before and after cannot be confused for one
/// another.
///
/// An input an earlier invocation produced is held to the same rule
/// against the bytes that invocation writes *now*: a committed
/// comparison whose after side stopped being the current output of the
/// command it claims would no longer carry that digest in
/// `after.identity.sha256`.
#[test]
fn every_committed_document_identifies_its_inputs_by_content_and_embeds_no_checkout_path() {
    let root = repo_root();
    let committed = root.join(docs_visuals::OUTPUT_DIR);
    let scratch = scratch_dir();
    // Nothing this test runs may write into the checkout, so the only
    // invocations it makes are the ones that write an intermediate, and
    // even their unused output directory is a throwaway.
    let unused = tempfile::tempdir().expect("creates output dir");
    let mut compared = 0usize;
    let mut prepared = 0usize;
    for command in docs_visuals::COMMANDS {
        // An invocation that writes an intermediate is run so the digest
        // its reader claims is the one the command produces today.
        if command.output.starts_with(docs_visuals::SCRATCH) {
            docs_visuals::run_docs_command(command, unused.path(), scratch.path(), &mut run)
                .unwrap_or_else(|error| panic!("prepares {}: {error}", command.output));
            prepared += 1;
            continue;
        }

        let html = std::fs::read_to_string(committed.join(command.output))
            .unwrap_or_else(|error| panic!("reads {}: {error}", command.output));
        assert!(
            !html.contains(docs_visuals::WORKING_DIR),
            "{} embeds a checkout-relative path; render it from the fixture directory",
            command.output
        );

        let (before, after) = inputs(command.arguments);
        assert_eq!(
            before.len(),
            1,
            "{} reads {before:?} rather than one clip per side",
            command.output
        );
        match after {
            Some(after) => {
                let data = embedded_json(&html, "comparison-report-data", command.output);
                for (side, input) in [("before", before[0]), ("after", after)] {
                    assert_eq!(
                        data[side]["identity"]["sha256"],
                        input_digest(input, scratch.path()),
                        "{} must carry {input} as its {side} side",
                        command.output
                    );
                }
                compared += 1;
            }
            None => {
                let data = embedded_json(&html, "report-data", command.output);
                assert_eq!(
                    data["file"], before[0],
                    "{} must name the fixture it was rendered from",
                    command.output
                );
            }
        }
    }
    assert!(
        prepared > 0 && compared > 0,
        "the manifest keeps a comparison against a clip AnimSmith itself produced"
    );
}

/// A document whose input an earlier invocation wrote must not depend on
/// where that invocation put it. The comparison records each side by
/// content identity rather than by path, so rendering it into two
/// differently named scratch directories yields the same bytes — which is
/// what lets the result be committed at all, and what stops a
/// regenerate-and-commit cycle from laundering one machine's temporary
/// path into `docs/visuals/`.
#[test]
fn a_prepared_report_does_not_depend_on_where_its_input_was_written() {
    let mut runs: Vec<Vec<(&str, String)>> = Vec::new();
    for prefix in [
        "animsmith-scratch-a-",
        "animsmith-scratch-b-with-a-longer-name-",
    ] {
        let scratch = scratch_dir_named(prefix);
        let out_dir = tempfile::tempdir().expect("creates output dir");
        let scratch_path = scratch.path().to_str().expect("utf-8 scratch path");
        let mut rendered = Vec::new();
        for command in docs_visuals::COMMANDS {
            let writes = command.output.starts_with(docs_visuals::SCRATCH);
            let reads = command
                .arguments
                .iter()
                .any(|argument| argument.starts_with(docs_visuals::SCRATCH));
            if !writes && !reads {
                continue;
            }
            docs_visuals::run_docs_command(command, out_dir.path(), scratch.path(), &mut run)
                .unwrap_or_else(|error| panic!("renders {}: {error}", command.output));
            if reads {
                let html = std::fs::read_to_string(out_dir.path().join(command.output))
                    .unwrap_or_else(|error| {
                        panic!("reads the rendered {}: {error}", command.output)
                    });
                assert!(
                    !html.contains(scratch_path),
                    "{} embeds the scratch directory {scratch_path} it was rendered from",
                    command.output
                );
                rendered.push((command.output, html));
            }
        }
        assert!(
            !rendered.is_empty(),
            "the manifest keeps a document whose input AnimSmith itself produced"
        );
        runs.push(rendered);
    }
    assert_eq!(
        runs[0].iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        runs[1].iter().map(|(name, _)| *name).collect::<Vec<_>>()
    );
    for ((name, one), (_, other)) in runs[0].iter().zip(&runs[1]) {
        assert!(
            one == other,
            "{name} differs between two scratch directories, so its bytes are not committable"
        );
    }
}

/// The `docs/visuals/` file one reference on `page` names, if it names
/// one.
///
/// The destination is resolved against the page's own directory, so a
/// link to `../elsewhere/walk.report.html` — or to some other directory's
/// file of the same name — does not vouch for the visual here.
fn referenced_visual(page: &Path, destination: &str, visuals: &Path) -> Option<String> {
    let path = destination.split(['#', '?']).next().unwrap_or_default();
    if path.is_empty() || path.contains("://") {
        return None;
    }
    let mut resolved = page.parent()?.to_path_buf();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                resolved.pop();
            }
            std::path::Component::Normal(part) => resolved.push(part),
            _ => return None,
        }
    }
    (resolved.parent() == Some(visuals))
        .then(|| resolved.file_name()?.to_str().map(str::to_owned))
        .flatten()
}

/// Every `docs/visuals/` file one page embeds, pictures or links —
/// counting only what it renders, so a fenced example cannot vouch for a
/// visual nothing shows.
fn referenced_visuals(page: &Path, markdown: &str, visuals: &Path) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = animsmith_testkit::docs_markdown::rendered_html_references(
        markdown,
        &[("img", "src"), ("iframe", "src")],
    )
    .into_iter()
    .filter_map(|(_, destination)| referenced_visual(page, &destination, visuals))
    .collect();
    for event in pulldown_cmark::Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) = event
            && let Some(name) = referenced_visual(page, &dest_url, visuals)
        {
            names.insert(name);
        }
    }
    names
}

/// Every tracked page under `docs/`, except the provenance page, which
/// lists the whole directory and would vouch for anything in it.
fn documentation_pages(directory: &Path, pages: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("lists {}: {error}", directory.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            documentation_pages(&path, pages);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md")
            && path != repo_root().join(docs_visuals::OUTPUT_DIR).join("README.md")
        {
            pages.push(path);
        }
    }
}

/// A committed visual earns its bytes by being shown to a reader.
///
/// The drift guard above proves every generated file is what the tool
/// makes; this proves the set is no larger than the documentation needs.
/// A visual qualifies by being embedded, linked or pictured on a page, or
/// by being the report a committed chart is cut from — `walk.report.html`
/// is the second kind, embedded nowhere but the source of two charts.
/// Anything else is a rendered document no reader can reach, and it is
/// the manifest entry that should go rather than the file that should
/// stay.
#[test]
fn every_committed_visual_is_shown_on_a_page_or_is_a_charts_source() {
    let root = repo_root();
    let mut pages = Vec::new();
    documentation_pages(&root.join("docs"), &mut pages);
    assert!(pages.len() >= 10, "docs/ publishes the customer pages");

    let visuals = root.join(docs_visuals::OUTPUT_DIR);
    let mut referenced = BTreeSet::new();
    for page in &pages {
        let markdown =
            std::fs::read_to_string(page).unwrap_or_else(|error| panic!("reads {page:?}: {error}"));
        referenced.append(&mut referenced_visuals(page, &markdown, &visuals));
    }
    let chart_sources: BTreeSet<&str> = docs_visuals::CHARTS
        .iter()
        .map(|chart| chart.report)
        .collect();

    let unreachable: Vec<&str> = generated_names()
        .into_iter()
        .filter(|name| !referenced.contains(*name) && !chart_sources.contains(name))
        .collect();
    assert_eq!(
        unreachable,
        Vec::<&str>::new(),
        "every committed visual must be shown on a page under docs/ or be the report a \
         committed chart is cut from; drop the manifest entry and the file instead"
    );
}

/// Every colour a committed picture paints resolves through the
/// documentation theme's `--as-*` token, with the standalone value as its
/// fallback.
///
/// The site chooses light or dark by a class on the page, which an SVG's
/// own `prefers-color-scheme` cannot see: a reader who picks a theme
/// different from the operating system's would otherwise get a dark chart
/// on a light page. The build inlines these files into the pages that
/// show them, so the page's token wins there and the fallback still reads
/// correctly on GitHub or in a browser tab.
#[test]
fn every_committed_picture_paints_through_a_theme_token() {
    let visuals = repo_root().join(docs_visuals::OUTPUT_DIR);
    let mut checked = 0usize;
    for directory in [visuals.clone(), visuals.join("icons")] {
        for entry in std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("lists {}: {error}", directory.display()))
        {
            let path = entry.expect("directory entry").path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("svg") {
                continue;
            }
            let svg = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("reads {}: {error}", path.display()));
            let name = path.file_name().expect("a picture has a name").display();
            assert!(
                svg.contains("var(--as-"),
                "{name} paints nothing through a theme token"
            );
            assert_eq!(
                untokenized_colours(&svg),
                Vec::<String>::new(),
                "{name} paints a colour the page's theme cannot reach"
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 6,
        "every committed picture is checked, saw {checked}"
    );
}

/// Every colour literal in `svg` that is not the fallback of an `--as-*`
/// token, in document order.
fn untokenized_colours(svg: &str) -> Vec<String> {
    let mut loose = Vec::new();
    for (offset, _) in svg.match_indices('#') {
        let hex: String = svg[offset + 1..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        // `#chart-…` and `#icon-…` are selectors, not colours: a colour
        // is three, four, six or eight hexadecimal digits.
        if !matches!(hex.len(), 3 | 4 | 6 | 8) {
            continue;
        }
        let token = svg[..offset]
            .trim_end()
            .strip_suffix(',')
            .and_then(|head| head.rsplit_once("var(").map(|(_, token)| token));
        if !token.is_some_and(|token| {
            token.starts_with("--as-") && !token.contains(['(', ')', ',', ' '])
        }) {
            loose.push(format!("#{hex}"));
        }
    }
    loose
}

/// Every fixture a committed report is rendered from actually moves.
///
/// A report's pose view is drawn from the sampled pose grid, which holds
/// each bone's model-space position on every judged frame. A bone's own
/// rotation moves its children, never itself, so a clip whose only
/// animated joint is a leaf — or whose rotation turns about the axis its
/// child already lies on — leaves that grid constant: the report reports
/// real findings while drawing one unchanging skeleton on every frame,
/// and a reader scrubbing it sees nothing but the time label change.
/// `examples/assets/clip.glb` shipped in exactly that state, so the floor
/// is pinned here rather than left to whoever next opens a document.
///
/// One centimetre is deliberately low: it is far below anything a fixture
/// authored to demonstrate a symptom would move, and far above the noise
/// of a genuinely static bone.
///
/// A fixture that passes this is also the kind whose committed report is
/// sensitive to how the model matrices are multiplied, because its bone
/// positions come from a rotation rather than from exact adds. The
/// workspace pins `glam` to scalar arithmetic and the `libm` crate for
/// exactly that reason — see the cross-platform determinism section of
/// `DEVELOPMENT.md`.
#[test]
fn every_report_fixture_moves_a_bone_the_reader_can_see() {
    const FLOOR_M: f32 = 0.01;
    let fixtures = repo_root().join(docs_visuals::WORKING_DIR);
    // An intermediate is a command's own output rather than a committed
    // fixture, and it is the fixture beside it that this floor is about:
    // `clip-dirty.glb` is judged here, and its repair moves the same bone.
    let inputs: BTreeSet<&str> = docs_visuals::COMMANDS
        .iter()
        .flat_map(|command| command.arguments.iter().copied())
        .filter(|argument| {
            argument.ends_with(".glb") && !argument.starts_with(docs_visuals::SCRATCH)
        })
        .collect();
    assert!(!inputs.is_empty(), "the manifest names committed fixtures");

    for input in inputs {
        let document = animsmith_gltf::load(&fixtures.join(input))
            .unwrap_or_else(|error| panic!("loads the {input} fixture: {error}"));
        let grids = animsmith_core::metrics::MetricGrids::new(&document);
        for (index, clip) in document.clips.iter().enumerate() {
            let grid = grids
                .grid(index)
                .unwrap_or_else(|| panic!("{input} clip '{}' has a pose grid", clip.name));
            let travel = (0..grid.bone_count())
                .map(|bone| {
                    let first = grid.model_position(0, bone);
                    (1..grid.frame_count())
                        .map(|frame| grid.model_position(frame, bone).distance(first))
                        .fold(0.0f32, f32::max)
                })
                .fold(0.0f32, f32::max);
            assert!(
                travel >= FLOOR_M,
                "{input} clip '{}' draws the same skeleton on every judged frame \
                 (widest bone travel {travel:.4} m, floor {FLOOR_M} m): a report of it \
                 shows a still picture. Animate a joint that has a child, about an axis \
                 that child does not already lie on.",
                clip.name,
            );
        }
    }
}
