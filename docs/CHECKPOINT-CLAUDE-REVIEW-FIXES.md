# Checkpoint: Claude review fixes — 2026-11-04

Applied the review’s five **Fix now** items. The pre-change snapshot is
`checkpoint-backups/2026-11-04-pre-claude-fix-pass/`.

- Formula results now retain boolean type (`cached_bool`); renderers and file
  serialization handle it, including save/load followed by formula evaluation.
- Named references now contribute dependency edges, so ordering and named
  cycles work. Added a test documenting that `q3` parses as a cell coordinate.
- BASIC procedure cells can store explicit `reads` and `writes` grants. The
  reader, writer, CLI, and workspace runner use those grants; CLI flags add to
  them. BASIC writes to formula cells now fail transactionally. The demo writes
  its script result to a separate plain cell.
- Edge round-trip checks now assert. Coverage includes numeric-looking and
  whitespace cached text, embedded closing tags, and NaN/infinity bit patterns.
- Workspace changes mark the sheet name with `*`. Dirty quit asks for `q`
  again; dirty File > Retrieve asks for `y`. Save and reload clear the marker.

**Verification:** `cargo test --workspace` passed: 33 tests across CLI, core,
and edge suites, with 0 failures. `cargo run -p lotus-cli -- demo` regenerated
`demo.lotus`, and `cargo run -p lotus-cli -- run-basic demo.lotus sim.bas`
completed one write. The interactive workspace still has not been exercised
manually in a real terminal. `cargo fmt --all` could not run because rustfmt is
not installed in the active toolchain.

The review’s **Record for the rewrite** items remain deferred as requested.
