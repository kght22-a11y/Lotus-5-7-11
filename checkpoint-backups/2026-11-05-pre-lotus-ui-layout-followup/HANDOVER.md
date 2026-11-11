# Lotus 5-7-11: Developer Handover & Architecture State

**Last Updated:** 2026-11-02  
**Working Directory:** `/media/poop/Game Dump/whitepaper/new can/lotus`  
**Toolchain:** Rust 1.99.1 / Cargo (`$HOME/.cargo/bin`)

---

## 1. Project Status Summary

We have advanced the core architecture of **Lotus 5-7-11**, implementing:
1. **The Uncompressed Hybrid `.lotus` Format**: Human-readable in Notepad, ASCII-tagged in hex editors, inspectable and directly editable by an AI as a document, with raw uncompressed binary payloads preserved byte-for-byte.
2. **Dynamic Table of Contents (TOC)**: Dynamically calculated at `(maxX + 1, maxY + 1)` (e.g. `D27` for extents `C26`), referencing all named items.
3. **Reactive Spreadsheet DAG**: Clean-room Pratt formula parser, built-in function library, dependency DAG, cycle detection (`#CYCLE!`), and topological incremental recalculation.
4. **Embedded Modern BASIC Runtime**: Single-pass bytecode compiler and deterministic stack VM with instruction step budgets, per-cell read/write grants, and transactional staged write commits. A script can access its owning cell by default; other cells need explicit grants.
5. **Memory Autonomy**: `#![no_std]`-compatible memory arena (`BumpArena` and `MemoryRegion`) allowing the engine to manage its own memory space on x86-64 without libc/OS dependencies, laying the groundwork for BareMetal OS (Return Infinity) exokernel execution.
6. **Z-Axis Historical Layers & Tombstone Overlays**: Per-sheet Z-layer deltas with tombstones are serialized in `.lotus`. `history` lists updates and `view-z` composes a selected historical view without mutating the base sheet.
7. **Lotus 1-2-3 Hierarchical Menu State Machine (`/` slash menu)**: The authentic Lotus keyboard menu workflow with two-line display, first-letter quick jumps (e.g. `/FS` for File Save, `/WR` for Worksheet Recalculate), arrow-key cycling, and Escape key backtracking.
8. **Embedded Document Graph & Cross-References**: First-class embedded Markdown documents referencing live spreadsheet cells (`{{cell:revenue}}`, `{{cell:A1}}`), with live template evaluation and direct document ejection.
9. **Interactive Terminal Workspace Screen**: Full screen renderer combining the top cell status line, active Lotus menu bar, and the formatted grid.

---

## 2. Repository Layout

