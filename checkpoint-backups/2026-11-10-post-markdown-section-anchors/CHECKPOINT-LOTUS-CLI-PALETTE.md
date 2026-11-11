# Checkpoint: Lotus CLI palette

**Date:** 2026-11-05

## What changed

The terminal UI now uses a light gray worksheet background with dark text, blue column headings, and a blue/white highlight for the active cell. The first control-panel line uses blue when showing `EDIT` or `MENU`; the grid divider is dark gray. Color is applied only in the CLI rendering layer and does not alter workbook cell values or serialization.

The existing `NO_COLOR` behavior is preserved. In the development PTY, `TERM=dumb` and `NO_COLOR=1` suppressed ANSI colors. The palette was visually checked by rerunning with `TERM=xterm-256color` and `NO_COLOR` unset. If colors do not appear in a local terminal, try:

```sh
env -u NO_COLOR TERM=xterm-256color target/debug/lotus-cli workspace demo.lotus
```

## Backup

The pre-change snapshot is in `checkpoint-backups/2026-11-05-pre-cli-color-pass/`.

## Verification

- `cargo test --workspace` — passed (35 tests across the workspace).
- `cargo build -p lotus-cli` — passed.
- Interactive terminal output was checked with ANSI color enabled; quitting restored the cursor and terminal screen.

## Follow-up direction

Keep cell style metadata separate from values. A later rendering checkpoint can add an embedded vector-font renderer and a compact bundled font set, then take advantage of higher-resolution terminals where available. The recovered Lotus UNIX release described in [Tavis Ormandy’s write-up](https://lock.cmpxchg8b.com/linux123.html) is useful historical reference material for its font and terminal-driver assets; this project’s implementation remains its own.

