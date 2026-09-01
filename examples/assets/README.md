# Example assets

Small glTF clips used by the [examples cookbook](../README.md).
They are **procedurally generated**, not hand-authored — provenance is
the generator source, and they can be reproduced byte-for-byte at any
time.

The first four rows are the cookbook's clean/dirty pairs. The five
`walk-*` / `run-ring` rows after them are one fixture per runtime
symptom: each is the clean `walk.glb` cycle plus exactly the authored
defect its checks measure. Every row's findings are pinned by a test (see
[Regenerating](#regenerating)), so this table cannot drift away from what
the CLI reports.

| File | What it is | Fires | Config |
|------|------------|-------|--------|
| `clip.glb` | A clean two-bone rig (`root` → `spine`) with one 1 s rotation clip named `swing`. | nothing (exit 0) | none |
| `clip-dirty.glb` | The same clip with two deliberate, repairable defects: one non-unit rotation key and one sign-flipped key. Everything else is identical, so `fix` restores it exactly and `diff` reports no measurement drift. | `quat-norm`, `quat-flip` | none |
| `walk.glb` | A hips + two-foot rig (`pelvis` / `foot_l` / `foot_r`, resolving the `ue-mannequin` profile) with a 1 s walk cycle that closes exactly. The clean control the symptom fixtures below are mutations of. | nothing (exit 0) | [`walk.animsmith.toml`](../walk.animsmith.toml) |
| `walk-dirty.glb` | The same walk cut a quarter-cycle short, so the feet don't return to their first-frame pose — a popped loop seam. | `loop-closure`, `loop-seam`, `loop-seam-vel` | [`walk.animsmith.toml`](../walk.animsmith.toml) |
| `walk-short-channel.glb` | The walk plus a `foot_l` rotation channel that ends at 0.75 s while both translation channels run to 1.0 s — the limb an engine clamp-holds while the rest of the body keeps moving. | `duration-sanity` (warning; 0.25 s spread) | none; the check is mechanical |
| `walk-travel.glb` | The walk with the hips carried 1.2 m forward over the cycle: a root-motion clip, measured at 1.2 m/s. | `in-place` under gameplay-owned XZ, or `root-motion-speed` against a stale 1.0 ± 0.1 m/s pin | [`walk-travel-in-place.animsmith.toml`](../walk-travel-in-place.animsmith.toml), [`walk-travel-root-motion.animsmith.toml`](../walk-travel-root-motion.animsmith.toml) |
| `run-ring.glb` | One rig carrying four directional clips (`run_forward`, `run_backward`, `run_left`, `run_right`) built from the same analytic gait, with `run_left` entered a quarter cycle late — stride anchor 0.50 against the others' 0.75. | `gait-group` (0.20 cycle spread against a 0.15 cap) | [`run-ring.animsmith.toml`](../run-ring.animsmith.toml) |
| `walk-frozen-arm.glb` | The walk rig plus an `arm_l` whose rotation channel is keyed at five identical values. There is no `arm_r` at all — that is the channel the export dropped. | `frozen-bone` (`arm_l`), `missing-bones` (`arm_r`), `constant-track` (`arm_l`) | [`walk-frozen-arm.animsmith.toml`](../walk-frozen-arm.animsmith.toml) |
| `walk-scaled.glb` | The walk with a pelvis scale track stretching Y to 1.2 and back, plus a five-key `weapon_socket` translation channel that never moves — the one track `transform --prune-constant-tracks` removes. | `scale-keys`, `non-uniform-scale` (`pelvis`), `constant-track` (`weapon_socket`) | none; the checks are mechanical |
| `report-comparison-before.glb` | A five-bone synthetic gait with a left-foot endpoint seam, sampled stance slide, a closed root path, and a redundant constant quaternion track. | `loop-closure`, `loop-seam-vel`, `foot-slide`, `constant-track` | [`report-comparison.animsmith.toml`](../report-comparison.animsmith.toml) |
| `report-comparison-after.glb` | The paired synthetic gait with closed foot endpoints, corrected stance trajectories, a distinct closed root path, and the redundant quaternion track removed. | nothing (exit 0) | [`report-comparison.animsmith.toml`](../report-comparison.animsmith.toml) |

`clip-dirty.glb` is a `.glb` (not `.gltf`) on purpose: `fix` is
byte-surgical over a GLB binary chunk and skips the data-URI buffers a
`.gltf` embeds, so the repair workflow needs binary input.

`walk-travel.glb`'s two configs declare no `loop` and switch `foot-slide`
off, so each documented run shows the glide symptom alone. Both
suppressions describe the fixture rather than the symptom: the clip's
endpoints are 1.2 m apart by construction, so `loop = true` would bury
the finding under loop-closure and loop-seam errors, and its feet are the
clean walk's — swinging under a travelling pelvis rather than authored
planted — so every stance frame sweeps at the root's full speed. Both of
those families already have their own fixture: `walk-dirty.glb` for the
popped loop, `report-comparison-before.glb` for the within-clip skate.

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

All eleven assets are deterministic. Both the generator and
`example_assets_match_generator_output` in
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

The symptom fixtures are pinned the same way by
[`crates/animsmith/tests/symptom_fixtures.rs`](../../crates/animsmith/tests/symptom_fixtures.rs),
which runs the real CLI over each fixture/config pair and asserts the
exact set of check ids, severities, and clip/bone subjects in the table
above, together with the authored measurement behind each one. It also
pins the controls that make each row a mutation rather than a
coincidence: the same fixture with no config reports none of its contract
findings, and the clean rig under the same config reports none either.

## License

Generated from this repository's source; released under the same
`MIT OR Apache-2.0` terms as the crate.
