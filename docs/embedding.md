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
animsmith-core = "0.2"
animsmith-gltf = "0.2"
# Optional:
animsmith-fbx = "0.2"
animsmith-report = "0.2"
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
   defects load and become findings. The same document carries meshes, skins,
   factor-only materials, and available base-color and normal textures for
   scene round-trips.
   An FBX host that needs auditable scale-boundary facts uses
   `animsmith_fbx::load_scale_source` instead: the returned wrapper retains the
   document and `FbxScaleCapabilityInventory` from one parse. The inventory
   and normalized source-skeleton sidecar do not enable FBX scaling. `Complete`
   means the documented ufbx projection covers every representable node/skin
   identity and joint slot; a missing cluster bone downgrades the sidecar to
   `Unavailable` instead of dropping that slot. Unreadable ordered bind
   declarations retain no finite prefix. The local rests and bind matrices are
   adjusted/compensated or derived target-
   coordinate values, not exact authored FBX members. The inventory explicitly
   records baked curves, rebuilt payloads, and unavailable raw span proof.
2. **Resolve rig roles.** Use `resolve_configured_roles` to apply the same
   named/auto profile plus inline-override policy as the CLI. Lower-level
   `detect_profile`, `profile::resolve_named`, and
   `ResolvedRoles::from_names` remain available when a host intentionally
   owns a different policy. Checks consume roles, never project-specific bone
   names.
3. **Build `Config`.** The CLI's TOML is only one constructor. Deserialize
   the types from your schema or build them programmatically. Deserialization
   validates per-clip loop caps immediately; `evaluate_checks` also validates
   directly constructed `Config` values before inspecting or running the check
   catalog, so an invalid negative or non-finite cap fails closed as a typed
   `EvaluationError::InvalidConfiguration`. Call `Config::validate` directly
   only when the host wants that same error before it reaches evaluation.
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
clip measurements, add static asset measurements, run findings, render HTML, or
combine those results with host-owned checks. Share the same `MetricGrids`
within the limits documented by its rustdoc so those consumers judge one
sampled representation.

The `MetricGrids`, `measure_document`, and `measure_assets` rustdocs own cache
thread-safety, sampling, static-domain scope, and identity details.
`Primitive::additional_influence_sets` and the corresponding mesh measurement
records expose secondary glTF skin-attribute presence only. Mesh records also
preserve whether either side was unpaired on an individual primitive; they do
not retain or evaluate secondary per-vertex payloads.

For glTF/GLB, asset measurements additionally provide source-order material,
texture, and image inventory records plus explicit
`material_resource_coverage`. Complete glTF/GLB coverage is scoped to the five
documented core bindings, in order: `base_color`, `normal`,
`metallic_roughness`, `occlusion`, and `emissive`. It does not cover
extension-defined texture slots. These are loader observations, not a portable
image-management API: a host must handle `"unavailable"` coverage from other
loaders and must not infer writer preservation, image acceptance, repair,
resizing, transcode, color-space policy, or engine-import behavior from them.
Use the typed image metadata to inspect declared MIME separately from the
byte-detected container and decoded color/channel data; unavailable images
carry a reason instead of decoded metadata.

When the host needs to exchange the same JSON as the CLI, construct
`MeasurementContract`, `MeasureFileReport`/`LintFileReport`, and
`MeasureEnvelope`/`LintEnvelope` from `animsmith-core::contract`. That module
owns both immutable URNs and derives the lint/measure summary from the supplied
records.
It also exposes the typed `MeasurementReportInput` subset and
`MeasurementReportFile` records for consumers that need to validate and recover
every file's exact display path and full measurement contract from a current
measure or lint report. The core boundary preserves file order and accepts
empty or multi-file reports; each consumer owns any cardinality rule for its
workflow. `MeasurementReportInput::file_count` lets an adapter retain the raw
record count before `into_files` consumes and validates the report, while
`MeasurementReportError::File` carries a typed `MeasurementFileError` and an
index (also available through `file_index`) without adding consumer-specific
prose.
The compiling example emits a full schema-valid lint envelope; embedded
producers do not need to copy private CLI structs or hard-code protocol
identities. Host-specific sidecars remain appropriate when CLI interoperability
is not needed.

## Gate and stability contracts

The CLI convention is a useful default for an embedded gate:

- no error findings: success (warnings may remain visible);
- any `Severity::Error`: content rejection;
- loader/config/I/O error: operator failure, kept separate from findings.

Missing prerequisites are typed coverage gaps, and disabled/unselected checks
remain visible without executing. Severity overrides apply only to content
findings. Coverage is nonblocking by default; the embedding host owns any
required-check or release-lane policy.

