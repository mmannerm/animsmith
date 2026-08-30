# `animsmith.toml` configuration reference

`animsmith.toml` is an optional, strict project contract. Document-local
commands (`lint`, `measure`, `inspect`, `report`, `transform`, `fix`,
`generate`, and `evaluate-transition-poses`) use `--config FILE`, or auto-load
`./animsmith.toml`; no file means
built-in defaults. Collection commands have their own control-file rules (see
[`cli.md`](cli.md)). Unknown tables and keys are errors. The parser validates
finite numeric domains; embedded callers should also call
[`Config::validate`](https://docs.rs/animsmith-core/latest/animsmith_core/config/struct.Config.html#method.validate).

This page is the user-facing authority. Exact Rust types are in the
[`animsmith-core::config` API](https://docs.rs/animsmith-core/latest/animsmith_core/config/).
The check rationale and coverage vocabulary are in
[`game-ready-clips.md`](game-ready-clips.md).

## Precedence and the run model

CLI `--select`, `--allow`, and `--deny-warnings` apply after the file is read;
CLI selection and failure policy therefore override file run policy.
`severity` controls content findings, not coverage gaps or required engine
prediction-unavailable facets. `off` disables an applicable check only where
that check permits it; `note`, `warn` (or `warning`), and `error` enable an
opt-in check. `--allow ID` hides matching content findings from the CLI gate;
it never repairs input or suppresses required prediction work.

Clip selectors are resolved field-by-field. Every matching `clips` glob is
overlaid in lexicographic key order; a literal clip-name entry is applied last.
Thus an exact key wins over every glob, and a later lexicographically greater
glob wins only for fields it declares. `*` matches any sequence (including an
empty sequence); there is no `?` or character-class syntax. Empty or missing
selector lists declare no policy. Group member names are exact (not globs).

## Top-level tables

| Table/key | Type; default; units; allowed values | Omission and consumer | Prerequisite / precedence |
|---|---|---|---|
| `[rig]` | table; `{ profile = "auto", roles = {}, required_bones = none }` | Optional. `Config::rig` feeds role-aware checks, measurements, `inspect`, assembly, and engine paths. | `auto` scores built-ins; explicit profile or inline roles are resolved by the frontend before checks. |
| `[checks.<id>]` | map of check id → table; absent settings use each check's built-in defaults | `Config::checks` is consumed by the check runner and owning check. | `--select` narrows the catalog; severity is evaluated after selection. |
| `[runtime_nodes]` | table; absent; `selectors` is an optional string array | `Config::runtime_nodes` supplies the shared runtime-node policy. | If present it wins over legacy `checks.rest-world-scale.node_selectors`; both is an error. |
| `[clips."selector"]` | map of exact clip name or `*` glob → table; all fields omitted by default | `Config::clips` supplies effective expectations to clip-aware checks and transforms. | Glob overlay then exact overlay; omission means “inherit/no declaration”, not false or zero. |
| `[gait_groups."name"]` | named table; no groups by default | `Config::gait_groups` is consumed by `gait-group`. | Needs declared members, resolved feet, and measurable gait evidence; missing work is a coverage gap. |
| `[sync_groups."name"]` | named table; no groups by default | `Config::sync_groups` is consumed by `sync-group`. | Needs exact member clips and usable timing; optional `time_complement` adds a phase diagnostic. |
| `[engine]` | CLI-only table; absent means no engine prediction/advice | `EngineToml` supplies profile selection and settings to `generate addressability`, `generate import-advice`, and engine checks. | `profile`, `profile_revision`, `engine_version`, and `importer` are all required when any selection field is present. Unsupported tuples/settings fail closed. |
| `[transition_families."id"]` | CLI-only strict document declaration; absent/empty means no families | `evaluate-transition-poses` consumes `transition_families`; the collection variant consumes its `--families` control file. It is not part of core `Config`. | Exact schema/scope and bounded declaration; see [`collection-contracts.md`](collection-contracts.md#transition-families-148). |

## Rig selection

| Key | Type/default/allowed values | Omission, consumer, and prerequisites |
|---|---|---|
| `rig.profile` | string; `"auto"`; `auto`, `mixamo`, `ue-mannequin`, `humanoid` | `auto` chooses the unique best built-in match (at least two roles). A named profile is strict. Exact names are tried first, then one unique ASCII-case-insensitive match; ambiguity is typed. |
| `rig.roles.<role>` | string map; empty; role is `root`, `hips`, `spine`, `head`, `left_foot`, `right_foot`, `left_toe`, `right_toe`, `left_hand`, `right_hand` | Inline bindings are exact and override the selected profile only for named roles. Missing/colliding bones produce typed coverage/ambiguity, never guesses. |
| `rig.required_bones` | string array or omitted; no units; any bone names | Presence-only structural contract for sockets, IK targets, and masks. It does not require animation keys; use `clips.*.animates_bones` for that. |

## Checks and severity

Every check accepts `severity = "off"`, `"note"`, `"warn"`/`"warning"`, or
`"error"`. Omitted severity uses the check's default (all built-ins are
enabled except opt-in `constant-nonunit-scale` and `time-complement`; an
explicit non-off severity enables those). The runner still records
unselected, disabled, and not-applicable checks. See
[`game-ready-clips.md`](game-ready-clips.md#reading-a-lint-run).

| Check setting | Type; built-in default; units/domain | Consumer, activation, and skip behavior |
|---|---|---|
| `checks.<id>.severity` | enum; omitted; `off`, `note`, `warn`/`warning`, `error` | All checks. Content findings only; it does not hide gaps or required predictions. |
| `checks.loop-seam.max_ratio` | finite non-negative float; `1.5`; dimensionless | `loop-seam`; declared loops with usable foot-cycle evidence. |
| `checks.loop-seam.min_stride_step_m` | finite non-negative float; `0.02`; metres | `loop-seam` stride floor; short/unresolved gait evidence is a gap. |
| `checks.loop-closure.max_position_delta_m` | finite non-negative float; `0.01`; metres | `loop-closure`; declared loops; per-clip cap overrides it. |
| `checks.loop-closure.max_rotation_delta_deg` | finite non-negative float; `1.0`; degrees | `loop-closure`; declared loops; per-clip cap overrides it. |
| `checks.loop-seam-vel.max_velocity_delta_mps` | finite non-negative float; `0.1`; metres/second | `loop-seam-vel`; declared loops; per-clip cap overrides it. |
| `checks.loop-seam-rot.max_angular_velocity_delta_degps` | finite non-negative float; `5.0`; degrees/second | `loop-seam-rot`; declared loops; per-clip cap overrides it. |
| `checks.frozen-bone.min_rotation_deg` | finite non-negative float; `1.0`; degrees | `frozen-bone`; needs `animates_bones` and a resolvable animated track; missing evidence is a gap. |
| `checks.bind-pose.max_mean_rest_delta_deg` | finite non-negative float; `45.0`; degrees | `bind-pose`; needs rotation tracks and a valid rest pose; otherwise a gap. |
| `checks.foot-slide.contact_height_m` | finite non-negative float; `0.03`; metres | `foot-slide`; needs `speed_mps`, foot roles, and stance evidence. |
| `checks.foot-slide.max_slide_mps` | finite non-negative float; `0.3`; metres/second | `foot-slide`; same prerequisites as `contact_height_m`. |
| `checks.rest-world-scale.expected_uniform_scale` | finite positive float; `1.0`; dimensionless factor | `rest-world-scale`; dormant unless runtime selectors are nonempty; selectors resolve exactly one source node. |
| `checks.rest-world-scale.uniform_scale_tolerance` | finite non-negative float; `0.0001`; dimensionless factor | Inclusive tolerance for the expected factor; same selector prerequisite. |
| `checks.rest-world-scale.node_selectors` | optional string array; absent; exact names or `*` globs | Legacy alias for `runtime_nodes.selectors`; only used when shared field is absent. Both spellings are invalid together. |

Settings not consumed by a particular check are harmless in that check's
table, but a misspelled field is rejected by `deny_unknown_fields`.

## Clip expectations

| Key | Type; default; units/allowed values | Consumer, omission, and prerequisites |
|---|---|---|
| `clips.<selector>.loop` | optional bool; omitted; `true`/`false` | Declares cyclic intent. `true` activates loop checks; omission is not false evidence for other checks. |
| `clips.<selector>.max_loop_position_delta_m` | optional finite non-negative float; omitted; metres | Per-selector `loop-closure` cap; inherits global then `0.01`. |
| `clips.<selector>.max_loop_rotation_delta_deg` | optional finite non-negative float; omitted; degrees | Per-selector `loop-closure` cap; inherits global then `1.0`. |
| `clips.<selector>.max_loop_velocity_delta_mps` | optional finite non-negative float; omitted; metres/second | Per-selector `loop-seam-vel` cap; inherits global then `0.1`. |
| `clips.<selector>.max_loop_angular_velocity_delta_degps` | optional finite non-negative float; omitted; degrees/second | Per-selector `loop-seam-rot` cap; inherits global then `5.0`. |
| `clips.<selector>.duration_s` | optional `{ value: finite positive float, tolerance: finite non-negative float }`; omitted; seconds | `duration-sanity` pin. Omission means no duration contract, not zero. |
| `clips.<selector>.speed_mps` | optional pinned value/tolerance; omitted; metres/second; value positive, tolerance non-negative | Root-motion speed and `foot-slide`; the latter is not applicable without this declaration. |
| `clips.<selector>.movement_owner_xz`, `clips.<selector>.movement_owner_y`, `clips.<selector>.movement_owner_yaw` | optional enum; omitted; `gameplay` or `animation` | In-place/root-motion checks and downstream transform intent. Each axis is independent; omission is never inferred. |
| `clips.<selector>.in_place` | optional bool; omitted; `true` = gameplay XZ, `false` = animation XZ | Legacy XZ alias for `clips.<selector>.movement_owner_xz`. Cannot coexist with it in one entry; across layers it is normalized before overlay. |
| `clips.<selector>.fps` | optional finite positive float; omitted; frames/second | `fps` and `sync-group`; omission means no authored frame-grid contract. |
| `clips.<selector>.animates_bones` | optional string array; omitted; bone names | `missing-bones` requires keys and `frozen-bone` requires actual rotation movement. Distinct from `rig.required_bones`. |

## Runtime nodes and globs

`runtime_nodes.selectors` is an ordered string array. Each item is an exact
source-node name or a `*` glob and must resolve to exactly one named node for
`rest-world-scale`; no match and multiple matches are typed coverage gaps.
Duplicate spellings are de-duplicated at first occurrence. Selectors do not
cross node and bone namespaces, and do not infer a node from a rig role.

## Gait and same-time groups

| Key | Type/default/domain | Consumer and skip behavior |
|---|---|---|
| `gait_groups.<name>.clips` | string array; required; exact clip names | `gait-group` checks member existence, then phase for measurable members. |
| `gait_groups.<name>.max_gait_phase_spread` | finite float `[0, 0.5]`; required; cycle fraction | Maximum circular phase spread; missing roles/short clips/low evidence produce coverage. |
| `gait_groups.<name>.min_lr_amplitude_m` | finite non-negative float; `0.0`; metres | Excludes low-amplitude members as noise. |
| `sync_groups.<name>.clips` | string array; required; exact clip names | `sync-group` checks exact members at the same absolute time. |
| `sync_groups.<name>.max_duration_delta_s` | finite non-negative float; required; seconds | Largest permitted duration range. |
| `sync_groups.<name>.max_frame_count_delta` | unsigned integer; required; key-count difference | Largest permitted longest-channel key-count range. |
| `sync_groups.<name>.max_fps_delta` | finite non-negative float; required; frames/second | Largest permitted declared-FPS range. |
| `sync_groups.<name>.time_complement` | optional table; omitted disables diagnostic | Adds reflected-time phase comparison for each configured-order unordered pair. |
| `sync_groups.<name>.time_complement.min_reflected_time_advantage` | finite float `[0, 1]`; required; score | Minimum reflected-minus-same phase similarity advantage. |
| `sync_groups.<name>.time_complement.min_lr_amplitude_m` | finite non-negative float; required; metres | Minimum evidence amplitude before phase comparison. |

## Engine profiles (CLI)

The four selection keys have no implicit default: `engine.profile`,
`engine.profile_revision`, `engine.engine_version`, and `engine.importer`. Either omit `[engine]`
or specify all four. Exact supported tuples and setting defaults are owned by
the versioned [`animsmith-engine` profiles](https://docs.rs/animsmith-engine/latest/animsmith_engine/).
The profile is evidence about importer behavior, not a claim that an engine
loaded or played the asset.

`engine.settings` accepts profile-defined keys; unknown keys or values are
rejected when the selected profile resolves. Common revision-1 settings are
`engine.settings.convert_units` (bool), `engine.settings.bake_axis_conversion`
(bool), and `engine.settings.root_motion_source` (string path), plus per-clip
`clips.<selector>.engine_settings.root_rotation`,
`clips.<selector>.engine_settings.root_position_y`, and
`clips.<selector>.engine_settings.root_position_xz` (`bake` or `extract`).
Revision-2 profiles may define `engine.settings.animation_type`
(`generic|humanoid|legacy`), `engine.settings.avatar_setup`
(`create_from_this_model|copy_from_other_avatar`),
`engine.settings.import_animation` (bool),
`engine.settings.root_motion_source` (string),
`engine.settings.root_rotation`, `engine.settings.root_position_y`,
`engine.settings.root_position_xz` (`bake|extract`),
`engine.settings.rotate_scene_entity` (bool), `engine.settings.rotate_meshes`
(bool), `engine.settings.load_meshes` (`empty|nonempty`),
`engine.settings.extension_handler_environment`
(`bare_empty|bevy_pbr_stock_0_19`), `engine.settings.bevy_animation_feature`
(bool), `engine.settings.load_animations` (bool),
`engine.settings.animation_fps` (positive integer),
`engine.settings.animation_trimming` (bool), and `engine.settings.sample_rate`
(`default_30`, `source_determined`, or `custom_hz(N)`). Per-clip revision-2
root policies use the same `clips.<selector>.engine_settings.root_rotation`,
`clips.<selector>.engine_settings.root_position_y`, and
`clips.<selector>.engine_settings.root_position_xz` paths. A setting is either
explicit or the profile's verified default; no generic default is invented.

## Transition families

Document-local families are strict tables, not core `Config` fields:

```toml
[transition_families."walk_to_run"]
schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "document"
boundary = "both"                 # entry | exit | both
[transition_families."walk_to_run".basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[transition_families."walk_to_run".tolerances]
translation_m = 0.05
rotation_deg = 5.0
time_normalized = 0.05
[[transition_families."walk_to_run".members]]
take_index = 0
take_name = "walk"
```

The complete bounded grammar and allowed values are documented in
[`collection-contracts.md`](collection-contracts.md#transition-families-148).
It is read before generic config decoding, so malformed declarations report a
transition-family control error and never become ordinary check findings.

## Minimal and advanced configurations

The smallest semantic contract is one loop declaration:

```toml
[clips.walk]
loop = true
```

For a project contract, start with
[`examples/character.animsmith.toml`](../examples/character.animsmith.toml).
It demonstrates rig roles, runtime-node globs, independent movement ownership,
glob/exact cap precedence, severity, gait rings, and same-time groups. The
[`Mixamo tutorial`](mixamo-tutorial.md) and
[`examples/mixamo.animsmith.toml`](../examples/mixamo.animsmith.toml) show the
profile with an in-place XZ declaration. Bevy examples are
`bevy.animsmith.toml`, `bevy-v2.animsmith.toml`, and `bevy-v3.animsmith.toml`.
CI parse-tests these examples and this page's key inventory; update that test
when adding configuration authority.

## Common mistakes and errors

- `unknown field` or `unknown variant`: spelling/casing is not supported.
- `must be a finite non-negative number`: a cap, tolerance, or amplitude is
  negative, NaN, or infinite. Positive-only values have the corresponding
  positive error.
- `cannot declare both ...`: choose one runtime selector spelling and one
  horizontal movement-owner spelling per entry.
- `bad config ...`: parser, validation, profile, and transition-family errors
  include the selected path and exit `2`; no asset is linted. A coverage gap
  during a valid run is evidence of a missing prerequisite, not a parse error.

When changing a contract, run with `--format json` and retain the config path
and tool version with the output. A clean result is evidence for that file and
declaration set, not certification of retargeting, engine runtime behavior, or
artistic quality.
