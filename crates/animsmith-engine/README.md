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

## Install

```toml
[dependencies]
animsmith-core = "0.6"
animsmith-engine = "0.6"
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
