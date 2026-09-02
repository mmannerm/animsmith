# Documentation

## Start here

Two doors into AnimSmith, and one command behind both:

```console
$ animsmith lint walk.glb
```

- **Artists and animators** — catch a bad export while the DCC is still open.
  [Install](install.md), [first lint in 60 seconds](first-lint.md),
  [your first report](first-report.md), then
  [from export to handoff](animation-author-workflow.md).
- **Game developers** — know what a pack really contains and gate every
  re-export. [Install](install.md), [first lint in 60 seconds](first-lint.md),
  [your first report](first-report.md), then
  [from pack to engine gate](game-developer-intake-workflow.md).

Something specific already looks wrong in the engine? Start from the
[symptoms](symptoms/README.md).

## Every page

Find what you need by task — each page below owns one job. The Category
column names the documentation site's part and optional `Part › Group`, so
this table and the site sidebar carry the same structure in the same order.

| Document | Use it to… | Category |
|---|---|---|
| [Why animsmith](why-animsmith.md) | Decide whether animsmith fits your team — what it is, why it exists, and what it is worth by role. The canonical home of the positioning case. | Start |
| [Install](install.md) | Get the binary — prebuilt archive, `cargo install`, or the pure-Rust glTF-only build — and confirm it runs. | Start |
| [First lint in 60 seconds](first-lint.md) | See a finding, repair what is mechanically safe, and watch a declared contract catch a popped loop, on two committed sample clips. | Start |
| [Your first report](first-report.md) | Turn findings into skeleton playback, charts and one shareable HTML file, including the before/after comparison and the evidence-only form. | Start |
| [Mixamo tutorial](mixamo-tutorial.md) | Take a real Mixamo download end-to-end — download, convert, inspect, lint, fix, and grow a contract config with the built-in `mixamo` rig profile. | Start |
| [Symptom index](symptoms/README.md) | Start from what you see in the engine and route it to the page, check, repair and config that address it. | Symptoms |
| [The pose flickers, spins, or explodes](symptoms/pose-flickers.md) | Repair the rotation representation itself: non-unit keys, hemisphere flips, non-finite values, and what `fix` restores losslessly. | Symptoms |
| [The clip is the wrong length or freezes at the end](symptoms/wrong-length.md) | Find the channel that stopped early or the export range that drifted, and pin the duration and frame grid the clip owes you. | Symptoms |
| [The loop pops](symptoms/loop-pops.md) | Fix a looping clip that jumps or hitches at the wrap: what the seam checks measure, the before/after evidence, and the loop contract. | Symptoms |
| [The character glides or runs in place](symptoms/character-glides.md) | Settle whether gameplay or the animation owns horizontal movement, and hold the clip's measured travel to that decision. | Symptoms |
| [Feet skate when clips blend](symptoms/blend-skate.md) | Hold a directional set to one stride phase and one timing surface, and know which member is out. | Symptoms |
| [Feet slide within a clip](symptoms/feet-slide.md) | Fix a planted foot that skates during stance: the sampled stance intervals, the declared speed, and who owns the repair. | Symptoms |
| [A limb is T-posed, or a bone never moves](symptoms/limb-frozen.md) | Separate a bone that is absent from one that is keyed but frozen, and both from a clip authored against another bind. | Symptoms |
| [Files disagree about skeleton or clip identity](symptoms/identity-mismatch.md) | Keep the exact `(file, clip)` identity of a pack instead of trusting a repeated display name, and decide what a retarget still owes you. | Symptoms |
| [The file is bloated, or the retargeter chokes](symptoms/file-bloat.md) | Decide what is redundant exported data and what is authored scale, then remove only the part a transform can prove. | Symptoms |
| [Game-ready clips guide](game-ready-clips.md) | Understand what "game-ready" means — the staged [readiness ladder](game-ready-clips.md#the-readiness-ladder), who owns each level, and the complete check-to-symptom table behind the pages above. | Symptoms |
| [Animation troubleshooting](animation-troubleshooting.md) | Route a visible runtime symptom to the page that owns it, and answer the two that are not about a clip: a loader refusal and an unaddressable clip. | Symptoms |
| [For artists: from export to handoff](animation-author-workflow.md) | Take an authored export from immutable source through evidence-backed candidate handoff, without treating a safe mechanical edit as artistic approval. | Workflows |
| [For game developers: from pack to engine gate](game-developer-intake-workflow.md) | Take a pack or collection from inventory through an exact engine profile and an engine-observed gate; includes the complete Bevy 0.19.0 path. | Workflows |
| [Pipeline scenario guide](pipeline-scenarios.md) | Plan a raw-to-game-ready asset process — marketplace intake, mocap cleanup, outsourced acceptance, CI gating, and artifact storage. | Workflows |
| [Unity 6000.3 profile guide](engine-profile-unity.md) | Configure the exact Unity Generic/Humanoid profiles, importer advice, root-motion choices, and scale boundary. | Workflows › Engine profiles |
| [Unreal Engine 5.8 profile guide](engine-profile-unreal.md) | Map FBX animation, Skeleton, frame, unit, root-motion, and scale concerns to AnimSmith evidence without inventing importer settings. | Workflows › Engine profiles |
| [Godot 4.7 profile guide](engine-profile-godot.md) | Plan scene import, retargeting, animation slicing, root scale, and the current profile's explicit prediction boundary. | Workflows › Engine profiles |
| [Bevy 0.19.0 profile guide](engine-profile-bevy.md) | Generate exact `Animation{i}` selector evidence and keep runtime loading, graph, target, and scale responsibilities explicit. | Workflows › Engine profiles |
| [glTF and generic runtime guide](engine-profile-gltf-runtime.md) | Use the engine-neutral contract for custom runtimes, glTF units, source identity, scale repair, and downstream validation. | Workflows › Engine profiles |
| [Examples cookbook](../examples/README.md) | Do the work, copy-paste style — gate exports in CI, repair a broken export, trim or re-anchor a clip, encode a project contract config, migrate FBX/Mixamo exports, embed the checks in Rust. | Workflows |
| [Static asset workflow guide](static-asset-workflows.md) | Diagnose bounds and transform domains, preserve normal maps, bake supported static placement, attach explicit textures, and understand what still needs engine validation. | Workflows › Advanced workflows |
| [Scaling glTF safely](scale.md) | Choose whole-document unit conversion or rest/bind reparameterization, understand the exact-source rewrite/proof transaction, and interpret its support boundary. | Workflows › Advanced workflows |
| [Material texture recipes](material-texture-recipes.md) | Attach explicit BaseColor, normal, metallic-roughness, and occlusion images during conversion with deterministic resizing and provenance evidence. | Workflows › Advanced workflows |
| [Multi-source character assembly](character-assembly.md) | Combine an authoritative skinned base with exact takes and timeline windows from separate inputs, producing one deterministic GLB plus evidence. | Workflows › Advanced workflows |
| [Collection contract extensions](collection-contracts.md) | Read the contact-fragment, foot-cycle planner, and transition-family contracts, including strict manifest and contact-evidence bindings. | Workflows › Advanced workflows |
| [Configuration reference](configuration-reference.md) | Complete `animsmith.toml` tables, keys, defaults, precedence, validation, globs, rig profiles, and engine settings. | More › Reference |
| [Built-in check reference](built-in-checks.md) | Look up every registered built-in check: current IDs, default findings, prerequisites, config keys, gap semantics, and remediation boundaries. | More › Reference |
| [CLI reference](cli.md) | Look up a command, flag, or exit code. | More › Reference |
| [Machine-readable output](output.md) | Parse versioned `--format json` reports, glTF animation-addressability inventories, engine import advice, and producer evidence in a pipeline, validated by the JSON Schema under [`schemas/`](schemas/). | More › Reference |
| [Commercial-pack evaluation guide](commercial-pack-evaluations.md) | Read the maintained Mixamo and Protofactor technical-report/evidence pairs and separate their scoped findings from project acceptance. | More › Pack evaluations |
| [Animation-pack evaluation reports](reports/README.md) | Review evidence-backed marketplace-pack assessments and the reporting convention for constituent packs and collection-level cross-pack summaries. | More › Pack evaluations |
| [Embedding guide](embedding.md) | Choose library crates and integration boundaries, then follow the embedded gate flow with the runnable [`embed`](../crates/animsmith/examples/embed.rs) example. | More › Rust integration |
| [docs.rs API references](https://docs.rs/animsmith-core) | Look up exact published Rust API contracts for [`animsmith-core`](https://docs.rs/animsmith-core), [`animsmith-gltf`](https://docs.rs/animsmith-gltf), [`animsmith-fbx`](https://docs.rs/animsmith-fbx), [`animsmith-engine`](https://docs.rs/animsmith-engine), and [`animsmith-report`](https://docs.rs/animsmith-report). | More › Rust integration |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | Work on animsmith itself — contribution flow. | More › Project and contributing |
| [DEVELOPMENT.md](../DEVELOPMENT.md) | Set up a checkout and run the local verification commands. | More › Project and contributing |
| [RELEASING.md](../RELEASING.md) | Cut a release — the release-plz flow, the manual 0.1.0 bootstrap, and the published-doc-link policy. | More › Project and contributing |
| [SUPPORT.md](../SUPPORT.md) | Get help or file a bug. | More › Project and contributing |
| [SECURITY.md](../SECURITY.md) | Report a vulnerability privately. | More › Project and contributing |
| [DESIGN.md](../DESIGN.md) | Follow the architecture, check-catalog rationale, and roadmap. | More › Project and contributing |
| [Scale proof calibration](scale-calibration.md) | Review the implementation-owned calibration sweep, provenance magnitudes, historical policy measurements, and reproducible command behind `appendix-d-v6`. | More › Project and contributing |
| [research/](research/game-ready-animation-clips.md) | Read the dated research notes that inform the roadmap. | More › Project and contributing |
