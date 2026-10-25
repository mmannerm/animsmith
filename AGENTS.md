# AGENTS.md

Project rules for coding agents. General rules (simplicity, workflow,
attribution, the `audit-task` skill) come from the user-level agent
instructions; this file adds only what is specific to animsmith.

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for PR process, Conventional
Commits, documentation freshness and labels,
[DEVELOPMENT.md](DEVELOPMENT.md) for setup and commands, and
[DESIGN.md](DESIGN.md) for crate boundaries.

- New task worktree: `just worktree <agent>/<branch>`.
- Maintainers merge: leave a ready PR for their decision.

## Audit

- Gate: `just gates`, once per head before pushing.
- Outside CI: when the diff touches measurement code (`metrics.rs`,
  `sample.rs`, check algorithms), run `just golden` with
  `ANIMSMITH_GOLDEN_GLB` set if the licensed reference asset is available,
  and say either way. Run local engine, authorized-asset and manifest
  validation when a claim depends on them.
- Animation-pack workflow changes: reuse the `checks / animation-pack` CI
  result; it runs the same validator as `just animation-pack-skill`.
- Invariants: [.agents/audit-invariants.md](.agents/audit-invariants.md),
  cited as `invariant-N`.
- Docs: apply CONTRIBUTING.md's documentation-impact list; architectural
  decisions update DESIGN.md.
- Test bar: prefer analytic fixtures (synthetic clips with mathematically
  known metrics) and mutation tests (corrupt one field, assert exactly that
  finding names it).
