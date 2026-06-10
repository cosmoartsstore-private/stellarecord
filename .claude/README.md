# Claude/Codex Document Index

This is the first file agents should read when choosing which StellaRecord document to use.
Public user-facing documents remain in root `README.md` and `docs/`; `.claude/` only contains the agent index, internal notes, and archived working references.

## Public Documents

| Document | Audience | Use When |
| --- | --- | --- |
| `../README.md` | GitHub users, maintainers | Project overview, feature list, stack summary, build commands, install/uninstall behavior, repository layout |
| `../docs/spec.md` | Users who need detailed behavior, developers | Feature behavior, IPC reference, data flow, state management, concurrency, security model |
| `../docs/database.md` | Users/developers inspecting data | SQLite schema, tables, views, indexes, PRAGMAs, backup/restore notes |
| `../docs/tech-stack.md` | Users/developers reviewing decisions | Technology choices, ADRs, rejected alternatives, build/distribution rationale |

## Agent-Only Documents

| Document | Audience | Use When |
| --- | --- | --- |
| `../AGENT.md` | Codex, Claude | Durable project principles and work rules |
| `.claude/README.md` | Codex, Claude | Document map, internal notes, follow-up list, do-not-rework notes, completed work notes |
| `.claude/manual.html` | Codex, Claude | Archived hard-coded interaction sample; not app source and not public documentation |

## Which Document To Read

- Need project orientation or GitHub-visible explanation: read `../README.md`.
- Need exact feature behavior or IPC shape: read `../docs/spec.md`.
- Need schema details or DB storage behavior: read `../docs/database.md`.
- Need why a technology or architecture choice was made: read `../docs/tech-stack.md`.
- Need project philosophy, data-preservation rules, or coding rules: read `../AGENT.md`.
- Need prior audit notes, follow-up items, do-not-rework decisions, or completed work notes: continue in this file.

## Do Not Rework Without Requirement Change

- Do not protect the SQLite DB on uninstall. The DB and generated app data can be deleted and regenerated; only `Data/archive/` must survive without overwrite or deletion.
- Do not re-add pre-release DB migrations or old-schema compatibility code. The current DDL in `src-tauri/src/analyze/db.rs` is the canonical schema until a real released version needs migration support.
- Do not treat VRChat raw labels such as `hidden` as a persistence bug. UI-facing labels should be mapped by consumers, not rewritten in the stored log model.
- Do not add text-only keyword highlighting fallback to the log viewer. Category coloring can use text classification, but keyword highlights intentionally require DB-confirmed markers.
- Do not move public documentation from `README.md` or `docs/` into `.claude/`. Those files are GitHub/user-facing project documentation.
- Do not wire `.claude/manual.html` back into the app unless explicitly requested. It is an archived hard-coded interaction sample.

## Recently Completed Work

- Old `apps` schema migration code and its migration tests were removed.
- Public docs were updated so uninstall behavior says DB deletion is normal and `Data/archive/` is the only protected data directory.
- Root `manual.html` was moved to `.claude/manual.html` so build and format checks no longer process the archived sample as app source.
- Root-level memo ignore entries such as `fix-task.md` and `task-fix.md` were removed; memo-style material should live under `.claude/`.
- SQL identifier validation and byte-size formatting were commonized.
- Launcher app registration now opens an initialized main DB so the `apps` table exists before first import.

## Follow-Up Work

- The import parser still skips invalid UTF-8 lines. If DB import must preserve events from damaged lines, replace `BufRead::lines()` with byte-line decoding similar to the log viewer and audit parser assumptions.
- Code signing and automatic updates remain outside the current implementation scope.
- Narrow/snapped-window behavior remains a separate UI QA pass; do not treat the current desktop-oriented layout as fully responsive without explicit testing.

## Completed Checks

- Source and documentation files were checked as UTF-8 readable in the managed project files.
- No repo-local technical reason was found to read or rewrite Japanese comments as Shift_JIS/CP932.
- No mojibake markers such as U+FFFD replacement characters or common UTF-8/Shift_JIS mix-up fragments were found in source comments or docs during the 2026-06-06 check.

## Older Rechecked Non-Issues

- `visits.instance_type` and `notifications.target_instance_type` intentionally store VRChat raw labels such as `hidden`.
- Log viewer category coloring works without DB hints via text classification.
- Keyword highlights intentionally require DB-confirmed markers.
