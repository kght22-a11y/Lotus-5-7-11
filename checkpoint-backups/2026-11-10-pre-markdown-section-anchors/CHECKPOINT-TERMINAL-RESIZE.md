# Checkpoint: Terminal-Responsive Viewport

**Date:** 2026-11-04  
**Status:** Implemented; all 23 workspace tests pass  
**Pre-change backup:** `checkpoint-backups/2026-11-04-pre-terminal-resize/`

## Goal

Make the interactive workspace use the available terminal space and redraw after a window resize.

## Behavior

- The CLI reads the current terminal size before every screen draw.
- A Crossterm resize event returns to the draw loop, which reads the updated dimensions.
- Visible columns are sized to the available width using fixed 10-character cells, with long values shortened in-grid. The selected cell's full value remains on the status line.
- Visible rows use the terminal height after reserving six lines for status, menu/edit controls, grid headings, and the key hint.
- If the terminal size cannot be read, the loop falls back to 80 by 24.
- `WorkspaceSession::render_workspace_with_viewport` accepts the computed grid dimensions; the core remains independent of terminal APIs.

## Verification

- Added a CLI unit test for normal, wide, and very small terminal dimensions.
- `cargo test --workspace` passes: 1 CLI test, 20 core tests, and 2 edge tests; 0 failures.
- Real terminal resize behavior still needs hands-on confirmation.
