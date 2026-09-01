# Animation pack evidence appendix: Mixamo Locomotion Collection

> Companion report: [Technical report](mixamo-locomotion-collection.md)
>
> Evidence status: **partial** — this aggregation retains current constituent evidence only; licensed source archives, runtime tests, visual inspection, and collection-level compatibility tests remain external or unevaluated.
>
> Evaluation date: **2026-09-01**
>
> Current evaluator: **AnimSmith 0.10.0**
>
> Report format: **2**

This appendix records scrubbed current evidence only. The canonical [readiness ladder](../game-ready-clips.md#the-readiness-ladder) remains authoritative.

## Evaluation scope and provenance

| Field | Value |
|---|---|
| Pack/edition | Mixamo Locomotion Collection; local constituent revisions unknown |
| Vendor/source | Vendor identity observed in constituent delivered metadata; listing URLs not retained |
| Delivered scope | 249 extracted FBX files from nine constituent archive pairs; 231 manifest-declared motions retained as separate source metadata |
| Target use | Engine-neutral marketplace-intake aggregation |
| Target engines | Not evaluated |
| Target rigs/packs | Nine named constituent reports; target rig and cross-pack use not evaluated |
| Source manifest | Constituent source and extracted-inventory SHA-256 identities are scrubbed and retained externally |
| Evaluation manifest | Nine validated constituent manifests, each `urn:animsmith:skill:animation-pack-evaluation-manifest:1`; no new collection manifest or classification was generated in this rollup |
| Acquisition/license provenance | No controlling license/readme/terms evidence was delivered; not legal advice |

Included current reports: [Basic](mixamo-basic-locomotion.md), [Female Basic](mixamo-female-basic-locomotion.md), [Female](mixamo-female-locomotion.md), [Locomotion](mixamo-locomotion.md), [Longbow](mixamo-longbow-locomotion.md), [Magic](mixamo-magic-locomotion.md), [Male](mixamo-male-locomotion.md), [Pistol/Handgun](mixamo-pistol-handgun-locomotion.md), and [Rifle 8-Way](mixamo-rifle-8-way-locomotion.md). None are excluded from this nine-constituent rollup.

The evaluation manifest schema is `urn:animsmith:skill:animation-pack-evaluation-manifest:1`. The aggregate uses constituent-manifest file totals and separate manifest-declared-motion metadata. It does not infer a file-to-motion mapping, logical-motion classification, runtime set, or collection compatibility result.

### Evidence coverage

| Surface | Offered/delivered | Evaluated | Findings | Not evaluated and why |
|---|---:|---:|---:|---|
| Animation files | 249 | 249 | 0 baseline errors; 50 baseline warnings; 35,405 baseline notes | Constituent file-to-manifest mapping remains incomplete |
| Rigs/export variants | 9 constituent contexts | 9 lint-profile contexts | Mixamo profile used for constituent lint | Retarget behavior unavailable |
| AnimSmith baseline | 249 | 249 | 0 errors; 50 warnings; 35,405 notes | — |
| Declared contracts | 249 | 249 | In-place: 0 errors; 13 warnings. Root-motion: 52 stationary-root errors; 37 warnings | Archive-level declarations only; per-clip intent unavailable |
| Offline visual reports | 14 | 0 | 0 | Generated externally but not visually inspected |
| Engine import/playback | 4 runtimes | 0 | 0 | No engines/project supplied |
| Blend/mask/retarget | 1 collection scope | 0 | 0 | No collection runtime contract supplied |

### Claim legend

Claims use `observed-file`, `observed-animsmith`, and `not-evaluated` as defined by the assessment taxonomy.

## Evaluation manifest and taxonomy

### Canonical clip-role inventory

No collection-level canonical role inventory was refreshed. Each constituent's validated manifest preserves its own opaque `other-unknown` file units; the 231 manifest-declared motions remain separate metadata because source records were not linked to archive members.

| Canonical primary role | Logical motions | Delivered files | Evidence boundary |
|---|---:|---:|---|
| `idle-pose` | 0 | 0 | No fresh collection classification |
| `continuous-locomotion` | 0 | 0 | No fresh collection classification |
| `locomotion-transition` | 0 | 0 | No fresh collection classification |
| `airborne` | 0 | 0 | No fresh collection classification |
| `traversal` | 0 | 0 | No fresh collection classification |
| `action-interaction` | 0 | 0 | No fresh collection classification |
| `reaction-death` | 0 | 0 | No fresh collection classification |
| `emote-cinematic` | 0 | 0 | No fresh collection classification |
| `other-unknown` | 249 | 249 | One opaque unit per delivered FBX in the constituent manifests; no semantic collection classification asserted |
| **Total** | **249** | **249** | Derived from nine validated constituent manifests; 231 manifest-declared motions are separate metadata |

### Runtime-set inventory

No runtime sets were identified.

### Pipeline-stage coverage

| Stage | Coverage state | Evidence / remaining gate |
|---|---|---|
| Acquire | `partially-evaluated` | Constituent archive identities recorded externally; license terms unavailable |
| Preserve raw | `evaluated-clean` | Constituent archives were not modified; extraction was external |
| Inspect | `evaluated-clean` | AnimSmith 0.10.0 inspected every extracted FBX in each constituent baseline |
| Segment | `not-evaluated` | Constituent manifest-to-member clip linkage was not established |
| Root motion | `partially-evaluated` | Archive-variant declarations ran; per-clip ownership remains open |
| Conform | `not-evaluated` | No target contract authorized transforms |
| Validate | `partially-evaluated` | Constituent mechanical baseline and declarations ran; engine validation deferred |
| Optimize | `not-evaluated` | No optimization was authorized |
| Export | `not-evaluated` | No engine-facing export was evaluated |
| Gate/report | `partially-evaluated` | This report aggregates scrubbed current constituent evidence without new collection testing |

### Readiness evidence by clip set

| Role or runtime set | File-ready / clip-ready | Set-ready / rig-use | Runtime / acceptance boundary |
|---|---|---|---|
| Aggregate unclassified corpus | All 249 current baseline files loaded and have 0 mechanical errors; warnings and notes remain distinct hygiene evidence | Per-clip intent, loops, variants, and ownership remain incomplete | Engine, retarget, contact, visual, gameplay, and cross-pack acceptance not evaluated |

### Validation-profile status

| Validation profile | Selection | Result / next evidence |
|---|---|---|
| Marketplace intake | `selected` — `observed-pack-capability` | Constituent external inventories and baselines completed; license evidence remains unavailable |
| Blended locomotion | `selected` — `observed-pack-capability` | No collection runtime sets or engine blend tests were refreshed |
| Root-motion controller | `selected` — `observed-pack-capability` | Archive-level declarations differ; choose per-clip policy in the target project |
| State-machine transitions | `not-selected` | No target state-machine contract supplied |
| Layered upper body/weapons | `not-selected` | No mask or weapon contract supplied |
| Traversal/environment | `not-selected` | No target traversal contract supplied |
| Contact actions/interactions | `not-selected` | No target contact contract supplied |
| Retargeted/customizable characters | `not-selected` | No target rig or retargeter supplied |
| Motion matching/search | `not-selected` | No search runtime contract supplied |
| Networked movement | `not-selected` | No networked movement contract supplied |
| Runtime performance | `not-selected` | No target hardware/runtime contract supplied |

## Pack inventory and content evidence

The nine current constituent inventories reconcile to 249 FBX files and 231 manifest-declared motions: Basic 12/10, Female Basic 20/18, Female 18/16, Locomotion 20/18, Longbow 22/20, Magic 27/25, Male 18/16, Pistol/Handgun 29/27, and Rifle 8-Way 83/81 (files/motions). The archive member count exceeds each constituent's manifest motion count by two: one non-motion/reference asset per archive variant. Delivered metadata declares 30 fps FBX 2019 export preferences, not an engine import result.

## Mechanical baseline

| Finding/check | Affected scope | Potential impact | Evidence |
|---|---|---|---|
| No mechanical errors | 249 files | File-ready mechanical health only | observed-animsmith, AnimSmith 0.10.0; nine fresh empty baselines |
| `duration-sanity` warnings | 50 findings | No-track assets/clips and channel-end mismatches require semantic classification or review | observed-animsmith, AnimSmith 0.10.0 |
| `constant-track` notes | 35,405 notes across the corpus | Hygiene evidence, not authorization to rewrite | observed-animsmith, AnimSmith 0.10.0 |

## AnimSmith remediation evidence

| Source issue | Operation/declarations | Result | Independent verification | Remaining caveat |
|---|---|---|---|---|
| Archive-level movement ownership ambiguity | Current in-place and root-motion XZ controls | In-place: 0 errors, 13 warnings. Root-motion: 52 stationary-root errors, 37 warnings | Nine current constituent external runs; control findings are not a source repair | Per-clip ownership, controller integration, and all transforms remain outside this rollup |
| FBX remediation | Current FBX `fix --dry-run` representatives | Safe refusal: `fix` accepts only glTF/GLB | Captured exit 2 and stderr in the constituent evidence | No conversion or transform was authorized |

## Engine procedures and evidence

| Runtime | Version | Procedure | Observed result | Remaining gate |
|---|---|---|---|---|
| Unity | unspecified | Not run at collection scope | not-evaluated | Exact import/controller test |
| Unreal Engine | unspecified | Not run at collection scope | not-evaluated | Exact import/retarget test |
| Godot | unspecified | Not run at collection scope | not-evaluated | Exact import/AnimationTree test |
| Bevy | unspecified | Not run at collection scope | not-evaluated | Exact loader/graph test |

## Rig, masking, and compatibility evidence

| Pack/rig/set pair | Skeleton/retarget | Scale/axes | Root policy | Timing/blend | Overall evidence |
|---|---|---|---|---|---|
| Nine constituent corpora / target project | Mixamo profile resolved for constituent lint only | Not evaluated | Archive labels do not establish per-clip ownership | Not evaluated | unknown |
| Constituent-to-constituent pair | No overlapping-path or skeleton/reference-rig comparison refreshed | Not evaluated | Not evaluated | Not evaluated | unknown |

## Limitations and unknowns

1. This is an aggregation, not a new collection evaluation: no file-to-motion mapping, semantic role classification, runtime set, remediation, engine import, blend, retarget, masking, visual/contact, performance, or cross-pack test was performed.
2. License terms, target engine, target controller, target rig, and artistic/gameplay acceptance remain unavailable.

## Changes between AnimSmith versions

AnimSmith 0.10.0 — The nine constituent reports were refreshed and revalidated with the official evaluator, fresh preflight, current empty baselines, and current archive-variant controls. Current evidence uses output schema v19 and measurements schema v18.

AnimSmith 0.7.0 — Prior collection assertions, output schema v17, measurements schema v16, and retained evidence digests are historical only and superseded by the current constituent evidence.

## Reproduction

Official AnimSmith `0.10.0` provenance is tag `v0.10.0`, peeled source commit `db91d8dda3326f97f581d4d62104d928caec383f`, binary SHA-256 `2052ce64eda53d5037b305561dd0287209719d743b0a4051552e197fbfe4a387`, output schema v19, and measurements schema v18. Before exhaustive constituent execution, external evidence captured `animsmith --version`, top-level help, required-command help, and representative FBX admission; `inspect`, `measure`, `lint`, and `report` each exited 0. Each constituent then used an external explicit empty baseline config for per-file `inspect`, batch `measure --format json`, and `lint --format json`/Markdown, followed by archive-variant `lint --format json` controls. All baseline commands exited 0; root-motion controls exited 1 for findings; FBX `fix --dry-run` exited 2 because `fix` accepts only glTF/GLB. Configs, inventories, command outputs, exit codes, HTML reports, and SHA-256 digests remain outside the repository.

## Sources

- The nine linked constituent reports and external AnimSmith 0.10.0 evidence; no licensed payloads are published.
