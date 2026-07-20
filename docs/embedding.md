# Embedding animsmith in a pipeline

The CLI is one frontend. Asset pipelines that already own their contract
format, importer, build graph, and gate can call the same loaders,
measurements, and checks in process.

This guide covers integration decisions. Symbol-level contracts belong in
rustdoc, the [pipeline scenario guide](pipeline-scenarios.md) owns the larger
raw-to-game-ready process, and the [examples cookbook](../examples/README.md)
owns runnable CLI transcripts.

## Choose the crates

| Crate | Use it for |
|---|---|
| `animsmith-core` | Required embedding boundary: data model, rig roles, config, sampling, measurements, diffs, checks, and findings. No file I/O or format dependency. |
| `animsmith-gltf` | Load glTF/GLB, write glTF/GLB, or apply byte-surgical glTF repairs. |
| `animsmith-fbx` | Load FBX through `ufbx`; adds a bundled C build. Omit it from glTF-only pipelines. |
| `animsmith-report` | Render self-contained HTML from the same sampled grids and findings. |

The `animsmith` crate is the CLI binary, not a library facade.

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
# Optional:
animsmith-fbx = "0.1"
animsmith-report = "0.1"
```

docs.rs is the canonical reference for published APIs. The stable package
URLs are:
[animsmith-core](https://docs.rs/animsmith-core),
[animsmith-gltf](https://docs.rs/animsmith-gltf),
[animsmith-fbx](https://docs.rs/animsmith-fbx), and
[animsmith-report](https://docs.rs/animsmith-report). For the current workspace
state, build the same rustdocs locally with `just doc`.

Rustdoc owns signatures, type invariants, lifetimes, and the `Errors` and
`Panics` contracts. In particular, start at the `animsmith-core` crate root;
it is the compact API map rather than another copy of this guide.

The compiling end-to-end example is
[`crates/animsmith/examples/embed.rs`](../crates/animsmith/examples/embed.rs):

```console
cargo run -p animsmith --example embed
```

## Integration flow

1. **Load a `Document`.** Use `animsmith_gltf::load` or
   `animsmith_fbx::load`. glTF animation values remain authored; FBX scenes
   are normalized to metres, right-handed +Y-up coordinates and baked into
   linear TRS tracks. Structural failures are loader errors. Semantic
   defects load and become findings.
2. **Resolve rig roles.** Use `resolve_configured_roles` to apply the same
   named/auto profile plus inline-override policy as the CLI. Lower-level
   `detect_profile`, `profile::resolve_named`, and
   `ResolvedRoles::from_names` remain available when a host intentionally
   owns a different policy. Checks consume roles, never project-specific bone
   names.
3. **Build `Config`.** The CLI's TOML is only one constructor. Deserialize
   the types from your schema or build them programmatically.
4. **Create one `MetricGrids`.** Share it by reference with
   `measure_document`, `CheckCtx::new`, `evaluate_checks`, and optional report
   rendering so each clip is sampled once.
5. **Map results into the host.** `Finding` carries a stable check id,
   severity, optional clip/bone/time, measured and expected values, and a
   message. The host decides whether warnings fail its gate.

Call `evaluate_checks` with the full catalog and a `CheckSelection`. It
returns one `CheckEvaluation` per
catalog check, including disabled, unselected, not-applicable, partial, and
not-evaluated work. `CoverageGap::code` and `EvaluationScope::code` are the
machine fields; never reconstruct coverage by parsing a message. Content
findings are nested under their owning check and coverage gaps are never
encoded as findings.

Role resolution remains an explicit frontend step. Use
`resolve_configured_roles`, `CheckCtx`, and `Config::rig` rustdocs for the
exact profile, override, and unresolved-role contracts.

## Compose the outputs you need

An embedded gate does not need to reproduce every CLI output. It can emit
clip measurements, add mesh measurements, run findings, render HTML, or
combine those results with host-owned checks. Share the same `MetricGrids`
within the limits documented by its rustdoc so those consumers judge one
sampled representation.

The `MetricGrids`, `measure_document`, and `measure_meshes` rustdocs own cache
thread-safety, sampling, measurement scope, and map-key details.

When the host needs to exchange the same JSON as the CLI, construct
`MeasurementContract`, `FileReport`, and `ReportEnvelope` from
`animsmith-core::contract`. That module owns both immutable URNs and derives
the lint/measure summary from the supplied records. The compiling example
emits a full schema-valid lint envelope; embedded producers do not need to
copy private CLI structs or hard-code protocol identities. Host-specific
sidecars remain appropriate when CLI interoperability is not needed.

## Gate and stability contracts

The CLI convention is a useful default for an embedded gate:

- no error findings: success (warnings may remain visible);
- any `Severity::Error`: content rejection;
- loader/config/I/O error: operator failure, kept separate from findings.

Missing prerequisites are typed coverage gaps, and disabled/unselected checks
remain visible without executing. Severity overrides apply only to content
findings. Coverage is nonblocking by default; the embedding host owns any
required-check or release-lane policy.

For v0.1, prefer the crate-root flow: loader → role resolution → `Config` →
`MetricGrids` → measurements/checks → findings. The durable automation
contracts are deliberately narrower than the pre-1.0 Rust API:

- built-in check ids used by config and findings;
- CLI exit codes and the versioned
  [JSON envelope](output.md), when the host interoperates with the CLI.

The `animsmith-core` crate root owns the full
[API status](https://docs.rs/animsmith-core) contract. Rust symbols, model and
transform types, metric formulas, and diff thresholds may still be refined
before 1.0. Match `#[non_exhaustive]` result types with a fallback arm. The
`Check` trait supports experiments with custom checks, but there is no stable
plugin registry yet; wrapping animsmith findings with host-owned checks keeps
that boundary explicit.

## Migrating an existing pipeline

For a command-by-command migration plan, use the
[pipeline scenarios](pipeline-scenarios.md) for marketplace intake, mocap
cleanup, outsourced acceptance, CI gating, and raw/generated artifact
storage, then use the [cookbook](../examples/README.md) for exact commands.

For the library cutover itself:

1. Capture accepted measurements from the old pipeline as golden values.
2. Run the CLI and embedded path from the same `Config` until their findings
   agree.
3. Compare old and new measurement maps with `diff_measurements`; judge
   motion deltas rather than binary file differences.
4. Keep project-specific sidecars, hashes, provenance, and storage policy in
   the host pipeline.

## What the libraries do not own

- Parsing the host's contract files into `Config`.
- Hashing assets, tracking provenance, or deciding staleness.
- Choosing raw/generated artifact paths or retention policy.
- Artistic retargeting, contact cleanup, or motion editing.

Those decisions surround animsmith in the
[raw-to-game-ready pipeline](pipeline-scenarios.md); they are not API
responsibilities.
