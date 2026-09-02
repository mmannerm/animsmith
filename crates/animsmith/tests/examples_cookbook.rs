//! Drift guards for the examples cookbook (examples/README.md) and its
//! committed assets (examples/assets/). Two kinds of coverage:
//!
//! 1. `example_assets_match_generator_output` rebuilds the committed
//!    `.glb` from the shared `animsmith-testkit` documents (the same
//!    ones `gen_example_assets` writes) and asserts the bytes match, so
//!    the checked-in assets can never silently drift from the generator.
//! 2. The `cookbook_*` tests run the commands the cookbook documents
//!    against the committed assets and assert each one's exit code plus
//!    a distinctive contract detail. The user-visible `transform`
//!    transcripts are pinned verbatim because they include the complete
//!    written-artifact summary.

use animsmith_testkit::docs_markdown::fenced_blocks;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, Output};

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

/// A committed cookbook asset under `examples/assets/`.
fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/assets")
        .join(name)
}

fn unique_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("animsmith-cookbook-{name}-"))
        .tempdir()
        .expect("creates temp dir")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Run the CLI with `args` and return (exit code, stdout).
fn run(args: &[&str]) -> (Option<i32>, String) {
    let output = animsmith().args(args).output().expect("runs animsmith");
    (output.status.code(), stdout(&output))
}

// --- 1. Committed assets track the generator -------------------------

#[test]
fn example_assets_match_generator_output() {
    // The generator (crates/animsmith/examples/gen_example_assets.rs) and
    // this test both write the committed assets through the same
    // animsmith-testkit `write_example_assets` wiring, so a wrong
    // filename, dropped asset, or swapped clean/dirty document fails here
    // — not just when a human reruns the generator. (#117 replaced an
    // earlier `cargo run --example` subprocess with this in-process build.)
    let tmp = unique_temp_dir("gen");
    animsmith_testkit::write_example_assets(tmp.path(), |doc, path| {
        animsmith_gltf::write::write(doc, path).map(|_| ())
    })
    .expect("writes example assets");

    for name in [
        "clip.glb",
        "clip-dirty.glb",
        "walk.glb",
        "walk-dirty.glb",
        "walk-short-channel.glb",
        "walk-travel.glb",
        "run-ring.glb",
        "walk-frozen-arm.glb",
        "walk-scaled.glb",
        "report-comparison-before.glb",
        "report-comparison-after.glb",
    ] {
        let committed = std::fs::read(asset(name)).expect("reads committed asset");
        let regenerated = std::fs::read(tmp.path().join(name))
            .unwrap_or_else(|e| panic!("generator did not write {name}: {e}"));
        if committed != regenerated {
            // Report sizes/offset rather than dumping two 896-byte vectors.
            // A pure length change (identical prefix) has no differing
            // byte, so fall back to the point where the shorter file ends.
            let offset = committed
                .iter()
                .zip(&regenerated)
                .position(|(a, b)| a != b)
                .unwrap_or(committed.len().min(regenerated.len()));
            panic!(
                "examples/assets/{name} is stale ({} committed bytes vs {} regenerated, \
                 first difference at byte {offset}) — regenerate with \
                 `cargo run -p animsmith --example gen_example_assets`",
                committed.len(),
                regenerated.len(),
            );
        }
    }
}

#[test]
fn canonical_character_assembly_example_keeps_all_property_pruning_disabled() {
    let recipe: toml::Value =
        toml::from_str(include_str!("../../../examples/character-assembly.toml"))
            .expect("canonical character-assembly example parses as TOML");

    assert_eq!(
        recipe["prune_constant_tracks"].as_bool(),
        Some(false),
        "all-property pruning can remove completion-generated transition coverage"
    );
}

// --- 2. Documented commands still behave as the cookbook shows -------
//
// Covers every command in examples/README.md that runs against the
// committed assets. The cookbook's remaining commands target placeholder
// or FBX assets this repo does not ship (the convert/report/embed
// sections), so they are not smoke-tested here; the worked
// character.animsmith.toml parse is covered separately by
// `example_config_parses_verbatim` in cli_contract.rs.

