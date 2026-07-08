# animsmith — design & requirements

Status: v0.1 publishing design. Intended to keep the public crate and
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
  slice/trim + hold-extend, gait-anchor rotation, and format conversion
  including a full mesh/skin FBX→glTF path (a maintained replacement
  for the archived FBX2glTF). Out of scope stays *artistic*
  transformation: retargeting, motion editing, procedural animation —
  that is DCC work. The rule of thumb: animsmith may rewrite a clip
  only in ways whose correctness its own checks can verify.
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
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor]
animsmith fix     <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb> [--animation-only]
animsmith diff    <A> <B> [--format text|json]     # A/B: asset files or prior `measure` JSON
```

- `lint` = measure + judge against config. `measure` is lint minus
  judgment — it emits the raw measurement map (the substrate other
  pipelines pin their own contracts to).
- **Exit codes**: `0` clean or warnings-only, `1` at least one
  error-severity finding (or pending repairs under `fix --dry-run`),
  `2` operator/tool error (unreadable file, bad config).
  `--deny-warnings` promotes warnings to errors.
- Inputs: `.glb`, `.gltf` (+ external buffers), `.fbx` (via the `fbx`
  feature, default-on in the released binary).
- **Malformation policy**: *structural* malformation — keyframe/value
  count mismatch, zero-key channels, absolute or escaping external
  buffer URIs, non-forest node graphs (cycles or a node with two
  parents) — is rejected at load (operator error, exit 2; run
  glTF-Validator for the details). Recovering a non-forest graph would
  force an arbitrary parent choice or silently drop a cyclic subtree, so
  the loader rejects rather than repairs (decision recorded for #92).
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
- `convert` is compiled only with the `fbx` feature. `--no-default-features`
  remains a glTF-only pure-Rust CLI with validation, transform, fix, and
  diff commands intact; `report` is controlled separately by the
  `report` feature.

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

**Raw layer** — what the file says. Needed by the mechanical checks (NaN,
quaternion flips, key density, constant tracks):

```rust
pub struct Document { pub skeleton: Skeleton, pub clips: Vec<Clip>,
                     pub assets: SceneAssets,            // meshes/skins/materials, when the input carries them
                     pub source: SourceInfo }
pub struct Skeleton  { pub bones: Vec<Bone> }            // topological order, parents first
pub struct Bone      { pub name: String, pub parent: Option<BoneId>,
                       pub rest: Transform,              // node-local TRS
                       pub inverse_bind: Option<Mat4> }  // from skin, when present
pub struct Clip      { pub name: String, pub duration_s: f64, pub tracks: Vec<Track> }
pub struct Track     { pub bone: BoneId, pub property: Property,   // T | R | S
                       pub times: Vec<f32>, pub values: TrackValues,
                       pub interpolation: Interpolation }          // Linear | Step | CubicSpline
