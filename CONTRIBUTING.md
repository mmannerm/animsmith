# Contributing

Thanks for improving animsmith. This project is pre-1.0, so small
contract changes can still happen, but every user-visible change should
be explicit, tested, and documented.

## Pull Request Flow

All changes land through pull requests. Keep each PR focused on one
behavioral or documentation goal, and prefer follow-up issues over
expanding a PR after review has started.

Use this lifecycle for non-trivial changes:

1. Start from current `main`.
2. Implement the change with behavioral tests when behavior changes.
3. Update docs in the same PR whenever public behavior, commands,
   output, feature flags, or supported workflows change.
4. Run the required local gates from [DEVELOPMENT.md](DEVELOPMENT.md).
5. Open a draft PR with a description of the behavior change, the chosen
   design, verification performed, and known out-of-scope work.
6. Address review findings and run the project audit checklist for
   substantial changes.
7. Leave the PR for a maintainer merge decision.

Trivial documentation-only changes can use a shorter path, but they
still need a PR, a conventional title, and enough verification to show
the rendered links and affected files are correct.

## Conventional Commits

Every PR title and every non-merge commit that lands on `main` must use
Conventional Commits. CI enforces the accepted types from
[.commitlintrc.yml](.commitlintrc.yml):

```text
feat fix perf revert chore ci docs style refactor test build
```

Use `docs:` for documentation-only changes. Use `feat:`, `fix:`, or
`perf:` only when the commit should affect release notes and version
calculation. Release automation groups the merged conventional commits
into the changelog.

Agent-authored commits also need the agent attribution trailer required
by that agent's local instructions.

## Documentation Freshness

The PR description must call out documentation impact. If a change
affects user-visible behavior or public contracts, update the relevant
docs in the same PR or link a follow-up issue labeled `type:docs`.

Documentation impact includes:

- CLI commands, flags, exit codes, or examples.
- Machine-readable JSON output or schema ids.
- Public Rust symbols, crate features, loader boundaries, or README
  claims.
- Check ids, severities, thresholds, config keys, or measurement
  semantics.
- Release, support, security, or contributor workflows.

Do not duplicate durable process rules across multiple files. This file
owns contributor process. [DEVELOPMENT.md](DEVELOPMENT.md) owns local
setup and verification commands. Agent files may add agent-specific
deltas, but should link back here for the shared process.

## Audit Expectations

Run the project audit checklist before asking for a merge on substantial
changes: new checks, subcommands, measurement semantics, output
contracts, dependency additions, write/fix path changes, or broad docs
restructures.

The audit should check:

- Simplicity: the design is scoped to the issue and avoids needless
  abstractions.
- Tests: behavior changes have focused coverage and the local gates
  match CI expectations.
- Invariants: crate boundaries and dependency rules still match
  [DESIGN.md](DESIGN.md).
- Documentation: README files, docs, and PR text stay fresh.

Follow-up work found during audit should become GitHub issues rather
than TODO comments. Search existing issues first, then file a focused
issue with the right `type:*` and `priority:*` labels.

## Labels And Milestones

Use `type:*` labels to describe the work area:

- `type:bug` for incorrect behavior.
- `type:feature` for new capabilities or enhancements.
- `type:docs` for documentation, examples, and tutorial gaps.
- `type:refactor` for behavior-preserving code structure changes.
- `type:chore` for maintenance work.

Use `priority:high`, `priority:medium`, or `priority:low` when priority
is known. Put pre-publish work in the active `0.1.0` milestone unless a
maintainer chooses another milestone.

## Merge Policy

Maintainers merge PRs after review, verification, and any required audit
are complete. animsmith uses merge commits so every branch commit lands
on `main` with its own conventional subject; do not rely on squash merge
to repair commit history after review.