/// The commands of the cookbook's first section behave as it says.
///
/// What the page *prints* is compared line by line by
/// `every_runnable_cookbook_transcript_still_matches_the_cli`; this covers
/// the contract details around them — exit codes, `--select`/`--allow`
/// steering, and the JSON envelope the page only projects.
#[test]
fn cookbook_first_gate() {
    let clean = asset("clip.glb");
    let clean = clean.to_str().unwrap();
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();

    let (code, out) = run(&["inspect", clean]);
    assert_eq!(code, Some(0), "inspect clean");
    assert!(out.contains("swing"), "inspect names the clip: {out}");

    let (code, out) = run(&["measure", "--format", "json", clean]);
    assert_eq!(code, Some(0), "measure clean exits 0");
    let doc: Value = serde_json::from_str(&out).expect("measure --format json is valid JSON");
    assert!(
        doc["files"][0]["measurements"]["clips"]
            .get("swing")
            .is_some(),
        "measure reports the clip's metrics: {out}"
    );

    let (code, out) = run(&["lint", clean]);
    assert_eq!(code, Some(0), "lint clean exits 0");
    assert!(
        out.contains("0 error(s)"),
        "lint reports no findings: {out}"
    );

    let (code, out) = run(&["lint", dirty]);
    assert_eq!(code, Some(1), "lint dirty exits 1");
    assert!(
        out.contains("quat-norm") && out.contains("quat-flip"),
        "lint dirty names both checks: {out}"
    );

    // The documented `--deny-warnings` command exits 1 on the dirty
    // asset and still prints both findings.
    let (code, out) = run(&["lint", "--deny-warnings", dirty]);
    assert_eq!(code, Some(1), "--deny-warnings dirty exits 1");
    assert!(
        out.contains("quat-norm") && out.contains("quat-flip"),
        "--deny-warnings still reports the findings: {out}"
    );

    // Prove the promotion itself: --select isolates the warning (exit 0
    // confirms the quat-norm error was dropped), then --deny-warnings
    // flips that warning-only run to 1.
    let (code, out) = run(&["lint", "--select", "quat-flip", dirty]);
    assert_eq!(code, Some(0), "warning-only run exits 0");
    assert!(
        out.contains("quat-flip") && !out.contains("quat-norm"),
        "--select isolates the warning: {out}"
    );
    let (code, _) = run(&["lint", "--deny-warnings", "--select", "quat-flip", dirty]);
    assert_eq!(code, Some(1), "--deny-warnings promotes the warning");

    let (code, out) = run(&["lint", "--format", "json", dirty]);
    assert_eq!(code, Some(1), "json lint dirty exits 1");
    let doc: Value = serde_json::from_str(&out).expect("lint --format json is valid JSON");
    let ids: Vec<&str> = doc["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .flat_map(|check| check["findings"].as_array().unwrap())
        .filter_map(|finding| finding["check_id"].as_str())
        .collect();
    assert!(
        ids.contains(&"quat-norm") && ids.contains(&"quat-flip"),
        "json findings name both checks: {ids:?}"
    );
}

#[test]
fn cookbook_repair_roundtrip() {
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();
    let tmp = unique_temp_dir("repair");
    let fixed = tmp.path().join("fixed.glb");
    let fixed = fixed.to_str().unwrap();

    let (code, out) = run(&["fix", "--dry-run", dirty]);
    assert_eq!(code, Some(1), "dry-run with pending repairs exits 1");
    assert!(out.contains("would fix"), "dry-run reports repairs: {out}");

    let (code, _) = run(&["fix", dirty, "-o", fixed]);
    assert_eq!(code, Some(0), "fix -o exits 0");

    let (code, out) = run(&["lint", fixed]);
    assert_eq!(code, Some(0), "repaired asset lints clean");
    assert!(
        out.contains("0 error(s)"),
        "repaired asset has no findings: {out}"
    );

    let (code, out) = run(&["diff", dirty, fixed]);
    assert_eq!(code, Some(0), "lossless repair diffs clean");
    assert!(
        out.contains("no significant movement"),
        "diff reports no movement: {out}"
    );
}

/// The `transform` transcripts, pinned as whole outputs.
///
/// The page's own copies are executed and compared by
/// `every_runnable_cookbook_transcript_still_matches_the_cli`; this adds
/// exact whole-stdout equality, which covers the written-artifact summary
/// the page abridges.
#[test]
fn cookbook_transform() {
    let clean = asset("clip.glb");
    let clean = clean.to_str().unwrap();
    let tmp = unique_temp_dir("transform");
    let sliced = tmp.path().join("sliced.glb");
    let sliced = sliced.to_str().unwrap();
    let held = tmp.path().join("held.glb");
    let held = held.to_str().unwrap();

    let (code, out) = run(&["transform", clean, "-o", sliced, "--slice", "0.5:1.0"]);
    assert_eq!(code, Some(0), "slice exits 0");
    assert_eq!(
        out,
        format!(
            "  sliced 'swing' to [0.5:1]s (3 keys max)\nwrote {sliced} (3 node(s), 1 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))\n"
        ),
        "slice transcript matches the cookbook"
    );

    let (code, out) = run(&["diff", clean, sliced]);
    assert_eq!(code, Some(1), "slice moves measurements, diff exits 1");
    // "moved" is a per-metric change line, so it distinguishes a moved
    // diff from the clean `0 significant change(s)` output (which also
    // contains the substring "significant change").
    assert!(out.contains("moved"), "diff lists the moved metrics: {out}");

    let (code, out) = run(&["transform", clean, "-o", held, "--hold-extend", "0.5"]);
    assert_eq!(code, Some(0), "hold-extend exits 0");
    assert_eq!(
        out,
        format!(
            "  hold-extended 'swing' by 0.5s\nwrote {held} (3 node(s), 1 clip(s), 0 mesh(es) / 0 position(s), 0 material(s))\n"
        ),
        "hold transcript matches the cookbook"
    );

    // Reuse the written file: the hold extends the clip's duration, so a
    // diff against the source reports movement — guards a no-write success.
    let (code, out) = run(&["diff", clean, held]);
    assert_eq!(code, Some(1), "hold-extend changes the clip, diff exits 1");
    assert!(out.contains("moved"), "diff lists the moved metrics: {out}");
}

#[test]
fn cookbook_synchronized_report_acceptance_matrix() {
    let before = asset("report-comparison-before.glb");
    let after = asset("report-comparison-after.glb");
    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/report-comparison.animsmith.toml");
    let tmp = unique_temp_dir("report-comparison");
    let report = tmp.path().join("comparison.html");
    let output = animsmith()
        .args([
            "--config",
            config.to_str().unwrap(),
            "report",
            before.to_str().unwrap(),
            "--compare-after",
            after.to_str().unwrap(),
            "--before-clip",
            "acceptance-matrix",
            "--after-clip",
            "acceptance-matrix",
            "--output",
            report.to_str().unwrap(),
        ])
        .output()
        .expect("runs documented comparison report");
    assert!(
        output.status.success(),
        "comparison report stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(&report).expect("reads comparison report");
    let marker = r#"<script type="application/json" id="comparison-report-data">"#;
    let data_start = html.find(marker).expect("comparison data marker") + marker.len();
    let data_end = html[data_start..]
        .find("</script>")
        .expect("comparison data close")
        + data_start;
    let data: Value =
        serde_json::from_str(&html[data_start..data_end]).expect("comparison data JSON");
    let before_findings = data["before"]["findings"]
        .as_array()
        .expect("before findings");
    assert!(
        before_findings
            .iter()
            .any(|row| { row["check"] == "loop-closure" && row["bone"] == "left_foot" })
    );
    assert!(
        before_findings
            .iter()
            .any(|row| { row["check"] == "foot-slide" && row["bone"] == "left_foot" })
    );
    assert!(
        before_findings
            .iter()
            .any(|row| row["check"] == "constant-track" && row["bone"] == "hand")
    );
    assert_eq!(
        data["before"]["contexts"]["stances"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        data["before"]["contexts"]["seams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["subject_bone_name"] == "left_foot")
    );
    assert_eq!(
        data["before"]["contexts"]["structural"][0]["evidence_kind"],
        "structural"
    );
    assert!(
        data["after"]["contexts"]["structural"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        data["after"]["findings"].as_array().unwrap().is_empty(),
        "the repaired example side should be matrix-clean"
    );

    let (code, diff) = run(&["diff", before.to_str().unwrap(), after.to_str().unwrap()]);
    assert_eq!(code, Some(1), "fixture pair intentionally changes motion");
    assert!(
        diff.contains("moved") || diff.contains("appeared") || diff.contains("disappeared"),
        "typed diff still reports measurement movement independently: {diff}"
    );
}

#[test]
fn cookbook_config_steering() {
    let dirty = asset("clip-dirty.glb");
    let dirty = dirty.to_str().unwrap();

    // --select runs only the named check.
    let (code, out) = run(&["lint", "--select", "quat-norm", dirty]);
    assert_eq!(code, Some(1), "select quat-norm still errors");
    assert!(out.contains("quat-norm"), "select keeps quat-norm: {out}");
    assert!(!out.contains("quat-flip"), "select drops quat-flip: {out}");

    // --allow suppresses the named check's findings; the quat-norm
    // error still fails the run.
    let (code, out) = run(&["lint", "--allow", "quat-flip", dirty]);
    assert_eq!(code, Some(1), "allow keeps the quat-norm error");
    assert!(
        out.contains("quat-norm") && !out.contains("quat-flip"),
        "allow hides only quat-flip, keeping quat-norm: {out}"
    );

    // A severity override demotes the warning to a note.
    let tmp = unique_temp_dir("config");
    let cfg = tmp.path().join("demote.toml");
    std::fs::write(&cfg, "[checks.quat-flip]\nseverity = \"note\"\n").expect("writes config");
    let (code, out) = run(&["lint", "--config", cfg.to_str().unwrap(), dirty]);
    assert_eq!(code, Some(1), "quat-norm error keeps exit 1");
    assert!(
        out.contains("note[quat-flip]"),
        "override demotes quat-flip to a note: {out}"
    );
}

/// The contract page shows one complete `animsmith.toml`, and a reader is
/// meant to copy it. It is therefore not retyped prose but the committed
/// contract this suite already runs against both walk fixtures, quoted line
/// for line: a page that drifts from the file would hand that reader a config
/// no gate has ever executed.
///
/// Line for line rather than byte for byte, because the two sides arrive by
/// different routes. The fence comes out of the Markdown parser, which
/// normalises every line ending to `\n`; the config is read raw and carries
/// whatever the checkout wrote, which under Windows `autocrlf` is CRLF.
/// Nothing embeds this file's bytes, so it earns no `text eol=lf` attribute
/// the way the report assets and drawings do — the promise here is the content
/// of each line, which is exactly what `str::lines` compares.
#[test]
fn contract_page_quotes_the_committed_walk_config_exactly() {
    let page = std::fs::read_to_string(repo_path("docs/declaring-the-contract.md"))
        .expect("reads the contract page");
    let quoted = fenced_blocks(&page, "toml");
    assert_eq!(
        quoted.len(),
        1,
        "the page shows exactly one complete contract"
    );
    let committed = std::fs::read_to_string(repo_path("examples/walk.animsmith.toml"))
        .expect("reads the committed walk contract");
    let shown = lines_of(&quoted[0]);
    let tracked = lines_of(&committed);
    // Named rather than dumped: two forty-line vectors side by side hide the
    // one line that actually moved.
    if let Some((number, (shown, tracked))) = shown
        .iter()
        .zip(&tracked)
        .enumerate()
        .find(|(_, (shown, tracked))| shown != tracked)
    {
        panic!(
            "the quoted contract must be examples/walk.animsmith.toml line for line; \
             line {} is {shown:?} on the page and {tracked:?} in the file",
            number + 1
        );
    }
    assert_eq!(
        shown.len(),
        tracked.len(),
        "the page must quote every line of examples/walk.animsmith.toml and no more"
    );
}

/// The lines of `text` with only `\r\n` folded to `\n`. Nothing else is
/// touched: trailing whitespace and a missing final newline stay differences,
/// because splitting on the newline alone keeps the empty last element a
/// newline-terminated file produces.
fn lines_of(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split('\n')
        .map(str::to_owned)
        .collect()
}

/// The code spans of one column of the table whose leading header cell is
/// `header`, one `Vec` per body row, read with the parser rather than by
/// splitting on `|` so a decoy table or a pipe in prose cannot feed the gate.
fn table_column_codes(markdown: &str, header: &str, column: usize) -> Vec<Vec<String>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let (mut in_head, mut is_wanted, mut cell) = (false, false, 0usize);
    let (mut first_header, mut codes, mut rows) = (None, Vec::new(), Vec::new());
    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Table(_)) => first_header = None,
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                in_head = false;
                is_wanted = first_header.as_deref() == Some(header);
            }
            Event::Start(Tag::TableRow) => cell = 0,
            Event::Start(Tag::TableCell) => cell += 1,
            Event::End(TagEnd::TableCell) if is_wanted && !in_head && cell == column => {
                rows.push(std::mem::take(&mut codes));
            }
            Event::Code(code) if is_wanted && !in_head && cell == column => {
                codes.push(code.into_string());
            }
            Event::Text(text) if in_head && cell == 1 => {
                first_header.get_or_insert_with(String::new).push_str(&text);
            }
            _ => {}
        }
    }
    rows
}

