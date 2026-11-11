# Checkpoint: Real terminal validation — 2026-11-05

Launched `lotus workspace` through a PTY using a disposable copy of
`demo.lotus`. The untouched project demo remains unchanged. The complete
pre-checkpoint source and demo snapshot is in
`checkpoint-backups/2026-11-05-pre-tty-validation/`; the interactive test copy
and its saved Z layers are also there.

## Confirmed

- Workspace starts in alternate-screen/raw mode and renders the grid.
- Enter, Backspace, printable input, and Enter commit an edit as a Z layer;
  formulas recalculate and the title displays `*` while dirty.
- Dirty `q` displays a second-press prompt. A different key stays in the
  workspace. Ctrl-S saves and clears `*`.
- File > Retrieve prompts before discarding dirty edits. Pressing `y` reloads
  the last saved workbook and clears the dirty state.
- Menu > Basic > Run selected `sim.bas`, used its declared grants, wrote the
  plain output cell, and recorded the result as a Z layer.
- A normal `q` exits cleanly; the terminal output includes cursor-show and
  alternate-screen-restore sequences. The saved test workbook reports Z=1
  (A1=9000) and Z=2 (B2=1800).

## Not exercised

Terminal resizing, other terminal emulators/platforms, and error-path
restoration were not tested. This was a manual interaction check, not a code
change; the prior `cargo test --workspace` result remains 33 passing tests.
