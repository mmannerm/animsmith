# AGENTS.md

Project rules for coding agents. Maintainer agents use the shared
[engineering instructions](https://github.com/mmannerm/agents/blob/main/AGENTS.md)
and [audit-task workflow](https://github.com/mmannerm/agents/blob/main/skills/audit-task/SKILL.md)
from user-level configuration. Resolve that workflow to the central
source; report a missing or conflicting installation instead of silently
skipping required review or using a stale local copy. Record its revision
in the review. Project contracts and gates below remain authoritative.

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for PR process, Conventional
Commits, documentation freshness and labels,
[DEVELOPMENT.md](DEVELOPMENT.md) for setup and commands, and
[DESIGN.md](DESIGN.md) for crate boundaries. New task worktree:
`just worktree <agent>/<branch>`.

## Audit

- Gate: `just gates`.
- Outside CI: when the diff touches measurement code (`metrics.rs`,
  `sample.rs`, check algorithms), run `just golden` with
  `ANIMSMITH_GOLDEN_GLB` set if the licensed reference asset is available,
  and say either way. Run local engine, authorized-asset and manifest
  validation when a claim depends on them.
- Invariants: [.agents/audit-invariants.md](.agents/audit-invariants.md),
  cited as `invariant-N`.
- Test bar: prefer analytic fixtures (synthetic clips with mathematically
  known metrics) and mutation tests (corrupt one field, assert exactly that
  finding names it).