/// The Surface column names config keys in prose, outside the fenced block
/// and outside the runnable examples, so nothing else would catch a typo in
/// one: `movement_owner_zx` reads exactly like the real key and no gate would
/// see it. Each backticked `[table]` or key there must therefore appear in the
/// reference that owns the vocabulary.
#[test]
fn every_config_key_the_contract_table_names_exists_in_the_reference() {
    let page = std::fs::read_to_string(repo_path("docs/declaring-the-contract.md"))
        .expect("reads the contract page");
    let reference = std::fs::read_to_string(repo_path("docs/configuration-reference.md"))
        .expect("reads the configuration reference");
    let named = table_column_codes(&page, "Surface", 1);
    assert!(
        named.len() >= 10,
        "the surface table must name the config surface of every row: {named:?}"
    );

    let known = identifiers(&reference);
    let mut checked = 0usize;
    for (row, codes) in named.iter().enumerate() {
        for code in codes {
            for token in identifiers(code) {
                assert!(
                    known.contains(&token),
                    "row {} names {code:?}, whose {token:?} is not an identifier \
                     docs/configuration-reference.md spells",
                    row + 1
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 20,
        "every identifier the surface column spells is looked up, found {checked}"
    );
}

/// Every complete identifier `text` spells: a run of ASCII letters, digits and
/// underscores, with a bare number dropped.
///
/// The same rule runs over both sides, which is the whole point. Asking
/// whether the reference *contains* a key matched any prefix of one, so
/// `movement_owner_x` — a key no config would take — passed on the strength
/// of the real `movement_owner_xz`, and the page could have shipped it.
fn identifiers(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|token| !token.is_empty() && !token.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_owned)
        .collect()
}

/// The boundary rule is what makes that lookup mean anything, so it is proved
/// on a fixture rather than trusted: a prefix of a real key is a different
/// identifier, and both sides tokenise alike.
#[test]
fn an_identifier_that_merely_prefixes_a_real_key_is_not_found() {
    let reference = "| `clips.<selector>.movement_owner_xz` | optional enum; omitted |\n";
    let known = identifiers(reference);
    assert!(
        known.contains("movement_owner_xz"),
        "the real key is found: {known:?}"
    );
    for truncated in ["movement_owner_x", "movement_owner", "movement"] {
        assert!(
            !known.contains(truncated),
            "{truncated:?} is a prefix, not the key: {known:?}"
        );
    }
    assert_eq!(
        identifiers("[clips.\"<name>\"] movement_owner_xz"),
        ["clips", "name", "movement_owner_xz"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "the page side spells the same identifiers the reference side does"
    );
}

/// Every "Minimal example" in the contract page is offered to a reader as a
/// config to copy, so each one has to be a config the binary accepts. A cell
/// that omits a required key — a gait group without its phase tolerance, an
/// `[engine]` table without its full tuple — is refused with exit `2` and no
/// asset is linted, which is the failure this runs for. Findings are fine:
/// only a config refusal fails the gate.
///
/// Each snippet is linted on its own rather than appended to the walk
/// contract, because several declare a table that contract already opens
/// (`[rig]`, `[clips.walk]`) and TOML refuses the duplicate — a failure the
/// test would have manufactured itself.
#[test]
fn every_documented_contract_surface_is_a_config_the_binary_accepts() {
    let page = std::fs::read_to_string(repo_path("docs/declaring-the-contract.md"))
        .expect("reads the contract page");
    let examples = table_column_codes(&page, "Surface", 5);
    assert!(
        examples.len() >= 10,
        "the surface table must document an example per row: {examples:?}"
    );
    let walk = asset("walk.glb");
    let temp = unique_temp_dir("contract-surfaces");
    let mut linted = 0usize;
    for (row, snippet) in examples.iter().enumerate() {
        if snippet.is_empty() {
            // A row whose example is a link to another page's contract.
            continue;
        }
        let config = temp.path().join(format!("surface-{row}.animsmith.toml"));
        std::fs::write(&config, snippet.join("\n") + "\n").expect("writes the documented config");
        let output = animsmith()
            .args(["lint", "--config"])
            .arg(&config)
            .arg(&walk)
            .output()
            .expect("runs animsmith lint");
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "row {} documents a config the binary refuses ({:?}):\n{}\nstderr:\n{}",
            row + 1,
            output.status.code(),
            snippet.join("\n"),
            String::from_utf8_lossy(&output.stderr)
        );
        linted += 1;
    }
    assert!(linted >= 10, "every documented surface example is linted");
}

#[test]
fn cookbook_semantic_contract() {
    let walk = asset("walk.glb");
    let walk = walk.to_str().unwrap();
    let dirty = asset("walk-dirty.glb");
    let dirty = dirty.to_str().unwrap();
    let config = repo_path("examples/walk.animsmith.toml");
    let config = config.to_str().unwrap();

    // The rig's bone names resolve a built-in profile with no config.
    let (code, out) = run(&["inspect", walk]);
    assert_eq!(code, Some(0), "inspect walk");
    assert!(
        out.contains("ue-mannequin"),
        "inspect detects the profile: {out}"
    );

    // The clean rig passes its contract.
    let (code, out) = run(&["lint", "--config", config, walk]);
    assert_eq!(code, Some(0), "clean walk passes its contract");
    assert!(out.contains("0 error(s)"), "walk has no findings: {out}");

    // The popped-seam rig fails C0 pose closure, locomotion-relative seam,
    // and C1 velocity continuity under the same contract.
    let (code, out) = run(&["lint", "--config", config, dirty]);
    assert_eq!(code, Some(1), "popped seam fails loop-seam");
    assert!(out.contains("loop-seam"), "names loop-seam: {out}");
    let (_, json) = run(&["lint", "--config", config, "--format", "json", dirty]);
    let doc: Value = serde_json::from_str(&json).expect("lint --format json is valid JSON");
    let ids: Vec<(&str, &str)> = doc["files"][0]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .flat_map(|check| check["findings"].as_array().unwrap())
        .map(|finding| {
            (
                finding["check_id"].as_str().unwrap_or_default(),
                finding["severity"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        ids,
        vec![
            ("loop-closure", "error"),
            ("loop-seam", "error"),
            ("loop-seam-vel", "error"),
        ],
        "the popped seam produces the three complementary findings: {ids:?}"
    );

    // Without the contract, all loop checks have nothing to judge and skip —
    // the semantic checks enforce declared expectations, not a guess.
    let (code, out) = run(&["lint", dirty]);
    assert_eq!(code, Some(0), "bare lint skips loop-seam");
    assert!(
        !out.contains("loop-seam"),
        "bare lint does not run loop-seam: {out}"
    );
    assert!(
        out.contains("0 error(s)"),
        "bare lint has no findings: {out}"
    );
}

#[test]
fn cookbook_addressability_inventory() {
    let walk = asset("walk.glb");
    let walk = walk.to_str().unwrap();

    let (code, out) = run(&["generate", "addressability", walk]);
    assert_eq!(code, Some(0), "neutral addressability generation exits 0");
    let doc: Value = serde_json::from_str(&out).expect("default addressability output is JSON");
    assert_eq!(
        doc["schema"],
        "urn:animsmith:schema:gltf-animation-addressability:1"
    );
    assert_eq!(
        doc["inventory"]["animations"]["coverage"]["state"],
        "complete"
    );
    assert_eq!(
        doc["inventory"]["animations"]["rows"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert!(
        doc["bevy"].is_null(),
        "neutral output has no adapter: {out}"
    );

    let config =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/bevy.animsmith.toml");
    let (code, out) = run(&[
        "--config",
        config.to_str().unwrap(),
        "generate",
        "addressability",
        walk,
        "--format",
        "markdown",
    ]);
    assert_eq!(code, Some(0), "exact Bevy Markdown generation exits 0");
    assert!(
        out.starts_with("# glTF animation addressability v1\n") && out.contains("Animation0"),
        "Bevy presentation renders the documented selector: {out}"
    );
}

// --- 3. The cookbook's transcripts are executed, not spot-checked ----

/// Every path-shaped argument a documented command reads.
///
/// The value of `-o`/`--output` is what the command writes, so it is not
/// expected to exist beforehand — requiring it would skip every command
/// that produces a file, and with it every command reading what it wrote.
fn named_inputs(command: &str) -> Vec<&str> {
    const SUFFIXES: [&str; 6] = [".glb", ".gltf", ".fbx", ".toml", ".html", ".json"];
    let words: Vec<&str> = command.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .filter(|(index, word)| {
            SUFFIXES.iter().any(|suffix| word.ends_with(suffix))
                && !matches!(
                    index.checked_sub(1).map(|at| words[at]),
                    Some("-o" | "--output")
                )
        })
        .map(|(_, word)| *word)
        .collect()
}

/// A checkout-shaped directory holding the committed `examples/` tree, so
/// a documented relative path resolves exactly as it does for a reader.
fn cookbook_checkout() -> tempfile::TempDir {
    let temporary = unique_temp_dir("readme");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    animsmith_testkit::docs_transcripts::copy_tree(&source, &temporary.path().join("examples"));
    temporary
}

/// The cookbook's console transcripts are real output, and this runs them.
///
/// Every `$ animsmith …` command whose named files all exist by the time
/// the page reaches it is executed in a throwaway copy of `examples/`, in
/// the order the page prints it, and every line the page quotes under it
/// must still appear in that order — with a trailing `...` promising only
/// its prefix, the cookbook's own convention. The rest are skipped: the
/// page also documents placeholder inputs a reader supplies (`export.fbx`,
/// `old.glb`) and configs it tells the reader to write.
///
/// Spot-checking a transcript for a substring is how the `diff` block in
/// the transform section went one row stale on `main` without any gate
/// noticing, and how the first-gate `lint` block kept printing a coverage
/// line and a summary the CLI had stopped emitting.
#[test]
fn every_runnable_cookbook_transcript_still_matches_the_cli() {
    use animsmith_testkit::docs_markdown::fenced_blocks;
    use animsmith_testkit::docs_transcripts::{documented_commands, misdocumented_line};

    let page = "examples/README.md";
    let markdown = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(page),
    )
    .expect("reads the cookbook");
    let checkout = cookbook_checkout();
    let mut ran = 0usize;
    let mut skipped = 0usize;
    let mut projections = 0usize;

    for block in fenced_blocks(&markdown, "console") {
        for documented in documented_commands(&block, page) {
            let Some(arguments) = documented.command.strip_prefix("animsmith ") else {
                continue;
            };
            // A trailing `# …` aside is for the reader, not the shell.
            let arguments = arguments
                .split_once(" # ")
                .map_or(arguments, |(head, _)| head);
            if arguments.contains('|') {
                skipped += 1;
                continue;
            }
            let missing = named_inputs(arguments)
                .into_iter()
                .any(|path| !checkout.path().join(path).exists());
            if missing {
                skipped += 1;
                continue;
            }

            let output = Command::new(env!("CARGO_BIN_EXE_animsmith"))
                .args(arguments.split_whitespace())
                .current_dir(checkout.path())
                .output()
                .unwrap_or_else(|error| panic!("{page}: runs {arguments}: {error}"));
            let printed = stdout(&output);
            if let Some(claimed) = documented.exit {
                assert_eq!(
                    output.status.code(),
                    Some(claimed),
                    "{page}: `animsmith {arguments}` exit code changed\nstdout:\n{printed}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            // The page's own preamble says its JSON envelopes are abridged
            // illustrative projections with placeholder digests and build
            // provenance, so those are run for their exit code and not
            // compared line by line. Everything else is real output.
            let projected = documented
                .output
                .first()
                .is_some_and(|line| line.trim_start().starts_with('{'));
            if projected {
                projections += 1;
                ran += 1;
                continue;
            }
            let lines: Vec<&str> = printed.lines().collect();
            if let Some(index) = misdocumented_line(&documented.output, &lines) {
                let line = &documented.output[index];
                panic!(
                    "{page}: `animsmith {arguments}` no longer prints {line:?} where the page \
                     documents it, below the lines quoted before it\nstdout:\n{printed}"
                );
            }
            ran += 1;
        }
    }
    assert!(
        ran >= 22,
        "the cookbook must keep documenting runnable commands, ran {ran} and skipped {skipped}"
    );
    assert!(
        ran - projections >= 19,
        "most runnable cookbook commands must have their output compared, not just their \
         exit code: {ran} ran, {projections} of them abridged JSON projections"
    );
}
