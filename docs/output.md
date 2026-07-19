# Machine-Readable Output

animsmith's native JSON is a versioned envelope. It is intended to be the
stable source of truth for pipelines; other formats such as SARIF, GitLab
Code Quality, JUnit XML, CSV, or HTML should be serializers over this
contract.

## Common Envelope

```json
{
  "schema_version": 1,
  "schema": "https://raw.githubusercontent.com/mmannerm/animsmith/main/docs/schemas/output-v1.schema.json",
  "tool": { "name": "animsmith", "version": "0.1.0" },
  "command": "lint",
  "summary": { "files": 1, "findings": { "error": 0, "warning": 1, "note": 0 } },
  "files": []
}
```

This JSON envelope is the only stable, machine-readable output. `lint`
also offers `--format markdown`, but that rendering is presentation-only
for CI comments and asset-review threads (see
[cli.md](cli.md#ci-comments-lint---format-markdown)): it carries no
schema and no stability guarantees, and its layout may change between
releases. Parse JSON, not Markdown.

`schema_version` changes only on breaking JSON changes after this first
published envelope. Earlier development JSON shapes were not a published
contract. Until the first manifest-versioned release that contains this
schema file, the `schema` field points at `main`; release PRs may pin it
to the matching `vX.Y.Z` tag once that tag will contain the schema.
Additive fields may appear within the same version; consumers should
ignore fields they do not understand.

## Provisional v2 coverage preview

Issue #193 is incubating a result model that represents content findings and
evaluation coverage separately. During that experiment, `lint --format
json-v2-preview` emits `schema_version: 2` against
[`output-v2-preview.schema.json`](schemas/output-v2-preview.schema.json).
Its `schema` field is exactly
`https://raw.githubusercontent.com/mmannerm/animsmith/main/docs/schemas/output-v2-preview.schema.json`;
the preview schema's `$id`, the CLI constant, and this documented URL are
checked together to prevent contract drift.
The spelling and field names are intentionally preview-only; default
`--format json` remains the published v1 contract.

Each preview `files[].checks[]` record independently reports:

- `selection`: `selected` or `unselected`;
- `configuration`: `enabled` or `disabled`;
- `applicability`: `applicable` or `not_applicable`;
- `evaluation`: `complete`, `partial`, or `not_evaluated`;
- content `findings`;
- completed `evaluated_scopes` and typed coverage `gaps`.

Gap `code` and scope `code` are the machine fields; messages remain display
text. Disabled and unselected checks are activation states, not artificial
gaps. A not-applicable check is not reported as a clean evaluation. Partial
`gait-group` evaluation is the proving case: member-existence validation can
complete (and can emit a content error) while role-dependent phase coherence
reports a `roles_unresolved`, `insufficient_measurable_members`, or
`members_not_evaluated` gap. Legacy custom checks that emit diagnostic
findings through `run` retain that incomplete evidence as a
`legacy_diagnostic` gap rather than a v2 content finding.

The preview deliberately rejects `--allow` instead of deleting findings from
machine evidence. Content errors still exit 1, warnings exit 0 unless denied,
and coverage gaps are visible but nonblocking by default. Operator failures
remain exit 2 plus stderr during the experiment; whether final v2 adds a JSON
operator-error envelope is still an open contract decision.

## `measure` and `lint`

`measure --format json` emits `files[].measurements` and omits
`files[].findings`. `lint --format json` emits both. Each file record has:

| Field | Meaning |
|---|---|
| `path` | Input path as passed to the CLI. |
| `rig.profile` | Resolved built-in or custom profile name. |
| `rig.resolved_roles` | Role-to-bone-name map used by role-dependent checks. |
| `measurements` | Per-clip metric map. |
| `meshes` | Per-mesh geometry measurements; present only when the input carried scene assets (see below). |
| `findings` | Structured lint findings; omitted by `measure`. |

Findings carry `check_id`, `severity`, optional `clip`, optional `bone`,
optional `time_s`, optional measured/expected values, and a human message.
Treat `check_id` as the stable key for automation; treat `message` as
display text.

The findings array also carries evaluation-coverage diagnostics: a
check with declared work whose prerequisite is missing — typically an
unresolved rig role — reports a `note` finding whose message begins
with `skipped:`. The v1 envelope has no separate coverage field, so a
skip note is not structurally distinguishable from a content note, and
an absent finding does not distinguish a completed clean evaluation
from a check that was idle for this document and config, disabled with
`severity = "off"`, or outside `--select` —
[reading a lint run](game-ready-clips.md#reading-a-lint-run) separates
those states. Gate on findings and exit codes; do not infer evaluation
coverage from silence.

`measure` reports static (animation-independent) geometry when the input
carries meshes (FBX always; glTF when the file has mesh data). Each entry
in `files[].meshes` is:

| Field | Meaning |
|---|---|
| `name` | Mesh name. |
| `vertex_count` | Total position count across the mesh's primitives (indexed meshes count unique vertices, unindexed count corners). |
| `aabb` | `{ "min": [x,y,z], "max": [x,y,z] }` bounding box in scene units; omitted for a mesh with no finite positions (a mesh with none, or whose positions are all non-finite). |
| `max_joints_per_vertex` | Highest non-zero skin-influence count on any vertex; `0` for an unskinned mesh. |
| `weight_sum_min` / `weight_sum_max` | Range of per-vertex skin-weight sums (≈1.0 for a well-formed skin); omitted for an unskinned mesh. |

The `meshes` array is omitted entirely for asset-less inputs, so
skeleton/animation-only reports are unchanged. This is an additive field
under the same v1 schema.

## `diff`

`diff --format json` emits a compact envelope with `inputs`, `summary`,
and `deltas`:

```json
{
  "schema_version": 1,
  "schema": "https://raw.githubusercontent.com/mmannerm/animsmith/main/docs/schemas/output-v1.schema.json",
  "tool": { "name": "animsmith", "version": "0.1.0" },
  "command": "diff",
  "inputs": { "before": "old.glb", "after": "new.glb" },
  "summary": { "deltas": 1 },
  "deltas": [
    { "clip": "walk", "metric": "speed_mps", "before": 1.0, "after": 1.2, "note": "moved" }
  ]
}
```

`diff` accepts asset files or a single-file `measure`/`lint` JSON report.
Multi-file reports are rejected because there is no unambiguous pair to
compare.
