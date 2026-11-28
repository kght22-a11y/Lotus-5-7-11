# Checkpoint: Terminal grid alignment — 2026-11-05

Fixed the diagonal cell boundaries shown in the interactive workspace. Crossterm
raw mode disables the terminal's normal newline translation; the workspace was
writing bare LF characters, so each line moved down without returning to column
zero. Interactive screen output now converts line endings to CRLF, including the
key hint lines. The core grid renderer remains unchanged.

Pre-change sources and docs are backed up in
`checkpoint-backups/2026-11-05-pre-terminal-alignment-fix/`.

**Verification:** added a CLI regression test for CRLF conversion;
`cargo test --workspace` passes 34 tests. Rebuilt `target/debug/lotus-cli` and
opened the workspace in a PTY; emitted rows now begin with CRLF boundaries and
the process restores the terminal on exit.
