---
name: audit-task
description: End-of-task gate before marking a draft PR ready. Reads the PR description as the intent contract, verifies build/tests/lint, then orchestrates review passes for bugs, security plus this codebase's invariants, simplicity, and test quality, and posts ONE summary review comment on the PR (verdict + per-lens findings + proposed follow-up titles), re-edited in place on subsequent runs. Interactive mode (default): asks the user which proposed follow-ups to file as GitHub issues, then back-fills the PR comment with their issue numbers. Non-interactive mode (`--post-issues`): drops the ready-to-paste `gh issue create` blocks into a collapsed section of the PR comment.
---

# Task audit

Run this at the end of a substantial task — implementation appears
complete, tests pass locally, the **draft PR is open against `main`
with a written description**. Goal: integrate bug review, security
review, the simplicity lens, and the test-quality lens into one verdict
that also checks the things generic review passes can't see
(PR-description-vs-diff alignment, this codebase's specific
invariants).

This workflow is written to be followable by any agent (Claude, Codex,
…) as a checklist; nothing in it requires a specific skill runner.

## Inputs you must locate

1. **The draft PR and its description.** `gh pr view <N> --json
   title,body,headRefName,baseRefName`. The PR description is the
   intent contract — it has to state, in behavioural terms, what the
   diff changes. If no PR is open, ask the user to open a draft first;
   don't audit a branch in isolation.
2. **The issue(s) the PR closes.** Parse `Closes #NNN` / `Fixes #NNN`
   from the body and fetch each: `gh issue view <NNN> --json
   title,body`. Their acceptance criteria are part of the intent
   contract and feed the claims ledger (step 2).
3. **Diff under review.** `git diff origin/main...HEAD` (or `gh pr diff <N>`).
4. **Build + test status.** Run them yourself; do not trust prior claims.

## Required workflow

### 1. Build, test, lint

Run and capture the output:

```
just gates
```

(`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo test --workspace`, and the `--no-default-features`
CLI build.) If the diff touches measurement code (`metrics.rs`,
`sample.rs`, check algorithms), also run the env-gated golden tests
(`just golden`) when the reference assets are available, and say so
either way. Any non-zero exit code is a hard fail. Report the exact
error and BLOCK.

### 2. PR-description intent adherence — the audit-specific check

**Clarity gate.** Read the PR body. If it's too vague to audit against
— e.g. "misc fixes", "cleanup", or it only describes mechanics without
stating the behaviour change — BLOCK and ask the author to rewrite. A
usable description names:
- What behaviour changes (or is added) from the user's perspective.
- For new features, the design choice picked during discovery.
- Any deliberately out-of-scope items.

