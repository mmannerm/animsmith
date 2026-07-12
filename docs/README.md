# Documentation

Find what you need by task — each page below owns one job.

| Document | Use it to… |
|---|---|
| [Why animsmith](why-animsmith.md) | Decide whether animsmith fits your team — what it is, why it exists, and what it is worth by role. The canonical home of the positioning case. |
| [Game-ready clips guide](game-ready-clips.md) | Understand what "game-ready" means — the staged readiness ladder and what animsmith validates at each level — and why a check fires: every runtime failure mode, mapped to the checks, repairs, and config that address it. |
| [Pipeline scenario guide](pipeline-scenarios.md) | Plan a raw-to-game-ready asset process — marketplace intake, mocap cleanup, outsourced acceptance, CI gating, and artifact storage. |
| [Examples cookbook](../examples/README.md) | Do the work, copy-paste style — gate exports in CI, repair a broken export, trim or re-anchor a clip, encode a project contract config, migrate FBX/Mixamo exports, embed the checks in Rust. |
| [Mixamo tutorial](mixamo-tutorial.md) | Take a real Mixamo download end-to-end — download, convert, inspect, lint, fix, and grow a contract config with the built-in `mixamo` rig profile. |
| [cli.md](cli.md) | Look up a command, flag, or exit code. |
| [embedding.md](embedding.md) | Choose library crates and integration boundaries, then follow the embedded gate flow with the runnable [`embed`](../crates/animsmith/examples/embed.rs) example. |
| [docs.rs API references](https://docs.rs/animsmith-core) | Look up exact published Rust API contracts for [`animsmith-core`](https://docs.rs/animsmith-core), [`animsmith-gltf`](https://docs.rs/animsmith-gltf), [`animsmith-fbx`](https://docs.rs/animsmith-fbx), and [`animsmith-report`](https://docs.rs/animsmith-report). |
| [output.md](output.md) | Parse the versioned `--format json` envelope in a pipeline, validated by the JSON Schema under [`schemas/`](schemas/). |
| [README](../README.md) | Install and quickstart, plus the check and configuration reference. |
| [DESIGN.md](../DESIGN.md) | Follow the architecture, check-catalog rationale, and roadmap. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) / [DEVELOPMENT.md](../DEVELOPMENT.md) | Work on animsmith itself — contribution flow and development setup. |
| [RELEASING.md](../RELEASING.md) | Cut a release — the release-plz flow, the manual 0.1.0 bootstrap, and the published-doc-link policy. |
| [SUPPORT.md](../SUPPORT.md) / [SECURITY.md](../SECURITY.md) | Get help, file a bug, or report a vulnerability privately. |
| [research/](research/game-ready-animation-clips.md) | Read the dated research notes that inform the roadmap. |
