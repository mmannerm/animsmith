# Machine-Readable Output

animsmith's native JSON is a versioned envelope. It is intended to be the
stable source of truth for pipelines; other formats such as SARIF, GitLab
Code Quality, JUnit XML, CSV, or HTML should be serializers over this
contract.

## Common Envelope

```json
{
  "schema_version": 1,
  "schema": "https://raw.githubusercontent.com/mmannerm/animsmith/v0.1.0/docs/schemas/output-v1.schema.json",
  "tool": { "name": "animsmith", "version": "0.1.0" },
  "command": "lint",
  "summary": { "files": 1, "findings": { "error": 0, "warning": 1, "note": 0 } },
  "files": []
}
```

`schema_version` changes only on breaking JSON changes. The `schema`
field points at the schema file from the Cargo package version's release
tag so consumers can validate against an immutable contract snapshot.
Additive fields may appear within the same version; consumers should
ignore fields they do not understand.

## `measure` and `lint`

`measure --format json` emits `files[].measurements` and omits
`files[].findings`. `lint --format json` emits both. Each file record has:

| Field | Meaning |
|---|---|
| `path` | Input path as passed to the CLI. |
| `rig.profile` | Resolved built-in or custom profile name. |
| `rig.resolved_roles` | Role-to-bone-name map used by role-dependent checks. |
| `measurements` | Per-clip metric map. |
| `findings` | Structured lint findings; omitted by `measure`. |

Findings carry `check_id`, `severity`, optional `clip`, optional `bone`,
optional `time_s`, optional measured/expected values, and a human message.
Treat `check_id` as the stable key for automation; treat `message` as
display text.

## `diff`

`diff --format json` emits a compact envelope with `inputs`, `summary`,
and `deltas`:

```json
{
  "schema_version": 1,
  "schema": "https://raw.githubusercontent.com/mmannerm/animsmith/v0.1.0/docs/schemas/output-v1.schema.json",
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
