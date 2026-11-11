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
6. **Z-Axis Historical Layers & Tombstone Overlays**: Discrete Z-axis historical slice engine that overlays changes chronologically and supports explicit deletion tombstones without duplicating workbook snapshots.
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
│   │       │   └── workbook.rs               # Workbook container, eject/import methods
│   │       ├── storage/                      # Hybrid serialization & streaming parser
│   │       │   ├── mod.rs
│   │       │   ├── writer.rs                 # serialize_workbook (UTF-8 framing + raw blobs)
│   │       │   └── reader.rs                 # deserialize_workbook (streaming byte scanner)
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
│   │       │   └── mod.rs                    # HistoryEngine, HistoricalRecord & Tombstones
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

### Run All Unit & Integration Tests (18/18 passing across core and edge suites)
```bash
cargo test
```
Runs 16 core unit tests plus the comprehensive `tests/edge.rs` suite covering:
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
   AI-facing Lotus operations must use the same workbook-scoped permission checks. Workbook content cannot expand the AI's authority through Lotus; separate host-provided tools remain governed by the host.
4. **Z-Axis Historical Overlays**:
   `HistoryEngine::composite_at_z` composites historical layers $0 \dots Z$ sequentially without snapshot duplication. Explicit tombstones track deletions.
5. **Lotus Menu Workflow**:
   `MenuState` in `crates/lotus-core/src/tui/menu.rs` handles the `/` slash menus with two-line displays, letter shortcuts (e.g. `/FS`), and Escape backtracking.
6. **Memory Autonomy**:
   `BumpArena` in `crates/lotus-core/src/memory/arena.rs` allows the entire engine to be initialized from a flat physical memory slice passed by BareMetal OS.

---

## 5. Next Steps for Next Week

Checkpoint attempted on 2026-11-02: the interactive TTY loop is implemented in `lotus-cli` as `workspace <file.lotus>`. `cargo check -p lotus-cli` passes; the UI has not yet been exercised in a real terminal and tests were not run. The compile check also required completing the existing formula `%` operator implementation. See `CHECKPOINT-INTERACTIVE-WORKSPACE.md` for controls, limits, and verification status. A pre-change snapshot is kept at `checkpoint-backups/2026-11-02-pre-tty-checkpoint/`.

Runtime boundary checkpoint started on 2026-11-02: BASIC cell reads and writes are now denied unless they target the script's owning cell or are explicitly granted. See `CHECKPOINT-RUNTIME-CELL-BOUNDARY.md`; a pre-change snapshot is kept at `checkpoint-backups/2026-11-02-pre-runtime-boundary/`.

Next, continue with:
- **Interactive TTY hardening**:
  - Build and exercise the CLI in a real terminal; confirm Crossterm dependency resolution, error recovery, and screen restoration.
  - Add a viewport renderer before removing the current A-Z / 1-100 navigation clamp.
  - Add multi-sheet and BASIC script selection.
- **Embedded Document Graph (Markdown support)**:
  - Adding embedded document objects with bi-directional references between cells and document sections.
- **BareMetal OS Boot Stub & ggml Unikernel Interface**:
  - Writing the raw x86-64 execution handoff stub connecting the exokernel memory window to `lotus-core`.
