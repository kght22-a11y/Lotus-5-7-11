# Lotus 5-7-11 Progress Report

**As of:** 2026-11-22  
**Status:** Functional Rust prototype; core workflows, vector engine, and zero-dependency native desktop GUI with nested context menus are fully implemented and operational.

## Project direction

Lotus is a self-contained workbook that holds values, formulas, small sandboxed BASIC programs, documents, and its own update history. The workbook file is also the AI exchange surface: updates arrive as Z-layer batches, and the viewer reads those updates from the file. The runtime is intended to stay inside the workbook boundary.

The desktop interface operates without heavy third-party GUI frameworks (`winit`/`softbuffer` removed). The vector rasterizer blits directly to standard 32-bit linear pixel surfaces via native Win32 GDI DIBs, maintaining strict parity with future BareMetal OS / exokernel Linear FrameBuffer (LFB) execution.

## Working prototype

- **Workbook file:** readable tagged format with sheets, named cells, embedded documents, formulas, BASIC source, cell layout spans (`colSpan`, `rowSpan`), custom column/row sizing, overlays, and exact raw blob payloads.
- **Calculations:** formula parsing, dependency ordering, recalculation, cycle detection, and typed cached results.
- **BASIC:** embedded bytecode interpreter with instruction limits, per-cell read/write grants, and staged writes.
- **History:** per-sheet Z layers and tombstones; the base sheet stays intact. The CLI can browse and compare historical views.
- **AI file exchange:** the viewer checks the workbook every 30 seconds, then every 10 seconds for one minute after a user save. Complete appended batches can contain many Z layers. Invalid or incompatible external file changes are held back to protect the workbook.
- **Documents:** embedded Markdown renders live cell references including sheet-qualified ones. Cells point back to documents. `links` inspects both directions. `doc id#fragment` renders a named section. `orphans` audits the entire workbook for broken references, dangling formulas, and unreferenced documents.
- **Vector GUI (`lotus-gui`):** Zero-external-dependency native desktop interface. Clean Lotus 1-2-3 two-line control panel on top (no desktop menu bars). Keyboard-first navigation (arrows, Enter, Esc, `/` menu, Ctrl shortcuts) + full mouse support (cell selection, right-click context menu with nested Lotus commands and right-aligned hotkey hints).

## Useful commands

```sh
cargo test --workspace --offline
cargo build --release --offline
cargo run -p lotus-gui --release
cargo run -p lotus-cli -- workspace demo.lotus
cargo run -p lotus-cli -- doc demo.lotus briefing
cargo run -p lotus-cli -- orphans demo.lotus
```

The latest recorded workspace test run passed **71 tests**: 48 core, 14 CLI, 2 edge, and 7 vector raster integration tests.

## Vector Engine & Native GUI Harness

- **Vector IR & Command Stream (`lotus_core::model::vector`)**: `Point2D`, `Rect2D`, `ColorRgba`, `StrokeStyle`, and `VectorBuffer` with compact binary serialization (`LVEC\x01`).
- **Hershey Stroke Font Engine (`lotus_core::view::font::hershey`)**: Clean `#![no_std]` decoder for Dr. A. V. Hershey's 1967 Roman Simplex font (ASCII 32..=126).
- **ClearType-Style Subpixel Software Rasterizer (`lotus_core::view::raster`)**: Pure Rust rasterizer rendering directly to linear 32-bit pixel buffers (`&mut [u32]`) using 3x horizontal subpixel bins, low-pass FIR energy filtering, and anti-aliasing.
- **Vector Workspace Screen (`lotus_core::view::workspace_view`)**: Renders control panel, column/row headers, spreadsheet cells, and active focus cursor box. Includes cell hit-testing from mouse coordinates.
- **Context Menu & Nested Lotus Commands (`lotus_core::view::gui`)**: Right-click vector context menu with standard actions (Cut, Copy, Paste, Clear) and nested Lotus `/` commands (`/WR`, `/WO`, `/FS`, `/FR`, `/BR`, `/TJ`, `/Q`) with hotkey hints.
- **Zero-Dependency Win32 Harness (`lotus-gui`)**: 673 KB executable using direct Windows GDI DIB blitting and native clipboard FFI with zero external crates.

## Still in front of us

1. AI Exchange & Turn Provenance: implement the 3 exchange lanes (manual copy/paste, file polling, API pipe) and turn flags defined in the design spec.
2. BareMetal OS LFB handoff: direct physical framebuffer pointer blitting.
3. Interactive cell-to-document hyperlinking within the vector GUI.


This remains a prototype rather than a finished end-user application. The immediate goal is to make the important workflows function end to end; a later rewrite can tighten the architecture once those behaviors are better understood.

## Checkpoints and backup

See `HANDOVER.md` for the source layout and detailed checkpoint history. The latest work includes `CHECKPOINT-MARKDOWN-SECTION-ANCHORS.md`, `CHECKPOINT-BIDIRECTIONAL-DOCUMENT-LINKS.md`, and `CHECKPOINT-Z-LAYER-COMPARE.md`.

The handover and checkpoint notes were copied before this session's changes to `checkpoint-backups/2026-11-10-pre-markdown-section-anchors/`.
