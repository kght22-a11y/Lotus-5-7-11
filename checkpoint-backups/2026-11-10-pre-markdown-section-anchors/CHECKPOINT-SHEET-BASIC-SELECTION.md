# Checkpoint: Sheet and BASIC Selection

**Date:** 2026-11-04  
**Status:** Implemented; all 26 workspace tests pass  
**Pre-change backup:** `checkpoint-backups/2026-11-04-pre-sheet-script-selection/`

## Workspace controls

- `Tab` switches to the next worksheet; `Shift-Tab` switches to the previous one. Selection wraps at either end.
- The active sheet name appears in the top status line. The cursor coordinate is preserved when changing sheets.
- `/` then `B` then `R` opens BASIC selection for the active sheet.
- Left and Right choose among BASIC cells on that sheet. Enter runs the selected procedure; Escape cancels.
- Selecting a procedure moves the grid cursor to its cell. A cell's name labels the choice; unnamed procedures use their coordinate.
- The selected procedure runs with the runtime's default owner-cell access and step budget. Cross-cell access still needs explicit grants; this selector does not grant any.

## Implementation

The sheet index and BASIC choices are kept by the CLI workspace loop. `WorkspaceSession` remains a small renderer and cursor state machine, and the core library has no terminal dependency.

## Verification

- Added CLI coverage for sheet-switch wraparound and collecting BASIC procedures from the selected sheet.
- `cargo test --workspace` passes: 4 CLI tests, 20 core tests, and 2 edge tests; 0 failures.
- The keyboard flow still needs hands-on use in a real terminal.
