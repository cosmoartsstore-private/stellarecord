# Agent Entry

This is the Codex entry point for StellaRecord.
Common ethics, engineering philosophy, comment policy, and workflow rules live in the Codex global rules.

## Next Documents

- `.claude/README.md` - project-local index, StellaRecord constraints, do-not-rework notes, follow-up notes, and completed work notes.
- `README.md` - public project overview, feature list, stack summary, build commands, install/uninstall behavior, and repository layout.
- `docs/spec.md` - feature behavior, IPC reference, data flow, state management, concurrency, and security model.
- `docs/database.md` - SQLite schema, tables, views, indexes, PRAGMAs, and backup/restore notes.
- `docs/tech-stack.md` - technology choices, ADRs, rejected alternatives, and build/distribution rationale.

## Routing

- Need project-local rules or prior agent decisions: read `.claude/README.md`.
- Need GitHub-visible or user-facing explanation: read `README.md` or `docs/`.
- Need an archived working sample or audit handoff: keep it under `.claude/`; do not move it into public docs.
