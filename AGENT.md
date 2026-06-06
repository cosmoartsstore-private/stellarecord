# Agent Notes

These notes prevent future sessions from redoing already-audited StellaRecord work.

## Completed Decisions

- Source and documentation files are UTF-8 readable. A byte-level UTF-8 validation pass over source/docs/config files found no invalid UTF-8 in the managed project files.
- There is no repo-local technical reason to read or rewrite Japanese comments as Shift_JIS/CP932. Use UTF-8 for Japanese comments and docs unless a specific external tool boundary proves otherwise.
- Comment style follows the neighboring Alpheratz project: explain module boundaries, public contracts, invariants, and non-obvious failure handling; avoid comments that merely restate simple assignments or JSX structure.
- Managed archive IPC arguments must be treated as untrusted strings. Use `resolve_managed_archive_path` and accept only single file names matching `output_log_*.txt.tar.zst`.
- Log viewer display is allowed to decode damaged UTF-8 lossily with U+FFFD so one broken line does not hide the rest of the log.

## Known Follow-Up Work

- The import parser still skips invalid UTF-8 lines. If DB import must preserve events from damaged lines, replace `BufRead::lines()` with byte-line decoding similar to the log viewer and audit parser assumptions.
- `apps` migration uses `INSERT OR IGNORE`; old databases with duplicate `path` rows can drop duplicates without an explicit count log. Add migration telemetry before broad legacy migration work.
- Code signing and automatic updates remain outside the current implementation scope.
- Narrow/snapped-window behavior remains a separate UI QA pass; do not treat the current desktop-oriented layout as fully responsive without explicit testing.

## Rechecked Non-Issues

- `visits.instance_type` and `notifications.target_instance_type` intentionally store VRChat raw labels such as `hidden`. UI-facing labels should be mapped by consumers, not rewritten in the persisted log model.
- Log viewer category coloring works without DB hints via text classification. Keyword highlights intentionally require DB-confirmed markers.
- No mojibake markers (U+FFFD replacement characters or common UTF-8/Shift_JIS mix-up fragments) were found in source comments or docs during the 2026-06-06 check.

## Work Rules

- Check `git status --short` before edits.
- Use `apply_patch` for manual file edits.
- Keep Japanese comments/docs UTF-8.
- Run at least `npm run test` and `npm run test:rust` before handing off source changes; run `npm run verify` when the change touches lint-sensitive TypeScript/Rust boundaries.
