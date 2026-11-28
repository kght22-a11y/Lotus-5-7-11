# Checkpoint: Lotus-style edit control panel — 2026-11-05

The user’s Ubuntu Terminal screenshot showed the grid, but not the sheet title
or status/edit line above it. The viewport reserved too few terminal rows, so
the rendered frame scrolled its first lines offscreen. Reduced the grid height
reservation by two rows. The top control panel now follows the classic Lotus
layout: line one shows sheet/cell details; line two shows `EDIT: <buffer>` and
the commit/cancel keys. Lotus 1-2-3 references describe a three-line control
panel and placing the entry on its second line in EDIT mode ([screen guide](https://www.palmtoppaper.com/ptphtml/23/pt23003e.htm), [F2 editing guide](https://www.palmtoppaper.com/ptphtml/26/pt260034.htm)).

The pre-change snapshot is
`checkpoint-backups/2026-11-05-pre-terminal-alignment-fix/`.

**Verification:** added a regression test for the second-line edit preview and
updated terminal-size expectations. `cargo test --workspace` passes 35 tests.
Rebuilt the CLI and launched it in a 46-row PTY: the sheet/cell details remain
on line one, the edit buffer is on line two, and 38 grid rows and key hints
remain in frame.
