# Machine-Readable Output

animsmith's native JSON is the stable source of truth for pipeline adapters.
Text and Markdown lint output are presentation views over the same evaluation
results. The HTML report remains a sampled-motion view with content findings;
future machine serializers should project the JSON contract.

## Contract identities

Every JSON command emits output contract v2 with the immutable protocol
identity `urn:animsmith:schema:output:2`. The retrievable schema is
[`output-v2.schema.json`](schemas/output-v2.schema.json); its repository URL
is a retrieval location, not the protocol identity.

Measurement evidence is nested and independently versioned as
`urn:animsmith:schema:measurements:1`. Its retrievable schema is
[`measurements-v1.schema.json`](schemas/measurements-v1.schema.json). A future
measurement-definition change can therefore bump that contract without
redesigning the outer result envelope.

The project is alpha, so the final v2 cutover intentionally does not read or
emit earlier v1 or preview reports. Regenerate old reports with the current
`animsmith measure --format json` before passing them to `diff`.

## Common envelope

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
  "tool": {
    "name": "animsmith",
    "version": "0.1.0",
    "source": {
      "revision": "0123456789abcdef0123456789abcdef01234567",
      "dirty": false
    }
  },
  "command": "measure",
  "summary": { "files": 1 },
  "files": []
}
```

`tool.version` is the package's plain semantic version. Source revision and
dirty state are separate fields so automation never has to parse a decorated
version string. Packaged source records its Cargo VCS revision and leaves
`dirty` as `null`; builds without trustworthy VCS metadata may leave both
fields `null`.

Operator failures do not emit a JSON envelope. They exit 2, write a diagnostic
to stderr, and leave stdout empty. Content findings exit 1 at the configured
threshold; coverage gaps are evidence and are nonblocking by default.

## `measure` and `lint`

Both commands put evidence under `files[].measurements`:

```json
{
  "schema_version": 1,
  "schema": "urn:animsmith:schema:measurements:1",
  "clips": {},
  "meshes": []
}
```

`clips` maps clip names to duration, frame count, animated bones, rotation
ranges, and optional role-dependent gait, seam, and speed metrics. `meshes`
is omitted when empty; when present it carries vertex counts, finite AABBs,
skin influence counts, and weight-sum ranges. Measurement contract v1 preserves
the currently implemented fields. Issue #190 remains the authority for their
geometry-domain semantics and can advance the nested contract independently.

Lint adds exactly one `files[].checks[]` record for every built-in catalog
check. Each record keeps these dimensions independent:

- `selection`: `selected` or `unselected`;
- `configuration`: `enabled` or `disabled`;
- `applicability`: `applicable` or `not_applicable`;
- `evaluation`: `complete`, `partial`, or `not_evaluated`;
- content `findings`;
- completed `evaluated_scopes` and typed coverage `gaps`.

Gap and scope `code` fields are the machine contract; `message` is display
text and must never be parsed. Disabled, unselected, and not-applicable checks
are not artificial gaps. A partial check has at least one completed scope and
at least one gap. Applicable work that completed nothing has a gap and no
content findings. A scope can appear as completed and also be named by a gap
when a group-level calculation covered some but not all members.

Built-in gap codes are:

| Gap code | Meaning | Emitted by |
|---|---|---|
| `roles_unresolved` | Required semantic rig roles were not resolved. | `loop-seam`, `root-motion-speed`, `in-place`, `foot-slide`, `gait-group` |
| `measurement_unavailable` | A required numeric measurement could not be produced or did not meet its evidence floor. | `loop-seam`, `root-motion-speed`, `in-place`, `foot-slide`, `gait-group` |
| `insufficient_measurable_members` | Fewer than two gait-group members produced usable phases. | `gait-group` |
| `members_not_evaluated` | Some configured gait-group members did not produce usable phases. | `gait-group` |
| `invalid_declared_fps` | A declared frame rate was zero, negative, or non-finite. | `fps` |
| `insufficient_rotation_evidence` | Too few usable rotation tracks existed for a bind-pose comparison. | `bind-pose` |

Built-in completed/gap scope codes are:

| Scope code | Work unit | Emitted by |
|---|---|---|
| `member_existence` | Configured gait-group members were checked for existence. | `gait-group` |
| `phase_measurement` | One named clip's gait phase was measured or lacked usable evidence. | `gait-group` |
| `phase_coherence` | One named gait group's measurable phases were compared. | `gait-group` |
| `loop_seam` | One named clip's positional loop seam was measured. | `loop-seam` |
| `root_motion_speed` | One named clip's root-motion speed was measured. | `root-motion-speed` |
| `travel_mode` | One named clip's in-place/root-motion declaration was judged. | `in-place` |
| `foot_stance` | Whole-clip prerequisites for stance analysis were evaluated. | `foot-slide` |
| `left_foot_stance` | The named clip's left foot/toe stance was evaluated. | `foot-slide` |
| `right_foot_stance` | The named clip's right foot/toe stance was evaluated. | `foot-slide` |
| `frame_grid` | The named clip's declared frame grid was evaluated. | `fps` |
| `first_frame_rest_delta` | The named clip's first-frame/rest-pose rotation evidence was evaluated. | `bind-pose` |

The built-in gap and scope registries live in `animsmith_core`. A contract test
iterates both registries and fails if this reference table falls behind them.
Custom checks may add namespaced gap codes and their own scope vocabulary.

`summary.checks` reports a `total` and four independent partitions. Each of
`selection`, `configuration`, `applicability`, and `evaluation` sums to that
same total. `summary.checks.gaps` counts typed gaps, while
`summary.findings` counts content findings by severity.

`lint --format json` deliberately rejects `--allow` so machine evidence is
never deleted. `--allow` remains available for text and Markdown presentation
and their exit policy. Text and Markdown render coverage gaps separately from
findings and group repeated gaps by `(check_id, code)` for readability. Group
counts still reflect every underlying per-scope JSON gap.

## Findings and numeric values

Findings carry `check_id`, `severity`, optional `clip`, `bone`, `time_s`,
`measured`, and `expected` fields, plus a human message. Treat `check_id` and
the structured fields as automation data; treat `message` as display text.
The nested `check_id` intentionally repeats its owning check record so a
finding stays self-describing when extracted or consumed through the embedded
API; the evaluator rejects mismatched parent/child ids.

Numeric equality in the JSON contract means equality of decoded JSON numbers,
not byte-for-byte lexical spelling. For example, `1`, `1.0`, and `1e0` denote
the same numeric value to a conforming adapter.

## `diff`

`diff --format json` uses the same output v2 header and emits `inputs`, a
delta count, and structured metric deltas:

```json
{
  "schema_version": 2,
  "schema": "urn:animsmith:schema:output:2",
  "tool": {
    "name": "animsmith",
    "version": "0.1.0",
    "source": { "revision": null, "dirty": null }
  },
  "command": "diff",
  "inputs": { "before": "old.glb", "after": "new.glb" },
  "summary": { "deltas": 1 },
  "deltas": [
    { "clip": "walk", "metric": "speed_mps", "before": 1.0, "after": 1.2, "note": "moved" }
  ]
}
```

`diff` accepts asset files or one-file v2 `measure`/`lint` reports carrying
measurement contract v1. Multi-file reports and unsupported contract versions
are rejected as operator errors.
