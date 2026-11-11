# Lotus 5-7-11 Progress Report

**As of:** 2026-11-10  
**Status:** Functional Rust prototype; core workflows are taking shape, while the eventual vector-rendered interface and bare-metal target remain future work.

## Project direction

Lotus is a self-contained workbook that holds values, formulas, small sandboxed BASIC programs, documents, and its own update history. The workbook file is also the AI exchange surface: updates arrive as Z-layer batches, and the viewer reads those updates from the file. The runtime is intended to stay inside the workbook boundary.

The current terminal UI is a prototype of the interaction model. The intended interface is terminal-like, but should not inherit an 80-column character grid. A compact vector renderer and bundled fonts should allow a scalable, pannable view of the workbook. Logo-style turtle drawing in BASIC is a possible future use for that renderer.

## Working prototype

- **Workbook file:** readable tagged format with sheets, named cells, embedded documents, formulas, BASIC source, and exact raw blob payloads.
- **Calculations:** formula parsing, dependency ordering, recalculation, cycle detection, and typed cached results.
- **BASIC:** embedded bytecode interpreter with instruction limits, per-cell read/write grants, and staged writes.
- **History:** per-sheet Z layers and tombstones; the base sheet stays intact. The CLI can browse and compare historical views.
- **AI file exchange:** the viewer checks the workbook every 30 seconds, then every 10 seconds for one minute after a user save. Complete appended batches can contain many Z layers. Invalid or incompatible external file changes are held back to protect the workbook.
- **Documents:** embedded Markdown renders live cell references including sheet-qualified ones. Cells point back to documents. `links` inspects both directions. `doc id#fragment` renders a named section. `orphans` audits the entire workbook for broken references, dangling formulas, and unreferenced documents.
- **Interactive CLI:** Lotus-style grid, edit preview, slash menu (`/WO` orphan check), sheet switching, saves, and the current Lotus-inspired palette. Cell info panel shows document links.

## Useful commands

```sh
cargo test --workspace
cargo build -p lotus-cli
cargo run -p lotus-cli -- workspace demo.lotus
cargo run -p lotus-cli -- history demo.lotus
cargo run -p lotus-cli -- compare-z demo.lotus main 0 1
cargo run -p lotus-cli -- links demo.lotus main A1
cargo run -p lotus-cli -- doc demo.lotus briefing
cargo run -p lotus-cli -- doc demo.lotus "briefing#lotus-5-7-11-executive-briefing"
cargo run -p lotus-cli -- orphans demo.lotus
```

The latest recorded workspace test run passed **51 tests**: 14 CLI, 35 core, and 2 edge tests. The CLI build passed. The demo workbook was used for read-only command checks; `lotus orphans demo.lotus` reports all clear.

## Still in front of us

1. Define and then consume the final-line AI focus cursor with sheet and X/Y/Z coordinates. The current reader tolerates the tag, but the UI ignores it.
2. Interactive cell-to-document navigation: pressing Enter on a cell with `document_refs` should open a TUI viewer pane. The CLI-side `lotus doc` path works; the interactive path is not yet implemented.
3. Improve file-backed context retrieval while keeping the active working set and cell/runtime limits bounded.
4. Replace the terminal-character renderer with the vector-rendered, terminal-like UI; bundle fonts and consider turtle graphics in BASIC.
5. Explore the BareMetal OS and ggml unikernel handoff after the workbook workflows are useful and stable.

This remains a prototype rather than a finished end-user application. The immediate goal is to make the important workflows function end to end; a later rewrite can tighten the architecture once those behaviors are better understood.

## Checkpoints and backup

See `HANDOVER.md` for the source layout and detailed checkpoint history. The latest work includes `CHECKPOINT-MARKDOWN-SECTION-ANCHORS.md`, `CHECKPOINT-BIDIRECTIONAL-DOCUMENT-LINKS.md`, and `CHECKPOINT-Z-LAYER-COMPARE.md`.

The handover and checkpoint notes were copied before this session's changes to `checkpoint-backups/2026-11-10-pre-markdown-section-anchors/`.
