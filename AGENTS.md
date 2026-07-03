# AGENTS.md

Instructions for Codex and other non-Claude coding agents.

Read and follow [.agent-instructions/shared.md](.agent-instructions/shared.md).

## Codex-specific notes

- The audit workflow at `.claude/skills/audit-task/SKILL.md` is written
  to be followable by any agent: run it as a checklist even without
  Claude's skill runner. The criteria files it references
  (`simplicity-criteria.md`, `test-criteria.md`,
  `code-invariants.md`) live in the same directory.
- For the code-review pass in lifecycle step 6, use the strongest
  review surface available in your environment (Codex review, or a
  manual pass over `gh pr diff`).
- End commits with `Co-authored-by: Codex <codex@openai.com>`.
