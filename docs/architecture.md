# Current architecture

AnimSmith inspects skeletal animation and related source evidence, evaluates
explicit project contracts, and performs bounded mechanical preparation.
This page describes implementation ownership. Exact APIs live in the
[Rust references](embedding.md#choose-the-crates), user behavior in the
[CLI guide](cli.md), and immutable wire identities in the
[output reference](output.md). Active sequencing belongs to
[GitHub milestones](https://github.com/mmannerm/animsmith/milestones) and
[the roadmap ledger](https://github.com/mmannerm/animsmith/issues/165).

[DESIGN.md](../DESIGN.md) retains the historical rationale and accepted
operation/protocol decisions. Its numerical, preservation, authority and
failure requirements remain authoritative; this overview does not relax or
replace them. Dated [research](research/game-ready-animation-clips.md) is input
to decisions, not evidence that a feature ships.

## Workspace and dependency direction

The [workspace manifest](../Cargo.toml) lists six publishable crates and one
unpublished test-support crate. Production dependencies point toward core;
core does not depend on a format, an engine, the report renderer, or the CLI.

| Crate | Owns | Production workspace dependencies |
|---|---|---|
| [`animsmith-core`](../crates/animsmith-core/README.md) | Format-neutral model, semantic roles, sampling, measurements, checks, transforms, and strict versioned evidence contracts | None |
| [`animsmith-gltf`](../crates/animsmith-gltf/README.md) | glTF/GLB decoding, captured source facts and dependencies, byte repair, capability inventory, and format-specific emission/proof | core |
| [`animsmith-fbx`](../crates/animsmith-fbx/README.md) | ufbx integration, coordinate/unit interpretation, take baking, and captured source inventory | core |
| [`animsmith-engine`](../crates/animsmith-engine/README.md) | Versioned importer facts, settings resolution, predictions, advice, and engine evidence | core |
| [`animsmith-report`](../crates/animsmith-report/README.md) | Offline HTML presentation of validated evidence, playback and illustrative comparison | core |
| [`animsmith`](../README.md) | CLI parsing, filesystem access, config construction, command orchestration, publication and process outcomes | core, gltf, engine; optional fbx and report |
| `animsmith-testkit` | Shared synthetic fixtures, parsed documentation helpers and CLI process tests; `publish = false` | core with its fixture feature |

The CLI's default `fbx` and `report` features enable the two optional crates.
The minimal build excludes them; glTF ingestion and engine evidence remain.
The FBX crate isolates ufbx's C build. Core's production dependencies are
`glam`, `serde`, `serde_json`, `serde_jcs`, `sha2` and `thiserror`; JSON is for
its own bounded contracts, not file-format parsing or configuration I/O.

[`tools/bevy-readback`](../tools/bevy-readback/README.md) is excluded from this
workspace. It owns an exact engine/toolchain/lock lifecycle and an opt-in real
engine validation lane. A profile prediction or a valid probe manifest does
not prove an engine loaded an asset. The separate [fuzz workspace](../fuzz/README.md)
exercises untrusted loader/repair entry points without adding nightly tooling
to normal library consumers.

## Inspection and evidence flow

1. A frontend obtains input bytes and delegates format interpretation to the
   corresponding loader. Format crates own rooted resource access. Core owns
   the format-neutral vocabulary and validation of source/dependency evidence.
2. A `LoadedSource` keeps the normalized `Document` with immutable facts tied
   to the captured primary source and dependency closure. Display names and
   mutable animation values cannot reconstruct exact authored identities.
   Consuming only the document deliberately discards that authority.
3. The frontend validates its typed `Config` and resolves semantic rig roles.
   A rig profile maps roles to bones; project intent and engine importer
   settings have separate owners. Embedders construct config directly;
   the CLI reads TOML.
4. One `MetricGrids` owner supplies sampled poses for measurements and checks.
   `CheckCtx` carries the shared document, role and config context, while
   `evaluate_checks` validates check results. The CLI's `LintAnalysis` shares
   the source, roles, grids and current result between lint and report paths.
5. Core-owned versioned envelopes hold findings, measurements and coverage.
   Engine prediction consumes source facts plus resolved importer settings;
   actual readback is separate observed evidence bound to its inputs.
6. Text, JSON and HTML present the resulting evidence. The report renderer
   validates its input contract, retains original engine evidence and derives
   a bounded display projection. Rendering does not grant a source permission
   to load, rewrite, publish, or claim engine/runtime correctness.

The runnable [embedding example](../crates/animsmith/examples/embed.rs) and
[embedding guide](embedding.md) show this flow without CLI-owned filesystem
policy. A caller should reuse the existing sampling and evidence owners
rather than independently recomputing measurements for each presentation.

## Source authority and operation boundaries

A normalized model is useful for analysis, but it is not a lossless container
for every source feature. Raw declarations, unsupported domains, source
identities and operation-specific capability inventories remain distinct.
Loaders preserve authored animation data; FBX's coordinate interpretation and
take baking are documented format-semantic exceptions, not cleanup.

Mechanical operations have different proof obligations:

- Surgical glTF repair owns the exact bytes of the selected repair. It must
  preserve unrelated payloads and be idempotent; a normalized re-export is
  not an implementation of that guarantee.
- Core transforms operate on the format-neutral model under their stated
  interpolation, timing and losslessness contracts. Format writers and the
  CLI then own representability, emitted-artifact verification and publication.
- Scale operations retain separate whole-document unit conversion and rest/bind
  reparameterization policies. Core plans/proves; format frontends rewrite
  exact source representations and reload candidates; CLI publication owns
  filesystem durability. See the accepted [scale decision](../DESIGN.md#appendix-d--decision-record-skinned-restbind-scale-canonicalization).
- Assembly combines an authoritative base and explicit selected takes. The
  [assembly guide](character-assembly.md) owns its supported operations and
  artifact/evidence pair. It must not infer general rewrite permission from a
  successful inspection or engine prediction.

Publication safety belongs at the actual filesystem boundary. Different
commands have explicitly documented publication guarantees; a staged producer
cannot lend its atomicity claim to an unrelated direct writer. Typed producer
outcomes distinguish source refusal from operator/publication failure before
human-readable prose is rendered. The [CLI guide](cli.md) and retained
[command decisions](../DESIGN.md#3-cli-surface) define those outcomes.

## Collections and engine integration

A collection manifest declares logical relationships across exact source files.
It does not turn ordinary multi-file linting into collection policy, and equal
display names do not make clips from different files interchangeable. Core
owns validated collection declarations, contact/transition contracts and pure
foot-cycle planning. Its JSON readers cover the contracts it defines. The CLI
decodes TOML declarations and owns the collection-output and foot-cycle
producer evidence envelopes and their strict readers, as well as rooted access,
retained source sessions and command execution.
The accepted [collection decision](../DESIGN.md#appendix-f--decision-record-file-scoped-clip-identity-and-collections)
and [collection guide](collection-contracts.md) define identity, completeness,
coverage and refusal semantics.

Engine profiles belong to `animsmith-engine`, not consumer-neutral checks.
A profile may state what an importer does; it may not silently change a core
check's tolerance or authorize an asset rewrite. Import advice, prediction,
readback and game acceptance are different evidence. See the retained
[engine decision](../DESIGN.md#appendix-e--decision-record-engine-profiles)
and the [engine intake workflow](game-developer-intake-workflow.md).

## Extending and reviewing the system

Use the existing owner for a new requirement: a new content check belongs with
core checks; format interpretation belongs with the loader; verified importer
facts belong with engine profiles; presentation belongs with reports or CLI
renderers. A new public boundary is justified by a distinct authority,
contract, state owner or lifecycle, not by a desire to wrap one function.

The stable [audit invariants](../.agents/audit-invariants.md) are the review
source for untrusted input, exact repair preservation, transform losslessness,
core independence, role resolution, reference numbers, public contracts,
licensed assets and loader fidelity. Retained decisions above supply the more
specific operation and protocol requirements. Changes to those obligations
require an explicit policy decision; documentation reorganization is not one.

[Development](../DEVELOPMENT.md) owns commands and test environments, and
[contributing](../CONTRIBUTING.md) owns delivery. Verification should exercise
observable behavior: analytic synthetic metrics, exact emitted bytes,
independent mutation/refusal expectations and real CLI boundaries. A source
spelling check cannot establish those properties. Licensed golden checks,
actual engine runs and browser execution prove different claims; record which
were run and which evidence is unavailable.
