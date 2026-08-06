//! CLI contract for the `scale` producer.
//!
//! Every test here drives the real binary, because the properties under test
//! are the producer's: which files exist afterwards, what the exit code is,
//! which stream carried the record, and whether two identical invocations
//! produce identical bytes. None of those is observable from inside the
//! library.
//!
//! This file must keep building and passing under `--no-default-features`:
//! `scale` is the first evidence-emitting producer in the minimal binary, and
//! a feature-gated import here would silently drop that coverage.

use animsmith_testkit::{rest_bind_scale_rig_glb, rest_bind_scale_rig_gltf};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SCALE_EVIDENCE_SCHEMA: &str =
    include_str!("../../../docs/schemas/scale-evidence-v1.schema.json");
const SCALE_EVIDENCE_SCHEMA_ID: &str = "urn:animsmith:schema:scale-evidence:1";

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A temporary directory holding the §D.3 case 2 rig as `rig.glb`.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        std::fs::write(dir.path().join("rig.glb"), rest_bind_scale_rig_glb())
            .expect("writes the rig fixture");
        Self { dir }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// `scale rest-bind` on the rig at the declared factor, into `out.glb`
    /// and `out.json`.
    fn rest_bind(&self, expected_factor: &str, format: &str) -> Output {
        animsmith()
            .current_dir(self.dir.path())
            .args([
                "scale",
                "rest-bind",
                "rig.glb",
                "-o",
                "out.glb",
                "--source-skin-index",
                "0",
                "--source-root-node-index",
                "0",
                "--expected-factor",
                expected_factor,
                "--evidence",
                "out.json",
                "--format",
                format,
            ])
            .output()
            .expect("runs animsmith")
    }

    fn whole_document(&self, factor: &str, format: &str) -> Output {
        animsmith()
            .current_dir(self.dir.path())
            .args([
                "scale",
                "whole-document",
                "rig.glb",
                "-o",
                "out.glb",
                "--factor",
                factor,
                "--evidence",
                "out.json",
                "--format",
                format,
            ])
            .output()
            .expect("runs animsmith")
    }
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(SCALE_EVIDENCE_SCHEMA).expect("valid schema JSON");
    jsonschema::options()
        .build(&schema)
        .expect("scale evidence schema compiles")
}

fn assert_schema_valid(instance: &Value) {
    let validator = validator();
    let errors: Vec<String> = validator
        .iter_errors(instance)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect();
    assert!(errors.is_empty(), "schema errors: {}", errors.join("; "));
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("reads JSON")).expect("valid JSON")
}

// --- Published pairs --------------------------------------------------------

