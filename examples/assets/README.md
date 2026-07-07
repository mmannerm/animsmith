# Example assets

Small glTF clips used by the [examples cookbook](../../docs/examples.md).
They are **procedurally generated**, not hand-authored — provenance is
the generator source, and they can be reproduced byte-for-byte at any
time.

| File | What it is |
|------|------------|
| `clip.glb` | A clean two-bone rig (`root` → `spine`) with one 1 s rotation clip named `swing`. Lints clean (exit 0). |
| `clip-dirty.glb` | The same clip with two deliberate, repairable defects: one non-unit rotation key (`quat-norm`) and one sign-flipped key (`quat-flip`). Everything else is identical, so `fix` restores it exactly and `diff` reports no measurement drift. |

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

## License

Generated from this repository's source; released under the same
`MIT OR Apache-2.0` terms as the crate.
