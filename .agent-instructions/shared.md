# Shared Agent Instructions

These instructions apply to every coding agent (Claude, Codex, others)
working in this repository.

## Instruction maintenance

Keep shared project rules in this file. Keep only tool-specific
bootstrap, settings, and skill-location notes in `CLAUDE.md` and
`AGENTS.md`.

## Agent attribution

Agent-authored GitHub work must be attributable so reviewers can tell
which agent did what.

- Agent-authored commits must include an agent `Co-authored-by` trailer,
  e.g. `Co-authored-by: Codex <codex@openai.com>` or
  `Co-authored-by: Claude <claude@anthropic.com>` (model-qualified
  variants are fine).
- Agent-authored PR descriptions, PR comments, and review comments must
  include a clear attribution line naming the agent that wrote them.
- Do not edit another agent's PR comments or reviews unless the user
  explicitly asks for that exact update. Post a separate attributed
  comment for new findings or follow-up work.

## Commands

```bash
just build            # debug build of the whole workspace
just test             # full test suite
just gates            # everything PR CI runs: fmt --check, clippy -D warnings, test, no-default-features build
just golden           # env-gated golden + CI-visible FBX tests; see Golden tests

just worktree <branch>   # new worktree off fresh origin/main for a task
just worktree-prune      # remove worktrees whose merged branch is gone
```

`just gates` green locally means PR CI will be green — run it before
every push.

## Architecture

Five-crate workspace; the split enforces the dependency invariants in
`.claude/skills/audit-task/code-invariants.md` (read that file before
substantial work — the audit checks against it):

- **`animsmith-core`** — engine-agnostic data model, sampler/FK
  (`PoseGrid`), metrics, checks, transforms, config. Deps: glam, serde,
  thiserror ONLY. No file formats, no I/O.
- **`animsmith-gltf`** — glTF/GLB ingestion, the writer, byte-surgical
  `fix`.
- **`animsmith-fbx`** — ufbx ingestion (isolates the C build).
- **`animsmith-report`** — self-contained HTML report (hand-written
  viewer, no JS dependencies).
- **`animsmith`** — the CLI. Features `fbx` + `report` default on;
  `--no-default-features` must always build (CI checks it).

The design doc is `DESIGN.md` — current architecture, check catalog,
roadmap. If a substantial PR makes or changes an architectural
decision, update `DESIGN.md` in the same PR; `/audit-task` checks this.
The publishable library crates also carry crate-local READMEs for their
crates.io pages. If a PR changes a claim one of those READMEs makes —
public symbols or signatures, feature flags, loader/report boundaries,
or linked doc paths/anchors — update the README in the same PR, or
track the docs update with a `type:docs` issue or comment; `/audit-task`
checks this.

## Golden tests

Licensed assets (Mixamo, Protofactor) must NEVER be committed. The
reference golden test is env-gated:

```bash
ANIMSMITH_GOLDEN_GLB=/path/to/reference-character.glb just golden
```

The FBX mesh/skin/clip coverage uses self-authored checked-in fixtures
and runs in normal CI. When `ANIMSMITH_GOLDEN_GLB` is unset, the golden
test prints the grep-able marker `ANIMSMITH_GOLDEN_SKIP` under
`--nocapture`.

Only CC0 or procedurally generated fixtures may live in `testdata/`.

## Per-task workflow

Tasks come in two sizes — only the second triggers the full audit + PR
cycle.

**Trivial tasks** — one-line config tweak, dep bump, typo, doc fix.
These still go through a PR (branch protection allows nothing else) but
skip the audit; conventional-commit type `chore`/`docs`/`ci` keeps them
out of release notes. The merge-permission rule still applies: the
user merges, or pre-approves the batch.

**Substantial tasks** — new checks, new subcommands, changes to
measurement semantics, anything user-visible. Target each PR at under
~1000 lines of real change; split if it threatens the ceiling.

### Substantial-task lifecycle

1. **Agree intent in conversation.** For genuinely new features, walk a
   feature-discovery pass (design questions, DESIGN.md alignment)
   before writing code. For fixes/refactors the conversation itself is
   the intent agreement.
2. **Start a fresh worktree from `main`:** `just worktree <branch>`.
   One worktree per substantial task; never branch a task off another
   in-flight task's branch. This is what lets multiple agents work in
   parallel without fighting over one checkout.
3. **Implement + add behavioural tests.** Prefer analytic fixtures
   (synthetic clips with mathematically known metrics) and mutation
   tests (corrupt one field, assert exactly that finding). Tests must
   not re-implement the production code's arithmetic.
4. **Run `just gates`.** Also run relevant golden tests if measurement
   code changed.
5. **Open the PR as a draft with a written description.** The PR
   description is the intent contract: what behaviour changes, the
   chosen design for new features, and deliberately out-of-scope items.
   Every commit and the PR title must be Conventional Commits — CI
   enforces both (`.commitlintrc.yml`), and release-plz turns them
   into the release notes and the semver bump.
6. **Review + audit.** Run the strongest available code-review pass
   (Claude: `/code-review`; Codex: its review surface) and address
   findings. Optionally run a simplification pass (Claude: `/simplify`)
   for quality cleanups. Then invoke or follow the **`audit-task`**
   workflow (`.claude/skills/audit-task/SKILL.md` — it is written to be
   followable by any agent, not just Claude). Address all BLOCKers.
7. **Report the verdict and STOP — the user merges.** An agent never
   merges or arms auto-merge on its own: merging requires the audit
   verdict (APPROVE / APPROVE WITH FOLLOW-UPS) **and** the user's
   explicit permission or a standing pre-approval for that PR. Green CI
   and a passing audit are necessary, not sufficient. End the task at
   "PR open, audited, awaiting your merge decision".
   Merge-commit strategy (NOT squash): every branch commit lands on
   `main` and must stand alone as a conventional commit. The `main`
   pipeline re-tests and auto-publishes a GitHub Release when the
   commits warrant a bump.
8. **File follow-up issues** the audit drafted (the user picks which).
9. **Prune the worktree:** `just worktree-prune` after merge.

### Follow-up tracking

Deferred work goes to **GitHub issues** with `type:*` and `priority:*`
labels — not TODO files, not comments. The audit emits ready-to-paste
`gh issue create` blocks and deduplicates against existing issues.

### What counts as "substantial"

Rule of thumb — if any of these are true, it's substantial:
- Adds/changes a check, a CLI subcommand, or any measurement.
- Changes numbers an existing consumer pins (golden values, JSON
  schema, exit codes, check ids).
- Adds a dependency that ships in a published crate.
- Touches the write/fix path (output bytes).

If none apply and the diff is well under a few hundred lines, it's
trivial.