#[test]
fn rest_bind_publishes_a_pair_whose_evidence_names_the_appendix_d3_case_2_rewrite() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.01", "text");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(
        stdout(&output).starts_with("wrote out.glb and out.json (rest-bind factor 0.01,"),
        "stdout:\n{}",
        stdout(&output)
    );

    let record = read_json(&fixture.path("out.json"));
    assert_schema_valid(&record);
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["schema"], SCALE_EVIDENCE_SCHEMA_ID);
    assert_eq!(record["command"], "scale");
    assert_eq!(record["outcome"], "published");
    assert_eq!(record["rejection"], Value::Null);
    assert_eq!(record["operation"]["kind"], "rest-bind");
    assert_eq!(record["operation"]["source_skin_index"], 0);
    assert_eq!(record["operation"]["source_root_node_index"], 0);

    // Declared paths, verbatim: the canonical forms the distinctness check
    // computes are never serialized.
    assert_eq!(record["paths"]["input"], "rig.glb");
    assert_eq!(record["paths"]["output"], "out.glb");
    assert_eq!(record["paths"]["evidence"], "out.json");

    // The affected closure in raw source-node space: the scaled root, its
    // joint, and the transform-only attachment. Node 3, the mesh holder, is
    // outside it. Skin 0's only joint is node 1, which is inside.
    let result = &record["result"];
    assert_eq!(
        result["affected"]["source_nodes"],
        serde_json::json!([0, 1, 2])
    );
    assert_eq!(result["affected"]["source_skins"], serde_json::json!([0]));
    assert_eq!(result["affected"]["transform_only_attachment_count"], 1);

    // Exactly the §D.2 node members this operation rewrites: the root's
    // absorbed local scale, and the two descendant local translations.
    assert_eq!(
        result["artifact"]["rewritten_json_pointers"],
        serde_json::json!([
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ])
    );
    // The inverse-bind accessor and the affected translation output.
    assert_eq!(
        result["artifact"]["rewritten_accessors"],
        serde_json::json!([3, 5])
    );
    assert_eq!(result["artifact"]["container"], "glb");
    assert_eq!(result["proof"]["read_back_digest_matches"], true);

    // The published digest and byte count describe the file that landed.
    let published = std::fs::read(fixture.path("out.glb")).expect("reads the artifact");
    assert_eq!(
        result["artifact"]["bytes"].as_u64(),
        Some(published.len() as u64)
    );

    // §D.6 wants both observed-factor witnesses. The declared factor is the
    // exact `f64` decimal `0.01`; both witnesses are the `f32` the loader
    // read for the root's authored `0.01`, which is a different number.
    assert_eq!(result["factors"]["declared"], 0.01);
    let observed = f64::from(0.01f32);
    assert_eq!(result["factors"]["planned_observed"], observed);
    assert_eq!(result["factors"]["proved_observed"], observed);
    assert_eq!(
        result["factors"]["divergence_ceiling"], 7.103515625e-5,
        "the `appendix-d-v2` common-factor band plus its unit-scale postcondition"
    );

    assert_eq!(result["tolerance"]["policy_id"], "appendix-d-v2");

    // The two residuals `prove_scale` evaluates unconditionally, gated here
    // on the source's own payloads rather than on a plan obligation. The rig
    // has animation tracks, so the per-element comparison ran; its only
    // skinned instance has its joint *inside* the closure, so the
    // outside-the-closure bind comparison had nothing to walk and reports an
    // absence rather than a zero.
    let residuals = &result["proof"]["residuals"];
    assert_eq!(residuals["track_value"]["evaluated"], true);
    assert_eq!(residuals["track_value"]["max"], 0.0);
    assert_eq!(residuals["unaffected_inverse_bind"]["evaluated"], false);
    assert_eq!(residuals["unaffected_inverse_bind"]["max"], Value::Null);

    assert_eq!(
        result["domain_rewrites"],
        serde_json::json!({
            "rest_hierarchy": true,
            "translation_animation": true,
            "inverse_binds": true,
            "base_mesh_positions": false
        }),
        "the reparameterization leaves base mesh POSITION alone: the vertices \
         are already authored in the correct world space"
    );
}

