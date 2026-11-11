# Checkpoint: Workbook file polling and update batches

**Date:** 2026-11-05

## What changed

The interactive workspace checks the workbook file every 30 seconds. After the user saves through Ctrl-S or the slash menu, it checks every 10 seconds for one minute, then returns to the 30-second interval. Crossterm event polling lets those checks run while the UI waits for input.

The CLI hashes the file contents on each check and reparses only when the hash changes. It accepts append-only Z-layer updates while preserving the base sheet. Multiple branch batches are rebased after local layers so a local and incoming update are both retained. The active live view refreshes automatically; a user browsing older history stays at that selected historical layer.

Malformed or incomplete batches are held back. Changes to workbook identity, base sheets, or embedded documents are not imported automatically. Ctrl-S is blocked while an unaccepted file change is present, protecting the external edit; use File/Retrieve after resolving the file to load it explicitly.

## Append format

Append a complete `<history>` trailer after the closing `</workbook>` tag. A batch can contain any number of consecutive `<layer>` entries, including a dozen or more. The custom Lotus reader scans tags as a stream, so the appended trailer is accepted even though this is not a single-root XML document. See `CHECKPOINT-FILE-Z-LAYERS.md` for an example.

The future final line is reserved for an AI attention cursor with sheet and X, Y, Z coordinates. It may later guide the initial UI focus and provide a starting point for links. The current viewer ignores this line, as requested; no cursor behavior is implemented yet.

## Backup

The full pre-change source, demo workbook, and handover snapshot is in `checkpoint-backups/2026-11-05-pre-file-update-polling/`.

## Verification

- `cargo test --workspace` — passed: 10 CLI, 29 core, and 2 edge tests (41 total).
- `cargo build -p lotus-cli` — passed after the polling implementation.
- Tests cover a 12-layer trailer, rejection of incomplete batches and base rewrites, importing a file update, preserving/rebasing local layers, and both polling intervals.

