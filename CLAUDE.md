# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

Read and follow [CONTRIBUTING.md](CONTRIBUTING.md),
[DEVELOPMENT.md](DEVELOPMENT.md), and
[.agent-instructions/shared.md](.agent-instructions/shared.md).

## Claude-specific notes

- Keep Claude-specific settings and skills under `.claude/`.
- The local `/audit-task` skill lives at `.claude/skills/audit-task/`.
- Use the installed `/code-review` plugin for the author-side audit review pass
  and the `/simplify` plugin for simplification cleanups. These do not replace
  the required Codex CLI cross-model audit.
- End commits with `Co-Authored-By: Claude <model> <noreply@anthropic.com>`.