```
.
├── Cargo.toml                                # Workspace configuration
├── Lotus 5-7-11.md                           # Original concept & architecture whitepaper
├── Lotus 5-7-11 - The Sovereign Workspace.md # Philosophical & architectural manifesto
├── HANDOVER.md                               # This handover document
├── CHECKPOINT-FILE-Z-LAYERS.md               # File-based AI update layer checkpoint
├── CHECKPOINT-TERMINAL-VIEWPORT.md            # Fixed terminal viewport checkpoint
├── CHECKPOINT-TERMINAL-RESIZE.md              # Terminal-sized viewport checkpoint
├── CHECKPOINT-SHEET-BASIC-SELECTION.md         # Multi-sheet and BASIC selection checkpoint
├── CHECKPOINT-EDITOR-Z-PATCHES.md               # Editor changes recorded as Z-layer patches
├── demo.lotus                                # Generated sample workbook
├── crates/
│   ├── lotus-core/                           # #![no_std]-compatible core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── memory/                       # Autonomous memory space management
│   │       │   ├── mod.rs
│   │       │   └── arena.rs                  # MemoryRegion & BumpArena
│   │       ├── model/                        # Domain types & spatial coordinates
│   │       │   ├── mod.rs
│   │       │   ├── coord.rs                  # Coord (col, row) & A1 string conversions
│   │       │   ├── cell.rs                   # Cell & CellValue (Number, Formula, Basic, Blob)
│   │       │   ├── sheet.rs                  # Sheet sparse map & bounding extents
│   │       │   ├── toc.rs                    # Dynamic TOC structure & entries
│   │       │   └── workbook.rs               # Workbook container, history, eject/import methods
│   │       ├── storage/                      # Hybrid serialization & streaming parser
│   │       │   ├── mod.rs
│   │       │   ├── writer.rs                 # serialize_workbook (base + Z layers + raw blobs)
│   │       │   └── reader.rs                 # deserialize_workbook (streaming byte scanner and Z validation)
│   │       ├── calc/                         # Reactive spreadsheet engine
│   │       │   ├── mod.rs
│   │       │   ├── ast.rs                    # Expr AST & operators
│   │       │   ├── lexer.rs                  # Formula tokenizer
│   │       │   ├── parser.rs                 # Pratt recursive-descent parser
│   │       │   ├── eval.rs                   # Evaluator & built-in function library
│   │       │   └── engine.rs                 # CalcEngine (DAG, cycle check, topological sort)
│   │       ├── basic/                        # Sandboxed Modern BASIC
│   │       │   ├── mod.rs
│   │       │   ├── bytecode.rs               # Opcode & Value definitions
│   │       │   ├── compiler.rs               # Single-pass BASIC compiler
│   │       │   ├── vm.rs                     # VM with step watchdog & per-cell access checks
│   │       │   └── runner.rs                 # Transactional staging coordinator
│   │       ├── history/                      # Z-Axis History & Provenance Overlays
│   │       │   └── mod.rs                    # HistoryEngine, ordered layers & tombstones
│   │       ├── tui/                          # Interactive Lotus 1-2-3 UI & Menus
│   │       │   ├── mod.rs
│   │       │   ├── menu.rs                   # Hierarchical '/' slash menu state machine
│   │       │   └── editor.rs                 # WorkspaceSession & terminal screen composer
│   │       └── view/                         # Grid rendering & presentation
│   │           ├── mod.rs
│   │           └── grid.rs                   # Terminal grid rasterizer
│   └── lotus-cli/                            # Host harness & CLI runner
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                       # CLI commands (view, screen, inspect, set, recalc, run-basic, eject, import)
```

---

## 3. How to Run & Verify Everything

Make sure Cargo is in your path (`export PATH="$HOME/.cargo/bin:$PATH"`):

### Run All Unit & Integration Tests (latest: 35 passing across CLI, core, and edge suites)
```bash
cargo test
```
Latest run: 6 CLI tests, 27 core unit tests, and 2 `tests/edge.rs` integration tests. This includes:
- Shared exokernel substrate layout partitioning and zero-copy parameter mailbox for `ggml`.
- Floating point extremes (NaN, $\pm\infty$, $-0.0$, subnormals, high precision).
- Raw binary payloads containing null bytes `\0`, invalid UTF-8 `\xff\xfe`, and literal `</cell>` strings.
- Entity decoding order (`&amp;` decoded last to prevent entity collapse).
- Preserved text whitespace, CRLF, and newlines.
- Formula strings containing quotes (`=IF(1>0,"a","b")`).
- Maximum grid bounds (e.g. `XFD1048576` with TOC at `XFE1048577`).

### Render the Full Lotus 1-2-3 Interactive Workspace Screen
```bash
cargo run -p lotus-cli -- screen demo.lotus
```
Displays the cell status line, the active `/` Lotus menu bar with two-line explanation, and the rasterized grid with the Dynamic TOC marker at `D27`.

### View Just the Visual Grid
```bash
cargo run -p lotus-cli -- view demo.lotus
```

### View Embedded Live Markdown Documents
```bash
cargo run -p lotus-cli -- doc demo.lotus briefing
```
Renders the embedded Markdown report with live cell values and formulas interpolated.

### Inspect Workbook Structure & TOC Entries
```bash
cargo run -p lotus-cli -- inspect demo.lotus
```

### Inspect and View Historical Z Layers
```bash
cargo run -p lotus-cli -- history demo.lotus
cargo run -p lotus-cli -- view-z demo.lotus main 0
```

### Mutate Cells & Trigger Formula Recalculation
```bash
cargo run -p lotus-cli -- set demo.lotus A1 9000
cargo run -p lotus-cli -- view demo.lotus
```

