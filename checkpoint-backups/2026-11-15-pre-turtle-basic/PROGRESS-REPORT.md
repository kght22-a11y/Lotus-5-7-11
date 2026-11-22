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

The latest recorded workspace test run passed **59 tests**: 14 CLI, 40 core, 2 edge, and 3 vector raster integration tests. The CLI build passed.

## Vector Engine, Hershey Fonts & Subpixel Rasterizer (Phase 1 Implemented)

- **Vector IR & Command Stream (`lotus_core::model::vector`)**: `Point2D`, `Rect2D`, `ColorRgba`, `StrokeStyle`, and `VectorBuffer` with compact binary serialization (`LVEC\x01`) suitable for storage in `CellValue::Blob` at the binary end of `.lotus`.
- **Hershey Stroke Font Engine (`lotus_core::view::font::hershey`)**: Clean `#![no_std]` decoder for Dr. A. V. Hershey's 1967 Roman Simplex font (ASCII 32..=126) from embedded JHF data. Scales to any DPI or point size without font libraries.
- **ClearType-Style Subpixel Software Rasterizer (`lotus_core::view::raster`)**: Pure Rust rasterizer rendering directly to linear 32-bit pixel buffers (`&mut [u32]`) using 3x horizontal subpixel bins, low-pass FIR energy filtering (`[1/4, 1/2, 1/4]`), anti-aliasing, and affine pan/zoom transforms. Includes standalone uncompressed BMP exporter (`to_bmp_bytes`) for visual snapshot testing.
- **Dynamic TOC Extent Isolation**: Far-out vector asset rows (e.g., row 10,000+) carry `is_asset: true` and are excluded from `sheet.active_grid_bounds()`, preventing vector and media blobs from displacing the dynamic Table of Contents anchored at `(maxX + 1, maxY + 1)` of the active spreadsheet.

## Still in front of us

1. Add Logo-style turtle graphics statements (`FD`, `BK`, `RT`, `LT`, `PU`, `PD`, `COLOR`, `WIDTH`, `LABEL`) to embedded BASIC, generating vector streams.
2. Cell curtains/overlays: allow visible cells to project vector overlays across spans while hit-testing passes through to underlying formulas.
3. Define and consume the final-line AI focus cursor with sheet and X/Y/Z coordinates.
4. Interactive cell-to-document navigation in the workspace.
5. Desktop UI harness (`lotus-ui` with `winit` + `softbuffer` blitting the rasterizer output) and BareMetal OS LFB handoff.

This remains a prototype rather than a finished end-user application. The immediate goal is to make the important workflows function end to end; a later rewrite can tighten the architecture once those behaviors are better understood.

## Checkpoints and backup

See `HANDOVER.md` for the source layout and detailed checkpoint history. The latest work includes `CHECKPOINT-MARKDOWN-SECTION-ANCHORS.md`, `CHECKPOINT-BIDIRECTIONAL-DOCUMENT-LINKS.md`, and `CHECKPOINT-Z-LAYER-COMPARE.md`.

The handover and checkpoint notes were copied before this session's changes to `checkpoint-backups/2026-11-10-pre-markdown-section-anchors/`.
