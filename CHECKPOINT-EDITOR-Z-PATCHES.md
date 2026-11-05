# Checkpoint: Record Editor Changes as Z Layers

**Date:** 2026-11-04  
**Status:** Implemented; all 26 workspace tests pass  
**Pre-change backup:** `checkpoint-backups/2026-11-04-pre-sheet-script-selection/`

## Behavior

- The interactive workspace displays the latest composed Z state for every worksheet and recalculates that view for display.
- Committing a cell edit compares the edited view with its prior state and appends only the changed cell as the next Z layer. Deleting a cell appends a tombstone.
- BASIC procedures launched from the workspace run against the composed active sheet. Successful cell changes are collected into one new layer.
- Formula cached values are recalculated in the composed view but do not create extra history records when the formula itself did not change.
- The base sheet remains Z=0 and is not modified by workspace edits or BASIC writes.
- Changes remain in memory until Save or File > Save. The status line reports the assigned Z value and reminds the user to save.
- The workspace opens at the latest Z. Earlier states can be inspected with the existing `history` and `view-z` commands; selecting an earlier Z in the interactive workspace is not implemented yet.

Each cell edit creates one layer; a BASIC procedure's successful cell writes are grouped into one layer. The history records contain the cell values, not an operation log.

## Verification

- `cargo test --workspace` passes: 4 CLI tests, 20 core tests, and 2 edge tests; 0 failures.
- Added coverage for producing a patch from edits and deletions while ignoring formula-cache-only changes.
- Hands-on terminal testing is still needed for editing, running scripts, and saving the resulting workbook.
