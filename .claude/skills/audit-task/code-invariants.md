# Codebase-specific invariants

General review passes cover generic bugs and security patterns. This
file lists the invariants specific to animsmith that a reviewer can't
infer from a diff alone. The audit checks each one; if the diff touches
none of a section's area, say "not in scope" explicitly — do not
fabricate findings.

Each invariant has a stable identifier (`invariant-1` … `invariant-9`).
Cite invariants by that id in reviews and PR comments — never as `#1`,
which GitHub auto-links to issue #1.

## invariant-1: Untrusted input must never panic

`lint` / `inspect` / `measure` / `report` run on arbitrary files a user
downloaded from anywhere. Malformed input must produce a `LoadError`
(operator error, exit 2) or a finding — never a panic, never unbounded
allocation driven by a length field.

- No `unwrap()`/`expect()`/indexing on values derived from file content
  (accessor counts, indices, offsets, string lengths).
- Out-of-range indices (bone ids, buffer offsets, material slots) are
  skipped or reported, not trusted.
- NaN/Inf in file data flows to the `nan` check, not into a crash.

## invariant-2: Byte-surgical fix guarantee

`fix` may change ONLY the bytes of the values it repairs. File length
is preserved; the JSON chunk, meshes, skins, materials, textures, and
every untouched accessor are byte-identical. A fix must be idempotent
(second run changes nothing) and lossless (a negated quaternion is the
same rotation). Any new fix must keep these three properties and test
them.

## invariant-3: Transform losslessness

Transforms may only do what their contract states: `slice` copies the
kept keys verbatim (no resampling), gait-anchor rotation is
frame-quantized so every resample lands on an existing key, and
`hold-extend` duplicates the final value. A transform that silently
interpolates new values where the contract promises copies is a
blocker.

## invariant-4: Engine-agnostic core

`animsmith-core` depends on glam + serde + thiserror ONLY. No file
formats, no filesystem, no gltf/ufbx/toml types in its public API.
Format crates depend on core, never the reverse. The TOML config is one
constructor of `Config`; embedding pipelines build it programmatically
— core must never parse a config file itself.

## invariant-5: Roles, not bone names

Checks and metrics reference `Role`s resolved through a rig profile,
never literal bone names. A check whose required roles don't resolve is
skipped with a note — it must never produce a false failure on an
unknown rig. Bone-name knowledge lives only in `profile.rs` bindings
and user config.

## invariant-6: Reference-number stability

The loop-seam / gait-phase / root-motion algorithms are ports verified
against an external reference implementation (golden tests, env-gated).
A diff that changes their numbers — sampling grid rule, seam
denominator, trough phase, cycle-period convention — needs the golden
values re-verified and the change justified in the PR description.
"Tests updated to match new output" without justification is a blocker.

## invariant-7: Stable public contracts

- Check ids (`loop-seam`, `quat-flip`, …) are config keys and JSON
  fields: renaming one is a breaking change.
- The machine-readable output schema is versioned; any breaking shape
  change bumps `SCHEMA_VERSION`.
- Exit codes are wire contracts: 0 clean/warnings, 1 error findings,
  2 operator error.
- Default severities and tolerances are behavioural contracts; changing
  a default needs a stated reason in the PR description.

## invariant-8: No licensed assets in the repo

Mixamo, Protofactor, and other licensed content must never be committed
— not as fixtures, not as "small excerpts". Reference tests against
them are env-gated and skip when unset. `testdata/` holds only CC0 or
procedurally generated files.

## invariant-9: Loaders preserve authored data

Ingestion must hand checks the file's real data: no quaternion
renormalization, no resampling, no key deduplication at load time. The
mechanical checks are only meaningful if they see the bytes the author
shipped. Two documented exceptions, both format semantics rather than
cleanup:

- **Space conversion in the FBX loader** — coordinate/unit conversion is
  the format's meaning, not a modification of the authored motion.
- **FBX animation baking** — the FBX loader evaluates each take through
  `ufbx::bake_anim` (see `crates/animsmith-fbx/src/lib.rs`) rather than
  reading raw curve keys. FBX animation is not a flat list of keyed
  values: it layers curves, pre/post-rotation, and node transforms that
  only a full evaluation resolves into the per-bone TRS the core model
  and checks expect. Baking is therefore that resolution, but it does
  resample — the baked keys (emitted as LINEAR) are not necessarily the
  author's original curve keys, so a check reading FBX-sourced key times
  or interpolation sees baked output, not the raw FBX curve. glTF, whose
  on-disk form already matches the core model, is read without baking.
