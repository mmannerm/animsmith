# Documentation

Every animsmith document, and the task it serves — this table is the
single routing surface; the other pages link back here rather than
re-listing each other.

| Document | Use it to… |
|---|---|
| [Why animsmith](why-animsmith.md) | Decide whether animsmith fits your team — what it is, why it exists, and what it is worth by role. The canonical home of the positioning case. |
| [Game-ready clips guide](game-ready-clips.md) | Understand what "game-ready" means and why a check fires — every runtime failure mode, mapped to the checks, repairs, and config that address it. |
| [Pipeline scenario guide](pipeline-scenarios.md) | Plan a raw-to-game-ready asset process — marketplace intake, mocap cleanup, outsourced acceptance, CI gating, and artifact storage. |
| [Examples cookbook](../examples/README.md) | Do the work, copy-paste style — gate exports in CI, repair a broken export, trim or re-anchor a clip, encode a project contract config, migrate FBX/Mixamo exports, embed the checks in Rust. |
| [cli.md](cli.md) | Look up a command, flag, or exit code. |
| [embedding.md](embedding.md) | Call the checks from Rust instead of the CLI, paired with the runnable [`embed`](../crates/animsmith/examples/embed.rs) example. |
| [output.md](output.md) | Parse the versioned `--format json` envelope in a pipeline, validated by the JSON Schema under [`schemas/`](schemas/). |
| [README](../README.md) | Install and quickstart, plus the check and configuration reference. |
| [DESIGN.md](../DESIGN.md) | Follow the architecture, check-catalog rationale, and roadmap. |
| [CONTRIBUTING.md](../CONTRIBUTING.md) / [DEVELOPMENT.md](../DEVELOPMENT.md) | Work on animsmith itself — contribution flow and development setup. |
| [research/](research/game-ready-animation-clips.md) | Read the dated research notes that inform the roadmap. |
