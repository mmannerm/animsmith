# Documentation

Reference material for animsmith. Start with the
[game-ready clips guide](game-ready-clips.md) for *why* the checks
exist, and the [examples cookbook](../examples/README.md) for runnable,
copy-into-your-project workflows; the pages here are the topic-by-topic
references those draw on.

## I want to…

| I want to… | Go to |
|---|---|
| Understand what "game-ready" means, and why a check fires | [Game-ready clips guide](game-ready-clips.md) |
| Gate animation exports in CI | [Cookbook §1 — a first CLI gate](../examples/README.md#1-a-first-cli-gate) |
| Repair a broken export | [Cookbook §2 — repairing an asset](../examples/README.md#2-repairing-an-asset) |
| Trim, extend, or re-anchor a clip | [Cookbook §3 — editing a clip](../examples/README.md#3-editing-a-clip) |
| Encode my project's animation contract | [Cookbook §4 — a project contract config](../examples/README.md#4-a-project-contract-config) |
| Migrate an FBX or Mixamo export to glTF | [Cookbook §5 — migrating an FBX export](../examples/README.md#5-migrating-an-fbx-export-default-features-only) |
| Call the checks from Rust | [Cookbook §6 — embedding](../examples/README.md#6-embedding-animsmith-as-a-library-gate) + [embedding.md](embedding.md) |
| Parse animsmith's JSON output in a pipeline | [output.md](output.md) |

## Reference pages

| Document | What it covers |
|---|---|
| [Game-ready clips guide](game-ready-clips.md) | What makes a clip game-engine friendly and why — every runtime failure mode, mapped to the checks, repairs, and config that address it. Start here for the why. |
| [Examples cookbook](../examples/README.md) | Runnable workflows — CLI gates, repair, clip edits, contract configs, FBX migration, and library embedding. Several double as CI/acceptance gates. |
| [cli.md](cli.md) | CLI reference: every command, flag, and exit code. |
| [embedding.md](embedding.md) | Driving the check catalog from Rust instead of the CLI, paired with the runnable [`embed`](../crates/animsmith/examples/embed.rs) example. |
| [output.md](output.md) | The versioned `--format json` envelope, with the JSON Schema under [`schemas/`](schemas/). |

See also the repository root: [README.md](../README.md) for an overview and
the configuration reference, [DESIGN.md](../DESIGN.md) for architecture and
roadmap, and [CONTRIBUTING.md](../CONTRIBUTING.md) /
[DEVELOPMENT.md](../DEVELOPMENT.md) for working on animsmith itself.
Dated research notes that inform the roadmap live under
[`research/`](research/game-ready-animation-clips.md).
