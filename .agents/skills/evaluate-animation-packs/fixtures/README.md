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
