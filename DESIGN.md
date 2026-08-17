# animsmith — design & requirements

Status: pre-1.0 publishing design. Intended to keep the public crate and
CLI surface aligned while the project is still willing to make breaking
changes.
Origin: extracted from a private game project's animation pipeline
(design session 2026-07-03); that project — "the incubating project"
below — is the first consumer, not the scope.

---

## 1. Mission & positioning

**animsmith is a linter for skeletal animation clips.** It answers the
question every game team answers by hand today: *does this clip have
game-engine-friendly characteristics?* — does the loop actually close, does
the walk cycle's declared speed match its root motion, do the feet slide
during stance, is the rig conformant, are the quaternions sane.

**The gap it fills.** Nothing open-source does game-semantics clip
validation:

- **Khronos glTF-Validator** checks *spec conformance* (accessor validity,
  NaN, quaternion norms at the container level) — it has no concept of a
  loop, a gait, or root motion.
- **ozz-animation** has a motion-extraction sample (a good root-motion
  measurement reference) but no lint pipeline.
- Academic metric code (foot-skate ratio, jitter, penetration) lives in
  ML-evaluation repos, not artist tools.
- Engine importers (Unreal Interchange + Data Validation, Unity
  AssetPostprocessor, Godot import sidecars) give teams a *place* to hang
  custom checks, but the checks themselves are always studio-custom and
  re-derived from scratch.

animsmith packages those checks as a standalone Rust library + CLI: glTF/GLB
native, FBX ingested via ufbx, engine-agnostic core, machine-readable
output, and a self-contained HTML report with a 3D preview.

**What it is not (scope guardrails):**

- **Not a spec validator.** Run glTF-Validator for container conformance;
  animsmith assumes a parseable file and judges its *content*.
- **Not an art exporter.** `convert` (FBX→glTF) exists so clips can enter
  the lint pipeline directly from a DCC export; it promises animation and
  skinning fidelity, not material/shading fidelity.
- **A transformer for pipeline-mechanical operations only** (scope
  widened 2026-07-03; see Appendix A). In scope: `fix` for lossless
  mechanical repairs (quaternion unit normalization and hemisphere
  normalization), frame-range
  slice/trim + hold-extend, fail-closed in-place gait-anchor rotation, opt-in
  pruning of provably constant multi-key tracks, and format conversion
  including a full mesh/skin FBX→glTF path (a maintained replacement
  for the archived FBX2glTF). Out of scope stays *artistic*
  transformation: retargeting, motion editing, procedural animation —
  that is DCC work. The rule of thumb: animsmith may rewrite a clip
  only in ways whose correctness its own checks can verify.
- **Gait anchoring is an explicitly in-place operation.** The core boundary
  requires a movement policy; the shipped CLI and assembly switches select
  only `InPlace`. Before any channel is cyclically reordered, AnimSmith samples
  the resolved Root role (falling back to Hips) and refuses missing/non-finite
  evidence, horizontal accumulation above 1 cm, or yaw accumulation above 1°.
  Every nonconstant channel the operation would rotate must contain exactly one
  key at each declared whole-frame sample over `[0, duration]`, at the exact
  representable f32 `key / fps` time and exact period endpoint. Sparse,
  differently framed, duplicate-time, or off-grid evidence refuses; duplicate
  `(bone, property)` channels, including constant channels, also refuse before
  sampling. A phase shift must bijectively permute authored values rather than
  synthesize values at omitted frames. The
  verifier samples those exact admitted f32 key times, and mutation is an
  integer-index permutation rather than a second floating-point resample.
  Constant channels need no grid because cyclic reordering cannot change them;
  their key times cannot influence the declared period or shift.
  Before allocating a pose grid, the public core boundary validates all track
  cardinalities, targets, finite values, all resolved metric-role indices, and
  the complete acyclic parents-before-children skeleton. Both declared-frame
  and maximum-authored-key work must independently satisfy the inclusive bounds
  `declared frames × skeleton bones <= 1,000,000`, `declared frames × tracks <=
  1,000,000`, and `maximum authored keys × skeleton bones <= 1,000,000`; every
  verifier and metric sample uses the one bounded declared grid. The 1 cm endpoint-displacement and
  1° accumulated-yaw caps are applied directly, without any sampled-step
  allowance an interior outlier could inflate. Yaw is derived from model-space
  f32 quaternions as binary64 headings, with full-turn crossings counted and
  the first heading subtracted from the final unwrapped heading. The result has
  no segment-count-dependent summation error. Only four f32 successors at each
  inclusive cap cover authored endpoint translation/quaternion quantization;
  this gait-local rule does not widen other checks. Standalone transform output
  is transactional too: per-clip success lines and the write summary remain
  buffered until every selected clip and the artifact write succeed. This
  admits tightly closed cyclic pelvis/root sway while refusing authored travel
  and turns. Root-motion phase offsets,
  root-motion extraction, and trajectory-preserving cyclic rebasing remain
  runtime or separately designed operations; gait anchoring must not improvise
  them.
- **Not a runtime.** It models how engines sample animation; it does not
  play games.

## 2. Users & use cases

1. **Artist inner loop** — `animsmith lint export.fbx` seconds after a DCC
   export, before any engine import or bake. Catches "the loop pops",
   "wrong rig", "cm instead of m" while the DCC session is still open.
   This is the highest-value loop: the alternative is discovering the
   problem after the slowest step of the pipeline.
2. **CI gate** — `animsmith lint --format json` in CI on committed assets;
   stable JSON schema, exit codes, per-check severity config, baseline
   file for adopting teams with a dirty back catalog.
3. **Pipeline library** — engine pipelines embed `animsmith-core` and build
   check sets programmatically. First consumer: the incubating
   project's asset gate replaces ~1000 LOC of measurement Python with
   library calls.
4. **PR-review artifact** — `animsmith report clip.glb -o report.html`
   produces a single offline HTML file with 3D skeleton playback and
   metric charts; attach it to a PR or CI artifacts so a reviewer can *see*
   the seam pop the numbers describe.

## 3. CLI surface

```
animsmith lint    <file...> [--config animsmith.toml] [--select ids] [--deny warn] [--format text|json]
animsmith measure <file...> --format json          # measurements only, no judgment
animsmith inspect <file>                           # clips, durations, tracks, bones, detected rig profile
animsmith report  <file> -o report.html [--clip name]
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor] [--drop-duplicate-loop-endpoint] [--prune-constant-tracks]
animsmith fix     <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb> [--material-texture-recipe recipe.toml] [--animation-only|--bake-static-mesh-transforms] [--format text|json]
animsmith assemble <recipe.toml> -o <out.glb> --evidence <out.json>
animsmith diff    <A> <B> [--format text|json]     # A/B: assets or one-file output-v7 measure/lint JSON
```

- `lint` = measure + judge against config. `measure` is lint minus
  judgment — both emit the independently versioned measurement contract
  that other pipelines can pin.
- **Exit codes**: `0` no failing findings (warnings, notes, and nonblocking
  coverage gaps may remain), `1` at least one error-severity finding (or
  pending repairs under `fix --dry-run`) and any producer refusal established
  as a fact about source asset bytes, `2` operator/tool error (unreadable
  declared file, bad config or recipe syntax, unsafe/unwritable path, or
  publication failure).
  `--deny-warnings` promotes warnings to errors.
- **Producer outcome authority is typed before prose exists.** `convert` and
  `assemble` carry `Published`, `Rejected`, and `Operator` through one shared
  boundary. Loader, selector, transform, proof, and representability failures
  reached after source bytes were read are asset refusals; invocation,
  recipe-validation, path, file-read, temporary-file, serialization, and
  publication failures are operator errors. Mixed error enums (notably the
  material-texture recipe) are matched by variant at the call site. No exit
  decision parses an error string. JSON refusals use the separately immutable
  `urn:animsmith:schema:producer-refusal:1` record; existing conversion,
  assembly, output, and scale evidence identities remain unchanged. Text
  refusals are one escaped stderr diagnostic carrying the same stable kind.
  Every refusal precedes publication, preserving a prior assembly pair and a
  prior convert artifact; this does not claim that convert's direct writer can
  roll back an operator I/O failure.
- **Stdout is reporting, not outcome authority.** Every JSON, text, and
  Markdown stdout write, including parser-rendered help and version text, uses
  a fallible checked write followed by a best-effort checked stderr diagnosis.
  A closed pipe or other write failure never panics or replaces the outcome
  already established by a stdout-bearing path: shipped success paths retain
  `0`, while finding and refusal paths retain `1`. Operator errors occur
  before stdout reporting, remain stderr-only, and retain `2`.
  This deliberately includes output-centric commands such as `inspect`: exit
  `0` states that inspection completed, not that an external consumer accepted
  every byte. Assigning a new outcome to text alone would make exit semantics
  depend on presentation format and would relabel real findings or completed
  publication as operator errors. JSON serialization failure remains an
  operator error because it means no truthful record could be formed; failure
  to deliver already-rendered bytes is only a reporting failure.
  Multi-part human output is attempted as one checked stream where one command
  can produce multiple records; in particular, all selected `fix` repair
  reports and all parts of one conversion summary yield at most one
  stdout-failure diagnostic.
  Parser output remains clap-owned: the checked boundary calls clap's
  fallible writer directly so its configured automatic/forced ANSI styling is
  preserved instead of being stripped while materializing a plain string.
- Human-readable command results are assembled by pure functions in the CLI's
  renderer module. Command dispatch keeps execution, file writes, and exit
  policy in `main.rs` and passes structured values to the renderer; the
  renderer is the single owner of escaping untrusted text for terminal-safe
  presentation. JSON result serialization remains a separate, unchanged path.
- `transform --prune-constant-tracks` is an opt-in, candidate-first mechanical
  edit: it shares the `constant-track` tolerances (vector components `1e-4`,
  sign-invariant rotations `1e-3` radians), verifies the resulting sampled
  local TRS and model-space position/rotation, and reports every original track
  index it removed or retained.
  It runs after selected transforms and never substitutes for DCC curve cleanup
  or key reduction. Per-clip `animates_bones` names protect motion evidence;
  `[rig] required_bones` remains a skeleton-presence-only contract.
- Inputs: `.glb`, `.gltf` (+ external buffers), `.fbx` (via the `fbx`
  feature, default-on in the released binary).