#[test]
fn whole_document_publishes_the_exact_binary32_narrowing_residuals_the_factor_costs() {
    // Hand-computed, not observed. The conversion narrows each product to
    // `f32` exactly once, so the residual against the `f64` expectation is
    // the narrowing error of that one rounding:
    //
    //   f32(0.01)                    = 0.00999999977648258209228515625
    //   f64(0.01)                    = 0.01000000000000000020816681711721685...
    //   |f32(0.01) - 0.01|           = 2.2351741811588166e-10
    //
    // That is the largest per-element error any rewritten element in this rig
    // takes, because `1.0` is the largest source magnitude whose product with
    // `0.01` is not exactly representable: the rig's other scale-bearing
    // values are `100`, `300` and `-100` (whose products `1`, `3`, `-1` are
    // exact) and the mesh's own `0.5`, `1.25`, `0.75` and `0.25` (whose
    // products carry strictly smaller absolute error, the largest of them
    // being 1.862645142292063e-10 at `1.25`).
    const NARROWING_AT_ONE: f64 = 2.2351741811588166e-10;
    // `mesh_position` is a per-vertex L2 norm rather than a per-component
    // error, so its maximum is at the vertex with three inexact components:
    //   (-0.5, 0.75, 0.5) -> sqrt(1.1175870905794083e-10^2
    //                           + 1.6763806315323038e-10^2
    //                           + 1.1175870905794083e-10^2)
    const MESH_POSITION_MAX: f64 = 2.3039648069873242e-10;

    let fixture = Fixture::new();
    let output = fixture.whole_document("0.01", "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    let result = &record["result"];

    assert_eq!(
        result["proof"]["artifact"]["length_factor_residual"]
            .as_f64()
            .expect("a finite residual"),
        NARROWING_AT_ONE
    );
    assert_eq!(
        result["proof"]["residuals"]["mesh_position"]["max"]
            .as_f64()
            .expect("a finite residual"),
        MESH_POSITION_MAX
    );
    // Both literals above are non-zero, so this fixture shows the residual
    // pipeline reporting a genuine measurement rather than only the zeros an
    // exactly-representable factor produces. `evaluated` is the other half of
    // that claim: a `max` of `null` here would mean nothing was checked.
    assert_eq!(
        result["proof"]["residuals"]["mesh_position"]["evaluated"],
        true
    );

    // Whole-document conversion's closure is every node and every skin.
    assert_eq!(
        result["affected"]["source_nodes"],
        serde_json::json!([0, 1, 2, 3])
    );
    assert_eq!(result["affected"]["source_skins"], serde_json::json!([0]));
    assert_eq!(result["domain_rewrites"]["base_mesh_positions"], true);
    // §D.7: this operation's factor has no measurable source counterpart, so
    // both witnesses are the declared factor and they cannot diverge.
    assert_eq!(result["factors"]["planned_observed"], 0.01);
    assert_eq!(result["factors"]["proved_observed"], 0.01);
    assert_eq!(result["factors"]["divergence"], 0.0);
}

#[test]
fn the_json_gltf_container_publishes_the_same_rewrite_and_records_its_container() {
    // The same rig payload, in the other container. `.gltf` is the case where
    // the single buffer is a base64 data URI, so the artifact re-encodes it —
    // which a GLB, whose payload is the BIN chunk, never does.
    let fixture = Fixture::new();
    std::fs::write(fixture.path("rig.gltf"), rest_bind_scale_rig_gltf()).unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "rest-bind",
            "rig.gltf",
            "-o",
            "out.gltf",
            "--source-skin-index",
            "0",
            "--source-root-node-index",
            "0",
            "--expected-factor",
            "0.01",
            "--evidence",
            "out.json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr:\n{}",
        stderr(&output)
    );
    let result = read_json(&fixture.path("out.json"))["result"].clone();
    assert_eq!(result["artifact"]["container"], "gltf");
    assert_eq!(
        result["artifact"]["reencoded_buffers"],
        serde_json::json!([0]),
        "the data-URI buffer is re-encoded; a GLB BIN chunk is not"
    );
    // The same §D.2 node members as the GLB run: the operation is a property
    // of the document, not of how it was packaged.
    assert_eq!(
        result["artifact"]["rewritten_json_pointers"],
        serde_json::json!([
            "/nodes/0/scale",
            "/nodes/1/translation",
            "/nodes/2/translation"
        ])
    );
    // The published artifact is JSON glTF, not a GLB.
    let published = std::fs::read(fixture.path("out.gltf")).unwrap();
    assert!(
        !published.starts_with(b"glTF"),
        "the container is preserved"
    );
    serde_json::from_slice::<Value>(&published).expect("the artifact is JSON glTF");
}

#[test]
fn an_unevaluated_obligation_publishes_null_rather_than_a_zero_residual() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.whole_document("0.01", "text").status.code(),
        Some(0)
    );
    let residuals = read_json(&fixture.path("out.json"))["result"]["proof"]["residuals"].clone();

    // A whole-document conversion declares no rest/bind postcondition and no
    // transform-only attachment, and the rig's only animation is `LINEAR`, so
    // it has no cubic-segment interior time. Reporting `0.0` for any of the
    // three would be a record of a claim nothing checked.
    for claim in ["unit_scale", "transform_only_affine", "cubic_interior"] {
        assert_eq!(residuals[claim]["evaluated"], false, "{claim}");
        assert_eq!(residuals[claim]["max"], Value::Null, "{claim}");
    }
    // And the counterpart: a claim that *was* evaluated reports a number,
    // including when that number is a checked zero.
    assert_eq!(residuals["skin_matrix"]["evaluated"], true);
    assert_eq!(residuals["skin_matrix"]["max"], 0.0);
}

#[test]
fn identical_inputs_and_arguments_produce_byte_identical_artifact_and_evidence() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(first.rest_bind("0.01", "text").status.code(), Some(0));
    assert_eq!(second.rest_bind("0.01", "text").status.code(), Some(0));

    // Different temporary directories, so any absolute host path or
    // canonicalized form leaking into the record would show up here — as
    // would a timestamp.
    assert_ne!(first.dir.path(), second.dir.path());
    assert_eq!(
        std::fs::read(first.path("out.glb")).unwrap(),
        std::fs::read(second.path("out.glb")).unwrap(),
    );
    assert_eq!(
        std::fs::read(first.path("out.json")).unwrap(),
        std::fs::read(second.path("out.json")).unwrap(),
    );
}

