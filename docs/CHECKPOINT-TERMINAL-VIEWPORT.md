# Checkpoint: Fixed Terminal Viewport

**Date:** 2026-11-03  
**Status:** Implemented; all 22 workspace tests pass  
**Pre-change backup:** `checkpoint-backups/2026-11-03-pre-viewport/`

## Goal

Let the interactive terminal workspace move through the full worksheet without rendering every row and column between A1 and the cursor.

## Changes

- Added `GridRenderer::render_viewport`, which renders a fixed window around the selected cell and marks the active row and cell.
- Wired the terminal workspace renderer to a fixed-size viewport; terminal-aware sizing is covered by `CHECKPOINT-TERMINAL-RESIZE.md`.
- Cursor navigation now covers spreadsheet coordinates through XFD1048576 instead of stopping at A-Z / rows 1-100.
- TOC jump positions are no longer clamped to the visible region; the viewport follows the selected TOC position.
- The existing full-sheet `render_sheet` path remains available for static CLI views.

## Verification

- Added a focused renderer test for a remote cursor, active-cell marker, and fixed output height.
- `cargo test --workspace` passes: 20 core tests and 2 edge tests, 0 failures.
- The keyboard UI has not been exercised in a real terminal in this checkpoint.
- The terminal viewport now uses fixed-width cells to avoid overflowing when a visible value is long.

## Remaining UI work

- Terminal resizing is implemented in `CHECKPOINT-TERMINAL-RESIZE.md`; hands-on verification remains.
- Add sheet and BASIC procedure selection.
- Exercise key handling and alternate-screen restoration in a real terminal.
