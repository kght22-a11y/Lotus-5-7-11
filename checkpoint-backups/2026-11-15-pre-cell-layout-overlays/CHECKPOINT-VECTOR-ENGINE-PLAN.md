# Checkpoint: Vector Engine, Hershey Font & Subpixel Rasterizer Plan

**Date:** 2026-11-15  
**Status:** In Progress — Implementation Commenced  
**Pre-modification Snapshot:** `checkpoint-backups/2026-11-15-pre-vector-engine/`

---

## 1. Vision & Core Principles

> *"Synergy is the #1 primal because everything is everything."*

The vector engine in Lotus 5-7-11 is not an ornamental GUI toolkit; it is a unified, self-contained computational primitive that bridges:
1. **Typography**: Single-stroke Hershey fonts (1967, public domain, vector/polyline based) scaling seamlessly from 8pt cell text to 72pt headers.
2. **Procedural Graphics**: Logo-style Turtle commands in embedded BASIC (`FD`, `BK`, `RT`, `LT`, `PU`, `PD`, `COLOR`, `WIDTH`, `LABEL`) that emit vector streams.
3. **Generative UI & Cell Overlays ("Curtains")**: Non-interactive vector projections that sit over functional spreadsheet cells (e.g. gauges, dials, sparklines, HUD brackets) while keyboard and mouse focus passes right through to the underlying calculation formulas.
4. **Resolution Independence**: What You See Is What You Get (WYSIWYG) across modern high-resolution displays (1080p, 1440p, 4K) through dynamic rasterizer-level zooming, without pixelation or pre-rendered asset bloat.
5. **ClearType Subpixel Rasterization**: 3x horizontal subpixel resolution evaluation with FIR energy filtering (`[1/4, 1/2, 1/4]`) to keep thin single-stroke vector lines crisp and legible on LCD displays without color fringing.
6. **Longevity & Storage Isolation**: Vector graphics are preserved as exact binary blobs at the binary end of the file. Far-out rows storing vector assets are excluded from active calculation bounds, keeping the Dynamic Table of Contents (TOC) anchored cleanly at `(maxX + 1, maxY + 1)` of the active spreadsheet grid.
7. **Autonomous `#![no_std]` Execution**: The entire pipeline operates on raw linear framebuffers (`&mut [u32]`), ready for both desktop OS (`softbuffer` / `winit`) and BareMetal OS (VESA / GOP physical LFB).

---

## 2. Architecture & Components

```
[BASIC Turtle] ───┐
[Hershey Font]  ───┼──> VectorBuffer (MoveTo, LineTo, Color, Width) ──> Far-Out Blob Storage (Excluded from TOC)
[Grid/Curtains] ───┘                                               │
                                                                   ▼
                                             Subpixel ClearType Software Rasterizer
                                             (Transform -> Subpixel Bins -> FIR -> Blit)
                                                                   │
                                                                   ▼
                                             Linear Framebuffer Surface (&mut [u32])
                                             (Exportable to .bmp, softbuffer, or BareMetal LFB)
```

### Component Breakdown
1. **Vector IR (`lotus_core::model::vector`)**:
   - `Point2D`, `Rect2D`, `ColorRgba`, `StrokeStyle`.
   - `VectorOp`: `MoveTo`, `LineTo`, `SetColor`, `SetWidth`, `ClosePoly`.
   - Binary serialization/deserialization for storage in `CellValue::Blob`.
2. **Hershey Font Engine (`lotus_core::view::font::hershey`)**:
   - Compact static byte/coordinate table for printable ASCII (32..126).
   - Metrics: left margin, right margin, stroke paths, pen-up sentinels.
   - Text layout function pushing vector operations to a `VectorBuffer`.
3. **Subpixel Rasterizer (`lotus_core::view::raster` / `lotus_raster`)**:
   - `Bitmap` surface with raw `Vec<u32>` buffer and pitch.
   - Anti-aliased line rendering with 3x horizontal subpixel coverage bins.
   - FIR filtering for color fringe suppression.
   - Zoom/pan affine transform matrix.
   - Headless `.bmp` exporter for visual test verification.
4. **Cell Classification & TOC Bounds Isolation**:
   - Separate active spreadsheet bounds (`active_grid_bounds`) from asset storage rows.
   - Far-out vector cells at row $10,000+$ do not shift the TOC marker from `(maxX + 1, maxY + 1)`.
5. **Logo Turtle in BASIC**:
   - Turtle state machine: position $(x, y)$, heading $\theta$, pen state, color, width.
   - New statements: `FORWARD` (`FD`), `BACK` (`BK`), `RIGHT` (`RT`), `LEFT` (`LT`), `PENUP` (`PU`), `PENDOWN` (`PD`), `PENCOLOR`, `PENWIDTH`, `LABEL`, `HOME`, `CLEAR`.
   - Output staging into cell blobs.
6. **Curtains / Overlays**:
   - Cell metadata `overlay = "blob_name"`, `span = (cols, rows)`.
   - Pass-through hit testing.

---

## 3. Test & Verification Plan

- [x] **Vector Serialization Tests**: Verify compact binary packing and unpacking of `VectorOp` streams.
- [x] **Hershey Font Decoding Tests**: Validate all printable ASCII characters (32..126) decode with non-zero strokes, proper left/right margins, and accurate pen-up sentinels.
- [x] **Rasterizer Dimensions & Clipping Tests**: Test rendering lines inside, partially overlapping, and outside the bitmap bounds without memory panics.
- [x] **ClearType Subpixel & Line Crispness Tests**: Verify single-pixel horizontal and vertical stroke rasterization and FIR filter energy balance.
- [x] **Bitmap Snapshot Tests**: Export rendered text and geometric shapes to `.bmp` and verify pixel coverage distribution.
- [x] **TOC Isolation Tests**: Verify that adding vector assets in row $10,000+$ leaves `active_grid_bounds()` and Dynamic TOC position unchanged.
- [x] **Turtle Graphics Tests**: Test square and triangular turtle procedures in BASIC and verify geometric closing coordinate accuracy, Hershey text labels along trajectories, and step budget compliance.
- [x] **End-to-End Pipeline Test**: BASIC Turtle execution -> Blob commit to A10000 -> Subpixel Rasterization with FIR filter -> BMP file export.
- [ ] **Curtains / Overlays**: Visual projection over functional cells with pass-through cursor navigation.
