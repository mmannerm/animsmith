# AGENTS.md

Instructions for Codex and other non-Claude coding agents.

Read and follow [CONTRIBUTING.md](CONTRIBUTING.md),
[DEVELOPMENT.md](DEVELOPMENT.md), and
[.agent-instructions/shared.md](.agent-instructions/shared.md).

## Codex-specific notes

- The audit workflow at `.claude/skills/audit-task/SKILL.md` is written
  to be followable by any agent: run it as a checklist even without
  Claude's skill runner.
- For the audit code-review pass, run Codex review when available;
  otherwise make a manual pass over `gh pr diff`.
- End commits with `Co-authored-by: Codex <codex@openai.com>`.
