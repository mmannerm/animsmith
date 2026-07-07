# Fuzz targets

Coverage-guided fuzzing for the loader entry points that ingest untrusted
files. This is the executable check on **invariant-1** (see
`.claude/skills/audit-task/code-invariants.md`): `lint` / `inspect` /
`measure` / `report` / `fix` run on arbitrary downloaded files, so a
malformed input must produce a `LoadError` — never a panic, never an
unbounded (length-field-driven) allocation.

## Targets

| Target | Entry point | Surface |
| --- | --- | --- |
| `gltf_load` | `animsmith_gltf::load` | GLB container framing, embedded JSON, accessor offsets, node topology |
| `gltf_fix_quat_hemisphere` | `FixSession::apply_to_path(..., Repair::QuatFlip)` | read → byte-patch rotation accessors → re-derive GLB chunk bounds on write |
| `fbx_load` | `animsmith_fbx::load` | the ufbx C-library boundary and the animation bake after it |

Each target writes its fuzz bytes to a scratch file and calls the real
`&Path`-based public API (see `src/lib.rs`).

## Running

Requires a nightly toolchain and `cargo-fuzz`:

```bash
rustup toolchain install nightly --component rust-src
cargo install cargo-fuzz

# Run one target off its seed corpus (new inputs land in the first dir):
cargo +nightly fuzz run --release gltf_load \
    fuzz/corpus/gltf_load fuzz/seeds/gltf_load -- -max_total_time=60
```

`--release` is deliberate: it matches the panic semantics of the shipped
CLI that invariant-1 governs (debug assertions and overflow checks off),
so a reported crash is one the real tool can actually hit. AddressSanitizer
stays on regardless. See `Cargo.toml` for the full rationale.

## Corpus and regression fixtures

- `fuzz/seeds/<target>/` — checked-in starting inputs: valid samples
  derived from `testdata/`, plus `regression-*` files, one per crash
  found. These are read-only starting points passed to every run.
- `fuzz/corpus/<target>/` — the live working corpus a run grows locally.
  Gitignored; safe to delete.
- `fuzz/artifacts/<target>/` — crash reproducers libFuzzer drops.
  Gitignored. To promote one into a permanent regression, copy it to
  `fuzz/seeds/<target>/regression-<slug>` and add a matching unit test.

Every committed crasher is also pinned by a unit test in
`crates/animsmith-gltf/tests/hardening.rs`, so the fix is enforced by the
stable-toolchain PR CI even though fuzzing itself is nightly-only.

## CI

`.github/workflows/fuzz.yml` runs each target for 60s weekly and on
demand (`workflow_dispatch`). It is **not** a PR gate — fuzzing does not
slow the merge path; the checked-in regression seeds reproduce known
crashers on the first execution regardless.
