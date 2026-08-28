# Animation pack evidence appendix: Mixamo Locomotion Collection

> Companion report: [Technical report](mixamo-locomotion-collection.md)
>
> Evidence status: **partial** — source archives were evaluated externally; licensing, runtime, and visual evidence remain unavailable.
>
> Evaluation date: **2026-08-26**
>
> Current evaluator: **AnimSmith 0.7.0**
>
> Report format: **2**

This appendix records scrubbed evidence only. The canonical [readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Mixamo Locomotion Collection; local revision unknown |
| Vendor/source | Vendor identity observed in delivered metadata; listing URL not retained |
| Delivered scope | 249 extracted FBX files from paired archive variants |
| Target use | Engine-neutral marketplace intake |
| Target engines | Not evaluated |
| Target rigs/packs | One declared character identity; cross-pack use not evaluated |
| Source manifest | Scrubbed SHA-256 retained externally |
| Evaluation manifest | `mixamo-locomotion-collection.manifest.json`; SHA-256 `9601129c4237e5b331fd7de8c6c705c7bb8dcd702a219d31c250a8ce57b77cf1`; schema `urn:animsmith:skill:animation-pack-evaluation-manifest:1` |
| Acquisition/license provenance | No controlling license/readme/terms evidence was delivered; not legal advice |

The evaluation manifest schema is `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. The validated rollup models every delivered FBX as an opaque `other-unknown` unit because vendor records were not linked to archive members; 231 remains separate source metadata.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 249 | 249 | 50 baseline warnings | Semantic file-to-manifest mapping incomplete |
| Rigs/export variants | 1 | 1 | Mixamo profile used | Retarget behavior unavailable |
| AnimSmith baseline | 249 | 249 | 0 errors | — |
| Declared contracts | 249 | 249 | 52 variant-contract findings | Per-clip intent unavailable |
| Offline visual reports | 18 | 0 | 0 | Generated externally but not visually inspected |
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
| `other-unknown` | 249 | 249 | One opaque unit per delivered FBX; no semantic classification asserted |
| **Total** | **249** | **249** | Validated scrubbed external manifest |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Local archive identity recorded; license terms unavailable |
| Preserve raw | `evaluated-clean` | Archives were not modified; extraction was external |
| Inspect | `evaluated-clean` | AnimSmith 0.7.0 inspected every extracted FBX |
| Segment | `not-evaluated` | Manifest-to-member clip linkage was not established |
| Root motion | `partially-evaluated` | Variant-level XZ contracts ran; per-clip ownership remains open |
| Conform | `not-evaluated` | No target contract authorized transforms |
| Validate | `partially-evaluated` | Mechanical baseline and variant contracts ran; engine validation deferred |
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

The external inventory contains 249 FBX files and 231 manifest-declared motions. Archive member counts exceed manifest motion counts by one non-motion/reference asset per archive. Delivered metadata declares 30 fps FBX 2019 export preferences, but this is not a target-engine import result.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| No mechanical errors | 249 files | File-ready mechanical health only | observed-animsmith |
| `duration-sanity` warnings | 50 files | No-track assets/clips require semantic classification | observed-animsmith |
| `constant-track` notes | Entire corpus | Hygiene evidence, not authorization to rewrite | observed-animsmith |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Variant-level ownership mismatch | FBX `fix --dry-run` representative | Safe refusal: fix accepts only glTF/GLB | Captured exit 2 and stderr | No conversion or transform was authorized |

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

AnimSmith 0.7.0 — Initial evaluation; no earlier AnimSmith comparison.

## Reproduction

AnimSmith `0.7.0` (tag `v0.7.0`, commit `461ac8a4f6bb368eb8637471a796f13eeb647140`, binary SHA-256 `01a501999c91d93abfb32b1f48241fccc70914fac27c9a650c31df44262578d8`, output schema v17, measurements schema v16) was captured with external empty-baseline and variant contract configs. The immutable archive inventory is bound by SHA-256 `232d3bc7f425edbae0f9cbfed358120780d03d13aad04d7f889fb3d7d2dbf594`; the extracted-corpus inventory by `5afd0fdf031272f20220ee19030fc695cb91dcb408e5aa835e43465b7867f1e2`. Portable evidence digests: measurements `aa0df4c34e2751d4187fdbdf88ce37c58902bd828bba1fe0c2fbee954d866d97`; baseline lint `c1a1a35b1ec7558f09c93eb2b1f4f034fcb1a2d7e564f7c33fa798da8796946e`; in-place contract `ed6aeffc54e453290d19d71ebae902016700517fc368b8bdf869cd9f37d556e7`; root-motion contract `ee4efa06c002ab0e5e1a55a26ed783472ff6bdddd2a6fb6208b014e482118dbe`; nine-pack summary `b4201bad2f4def7f157f5b68d16a5c07c650302938d9589547eb529cf2823fdc`. All payloads remain external.

## Sources

- Delivered metadata and external AnimSmith evidence; no licensed payloads are published.
