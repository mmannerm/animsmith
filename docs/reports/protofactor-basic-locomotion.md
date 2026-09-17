# Animation pack evaluation: Protofactor Basic Locomotion Animset

> Technical verdict: **Usable with conditions**
>
> Evaluation completeness: **partial** — current mechanical and declared-contract evidence is exhaustive, with a fresh bounded Unity source import/mixer probe; visual and gameplay acceptance remain open.
>
> Confidence: **medium**
>
> Evaluation date: **2026-09-16**
>
> Current evaluator: **AnimSmith 0.14.0**
>
> Report format: **2**
>
> Detailed evidence: [Protofactor Basic Locomotion evidence](protofactor-basic-locomotion-evidence.md)

## Technical decision

**For a kinematic character: a useful locomotion foundation, conditional on phase and loop cleanup.** Start with the three in-place direction sets below. Do not put every delivered movement into one blend tree: cover, jumps, throws, and turns need state-specific selection and ownership. Equal duration within each ring helps synchronization but does not prove foot contacts or crossfades.

`observed-animsmith`: the official 0.14.0 evaluator inspected and measured all 179 FBXs; 58/177 declared-contract files pass and 119 fail per output format. Conditions below distinguish source findings, evaluator policy and untested gameplay. No remediation candidate is promoted.

## Capability coverage

### Complete core

- No complete gameplay core has been validated on a target game controller. Candidate content and integration scope are listed below; completed file checks do not establish gameplay completeness.

### Partial supporting gameplay

- Three complete eight-direction source selections support proposed walk/run/crouch trees. Cover, turn, jump and action families need separate reviewed states; start/stop, contact and controller behavior remain open.

### Absent

- No current evidence establishes additive, first-person, paired interaction, engine acceptance, or artistic readiness.

## Runtime sets and authored motion

New evaluator-selected generic scenarios: current source bytes establish the exact members, take names, durations, and measured roots below. Names suggest gameplay roles; topology and semantic intent are hypotheses requiring clip review. These are not reconstructed historical manifests or measured collection-output sets. Every listed member uses `Take 001`; full identities are in the external selected-set ledger.

