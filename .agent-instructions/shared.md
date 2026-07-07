# Shared Agent Instructions

These instructions apply to coding agents working in this repository.
They intentionally carry only agent-specific deltas. Shared contributor
process lives in [CONTRIBUTING.md](../CONTRIBUTING.md), local setup and
verification live in [DEVELOPMENT.md](../DEVELOPMENT.md), and
architecture lives in [DESIGN.md](../DESIGN.md).

## Baseline

- Follow [CONTRIBUTING.md](../CONTRIBUTING.md) for PR lifecycle,
  Conventional Commits, documentation freshness, audit expectations,
  labels, follow-up issues, and merge policy.
- Follow [DEVELOPMENT.md](../DEVELOPMENT.md) for toolchain setup,
  `just gates`, docs, golden tests, package checks, and local commands.
- Follow [DESIGN.md](../DESIGN.md) for crate boundaries and
  architecture.

## Agent Attribution

Agent-authored GitHub work must be attributable so reviewers can tell
which agent did what.

- Agent-authored commits must include an agent `Co-authored-by` trailer,
  such as `Co-authored-by: Codex <codex@openai.com>` or
  `Co-authored-by: Claude <claude@anthropic.com>`.
- Agent-authored PR descriptions, PR comments, and review comments must
  include a clear attribution line naming the agent that wrote them.
- Do not edit another agent's PR comments or reviews unless the user
  explicitly asks for that exact update. Post a separate attributed
  comment for new findings or follow-up work.

## Agent Worktrees

For substantial work, use one branch and one worktree per task. Start
from freshly fetched `origin/main`, and do not branch a new task from
another in-flight task's branch. The repo helper is:

```console
$ just worktree codex/descriptive-branch
```

If you are already in a task-specific clean worktree, make sure the
branch starts from current `origin/main` before editing. Never discard or
overwrite user changes to move worktrees around.

## Audit Checklist

For substantial changes, follow `.claude/skills/audit-task/SKILL.md` as
a checklist even outside Claude. The criteria files it references live
next to it:

- `.claude/skills/audit-task/simplicity-criteria.md`
- `.claude/skills/audit-task/test-criteria.md`
- `.claude/skills/audit-task/code-invariants.md`

For the code-review pass, use the strongest review surface available in
your environment. Address all blockers before reporting a PR as ready
for a maintainer merge decision.
