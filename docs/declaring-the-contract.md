# Declaring the contract: who writes what

`animsmith.toml` is where a team writes down what a clip is *supposed* to
do. Mechanical checks need none of it — they run on any file. Everything
else waits for a declaration: with no `loop = true`, the seam checks have
nothing to judge and record themselves as not applicable, and a check that
needs a rig role it cannot resolve reports a typed coverage gap instead of
guessing.

So the question this page answers is not what the keys mean — the
[configuration reference](configuration-reference.md) is the exhaustive
authority for that — but **who is allowed to write each one**. Some
surfaces are facts of the export and an animator can declare them alone.
Some are decisions only the game can make, and an animator who guesses at
them writes a contract that fails for the wrong reason. The rest have to be
agreed once and then committed beside the assets, like code.

## The surfaces, and who owns each

"Owner" is who decides the value, not who types it: the file itself is
committed once, reviewed like code, and read by everyone.

| Surface | What it declares | Who owns the value | Where the value comes from | Minimal example |
|---|---|---|---|---|
| `[rig] profile`, `[rig.roles]` | Which bones play hips, feet, toes, root and the rest, so role-dependent checks can run at all | Artist alone — it is a fact of the exported skeleton | `animsmith inspect` prints the profile it resolved and the bones it bound | `[rig]`<br>`profile = "mixamo"` |
| `[rig] required_bones` | Bones that must exist even without keys: sockets, IK targets, mask roots | Decided together — the game names them, the export must carry them | The engine project's attachment, mask and IK code | `[rig]`<br>`required_bones = ["weapon_socket"]` |
| `[clips."<name>"] loop` and its `max_loop_*` caps | That the clip is a cycle, and how far its endpoints may drift before that is a defect | Artist alone for `loop`; the caps are a project bar decided together | The take was authored as a cycle; the caps start at the defaults and tighten once the back catalogue is clean | `[clips.walk]`<br>`loop = true` |
| `[clips."<name>"] duration_s`, `fps` | The length and the authored frame grid the clip owes the runtime | Decided together — the grid is an export setting, the length is a gameplay expectation | The DCC export range and frame rate, confirmed by `animsmith measure` | `[clips.walk]`<br>`fps = 30.0` |
| `[clips."<name>"] movement_owner_xz`, `movement_owner_y`, `movement_owner_yaw` | Whether gameplay code or the animation moves the character on each axis | Game developer alone — it is a character-controller decision | The engine project: whichever of the controller and the root motion is actually driving movement | `[clips.walk]`<br>`movement_owner_xz = "gameplay"` |
| `[clips."<name>"] speed_mps` | The travel speed the controller expects from a root-motion clip | Decided together — the controller states it, the clip has to hold it | `animsmith measure` reports the measured travel; the pin is the agreed value, not the measurement | `[clips.run]`<br>`speed_mps = { value = 3.5, tolerance = 0.2 }` |
| `[clips."<name>"] animates_bones` | Bones this take must not only carry but actually move | Decided together — gameplay needs the motion, the rig has to produce it | The engine project's expectations, checked against the DCC export | `[clips.walk]`<br>`animates_bones = ["arm_l"]` |
| `[gait_groups."<name>"]`, `[sync_groups."<name>"]` | Which clips the runtime blends as one directional ring, and which it plays at the same time | Game developer alone — a blend ring is a runtime intent, invisible in the files | The blend graph or animation state machine that will crossfade them | `[gait_groups.walk-ring]`<br>`clips = ["walk_f", "walk_b"]`<br>`max_gait_phase_spread = 0.15` |
| `[runtime_nodes] selectors` | Attachment nodes whose rest-world scale the runtime relies on | Decided together — the game names the sockets, the export sets their scale | The engine project's socket names; `[checks.rest-world-scale]` carries the expected factor | `[runtime_nodes]`<br>`selectors = ["weapon_socket"]` |
| `[engine]`, `[engine.settings]` | Which importer boundary an engine prediction is about | Game developer alone — it is the shipping project's own version and settings | The engine project: its version, importer and import settings | `[engine]`<br>`profile = "bevy"` |
| `[checks.<id>] severity` | What blocks the gate, what only reports, and what is off for now | Decided together — it is the project's quality bar | A lint run over the current assets shows what promoting a check would cost | `[checks.quat-flip]`<br>`severity = "note"` |
| Collection manifest and `[transition_families]` | Which physical files hold which logical clips, and which takes form a transition family | Game developer alone, with the pack owner — it is an inventory of a delivery | The delivery itself; see [collection contracts](collection-contracts.md) | See [collection contracts](collection-contracts.md#transition-families-148) |

Two rules follow from that table. An artist can go a long way alone:
a rig profile, `loop`, `fps` and `animates_bones` already arm the checks
that catch a popped seam, a short channel and a frozen arm. But
`movement_owner_*`, `speed_mps` and the blend groups are the game's
decisions — declared without the game developer, they measure the clip
against a guess.

Nothing is inferred from omission. Leaving `loop` out does not declare the
clip non-cyclic; it declares nothing, and the loop checks stay not
applicable. Ownership is the same: each movement axis is independent, and
an omitted one is never assumed.

## A complete minimal contract

This is [`examples/walk.animsmith.toml`](../examples/walk.animsmith.toml),
the committed contract for the sample walk cycle in
[first lint in 60 seconds](first-lint.md) — the smallest file that makes
the semantic checks fire:

```toml
# A runnable contract for the committed `walk.glb` / `walk-dirty.glb`
# example rigs — the smallest config that makes the semantic checks fire.
#
#   animsmith lint --config examples/walk.animsmith.toml examples/assets/walk.glb
#
# See examples/character.animsmith.toml for the full game-character shape
# (locomotion rings, speed pins, per-check overrides).

[rig]
# The rig's bone names (pelvis / foot_l / foot_r) resolve the built-in
# ue-mannequin profile, so "auto" binds hips + feet with no inline map.
profile = "auto"

[clips.walk]
# Declaring the clip as a loop arms role-free per-bone C0/C1 checks plus the
# locomotion-specific feet-relative-to-hips loop-seam check.
loop = true
# Gameplay owns horizontal travel (the clip is a treadmill cycle), so
# `in-place` expects no measured XZ root travel.
movement_owner_xz = "gameplay"
# A per-clip cap can be stricter than a project-wide loop default. Omitted
# dimensions continue to inherit their global caps.
max_loop_rotation_delta_deg = 0.5

[checks.loop-seam]
# Fail a wrap discontinuity more than 1.6× the in-clip stride step.
max_ratio = 1.6

[checks.loop-closure]
# Model-space endpoint pose caps. Inherited root travel may require a larger
# position cap (or disabling this check) for extracted root-motion clips.
max_position_delta_m = 0.01
max_rotation_delta_deg = 1.0

[checks.loop-seam-vel]
# Fail when incoming/outgoing model-space linear velocity differs by >0.1 m/s.
max_velocity_delta_mps = 0.1

[checks.loop-seam-rot]
# Fail when shortest-path model-space angular velocity differs by >5 deg/s.
max_angular_velocity_delta_degps = 5.0

# The same three caps can live in a `[clips."name_*"]` glob. They do not
# configure angular seam velocity, which is outside these overrides.
```

Run it against the committed clip and the loop checks pass; run it against
the same clip cut a quarter-cycle short and they fail with the measured
distance. The [examples cookbook](../examples/README.md#4-a-project-contract-config)
walks that pair, and
[`examples/character.animsmith.toml`](../examples/character.animsmith.toml)
grows the same shape to a full character with rings, speed pins and
per-check overrides.

## Where to go next

The [configuration reference](configuration-reference.md) owns every key,
default, unit and precedence rule; the
[built-in check reference](built-in-checks.md) owns what each declaration
arms. When a declaration turns out to be wrong rather than missing, the
[symptom index](symptoms/README.md) routes what you see in the engine to
the page that walks it.
