# animsmith CLI

`animsmith` is designed for artist inner loops, CI gates, and pipeline
automation. It reads glTF/GLB everywhere; the released default build also
reads FBX through the `fbx` feature. The CLI is tested on Linux, macOS,
and Windows.

Install the released CLI with:

```console
cargo install animsmith
```

From a source checkout, prefix commands with `cargo run -p animsmith --`.

## Help

Every command has generated help:

```console
animsmith --help
animsmith lint --help
animsmith fix --help
```

There are no man pages yet, so `--help` is the canonical installed CLI
reference. The help output reflects compile-time features: a
`--no-default-features` binary omits feature-gated commands such as
`report` and `convert`.

## Commands

```console
animsmith inspect <file>
animsmith measure <file...> [--format text|json]
animsmith lint <file...> [--format text|json] [--select id[,id]] [--allow id[,id]] [--deny-warnings]
animsmith report <file> -o <report.html> [--clip name]
animsmith transform <file> -o <out.glb> [--clip name] [--slice START:END] [--hold-extend SECONDS] [--gait-anchor] [--fps N]
animsmith fix <file> (-o <out.glb>|--in-place|--dry-run) [--repair id[,id]]
animsmith convert <in.fbx|in.glb|in.gltf> -o <out.glb|out.gltf> [--animation-only]
animsmith diff <before> <after> [--format text|json]
```

`--config animsmith.toml` is global. Without it, the CLI auto-loads
`./animsmith.toml` when present and otherwise uses built-in defaults.

## Exit Codes

| Code | Meaning |
|---:|---|
| 0 | Clean, or warnings only. |
| 1 | At least one failing finding, a significant `diff`, or pending repairs under `fix --dry-run`. |
| 2 | Operator/tool error: unreadable input, bad config, unsupported format, or invalid flags. |

Use `lint --deny-warnings` when CI should fail on warnings as well as
errors. `fix --dry-run` is the repair check mode: it exits 1 when the
file has repairable defects and 0 otherwise, so CI can gate on "this
asset needs fixing" without writing anything. The exit code
reflects repairs `fix` would actually perform: tracks it cannot patch
(data-URI buffers, cubic `quat-norm` tracks, quantized rotations) are printed as
`skipped[...]` but do not fail the check — gate on `lint` (the
`quat-norm` or `quat-flip` checks) when detection alone should fail CI.

## Feature Flags

The default binary enables `fbx` and `report`.

```console
cargo install animsmith
cargo install animsmith --no-default-features
```

The no-default-features build has no C toolchain dependency and keeps the
glTF-only workflow: `inspect`, `measure`, `lint`, `transform`, `fix`, and
`diff`. The HTML `report` command is controlled by the `report` feature.
`convert` accepts FBX or glTF input (a glTF input is re-emitted,
carrying its geometry) but is compiled only with the `fbx` feature.

## Repairs

Every repair is safe, lossless, and idempotent — that is the bar for
adding one. Repairs have stable ids so scripts can pin exact behavior:

| Repair id | Behavior |
|---|---|
| `quat-norm` | Unit-normalizes finite, non-zero LINEAR/STEP quaternion keys. This is lossless because scaling a quaternion does not change the represented rotation after normalization. CUBICSPLINE tracks are skipped to preserve tangents. |
| `quat-flip` | Normalizes adjacent quaternion keys to the same hemisphere. This is lossless because `q` and `-q` represent the same rotation. |

By default `fix` runs every repair. `--repair id[,id]` pins an exact
list (`animsmith fix --help` names the valid ids). `fix` writes only
when you explicitly choose a destination; `--dry-run` reports and sets
the exit code without writing:

```console
animsmith fix clip.glb --dry-run
animsmith fix clip.glb -o fixed.glb
animsmith fix clip.glb --in-place
animsmith fix clip.glb --repair quat-norm,quat-flip -o fixed.glb
```

## Machine Output

`measure`, `lint`, and `diff` support `--format json`. The native JSON
contract is the source of truth and is versioned with `schema_version`.
See [output.md](output.md) and
[schemas/output-v1.schema.json](schemas/output-v1.schema.json).

Native JSON is deliberately shaped so serializers can be added later
without redesigning the checks: SARIF for code scanning, GitLab Code
Quality/CodeClimate for MR widgets, JUnit XML for CI dashboards, and CSV
for ad-hoc analysis.