// --- Refusals ---------------------------------------------------------------

#[test]
fn a_refused_run_publishes_nothing_and_leaves_a_prior_pair_byte_identical() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("out.glb"), b"previous artifact").unwrap();
    std::fs::write(fixture.path("out.json"), b"previous evidence").unwrap();

    // The rig's root is authored at `0.01`; declaring `0.02` is a fact about
    // the source, not a typo the tool can repair.
    let output = fixture.rest_bind("0.02", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(fixture.path("out.glb")).unwrap(),
        b"previous artifact"
    );
    assert_eq!(
        std::fs::read(fixture.path("out.json")).unwrap(),
        b"previous evidence"
    );

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["outcome"], "rejected");
    assert_eq!(record["result"], Value::Null);
    assert_eq!(record["rejection"]["stage"], "plan");
    assert_eq!(record["rejection"]["kind"], "factor-mismatch");
    assert_eq!(record["rejection"]["violations"], serde_json::json!([]));
    // A refusal still identifies the input it refused and inventories it.
    assert!(record["input"]["sha256"].is_string());
    assert_eq!(record["capability"]["container"], "glb");
}

#[test]
fn a_refusal_in_text_mode_writes_prose_to_stderr_and_nothing_to_stdout() {
    let fixture = Fixture::new();
    let output = fixture.rest_bind("0.02", "text");

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output)
            .starts_with("animsmith: scale rest-bind refused rig.glb: [factor-mismatch]"),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn a_factor_the_plan_accepts_but_the_bytes_cannot_hold_is_refused_at_the_rewrite() {
    // `1e37` narrows to a usable `f32`, so planning accepts it; the rig's
    // largest length is the `300` translation key, and `300 * 1e37 = 3e39`
    // overflows binary32, whose maximum is about `3.4e38`. The refusal is
    // therefore raised where the bytes are written, not where the factor is
    // validated — which is what distinguishes the two stages.
    let fixture = Fixture::new();
    let output = fixture.whole_document("1e37", "json");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "rewrite");
    assert_eq!(record["rejection"]["kind"], "value-not-representable");
}

#[test]
fn an_unsupported_source_domain_is_refused_with_its_typed_violations() {
    let fixture = Fixture::new();
    // A camera is one of the raw domains the #280 preflight refuses outright:
    // the scale operations cannot preserve or convert it.
    std::fs::write(
        fixture.path("camera.gltf"),
        br#"{"asset":{"version":"2.0"},
             "cameras":[{"type":"orthographic",
                         "orthographic":{"xmag":1,"ymag":1,"zfar":10,"znear":1}}],
             "nodes":[{"name":"only"}],
             "scenes":[{"nodes":[0]}],"scene":0}"#,
    )
    .unwrap();
    let output = animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "whole-document",
            "camera.gltf",
            "-o",
            "out.gltf",
            "--factor",
            "0.01",
            "--evidence",
            "out.json",
            "--format",
            "json",
        ])
        .output()
        .expect("runs animsmith");

    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr:\n{}",
        stderr(&output)
    );
    assert!(!fixture.path("out.gltf").exists());
    assert!(!fixture.path("out.json").exists());

    let record: Value = serde_json::from_str(&stdout(&output)).expect("stdout is one JSON record");
    assert_schema_valid(&record);
    assert_eq!(record["rejection"]["stage"], "preflight");
    assert_eq!(record["rejection"]["kind"], "unsupported-source-domain");
    let kinds: Vec<&str> = record["rejection"]["violations"]
        .as_array()
        .expect("violations array")
        .iter()
        .map(|violation| violation["kind"].as_str().expect("a violation kind"))
        .collect();
    assert!(kinds.contains(&"camera"), "{kinds:?}");
    // The refusal carries the complete inventory gathered before rejection,
    // so a consumer can see what else the source declared.
    assert_eq!(record["capability"]["camera_count"], 1);
    assert_eq!(record["capability"]["container"], "gltf");
}

// --- Operator errors --------------------------------------------------------

/// Run `scale rest-bind` with substituted paths and report the exit code and
/// stderr.
fn rest_bind_paths(fixture: &Fixture, input: &str, output: &str, evidence: &str) -> Output {
    animsmith()
        .current_dir(fixture.dir.path())
        .args([
            "scale",
            "rest-bind",
            input,
            "-o",
            output,
            "--source-skin-index",
            "0",
            "--source-root-node-index",
            "0",
            "--expected-factor",
            "0.01",
            "--evidence",
            evidence,
        ])
        .output()
        .expect("runs animsmith")
}

