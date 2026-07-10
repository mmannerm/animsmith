# animsmith-core

## Overview

`animsmith-core` is animsmith's engine-agnostic library crate. This
README is a compact crates.io and repository index; the crate-root
rustdoc owns the embedding flow, API status, extension points, and
panic/error contracts.

## Install

```toml
[dependencies]
animsmith-core = "0.1"
animsmith-gltf = "0.1"
```

## Feature Flags

- `fixtures` (off by default) — exposes `animsmith_core::fixtures`, the
  analytic-clip fixture builders shared with animsmith's own tests and
  example-asset generator. Adds no dependency (the builders take their
  sine as a parameter). Internal to the animsmith workspace and **not**
  part of the crate's stable API; downstream code should not depend on
  it.

The workspace MSRV is Rust 1.88.

## More Details

- [API reference on docs.rs after publication](https://docs.rs/animsmith-core)
- [Embedding animsmith in a pipeline](https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md)
- [Raw asset to game-ready pipeline scenarios](https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md)
- [Workspace design](https://github.com/mmannerm/animsmith/blob/main/DESIGN.md)
- [CLI crate and examples](https://github.com/mmannerm/animsmith/tree/main/crates/animsmith)

## License

Licensed under either the MIT license or the Apache License, Version
2.0, at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in animsmith by you is licensed as MIT OR
Apache-2.0, without any additional terms or conditions.
