# Embedding animsmith in a pipeline

The CLI is one frontend. Asset pipelines that already have their own
contract formats, sidecar schemas, and gates embed the library instead:
`animsmith-core` (measurements, checks, config — no file formats, no
I/O) plus one ingestion crate (`animsmith-gltf`, `animsmith-fbx`).

Add the crates you need:

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
# Optional, only when you ingest FBX:
animsmith-fbx = "0.1"
```

Canonical API docs live on docs.rs after publish:
[animsmith-core](https://docs.rs/animsmith-core),
[animsmith-gltf](https://docs.rs/animsmith-gltf), and
[animsmith-fbx](https://docs.rs/animsmith-fbx). `animsmith-core` is the
embedding boundary; the CLI crate is not the library API. The Rust API
is still pre-1.0 and experimental, so prefer the crate-root catalog
functions and documented data/config types over internal modules.
`animsmith-core` re-exports `glam` as `animsmith_core::glam` because its
public types use `glam` vectors, quaternions, and matrices.

The worked, compiling version of everything below is
[`crates/animsmith/examples/embed.rs`](../crates/animsmith/examples/embed.rs)
— run it with:

```console
cargo run -p animsmith --example embed
```

## The five steps

```rust
use animsmith_core::{CheckCtx, Config, all_checks, run_checks};
use animsmith_core::measure::measure_document;
use animsmith_core::profile::detect_profile;

// 1. Load. Values arrive exactly as authored — no renormalization,
//    no resampling — so checks judge the real bytes.
let doc = animsmith_gltf::load(path)?;

// 2. Resolve rig roles. Checks never reference bone names, only
//    roles; auto-detection scores the built-in profiles, or bind
//    roles explicitly with `ResolvedRoles::from_names`.
let roles = detect_profile(&doc.skeleton).unwrap_or_default();

// 3. Declare expectations programmatically. The TOML file is just
//    one constructor of `Config` — an embedder builds the same
//    struct from its own contract format and never teaches
//    animsmith that format.
let mut config = Config::default();
config.clips.insert("run_*".into(), /* looping, speed pins, … */);

// 4. Measure (numbers without judgment) and lint (numbers judged
//    against the config).
let measurements = measure_document(&doc, &roles, &config);
let ctx = CheckCtx::new(&doc, &roles, &config);
let findings = run_checks(&ctx, &all_checks());

// 5. Map severities to your gate. The CLI's convention: exit 0 =
//    clean or warnings-only, 1 = any Error finding, 2 = operator
//    error (unreadable file, bad config).
```

Key types, all in `animsmith-core`:

| Type | Role |
|---|---|
| `Document` / `SceneAssets` | skeleton + clips / meshes + materials |
| `ResolvedRoles` | role → bone binding (profile or explicit) |
| `Config`, `ClipExpectations`, `Pinned` | what the author declares |
| `Finding` | structured verdict: `check_id`, severity, clip/bone/time, measured vs expected |
| `measure::ClipMeasurements` | the raw metric map `measure` emits |

## Stability contracts

Embedders can pin on these; changing any of them is a breaking change
(see `.claude/skills/audit-task/code-invariants.md` §7):

- **Check ids** (`loop-seam`, `quat-flip`, …) — config keys and JSON
  fields.
- **JSON output schema** — versioned via `schema_version`; JSON is an
  envelope with `tool`, `command`, `summary`, and `files`.
- **Exit codes** — 0 / 1 / 2 as above.
- **Measurement semantics** — the sampling model is "glTF-spec
  interpolation on a uniform grid, wrap pair = (last frame, 0)";
  metric changes require golden-test re-verification.

For v0.1, the most stable integration path is:

1. Load with `animsmith-gltf` or `animsmith-fbx`.
2. Build `Config` from your own contract format.
3. Call `measure_document`, `CheckCtx::new`, `all_checks`, and
   `run_checks`.
4. Map `Finding` values into your gate/reporting system.

The `Check` trait is public because the built-in catalog uses it and
advanced embedders may want to experiment, but custom-check/plugin
registration is not yet a stability promise. Prefer wrapping animsmith's
findings with your own pipeline checks until a registry API lands.

## Migrating a script-based bake pipeline

The subcommands were designed as drop-in replacements for the pieces a
typical Python/DCC bake pipeline accumulates. The mapping:

| If your pipeline has… | Replace with | Notes |
|---|---|---|
| An FBX→glTF converter binary (e.g. the archived FBX2glTF) | `animsmith convert in.fbx -o out.glb` | Triangulated + welded meshes, skins, factor materials, embedded PNG/JPEG base-color textures; `--animation-only` to strip geometry. Units/axes normalized to glTF conventions; scale-compensated (Maya-style) rigs handled. |
| A quaternion hemisphere-normalization pass | `animsmith fix clip.glb -o out.glb` or `animsmith fix clip.glb --in-place` | Byte-surgical: only repaired keys change, container/meshes byte-identical; lossless and idempotent; handles LINEAR/STEP/CUBICSPLINE. Use `--dry-run` to inspect first (exits 1 when repairs are pending). |
| Frame-range slicing, final-pose hold extension, cycle re-anchoring scripts | `animsmith transform --slice a:b --hold-extend s --gait-anchor --fps n` | Slice copies keys verbatim (half-frame epsilon); gait-anchor is frame-quantized and picks the cleanest wrap of the ±1-frame candidates. |
| Loop-seam / gait-phase / root-motion measurement scripts | `animsmith measure --format json`, or the library API above | Same numbers, golden-tested; feed them to your own sidecar/contract format via the library. |
| A post-bake contract/validation gate | `animsmith lint --config your.toml` (or a programmatic `Config`) | See [`examples/character.animsmith.toml`](../examples/character.animsmith.toml) for a game character's contract expressed as config. |

Migration recipe that has worked in practice:

1. **Convert first, verify second.** Run the new converter alongside
   the old one and compare with `animsmith diff old.glb new.glb` —
   it compares the *measurements that matter* (seams, gait, speeds,
   rotation ranges) rather than bytes.
2. **Pin the old pipeline's numbers as golden values** (env-gated
   tests against your shipped assets) before switching, so the
   cutover is provable rather than hopeful.
3. **Encode the contract as config last**, once lint runs clean on
   the current assets — every pre-existing violation you discover is
   either a real bug (fix the asset) or a tolerance your contract
   needs to state explicitly.
4. Keep your sidecar/hash/provenance machinery on your side of the
   line: animsmith deliberately does not know your contract format.

## What the library will not do

- Parse your contract files (build `Config` yourself — that is the
  API contract that keeps animsmith engine-agnostic).
- Hash files, track provenance, or decide staleness (pipeline
  concerns; you have the bytes).
- Panic on malformed input (a `LoadError` or findings, never a crash
  — report an issue if you find otherwise).