#[test]
fn two_arguments_naming_one_file_are_an_operator_error() {
    let fixture = Fixture::new();
    for (input, output, evidence, expected) in [
        ("rig.glb", "rig.glb", "out.json", "input and output"),
        ("rig.glb", "out.glb", "rig.glb", "input and evidence"),
        // Lexically different, one file: the check resolves each parent
        // directory before comparing.
        ("rig.glb", "out.glb", "./out.glb", "output and evidence"),
    ] {
        let run = rest_bind_paths(&fixture, input, output, evidence);
        assert_eq!(
            run.status.code(),
            Some(2),
            "{input} {output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
        assert!(
            stderr(&run).contains(expected),
            "{input} {output} {evidence}\nstderr:\n{}",
            stderr(&run)
        );
    }
    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn a_non_gltf_extension_and_a_container_swap_are_operator_errors() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("rig.fbx"), b"not read").unwrap();
    // Extension the producer does not read at all.
    let wrong_input = rest_bind_paths(&fixture, "rig.fbx", "out.glb", "out.json");
    assert_eq!(wrong_input.status.code(), Some(2));
    assert!(
        stderr(&wrong_input).contains("self-contained glTF/GLB only"),
        "stderr:\n{}",
        stderr(&wrong_input)
    );

    // The rewrite operates on the source's own bytes and preserves its
    // container, so it cannot honour a `.gltf` destination for a GLB source.
    let swapped = rest_bind_paths(&fixture, "rig.glb", "out.gltf", "out.json");
    assert_eq!(swapped.status.code(), Some(2));
    assert!(
        stderr(&swapped).contains("must keep the source container"),
        "stderr:\n{}",
        stderr(&swapped)
    );

    // A GLB whose extension claims JSON glTF: the extension is checked
    // against the container the preflight actually found.
    std::fs::copy(fixture.path("rig.glb"), fixture.path("mislabelled.gltf")).unwrap();
    let mislabelled = rest_bind_paths(&fixture, "mislabelled.gltf", "out.gltf", "out.json");
    assert_eq!(
        mislabelled.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&mislabelled)
    );
    assert!(
        stderr(&mislabelled).contains("its extension declares"),
        "stderr:\n{}",
        stderr(&mislabelled)
    );

    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.gltf").exists());
    assert!(!fixture.path("out.json").exists());
}

#[test]
fn an_unusable_input_or_destination_is_an_operator_error() {
    let fixture = Fixture::new();
    let missing_input = rest_bind_paths(&fixture, "absent.glb", "out.glb", "out.json");
    assert_eq!(missing_input.status.code(), Some(2));
    assert!(
        stderr(&missing_input).contains("cannot read"),
        "stderr:\n{}",
        stderr(&missing_input)
    );

    let missing_dir = rest_bind_paths(&fixture, "rig.glb", "absent/out.glb", "out.json");
    assert_eq!(missing_dir.status.code(), Some(2));
    assert!(
        stderr(&missing_dir).contains("output directory"),
        "stderr:\n{}",
        stderr(&missing_dir)
    );

    // A destination that exists but is not a regular file. This is the case
    // the destination check owns by itself: a missing *directory* is also
    // caught by resolving the path identity for the distinctness check, so
    // that case alone cannot show this guard is wired.
    std::fs::create_dir(fixture.path("dir.glb")).unwrap();
    let directory_artifact = rest_bind_paths(&fixture, "rig.glb", "dir.glb", "out.json");
    assert_eq!(
        directory_artifact.status.code(),
        Some(2),
        "stderr:\n{}",
        stderr(&directory_artifact)
    );
    assert!(
        stderr(&directory_artifact).contains("dir.glb is not a regular file"),
        "stderr:\n{}",
        stderr(&directory_artifact)
    );

    std::fs::create_dir(fixture.path("dir.json")).unwrap();
    let directory_evidence = rest_bind_paths(&fixture, "rig.glb", "out.glb", "dir.json");
    assert_eq!(directory_evidence.status.code(), Some(2));
    assert!(
        stderr(&directory_evidence).contains("dir.json is not a regular file"),
        "stderr:\n{}",
        stderr(&directory_evidence)
    );

    assert!(!fixture.path("out.glb").exists());
    assert!(!fixture.path("out.json").exists());
}
