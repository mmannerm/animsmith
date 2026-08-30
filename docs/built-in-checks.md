# Built-in check reference

This is the customer-facing reference for every registered built-in check in
AnimSmith's current catalog. It complements the concise [README check
catalog](../README.md#checks), the symptom-first [game-ready clips
guide](game-ready-clips.md), and the exact Rust API on
[docs.rs](https://docs.rs/animsmith-core).

Use this page when you need the current built-in ID, default findings,
declarations or prerequisites, config keys and units, skip or coverage-gap
semantics, and the practical boundary between built-in tooling, runtime policy,
and DCC-side correction.

`off` in the inventory below means the check is registered but opt-in by
default. Once selected, every built-in check can still be made inactive with
`[checks.<id>] severity = "off"`, and any selected opt-in check becomes active
only after an explicit `note`, `warn`, or `error` severity.

## Inventory

| id | class | default findings | declarations or prerequisites | config keys | tooling |
|---|---|---|---|---|---|
| [nan](#nan) | mechanical | `error` | none | `severity` | DCC/export repair |
| [time-monotonic](#time-monotonic) | mechanical | `error`, `note` | none | `severity` | DCC/export repair |
| [quat-norm](#quat-norm) | mechanical | `error` | none | `severity` | `fix` |
| [quat-flip](#quat-flip) | mechanical | `warning` | none | `severity` | `fix` |
| [duration-sanity](#duration-sanity) | mechanical | `error`, `warning` | none | `severity`, `clips.<name>.duration_s.value`, `clips.<name>.duration_s.tolerance` | DCC/export repair, `transform --slice`, `transform --hold-extend` |
| [scale-keys](#scale-keys) | mechanical | `warning` | none | `severity` | DCC cleanup |
| [non-uniform-scale](#non-uniform-scale) | mechanical | `warning` | none | `severity` | DCC cleanup |
| [constant-nonunit-scale](#constant-nonunit-scale) | mechanical | `off`, `note` | none | `severity` | DCC cleanup |
| [constant-track](#constant-track) | mechanical | `note` | none | `severity` | `transform --prune-constant-tracks` |
| [required-bones](#required-bones) | contract-aware | `error` | `rig.required_bones`, usable skeleton | `severity`, `rig.required_bones` | DCC/export repair |
| [rest-world-scale](#rest-world-scale) | contract-aware | `warning` | `runtime_nodes.selectors` or legacy node selector alias, source-node transform evidence | `severity`, `runtime_nodes.selectors`, `checks.rest-world-scale.node_selectors`, `expected_uniform_scale`, `uniform_scale_tolerance` | `scale`, DCC/export repair |
| [missing-bones](#missing-bones) | contract-aware | `error` | `clips.<name>.animates_bones` | `severity`, `clips.<name>.animates_bones` | DCC/export repair |
| [frozen-bone](#frozen-bone) | contract-aware | `error` | `clips.<name>.animates_bones`, keyed bone tracks | `severity`, `min_rotation_deg`, `clips.<name>.animates_bones` | DCC/export repair |
| [duplicate-loop-endpoint](#duplicate-loop-endpoint) | contract-aware | `warning` | `clips.<name>.loop`, authored endpoint analysis | `severity`, `clips.<name>.loop` | `transform --drop-duplicate-loop-endpoint` |
| [loop-closure](#loop-closure) | contract-aware | `error` | `clips.<name>.loop`, usable loop-continuity samples | `severity`, `max_position_delta_m`, `max_rotation_delta_deg`, `clips.<name>.loop`, `clips.<name>.max_loop_position_delta_m`, `clips.<name>.max_loop_rotation_delta_deg` | DCC/export repair |
| [loop-seam](#loop-seam) | contract-aware | `error` | `clips.<name>.loop`, hips and foot roles, sampled stride step | `severity`, `max_ratio`, `min_stride_step_m`, `clips.<name>.loop` | DCC/export repair |
| [loop-seam-vel](#loop-seam-vel) | contract-aware | `error` | `clips.<name>.loop`, usable loop-continuity samples | `severity`, `max_velocity_delta_mps`, `clips.<name>.loop`, `clips.<name>.max_loop_velocity_delta_mps` | DCC/export repair |
| [loop-seam-rot](#loop-seam-rot) | contract-aware | `error` | `clips.<name>.loop`, usable loop-continuity samples | `severity`, `max_angular_velocity_delta_degps`, `clips.<name>.loop`, `clips.<name>.max_loop_angular_velocity_delta_degps` | DCC/export repair |
| [root-motion-speed](#root-motion-speed) | contract-aware | `error` | `clips.<name>.speed_mps`, non-gameplay XZ owner, root/hips role, sampled root travel | `severity`, `clips.<name>.speed_mps.value`, `clips.<name>.speed_mps.tolerance`, `clips.<name>.movement_owner_xz`, `clips.<name>.in_place` | DCC/export repair |
| [gait-group](#gait-group) | contract-aware | `error` | `gait_groups.<name>.clips`, hips and foot roles, measurable left/right gait amplitude | `severity`, `gait_groups.<name>.clips`, `gait_groups.<name>.max_gait_phase_spread`, `gait_groups.<name>.min_lr_amplitude_m` | DCC/export repair |
| [sync-group](#sync-group) | contract-aware | `error` | `sync_groups.<name>.clips`, at least two present members | `severity`, `sync_groups.<name>.clips`, `sync_groups.<name>.max_duration_delta_s`, `sync_groups.<name>.max_frame_count_delta`, `sync_groups.<name>.max_fps_delta` | DCC/export repair |
| [time-complement](#time-complement) | contract-aware | `warning` | `sync_groups.<name>.time_complement`, hips and foot roles, measurable same-time gait phase | `severity`, `sync_groups.<name>.clips`, `sync_groups.<name>.time_complement.min_reflected_time_advantage`, `sync_groups.<name>.time_complement.min_lr_amplitude_m` | author/runtime review |
| [in-place](#in-place) | contract-aware | `error` | `clips.<name>.movement_owner_xz` or `clips.<name>.in_place`, root/hips role, sampled root travel | `severity`, `clips.<name>.movement_owner_xz`, `clips.<name>.in_place` | DCC/export repair |
| [fps](#fps) | contract-aware | `warning` | `clips.<name>.fps` | `severity`, `clips.<name>.fps` | DCC/export repair, `transform --slice` |
| [bind-pose](#bind-pose) | contract-aware | `warning` | clips with at least three usable first-frame rotation tracks | `severity`, `max_mean_rest_delta_deg` | DCC/export repair |
| [foot-slide](#foot-slide) | contract-aware | `warning` | `clips.<name>.speed_mps`, root/hips role, resolved foot or toe roles, sampled stance windows | `severity`, `contact_height_m`, `max_slide_mps` | DCC/export repair, engine review |

## Mechanical checks

### `nan`

- Default findings: `error`.
- Measurement and finding: flags any non-finite key time or keyed value. A
  finding means interpolation and pose evaluation are no longer trustworthy.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.nan] severity` only.
- Inactive, not-applicable, and coverage gaps: never reports
  `not_applicable` or a typed coverage gap once selected.
- Remediation and boundary: there is no safe built-in repair because the source
  float is undefined. Re-export or correct the source in the DCC/export step.
- Runtime and references: one poisoned float can explode, freeze, or otherwise
  corrupt the pose in-engine. See [The pose flickers, spins, or
  explodes](game-ready-clips.md#the-pose-flickers-spins-or-explodes) and
  [API: `Nan`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/nan/struct.Nan.html).

### `time-monotonic`

- Default findings: `error` for negative or non-increasing key times, `note`
  for a first key that starts materially after zero.
- Measurement and finding: checks that key times start at or near zero and move
  strictly forward. A finding means the runtime may clamp-hold or sample
  incorrectly
  the track.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.time-monotonic] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: fix the export or source timing in the DCC. Use
  `transform --slice` only for an intentional clip trim, not to salvage invalid
  key ordering.
- Runtime and references: late starts can hold an unauthored pose; bad ordering
  is a hard data defect. See [The pose flickers, spins, or
  explodes](game-ready-clips.md#the-pose-flickers-spins-or-explodes) and
  [API: `TimeMonotonic`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/time_monotonic/struct.TimeMonotonic.html).

### `quat-norm`

- Default findings: `error`.
- Measurement and finding: reports the worst rotation key whose quaternion norm
  deviates materially from `1.0`.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.quat-norm] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: `animsmith fix` can renormalize finite, non-zero
  quaternions losslessly. If the source exporter keeps reintroducing the issue,
  correct that upstream.
- Runtime and references: non-unit rotations skew interpolation, skinning, and
  blends. See [The pose flickers, spins, or
  explodes](game-ready-clips.md#the-pose-flickers-spins-or-explodes), [CLI
  repairs](cli.md#repairs), and
  [API: `QuatNorm`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/quat_norm/struct.QuatNorm.html).

### `quat-flip`

- Default findings: `warning`.
- Measurement and finding: counts adjacent rotation keys on opposite
  hemispheres. A finding means neighborhood-uncorrected slerp can take the long
  path.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.quat-flip] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: `animsmith fix` can flip equivalent quaternion
  signs into a hemisphere-consistent track. Re-export if the source keeps
  emitting unstable signs.
- Runtime and references: the visible symptom is a sudden long-way spin between
  keys. See [The pose flickers, spins, or
  explodes](game-ready-clips.md#the-pose-flickers-spins-or-explodes), [CLI
  repairs](cli.md#repairs), and
  [API: `QuatFlip`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/quat_flip/struct.QuatFlip.html).

### `duration-sanity`

- Default findings: `error` for invalid or missed declared duration pins and
  degenerate clip lengths, `warning` for trackless clips or mismatched channel
  end times.
- Measurement and finding: checks clip duration, optional declared
  `duration_s`, and spread between multi-key channel end times.
- Prerequisites and applicability: none. Declared duration pins refine a check
  that is otherwise generic.
- Config, defaults, and units: `[checks.duration-sanity] severity`; optional
  `clips.<name>.duration_s.value` and `clips.<name>.duration_s.tolerance`, both
  in seconds.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: correct the authored range or export in the DCC.
  `transform --slice` and `transform --hold-extend` are explicit editing tools
  when the clip is intentionally being reshaped, not automatic correctness
  repairs.
- Runtime and references: bad duration contracts freeze shorter channels or
  desync gameplay timing. See [The clip is the wrong length or freezes at the
  end](game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end)
  and [API:
  `DurationSanity`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/duration_sanity/struct.DurationSanity.html).

### `scale-keys`

- Default findings: `warning`.
- Measurement and finding: reports temporally varying scale tracks.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.scale-keys] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: remove accidental animated scale in the DCC or
  export settings. Treat retained scale animation as an intentional authored
  choice that downstream runtime and retargeter owners must explicitly accept.
- Runtime and references: many rigs and retargeters mishandle animated scale.
  See [The file is bloated, or the retargeter
  chokes](game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) and
  [API:
  `ScaleKeys`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/scale_keys/struct.ScaleKeys.html).

### `non-uniform-scale`

- Default findings: `warning`.
- Measurement and finding: reports scale tracks whose evaluated axis lengths are
  materially unequal.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.non-uniform-scale] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: normalize the authored transform in the DCC or
  export path unless your runtime and retargeter contract explicitly accepts
  the non-uniform scale.
- Runtime and references: non-uniform scale is a common retargeting and rig
  failure source. See [The file is bloated, or the retargeter
  chokes](game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) and
  [API:
  `NonUniformScale`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/non_uniform_scale/struct.NonUniformScale.html).

### `constant-nonunit-scale`

- Default findings: `off` by default; once enabled, findings are `note`.
- Measurement and finding: reports scale channels that stay constant but not at
  unit scale, including single-key pins.
- Prerequisites and applicability: none. This is a policy signal, not a
  mandatory file-ready failure.
- Config, defaults, and units: `[checks.constant-nonunit-scale] severity` only.
- Inactive, not-applicable, and coverage gaps: inactive until an explicit
  severity enables it; never reports typed gaps once active.
- Remediation and boundary: clean up the authored scale in the DCC if your
  runtime policy expects unit scale. Keep it enabled only when your team wants
  to surface that policy in lint results.
- Runtime and references: constant non-unit scale can still affect import,
  attachment, and retargeting expectations. See [The file is bloated, or the
  retargeter
  chokes](game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes) and
  [API:
  `ConstantNonunitScale`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/constant_nonunit_scale/struct.ConstantNonunitScale.html).

### `constant-track`

- Default findings: `note`.
- Measurement and finding: reports multi-key translation, rotation, or scale
  tracks that never materially move.
- Prerequisites and applicability: none. This is generic file-ready coverage.
- Config, defaults, and units: `[checks.constant-track] severity` only.
- Inactive, not-applicable, and coverage gaps: never `not_applicable` and no
  typed gaps once selected.
- Remediation and boundary: `transform --prune-constant-tracks` can remove the
  subset AnimSmith can prove redundant. Use DCC cleanup when the track carries
  authoring intent, unsupported curves, or broader rig cleanup.
- Runtime and references: constant multi-key tracks are usually export bloat,
  but removing them still needs contract review for transition and required
  motion evidence. See [The file is bloated, or the retargeter
  chokes](game-ready-clips.md#the-file-is-bloated-or-the-retargeter-chokes),
  [Editing a clip](../examples/README.md#3-editing-a-clip), and
  [API:
  `ConstantTrack`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/constant_track/struct.ConstantTrack.html).

## Contract-aware checks

### `required-bones`

- Default findings: `error`.
- Measurement and finding: checks that every distinct name in
  `rig.required_bones` exists exactly once in the skeleton.
- Prerequisites and applicability: active only when `rig.required_bones` is
  non-empty. A usable skeleton is required.
- Config, defaults, and units: `[checks.required-bones] severity` and
  `rig.required_bones`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no
  declared required bones; reports a `skeleton_unavailable` coverage gap when
  the file has no usable skeleton.
- Remediation and boundary: fix the exported skeleton, naming, or rig mapping
  in the DCC/export pipeline. This is not a clip-level animation repair.
- Runtime and references: missing sockets, IK targets, or mask bones break
  downstream binding even if no clip animates them. See [A limb is T-posed, or
  a bone never
  moves](game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves) and
  [API:
  `RequiredBones`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/required_bones/struct.RequiredBones.html).

### `rest-world-scale`

- Default findings: `warning`.
- Measurement and finding: checks the effective rest-world linear transform of
  explicitly selected source nodes against a uniform scale policy.
- Prerequisites and applicability: active only when `runtime_nodes.selectors`
  or the legacy `checks.rest-world-scale.node_selectors` alias declares at
  least one selector. Source-node transform evidence must exist, and each
  selector must resolve exactly once.
- Config, defaults, and units: `[checks.rest-world-scale] severity`,
  `expected_uniform_scale` default `1.0`, and `uniform_scale_tolerance` default
  `0.0001`; selectors live in `runtime_nodes.selectors`, with the legacy alias
  accepted only when the shared field is absent.
- Inactive, not-applicable, and coverage gaps: `not_applicable` without
  selectors; gaps report no-match, ambiguous selector resolution, or unavailable
  source-node transform evidence.
- Remediation and boundary: if the source is uniformly wrong, decide between
  `scale whole-document`, `scale rest-bind`, or DCC/export correction based on
  the actual scale problem. This check reports source-node evidence; it does
  not infer which rewrite you intend.
- Runtime and references: wrong rest-world scale breaks sockets, attachments,
  gameplay references, and engine-side assumptions even when animation keys are
  otherwise valid. See [Scaling glTF safely](scale.md), [glTF and generic
  runtime guide](engine-profile-gltf-runtime.md), and [API:
  `RestWorldScale`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/rest_world_scale/struct.RestWorldScale.html).

### `missing-bones`

- Default findings: `error`.
- Measurement and finding: checks that each declared `animates_bones` member
  exists and carries at least one keyed track in the clip.
- Prerequisites and applicability: active only on clips that declare
  `animates_bones`.
- Config, defaults, and units: `[checks.missing-bones] severity` and
  `clips.<name>.animates_bones`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` on clips
  without `animates_bones`; no typed coverage gaps once the declaration exists.
- Remediation and boundary: restore the missing keyed channel or correct the
  source clip and rig in the DCC/export path.
- Runtime and references: declared motion that never reaches the file reads as
  a static or wrong-rig limb at runtime. See [A limb is T-posed, or a bone
  never moves](game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves)
  and [API:
  `MissingBones`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/missing_bones/struct.MissingBones.html).

### `frozen-bone`

- Default findings: `error`.
- Measurement and finding: checks whether each declared animated bone rotates
  beyond the configured floor across the clip.
- Prerequisites and applicability: active only on clips that declare
  `animates_bones`. The bone must exist and carry at least one keyed track.
- Config, defaults, and units: `[checks.frozen-bone] severity`,
  `min_rotation_deg` default `1.0`, and `clips.<name>.animates_bones`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` on clips
  without `animates_bones`; no typed gaps once the declaration exists. Missing
  bones themselves belong to `missing-bones`.
- Remediation and boundary: correct the source clip, wrong slice, or masked-out
  track in the DCC/export pipeline.
- Runtime and references: the visible symptom is a limb that stays pinned,
  T-posed, or otherwise fails to animate even though the clip says it should.
  See [A limb is T-posed, or a bone never
  moves](game-ready-clips.md#a-limb-is-t-posed-or-a-bone-never-moves) and
  [API:
  `FrozenBone`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/frozen_bone/struct.FrozenBone.html).

### `duplicate-loop-endpoint`

- Default findings: `warning`.
- Measurement and finding: checks for the strict authored-key case where a
  declared loop repeats its first pose at the final endpoint and that closing
  key can be removed mechanically.
- Prerequisites and applicability: active only on clips declared
  `loop = true`, and only when authored endpoint analysis is available.
- Config, defaults, and units: `[checks.duplicate-loop-endpoint] severity` and
  `clips.<name>.loop`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares a loop. Reports a measurement-unavailable gap when authored endpoint
  analysis cannot classify the clip.
- Remediation and boundary: `transform --drop-duplicate-loop-endpoint` can trim
  the mechanically removable endpoint subset. Non-classified loop endpoint modes
  still need authored review or a different transform.
- Runtime and references: redundant inclusive endpoints are a common DCC export
  artifact and can affect downstream loop handling. See [The loop
  pops](game-ready-clips.md#the-loop-pops), [Editing a
  clip](../examples/README.md#3-editing-a-clip), and [API:
  `DuplicateLoopEndpoint`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/duplicate_loop_endpoint/struct.DuplicateLoopEndpoint.html).

### `loop-closure`

- Default findings: `error`.
- Measurement and finding: checks the largest per-bone model-space position and
  rotation delta between the final and first loop sample.
- Prerequisites and applicability: active only on clips declared `loop = true`
  and only when loop-continuity samples exist.
- Config, defaults, and units: `[checks.loop-closure] severity`,
  `max_position_delta_m` default `0.01` metres, `max_rotation_delta_deg`
  default `1.0` degrees, plus per-clip overrides
  `clips.<name>.max_loop_position_delta_m` and
  `clips.<name>.max_loop_rotation_delta_deg`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares a loop. Reports measurement-unavailable gaps when the sampled
  loop-continuity evidence is missing or unusable for one or more bones.
- Remediation and boundary: re-author, reslice, or re-export the loop in the
  DCC. This is a diagnosis, not an automatic loop repair.
- Runtime and references: a failed closure produces an obvious pose jump at the
  wrap. See [The loop pops](game-ready-clips.md#the-loop-pops) and [API:
  `LoopClosure`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/loop_closure/struct.LoopClosure.html).

### `loop-seam`

- Default findings: `error`.
- Measurement and finding: checks the feet-relative-to-hips wrap discontinuity,
  normalized by seam-adjacent in-clip step length.
- Prerequisites and applicability: active only on clips declared `loop = true`
  and only when hips plus at least one foot role resolve and a usable stride
  step can be sampled.
- Config, defaults, and units: `[checks.loop-seam] severity`, `max_ratio`
  default `1.5`, and `min_stride_step_m` defaulting to the built-in minimum
  stride floor used by the metrics.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares a loop. Gaps report unresolved gait roles, clips too short to sample
  a cycle, unavailable foot-cycle metrics, or no usable stride step.
- Remediation and boundary: fix the loop cut or cycle in the DCC. AnimSmith
  intentionally reports the seam evidence rather than inventing a repair.
- Runtime and references: the symptom is a once-per-cycle pop even when the raw
  file is format-valid. See [The loop pops](game-ready-clips.md#the-loop-pops)
  and [API:
  `LoopSeam`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/loop_seam/struct.LoopSeam.html).

### `loop-seam-vel`

- Default findings: `error`.
- Measurement and finding: checks the largest per-bone model-space linear
  velocity change between the incoming and outgoing loop seam.
- Prerequisites and applicability: active only on clips declared `loop = true`
  and only when loop-continuity samples exist.
- Config, defaults, and units: `[checks.loop-seam-vel] severity`,
  `max_velocity_delta_mps` default `0.1` metres per second, plus the per-clip
  override `clips.<name>.max_loop_velocity_delta_mps`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares a loop. Reports measurement-unavailable gaps when loop-continuity
  samples are missing or unusable.
- Remediation and boundary: correct the authored wrap or the export timing in
  the DCC; there is no built-in automatic velocity repair.
- Runtime and references: this is the once-per-cycle hitch where the clip hits
  the right pose but changes speed abruptly at the seam. See [The loop
  pops](game-ready-clips.md#the-loop-pops) and [API:
  `LoopSeamVelocity`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/loop_seam_vel/struct.LoopSeamVelocity.html).

### `loop-seam-rot`

- Default findings: `error`.
- Measurement and finding: checks the largest per-bone model-space angular
  velocity change between the incoming and outgoing loop seam.
- Prerequisites and applicability: active only on clips declared `loop = true`
  and only when loop-continuity samples exist.
- Config, defaults, and units: `[checks.loop-seam-rot] severity`,
  `max_angular_velocity_delta_degps` default `5.0` degrees per second, plus the
  per-clip override `clips.<name>.max_loop_angular_velocity_delta_degps`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares a loop. Reports measurement-unavailable gaps when loop-continuity
  samples are missing or unusable.
- Remediation and boundary: correct the authored wrap or export timing in the
  DCC; there is no built-in automatic angular-velocity repair.
- Runtime and references: this is the rotational version of a loop hitch or
  pulse at the seam. See [The loop pops](game-ready-clips.md#the-loop-pops) and
  [API:
  `LoopSeamRotation`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/loop_seam_rot/struct.LoopSeamRotation.html).

### `root-motion-speed`

- Default findings: `error`.
- Measurement and finding: compares measured horizontal root travel against a
  declared `speed_mps` pin for clips whose XZ owner is not gameplay.
- Prerequisites and applicability: active only when a clip declares
  `speed_mps` and does not declare gameplay-owned XZ travel. Root or hips role
  resolution and measurable root travel are required.
- Config, defaults, and units: `[checks.root-motion-speed] severity`,
  `clips.<name>.speed_mps.value`, `clips.<name>.speed_mps.tolerance`, and the
  XZ owner via `clips.<name>.movement_owner_xz` or the compatibility alias
  `clips.<name>.in_place`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no
  relevant speed pin. Gaps report unresolved root or hips roles or unavailable
  measured root-motion speed.
- Remediation and boundary: fix the clip's actual travel or the declared speed
  contract in the DCC/export path or config. This is not a runtime retiming
  suggestion from AnimSmith.
- Runtime and references: stale speed pins make motion-scaled playback slide or
  moonwalk. See [The character glides or runs in
  place](game-ready-clips.md#the-character-glides-or-runs-in-place) and [API:
  `RootMotionSpeed`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/root_motion_speed/struct.RootMotionSpeed.html).

### `gait-group`

- Default findings: `error`.
- Measurement and finding: compares measured gait phases across each declared
  blend-ring group and reports spread beyond the configured cap.
- Prerequisites and applicability: active only when `gait_groups` is non-empty.
  Hips and foot roles must resolve, and at least two members need measurable
  gait evidence above the configured amplitude floor.
- Config, defaults, and units: `[checks.gait-group] severity`, plus
  `gait_groups.<name>.clips`, `gait_groups.<name>.max_gait_phase_spread` in
  cycle fraction, and `gait_groups.<name>.min_lr_amplitude_m` in metres.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no gait
  groups. Gaps report unresolved gait roles, missing members, too few
  measurable members, or members excluded because phase evidence is
  unavailable or below the amplitude floor.
- Remediation and boundary: correct clip timing, loop structure, or selection
  in the DCC and review the declared group. AnimSmith does not reorder or
  retime the group automatically.
- Runtime and references: phase drift between directional blends shows up as
  sliding or popping feet when clips crossfade. See [Directional blend members
  travel at different
  speeds](game-ready-clips.md#directional-blend-members-travel-at-different-speeds)
  and [API:
  `GaitGroup`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/gait_group/struct.GaitGroup.html).

### `sync-group`

- Default findings: `error`.
- Measurement and finding: compares declared same-time group members for
  duration, frame count, frame grid, and loop endpoint mode compatibility.
- Prerequisites and applicability: active only when `sync_groups` is non-empty,
  and each group needs at least two present members before full comparison is
  possible.
- Config, defaults, and units: `[checks.sync-group] severity`, plus
  `sync_groups.<name>.clips`, `sync_groups.<name>.max_duration_delta_s` in
  seconds, `sync_groups.<name>.max_frame_count_delta` in frames, and
  `sync_groups.<name>.max_fps_delta` in frames per second.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no sync
  groups. Gaps report missing members, too few present or measurable members,
  unavailable durations, unusable frame-grid evidence, or unavailable loop
  endpoint evidence.
- Remediation and boundary: realign or rebuild the compared clips and the group
  declaration in the DCC/export pipeline. This check reports compatibility; it
  does not choose a runtime retiming strategy.
- Runtime and references: mismatched same-time clips blend or switch at
  different semantic moments. See [Feet skate when clips
  blend](game-ready-clips.md#feet-skate-when-clips-blend) and [API:
  `SyncGroupCheck`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/sync_group/struct.SyncGroupCheck.html).

### `time-complement`

- Default findings: `warning`.
- Measurement and finding: compares each declared same-time pair's gait-phase
  similarity against the similarity after reflecting one member's normalized
  cycle time.
- Prerequisites and applicability: active only when a sync group enables
  `time_complement`. Hips and foot roles must resolve, and at least two members
  need measurable gait phase above the configured amplitude floor.
- Config, defaults, and units: `[checks.time-complement] severity`, plus
  `sync_groups.<name>.time_complement.min_reflected_time_advantage` in `[0, 1]`
  and `sync_groups.<name>.time_complement.min_lr_amplitude_m` in metres. The
  check also uses `sync_groups.<name>.clips`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no
  enabled time-complement policy. Gaps report missing members, unresolved gait
  roles, unavailable phase evidence, or too few measurable members.
- Remediation and boundary: treat this as a sync-diagnostic for authoring or
  runtime review, not as proof that time reflection is the right runtime fix.
- Runtime and references: a warning means same-time pairing may be working
  against the measured gait evidence. See [Feet skate when clips
  blend](game-ready-clips.md#feet-skate-when-clips-blend) and [API:
  `TimeComplement`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/time_complement/struct.TimeComplement.html).

### `in-place`

- Default findings: `error`.
- Measurement and finding: compares the declared horizontal movement owner to
  measured horizontal root travel.
- Prerequisites and applicability: active only when a clip declares
  `movement_owner_xz` or its compatibility alias `in_place`. Root or hips role
  resolution and measurable root travel are required.
- Config, defaults, and units: `[checks.in-place] severity`,
  `clips.<name>.movement_owner_xz`, or the compatibility alias
  `clips.<name>.in_place`.
- Inactive, not-applicable, and coverage gaps: `not_applicable` when no clip
  declares horizontal movement ownership. Gaps report unresolved root or hips
  roles or unavailable measured root speed.
- Remediation and boundary: fix the clip or the declared ownership in the DCC,
  export path, or contract config. This is an intent mismatch, not a built-in
  transform target.
- Runtime and references: a mismatch becomes visible gliding or running in
  place. See [The character glides or runs in
  place](game-ready-clips.md#the-character-glides-or-runs-in-place) and [API:
  `InPlace`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/in_place/struct.InPlace.html).

### `fps`

- Default findings: `warning`.
- Measurement and finding: checks that a declared clip duration spans a whole
  number of frames and that keyed times stay on the declared frame grid.
- Prerequisites and applicability: active only on clips that declare `fps`.
- Config, defaults, and units: `[checks.fps] severity` and `clips.<name>.fps`
  in frames per second.
- Inactive, not-applicable, and coverage gaps: `not_applicable` on clips
  without `fps`. Reports an `invalid_declared_fps` coverage gap when the
  declared frame rate is non-finite or non-positive.
- Remediation and boundary: correct the authored frame rate or retiming in the
  DCC/export path. `transform --slice` is appropriate for an intentional
  frame-aligned cut, not for arbitrary resampling drift.
- Runtime and references: off-grid keys and fractional frame counts are common
  importer pain points. See [The clip is the wrong length or freezes at the
  end](game-ready-clips.md#the-clip-is-the-wrong-length-or-freezes-at-the-end)
  and [API:
  `Fps`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/fps/struct.Fps.html).

### `bind-pose`

- Default findings: `warning`.
- Measurement and finding: checks the mean first-frame rotation deviation from
  rest across bones with usable rotation tracks.
- Prerequisites and applicability: active whenever the file has clips. A clip
  needs at least three usable first-frame rotation tracks before the check can
  evaluate it.
- Config, defaults, and units: `[checks.bind-pose] severity` and
  `max_mean_rest_delta_deg` default `45` degrees.
- Inactive, not-applicable, and coverage gaps: `not_applicable` only when the
  document has no clips. Reports `insufficient_rotation_evidence` when a clip
  lacks enough usable first-frame rotation tracks.
- Remediation and boundary: correct the seed rig, export skeleton, or clip
  source in the DCC/export path. This is not a rest-bind scale rewrite.
- Runtime and references: a mismatched bind often presents as the wrong base
  pose or wrong-skeleton clip. See [Files disagree about skeleton or clip
  identity](game-ready-clips.md#files-disagree-about-skeleton-or-clip-identity)
  and [API:
  `BindPose`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/bind_pose/struct.BindPose.html).

### `foot-slide`

- Default findings: `warning`.
- Measurement and finding: checks whether each stance foot moves consistently
  with the declared `speed_mps` and the clip's measured horizontal root speed.
- Prerequisites and applicability: active only on clips that declare
  `speed_mps`. Root or hips roles must resolve, the clip must be long enough to
  sample stance, and a foot or toe role must resolve for each judged side.
- Config, defaults, and units: `[checks.foot-slide] severity`,
  `contact_height_m` default `0.03` metres, `max_slide_mps` default `0.3`
  metres per second. The clip-level `speed_mps` declaration is the
  prerequisite, not a foot-slide check setting.
- Inactive, not-applicable, and coverage gaps: `not_applicable` with no speed
  pin. Gaps report unresolved root or hips roles, clips too short to sample
  stance, unavailable root-motion speed, or unresolved foot or toe roles.
- Remediation and boundary: fix foot plants, root travel, or the declared
  speed pin in the DCC/export path and then review the runtime blend setup.
  This heuristic check intentionally does not auto-rewrite contact or infer
  movement ownership.
- Runtime and references: failed stance motion shows up as skating or slipping
  feet in motion or blends. See [Feet slide within one
  clip](game-ready-clips.md#feet-slide-within-one-clip) and [API:
  `FootSlide`](https://docs.rs/animsmith-core/latest/animsmith_core/checks/foot_slide/struct.FootSlide.html).
