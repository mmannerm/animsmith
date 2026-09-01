# Animation pack evidence appendix: Mixamo Female Locomotion

> Companion report: [Technical report](mixamo-female-locomotion.md)
>
> Evidence status: **partial** — source archives were evaluated externally; licensing, runtime, and visual evidence remain unavailable.
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**

This appendix records scrubbed evidence only. The canonical [readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Mixamo Female Locomotion; local revision unknown |
| Vendor/source | Vendor identity observed in delivered metadata; listing URL not retained |
| Delivered scope | 18 extracted FBX files from paired archive variants |
| Target use | Engine-neutral marketplace intake |
| Target engines | Not evaluated |
| Target rigs/packs | One declared character identity; cross-pack use not evaluated |
| Source manifest | Scrubbed SHA-256 retained externally |
| Evaluation manifest | `mixamo-female-locomotion.manifest.json`; SHA-256 `548ee8c16b12a5564d0f422a51dc35610a7721b817d0cdb5c4e08c1f38350716`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | No controlling license/readme/terms evidence was delivered; not legal advice |

The evaluation manifest schema is `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. The validated manifest models every delivered FBX as an opaque `other-unknown` unit because vendor records were not linked to archive members; 16 remains separate source metadata.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 18 | 18 | 0 errors; 4 baseline warnings | Semantic file-to-manifest mapping incomplete |
| Rigs/export variants | 1 | 1 | Mixamo profile used | Retarget behavior unavailable |
| AnimSmith baseline | 18 | 18 | 0 errors; 4 warnings; 2,559 notes | — |
| Declared contracts | 18 | 18 | 4 root-motion stationary-root errors; 2 in-place and 2 root-motion duration warnings | Per-clip intent unavailable |
| AnimSmith visual reports | 2 | 0 | 0 | Generated externally but not visually inspected |
| Engine import/playback | 4 | 0 | 0 | No engines/project supplied |
| Blend/mask/retarget | 1 | 0 | 0 | No runtime contract supplied |

### Claim legend

Claims use observed-file, observed-animsmith, and not-evaluated as defined by the assessment taxonomy.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | Not classified |
| `continuous-locomotion` | 0 | 0 | Not classified |
| `locomotion-transition` | 0 | 0 | Not classified |
| `airborne` | 0 | 0 | Not classified |
| `traversal` | 0 | 0 | Not classified |
| `action-interaction` | 0 | 0 | Not classified |
| `reaction-death` | 0 | 0 | Not classified |
| `emote-cinematic` | 0 | 0 | Not classified |
| `other-unknown` | 18 | 18 | One opaque unit per delivered FBX; no semantic classification asserted |
| **Total** | **18** | **18** | Validated scrubbed external manifest |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Local archive identity recorded; license terms unavailable |
| Preserve raw | `evaluated-clean` | Archives were not modified; extraction was external |
| Inspect | `evaluated-clean` | AnimSmith 0.10.0 inspected every extracted FBX |
| Segment | `not-evaluated` | Manifest-to-member clip linkage was not established |
| Root motion | `partially-evaluated` | In-place XZ contract has 0 errors and 2 duration warnings; root-motion XZ contract has 4 stationary-root errors and 2 duration warnings; per-clip ownership remains open |
| Conform | `not-evaluated` | No target contract authorized transforms |
| Validate | `partially-evaluated` | Mechanical baseline and current variant contracts ran; engine validation deferred |
| Optimize | `not-evaluated` | No optimization was authorized |
| Export | `not-evaluated` | No engine-facing export was evaluated |
| Gate/report | `partially-evaluated` | This scrubbed report retains the evidence boundary |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Unclassified delivered corpus | Mechanical baseline completed with no errors | Per-clip intent, loops, and ownership incomplete | Engine, retarget, contact, visual, and gameplay acceptance not evaluated |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `observed-pack-capability` | External inventory and baseline completed; license evidence remains unavailable |
| Blended locomotion | `selected` — `observed-pack-capability` | Runtime sets and engine blend tests not evaluated |
| Root-motion controller | `selected` — `observed-pack-capability` | Variant-level ownership differs; choose per-clip policy in the target project |
| State-machine transitions | `not-selected` | No target state-machine contract supplied |
| Layered upper body/weapons | `not-selected` | No mask or weapon contract supplied |
| Traversal/environment | `not-selected` | No target traversal contract supplied |
| Contact actions/interactions | `not-selected` | No target contact contract supplied |
| Retargeted/customizable characters | `not-selected` | No target rig or retargeter supplied |
| Motion matching/search | `not-selected` | No search runtime contract supplied |
| Networked movement | `not-selected` | No networked movement contract supplied |
| Runtime performance | `not-selected` | No target hardware/runtime contract supplied |

## Pack inventory and content evidence

The external inventory contains 18 FBX files and 16 manifest-declared motions. Archive member counts exceed manifest motion counts by one non-motion/reference asset per archive. Delivered metadata declares 30 fps FBX export preferences, but this is not a target-engine import result.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| No mechanical errors | 18 files | File-ready mechanical health only | observed-animsmith, AnimSmith 0.10.0 |
| `duration-sanity` warnings | 4 findings | Two no-track clips and two channel-end mismatches require semantic classification or review | observed-animsmith, AnimSmith 0.10.0 |
| `constant-track` notes | Entire corpus | Hygiene evidence, not authorization to rewrite | observed-animsmith |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Variant-level ownership mismatch | FBX `fix --dry-run` representative | Safe refusal: fix accepts only glTF/GLB | Captured exit 2 and stderr under AnimSmith 0.10.0 | No conversion or transform was authorized |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | Not run | not-evaluated | Exact import/controller test |
| Unreal Engine | unspecified | Not run | not-evaluated | Exact import/retarget test |
| Godot | unspecified | Not run | not-evaluated | Exact import/AnimationTree test |
| Bevy | unspecified | Not run | not-evaluated | Exact loader/graph test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Delivered corpus / target project | Mixamo profile resolved for lint only | Not evaluated | Archive variant labels insufficient per clip | Not evaluated | unknown |

## Limitations and unknowns

1. License, target engine, project controller, runtime-set membership, source-to-manifest member mapping, retargeting, visual/contact quality, and cross-pack compatibility remain unverified.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — Official `v0.10.0` rerun revalidated output schema v19 and measurements schema v18: 18/18 FBX admitted, baseline 0 errors, and the root-motion declaration found 4 stationary-root errors. AnimSmith 0.7.0 (output schema v17; measurements schema v16) is retained historical evidence only.

## Reproduction

working-tree state is N/A (official release artifact).

Reproduction used the official release artifact: URL `https://github.com/mmannerm/animsmith/releases/download/v0.10.0/animsmith-v0.10.0-x86_64-unknown-linux-gnu.tar.gz`; archive SHA-256 `8de4f97949fbc61fc3aec1d5f22272735ffe06937a0fea5c998cb3e0f639c662`; member `animsmith-v0.10.0-x86_64-unknown-linux-gnu/animsmith`; binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`. Official tag is `v0.10.0`, peeled commit `db91d8dda3326f97f581d4d62104d928caec383f`; working-tree state `N/A`; compiled features `fbx,report`; output schema v19 and measurements schema v18.

Required evaluator checks succeeded for `--version`, top-level `--help`, `inspect --help`, `measure --help`, `lint --help`, and `report --help`, and representative FBX admission succeeded. Safe normalized preflight status is `preflight-status-v1`, SHA-256 `739930cd9c04189be3ffe1d3f7381800898d8d4b35c75c9a519e57f6cfad1fad`; available help digests are version `35bb24c48d2e2fcf9ae5753338f09722ee50f9c002d00df7d0d30afa5d2ac4a0`, top-level `905f42d113783490bc8f3dea6bcda6bfc648a7e2f29b71fec99c41e6701099c8`, inspect `c45acaabc9e357af7c2c6265a18dccbb67b6cf1cb7dc45263652e0f5fa922c0b`, measure `cca0b44510c65ee671ac13b591712b6c67f6b1a4f2efdb0aca4ba26293e3d169`, lint `e216ccd638df639853687025f4eaab35e03c32d306b0dbcd0cb2c5c19ff53def`, and report `8298b2c9358bc796362eae5aeae22d86c9a06843c0dec7eb0f06cee82b27ad03`. External empty-baseline and variant-contract configs, inventories, command outputs, exit codes, and remaining digests are retained outside the repository.

## Sources

- Delivered metadata and external AnimSmith 0.10.0 evidence; no licensed payloads are published.
