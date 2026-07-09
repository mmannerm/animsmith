# Example assets

Small glTF clips used by the [examples cookbook](../README.md).
They are **procedurally generated**, not hand-authored — provenance is
the generator source, and they can be reproduced byte-for-byte at any
time.

| File | What it is |
|------|------------|
| `clip.glb` | A clean two-bone rig (`root` → `spine`) with one 1 s rotation clip named `swing`. Lints clean (exit 0). |
| `clip-dirty.glb` | The same clip with two deliberate, repairable defects: one non-unit rotation key (`quat-norm`) and one sign-flipped key (`quat-flip`). Everything else is identical, so `fix` restores it exactly and `diff` reports no measurement drift. |
| `walk.glb` | A hips + two-foot rig (`pelvis` / `foot_l` / `foot_r`, resolving the `ue-mannequin` profile) with a 1 s walk cycle that closes exactly. Fires the semantic checks; passes [`walk.animsmith.toml`](../walk.animsmith.toml). |
| `walk-dirty.glb` | The same walk cut a quarter-cycle short, so the feet don't return to their first-frame pose — a popped loop seam. Fails `loop-seam` under the same contract. |

`clip-dirty.glb` is a `.glb` (not `.gltf`) on purpose: `fix` is
byte-surgical over a GLB binary chunk and skips the data-URI buffers a
`.gltf` embeds, so the repair workflow needs binary input.

## Regenerating

```console
cargo run -p animsmith --example gen_example_assets
```

The generator lives at
[`crates/animsmith/examples/gen_example_assets.rs`](../../crates/animsmith/examples/gen_example_assets.rs).
Pass an output directory to write elsewhere:

```console
cargo run -p animsmith --example gen_example_assets -- /some/dir
```

Both the generator and `example_assets_match_generator_output` in
[`crates/animsmith/tests/examples_cookbook.rs`](../../crates/animsmith/tests/examples_cookbook.rs)
write these assets through the same `write_example_assets` wiring in
[`animsmith-testkit`](../../crates/animsmith-testkit), so changing that
builder or its filename wiring (or hand-editing the committed bytes)
without regenerating fails CI. That test file
also covers every [cookbook](../README.md) command that runs
against these committed assets — the first-gate, repair, transform, and
config-steering workflows — asserting each one's exit code plus a
distinctive output or downstream-state check, so those commands can't
drift out from under the docs unnoticed. (The cookbook's remaining
examples use placeholder or FBX assets this repo doesn't ship, so they
aren't smoke-tested here.) The guard set is maintained here rather than
derived from the doc, so a newly documented committed-asset command
needs its own check added.

## License

Generated from this repository's source; released under the same
`MIT OR Apache-2.0` terms as the crate.
