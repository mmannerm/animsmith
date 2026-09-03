# animsmith-engine

> **Pre-1.0:** Breaking changes are expected between minor releases. Pin
> dependency versions and review the release notes before upgrading.

## Overview

`animsmith-engine` provides AnimSmith's immutable V1 engine-import profile
registry and its deterministic two-phase settings resolver. The registry is
strict: callers select one exact family, profile revision, engine version, and
importer tuple, then resolve it against an authoritative input format and the
actual clip names.

The crate performs no filesystem access, parses no configuration format, and
does not depend on an animation format crate or engine SDK. Unknown facts stay
explicitly unknown; the registry does not predict engine output. A one-way
adapter can publish an already-resolved profile and its same-load
`animsmith-core::LoadedSource` evidence as prediction-provenance V1 without
rerunning resolution or reading source bytes.

The public `EngineAddressabilityCheck` evaluates only the frozen Bevy 0.19.0
glTF source-animation index rule. It borrows same-load source evidence and
emits standard AnimSmith engine-prediction facets; callers may validate the
`ENGINE_CHECK_IDS_V1` catalog before asset I/O. It does not predict named
asset labels, target-path identifiers, runtime-node selection, or imported
transform behavior.

`BevyAnimationAssetLabelV1` is the bounded authority for the exact
`Animation{source_clip_index}` display selector. The
`build_bevy_animation_addressability_adapter_v1` helper sends that same
existing check through the ordinary evaluation lifecycle once and packages
its unchanged record with same-load provenance and a neutral
`GltfAnimationAddressabilityInventoryV1`. It returns no adapter for absent or
non-Bevy provenance and does not add a second check or runtime-existence claim.

`GltfAddressabilityV2` is the separate rich contract
`urn:animsmith:schema:gltf-addressability:2`. It preserves the V1 inventory and
adds independently covered raw glTF scene, node, skin, attachment, path,
default-scene, named-map, and unique-target projections. Complete empty
coverage proves absence; partial prefixes and unavailable domains do not.
Explicit `skin.skeleton` remains source evidence only because Bevy ignores it;
scene-instantiated `SkinnedMesh` attachment is outside this static contract.

Its optional `BevyGltfAddressabilityRulesV1` bundle is pinned to Bevy
`v0.19.0` commit `c6f634ca9f406d68ba5109d921247b654cb42c10`, `bevy_gltf 0.19.0`,
locked `gltf 1.4.1`, and commit-pinned loader, label, path, target-id, feature,
and root `Cargo.lock` sources. The isolated probe's committed lock is the graph
authority: every Bevy 0.19 release crate in that lock is `0.19.0`, and the
probe rejects internal Bevy patch drift; independently-versioned helpers retain
their own versions. The adapter
reuses one `engine-addressability` evaluation and
the existing `Animation{i}` selector. It predicts `Scene{i}`, an optional
route from `Gltf.default_scene` to an existing scene, eager
`Skin{i}/InverseBindMatrices` for every source skin, and conditional `Skin{i}`
when any source node references it. There is no `DefaultScene` label and no
fabricated `Scene0`; typed source indices remain authoritative over collected
map/vector order.

Target UUID prediction requires explicit `TargetPointerWidth::Bits32` or
`Bits64`, matching Bevy's target-width little-endian hashing; host width is never
inferred. Incomplete, unreachable, multiply reachable, colliding, or disabled
target work is required-unavailable. The explicit `target_coverage` projection
distinguishes a complete (including empty) target domain from
`target_domain_truncated`; other rich projection budget failures use
`projection_bounds_exceeded`. V2 has explicit row, reference, text,
path, and staged-reader bounds and makes no claim that Bevy loaded, spawned,
retained, wired, or played the asset.

`BevyReadbackV1` is the separately strict, engine-neutral public contract for
an owner-run exact-Bevy observation. It records the frozen harness and lock,
the immutable private snapshot bytes actually given to `AssetServer`, the exact
V2 document and V4 provenance identities, bounded redacted warning metadata,
terminal lifecycle, and available typed inventories. Its reader rejects unknown
fields, malformed or oversized input, noncanonical rows, changed self-identity,
and any stored conformance result that does not recompute from the strict V2
prediction. The probe preserves closure-relative paths in its private,
read-only snapshot, then re-hashes and removes that retained tree after all
observation and before report serialization; on Windows the probe additionally
refuses final reparse points and validates each opened source handle's final
path remains under the authorized root, so neither the owner path nor the
temporary path enters the contract.
The headless Bevy executable is deliberately excluded from this Rust-1.88
workspace because its isolated build records and enforces the exact official
compiler identity `rustc 1.95.0 (59807616e 2026-04-14)`; see
`tools/bevy-readback/README.md`. Its `Cargo.lock` is therefore outside this
crate, so `BEVY_READBACK_V1_LOCK_BYTES` and `BEVY_READBACK_V1_LOCK_SHA256` are
parsed at compile time from `src/bevy_readback_lock.txt`, two lines written
from that lock by the repository's `just bevy-readback-lock-refresh` rather
than by hand. The `just bevy-readback-lock` gate renders those lines from the
lock and compares them; it never rewrites the committed file.

Resolved settings V1 materializes at most 4,096 actual clip rows. Inputs above
that bound return a typed `ResolutionError::ResolvedSettingsContract`; callers
must not truncate the clip list and claim complete prediction provenance.

`EngineImportAdviceV1` is the separate bounded producer/readback contract for
those materialized settings. Unity 6000.3 Generic/Humanoid projects its exact
document and clip importer values beside same-load provenance, source versus
normalized clip identity, explicit intent, and normalized measurement
availability. Unreal 5.8 and Godot 4.7 revision 1 emit a typed refusal because
their immutable profiles have no setting vocabulary. The contract makes no
filesystem writes and does not infer frame coordinates, sampling, units, or
root-motion behavior.

`EngineImportAdviceV2` is the separate immutable document-level contract
`urn:animsmith:schema:engine-import-advice:2` for the
exact Godot 4.7 revision-2 (`resource-importer-scene`, glTF JSON/GLB) and
Unreal 5.8 revision-2 (`fbx-importer`, FBX) tuples. Godot projects only
`animation/fps` (1..120, verified default 30) and `animation/trimming`
(verified default false). Unreal requires explicit `sample_rate` from the
closed `default_30`, `source_determined`, or `custom_hz(1..48000)` domain.
The V2 envelope references output-v15's V4 provenance/basis types and
has one document-scope projection basis plus an optional native projection, or
a typed refusal. It remains a
same-load parameter projection: engine execution, imported-asset readback,
runtime behavior, and project-file writes are outside its boundary.

## Install

```toml
[dependencies]
animsmith-core = "0.13"
animsmith-engine = "0.13"
```

The compiling example and the full registry API are in the crate-level API
documentation.

## Feature Flags

This crate has no public feature flags. The workspace MSRV is Rust 1.88.

## More Detail

- [API reference on docs.rs](https://docs.rs/animsmith-engine)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)

## License

Licensed under either the MIT license or the Apache License, Version 2.0, at
your option.
