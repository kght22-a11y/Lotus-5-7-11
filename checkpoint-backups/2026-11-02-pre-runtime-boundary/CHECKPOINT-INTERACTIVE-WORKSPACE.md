# Checkpoint: Interactive Terminal Workspace

**Date:** 2026-11-02  
**Status:** Implemented; `cargo check -p lotus-cli` passes  
**Pre-change backup:** `checkpoint-backups/2026-11-02-pre-tty-checkpoint/`

## Goal

Turn the existing `WorkspaceSession`, menu state machine, and renderer into a usable keyboard-driven terminal workspace, as identified in the previous handover's next steps.

## How to launch

```sh
cargo run -p lotus-cli -- workspace demo.lotus
```

The existing `screen` command remains a static rendering/demo command. The new `workspace` command enters raw terminal mode and uses an alternate screen.

## Controls

| Key | Behavior |
| --- | --- |
| Arrow keys | Move the selected cell |
| Enter | Begin editing the selected cell |
| Enter while editing | Commit and recalculate the sheet |
| Escape while editing | Discard the edit |
| Backspace / printable characters | Edit the current cell text |
| `/` | Open the Lotus menu |
| Left / Right in menu | Change selected menu item |
| Down / Enter in menu | Open submenu or activate selected item |
| Up / Escape in menu | Go back one menu level or close it |
| Menu shortcut letter | Select/open the matching item or execute it |
| Ctrl-S | Save workbook |
| `q` or Escape outside edit/menu | Exit |

The menu exposes save, reload, inspect summary, recalculate, run the first embedded BASIC cell, jump to the computed TOC, and quit. BASIC execution uses the existing runner's default step budget and no explicit cell confinement.

## Implementation notes

- Terminal event handling is in `crates/lotus-cli/src/main.rs`; `lotus-core` remains free of terminal dependencies.
- `crossterm` is a CLI-only dependency for key events, raw mode, and alternate-screen handling.
- The first compile check exposed an incomplete `%` operator in the existing formula engine (`BinaryOp::Mod` was absent from evaluation and parser matches). Added modulo tokenization, parsing precedence, and evaluation so the project compiles with the updated dependency graph.
- Cursor movement is bounded to columns A-Z and rows 1-100 because the current renderer draws from the origin through the cursor. TOC jump is clamped to the same visible range.
- Cell text is classified using the existing behavior: leading `=` is a formula, numeric and boolean strings become typed values, and other input becomes text. Empty input removes the cell.
- Save writes the workbook back to the supplied path. There is no autosave; use Ctrl-S or File > Save.
- The terminal is restored after the event loop returns, including ordinary I/O errors from inside the loop.

## Backup inventory

The pre-change snapshot contains `Cargo.toml`, `Cargo.lock`, all `crates/`, the handover and design documents, `briefing.md`, `demo.lotus`, and `ejected_weights.bin`. It does not include generated `target/` build artifacts. The checkpoint note itself is new and therefore is not in the pre-change snapshot.

## Follow-up

- `cargo check -p lotus-cli` passed after dependency resolution and the modulo fix. Tests were not run, and the interactive UI has not yet been exercised in a real terminal.
- Run the CLI in a real terminal and check resize behavior, key handling, and alternate-screen restoration.
- Consider a viewport renderer so navigation can cover the full spreadsheet without rendering every row from A1.
- Add a prompt/selector for choosing among multiple BASIC cells and sheets.
- Add explicit visual feedback for save/reload errors and unsaved changes.
- Check non-Unix terminal support through Crossterm on target platforms.