Built-in scope and gap codes may be emitted only by the checks declared in the
core evidence-code authority; `CheckEvaluation::evaluated` returns a typed
error when a different check claims one. Embedded checks retain an open
vocabulary by using namespaced custom codes such as `acme:input_unavailable`.
The public built-in-code slices are available to consumers that need to
enumerate or allow-list animsmith-owned codes.

For the current pre-1.0 API, prefer the crate-root flow: loader → role
resolution → `Config` → `MetricGrids` → measurements/checks → findings. The
durable automation contracts are deliberately narrower than the pre-1.0 Rust
API:

- built-in check ids used by config and findings;
- CLI exit codes and the versioned
  [JSON envelope](output.md), when the host interoperates with the CLI.

The `animsmith-core` crate root owns the full
[API status](https://docs.rs/animsmith-core) contract. Rust symbols, model and
transform types, metric formulas, and diff thresholds may still be refined
before 1.0. Match `#[non_exhaustive]` result types with a fallback arm. The
`Check` trait supports experiments with custom checks, including opt-in checks
that override `enabled_by_default`; an explicit "note", "warn", or "error"
setting activates one. There is no stable plugin registry yet; wrapping
animsmith findings with host-owned checks keeps that boundary explicit.

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

## Scale plan and proof contracts

The [scale workflow](scale.md) is the operator-facing guide. Normative algebra,
tolerances, refusal boundaries, and ownership live in
[DESIGN.md Appendix D](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization);
the [output reference](output.md) owns the evidence wire contract.

For an embedded producer:

1. Preflight the exact source format with
   `animsmith_gltf::preflight_scale_source`, then project it with
   `animsmith_gltf::capability_facts` into `ScaleCapabilityFacts`.
2. Call `plan_scale` once with an explicit `ScaleOperation`. The opaque plan
   owns the operation parameters and a numeric-free structural ledger.
3. Apply that same plan to the exact source representation. For glTF/GLB,
   `rewrite_scale_plan` cross-checks raw hierarchy and container identities
   against the plan before changing JSON or accessor bytes.
4. Reload the emitted artifact, wrap the document with
   `ScaleCandidate::from_document`, and call `prove_scale` with the original
   source and same plan.
5. Run the format artifact proof as well. Core proof covers normalized semantic
   claims; artifact proof covers exact raw preservation, container structure,
   declared bounds, aliasing, deterministic bytes, and the actual write set.
6. Publish only the proved artifact and matching evidence as one coordinated
   pair, preserving the CLI's rollback and documented process-crash semantics.

Those producer steps currently apply to glTF/GLB only. FBX embedders may call
`animsmith_fbx::capability_facts` to inspect the conservative projection, but
it remains unsupported for both operations. The FBX inventory makes the later
writer/proof work explicit; it is not authority to substitute a normalized
writer or claim raw FBX spans, object properties, curve keys, or vertex
identity were preserved.

`ScaleCandidate` grants no authority: proof independently validates the
source, plan inventory, candidate structure, and numeric claims. A frontend
must not substitute a normalized writer for exact-source rewrite when the
operation promises preservation of unknown or format-only payload.

`ScalePlan::ledger()` exposes read-only field, payload-shape, source-topology,
and proof-claim views for adapters. These rows name structural ownership and
identity, not resolved multipliers or expected numeric values. Writer and proof
may share this numeric-free binding vocabulary, but each must derive its own
numeric expectations. Under unavailable source coverage, best-effort raw locals
remain compatibility output rather than replay identity.

Each `ScaleProof` claim is a read-only `ScaleProofResidual` carrying its
maximum and comparison count together. `evaluated()` distinguishes a measured
zero from an obligation that walked nothing. The fixed
`ScaleTolerancePolicy::APPENDIX_D_V6` identity also owns the sampled-work
budget; proof refuses over-budget input rather than proving a subset.

The glTF convenience functions `rewrite_linear_units` and
`rewrite_rest_bind` compile and delegate to the same plan-taking adapter.
Use the plan-taking boundary when the host already owns the plan. The format
frontend retains raw capability, hierarchy, component, type/count/range,
alias/overlap, and complement checks that core cannot express.

See the public rustdoc for exact selectors, row variants, errors, and proof
fields. Calibration populations and historical timing notes are intentionally
separate in [scale-calibration.md](scale-calibration.md).

## What the libraries do not own

- Parsing the host's contract files into `Config`.
- Hashing assets, tracking provenance, or deciding staleness.
- Choosing raw/generated artifact paths or retention policy.
- Artistic retargeting, contact cleanup, or motion editing.

Those decisions surround animsmith in the
[raw-to-game-ready pipeline](pipeline-scenarios.md); they are not API
responsibilities.
