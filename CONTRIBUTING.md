# Contributing

Thanks for improving animsmith. This project is pre-1.0, so small
contract changes can still happen, but every user-visible change should
be explicit, tested, and documented.

## Pull Request Flow

All changes land through pull requests. Keep each PR focused on one
behavioral or documentation goal, and prefer follow-up issues over
expanding a PR after review has started.

Before coding, state the user outcome, invariants, non-goals, and
simplest maintainable end state if the change were designed today.
Inspect related issues and existing authorities before adding a new
concept. Focus means one coherent outcome, not the smallest diff;
prefer a larger coherent refactor over preserving duplicate authority.

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

## MSRV Policy

The minimum supported Rust version is a floor, not a ratchet. It moves
only when something in the tree actually requires a newer compiler —
normally a dependency raising its own `rust-version`, occasionally a
language feature the code has a concrete reason to adopt. It is never
raised merely because a newer stable exists.

The ceiling on that movement is six months: a bump must not require a
compiler released within the last six months, so consumers who pin
toolchains always have a two-release window to catch up. Six months is
stated in months rather than as N-minus-releases because months are the
unit downstream users experience, and the rule then survives a change in
Rust's release cadence.

Day-to-day development still happens on current stable, and CI checks
stable on Linux, macOS, and Windows. The MSRV is what the published
crates promise, not what maintainers run.

`workspace.package.rust-version` in the root `Cargo.toml` is the single
source of truth. Nothing else states the number independently: the CI
MSRV job reads it out of the manifest, and two gates hold the prose to
it (see the Toolchain section of [DEVELOPMENT.md](DEVELOPMENT.md)). A
bump is therefore a one-line manifest edit plus whatever prose the gates
flag.

Because an MSRV bump is a compatibility change for downstream crates, it
warrants a minor version rather than a patch, and a changelog entry
saying so.

Workflows must not pin `dtolnay/rust-toolchain` to a version-shaped ref
such as `@1.88`. Dependabot reads those as action tags and bumps them,
which silently rewrites the Rust version — it once proposed `@1.100`, a
nightly number, for the MSRV job. Use a branch ref (`@stable`,
`@nightly`) or a SHA pin with an explicit `toolchain:` input;
`scripts/check-github-community-files.sh` enforces this.

## Documentation Freshness

The PR description must call out documentation impact. If a change
affects user-visible behavior or public contracts, update the relevant
docs in the same PR or link a follow-up issue labeled `type:docs`.

Documentation impact includes:

- CLI commands, flags, exit codes, or examples.
- Machine-readable JSON output or schema ids.
- Public Rust symbols, crate features, loader boundaries, or README
  claims.
- Task guides, tutorials, example projects, and current version or
  project-status claims.
- Check ids, severities, thresholds, config keys, or measurement
  semantics.
- Release, support, security, or contributor workflows.

Review the complete affected stakeholder journey, not only files named
by the issue or touched by the implementation. Search the root and crate
READMEs, `docs/`, `examples/`, rustdoc, and release/process docs for
superseded commands, identifiers, schemas, versions, examples, and
status claims. Preserve clearly historical references.

Mechanical freshness checks cover the parts that are cheap and reliable:
schema URL consistency, GitHub community files and PR-template coverage,
crate README/package inclusion, docs.rs manifest metadata, and rustdoc
missing-docs enforcement. Semantic docs freshness remains a review
requirement: update docs in the same PR as the behavior change, or link a
focused `type:docs` issue.

During pre-1.0 development, published package READMEs intentionally link
deeper repository docs to latest `main` with
`github.com/mmannerm/animsmith/blob/main/...` URLs. If a release later
needs version-pinned README links, update [RELEASING.md](RELEASING.md)
and the package-readiness gate in the same PR.

Do not duplicate durable process rules across multiple files. This file
owns contributor process. [DEVELOPMENT.md](DEVELOPMENT.md) owns local
setup and verification commands. Agent files may add agent-specific
deltas, but should link back here for the shared process.

## Audit Expectations

Run the project audit checklist before asking for a merge on substantial
changes: new checks, subcommands, measurement semantics, output
contracts, dependency additions, write/fix path changes, or broad docs
restructures.

Substantial agent-authored changes also require one reciprocal cross-model CLI
audit: Codex invokes Claude, and Claude invokes Codex. The versioned
[audit workflow](.claude/skills/audit-task/SKILL.md) owns reviewer selection,
model/effort, session reuse, attribution, and exact-head evidence rules.

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
is known. Put pre-release work in the active release milestone unless a
maintainer chooses another milestone.

## Merge Policy

Maintainers merge PRs after review, verification, and any required audit
are complete. animsmith uses merge commits so every branch commit lands
on `main` with its own conventional subject; do not rely on squash merge
to repair commit history after review.
