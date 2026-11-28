# Checkpoint: Interactive Z-layer browsing

**Date:** 2026-11-05

## What changed

The interactive workspace can now inspect a sheet’s stored history without leaving the grid:

- Press `[` to move one layer toward the original sheet (Z=0). From the live view, this opens the previous saved layer.
- Press `]` to move one layer toward the latest state. Advancing past the last historical layer returns to the live view.
- Press `0` to return directly to the live view.
- The active sheet title shows the selected Z value, and the status line marks historical views as read-only.
- `Enter` and `/` are blocked while viewing history. Sheet switching returns to that sheet’s live view.

The displayed historical sheet is composed from the workbook’s base cells and stored overlays. Edits remain recorded as new layers through the existing flow; viewing history does not rewrite the base sheet or saved history.

## Backup

The full pre-change source, demo workbook, and handover snapshot is in `checkpoint-backups/2026-11-05-pre-interactive-z-browse/`.

## Verification

- `cargo test --workspace` — passed: 7 CLI, 27 core, and 2 edge tests (36 total).
- `cargo build -p lotus-cli` — passed.
- Interactive PTY check: edited a temporary copy of `demo.lotus` to create Z=1, used `[` to view Z=0, confirmed the old value, used `]` to restore Z=1 and the updated value, then quit with the terminal restored.
- `cargo fmt --check` could not run because `rustfmt` (`cargo-fmt`) is not installed for this toolchain.

## Next

Add a compact comparison between two selected Z-layer views.