### Execute Embedded BASIC Scripts
```bash
cargo run -p lotus-cli -- run-basic demo.lotus sim.bas --read revenue,tax_rate --write tax_amount
```
Executes the script inside the bytecode VM, atomically commits mutations, and recalculates dependent formulas.

### Eject a Raw Binary Cell Payload to a Disk File
```bash
cargo run -p lotus-cli -- eject demo.lotus weights.bin ejected_weights.bin
hexdump -C ejected_weights.bin
```

---

## 4. Key Architectural Patterns to Remember

1. **Cell Payloads are Exact Byte Slices**:
   In `demo.lotus`, raw binary data is stored immediately inside `<cell ... type="blob" bytes="32">` with zero encoding overhead. When `eject_cell` is called, it returns `&[u8]` directly.
2. **Formula Dependency Ordering**:
   `CalcEngine::recalculate_sheet` uses Kahn's algorithm over formula nodes. Static cells (numbers, text) do not block topological sorting. Cycles are detected and safely marked with `#CYCLE!`.
3. **BASIC Cell Access**:
   `BasicRunner::execute` grants the script access to its owning cell only by default. Additional cell reads and writes require separate explicit coordinate grants. An unauthorized access fails and all staged writes are discarded. The CLI accepts `--read` and `--write` lists of coordinates or names on the script's sheet. The runtime is permanently workbook-contained: it cannot access files, networks, processes, or operating-system services. Users bring external inputs into the workbook before running a script; import and export remain host-side workbook operations.
   The workbook file is the AI exchange artifact: the AI submits changes as Z-layer updates, and Lotus validates and records them. This layer-update workflow is not yet wired into `.lotus` serialization or the CLI. BASIC procedures remain separately restricted to their owning cell and explicit cell grants. Workbook content cannot give the runtime access to the host; separate tools are governed by the host.
4. **Z-Axis Historical Overlays**:
   `HistoryEngine::composite_at_z` composites historical layers $0 \dots Z$ sequentially without snapshot duplication. Explicit tombstones track deletions.
5. **Lotus Menu Workflow**:
   `MenuState` in `crates/lotus-core/src/tui/menu.rs` handles the `/` slash menus with two-line displays, letter shortcuts (e.g. `/FS`), and Escape backtracking.
6. **Memory Autonomy**:
   `BumpArena` in `crates/lotus-core/src/memory/arena.rs` allows the entire engine to be initialized from a flat physical memory slice passed by BareMetal OS.

---

## 5. Next Steps for Next Week

Checkpoint attempted on 2026-11-02: the interactive TTY loop is implemented in `lotus-cli` as `workspace <file.lotus>`. `cargo check -p lotus-cli` passes; the UI has not yet been exercised in a real terminal. The compile check also required completing the existing formula `%` operator implementation. See `CHECKPOINT-INTERACTIVE-WORKSPACE.md` for controls, limits, and verification status. A pre-change snapshot is kept at `checkpoint-backups/2026-11-02-pre-tty-checkpoint/`.

Runtime boundary checkpoint completed on 2026-11-02: BASIC cell reads and writes are now denied unless they target the script's owning cell or are explicitly granted. See `CHECKPOINT-RUNTIME-CELL-BOUNDARY.md`; a pre-change snapshot is kept at `checkpoint-backups/2026-11-02-pre-runtime-boundary/`.

File-based Z-layer checkpoint completed on 2026-11-02: `.lotus` stores per-sheet updates and tombstones; the CLI can list them and render a selected composite layer. See `CHECKPOINT-FILE-Z-LAYERS.md`; a pre-change snapshot is kept at `checkpoint-backups/2026-11-02-pre-z-layer-serialization/`.

Regression verification: `cargo test --workspace` passes all 26 tests (4 CLI, 20 core, 2 edge). Coverage includes BASIC read/write grants and staged rollback, Z-layer serialization, binary payloads, tombstones, composition, invalid layer ordering, viewport behavior, terminal-size calculation, sheet switching, BASIC selection, and editor patch composition. See `checkpoint-backups/2026-11-03-pre-regression-tests/` for the earlier test-source snapshot.

