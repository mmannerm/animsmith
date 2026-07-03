# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

Read and follow [.agent-instructions/shared.md](.agent-instructions/shared.md).

## Claude-specific notes

- Keep Claude-specific settings and skills under `.claude/`.
- The local `/audit-task` skill lives at `.claude/skills/audit-task/`.
- Use the installed `/code-review` plugin for the review pass and the
  `/simplify` plugin for simplification cleanups (lifecycle step 6 in
  the shared instructions).
- End commits with `Co-Authored-By: Claude <model> <noreply@anthropic.com>`.
