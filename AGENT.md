# Agent Guide

This file keeps durable StellaRecord engineering principles for Codex and Claude.
Start from `.claude/README.md` for the full document index, do-not-rework notes, completed work notes, and internal notes.
Public user-facing documents stay in root `README.md` and `docs/`; do not move them into `.claude/`.

## Project Principles

- Treat the current source and schema as canonical while the app is unreleased. Do not add compatibility code or DB migrations for pre-release states.
- Use UTF-8 for source, comments, and docs unless a specific external tool boundary proves another encoding is required.
- Comments should explain module boundaries, public contracts, invariants, and non-obvious failure handling. Avoid restating simple assignments or JSX structure.
- `Data/archive/` contains irreplaceable compressed logs and must survive uninstall without overwrite or deletion.
- The SQLite DB, app logs, WebView cache, and other generated data may be deleted and regenerated.
- Managed archive IPC arguments are untrusted. Use `resolve_managed_archive_path` and accept only single file names matching `output_log_*.txt.tar.zst`.
- Persist VRChat raw labels such as `hidden` in the log model. UI-facing labels are consumer responsibility.
- Log viewer display may decode damaged UTF-8 lossily with U+FFFD so one broken line does not hide the rest of the log.

## Work Rules

- Check `git status --short` before edits.
- Read `.claude/README.md` first when looking for project documents, internal notes, or the right source of truth.
- Use `apply_patch` for manual file edits.
- Keep Japanese comments/docs UTF-8.
- Keep project philosophy in this file. Put memo-style notes and internal samples under `.claude/`.
- Run at least `npm run test` and `npm run test:rust` before handing off source changes; run the relevant lint/build checks when the change touches TypeScript, Rust, docs, or tooling boundaries.