**Delivery check — the claims ledger, delegated cold.** This is the
one pass most prone to author bias: on your own PR you read the tests
as doing what you intended, not what they literally do. So do **not**
run it from your own summary. Spawn a fresh subagent — one that has not
seen the implementation discussion — with the **verbatim** contents of
`intent-criteria.md` (in this skill's directory) as its prompt context.
Hand it only three raw artifacts: the raw PR body, the raw text of each
closed issue, and the diff. Do **not** brief it with what you think the
tests assert — that anchors it to the happy path you already believe
in. Ask it to build the ledger and read off the verdict.

The ledger's row shape, the concrete buggy-impl triggers, the
retained-behaviour and no-tests carve-outs, and the mechanical BLOCK
rule all live in `intent-criteria.md` — it is the single source of
truth; don't restate them here. In short: a non-empty "buggy impl still
passes" column, or a proving line of NONE on a claim the diff is meant
to deliver, is a **delivery gap** and BLOCKs.

If your environment cannot spawn subagents, run `intent-criteria.md`
against the three artifacts yourself in a separate, clean pass — discard
your mental model first and work only from the diff.

**Design-doc check.** If the diff makes or changes an architectural
decision (new crate, new check tier, changed measurement semantics, new
public contract), verify `DESIGN.md` was updated in the same PR. A
decision-level change with no doc update is a *delivery gap*. Diffs
that don't change documented architecture: state "design docs: not
applicable".

### 3. Run a bug-focused code review

Run the strongest code-review pass available in the current agent
environment against the open PR or branch diff (Claude: the
`/code-review` plugin; Codex: its review surface; otherwise a manual
bug-focused pass). Report only findings with at least 80% confidence.

Treat its findings as required reading. Don't dismiss them without
specific counter-arguments. Don't duplicate what it already covered.

### 4. Security review plus codebase invariants

First, run the strongest general security pass available (Claude: the
`/security-review` skill; otherwise a manual security-focused review
over the diff — untrusted input handling, path traversal in file
outputs, allocation bombs).

Then apply this codebase's invariants from `code-invariants.md` (in
this skill's directory). Read it verbatim; for each invariant, decide
whether the diff touches the relevant area and either flag a finding or
say "not in scope" explicitly. Do not fabricate.

### 5. Simplicity-first lens

Spawn a fresh adversarial subagent (has not seen the implementation
discussion) with the **verbatim** contents of `simplicity-criteria.md`
(in this skill's directory) as its prompt context. Give it the PR
description and the diff and ask it to apply the criteria. If your
environment cannot spawn subagents, perform this pass yourself in a
separate, clean pass — after re-reading the criteria, arguing from the
clean-slate design, not from the diff.

Categorise findings:
- **SIMPLIFY** — the diff could land with fewer concepts / fewer code
  paths / a generalisation of existing behaviour.
- **REFACTOR** — the resulting end-state is more complex than a
  clean-slate design would be; recommend a refactor either inside the
  PR or as a follow-up issue.
- **NIT** — a minor structural improvement.

The meta-rule the subagent applies: *existing structure is evidence,
not a constraint*. Findings that say "this fits the current code so
it's fine" without engaging with the clean-slate question are not
useful — push back on them.

### 6. Test-quality lens

Spawn a second fresh adversarial subagent with the **verbatim**
contents of `test-criteria.md` as its prompt context. Give it the diff
(focused on test files + the code those tests cover) and ask it to
apply the criteria.

Categorise findings:
- **IMPL-COUPLED-TEST** — a test that would fail under a
  behaviour-preserving refactor (mocks mirroring structure, asserts on
  private items, asserts on internal call ordering not part of the
  contract).
- **MISSING-BEHAVIOURAL-TEST** — a delivered behaviour from the PR
  description is only validated through implementation-level tests, not
  through the public contract.
- **NIT** — minor coupling.

Project-specific bar: prefer analytic fixtures (synthetic clips with
mathematically known metrics) and mutation tests (corrupt one field,
assert exactly that finding names it). A metric test that asserts
against a value produced by the code under test is not a test.

### 7. Produce the audit report

The audit emits **one** summary review comment on the PR. Re-runs
**edit that comment in place** rather than appending new ones; identify
it on subsequent runs by the `<!-- audit-task agent=<agent> -->` HTML
marker at the top of the body, where `<agent>` is your own lowercase
slug — `claude` or `codex`.

The marker **must carry the agent slug**, and it is the **only** key
you match on (the human attribution line at the bottom is for readers,
not for matching). Multiple agents (Claude and Codex) run this skill on
the same repo and post under the **same GitHub account**, so a bare
`<!-- audit-task -->` marker cannot tell whose comment is whose —
matching on it lets one agent overwrite another's audit. Rules:

- Match on the **exact, full** marker `<!-- audit-task agent=<agent> -->`
  for your own slug — never a loose `audit-task` / `<!-- audit-task`
  substring, which also matches other agents' comments.
- Edit a comment **only** when its marker is exactly your own slug.
  **Never** PATCH a comment carrying a different slug (or the bare
  marker) — even when your own comment is absent; in that case post a
  **new** comment.
- If your exact marker matches more than one comment, that is an error
  state: reconcile by hand, do not blind-PATCH the first hit.

Mechanism: `gh pr comment <N> --body "<body>"` for the initial post,
then `gh api repos/<owner>/<repo>/issues/comments/<comment_id> -X PATCH
-f body=...` to edit on re-runs. Find your existing comment id with an
exact-marker filter, e.g. `gh api
repos/<owner>/<repo>/issues/<N>/comments --jq '.[] | select(.body |
contains("<!-- audit-task agent=<agent> -->")) | .id'`. Include your
agent attribution line at the bottom of the comment.

#### PR comment structure

```markdown
<!-- audit-task agent=<agent> -->
## Audit: <PR title> (#<N>)

**Verdict:** [APPROVE] / [APPROVE WITH FOLLOW-UPS] / [BLOCK]

### Build / test / lint
- just gates: ✓ / ✗ <details>
- golden:     ✓ / ✗ / skipped (assets unavailable) / not applicable

### Findings

**Intent (claims ledger):** ledger verdict — "clean" or the offending
rows as `claim → proving line → buggy impl that still passes`, with
file:line refs.

**Design docs:** updated (DESIGN.md §) / not applicable / delivery gap.

**Code review:** summary or link to the code-review pass.

**Security:** general-pass summary + code-invariant findings (or "not in scope" per invariant area).

**Simplicity:** SIMPLIFY / REFACTOR / NIT bullets with file:line refs.

**Test quality:** IMPL-COUPLED-TEST / MISSING-BEHAVIOURAL-TEST / NIT bullets with file:line refs.

### Required before merge
(Only present if verdict is BLOCK.)
- <Title> — <one-line gist>. file:line.

### Follow-up issues proposed
(Only present if verdict is APPROVE WITH FOLLOW-UPS.)
- <Title> — <one-line gist>. (filed: #NNN) / (to file)
- Extends #NNN — <what's new vs the existing scope>. (commented: URL) / (to comment)
- Already tracked: #NNN — <existing title>.
```

Wrap any finding body longer than ~3 lines in
`<details><summary>...</summary>...</details>` so the comment stays
scannable.

#### Issue labels

Every proposed follow-up carries one **type** label and one
**priority** label. Types: `type:bug`, `type:feature`, `type:refactor`,
`type:chore`, `type:docs`. Priorities: `priority:high`,
`priority:medium`, `priority:low`.

Default mapping from audit-finding category to `(type, priority)`:

| Audit category                                     | type            | priority          |
| -------------------------------------------------- | --------------- | ----------------- |
| Intent / per-claim delivery gap                     | `type:bug`      | `priority:high`   |
| Code-invariant finding                              | `type:bug`      | `priority:high`   |
| Test MISSING-BEHAVIOURAL-TEST (real coverage hole)  | `type:bug`      | `priority:medium` |
| Simplicity REFACTOR (architectural change)          | `type:refactor` | `priority:medium` |
| Simplicity SIMPLIFY                                 | `type:refactor` | `priority:low`    |
| Simplicity NIT / Test IMPL-COUPLED-TEST / Test NIT  | `type:refactor` | `priority:low`    |

The labels must exist on the repo first — bootstrap once via
`gh label create`.

#### `gh issue create` block template

```bash
gh issue create \
    --title "<short title>" \
    --label "type:<type>,priority:<priority>" \
    --body "$(cat <<'EOF'
## Context
<one paragraph: where in the current PR this came up, link to the PR / file:line>

## Acceptance criteria
- <behaviour-stated criterion>

## Out of scope
- <if any>

---
Surfaced by `/audit-task` on PR #<N>.
EOF
)"
```

#### Deduplicate against existing issues

Before emitting any `gh issue create` block, search existing issues:
`gh issue list --state all --search "<terms>" --json
number,title,state,body,labels --limit 10`. Classify each proposal as
**NEW** (emit create block), **DUPLICATE of #NNN** (drop; list under
"Already tracked"), or **EXTENDS #NNN** (emit a `gh issue comment
<NNN>` block describing what's new). Be conservative on EXTENDS — a
borderline match files NEW with `Related: #NNN` in the body.

#### Interactive mode (default)

After posting the PR comment: present the ready-to-paste blocks in
chat, ask the user which to file (`all` / `1,3` / `none`, label
overrides allowed), run the selected commands, then PATCH the PR
comment to replace `(to file)` → `(filed: #NNN)`.

#### Non-interactive mode (`--post-issues`)

Skip the selection step; append all actionable blocks to the PR comment
inside one collapsed `<details>` block. No auto-filing — a human copies
the commands later.

#### Gates

- **BLOCK**: do **not** mark the PR ready, do not merge. Re-run after
  each fix; each re-run edits the same PR comment.
- **APPROVE WITH FOLLOW-UPS**: PR can be marked ready and merged;
  follow-up filing doesn't gate the merge.
- **APPROVE**: PR can be marked ready and merged.

## What this skill is NOT

- **Not** a re-implementation of a code-review or security-review
  workflow — delegate to the strongest available surface; only the
  codebase-specific invariants live here.
- **Not** the design phase. Architecture exploration runs before
  implementation; its decisions belong in the PR description and
  DESIGN.md.
- **Not** a multi-comment fragmenter. One summary review comment per
  PR, edited in place on re-runs.
- **Not** an auto-filer of issues, and **not** a duplicate-filer.
- **Not** the only place codebase rules live — `.agent-instructions/
  shared.md` does that. This skill's unique value is the
  PR-description-vs-diff adherence check, the codebase invariants, the
  simplicity and test-quality lenses, and the integrated verdict as one
  durable PR comment.