- **Malformation policy**: *structural* malformation — keyframe/value
  count mismatch, zero-key channels, absolute or escaping external
  buffer URIs, non-forest node graphs (cycles or a node with two
  parents), or a primitive accessor whose dense or sparse byte extent its
  declared buffer view or resolved buffer bytes cannot satisfy, an integer
  `TEXCOORD_0` or `WEIGHTS_0` accessor that omits `normalized: true`, or an
  animation sampler accessor whose declared element does not match its
  property-selected reader — is rejected
  at load (operator error, exit 2; run glTF-Validator for the details).
  Recovering a non-forest graph would force an arbitrary parent choice or
  silently drop a cyclic subtree, so the loader rejects rather than repairs
  (decision recorded for #92). Likewise, substituting an empty position/index
  vector for a short accessor would make checks report on geometry the file did
  not author. Primitive shortfalls therefore use a located
  `PrimitiveAccessorLayout` refusal; inverse-bind shortfalls remain typed
  source evidence because that domain already models unavailable bind
  declarations (decision recorded for #329).
  Sampler input is `SCALAR`/`FLOAT`; translation and scale output are
  `VEC3`/`FLOAT`; rotation output retains all five glTF quaternion encodings
  (`BYTE`, `UNSIGNED_BYTE`, `SHORT`, `UNSIGNED_SHORT`, `FLOAT`) as `VEC4`,
  and morph-weight output retains those same component encodings as `SCALAR`.
  Declared lookalikes are never reinterpreted merely because their byte size
  happens to match the reader's element (decision recorded for #327).
  The same fail-closed rule keeps invariant-9 honest for integer UVs and
  weights: normalized `UNSIGNED_BYTE`/`UNSIGNED_SHORT` values are decoded to
  their declared float range, and `FLOAT` values pass through, but an
  unnormalized integer accessor is refused before decoding. The `gltf` reader
  rescales those integers whether or not the flag is set, and the loader has
  no finding channel in which to disclose that unauthorized reinterpretation;
  allowing it would make `measure` report derived values as authored data
  (decision recorded for #328).
  *Semantic* defects — NaN times or values, non-unit quaternions,
  hemisphere flips, seam pops — load fine and are judged by the checks;
  sampling is panic-free under them by construction.
- `fix` intentionally requires either `-o/--output` or `--in-place` for
  writes; `--dry-run` is the check mode — it inspects only and exits `1`
  when repairs are pending, mirroring `lint`. Repairs are addressed by
  stable ids; every repair must be safe, lossless, and idempotent.
  Repair taxonomy (risk-tier groups) is deliberately deferred until a
  repair exists that doesn't meet that bar.
- **`fix` stays byte-surgical — a product requirement, not an accident**
  (decision recorded for #33). It patches only the offending animation
  bytes in the original container and copies everything else through
  verbatim, so meshes, skins, materials, and textures survive a repair
  bit-for-bit. Folding hemisphere/norm repair into a core `transform`
  and re-emitting through the unified `Document` writer was considered
  and rejected: the model writer re-emits and reorders accessors, so it
  is not byte-identical and would rewrite bytes `fix` must leave
  untouched. The `Document` round-trip is the right tool for
  `convert`/`transform`; in-place `fix` is not a round-trip.
- `convert` and `assemble` are compiled only with the `fbx` feature. `--no-default-features`
  remains a glTF-only pure-Rust CLI with validation, transform, fix, and
  diff commands intact; `report` is controlled separately by the
  `report` feature.
- `convert --bake-static-mesh-transforms` is an opt-in static geometry
  operation. It accumulates rest hierarchy transforms into positions and
  inverse-transpose normalized normals, then writes canonical identity-root
  geometry. It retains topology, UVs, and the model-supported material and
  embedded base-color, normal, metallic-roughness, and occlusion-texture data. It fails closed for any
  animation track, skin signal, uninstanced or shared mesh definition,
  malformed/non-finite data, singular or near-singular transform, or
  reflection; it neither bakes skinning nor guesses animated or reflected
  semantics. It conflicts with `--animation-only`, leaves default conversion
  unchanged, and is deterministic for repeated same-platform input/options.

- `convert --material-texture-recipe` supplies exact named BaseColor, normal,
  metallic-roughness, and occlusion mappings for a conversion. It conflicts with `--animation-only`, is
  compatible with static baking, and leaves the ordinary linked/embedded
  texture path untouched when omitted. Its versioned recipe and provenance
  contracts pin path containment, image processing, and encoder behavior.

- `assemble` is the versioned generic boundary for a skinned base plus clips
  spread across separate files or master timelines. It permits only explicit,
  mechanically verifiable operations: exact-name skeleton remap, exact mesh
  selection, clip slicing/endpoint/hold/gait operations, named channel removal,
  rest-track completion, quaternion cleanup, opt-in constant-track pruning
  after all other transforms while protecting effective-clip `animates_bones`
  names, fail-closed exact-name node-subtree removal after animation transforms,
  bind-consistent skin canonicalization, material recipes, and deterministic
  GLB/evidence emission. Node-subtree removal refuses any surviving track,
  mesh-instance, skin-joint, or complete source-skin reference; it remaps the
  surviving hierarchy but does not reparent descendants or garbage-collect
  mesh, material, or texture resources.
  Character-assembly recipe/evidence v3 introduced that node-removal contract;
  v4 retains it unchanged while adding optional rest/bind scale integration.
  Selectors resolve by exact unique name against the post-canonicalization base;
  their descendant union is planned before completion, excluded from completion
  targets, and projected after clip processing and constant-track pruning.
  `animates_bones` may therefore retain a protected track and force refusal,
  while `[rig] required_bones` remains lint presence policy rather than an
  assembly selector. Accepted non-empty projection clears optional source-native
  skeleton identity, because it cannot remain a complete account after authored
  deletion. Because mesh-bearing nodes are refused, removal cannot newly orphan
  asset resources; only the earlier explicit mesh-instance selection performs
  resource pruning. Evidence lists every removed node once in original
  parent-before-child order, distinguishes direct selectors from descendants,
  and retains original node and parent indices.
  Archive extraction, gameplay naming and acceptance policy, cache/generation
  policy, and publication remain with the consumer.

## 4. Repository & crate layout

One public repo, one cargo workspace, five published crates (plus one
`publish = false` dev crate, `animsmith-testkit`). The split is driven by
two hard constraints: the core must be consumable with zero C compilation
and minimal deps; FBX support pulls in a C build step most library
consumers must never pay for.

```
animsmith/
├── Cargo.toml                  # workspace, edition 2024
├── LICENSE-MIT / LICENSE-APACHE
├── THIRD-PARTY.md              # ufbx (MIT OR PDDL-1.0), vendored viewer assets
├── crates/
│   ├── animsmith-core/          # data model, sampling/FK, metrics, diffs, checks, config, findings
│   ├── animsmith-gltf/          # glTF/GLB → core model; GLB writer for `convert`
│   ├── animsmith-fbx/           # ufbx wrapper → core model; isolates the C build
│   ├── animsmith-report/        # self-contained HTML report generation
│   ├── animsmith/               # CLI binary (features: fbx, report — default on)
│   └── animsmith-testkit/       # publish=false: fixture builders shared by tests + the example asset generator
├── assets/viewer/              # viewer JS/CSS, inlined via include_str!
├── fuzz/                       # cargo-fuzz targets for the untrusted-input loaders
└── testdata/                   # CC0 rigs + procedurally corrupted fixtures
```

- **animsmith-core**: deps `glam` (the de-facto Rust game-math crate — do
  not hand-roll mat4/quat as the Python did), `serde`, `thiserror`. No
  file-format knowledge, no I/O. This is what embedding pipelines link.
- **animsmith-gltf**: the `gltf` crate with trimmed features (no image
  decoding); owns GLB emission via `gltf-json`.
- **animsmith-fbx**: `ufbx` (official bindings, v0.11.x, actively
  maintained; bundles the single-file C library via `cc` — no system
  deps). A separate crate rather than a feature flag so the C toolchain
  requirement is structurally isolated.
- **animsmith (CLI)**: `clap`, `serde_json`, `toml`. `--no-default-features`
  yields a pure-Rust glTF-only build.
- Toolchain: stable Rust, edition 2024, MSRV pinned in CI. License:
  MIT OR Apache-2.0. All crate names verified free on crates.io
  (2026-07-03).
- **fuzz/**: a nightly-only cargo-fuzz workspace (detached from the main
  workspace) with libFuzzer targets for the three entry points that ingest
  untrusted files — `animsmith_gltf::load`, `FixSession::apply_to_path(..., Repair::QuatFlip)`,
  and `animsmith_fbx::load`. These are the executable check on invariant-1
  ("untrusted input must never panic or OOM"): targets run in release mode
  to match the shipped CLI's panic semantics, with AddressSanitizer on. A
  weekly CI job (`fuzz.yml`) runs each target for 60s off a checked-in seed
  corpus; minimized crashers are committed under `fuzz/seeds/` as regression
  fixtures, each also pinned by a unit test in `animsmith-gltf`'s hardening
  suite. Continuous/long-running fuzzing (OSS-Fuzz) is deferred.

## 5. Core data model

Two representations of a loaded file, because checks genuinely need both:

**Raw layer** — what the file says. The document keeps the skeleton hierarchy
and rest pose, authored clips and tracks, optional scene assets, and source
metadata together. Mechanical checks use this layer for defects such as NaNs,
quaternion flips, key density, and constant tracks. Exact Rust types and fields
belong to the `animsmith-core` model rustdocs; build them with `just doc` or use
the package README's stable docs.rs link.

`assets` (meshes, skins, PBR materials, and embedded base-color, normal,
metallic-roughness, and occlusion textures) is the scene-asset half of
the document. Both the FBX and glTF loaders populate it from a single
`load` (there is no separate assets-carrying entry point — the two
loaders share one shape); it is empty when the input carries no scene
assets. The check catalog ignores it — checks judge animation — but it
rides the one `load`/`write` round-trip, so `transform` and `convert`
preserve geometry rather than silently dropping it, and `measure`
reports versioned static scene measurements from it (#16): geometry-definition
boxes in primitive coordinates, node-instance boxes in default/rest node-world
coordinates, and per-scene unions. These are not runtime-visible bounds:
animation, skin deformation, morph deformation, and runtime placement are
excluded. The output contract records unavailable static node bounds and scene
partial coverage rather than serializing non-finite values.

The glTF loader also retains metadata-only presence for secondary
`JOINTS_n`/`WEIGHTS_n` attributes. The measurement contract aggregates those
independent sides per mesh definition so unsupported or unpaired sets are
observable without changing the primary four-influence semantics. Secondary
per-vertex payloads, repair, and writer preservation remain outside the core
skinning model.

The scene-asset model keeps source mesh definitions separate from their
mesh-bearing node instances and retains declared scene roots plus the optional
default-scene index. That identity is measurement evidence: duplicate names do
not collapse, shared definitions are reported once, and nodes outside every
declared scene remain observable. The current glTF writer still normalizes an
emitted document to its existing single generated scene; preserving authored
multi-scene membership across `transform`/`convert` is not part of the static
measurement contract.

`validate_document_shape` is the model-owned snapshot check for the structural
invariants full-shape strict operations such as scale share: finite
parent-before-child rest topology, source identity/projection topology,
clip/track storage, and scene instance/inverse-bind references. `Document`
remains publicly mutable, so a
successful check is not a durable token; strict operations revalidate every
input at every public boundary. This is deliberately not universal model
validity. Measurement can retain finite instance evidence from hierarchy
branches unrelated to a malformed chain, sampling remains panic-free and
tolerant of hostile standalone tracks, and static baking validates only the
fields its static operation consumes. Scale's
negative-weight, finite-position, capability, closure, affine, parity, and
proof rules remain operation-specific.

Ingestion is **triangle-list only** — the target inputs are skinned game
rigs, and the model and writer carry no primitive-topology field. A
non-`TRIANGLES` glTF primitive (points, lines, strips, fans) is skipped
rather than misread as a triangle list; other topologies and their
retriangulation are out of scope for now.

**Sampled layer** — what a game runtime sees. A `PoseGrid` built by a
`ClipSampler`: uniform time grid over `[0, duration]` (resolution = max
channel key count, or explicit fps), glTF-spec interpolation semantics
(lerp for T/S, shortest-path slerp for R with negation on `dot < 0`, STEP
hold, cubic-spline Hermite), clamp at ends. For clips declared `loop`,
the wrap pair is `(last frame, frame 0)` — the seam definition every loop
check shares. FK accumulates every skeleton node's local TRS to model
space, including each root node's own transform. Metrics that need a
body-relative frame derive it from resolved roles such as hips and feet.
The metric grid is computed once per clip and shared across checks,
measurements, and reports through the lazy `MetricGrids` owner.

**Rig profiles** — checks reference semantic roles rather than bone names.
Profiles bind those roles to source-rig names; the exact public role set,
binding types, and matcher fallback belong to the `animsmith-core` profile
rustdocs.

Built-in profiles ship for `mixamo` (`mixamorig:Hips`…), `ue-mannequin`
(`pelvis`, `foot_l`…), and `humanoid` (`humanoid_ Pelvis`,
`humanoid_ L Foot`…), plus **auto-detection** that scores every profile by
resolved-role coverage and reports the winner in `inspect`. A check whose
required roles don't resolve records a typed coverage gap — never a false
finding. This is the single design rule that makes the tool useful outside
its birthplace: tolerance data and bone names are config; the math is not.

The runner owns selection, activation, and severity policy. Each check exposes
a cheap applicability predicate and one evaluation method. The evaluation
returns content findings separately from completed scopes and typed coverage
gaps. Severity overrides therefore cannot turn missing evidence into a false
error. `gait-group`, for example, can complete member-existence validation
while reporting phase coherence as a gap. `severity = "off"` disables a check
without hiding its applicability record. A check may also declare an opt-in
default; assigning "note", "warn", or "error" enables it while retaining the
same independent configuration record.

The built-in evidence-code declarations are the single authority for each
scope or gap code's machine identity, meaning, and allowed emitting check ids.
The evaluation boundary rejects a built-in code emitted by an undeclared
check, and the output-reference gate derives its exact inventory, meanings,
and emitters from those same declarations. Embedded checks remain open-ended:
they use namespaced custom codes rather than extending a closed enum.

**Checks** implement one trait and emit structured findings:

```rust
pub trait Check {
    fn id(&self) -> &'static str;              // "loop-seam", "quat-flip", …
    fn enabled_by_default(&self) -> bool;       // true unless policy is opt-in
    fn applicability(&self, ctx: &CheckCtx) -> Applicability;
    fn evaluate(&self, ctx: &CheckCtx) -> CheckOutput;
}
pub struct Finding {
    pub check_id: &'static str, pub severity: Severity,   // Note | Warning | Error
    pub clip: Option<String>, pub bone: Option<String>, pub time_s: Option<f32>,
    pub measured: Option<Value>, pub expected: Option<Value>,
    pub message: String,
}
```

The structured fields (not just a message string) are what make `diff`,
the JSON schema, and the HTML report cheap.

## 6. Check catalog

Tiers are shipping priority. "Prior art" = a proven implementation exists
in the incubating project's pipeline to port, with real-world numbers to
golden-test against.

### P0 — mechanical + the two killer semantic checks (v0 core)

| id | what it checks | needs | config | prior art |
|---|---|---|---|---|
| `nan` | NaN/Inf in key times or values | raw | — | new (trivial) |
| `time-monotonic` | non-increasing/duplicate key times; first key ≫ 0 | raw | epsilon | new |
| `quat-norm` | rotation keys with \|q\|−1 beyond tolerance | raw | eps (1e-3) | new |
| `quat-flip` | adjacent keys with `dot < 0` (long-way slerp in engines that don't neighborhood-correct) | raw | severity | new |
| `duration-sanity` | zero/degenerate duration; channels within one clip ending at different times; frame count non-integral at declared fps | raw + meta | expected fps list, pinned duration | reference contract `duration_s` pin |
| `scale-keys` | interpolation-aware temporal scale variation beyond tolerance | raw trajectory | severity | new |
| `non-uniform-scale` | component inequality anywhere on the interpolation-aware scale trajectory | raw trajectory | severity | new |
| `constant-nonunit-scale` | constant non-unit scale channel or single-key pin | raw trajectory | opt-in severity (off by default) | new |
| `constant-track` | redundant multi-key representation whose interpolation-aware trajectory never moves | raw trajectory | eps | new |
| `frozen-bone` | required bone's max angular deviation from first frame below floor | grid + roles/meta | `min_rotation_deg` | reference contract rotation floor + measured rotation ranges |
| `duplicate-loop-endpoint` | strict authored-key duplicate closing endpoint in a declared loop; warning/default-on | common finite strictly increasing timeline, valid cardinality, matching endpoints, interior motion + loop declaration | — | mechanical subset of #22's endpoint classifier plus open-cycle transform |
| `loop-closure` | maximum per-bone model-space last→first position and shortest-path rotation delta | grid + loop declaration | global `max_position_delta_m` / `max_rotation_delta_deg`; clip `max_loop_position_delta_m` / `max_loop_rotation_delta_deg` | consumer-neutral authoring-loop requirement |
| `loop-seam` | last→first position wrap discontinuity of feet-relative-to-hips, normalized by the *local neighbour* per-frame step, with a stride floor so stationary clips skip | grid + Hips/feet/toe roles | `max_ratio`, `min_stride_step_m` | `locomotion_metrics.py` — port verbatim |
| `loop-seam-vel` | maximum per-bone model-space difference between the in-clip velocities entering and leaving the wrap | grid + loop declaration | global `max_velocity_delta_mps`; clip `max_loop_velocity_delta_mps` | consumer-neutral authoring-loop requirement |
| `loop-seam-rot` | maximum per-bone shortest-path model-space angular-velocity difference between the in-clip steps entering and leaving the wrap | grid + loop declaration | `max_angular_velocity_delta_degps` | consumer-neutral authoring-loop requirement |
| `root-motion-speed` | horizontal root/hips displacement ÷ duration vs declared `speed_mps`; flags stray speed pins on non-locomotion clips | grid + Root/Hips | pinned speed + tolerance (reference gate: 15%) | reference bake |
| `missing-bones` | declared-required animated bones absent; tracks targeting nodes outside the skeleton | raw + meta | bone/role list | reference contract `animates_bones` |
| `required-bones` | declared required rig bones absent from the skeleton; static sockets and IK targets are valid | skeleton + rig config | `[rig] required_bones` | consumer-neutral structural rig contract |
| `naming` | clip names vs convention pattern | meta | regex/glob | new |
| `units-sanity` | hips rest height wildly outside human scale (the cm-vs-m export classic) | skeleton + profile | height band | new |

### P1 — locomotion semantics (the first-of-kind tier)

| id | what it checks | prior art |
|---|---|---|
| `gait-phase` / `gait-group` | stride-phase anchor from the fundamental-harmonic trough of the left-minus-right foot-height signal; circular phase spread across a declared clip ring (directional-blend coherence), with an `lr_amplitude` confidence floor | reference metrics module + gait-group contract — port verbatim |
| `in-place` | classify in-place vs root-motion (net + per-frame root displacement) and compare against the clip's declared expectation | new; trivial on the grid |
| `foot-slide` | detect stance (foot height + near-zero vertical velocity), measure horizontal foot velocity during stance in the travel-cancelled frame | new; hardest check — ships opt-in until corpus-tuned |
| `bind-pose` | rest pose vs first frame delta (clip authored against wrong bind); T-pose/A-pose classification; node-TRS rest disagreeing with IBM-derived rest (the disagreement is itself a finding) | reference sidecar already derives rest from IBMs |
| `axis-conventions` | character forward/up at rest vs declared axes; root orientation drift over a loop | reference contract axis vocabulary |
| `key-density` | keys/sec far above the clip fps (unbaked-curve bloat) or far below (starved track) | new |

### P2 — corpus/cross-clip

Cross-clip skeleton & rest-pose consistency across a directory;
ground-penetration of feet/toes; mirrored-pair symmetry (`walk_left` vs
`walk_right`); additive-clip suitability; compression-noise metrics
(per-track jerk); morph-target weight ranges; SARIF output.

## 7. Configuration

TOML (`animsmith.toml`, or `--config`); Rust-ecosystem norm and
diff-friendly in asset repos:

```toml
[rig]
profile = "mixamo"                 # or "auto", or an inline role map:
# A presence-only structural contract; these do not need animation tracks.
required_bones = ["root", "weapon_socket", "ik_hand_l"]
# [rig.roles]
# hips = "humanoid_ Pelvis"
# left_foot = "humanoid_ L Foot"

[checks.loop-seam]
severity = "error"                 # off | note | warn | error
max_ratio = 1.5

[checks.loop-closure]
max_position_delta_m = 0.01
max_rotation_delta_deg = 1.0

[checks.loop-seam-vel]
max_velocity_delta_mps = 0.1

[checks.loop-seam-rot]
max_angular_velocity_delta_degps = 5.0

[checks.quat-flip]
severity = "warn"

[clips."run_*"]                    # glob; exact > glob, later entries win ties
loop = true
in_place = true
max_loop_position_delta_m = 0.04   # finite per-family override; global is fallback
max_loop_rotation_delta_deg = 2.0
max_loop_velocity_delta_mps = 0.2
speed_mps = { value = 3.1, tolerance = 0.25 }
fps = 30

[clips.run_forward]                # exact fields overlay matching globs
max_loop_rotation_delta_deg = 0.5

[gait_groups.run-ring]
clips = ["run_forward", "run_back", "run_left", "run_right"]
max_gait_phase_spread = 0.08
min_lr_amplitude_m = 0.05
```

CLI flags override file config (`--select`, `--allow`, `--deny`).

**Engine-agnosticism rule:** the TOML file is merely *one* constructor of
a `CheckSet`. Embedding pipelines build check sets
programmatically through the library API and keep their own contract
formats, hashing, and tolerance semantics on their side. animsmith never
learns an embedder's contract schema.

## 8. Output formats

- **Text** (default): findings grouped per clip, measured-vs-expected on
  one line, colored; `--quiet` for CI summaries.
- **JSON** (`--format json`): final output v7, identified by
  `urn:animsmith:schema:output:7`. Lint emits one result per catalog check and
  represents selection, configuration, applicability, evaluation coverage,
  content findings, completed scopes, and typed gaps independently. Measure
  and lint share a nested, independently versioned measurement contract. The
  current contract inventories glTF material definitions, the five core
  `base_color`, `normal`, `metallic_roughness`, `occlusion`, and `emissive`
  texture slots, texture-to-image identity, and decoded image metadata in
  source order. Complete coverage is scoped to that documented core domain;
  extension-defined texture slots are not implied. The contract makes absent
  loader support explicit as unavailable coverage and separates a
  source-declared MIME type from byte-detected container and decoded image
  facts. This remains measurement evidence, not an image acceptance, repair,
  resize, transcode, color-space, writer-preservation, or recipe-authority
  policy.
  Measurements v13 retains exact raw inverse-bind matrices and publishes
  affine and conditioning evidence before deriving their inverses. It also
  carries forward v12's corrected linear-transform observations through shared
  f64 affine facts; v12 and earlier contracts remain immutable historical
  evidence.
  `convert --format json` instead emits the separately versioned
  `urn:animsmith:schema:conversion-evidence:2` producer-evidence contract:
  requested options, written-artifact counts, optional static mesh bake entries
  with source/output identities and applied world transforms, and deterministic
  material-texture recipe provenance when requested. v1 remains historical;
  the current CLI emits v2 exclusively.
  CLI exit status derives only from content severity (warnings block only
  with `--deny-warnings`); coverage gaps are nonblocking evidence.
  The output-v7 envelope types and immutable identities live in
  `animsmith-core` so CLI and embedded producers serialize the same reporting
  contract. Static-bake evidence is also a public core type; the conversion
  envelope remains a CLI producer contract.
- **Future serializers**: no game-industry standard exists for skeletal
  animation lint results. Keep native JSON as the source of truth, then
  add serializers where downstream tools expect them: SARIF for code
  scanning, GitLab Code Quality/CodeClimate for MR widgets, JUnit XML for
  CI test dashboards, and CSV/HTML for humans.
- **`diff A B`**: compares measurement maps per metric with per-metric
  significance thresholds (defaults derived from configured tolerances);
  prints deltas; exits 1 on significant movement. Primary use: "did this
  DCC re-export change anything that matters?"

## 9. HTML report (the visual preview)

`animsmith report clip.glb -o report.html` → **one self-contained offline
HTML file** (CI-artifact- and PR-attachment-friendly; no CDN, no install).

**Key design choice: no three.js, no `<model-viewer>`.**

- `<model-viewer>` can play a GLB but exposes no skeleton/per-frame API —
  it cannot draw bone lines, foot trails, or sync to charts. Fails the
  requirement outright.
- three.js (~650KB inlined + GLTFLoader) works, but it would *re-sample
  the animation in JS*, and its loop/slerp behavior may disagree subtly
  with what the linter measured — the preview could contradict the
  findings it illustrates. It is also an update treadmill.
- The decisive observation: **the report never needs to sample animation
  in JS.** The Rust side already computed the `PoseGrid` — model-space
  joint positions for every frame the checks judged. Embed that.

So the viewer is a hand-written **WebGL2 skeleton renderer (~15KB)**:
bones as line segments, joint dots, root-motion and foot trails, orbit
camera, play/scrub transport. It renders exactly the frames the checks
measured — when `loop-seam` flags the wrap at frame N, the viewer scrubs
to *that* frame N. Determinism is the feature.

- **Embedded data**: pose grids as base64 Float32Array in
  `<script type="application/json">` blocks (~60 bones × 3 floats × 300
  frames ≈ 290KB base64 per clip; f16 quantization is the escape hatch if
  reports grow). The source GLB embedded once as a download button.
- **Charts**: Rust-generated inline SVG — root-motion top-down path, foot
  heights, L−R gait signal with the fitted fundamental, per-bone
  seam-delta bars — with a small shared JS playhead syncing a cursor line
  across all charts and the 3D view.
- **Findings panel**: each finding links to its clip + time; clicking
  scrubs the viewer.
- A skinned-mesh view (vendored three.js, `--report full`) is a P2 option
  the crate layout leaves room for; it is presentation polish, not v1.

## 10. FBX ingestion (`animsmith-fbx` + `convert`)

- **Library**: the official `ufbx` Rust bindings (v0.11.x, actively
  maintained; the same C foundation the incubating pipeline already trusted, so
  behavior is already trusted in the incubating pipeline).
- **Normalization at load** via `LoadOpts`: target axes = glTF convention
  (right-handed, +Y up, −Z forward), `target_unit_meters = 1.0` (FBX
  defaults to cm), transform-adjust space conversion (don't rewrite
  geometry), helper-node handling for 3ds Max geometric transforms. The
  core model only ever sees glTF-convention data regardless of source.
- **Animation extraction** uses ufbx's `bake_anim` — it evaluates anim
  stacks/layers, cubic/TCB curves, pre/post-rotation, and inherit-scale
  modes into resampled TRS keys (rate from the FBX TimeMode,
  overridable). Each anim stack (take) becomes one core `Clip`. This
  sidesteps the entire FBX-curve-semantics swamp.
- **Scale inventory without enablement**: the same successful parse produces
  a deterministic ufbx-side status for every current Appendix D.4 domain and a
  documented normalized source-node/source-skin projection when every declared
  skin slot can be represented. A missing cluster bone downgrades that generic
  projection to unavailable instead of silently dropping the slot. It
  explicitly names coordinate/inherit normalization, baked and lost curve state, generated
  normals/helper nodes, cluster-derived bind provenance, four-influence
  truncation/renormalization, rejected negative/non-finite/unrepresentable
  influences, triangulation/welding, omitted face/edge payloads, uninstanced
  mesh definitions, unsupported source data, and ufbx source identities. Only
  successfully projected cluster binds count as bone-convenience overwrites.
  One unreadable cluster makes its ordered bind declaration
  atomically unreadable; finite prefixes are not retained under shifted slots.
  Stackless authored curves, point/line faces, and attached zero-face mesh
  definitions are present-but-unsupported rather than absent or rebuilt; the
  zero-face definitions retain an exact count and stable source identity.
  Retained normalized meshes keep one definition per stable ufbx mesh identity;
  shared source geometry produces multiple node instances that reference the
  same compact normalized definition. They keep their stable ufbx typed
  identity so source-skin attachment joins cannot shift
  when an earlier source mesh emits no primitive.
  Its core capability projection remains
  unsupported; neither scale operation is an FBX producer yet, and no raw FBX
  span or artifact-preservation claim is made.
- **`convert`** emits glTF 2.0 GLB: nodes + skin (computed IBMs) + one
  animation per clip with at least one writable track; mesh + weights when
  present; `--animation-only` to strip mesh. The glTF writer returns counts
  derived from the emitted artifact, which both `convert` and `transform` use
  for their summaries and to report source clips omitted because they have no
  writable tracks.
  Explicitly *not* an art exporter — no material fidelity promise. Both
  `convert` and `transform` share one `load`→`write` round-trip over `Document`
  (assets included), so geometry survives a transform pass and
  `--animation-only` clears it uniformly across input formats (it is the only
  lever that drops geometry).
  `--bake-static-mesh-transforms` is the separate opt-in path for static,
  unskinned, singly-instanced geometry: it removes the effective rest
  hierarchy transform by baking positions and normals under an identity root
  and reports each applied transform in conversion evidence. Unsupported
  animated, skinned, malformed/non-finite, singular/near-singular, or
  reflected input is rejected before output.
- **FBX pitfalls double as checks** when linting `.fbx` directly: source
  unit ≠ 1m (warn even though we convert), Z-up source, namespace-prefixed
  bone names (profile matcher strips), default "Take 001" naming (feeds
  `naming`), baked-key explosion (feeds `key-density`), non-uniform
  inherited scale.

## 11. Roadmap

- **M0 — walking skeleton.** Repo bootstrap: workspace, dual license, CI
  (fmt/clippy/test on Linux/macOS/Windows), CC0 test fixtures. Core model
  + sampler/FK; `animsmith-gltf`; `inspect` and `measure --format json`;
  the mechanical P0 checks (`nan` → `constant-track`).
- **M1 — reference parity.** Rig profiles, TOML config, per-clip
  expectations; port `loop-seam`, `frozen-bone`, `root-motion-speed`,
  `gait-phase`/`gait-group` — **golden-tested against the reference
  implementation's verified production numbers**; `lint` with exit codes + stable JSON; adopt
  the reference project's mutation-test discipline (corrupt one field, assert the finding
  names exactly that field). **the incubating project's measure
  port lands here**: its sidecar tool becomes a thin wrapper over `animsmith-core` + `animsmith-gltf`
  (the embedder keeps its sidecar schema and hashing);
  `locomotion_metrics.py` and the animation half of `measured_sidecar.py`
  are deleted.
- **M2 — report, FBX, diff → v0.1.0 on crates.io.** HTML report; FBX
  ingestion + `convert`; `diff`. The v0.1 bar for "usable by an unaffiliated
  team": README quickstart works on a raw Mixamo-style GLB with zero
  config (profile auto-detect), built-in `mixamo` + `ue-mannequin`
  profiles, sample `animsmith.toml`, versioned JSON schema doc, no incubator
  vocabulary anywhere in the public API.
- **M3 — the hard semantics.** `foot-slide` (stance detection),
  `in-place`, `bind-pose`/`axis-conventions`, rotational/velocity loop
  seams, `--deny-warnings`, baseline/suppression file for teams adopting
  with a dirty back catalog.

## 12. Risks & open questions

1. **Foot-contact detection robustness** (M3) is the only research-grade
   item — thresholds vary with rig scale and style. Ship opt-in at `warn`
   until tuned on a corpus.
2. **Test-asset licensing**: Mixamo clips cannot be redistributed. Use
   CC0 rigged clips (Quaternius/KayKit/self-authored) + procedurally
   corrupted fixtures; budget M0 time for this.
3. **Sampling-semantics fidelity**: engines differ subtly (cubic
   handling, sub-frame wrap). The doc pins the model — "glTF-spec
   interpolation on a uniform grid, wrap = (last, 0)" — and exposes grid
   fps as config, accepting it is a model of runtimes, not all of them.
4. **ufbx 0.x churn** and thin docs.rs coverage: pin exact versions,
   treat the C library's docs as canonical, keep `animsmith-fbx` thin and
   swappable.
5. **Rest-pose truth**: node TRS and inverse-bind matrices can disagree.
   Rule: IBM-derived rest is authoritative when a skin exists, node TRS
   otherwise, and disagreement beyond tolerance is a `bind-pose` finding.
6. **Report size** on long/many clips: f16 quantization + per-clip lazy
   JSON blocks if it bites.
7. **Scope pressure toward transformation** will come ("you detected the
   seam pop — just fix it"). The linter-first line is the identity; only
   `convert`, `transform`, and mechanical/lossless `fix` operations may
   mutate data.

## Appendix A — naming decision record

Two decisions, both 2026-07-03 (the project was renamed the same day
it was built, before anything was published):

1. **`animlint`** was chosen first, on a linter-first scope with clip
   transformation explicitly out of scope. Rejected then: `gltf-lint`
   (glTF is the carrier, not the domain; permanently confusable with
   Khronos glTF-Validator's spec-conformance role; wrong the moment FBX
   input landed), `animkit`/`clipkit`/`clipforge` (existing projects),
   `gaitkeeper` (minor collisions; overemphasizes locomotion).

2. **Renamed to `animsmith`** the same day, when transformation became
   first-class: the incubating pipeline needs hemisphere normalization,
   frame-range slicing, hold-extend, and gait-anchor rotation sooner
   than later, and its archived-FBX2glTF conversion step wants a
   maintained replacement. The naming record had said the lint-first
   name "only breaks if clip transformation becomes first-class" —
   that fork flipped, so the name followed before the first crates.io
   publish made it permanent. `animsmith` and all sibling crate names
   verified free on crates.io with zero GitHub repository hits. Lint
   remains the flagship subcommand.

## Appendix B — prior-art map (first consumer)

The measurement algorithms, config vocabulary, and testing discipline
were extracted from a private game project's asset pipeline — the
uniform-grid sampler, the local-neighbour loop-seam denominator with
its stride floor, the L−R fundamental-harmonic gait anchor, the
root-motion speed gate, the `Pinned{value, tolerance}` expectation
shape, the `Finding`/severity/exit-code conventions, and the
mutation-test style (corrupt one field, assert the finding names it)
are all faithful ports, golden-tested against that pipeline's shipped
numbers. Its measurement scripts are deleted as the project migrates
onto the animsmith library — the standing proof that the public API is
sufficient for a real bake pipeline.

## Appendix C — result-contract ownership decisions

These decisions record result-contract ownership after output v3 finalization:

1. **The versioned envelope belongs to `animsmith-core`, not the CLI binary.**
   The CLI still supplies its build identity and frontend policy, but
   `MeasurementContract`, the command-specific `MeasureFileReport` and
   `LintFileReport` records, command-specific envelopes, summaries, and URNs
   are shared library types. This lets embedded producers emit the exact
   schema without copying private structs or protocol strings while making an
   invalid measure/lint record shape unrepresentable.

2. **Nested findings keep their `check_id`.** A `Finding` is also a standalone
   embedded result and the record consumed by text, Markdown, and HTML
   presentations. Keeping it self-describing avoids a second wire-only finding
   projection and supports extracting findings without retaining their parent
   check record. Output v3 therefore accepts the small nested redundancy, and
   `evaluate_checks` rejects a child id that disagrees with its parent rather
   than serializing ambiguous ownership.

3. **Read-side validation is consumer-neutral.** `animsmith-core` validates
   output and nested measurement identities and recovers every file's display
   path plus complete clip and mesh measurements in source order. It does not
   impose a file-count rule or prescribe CLI remediation. The `diff` frontend
   owns its single-file policy and operator guidance; embedded consumers may
   accept empty or multi-file reports according to their own workflows.

## Appendix D — decision record: skinned rest/bind scale canonicalization

**Status (2026-08-17): implemented for self-contained glTF/GLB and character
assembly recipe/evidence v4.** The shared
core plan, typed ledger, exact-source writers, independent core and artifact
proofs, atomic CLI publisher, and immutable scale-evidence v4 producer are the
current contract. Character assembly v4 can opt into rest/bind scale with
explicit selectors and factor after validating a versioned basis for the base
and every separate clip; recipe/evidence v1 through v3 remain unchanged.
Whole-document conversion supports
raw glTF `POSITION` morph deltas while preserving static JSON weights as
numeric values and animated weight accessor payloads byte-exactly; other morph
semantics and every rest/bind morph payload remain refused. FBX scale output
remains deferred.

### D.1 Problem and two distinct operations

A skinned mesh can already have the intended physical dimensions while its
joint hierarchy carries a compensating uniform scale. For example, a root can
contribute `0.01`, descendant translations can be authored 100 times larger,
and inverse-bind matrices can carry a compensating linear magnitude of 100.
Skin deformation is then visually correct, but a rigid object parented to a
joint inherits the unwanted `0.01` scale.

That case must not be confused with a document whose *entire* linear unit is
wrong. The following are separate, explicitly selected operations:

1. **Whole-document linear-unit conversion** changes physical size. The caller
   declares a finite factor `q > 0`; every represented length is converted by
   `q`. It is appropriate only when the source was authored in a different
   linear unit.
2. **Rest/bind hierarchy reparameterization** preserves already-correct world
   geometry. It removes one compensating inherited scale from a restricted
   skinned hierarchy, rebases local rest and animation translations, and
   regenerates inverse binds.

Neither operation may infer its factor or applicability from mesh bounds,
character height, joint lengths, inverse-bind magnitude, filename, or an asset
category. Measurements from #267 are diagnostic evidence, not conversion
authority. A caller must name the operation and declare or accept the exact
factor that the transform plan validates.

The declared unit-conversion transform is a finite positive scalar only; it is
not an axis, rotation, translation, shear, or reflection operation. The initial
reparameterization source class is finite, orientation-preserving, positive
uniform rest-world scale. Non-uniform scale, shear, reflection, singular or
near-singular matrices, non-finite values, and mixed effective factors in the
selected domain fail before any output is written.

Classification and proof share one versioned tolerance policy and compute in
`f64`, narrowing only at the writer model boundary. The current policy identity
is `appendix-d-v6`. The three finite widened axis lengths are sorted in
ascending order before their arithmetic mean is accumulated and divided by
three. This canonical association rule makes the common factor independent of
authored axis order, including when the sum lies on an equal-axis boundary.
Relative orthogonality, equal-axis, and common-factor tolerance is `1e-5`; an
axis is unequal when
`abs(length - average) > 1e-5 * max(average, length)`, relative to the longer
of the two and to nothing else; a determinant is singular when
`abs(det) <= 1e-6 * product(axis_lengths)`; scalar/vector comparison uses
`abs_error <= 1e-6 + 1e-5 * max(abs(before), abs(after))`; and shortest-path
rotation residual is at most `1e-5` radians. Every bound is inclusive: a
residual exactly equal to its bound is accepted. Exact float equality is
forbidden. The implementation records every threshold and observed maximum
residual in evidence, so noisy values such as `100.000015` can be accepted for
an explicit, reviewable reason rather than by implementation accident.

This is a pre-1.0 policy-identity cutover: the former public
`ScaleTolerancePolicy::APPENDIX_D_V5` associated constant is removed rather
than retained as an alias for the new axis-order-independent meaning. Evidence
records therefore use `appendix-d-v6`. The historical v5 calibration and
revision history live in the
[scale proof calibration notes](docs/scale-calibration.md); the recurrence v5
introduced and v6 retains is specified below as part of the current policy.

**The `f32`-rounding term, per obligation.** The scalar band above is stated
relative to the quantity being *compared*. For five of proof's obligations
that quantity is not the magnitude the arithmetic ran on, and a rotation can
separate the two without limit: a bound component near zero on a mesh four
thousand units across, a near-identity `W * B` whose translation column is the
difference of two terms of magnitude `abs(W)`, or a world translation whose
parent chain cancelled two terms larger still. The compared number is
then small and the absolute rounding error it carries is large, so a purely
relative band derives the tolerance from the small number and the error from
the big one — and `plan_scale` accepts a plan whose candidate `prove_scale`
then refuses. Sweeping a rotating rig at a declared factor of `3190`, that
happened on `86 %` of rotations before this policy, with residuals up to
`9.8e-4` against a `6.1e-5` band.

`appendix-d-v6` therefore retains the further quantity `f32_rounding_ulps =
4`, and the five obligations that compare `f32`-rounded arithmetic use

```text
abs_error <= 1e-6 + 1e-5 * max(abs(before), abs(after))
             + f32_rounding_ulps * magnitude * 2^-23
```

where `magnitude` is named per obligation below. The added term is *absolute*
in `magnitude`, never relative to the compared quantity, which is what makes
it incapable of loosening a comparison whose operands already are its own
magnitude: there it contributes `4 * 2^-23 = 4.77e-7` of them, twenty times
below the `1e-5` the relative band already allows. The `2^-23` is
`f32::EPSILON`, because every one of these residuals is the difference of two
`f32` quantities even though the subtraction itself is done in `f64`.

- **Skinned bounds** takes `magnitude` as the largest per-vertex arithmetic
  provenance over every contributing vertex — read off the candidate, and
  maxed against the source's own *rebased by the conversion factor*:
  `candidate.max(q * source)`, for the reason given after this list. Each
  influence has three stages whose cancellation a later stage cannot recover:

  1. the magnitude the `W * B * p` transform runs on, which is
     `max over i of sum over k of abs((W * B)_ik) * abs(p_k)` with `p` extended
     by the homogeneous `1` that `transform_point3` sums the translation column
     in with — the *product* `abs(W * B) * abs(p)` and not either factor alone
     — because the weighted sum over slots that follows can cancel those terms
     against each other;
  2. the magnitude the contributing slot's `W * B` composition ran on, which
     covers the cancellation that composition performs; and
  3. the magnitude that slot's joint world translation was itself accumulated
     from along its parent chain, which covers the cancellation performed one
     composition earlier.

  For one influence, the base is the maximum of those three stages. A vertex
  combines those influence bases with the same binary64 weighted average of
  the stored non-negative binary32 weights as its skinned point:
  `sum(w_i * base_i) / sum(w_i)`. The accumulator then takes the maximum vertex
  base. All three stages are load-bearing, and each dominates the others by an
  unbounded ratio on some rig. The skinned points alone miss a joint placed far
  from the geometry it carries — a `1000`-unit local translation under a `3190`
  root puts the joint `3.2e6` from the origin while its vertices skin to within
  one unit of it, and the bound inherits `1.6e-1` of error from a `9.7e-1`
  extreme. Dropping the slot-composition stage makes the calibration sweep
  refuse correct candidates in fifty-six of its seventy-two pre-v4 cells, at
  up to `62.6` ulps of the base that remains; dropping the transform stage
  outright refuses in thirty-four, and narrowing it to `abs(p)` alone refuses
  in fifteen at up to `47.7`.

  **Stage 1 has been wrong twice, in the same place.** It first read the
  *skinned* point's magnitude and claimed that covered the transform; the
  correction read the vertex position `abs(p)` and claimed the same. Both name
  one term of a product of two, and each was exact only on the rigs that
  happened to be built.

  - The skinned point coincides with the transform only while no two slots
    oppose. Two slots whose composed `W * B` differ by a half turn send a
    vertex `1000` units out to a blended origin, so the result reads `0` while
    every term summed to produce it carried that vertex's own ulp; put the
    joints near the origin as well and stages 2 and 3 read `1`. The residual is
    then `6.1e-5` — one ulp of `1000` — against a `1.5e-6` band, a demand of
    `503` ulps, rising to `65_527` at `abs(p) = 1e5`.
  - `abs(p)` coincides with the transform only while `abs(W * B)` is `1`, and
    the one rig that reached the cancellation composed `W * B` from a *pure
    rotation*, where it is `1` by construction. Give those same two slots a
    uniform scale of `k` and the sum still cancels while each term carries the
    rounding of a `k * abs(p)`-magnitude transform: at `k = 1024` and
    `abs(p) = 65536` the residual is `4.0` against a `0.0313` band, a demand of
    `512` ulps, and the rig refuses from `min(k, abs(p)) > 8` upward with the
    shortfall growing as `min(k, abs(p))`.

  Both need only that the bind pose is not the rest pose, which is ordinary
  content: with `W * B` the identity on every slot there is nothing for a blend
  to cancel *and* nothing but `1` for `abs(W * B)` to be, which is why a
  section's worth of fixtures built on analytic inverse binds could express
  neither case.
  `two_slots_whose_composed_binds_cancel_a_vertex_still_prove_its_bounds` pins
  the cancellation and
  `two_slots_with_a_scaled_composition_cancel_a_vertex_and_still_prove_its_bounds`
  pins the second factor; both build their binds independently of the rest
  world for that reason, and the
  [calibration sweep](docs/scale-calibration.md) carries composed slots at
  `abs(W * B) = {1e-3, 1, 1e3}` so a third factor of the same kind is visible
  to a measurement rather than only to a reviewer.

  **The blended point is not another stage.** Shared scale-input validation
  refuses every finite negative primary skin weight before planning. The
  implementation widens the stored binary32 weights, transformed-point
  numerator, and denominator to binary64, divides once, and narrows the final
  point to binary32. This prevents a lone subnormal contribution from
  disappearing before division, several large finite weights from overflowing
  their denominator, and rounded binary32 coefficients whose sum exceeds one
  from overflowing an otherwise finite convex blend at `f32::MAX`. Bounds
  residuals are compared per axis, so each blended component is already
  bounded by the same binary64 weighted transform operands. v3's L2 length
  could be up to `sqrt(3)` larger while naming no additional per-axis
  arithmetic; v4 removes it.

  This validation decision removes two formerly unbounded affine cases. With
  weights `1.0` and `-0.99999`, a positive near-zero denominator could amplify
  opposed transformed points to `1.997e8`; with identical transformed points,
  both numerator and denominator could cancel while the accumulation error was
  still divided by the near-zero sum. No finite ULP count could cover the
  latter generally. The two #336 counterexample rigs remain as fixtures and
  pin `NegativeSkinWeight` at the shared planning boundary instead of forcing
  tolerance machinery to describe invalid input.

  Weighting the provenance is equally load-bearing in the other direction. A
  stage at magnitude `1e20` with weight `1e-20` contributes about `1`, not
  `1e20`; an unweighted max would let an influence with essentially no effect
  buy the whole distant stage's tolerance. Separate named fixtures isolate the
  transform and slot-composition halves, a proof-level fixture measures the old
  and v4 detection floors, and the calibration has an explicit profile that
  gives each vertex's larger production influence base the smaller weight.

  The proof-level floor is an adjacent-binary32 bracket on the same
  candidate-only weight mutation in both policies. A frozen v3 evaluator first
  refuses at candidate weight bits `0x3a7a1be4` (the preceding
  `0x3a7a1be3` is accepted), where the Bounds residual is
  `9.5320709005312e13`. v4 searches both directions around the authored
  `1e-20` weight; its smaller refused residual is `2.4400651454925537e-6`,
  with the lower bracket `0x1e3c6f0a` refused / `0x1e3c6f0b` accepted and the
  upper bracket `0x1e3d5b21` accepted / `0x1e3d5b22` refused. That is a
  `3.906e19x` recovery in this adversarial rig. It is a measured detection
  floor for this fixture, not a universal precision claim, and the exact bits
  and residuals are pinned in the test.

  `magnitude` is deliberately *not* read off the bound corner being compared.
  A per-axis extreme is contributed by whichever vertex happened to be
  furthest along that axis, so three vertices at `(3000, .001, .002)`,
  `(.001, 3000, .003)` and `(.002, .003, 3000)` build a corner of magnitude
  `2.4e-3` out of vertices of magnitude `3000`. A corner-derived band is a
  million times tighter than the error the corner carries, and — read the
  other way, as a base for the *relative* term — a corner-derived tolerance
  admits `4.2e-2` on a component of magnitude `5.98`, which is `0.71 %`
  relative error and no longer a rounding allowance at all.

- **The skin equation** and **the inverse binds of an unaffected instance**
  take `magnitude` as the largest quantity any entry of the compared product
  was summed from: `max over (i, j) of sum over k of abs(a_ik) * abs(b_kj)`.
  For `W * B` that is `6.4e3` on the rig above, not the near-identity
  product's `1.0`: the residual there is `1.8e-4` against the `1.1e-5` band
  the product magnitude buys, a `16x` shortfall, and `2.50` ulps of the
  operand magnitude after it. The skin equation maxes that against the same
  parent-chain magnitude skinned bounds takes, because `abs(W) * abs(B)` reads
  the *already composed* `W` and so cannot see terms the chain that produced
  it already cancelled. A joint whose local offset points back along its
  parent's world translation leaves `abs(W) * abs(B)` at `1.0` while the
  residual it must admit is `6.25e-2` — `524288` ulps of that base, and `0.08`
  of the chain's. The skin equation reads that quantity off the candidate and
  maxes it against the source's rebased by the factor, exactly as skinned
  bounds does, and for the same reason. It is also not
  `matrix_magnitude(a) * matrix_magnitude(b)`, which replaces the sum over
  `k` with a product of two independent maxima and reads `7.6e6` where the
  arithmetic ran on `6.4e3` — a tolerance from that would accept a matrix
  that is entirely wrong, and the over-acceptance calibration fixture refuses
  exactly that case.

  For an unaffected instance's binds the two operands are the effective matrix
  and the declared factor, and scaling a column cancels nothing, so the term
  is inert there: a correctly built candidate leaves the effective bind
  unchanged and therefore has exactly zero residual. It is stated rather than
  omitted so that the policy quantity means one thing across every obligation
  that compares `f32` matrices.

- **Rest translation** and **sampled trajectory** compare a node's world
  translation between the two documents, and take `magnitude` as that node's
  parent-chain magnitude alone — the same quantity the two above max against,
  read off the rest pose and off the sampled pose respectively. There is no
  product to sum here: the comparison is between two composed world
  translations, and the only way one of them can be small while its error is
  large is the chain cancellation.

  The root carries zero provenance because its world is a copied local. For
  child `i`, with parent world `W_p`, local matrix `L_i`, and inherited
  provenance `C_parent`, the shipped binary64 recurrence is

  ```text
  s_i,r = sum over k in {0,1,2}
          f64(abs(W_p[r,k])) * f64(abs(L_i[k,3]))
  p_i,r = f64(abs(W_p[r,3]))
  m_i,r = s_i,r + min(max(p_i,r, f32::MIN_POSITIVE), s_i,r / 2^-23)
  m_i = max over spatial rows r of m_i,r
  C_i = C_parent + m_i
  ```

  The first term provisions the local three-product dot. The second provisions
  the final addition by paying the smaller of one ulp at the carried parent's
  scale and the whole new contribution being absorbed. The minimum-normal
  floor covers subnormal product rounding without imposing a fixed charge when
  `s_i,r = 0`. Only the three spatial output rows participate; affine row 3 is
  the exact homogeneous coordinate. Rest and sampled pose construction share
  this recurrence. Its calibration populations and revision history are
  recorded separately in the
  [scale proof calibration notes](docs/scale-calibration.md). The
  policy-neutral matrix rewrite, residual, and provenance kernels live in
  `crates/animsmith-core/src/scale/numeric.rs`; operation selection, tolerance
  decisions, reference expectations, connector products, and proof-owned
  expected values remain outside that leaf.

  This scalar is not a universal componentwise forward-error proof: it does
  not separately propagate error in the inherited linear block before that
  block acts on a later translation. The shipped claim is this additive
  recurrence and its checked empirical envelope, not an unbounded-depth
  analytic guarantee. At the later fixed stages, SkinMatrix maxes the
  accumulated chain against the `W * B` operand magnitude and Bounds feeds
  that slot base through the weight-proportional blend described above. A
  fully analytic matrix-error propagation would be a different model and
  policy contract.

  The chain is the **candidate's**, not the source's and not the max of the
  two. Whole-document conversion scales every translation by the factor and
  leaves every linear part alone, so the two documents' chains are that factor
  apart — subject to the candidate's `f32` narrowing of the factor, since the
  build scales by `factor as f32` while the proof rebases by the `f64` factor,
  a relative difference of at most `2^-24` (`1.49e-8` at `q = 0.1`, whose
  `f32` is `0.10000000149011612`) that this very rounding term covers many
  times over — and the source's rounding is rebased by the same factor before
  it is compared, so the residual scales with the candidate's chain at either
  end of the factor range, and under rest/bind the two chains are equal
  outright. A max over the two sides is therefore never *needed*, and under a
  shrinking conversion it is strictly worse: at a factor of `0.01` the source
  chain is `100x` the candidate's, so the max freezes the band at the source
  rig's size while the candidate it is spent on keeps shrinking, and the
  smallest genuinely wrong candidate either obligation refuses stops tracking
  the factor at all. On the cancelling-chain rig that costs `89x` of
  discriminating power at `0.01` and `648x` at `1e-4`.
  `a_shrinking_conversion_holds_rest_translation_to_the_candidate_s_own_chain`
  and `a_shrinking_conversion_holds_trajectory_to_the_candidate_s_own_chain`
  pin the tightening; the two `..._holds_..._to_the_candidate_side` fixtures
  pin the growing direction, where the candidate's chain is also the larger.

**Transform-only affine is not in this rounding class.** It compares a probe
point transformed through the complete expected and actual world affines as a
vector L2 residual against a vector-magnitude base, so its comparison base is
already the magnitude its arithmetic ran on and it takes no rounding term.

**Overflow is not `NaN`.** A skinned position that leaves the `f32` range is
reported as `InvalidSkinnedPrimitive { reason: "skinned_magnitude_overflow" }`,
separately from the `"non_finite_result"` reported for `NaN`. Both fail closed,
and neither has a fixed preflight magnitude limit: skinning accumulates a dot
product per axis, so overflow depends on orientation and operand magnitudes,
not only on the magnitude of the result.

The magnitude calculations follow the same finite-domain rule. Bounds are
compared per axis and never square a finite blended point into an L2 length
that could overflow above `sqrt(f32::MAX)`. The parent-chain
`abs(W_parent) * abs(t_local)` sums are accumulated in binary64. Composition
and column operand magnitudes are summed in `f32` lanes and recomputed in
binary64 whenever that sum leaves the `f32` range, so an operand magnitude is
never reported as `inf` while the composed result remains finite. These are
current `appendix-d-v6` proof and refusal semantics, not calibration
measurements.

**Why the source side is rebased.** Every residual above is `|a - q * b|` for
the candidate quantity `a` and the source quantity `b`, so the source operand
enters the comparison multiplied by the factor and its *rounding* is multiplied
by the factor with it: a source quantity accurate to `k` ulps of its own
magnitude contributes `q * k` ulps of that magnitude to the residual. A base
that reads the source side unrebased — which `max(source, candidate)` did for
skinned bounds and the skin equation — therefore states the source's error in
the wrong units by a factor of `q`. Under a shrinking conversion that is the
whole defect, and it is loose by exactly `1/q`: the band freezes at the source
rig's size while the candidate it is spent on keeps shrinking. Measured on the
far-joint rig, the smallest inverse-bind shift the skin equation refuses goes
from `1.9e-3` under the max to `1.3e-5` at `0.01` and `1.3e-7` at `1e-4`; the
smallest bounds error refused goes from `4.8e-1` to `4.8e-3` and `4.9e-5`.
`a_shrinking_conversion_rebases_the_skin_matrix_magnitude_by_the_factor` and
`a_shrinking_conversion_rebases_the_bounds_magnitude_by_the_factor` pin both.

Unlike the parent chain this does *not* reduce to the candidate's magnitude
alone, because these two magnitudes are products: only the terms carrying a
translation are a factor apart, and both sides retain an unscaled `O(1)` floor
from the composition's linear block and from the exact `1.0` the homogeneous
row contributes to every chain. Where that floor dominates the source,
`q * source` exceeds the candidate's magnitude under a growing conversion by up
to the whole factor. Production retains the `max` because it is the operand
bound justified by the comparison; no smaller candidate-only or `min` base is
part of the policy contract.

An earlier calibration tried to classify those smaller bases as equivalent by
dividing an obligation-wide maximum residual by a separately selected maximum
slot base. Those maxima can belong to different slots: after calibration began
recording each production comparison's own residual and provenance, the
shipped SkinMatrix demand was `2.213` while that mispairing still printed
`2.084`. The alternative-base figures and equivalence claim were removed
rather than preserved as false mutation evidence.

**Calibration evidence.** The checked-in release calibration, its deterministic
populations, current reference demand tables, historical policy comparisons,
and machine-local cost measurements live in the separate
[scale proof calibration notes](docs/scale-calibration.md). Those notes measure
this policy; the normative thresholds, provenance definitions, and refusal
boundaries remain in this decision record.

**Normative residual norm for the unit-scale postcondition.** The postcondition
"unit composed scale for every affected node" is measured **per axis**, as
`max(abs(scale_axis - 1))` over the three axes — an L-infinity norm, not an L2
norm over the three axes together. This is normative. Per-axis is what the
claim means, it is how the equal-axis check already measures, and it is the
same dimensionless relative quantity the scalar common-factor check compares,
so the input band and the postcondition are directly commensurable.

**How the common-factor band relates to it.** The common-factor band is the
normative *input* contract, and the unit-scale postcondition bound is *derived*
from it rather than declared independently. For an affected node `i` with
observed rest-world factor `s_i`, a domain common factor `s_0`, and a declared
factor `s_declared`, the candidate's composed scale on axis `k` of node `i` is
`axis_ik / s_declared`. Three independently bounded relative comparisons stand
between those two numbers, and an affected chain composes all three:

- the declared-factor match binds `s_0` to `s_declared` within `c = 1e-5`;
- the mixed-factor check binds every other affected node's `s_i` to `s_0`
  within the same `c`; and
- the equal-axis check binds each individual axis length `axis_ik` to `s_i`
  within the same `c`.

The third band is the easiest to lose sight of and cannot be dropped: `s_i` is
the *average* of node `i`'s three rest-world axis lengths, while the
postcondition measures an individual axis, so an axis is permitted a further
band away from the very number the mixed-factor check compared. A rig whose
root is off-factor, whose leaf is off-common-factor, and whose leaf axes are
non-uniform within the equal-axis band loads all three at once.

Each comparison is stated relative to `max` of its operands, so each
contributes at most `c / (1 - c)` when re-expressed relative to the smaller
one, and the composed worst case is `(1 - c)^-3 - 1 = 3.00006e-5`. The policy
therefore sets the postcondition bound to four common-factor bands — `4e-5` —
rounded up to the next power of two, `2^-14 = 6.103515625e-5`. Three of those
bands are the analytic composition above; the fourth is headroom for the `f32`
world-matrix composition and decomposition that produces the measured composed
scale. `2^-14` is also `512 * 2^-23`, and so lies on the binary32 mantissa grid
the measurement lives on, which is what makes the inclusive "at most" above
reachable rather than vacuous for this obligation.

Three bands rounded up (`2^-15 = 3.0517578125e-5`) would leave that analytic
worst case only four binary32 ulps of room — `5.17e-7`, or `4.34 * 2^-23`.
That is a rounding artefact, not headroom, and it would make the reserved
float-headroom band above a claim rather than a fact. The fourth band buys the
claim honestly, and it does not blunt the obligation: every build defect this
postcondition exists to catch — a dropped rebase, a factor applied twice, a
stale no-op candidate — is at least `1e-3`, so `6.1e-5` still leaves better
than a `16x` detection margin.

**No floor on the comparison base.** The common-factor and mixed-factor
comparisons are `abs(a - b) <= c * max(abs(a), abs(b))`, with nothing else
inside the `max`. Flooring that base — at `1.0`, at the `1e-6` absolute scalar
term, or at anything else — turns the band into an absolute tolerance below the
floor, which is a *relative* band of `floor * c / abs(s)` that widens without
limit as the declared factor shrinks. It therefore breaks the closure property
below `floor * c / 2^-14`; at a `1e-6` floor that is
`1e-11 * 16384 = 1.6384e-7` (the crossing point is *inversely* proportional to
the postcondition bound, so the `2^-15` bound an earlier revision declared put
it at twice that, `3.2768e-7`), and at a declared factor of `1e-9` such a band
admits `1e-2` relative error, `1000x` the declared policy. The degenerate
`a == b == 0` case needs no floor either: it compares `0 <= 0`.

What the derivation guarantees is the closure property: **any plan the
common-factor band accepts yields a candidate whose every affected node
satisfies the unit-scale postcondition** — at every declared factor the policy
admits, not merely near unit magnitude — with a full band to spare
(`6.1035e-5 - 3.00006e-5 = 3.1035e-5`). The guarantee is stated for the single
closed affected domain this record admits — one common factor, one declared
factor, hence exactly the three composed bands above. Admitting a second
independently classified factor in one plan would compose a fourth band and
require a new policy identity, not a wider bound at this one.

**Sampling budget.** The policy also bounds the total sampled proof work a
document may demand, as `sample_time_count * per_sample_work_units` against a
budget of `4e8` work units. The per-sample cost is every pass the sampled
obligations actually make, and each is charged once per document *side*, since
proof walks the source and the candidate:

```text
2 * bone_count
  + sum over affected skinned instances of
        [skin_and_bounds] *
          (3 * len(skin_joints)
           + 2 * (vertices over every primitive of its mesh))
```

Every sampled obligation poses both skeletons, hence the bone term. Only the
*source* bone count is measured, which is sound because proof rejects a
candidate whose bone count differs (`bone_count_mismatch`) before it charges
anything: the candidate document is caller-supplied, so an unchecked candidate
skeleton is proof work nothing has billed. The slot term is charged explicitly,
per instance: nothing bounds the instance count, and nothing bounds
`skin_joints`, which may repeat a joint, so slot work bears no relation to the
bone count and must not be folded into it. The vertex term charges each vertex
once, while the `f32`-rounding stage of the skinning walk runs once per non-zero
influence. A four-influence vertex therefore performs that stage four times for
the same vertex charge; the budget is the fixed released work proxy calibrated
for the documented populations, not an exact instruction counter.
`sample_time_count` counts key times
*and* cubic-segment interior times, because both are evaluated. Exceeding the
budget is a typed refusal raised before the first sample time is evaluated;
proof never silently samples a subset. The comparison is `>`, so the budget is
a ceiling a document may reach, matching the inclusive "at most" every other
policy quantity is stated with. The budget is part of the policy identity for
the same reason every tolerance is: it is recorded in evidence, and two
evidence records carrying the same policy id must describe the same amount of
checking.

The calibration notes record the historical timing shapes and measurements
that established this charge as a linear proxy for real work. Those seconds are
machine observations, not a normative wall-time guarantee.

The value has to clear real assets as well as bound bad ones: a 200-bone rig
with a 100k-vertex skinned mesh costs `201_000` units per sample time, so a
30-second clip at 30 fps costs `180_900_000`. A budget that refuses that
refuses a plausible production asset, with no way for a caller to opt into the
work.

Historical implementation cost and baseline comparisons are retained in the
[scale proof calibration notes](docs/scale-calibration.md), not in this
normative work-budget definition.

### D.2 Algebra and rewrite rules

Let `L_i(t)` and `W_i(t)` be node `i`'s local and composed world matrices, `G`
be a skinned primitive's geometry-bind-to-world matrix, and `B_i` be its
inverse bind. Valid input satisfies

```text
W_i(rest) * B_i = G
```

for every joint slot. Matrices below act on column vectors.

#### Whole-document linear-unit conversion

For the caller-declared factor `q`, define `U = scale(q)`. Coordinates change
basis as `M' = U M U^-1`. For a uniform `U`, this multiplies an affine matrix's
translation by `q` while leaving its dimensionless linear part unchanged.
Therefore:

- multiply every node-local rest translation by `q`;
- multiply every animation translation value and both glTF cubic-spline
  translation tangents by `q`; key times remain seconds;
- multiply mesh `POSITION` values and morph `POSITION` deltas by `q`;
- conjugate every per-skin inverse bind as `B_i' = U B_i U^-1`;
- multiply every supported camera, light, collision, or extension length by
  `q` through a field-specific handler; and
- leave rotations, normals under positive uniform conversion, UVs, weights,
  times, morph weights, and rest/animation scale channels unchanged.

glTF `LINEAR` and `STEP` samplers have values only; they do not store tangent
elements to rewrite. “Translation tangents” throughout this record therefore
means the explicit in/out elements of a `CUBICSPLINE` translation sampler.

Root-motion positions and velocities are not separate stored fields: they are
recomputed from the converted translation tracks. Distances and velocities
therefore change by `q`; durations do not. Animation scale is dimensionless
and is never multiplied as though it were a distance.

This operation preserves topology and semantics while intentionally producing
world positions `q` times the source positions. It is not a repair for an
already-correct mesh with a compensating skeleton scale.

#### Rest/bind hierarchy reparameterization

For the initial supported class, every affected joint has a rest-world linear
part within tolerance of `s R_i`, where `s > 0` is one common factor and `R_i`
is a proper rotation. The affected domain is a closed connected hierarchy:
the scaled ancestor, all selected skin joints and paths between them, and all
descendant nodes whose attachment transform would otherwise inherit `s`.
A strict source-node connector with no normalized bone may occur on a path
between two projected nodes when its only modeled contribution is its authored
static local rest, which must be finite and affine. Planning composes that
local in source order; the connector does not become an affected normalized
node, its authored local is preserved exactly, and it owns exact-preservation
and projected-successor bridge obligations rather than a numeric residual.
The selected root and every selected skin joint must still project to
normalized bones. An
unprojected descendant or attachment that would itself need rewriting, a
second effective factor, or an instance outside the closure still rejects the
plan.

The compiled rest/bind plan records this source topology once, keyed by raw
source-node identity. A projected row also records its normalized `BoneId`,
authoritative raw parent, nearest projected parent, role, and the incoming
connector tail from which the ordered span is derived. A connector topology
row records its source and parent identities; field rows state its exact
preservation, and the projected successor identifies ownership of the bridged
rewrite. Derived indexes may accelerate lookup, but they are not additional
topology authorities. This distinction matters to embedders: source-node
indices select and preserve raw structure, while `BoneId` identifies
normalized rewrite and residual domains.
The same global table also retains `OutsideDomain` rows, including every
source row for whole-document conversion, so raw parent/projection identity is
never inferred from absence.

For affected node `i`, define the constant basis correction
`C_i = scale(1 / s_i)`, where the initial contract admits `s_i = s` inside the
domain and `s_i = 1` at its parent boundary. The desired world matrix is

```text
W_i'(t) = W_i(t) * C_i
```

and the corresponding local matrix is

```text
L_i'(t) = C_parent^-1 * L_i(t) * C_i .
```

The same rule remains valid across a preserved static connector, but it does
not reduce to skipping that connector or blindly applying the projected
parent's scalar to the next raw local. Let `H` be the ordered product of
unprojected connector locals between projected parent `p` and projected node
`i`, and let `L_i` be `i`'s authored local after that connector span. Since
`H` is preserved, the projected successor's authored local must become

```text
L_i' = H^-1 * C_p^-1 * H * L_i * C_i .
```

Then `W_p' * H * L_i' = W_p * H * L_i * C_i`, as required. A translated
connector makes the conjugation's additive translation term observable, so a
non-identity connector fixture is required; an identity-only fixture cannot
distinguish composition from omission. This is a static-rest rule only:
`SourceNodeAsset` carries no raw animation channel for an unprojected row, so
support for a frontend that drops an animated connector requires a separate
source-animation inventory and rewrite contract.

Because every admitted `C` is a positive uniform scale, the result remains
representable as TRS. On a direct projected edge (`H = I`) — including the
normalized `Bone` local into which a loader has already folded a connector —
the node-local translation is multiplied by the parent's `s_parent`, rotation
is unchanged, and local scale is multiplied by the dimensionless ratio
`s_parent / s_i`. For a raw projected successor after a preserved connector,
the linear part receives that same ratio but its translation is instead
determined by `H^-1 * C_p^-1 * H * L_i * C_i`; a translated `H` contributes
an additive term, so the simple translation multiplier must not be applied to
that raw local. The connector product, inverse, additive term, complete
bridged translation sum, and bridged linear ratio application are evaluated
in `f64`, then each final source-local component is narrowed once to the
`f32` model boundary. Direct projected edges retain their established `f32`
arithmetic and association. Planning likewise keeps each admitted connector
span widened until its projected endpoint, so a built candidate that retains
Complete source projection remains a consumable factor-one planner input even
when the preserved connector-only product exceeds `f32`. Translation
animation values and both cubic translation
tangents on normalized nodes receive the existing parent-basis multiplier.
Scale animation on those normalized nodes is the same local-scale component
over time, so every stored VEC3 scale element receives `s_parent / s_i` too:
the selected closure root receives `1 / s`, a strict affected descendant
receives `s / s = 1`, and an unaffected node receives `1`. This is a topology
rule, not a constant-track predicate: `LINEAR`, `STEP`, and every value plus
in/out tangent of `CUBICSPLINE` are rebased. Rotation tracks and key times
remain unchanged.

A node on a direct projected edge that declares `matrix` rather than TRS
members takes the same rule in component form. Writing `L_i'` out in glTF's
column-major order, the nine linear entries (`0,1,2, 4,5,6, 8,9,10`) are
multiplied by `s_parent / s_i` and the three translation entries
(`12, 13, 14`) by `s_parent`. Components
`3, 7, 11, 15` are written back exactly as authored, which is the correct
rebase only because the §D.4 gate has already proved them exactly
`(0, 0, 0, 1)`: the basis change multiplies `3, 7, 11` by `1 / s_i`, a no-op
on an exact zero and on nothing else, and leaves `15` alone at any value, so
what the gate buys there is not the arithmetic but the premise — a `matrix`
is the node's whole transform, with no projective divide this record models,
only when `15` is exactly one. §D.3 case 4 records why that gate compares
exactly rather than within tolerance. A raw projected successor after a
preserved connector is governed by the full `H` product above, not this
component shortcut.

Mesh positions, morph deltas, and normals remain unchanged because their world
geometry is already correct. Each inverse bind is regenerated from the
unchanged geometry bind:

```text
B_i' = inverse(W_i'(rest)) * G = C_i^-1 * B_i .
```

Consequently `W_i'(t) * B_i' = W_i(t) * B_i`: the complete skin palette,
not merely joint origins, is analytically preserved.

Both bone convenience inverse binds and every per-instance skin-slot array must
be updated; the single bone-level value is never authority for a multi-skin
document. A raw format has one bind store per skin and no per-node bind, so the
frontend rewrites each affected skin's stored matrices with per-slot factors and
proves through the reloaded document that every per-instance array and bone
convenience value is correspondingly correct. The output deliberately changes
the affected nodes' composed scale to one while preserving their world
translations and orientations, animation trajectories, skinned vertices, and
bounds.

**Conflicting raw accessor claims reject.** Unlike whole-document conversion,
whose converted uses all take the same `q`, rest/bind derives a typed claim for
**every** raw accessor use, not only a scale-bearing write. A claim records the
accessor's expected component shape and its multiplier. Translation outputs,
scale outputs, and inverse-bind slots carry their topology- or slot-derived
multiplier. Factor-one preservation claims cover mesh attributes and indices,
sampler inputs, rotation and weights outputs, and sampler outputs that no
channel selects. The latter are still sampler-referenced payloads, not unused
bytes that the raw proof may overlook.

One accessor index must have compatible component shape and one multiplier
across all of those claims. Thus a translation or scale output reached from the
closure root and from a child, from an affected and unaffected node, or an
accessor used both as mesh `POSITION` and a translation or scale output can
reject; so can a same-index type conflict even when both factors are one. One
inverse-bind store shared by skins whose joints straddle the closure is the
same problem. Splitting the payload is not a remedy, because it changes array
lengths and destroys the identities the proof pins.

Distinct accessor indices are checked too: if their resolved byte ranges
overlap and either claim rewrites bytes, the operation refuses rather than
silently corrupt a preserved neighbour. Direct-image `bufferView` consumers
are not accessors and have their own separate image-overlap refusal. Within
the source domain admitted by the common conservative capability preflight,
uses whose claims agree impose no additional rest/bind restriction however
many reach one accessor. The preflight can still refuse a
scale-bearing/dimensionless alias or overlapping raw range before this ledger
runs. A declared factor of one makes every rest/bind multiplier one, so this
ledger's factor disagreement cannot fire on a no-op; it does not broaden the
preflight's supported raw domain.

### D.3 Worked synthetic cases

The implementation fixtures use literal analytic values rather than values
generated by the transform under test.

1. **Unit skeleton.** A root and child both have unit scale, the child rest
   translation is `(0, 1, 0)`, and the IBM is rigid. Reparameterization with
   an expected factor of one is a deterministic no-op. Requesting any other
   factor rejects as a source-fact mismatch. A whole-document conversion by
   `0.01` instead produces a `0.01 m` child offset and mesh while leaving scale
   channels at one.
2. **Compensated inherited scale.** A root has scale `0.01`; its child has
   local translation `(0, 100, 0)` and local scale one. Their composed child
   position is `(0, 1, 0)`, every selected joint has effective scale `0.01`,
   and the corresponding IBM linear columns have magnitude 100. The
   reparameterized root and child have unit composed scale, the child local
   translation is `(0, 1, 0)`, the IBM is rigid, and mesh positions and the
   child world position stay byte-for-byte or tolerance-equivalent as defined
   by the proof. A transform-only child at local offset `(1, 0, 0)` is rebased
   to `(0.01, 0, 0)`, preserving its world origin while its composed linear
   scale changes from `0.01` to one. Transforming a further off-origin point
   through that child's complete affine distinguishes the result from a no-op.
   Checking only `joint_translation + joint_rotation * offset` is invalid
   because it drops the very scale channel the fixture must prove. Unskinned
   geometry in the affected closure is initially rejected rather than silently
   resized; supporting it would require a separate geometry and instancing
   rule.
3. **Separate clips.** A compatible clip declares the same named parent
   topology, rest orientations, helper layout, and `0.01` translation basis;
   its child translation value and cubic tangents are rebased by `0.01` before
   remapping. A name-identical clip with a unit parent basis, different parent
   rotation, different helper layout, or a mixed factor fails compatibility
   before any keys are copied.
4. **Rejected affines.** Independent fixtures introduce `(0.01, 0.02, 0.01)`
   scale, a shear term, a negative determinant, a zero/near-zero determinant,
   `NaN`/infinity, and one joint with an effective factor different from the
   common factor. Each must produce a stable typed rejection, no artifact, and
   no partial evidence publication. Two kinds of near-valid noise are treated
   differently, by design. Values that reach the tolerance-bearing affine
   classification — a `100.000015` IBM column, or a rest scale axis skewed
   within `equal_axis` — are accepted when their measured residual is inside
   the declared tolerance, and that residual is recorded. Structural facts
   checked at the §D.4 capability gate are compared **exactly**: a node
   `matrix` whose last row is not exactly `(0, 0, 0, 1)` is a
   `NonAffineNodeMatrix` refusal, so `matrix[15] = 1.0000001` is rejected
   before any tolerance is consulted. The exactness is deliberate on three
   grounds: it matches glTF-Validator, whose `isTrsDecomposable`
   (`lib/src/utils.dart`) tests those same four components with exact `!=`
   before it applies any tolerance and reports `NODE_MATRIX_NON_TRS`;
   composing two affine matrices produces an exactly zero bottom row in
   floating point (`0*x + 0*y + 0*z + 1*0`), so no real composition pipeline
   emits noise there; and the §D.2 rewrite rules *rely* on it — the
   reparameterization's node-`matrix` rebase writes components
   `3, 7, 11, 15` back as authored, so a tolerated `matrix[3] = 1e-12` would
   be published unconverted where the basis change owed it `1 / s_i`, and a
   tolerated `matrix[15] = 1.0000001` would be rebased as though the node
   were affine. Relaxing it is a change to the §D.4 contract, not a tolerance
   widening; §D.1's ban on exact float equality binds the comparisons that
   tolerance policy governs — classification and proof — not this gate.

### D.4 Modeled domains and preservation coverage

The transform plan owns an explicit capability manifest. A field absent from
`Document` is not evidence that the source lacks that field; preflight must
inspect the raw input or a loader-supplied complete inventory before mutation.
The plan compiles every modeled container into a typed field ledger. Each row
identifies the target and component, then states either a structural semantic
rewrite kind or `PreserveExact`; absence is not used to mean preservation. A
row does not carry a resolved factor, expected value, or connector product:
candidate construction and proof independently derive those numbers from the
operation and validated topology. `PreserveExact` is typed builder write-set
ownership: the core builder does not write that field. It is not, by itself, a
bit-equality rule for normalized derived state presented to `prove_scale`.
Container rows carry the counts and shapes needed to describe every element
without storing one row per vertex, key, tangent, or matrix component. Candidate
construction consumes this ledger, so each authoritative modeled production
write discharges one rewrite row and every rewrite row must be discharged.
Whole-document conversion retains one compatibility-only exception: raw locals
kept beside `Unavailable` source coverage are still converted best-effort, as
before, but that coverage explicitly makes their identities non-authoritative,
so they produce neither ledger rows nor proof/replay claims. Factor-one rows remain
explicit where identity, aliasing, and write-set coverage depend on them.
Separate typed payload-shape rows retain document, clip, track, instance, mesh,
and primitive identity and cardinality, including empty containers, without
freezing key times or other numeric values. Exact replay therefore detects
added, removed, reordered, or retargeted payload that happens not to add a
field row or obligation.

| Domain | Current model boundary | Unit conversion | Reparameterization |
|---|---|---|---|
| Rest hierarchy | `Bone::rest`; source-node TRS/matrix and parent topology form a canonical typed source table | Scale translations; conjugate retained matrices; preserve topology explicitly | Rebase affected normalized/projected locals; exactly preserve typed static-connector rows; bridge only the projected successor; preserve orientations and topology explicitly |
| Translation animation | Values and cubic tangent triplets are retained for normalized nodes | Multiply values and both tangents by `q` | Multiply by the affected parent-basis factor; infer no raw animation support for an unprojected connector |
| Rotation and scale animation | Retained as dimensionless values | Leave unchanged | Rotation unchanged; rebase each scale VEC3 by `s_parent / s_i`, including cubic tangents |
| Root motion and velocity | Derived from translation tracks | Convert tracks, then recompute | Preserve sampled trajectory and derived velocity |
| Base mesh geometry | Base `POSITION` and normals are retained | Scale positions; normals unchanged | Leave positions and normals unchanged |
| Morphs | Not retained by the normalized core; raw glTF ownership inventories target accessor identities and weight locations | Raw glTF only: scale dense `f32` `POSITION` deltas once per unique accessor; preserve static JSON weights as numeric values and animated weight accessor payloads byte-exactly; reject other semantics and unsafe aliases/layouts | Reject: rest/bind morph preservation is not proven |
| Skin binds | Per-instance IBMs plus a lossy bone convenience value | Conjugate every per-skin matrix | Regenerate every per-skin matrix from output joints and unchanged `G` |
| Cameras/lights | Node transform only; typed fields are not modeled | Reject until all length fields have handlers | Reject when attached to the affected domain until preservation is proven |
| Collision/custom data | No semantic model for extras or extensions | Reject unless a registered handler covers every length | Reject when affected unless exact preservation is proven |
| Other vertex/source data | Several attributes, non-triangle modes, and extension payloads are not writer-preserved | Reject on the normalized-model route | Reject on the normalized-model route |
| Out-of-contract node transforms | A `matrix` beside a TRS member, or a `matrix` whose last row is not **exactly** `(0, 0, 0, 1)` — compared exactly at the gate, never within tolerance (§D.3 case 4) — parses but is not glTF 2.0 | Reject: which transform the author meant is unknowable, and `U M U^-1` leaves `3, 7, 11` unconverted where the basis change owes them `1 / q`, while a `15` other than one is invariant under the rewrite and so would be published still asserting a projective divide (§D.2) | Reject for the same reason, with `1 / s_i` for `1 / q` |
| Animation targeting a matrix node | glTF 2.0 requires an animated node to use TRS; a typed reader can otherwise decompose `matrix` and lose that source distinction | Reject at raw preflight | Reject at raw preflight |
| Shared raw accessor payloads | Every accessor use has a component-shape and typed field disposition; preserved uses remain explicit factor-one claims | Convert once per unique accessor: every converted use takes `q` | Reject same-index type/factor conflicts and overlapping distinct accessors when either range is rewritten (§D.2) |
| Unreferenced accessor payloads | Retained only in the raw source container, including dense bases and sparse index/value payloads | Preserve exactly; reject when any owned byte span overlaps a rewritten accessor | Preserve exactly; reject when any owned byte span overlaps a rewritten accessor |
| Image payload aliases | An `image` reads a `bufferView` directly and never becomes an accessor | Reject when its bytes overlap a rewritten accessor | Reject when its bytes overlap a rewritten accessor |

The connector exception is deliberately narrower than general helper-node
support. The source projection can attest an unprojected row's identity,
parent, and authored local rest, but not a raw animation channel targeting
that row. A frontend that omits such a channel from the normalized model has
not preserved it and cannot cite the static connector rule as evidence that
it is safe to rewrite.

The current glTF writer rebuilds nodes as TRS, emits only modeled triangle
attributes, creates skin/holder structures, and does not preserve arbitrary
source JSON. The FBX loader mechanically inventories every current row above,
including the unverifiable unreferenced-accessor analogue, and exposes a
documented normalized ufbx source-node/source-skin sidecar when every declared
joint slot is representable. A missing cluster bone downgrades the generic
sidecar to unavailable, and one unreadable converted bind makes its whole
ordered declaration unreadable with no retained prefix. Complete coverage here
means every adjusted/compensated ufbx node and skin has stable identity and
projected values; it does not relabel those values as raw authored FBX
transform or payload evidence. The inventory also records
that it bakes takes, normalizes coordinate and inheritance semantics, generates
some missing data, truncates/renormalizes influences, counts rejected invalid
or unrepresentable influences, triangulates, welds, and cannot prove raw
payload spans. Consequently neither current
load-`Document`-write route qualifies as a
preservation-proof frontend for these operations without the raw capability
preflight and explicitly bounded writer work. Unknown extensions or extras,
unsupported morph semantics or unsafe morph storage, cameras, lights, collision metadata, non-triangle primitives,
unmodeled vertex attributes, secondary influence payloads, malformed or
missing inverse binds, node transforms outside the glTF 2.0 contract, and image
payloads sharing bytes with a rewritten accessor fail closed. The FBX
inventory is evidence of why the current frontend remains refused, not an
enablement signal. A later FBX producer may enable only an operation whose
complete inventory/write/proof boundary it satisfies; it must state that FBX
curves were baked, not preserved as authored curves.

Source validity is decided once, at the preflight, and not re-derived per
operation. The preflight's byte-disjointness inspection therefore ranges every
accessor, whether or not a mesh, skin, or animation sampler references it, plus
every image view. An unreferenced accessor contributes its dense base when it
has one and the packed spans of its sparse indices and values; those bytes are
preserved source rather than expendable payload. A referenced sparse accessor
is still refused outright before disjointness matters, and extension-defined
consumers are unreachable while every extension is itself a refusal. Every
unreferenced span may sit adjacent to or alias other preserved data, but any
overlap with a rewritten accessor refuses with both owners located. Each
operation keeps its own guard as defence in depth, since the guard is what must
hold if the gate is ever relaxed, but it shares the gate's classifier rather
than re-deriving one.

The core typed ledger is also the semantic authority for both raw glTF scale
writers. Before binding raw fields, the adapter replays the plan's complete
structural inventory against the current normalized document; numeric-only
replay remains valid, while a stale payload, field, topology, or obligation
inventory refuses. One numeric-free glTF binding then projects canonical
source topology and field dispositions onto raw node members, animation
outputs, mesh payloads, and skin slots. The binding independently walks
`/nodes/*/children` and refuses disagreement with canonical topology before
writing bytes. Writer and artifact proof share those validated identities and
component-group names, but independently select components and derive numeric
multipliers; neither consumes the other's expected values, matrices, factor
helpers, or domain model. The core ledger never replaces raw claims over
scenes, materials, default scene, references, unknown payload, aliasing, byte
ranges, or container bytes.

### D.5 Separate-clip compatibility

Exact bone-name matching is necessary but insufficient. Before transforming or
remapping a separately supplied clip, the producer compares a versioned
skeleton-basis record containing:

- named parent topology and target node paths;
- finite local rest matrices, rest orientations, and helper-node layout;
- declared/normalized coordinate convention;
- effective uniform scale at every translation-track target;
- selected reparameterization domain and factor; and
- loader/tool identity plus the exact input-byte digest.

Compatibility requires topology and orientation agreement within their
declared rules and the analytically expected translation-basis relationship.
Different rest translation bases, parent scale, parent rotation, helper layout,
or effective factor reject before remapping. A digest identifies the evidence
record but does not replace tolerance-aware semantic comparison.

Character-assembly recipe/evidence v4 implements this record and comparator.
It captures the exact base and clip bytes once, performs the raw glTF
capability preflight and plan before remapping, and rebases each compatible
clip's translation values and both `CUBICSPLINE` translation tangents in the
source basis. Evidence pins every input digest and basis fingerprint together
with the compatibility result. V1 through v3 remain immutable and v3 rejects
the v4 block as unknown.

### D.6 Proof, evidence, publication, and rollback

The candidate is serialized before it is proved, and the proof runs **once**,
on the document reloaded from the exact artifact bytes — never on a separately
built in-memory candidate, which would prove a document the rewriter did not
emit. That single pass has two layers over the same reload: the
normalized-document layer, which produces the residuals below, and the
artifact layer, which re-derives directly from the container bytes every claim
the normalized model cannot represent. Publication carries the proof to the
file by digest, not by a second proof: the bytes are staged, read back, and
refused unless their SHA-256 equals that of the proved bytes. A second proof
would be redundant, the proof being deterministic over byte-identical input.

The artifact layer's claims are stated over raw source bytes — preservation of
every raw JSON location and every buffer byte outside the rewritten set, array
and index identity, honest reporting of the rewritten locations, container
framing, bounds consistency, and single-step narrowing — and presuppose a
rewriter that edits the source container in place. A frontend without one,
which rebuilds the file from the normalized model as an FBX writer would,
cannot make these claims and is **not** thereby exempt from the obligation
they discharge: that declared-unaffected payloads are unchanged. Such a
frontend owes that obligation by the other route — a complete loader-supplied
inventory of every §D.4 domain — and may enable an operation only over the
domains that inventory covers.

The exact raw-JSON-preservation refusal is diagnostic evidence as well as a
refusal: the current scale-evidence v4 record carries up to the first 16
deterministic differences, each as a pointer and an added/removed/changed
direction, plus the omitted remainder; the full count is the retained length
plus that remainder. The field is null when the exact-preservation walk did
not supply locations for a refusal; it is not an alternate spelling of the
capability preflight's violations.

For the glTF producers, that raw complement explicitly includes scene
membership, default-scene selection, materials, and their references: neither
scale operation owns a write location in those payloads, so the artifact layer
proves their raw JSON values exactly unchanged. The normalized core proof does
not also deep-compare `assets.scenes` or `assets.materials`; those model values
cannot replace the raw fields and references the container proof owns. A
rebuilding frontend therefore cannot cite the core proof for those domains
and must discharge them through the complete inventory route above before
enabling an operation.

The proof evaluates rest, every key time, and analytic or sufficiently bounded
interior times for cubic segments. For reparameterization it must prove,
within versioned tolerances:

- equal world joint translations and orientations before/after;
- equal sampled animated trajectories, root motion, and derived velocities;
- equal skinned vertex positions and bounds for analytic one-joint and
  multi-joint fixtures plus deterministic sampled production evidence;
- unchanged mesh/material/skin identity and declared unaffected payloads —
  including that each candidate mesh instance draws the same mesh, binds the
  same skin joints, hangs off the same node, and names the same source node as
  its source counterpart, none of which either operation rewrites, and that a
  skin *outside* the affected closure keeps each slot's effective inverse bind.
  Identity here covers *placement*, not only what an instance is: the node is
  what positions an unskinned prop, and the source node is what resolves the
  skin whose absent bind accessor stands for the format-defined identity
  default. Both are compared positionally, so the instance list must also
  agree in length and order. Candidate proof also compares normalized parent
  topology and source-projection coverage exactly for every operation. When
  coverage is `Complete`, the raw-node parent/bone map is exact too; rows under
  `Unavailable` coverage are non-authoritative, produce no raw topology or
  field rows, and are not compared. Neither
  operation rewrites any authoritative identity. For rest/bind, the compiled
  ledger includes an `ExactUnchangedWorldRest` obligation for every bone in the
  exact unaffected complement and proof compares each complete composed
  world-rest affine exactly. The discriminating case is an
  independent sibling or leaf outside the closure's ancestry; an unaffected
  ancestor was already constrained transitively by an affected descendant's
  world-rest residual. This is an exact semantic placement invariant, not a
  tolerance or a claim that equal matrices prove every stored local field was
  retained; exact local ownership is a separate field-ledger obligation.
  Whole-document conversion has no unaffected complement, but its typed global
  topology obligation remains in force. Before either operation builds or
  proves, it recompiles the supplied source and requires exact equality of the
  operation variant, canonical source topology, payload and instance
  inventory, field dispositions, and derived obligation rows. Added, removed,
  reordered, or retargeted positional payloads (including stable-order source
  skins, attachments, tracks, meshes, instances, slots, or evidence) therefore
  make the plan stale even when the former aggregate boolean inventory would
  have been unchanged. Canonical source-node and connector topology is keyed
  by raw source identity, so merely reordering its backing rows is not a
  topology change. Numeric values may still change when that exact inventory
  and ledger remain equal; replay does not freeze the planned source snapshot.
  The shared input/candidate checks, source-skin structural rows, canonical
  source topology and rest/bind domain derivation, world-pose and inverse-bind
  readers, and one-time affected-skin classification live in
  `crates/animsmith-core/src/scale/validation.rs`. Operation selection, both
  planners, typed field/payload/obligation ledger compilation, clip-evidence
  declaration, and complete numeric-value-free replay live in private
  `crates/animsmith-core/src/scale/planning.rs`. Planning consumes the shared
  validation/topology boundary and remains the single owner of the compiled
  plan vocabulary; validation does not depend back on planning or reconstruct
  a second plan vocabulary. The opaque candidate wrapper, feature-gated
  analytic reference builder, and writer-owned direct/connector source-local
  rebase arithmetic live in private
  `crates/animsmith-core/src/scale/reference.rs`. That module consumes the
  compiled plan and shared validation/numeric leaves; proof derives its own
  connector products, expected source locals, and scale-track boundary rather
  than importing the reference writer's expectations. Those proof-owned
  derivations, exact field discharge, sampled work and budget accounting,
  semantic residuals, and skin/bounds checks live in private
  `crates/animsmith-core/src/scale/proof.rs`. Its paired residual recorder is
  nested under `proof::residual`, so only proof can mutate a residual maximum
  and its comparison count; the public read-only values continue to resolve
  through the `animsmith_core::scale` facade.
  For an admitted connector span, typed obligations require every connector
  local to remain bit-exact and independently derive the projected successor's
  expected bridged source local from canonical source topology. The connector
  owns no normalized residual, but a writer and proof cannot agree on a wrong
  connector rebase by sharing the writer's product calculation. Rewritten
  direct rewritten source-local components compare under the `appendix-d-v6`
  scalar tolerance, rather than bit equality: a raw adapter may apply the same
  factor before its one `f32` narrowing while the normalized source view was
  already narrowed. Preserved source-local components retain their exact-bit
  obligation. A connector-bridged projected successor also remains bit-exact:
  that core-only connector path does not cross a raw frontend narrowing
  boundary, and its exact connector-projection contract is unchanged.
  Operation-fixed rewrite and tolerance policy fields are not source
  inventory. Numeric affine/factor classification is not repeated there, so
  proof's normalized-skeleton witness remains independent of planning's
  raw-projection witness; a structurally stale plan still cannot omit a newly
  added bone, connector, or payload from every proof walk. The
  unaffected-bind comparison is over each slot's effective bind, in the
  resolution order the model defines: per-instance array, then the bone
  convenience value, then the format-defined identity only when a complete
  attached source skin declares its inverse-bind accessor `Absent`. When both
  sides resolve, the effective matrices are compared even if their stored
  representations differ. Exactly one side resolving is missing proof
  evidence; neither side resolving is reported as out of the proof's scope,
  never as proven — a document carrying an unrelated skin with no bind
  evidence and no licensed identity default is not an operation this record
  touches, and must not be refused for lacking evidence about it;
- unit composed scale for every affected node, measured per axis against the
  derived bound of §D.1; and
- the analytically expected full world affine of a transform-only attached
  child: preserved origin/orientation and unit linear scale, so a no-op cannot
  pass.

For whole-document conversion, the corresponding length facts must differ by
exactly the declared factor within tolerance while dimensionless facts remain
equal. The raw glTF artifact proof independently walks admitted morph
`POSITION` accessors, requires one conversion per unique accessor (including
base/morph and cross-target aliases), compares every static JSON morph-weight
numeric value, and compares every animated weight accessor byte exactly. Both
operations prove finite output, the skin equation, deterministic artifact
bytes, and deterministic evidence bytes.

**Every obligation above is derived from the evidence it needs.** The typed
ledger contains a row only when the validated planned inventory carries the
payload that row reads — a transform-only attachment in the closure for a
full-affine probe, an affected translation track for key-time checks, an
affected cubic segment for interior samples, any affected track for sampled
trajectories, a skinned instance touching the closure for one shared skinned
evidence group, and a non-empty affected closure for the inseparable
rest/unit-scale group. Skin-matrix and bounds results remain separately
reported projections of that one skinned group; neither can be independently
armed, and unit scale cannot exist without the affected-rest obligation.
A row whose evidence disappears from a replayed source changes the recompiled
ledger and yields typed `PlanDocumentMismatch`, never a zero residual. A
counterpart or value missing inside an inventory-matched walk is typed
`MissingProofEvidence`. The residuals *are* the evidence record, so a record
stating "residual 0.0" for a claim nothing checked is false rather than
missing. This matters most for the transform-only obligation, which is the
claim ensuring a no-op cannot pass — a guarantee an empty probe loop reporting
zero would not provide.

Per-element track values, base mesh positions, and inverse binds outside the
affected closure are ordinary typed obligations rather than three special
ungated loops. Their rows are derived for every corresponding planned
container. For normalized `Bone::rest`, inverse binds, tracks, and meshes,
proof discharges ownership through the existing versioned residual or exact
semantic obligations, independently deriving rewritten expectations from the
operation. A normalized candidate may therefore be non-bit-identical while
remaining inside the declared tolerance; field-row ownership neither replaces
nor tightens that accepted set. Rewritten and preserved track/mesh comparisons
retain their released maxima and counts. Core bit equality applies only to
authored `SourceNodeRest` locals that every admitted producer copies, including
preserved connector locals. Exact structural obligations such as topology,
connector projection, and unchanged world rest own no numeric residual. Raw
byte-exact preservation of other fields — including signed zero and quaternion
sign — remains the artifact proof's PR-B responsibility. No separately mutable
boolean controls any of these walks.

Proof cost is bounded, not merely expected to be small. Source and candidate
world matrices are derived once per sample time and shared across the
trajectory, skin, and bounds obligations, and the skin and bounds obligations
share one walk over the affected instances and their vertices. The remaining
work is `sample_time_count * per_sample_work_units` with the per-sample cost
spelled out in §D.1 — bones, skin slots, and skinned vertices, each charged
once per document side — which the tolerance policy of §D.1 caps. A document
above that cap is refused with a
typed error naming the policy identity, the sample count, the per-sample cost,
the computed work, and the budget. It is never proved against a truncated
sample set: a bounded proof of a subset would publish evidence for claims it
did not check. Because the budget lives in the policy identity, it is not a
per-run flag, and the producer's fixed-policy rule below covers it.

The current producer emits immutable scale-evidence v4 containing
the operation kind, declared and observed factors, tolerance policy and
residuals, affected node/skin identities, raw capability manifest, input/output
byte digests and counts, proof sample coverage/results, tool identity, and
rejection reason. It is written as an
atomic artifact/evidence publication pair. Failure leaves no new output; an
existing pair is restored. The original input is retained so rollback is
selecting the prior artifact, not attempting an inverse float rewrite.

**Two observed factors, and the divergence between them.** For rest/bind
reparameterization, "observed factor" names two independent witnesses of the
same quantity, measured from deliberately different state: planning classifies
the raw source projection's node-local rests composed through the raw parent
chain, and proof reads the normalized skeleton's bone rests composed through
its own parent chain. For whole-document conversion, both fields are the
declared factor by definition and their divergence is exactly zero; that
operation does not classify scale from either hierarchy.
The raw composition includes every preserved static connector local. The
normalized witness reads the corresponding folded transform through the next
projected bone; the connector itself has no normalized identity and therefore
owns no residual obligation. After semantic residuals succeed, proof's typed
field-row discharge separately checks exact connector preservation and the
analytic bridged source local at each projected successor. This preserves the
released residual-error precedence while keeping both the connector's
preserved authored components and its bridged projected successor bit-exact.
Direct rewritten raw rows outside a connector bridge use the published scalar
tolerance at the frontend narrowing boundary described in the compiled-ledger
contract above.
Strict scale-input shape validation requires the two chains to *agree* — under
`Complete` source-skeleton coverage, which this operation requires anyway, a
projection that contradicts its own skeleton is refused before either witness
is taken — but nothing reconciles the two *readings*: each is still composed
from its own stored state, and that independence is the reason both witnesses
exist. The record therefore carries **both**, plus the relative divergence
`|planned - proved| / max(|planned|, |proved|)` between them explicitly, so a
consumer neither mistakes one witness for the other nor has to derive the
relationship from two separate policy fields.

The expected ceiling on that divergence is **the sum of two bands the policy
already declares**: the common-factor band plus the postcondition unit-scale
residual (`1e-5 + 2^-14 = 7.103515625e-5` under `appendix-d-v6`). Planning
binds its witness to the declared factor within the first band or refuses;
and for a candidate this operation built from the source under proof, that
candidate's composed root scale is the proof witness divided by the declared
factor, so the unit-scale postcondition binds the proof witness to the
declared factor within the second. The two bands are not stated the same way,
and the sum is a ceiling only up to that difference: planning's is relative to
the `max` of its two operands, exactly as the divergence is, while the
postcondition's is an absolute L∞ deviation from `1` on the *candidate's*
composed scale — so it bounds `|proved − declared|` as a fraction of the
declared factor rather than as a fraction of `max(planned, proved)`.

The divergence is **reported, not enforced**. The second step above holds for
a candidate this operation built from the source it is proved against — which
proof deliberately does not require — and it costs the binary32 rounding of
the rebase on the way, so the sum is the ceiling the design guarantees rather
than a bound proved to the last ulp; refusing at it would refuse documents
whose two witnesses each honour their own band. A divergence beyond it is not
evidence that the two parent chains disagree — those are validated to agree,
above — but that the state each witness reads differs: most often the raw
node-local rests against the normalized bone rests, two separately stored
descriptions of one rest pose. How far apart those are is a fact about the
input, not a residual this proof owns. Recording both witnesses and
their divergence introduces no band of its own and no new policy identity.

Migration is opt-in. `assemble.canonicalize_skin` in recipe v3 remains the
existing unanimated bind-geometry operation with identity source-to-metre
conversion; it does not gain rest-scale rewriting, and old recipes are not
silently reinterpreted. Recipe/evidence v4 adds the optional, explicit
`rest_bind_scale` operation and basis-compatibility evidence. Consumers that invert scaled
inverse binds with the rigid-only shortcut `-A^T t` can observe a change by
`s^2`; that exposes a pre-existing consumer error because the shortcut is valid
only for an orthonormal linear part, not a geometry change made by this
operation.

### D.7 CLI, configuration, and public API

The CLI uses a required operation subcommand, not one flag whose
meaning depends on the input:

```console
animsmith scale whole-document INPUT.glb -o OUTPUT.glb \
  --factor 0.01 --evidence OUTPUT.scale.json

animsmith scale rest-bind INPUT.glb -o OUTPUT.glb \
  --source-skin-index 0 --source-root-node-index 3 \
  --expected-factor 0.01 --evidence OUTPUT.scale.json
```

All numeric and source-identity arguments shown are required; there is no
inferred factor, implicit first skin/root, or in-place mode. Artifact and
evidence paths must be distinct from each other and the input. Initial support
is glTF/GLB only. The command uses the fixed tolerance-policy version recorded
in evidence rather than accepting per-run tolerance flags. A future policy
change requires a new policy identity and compatibility review.

The single-document producer has no `animsmith.toml` key and no separate plan
file: mutation must not become an incidental effect of a lint configuration.
Assembly recipe v4 exposes the same rest/bind operation only through an
optional `[rest_bind_scale]` block whose selectors and factor are all required.
It validates every base and clip basis before applying the operation or
remapping keys. Recipe v3 has no such block and continues to reject it as
unknown.

The public `animsmith-core` shape is a non-exhaustive `ScaleOperation` with
distinct `WholeDocumentLinearUnits { factor }` and
`RestBindUniformScale { source_skin_index, source_root_node_index,
expected_factor }` variants, carried by a `ScaleRequest`. Pure planning returns
either an opaque, non-exhaustive `ScalePlan` (operation variant, affected
closure, canonical topology, typed field rows, tolerance policy, and derived
obligations) or a typed `ScaleError`; proof returns a
`ScaleProof` with one read-only `ScaleProofResidual` per semantic claim. Each
value carries the maximum and number of comparisons together, so a residual
nothing walked is distinguishable from a measured zero (issue #319) and an
adapter cannot combine one claim's count with another claim's maximum (issue
#323).
Planning and proof take format-neutral node, track, bind, and capability facts.
They do not accept paths, glTF/ufbx types, config parsers, or publication
policy. The format frontend owns raw inventory and exact source rewriting; the
CLI owns atomic artifact/evidence publication. Applying a plan builds a
candidate and never mutates the caller's source on failure.

`animsmith-core::scale` implements this contract (issue #281): the
`ScaleOperation`/`ScaleRequest` planning entry point, the typed `ScalePlan`
and `ScaleError`, the no-authority candidate handoff, and paired `ScaleProof`
residual values. Any change to the operation variants, required selectors, or failure
boundary is a design change rather than an implementation detail.

`ScalePlan` exposes its compiled ledger only through non-exhaustive read-only
field, payload-shape, topology, and obligation row views; callers cannot
use those row values to construct or mutate a plan with an incoherent
operation, topology, field inventory, or obligation set. Plan
equality includes that exact structural ledger. The former public
`ScaleDomainRewrites` and `ScaleProofObligations` boolean bags and their plan
accessors are removed in this pre-1.0 API break: operation-fixed facts and
evidence gates have one typed authority rather than independently selectable
booleans. The CLI privately projects the same five `domain_rewrites` booleans
from the operation variant's frozen mapping, so payload presence and
factor-one rows cannot change their meaning. Immutable scale-evidence
v1/v2/v3, `ScaleProof` serialization, `appendix-d-v6`, tolerances, calibration
semantics, and policy pins do not change. The internal ledger is not serialized
into evidence.

`ScalePlan` and `ScaleProof` each carry the observed factor beside the
declared one, as §D.6's evidence contract requires (issue #290). The declared
factor is what the build applies and what every proof expectation is stated
in; the observed factor is the rest-world uniform factor measured at the
scaled root, which the declared-factor band admits without requiring equality.
Proof re-measures it from the source it is handed rather than reading it off
the plan, so evidence does not depend on planning having recorded it. It is
reported, not checked: the input band is planning's obligation and the
postcondition derived from it in §D.1 is proof's. For a whole-document
conversion the observed factor is the declared one, because §D.1 gives that
operation's factor no measurable source counterpart at all.

`animsmith-gltf` owns the glTF/GLB frontend of that contract:
`preflight_scale_source` (issue #280) captures the raw inventory,
`capability_facts` / `rewrite_linear_units` / `prove_rewritten_artifact`
(issue #282) implement the whole-document conversion of §D.2 directly on the
source's own JSON and buffer bytes, and `rewrite_rest_bind` /
`prove_rewritten_rest_bind` (issue #283) implement the rest/bind
reparameterization the same way. Because that route produces a candidate
core did not build, `ScaleCandidate::from_document` exists so the reloaded
artifact can reach `prove_scale`; the type asserts nothing that `prove_scale`
does not independently re-derive. Core exposes no production candidate
builder: its analytic reference constructor is available only through the
non-default `fixtures` feature for tests and calibration.

`rewrite_rest_bind` is the selector-taking convenience boundary; it compiles a
plan and delegates to the same plan-taking raw adapter used by
`rewrite_scale_plan`. The latter validates the plan against the supplied
source before any write, so the CLI can carry one authoritative compiled plan
through rewrite and proof without admitting stale-plan replay.
`GltfScaleArtifact` reports which operation produced it
beside the declared factor, because the two operations rewrite different
domains and a factor alone does not distinguish them. Beyond the shared
plan/proof rejections, the frontend owns three refusals the format-neutral
layer cannot state: the disagreeing-multiplier refusal of §D.2, and two
agreement checks over the raw node hierarchy, the loader's source-node
projection, and the normalized skeleton's parent links. `animsmith-core`
requires the last two of those three to describe the same tree — that is the
document-shape validation of §D.6 — but the raw `/nodes/*/children` arrays
live in JSON that never becomes a `Document` field, so bringing the third
description into the comparison is only possible here.

### D.8 Implementation ownership and deferred integrations

Putting all semantics in assembly was rejected: it would hide a reusable
single-document operation and preserve assembly's currently insufficient
separate-clip proof. A standalone command with unrelated duplicate math was
also rejected. The chosen end state is one shared core transform-plan and proof
layer with distinct, explicit frontends: the shipped single-document producer
and character-assembly recipe/evidence v4 integration. The existing
`canonicalize_skinned_bind_pose` remains the narrower unanimated bind-geometry
foundation; it is not silently widened or renamed into this contract.

**Shared affine classification, distinct acceptance contracts.** The core uses
one typed, symmetric `f64` positive-uniform `Mat3` classifier for both Appendix
D and skinned bind-pose canonicalization. Appendix D retains
`appendix-d-v6`'s `1e-5` equal-axis/orthogonality and `1e-6`
relative-determinant thresholds. The canonicalization input
`source_to_meters_y_up` retains its numeric `1e-4` equal-axis/orthogonality
thresholds and exact-zero determinant threshold; these are compatibility with
the existing assembly/canonicalization contract, not a second scale-policy
version.

Canonicalization does deliberately adopt the shared classifier's semantics:
axis equality is relative to the three-axis average and longer operand rather
than to the first column, derived arithmetic is `f64`, and singularity is
classified before non-uniformity or shear. Thus a unit basis with one axis at
`1.00015` is accepted while `1.00016` is refused, and an input that is both
exactly singular and sheared now uses the existing
`reflection_or_singular` reason rather than `non_uniform_or_sheared`. The
machine-readable vocabulary is unchanged; these symmetric boundaries and
precedence replace two independent algorithms with one explicit typed policy.
The classifier runs after canonicalization's separate existing
zero-translation/fourth-column `w_axis ~= Vec4::W` gate and its first-axis
`<= 1e-4` `zero_scale` reason. That zero-scale gate reads the same widened
`f64` axis length as the classifier, so a finite large basis cannot overflow a
binary32 length intermediate and be mislabeled as zero scale.
Near the shear boundary, the symmetric average base and binary64 dot products
can also refuse inputs that the former first-column/binary32 check accepted.

**Measurement shares facts, not the positive-only verdict.** Measurements v12
derives axis lengths, their canonical ascending-order mean, determinant,
axis-length product, and XY/XZ/YZ dot products from the same crate-private
widened `f64` fact record. It also uses the shared symmetric
mean/longer-operand equal-axis predicate, which removes v11's privileged-X
result and reconciles the finite
`diag(1, 1, 1.000012)` fixture. Measurement still asks a different question:
it keeps finite partial evidence for singular, reflected, sheared, and
non-uniform inputs; normalizes each orthogonality comparison by that axis
pair's length product; and classifies in singular, reflected, sheared,
unit/uniform, then non-uniform precedence. Calling the positive-uniform
classifier directly would discard those descriptive facts and impose the
operation's different precedence. Skin-bind summary factors are sorted before
their mean is derived and every factor is compared to that mean, so authored
joint order cannot choose `consistent_uniform` versus `mixed_uniform` or the
reported common factor. These corrected numeric semantics require the nested
measurements-v12 identity. The JSON shape remains unchanged, but the outer
identity advances to output-v6 because each immutable output schema statically
pins its nested measurement URN; measurements v11 and output-v5 remain
immutable historical evidence. In general, changing a nested identity that an
immutable outer schema references requires a new outer identity too; historical
outer schemas are never retargeted or broadened in place.

Measurements v13 makes inverse-bind derivation auditable and fail-closed. Each
per-joint bind observation retains the exact finite raw inverse-bind matrix for
its accessor slot. Before publishing an inverse, the source bottom row must be
within absolute `1e-6` of affine `[0, 0, 0, 1]`; this admits several binary32
round trips around one but refuses projective translation columns. The linear
3x3 reports the scale-free reciprocal infinity-norm condition number
`1 / (norm_inf(A) * norm_inf(inverse(A)))`. Exactly singular sources and
finite sources at or below `1e-6` use distinct `non_invertible` and
`ill_conditioned` unavailable reasons. This avoids the prior scale-bearing,
exact-zero determinant decision: a uniformly small but well-conditioned matrix
is assessed by its error amplification rather than its determinant magnitude.
The new nested shape advances the immutable outer envelope to output v7;
measurements v12 and output v6 remain historical.

The implementation status is:

- shipped: raw glTF/GLB capability preflight, the shared core plan/ledger and
  proof, preservation-safe whole-document and rest/bind exact-source writers,
  artifact proof, and atomic CLI publication with scale-evidence v4;
- shipped: glTF-only assembly integration in recipe/evidence v4, including the
  versioned basis fingerprint/comparator, pre-remap clip rebasing, exact input
  digests, compatibility evidence, and proof over the exact staged artifact;
- shipped: issue #286-A's explicit conservative ufbx-side status for every
  current D.4 domain and documented normalized source-skeleton projection, with
  both FBX scale operations
  still refused and no raw-span/artifact-preservation claim;
- deferred: FBX rest/bind output in the remaining issue #286 enablement slice,
  after an FBX writer and proof boundary can consume that inventory;
- shipped: raw glTF whole-document scaling for dense `f32` morph `POSITION`
  deltas with numeric-value preservation for static JSON weights and byte-exact
  preservation for animated weight accessors (issue #298);
  rest/bind morphs and a normalized core morph model remain out of scope.

The live implementation sequence is owned by tracker issue #344 and roadmap
issue #165, not by a numbered future-slice list in this decision record. Work
beyond this positive-uniform, fail-closed decision remains ADR-gated.