| Set/profile | Role or coordinate | Exact members | Variant/type | Timing or motion | Runtime contract |
|---|---|---|---|---|---|
| Walk eight directions | Proposed forward (0,1) | `Humanoid@WalkForwardUnarmed2.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed forward-left (-1,1) | `Humanoid@WalkForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed left (-1,0) | `Humanoid@WalkLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back-left (-1,-1) | `Humanoid@WalkBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back (0,-1) | `Humanoid@WalkBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed back-right (1,-1) | `Humanoid@WalkBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed right (1,0) | `Humanoid@WalkRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Walk eight directions | Proposed forward-right (1,1) | `Humanoid@WalkForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.333 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward (0,1) | `Humanoid@RunForward2Unarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward-left (-1,1) | `Humanoid@RunForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed left (-1,0) | `Humanoid@RunLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back-left (-1,-1) | `Humanoid@RunBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back (0,-1) | `Humanoid@RunBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed back-right (1,-1) | `Humanoid@RunBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed right (1,0) | `Humanoid@RunRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Run eight directions | Proposed forward-right (1,1) | `Humanoid@RunForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=0.667 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward (0,1) | `Humanoid@CrouchForwardUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward-left (-1,1) | `Humanoid@CrouchForwardLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed left (-1,0) | `Humanoid@CrouchLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back-left (-1,-1) | `Humanoid@CrouchBackwardsLeftUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back (0,-1) | `Humanoid@CrouchBackwardsUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed back-right (1,-1) | `Humanoid@CrouchBackwardsRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed right (1,0) | `Humanoid@CrouchRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |
| Crouch eight directions | Proposed forward-right (1,1) | `Humanoid@CrouchForwardRightUnarmed.fbx::Take 001` | set_type=directional-blend | duration=1.500 s | loop=unknown; movement=controller; contact=not-evaluated |

## Integration recipe

1. **Members/topology:** `topology=directional-blend`; create separate walk, run, and crouch two-parameter trees from the listed members. Use local horizontal velocity for direction; handle standing/crouching and cover as states.
2. **Timing/synchronization:** `sync=normalized-phase`; compare the same support-foot phase across neighbors. The current unpromoted gait-anchor candidates reduce phase spread, but recheck loop closure and sweep intermediate weights before adoption.
3. **State ownership:** `owner=controller`; the selected in-place roots have no measured travel. Let the kinematic controller own translation/collision and choose playback speed from observed stride/contact tests; zero root speed is not a usable speed threshold. Give RM actions an explicit separate movement policy.
4. **Composition constraints:** `composition=full-body`; preserve authored lower-body/torso coupling initially. Weapon layers need an explicit spine mask, reference-pose convention, and planted-foot test.
5. **Acceptance gate:** `gate=engine-and-visual-review`; test stops, reversals, diagonal transitions, slopes, and crouch changes on the target character. Source mixer execution does not validate exported candidates.

## Technical issue register

Severity describes impact as delivered for the stated use. Residual status is explicit; untested risks are not confirmed artist defects.

| ID | Severity | Problem and impact | Primary owner | Current action | Future AnimSmith potential | Evidence/status |
|---|---|---|---|---|---|---|
| BL-TIME | major | Twelve cover/throw files contain 36 negative-time findings; an importer can trim or interpret their pre-roll differently. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Re-export deliberate clip start/range, or review the twelve declared slice candidates. All time findings disappear; ten candidates also pass their declared lint. Sources remain unchanged. Candidate time-ordering residual is mechanically cleared in all twelve; two retain other lint findings. All twelve remain unpromoted pending contact/engine review. | Explicit range slicing exists; it does not recover missing artistic intent. | Baseline affected-file list and slice verification; `observed-animsmith`. |
| BL-PHASE | major | Walk/run/crouch direction sets have phase spread 0.6598/0.4630/0.7156 cycles; mismatched support feet can produce foot sliding when blended. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | artist-author | Align authored cycles to a common foot phase. Current anchored spreads are 0.0724/0.0938/0.0502 cycles in the same set order. Review 24 gait-anchor candidates; 22 still fail declared lint. Phase disagreement is mechanically reduced in candidates; residual loop/contact risk remains major for automatic adoption. No output promoted. | Current gait anchoring helps a measured component; contact and loop corrections still need validation. | Exact sets and source/output phase measurements; `observed-animsmith`, gameplay effect `inferred`. |
| BL-ROOT | major | Fourteen files fail the declared in-place policy; a controller that also moves the character may double-apply displacement. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | engine-config | Select a documented root policy per state and inspect those measured trajectories. Do not infer in-place from missing RM suffix alone. Accept only after displacement/collision tests; residual unresolved. | Root measurements identify the conflict; the project must choose movement ownership. | Declared contract ledger lists exact files; `observed-animsmith`. |
| BL-TRACK | minor | Constant tracks inflate storage; note counts alone do not establish runtime cost. [Readiness guidance](../game-ready-clips.md#the-readiness-ladder). | animsmith-current-declared | Three prune candidates pass baseline lint. Accept only after required tracks, masks and engine playback remain equivalent; residual unverified. | Current pruning is available; no measured frame-time improvement is claimed. | Three representative outputs; `observed-animsmith`. |

## Engine status

| Runtime | Evidence level | Technical result | Remaining gate |
|---|---|---|---|
| Unity 6000.5.8f1 | observed-engine | Source inventory: 177 Humanoid clips; 6/6 sample and 3/3 mixer executions. A separate named cross-pack subset has finite-pose/fixed-root assertions (26 total checks across five packs). | Inventory probe is execution-only; cross-pack probe adds finite poses, not visual/contact quality. Controller, candidate-output and build acceptance remain open. |
| Unreal Engine unspecified | not-evaluated | No current import or playback run. | Import, retarget, graph, and build tests. |
| Godot unspecified | not-evaluated | No current conversion, import, or playback run. | Conversion/import, graph, and export tests. |
| Bevy unspecified | not-evaluated | No current glTF handoff or runtime run. | Conversion, addressability, runtime, and performance tests. |

## Fit and limitations

Basic + Injured is a plausible state switch between healthy and injured locomotion; preserve each injury style and calibrate speed separately. Basic + melee packs is a plausible full-body armed/unarmed state change, with stance and grip continuity still untested. Basic + Climbing/Campfire needs an explicit exit from ground locomotion, environment alignment, and a return state. Fresh Unity checks support the named Basic-to-melee idle/walk combinations as technically executable with a fixed owner root; see the [collection appendix](protofactor-ultimate-animation-collection-evidence.md#rig-masking-and-compatibility-evidence). They do not approve unrestricted blending or upper-body overlays.

These are proposed integration decisions (`inferred`), with target-controller, contact and artistic acceptance still `not-evaluated`.

## Changes between AnimSmith versions

AnimSmith 0.14.0 — reran 179 input baselines, 177 per-file declared contracts and 39 remediation trials using the official release. Added current exact-member generic scenarios and developer/artist actions. AnimSmith 0.10.0 — superseded historical evidence; none of its generated outputs or engine results is relabelled as a fresh run.

## Evidence status

Current evidence uses the official 0.14.0 binary and the [canonical readiness ladder](../game-ready-clips.md#the-readiness-ladder). Commercial sources and derivatives remain external.

## Sources

- Protofactor, [Ultimate Animation Collection](https://protofactor.biz/product/ultimate-animation-collection/) — collection-level product context.
- AnimSmith, [game-ready clips](../game-ready-clips.md) and [CLI reference](../cli.md) — readiness and command boundaries.
- Unity 6 documentation: [Blend Trees](https://docs.unity3d.com/6000.0/Documentation/Manual/class-BlendTree.html), [Root Motion](https://docs.unity3d.com/6000.0/Documentation/Manual/RootMotion.html), and [Animation Layers](https://docs.unity3d.com/6000.0/Documentation/Manual/AnimationLayers.html) — engine concepts only, not evidence of pack quality.