```

`assets` (meshes, skins, factor-only materials) is the geometry half of
the document. Both the FBX and glTF loaders populate it from a single
`load` (there is no separate assets-carrying entry point — the two
loaders share one shape); it is empty only when the input carries no
geometry. The check catalog ignores it — checks judge animation — but it
rides the one `load`/`write` round-trip, so `transform` and `convert`
preserve geometry rather than silently dropping it, and `measure`
reports mesh-level measurements (vertex count, AABB, joints-per-vertex,
weight sums) from it (#16).

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
check shares. FK accumulates local TRS to model space; the scene-root
transform is excluded so measurements are independent of asset centering.
The metric grid is computed once per clip and shared across checks,
measurements, and reports through the lazy `MetricGrids` owner.

**Rig profiles** — checks never reference bone names; they reference
*roles*:

```rust
pub enum Role { Root, Hips, Spine, Head, LeftFoot, RightFoot, LeftToe, RightToe, LeftHand, RightHand, /* … */ }
pub struct RigProfile { pub name: String, pub bindings: Vec<(Role, NameMatcher)> }
// NameMatcher: Exact | Suffix | Glob, with an optional namespace-strip pass ("ns:Hips" → "Hips")
```

Built-in profiles ship for `mixamo` (`mixamorig:Hips`…), `ue-mannequin`
(`pelvis`, `foot_l`…), and `humanoid` (`humanoid_ Pelvis`,
`humanoid_ L Foot`…), plus **auto-detection** that scores every profile by
resolved-role coverage and reports the winner in `inspect`. A check whose
required roles don't resolve is *skipped with a note* — never a false
failure. This is the single design rule that makes the tool useful outside
its birthplace: tolerance data and bone names are config; the math is not.

The runner, not each check, owns that rule. A check declares its
prerequisites through `readiness(ctx)`; the runner emits one standardized
skip-note per unmet requirement. Crucially, a skip-note is a **diagnostic**
— exempt from the per-check `severity` override: `[checks.loop-seam]
severity = "error"` escalates loop-seam's *violations* but can never turn a
"roles unresolved" note into a false Error. Exemption is a property of the
finding (`Finding::diagnostic`), so a check with role-independent work can
stay `Ready` and mark its own skip-note: `gait-group` always validates that
its members exist (a config error needing no rig) and reports that Error
even when the rig is unresolved, while marking the *measurement* skip-note
a diagnostic. `severity = "off"` removes the check from the run set
entirely — it never executes.

**Checks** implement one trait and emit structured findings:

```rust
pub trait Check {
    fn id(&self) -> &'static str;              // "loop-seam", "quat-flip", …
    fn readiness(&self, ctx: &CheckCtx) -> Readiness;  // Ready | Skipped(reason) | Idle
    fn run(&self, ctx: &CheckCtx, out: &mut Findings);
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
| `scale-keys` | scale tracks present (warn); non-uniform scale (opt-in error) | raw | severities | new |
| `constant-track` | track never deviating from rest beyond eps (bloat), or a track that is *unexpectedly* constant | raw | eps | new |
| `frozen-bone` | required bone's max angular deviation from first frame below floor | grid + roles/meta | `min_rotation_deg` | reference contract rotation floor + measured rotation ranges |
| `loop-seam` | last→first position wrap discontinuity of feet-relative-to-hips, normalized by the *local neighbour* per-frame step, with a stride floor so stationary clips skip | grid + Hips/feet/toe roles | `max_ratio`, `min_stride_step_m` | `locomotion_metrics.py` — port verbatim |
| `root-motion-speed` | horizontal root/hips displacement ÷ duration vs declared `speed_mps`; flags stray speed pins on non-locomotion clips | grid + Root/Hips | pinned speed + tolerance (reference gate: 15%) | reference bake |
| `missing-bones` | declared-required animated bones absent; tracks targeting nodes outside the skeleton | raw + meta | bone/role list | reference contract `animates_bones` |
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
| `loop-seam-rot` / `loop-seam-vel` | rotational C0 and velocity C1 seam continuity | flagged in the incubating project, unimplemented |
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
# [rig.roles]
# hips = "humanoid_ Pelvis"
# left_foot = "humanoid_ L Foot"

[checks.loop-seam]
severity = "error"                 # off | note | warn | error
max_ratio = 1.5

[checks.quat-flip]
severity = "warn"

[clips."run_*"]                    # glob; exact > glob, later entries win ties
loop = true
in_place = true
speed_mps = { value = 3.1, tolerance = 0.25 }
fps = 30

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
- **JSON** (`--format json`): versioned native envelope —
  `{ schema_version, schema, tool: {name, version}, command, summary,
  files: [{path, rig: {profile, resolved_roles}, findings?, measurements}] }`.
  `measure` omits `findings`; `lint` emits both findings and
  measurements. The top-level envelope leaves room for multi-file runs,
  future metadata, and additional formats without changing per-file
  records.
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
- **`convert`** emits glTF 2.0 GLB: nodes + skin (computed IBMs) +
  animations always; mesh + weights when present; `--animation-only` to
  strip mesh. Explicitly *not* an art exporter — no material fidelity
  promise. Both `convert` and `transform` share one `load`→`write`
  round-trip over `Document` (assets included), so geometry survives a
  transform pass and `--animation-only` clears it uniformly across input
  formats (it is the only lever that drops geometry).
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
