# Documentation

Find what you need by task — each page below owns one job.

| Document | Use it to… |
|---|---|
| [Why animsmith](why-animsmith.md) | Decide whether animsmith fits your team — what it is, why it exists, and what it is worth by role. The canonical home of the positioning case. |
| [Game-ready clips guide](game-ready-clips.md) | Understand what "game-ready" means — the staged [readiness ladder](game-ready-clips.md#the-readiness-ladder) and what animsmith validates at each level — and why a check fires: every runtime failure mode, mapped to the checks, repairs, and config that address it. |
| [Pipeline scenario guide](pipeline-scenarios.md) | Plan a raw-to-game-ready asset process — marketplace intake, mocap cleanup, outsourced acceptance, CI gating, and artifact storage. |
| [Animation-pack evaluation reports](reports/README.md) | Review evidence-backed marketplace-pack assessments and the reporting convention for constituent packs and collection-level cross-pack summaries. |
| [Examples cookbook](../examples/README.md) | Do the work, copy-paste style — gate exports in CI, repair a broken export, trim or re-anchor a clip, encode a project contract config, migrate FBX/Mixamo exports, embed the checks in Rust. |
| [Mixamo tutorial](mixamo-tutorial.md) | Take a real Mixamo download end-to-end — download, convert, inspect, lint, fix, and grow a contract config with the built-in `mixamo` rig profile. |
| [Static asset workflow guide](static-asset-workflows.md) | Diagnose bounds and transform domains, preserve normal maps, bake supported static placement, attach explicit textures, and understand what still needs engine validation. |
| [Scaling glTF safely](scale.md) | Choose whole-document unit conversion or rest/bind reparameterization, understand the exact-source rewrite/proof transaction, and interpret its support boundary. |
| [cli.md](cli.md) | Look up a command, flag, or exit code. |
| [Material texture recipes](material-texture-recipes.md) | Attach explicit BaseColor, normal, metallic-roughness, and occlusion images during conversion with deterministic resizing and provenance evidence. |
| [Multi-source character assembly](character-assembly.md) | Combine an authoritative skinned base with exact takes and timeline windows from separate inputs, producing one deterministic GLB plus evidence. |
| [embedding.md](embedding.md) | Choose library crates and integration boundaries, then follow the embedded gate flow with the runnable [`embed`](../crates/animsmith/examples/embed.rs) example. |
| [docs.rs API references](https://docs.rs/animsmith-core) | Look up exact published Rust API contracts for [`animsmith-core`](https://docs.rs/animsmith-core), [`animsmith-gltf`](https://docs.rs/animsmith-gltf), [`animsmith-fbx`](https://docs.rs/animsmith-fbx), [`animsmith-engine`](https://docs.rs/animsmith-engine), and [`animsmith-report`](https://docs.rs/animsmith-report). |
| [output.md](output.md) | Parse versioned `--format json` reports, glTF animation-addressability inventories, and producer evidence in a pipeline, validated by the JSON Schema under [`schemas/`](schemas/). |
| [README](../README.md) | Install and quickstart, plus the check and configuration reference. |
| [DESIGN.md](../DESIGN.md) | Follow the architecture, check-catalog rationale, and roadmap. |
| [Scale proof calibration](scale-calibration.md) | Review the implementation-owned calibration sweep, provenance magnitudes, historical policy measurements, and reproducible command behind `appendix-d-v6`. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) / [DEVELOPMENT.md](../DEVELOPMENT.md) | Work on animsmith itself — contribution flow and development setup. |
| [RELEASING.md](../RELEASING.md) | Cut a release — the release-plz flow, the manual 0.1.0 bootstrap, and the published-doc-link policy. |
| [SUPPORT.md](../SUPPORT.md) / [SECURITY.md](../SECURITY.md) | Get help, file a bug, or report a vulnerability privately. |
| [research/](research/game-ready-animation-clips.md) | Read the dated research notes that inform the roadmap. |
