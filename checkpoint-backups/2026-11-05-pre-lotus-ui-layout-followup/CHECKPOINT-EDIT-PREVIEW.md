# Checkpoint: Keep edit preview visible — 2026-11-05

The user’s Ubuntu Terminal screenshot showed the grid, but not the sheet title
or status/edit line above it. The viewport reserved too few terminal rows, so
the rendered frame scrolled its first lines offscreen. Reduced the grid height
reservation by two rows and moved the live input preview to the first line:
`<sheet> | EDIT <cell>: <buffer>`. The next line shows the commit/cancel keys.

The pre-change snapshot is
`checkpoint-backups/2026-11-05-pre-terminal-alignment-fix/`.

**Verification:** added a regression test for the first-line edit preview and
updated terminal-size expectations. `cargo test --workspace` passes 35 tests.
Rebuilt the CLI and launched it in a 46-row PTY: the sheet title and status
line remain in frame, with 38 grid rows and the key hints visible. Pressing
Enter displays `Main Model | EDIT A1: 9000` before commit.