Terminal viewport checkpoint completed on 2026-11-03: interactive navigation uses a bounded viewport and can move across the worksheet without expanding the grid from A1. See `CHECKPOINT-TERMINAL-VIEWPORT.md`; pre-change source and docs are backed up in `checkpoint-backups/2026-11-03-pre-viewport/`.

Terminal resize checkpoint completed on 2026-11-04: the CLI recomputes viewport dimensions from terminal size on each draw and redraws after resize events. `cargo test --workspace` passes; real-terminal resizing has not been exercised. See `CHECKPOINT-TERMINAL-RESIZE.md`; a pre-change snapshot is at `checkpoint-backups/2026-11-04-pre-terminal-resize/`.

Sheet and BASIC selection checkpoint completed on 2026-11-04: Tab and Shift-Tab switch sheets; the BASIC menu lets the user choose a procedure on the active sheet. `cargo test --workspace` passes; the keyboard flow still needs hands-on use in a real terminal. See `CHECKPOINT-SHEET-BASIC-SELECTION.md`; a pre-change snapshot is at `checkpoint-backups/2026-11-04-pre-sheet-script-selection/`.

Editor Z-patch checkpoint completed on 2026-11-04: workspace edits and successful BASIC writes append cell changes as Z layers, leaving the base sheet intact. Save persists the updates; `history` and `view-z` inspect earlier states. See `CHECKPOINT-EDITOR-Z-PATCHES.md`; the pre-change files are in `checkpoint-backups/2026-11-04-pre-sheet-script-selection/`.

Claude review fix checkpoint completed on 2026-11-04: formula booleans retain type, named formula dependencies participate in ordering and cycle detection, BASIC cells carry explicit read/write grants, formula writes are rejected, edge round trips assert, and unsaved workspace layers prompt before quit/reload. `cargo test --workspace` passes 33 tests. The demo workbook was regenerated and its BASIC procedure ran successfully. See `CHECKPOINT-CLAUDE-REVIEW-FIXES.md`; pre-change files are in `checkpoint-backups/2026-11-04-pre-claude-fix-pass/`. `cargo fmt` was unavailable because rustfmt is not installed; manual terminal interaction remains unverified.

Real-terminal validation completed on 2026-11-05: exercised editing, Z-layer dirty marker, quit confirmation, Ctrl-S, dirty Retrieve confirmation, workspace BASIC execution with declared grants, and normal exit/terminal restoration in a PTY. The project demo was not modified; test workbook and a full source snapshot are under `checkpoint-backups/2026-11-05-pre-tty-validation/`. See `CHECKPOINT-REAL-TERMINAL-VALIDATION.md`. Resize behavior, multiple terminal emulators, and error-path restoration remain unverified.

Terminal alignment fix completed on 2026-11-05: raw mode had disabled LF-to-CRLF translation, causing screen rows to start at the previous cursor column. The CLI now emits CRLF in interactive screen output. `cargo test --workspace` passes 34 tests; a fresh PTY launch confirms the corrected line endings and clean terminal restoration. See `CHECKPOINT-TERMINAL-ALIGNMENT.md`; pre-change source snapshot is at `checkpoint-backups/2026-11-05-pre-terminal-alignment-fix/`.

Edit preview follow-up completed on 2026-11-05: reduced the grid viewport to keep the title and edit line on screen, and moved the live buffer onto the first line as `<sheet> | EDIT <cell>: <buffer>`. `cargo test --workspace` passes 35 tests; a 46-row PTY check confirms the first-line preview and footer stay visible. See `CHECKPOINT-EDIT-PREVIEW.md`.

Next, continue with:
- **Interactive TTY hardening**:
  - Build and exercise the CLI in a real terminal; confirm Crossterm dependency resolution, error recovery, and screen restoration.
- **Z-layer workflow follow-up**:
  - Let the interactive workspace select earlier Z layers for inspection.
  - Add a compact way to compare two layer views.
- **Embedded Document Graph (Markdown support)**:
  - Adding embedded document objects with bi-directional references between cells and document sections.
- **BareMetal OS Boot Stub & ggml Unikernel Interface**:
  - Writing the raw x86-64 execution handoff stub connecting the exokernel memory window to `lotus-core`.
