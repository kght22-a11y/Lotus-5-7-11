# Checkpoint: Vector Engine GUI & Nested Context Menu Implementation

**Date:** 2026-11-22  
**Goal:** Finalize the first implementation of the sovereign vector engine user interface, ditching bloated third-party dependencies (`winit`, `softbuffer`) in favor of a lean, zero-external-dependency Win32 window harness with mouse support and nested Lotus `/` command context menus.

---

## 1. Architectural Principles

1. **Zero External Dependencies:**
   - Eliminate `winit` and `softbuffer` from `lotus-gui`.
   - Implement native Win32 windowing and GDI DIB blitting via raw `extern "system"` FFI without third-party crates (`user32.dll`, `gdi32.dll`).
   - Retain 100% synergy with future BareMetal OS / exokernel LFB (Linear FrameBuffer) execution where the display is a raw `&[u32]` memory buffer.

2. **Clean Vector Workspace (No Desktop Menu Bar):**
   - No top-level desktop menu bar (File/Edit/View).
   - Lotus 1-2-3 authentic two-line control panel rendered directly via `SubpixelRasterizer` and `HersheyFont`:
     - **Line 1 (Status):** Sheet name, active coordinate (e.g. `A1`), cell format, value/formula, and status indicators (`[READY]`, `[EDIT]`, `[MENU]`).
     - **Line 2 (Edit/Menu):** Live cell edit buffer with cursor, or hierarchical Lotus slash menu commands.
   - Grid display below with scalable column widths, row heights, and active cell highlight.

3. **Interaction Model: Keyboard-First + Mouse Support:**
   - **Keyboard Navigation:**
     - Arrow keys move active cell cursor.
     - Typing numbers, text, or `=` immediately opens the live edit line.
     - `Enter` commits edits, evaluates formulas, and updates the dependency DAG.
     - `Escape` dismisses edits or closes active menus.
     - `/` opens the Lotus slash menu state machine.
     - `Ctrl+C`, `Ctrl+V`, `Ctrl+X`, `Ctrl+S` standard shortcuts.
   - **Mouse Interaction:**
     - **Left-Click:** Click any cell on the grid to set active focus; click edit line to place cursor; click menu items to trigger actions.
     - **Right-Click:** Opens the vector-rendered context menu at the cursor position.

4. **Right-Click Context Menu with Nested Lotus Commands:**
   - Standard clipboard entries:
     - `Cut` (`Ctrl+X`)
     - `Copy` (`Ctrl+C`)
     - `Paste` (`Ctrl+V`)
     - `Clear` (`Del`)
   - Nested Lotus `/` Command Submenus:
     - `Lotus Commands  >`
       - `Worksheet  >`
         - `Recalculate` (`/WR`)
         - `Orphans Audit` (`/WO`)
       - `File  >`
         - `Save` (`/FS`)
         - `Retrieve / Reload` (`/FR`)
         - `Inspect` (`/FI`)
       - `Basic  >`
         - `Run Script` (`/BR`)
       - `TOC  >`
         - `Jump to TOC` (`/TJ`)
       - `Quit` (`/Q`)
   - Submenus pop out on hover or click.
   - Hotkey hints are displayed right-aligned on each item to reinforce keyboard muscle memory.

---

## 2. Implementation Checklist

- [x] Create snapshot backup (`checkpoint-backups/2026-11-22-pre-vector-gui/`).
- [x] Remove `winit` and `softbuffer` from `crates/lotus-gui/Cargo.toml` (0 external crate dependencies).
- [x] Implement vector context menu and UI components in `lotus-core` (`crates/lotus-core/src/view/gui.rs`).
- [x] Implement full vector workspace renderer in `lotus-core` (`crates/lotus-core/src/view/workspace_view.rs`).
- [x] Implement zero-dependency native Win32 window harness in `crates/lotus-gui/src/win32.rs` and `main.rs` with native GDI blit and clipboard access.
- [x] Add integration tests verifying context menu hit testing, keyboard shortcuts, and rasterization (71 workspace tests passing).
- [x] Verify release build and relocate binary to `runtime/lotus-gui-win-0.25.exe` (673 KB executable).

---

## 3. Verification & Metrics

- **Workspace Test Suites:** 71 tests passing (48 core, 14 CLI, 2 edge, 7 vector raster).
- **Executable Footprint:**
  - `lotus-gui.exe`: 673 KB (pure Win32 + Lotus Core, zero external dependencies).
  - `lotus-cli.exe`: 901 KB (crossterm terminal harness).
- **Runtime Binaries:** Updated in `runtime/` (`lotus-gui-win-0.25.exe` and `lotus-can-win-0.25.exe`).

