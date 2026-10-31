# Evaluation-model fixtures

`collection-output-v11-complete.json` is license-safe synthetic evidence emitted
by `animsmith collection lint --format json` from two copies of
`crates/animsmith/testdata/rig.gltf`. The temporary manifest binds each `walk`
take into one complete `sync-group`; its explicit config maps `root` and `hips`
roles. The checked-in trailing LF is included in `work.serialized_bytes`.

The Rust unit test and hidden validation-only collection CLI read these exact
LF-pinned bytes through the authoritative `read_collection_output` V11 path.
Python V2 tests invoke that checkout-matched binary and apply synchronized
raw/model mutations; this is test evidence only, never a commercial report or
migration input.


`collection-output-v12-sampling-refused.json` is a synthetic reader vector derived
from `collection-output-v11-complete.json`: source `b` retains its exact input,
loader, dependency closure and observed take, but its analysis and logical
binding are explicitly unavailable with `sampling_limit_exceeded`. Source `a`
retains the independently known 1-second, 1-metre translation and 1 m/s speed.
The runtime set remains present and incomplete, with one measured member. This
is a manually constructed contract vector, not a claim that the current V11
producer emits sampling refusals. The serialized byte count is exact.
