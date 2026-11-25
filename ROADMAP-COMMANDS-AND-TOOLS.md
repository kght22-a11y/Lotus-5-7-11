# Lotus 5-7-11: Roadmap of Commands, Menus & Developer Tools

A practical checklist of classic Lotus 1-2-3 commands, sovereign workspace extensions, and AI exchange tools to consider implementing as the project matures.

---

## 1. Classic Spreadsheet Grid & Formatting (`/Worksheet`, `/Range`, `/Data`)

Classic Lotus 1-2-3 productivity relies on quick two-letter keyboard chords for range and grid manipulation.

### Range Commands (`/R`)
- [ ] **/Range Format (`/RF`):** Set display format for a cell or bounding range:
  - `/RFC` Currency (`$#,##0.00`)
  - `/RFP` Percent (`0.00%`)
  - `/RFF` Fixed decimal places (`0.00`)
  - `/RFD` Date formatting (`YYYY-MM-DD`)
  - `/RFG` General / default unformatted
- [ ] **/Range Erase (`/RE`):** Clear values and formulas across a selected rectangular range (`A1..C10`).
- [ ] **/Range Name (`/RN`):**
  - `/RNC` Create/assign a named reference to a cell or range (`revenue = A1`).
  - `/RND` Delete a named reference.
  - `/RNL` List all named ranges in the dynamic TOC.
- [ ] **/Range Span (`/RS`):** Merge active cell across multi-column (`col_span`) or multi-row (`row_span`) spans.
- [ ] **/Range Overlay (`/RO`):** Attach or detach a vector blob asset (`row 10,000+`) to project over a cell range.

### Worksheet Operations (`/W`)
- [ ] **/Worksheet Column-Width (`/WC`):** Interactively adjust column width for the current column or set default sheet width (`/WCD`).
- [ ] **/Worksheet Row-Height (`/WH`):** Interactively adjust row height for custom typography or chart banners.
- [ ] **/Worksheet Insert (`/WI`):**
  - `/WIR` Insert Row at cursor (shifting downstream formulas automatically).
  - `/WIC` Insert Column at cursor.
- [ ] **/Worksheet Delete (`/WD`):**
  - `/WDR` Delete Row.
  - `/WDC` Delete Column.
- [ ] **/Worksheet Titles / Freeze Panes (`/WT`):**
  - `/WTH` Freeze horizontal rows above cursor.
  - `/WTV` Freeze vertical columns left of cursor.
  - `/WTB` Freeze both (keeps row and column headers fixed during panning).
  - `/WTC` Clear frozen titles.

### Data Manipulation (`/D`)
- [ ] **/Data Sort (`/DS`):** Specify Data-Range, Primary Key column, and Ascending/Descending sort order.
- [ ] **/Data Fill (`/DF`):** Fill a range with an arithmetic sequence (Start, Step, Stop).
- [ ] **/Data Table (`/DT`):** One-variable and two-variable what-if sensitivity analysis tables.

---

## 2. Vector Graphics & Data Visualization (`/Graph`)

Lotus 1-2-3's revolutionary feature was instant graphing (`/G`). In Lotus 5-7-11, this pairs with the subpixel Hershey vector engine.

### Graph Configuration (`/G`)
- [ ] **/Graph Type (`/GT`):**
  - `/GTB` Bar chart (vertical/horizontal columns).
  - `/GTL` Line chart (continuous stroke paths).
  - `/GTP` Pie chart.
  - `/GTX` XY Scatter plot.
- [ ] **/Graph Ranges (`/GA` .. `/GF`):** Assign cell ranges to up to 6 data series (A through F) and `/GX` for X-axis category labels.
- [ ] **/Graph View (`/GV`):** Render vector graph fullscreen over the rasterizer surface (Escape returns to grid).
- [ ] **/Graph Options (`/GO`):** Configure Hershey font chart title, legend, grid lines, and axis tick labels.
- [ ] **/Graph Save / Commit (`/GS`):** Serialize vector chart buffer into a far-out blob asset cell (`row 10,000+`) with an active cell overlay.

### Turtle Graphics (`/GT` / `/BT`)
- [ ] **/Basic Turtle View (`/BTV`):** Run turtle script in active cell and render vector preview in split pane.
- [ ] **/Basic Turtle Commit (`/BTC`):** Manually trigger `COMMIT_TURTLE` to write vector paths into designated asset cell.

---

## 3. Embedded Modern BASIC IDE & Sandbox (`/Basic`)

- [ ] **/Basic Edit (`/BE`):** Open modal or split-pane text editor for the cell's embedded BASIC script.
- [ ] **/Basic Grants (`/BG`):** View and edit explicit capability grants (`basic_reads` and `basic_writes`) for the script.
- [ ] **/Basic Step / Debug (`/BD`):** Single-step through bytecode instructions (`Opcode`) with live stack, variable table, and watchdog budget inspection.
- [ ] **/Basic Eject (`/BJE`):** Export cell BASIC source to an external `.bas` file.
- [ ] **/Basic Import (`/BJI`):** Load `.bas` file into current cell.

---

## 4. AI Exchange & Turn Provenance (`/AI` & `Ctrl+Enter`)

Implementing the 3-lane AI architecture defined in the November 20 design specification.

### Turn Hand-Off & Status
- [ ] **Turn Handoff Chord (`Ctrl+Enter` or `/AT`):** Toggle active turn flag between `Human` and `AI`.
- [ ] **Turn State Indicator:** Top control panel badge: `[TURN: HUMAN]` vs. `[TURN: AI PENDING]`.

### AI Review & Diff Inspection (`/AR`)
- [ ] **/AI Review Batch (`/AR`):** Step through pending Z-layer changes introduced by the AI:
  - `Accept`: Merge change into base workbook.
  - `Accept Always`: Auto-accept remaining changes in this batch.
  - `Reject`: Discard proposed change.
  - `Keep as Provenance`: Retain in Z-layer history without committing to base sheet.
- [ ] **/AI Visual Diff (`/AD`):** Highlight changed cells on the grid in color (e.g. green for additions, cyan for formula tweaks, red outline for deletions) before approval.

### Transport Lanes (`/AX`, `/AI`, `/AP`)
- [ ] **/AI Export Prompt (`/AX`):** Copy a compact, token-efficient prompt to the system clipboard containing:
  - Active cell context + formula graph.
  - Briefing document references.
  - Current Z-layer delta template ready for LLM completion.
- [ ] **/AI Import Delta (`/AI`):** Paste an LLM-generated Z-layer XML batch from clipboard directly into the workbook.
- [ ] **/AI Pipe Config (`/AP`):** Configure local endpoint (local GGML / llama.cpp HTTP server, Ollama, or API pipe) for direct turn roundtrips.

---

## 5. Document Graph & Section Anchors (`/Doc`)

- [ ] **/Doc View (`/DV`):** Open vector viewer for embedded Markdown documents with live dynamic cell references (`{{cell:revenue}}`).
- [ ] **/Doc Edit (`/DE`):** In-place text editor for embedded Markdown documents.
- [ ] **/Doc Section Jump (`/DJ`):** Jump directly to named section anchors (`#section-slug`).
- [ ] **/Doc Link Cell (`/DL`):** Link active cell to an embedded document section.
- [ ] **/Doc Export (`/DX`):** Export resolved Markdown document to disk with current calculated values baked in.

---

## 6. Z-Axis History & Provenance Overlays (`/History`)

- [ ] **/History Timeline (`/HL`):** List all recorded Z-layers with timestamp, author tag (Human vs. AI), and cell change counts.
- [ ] **/History Step (`/HS` / `Alt+Left` & `Alt+Right`):** Step backwards and forwards in time, viewing the spreadsheet at historical Z coordinates without mutating base state.
- [ ] **/History Diff (`/HD`):** Compare two Z-layers side-by-side (`compare-z`).
- [ ] **/History Squash (`/HQ`):** Flatten approved Z-layers into base Z=0 to reclaim file size.

---

## 7. Storage, Sovereignty & System Substrate (`/File`, `/System`)

- [ ] **/File Eject Asset (`/FE`):** Eject raw binary blob (e.g. vector assets, media, GGML weights) from row 10,000+ to external file.
- [ ] **/File Import Asset (`/FI`):** Import any raw binary file into a named far-out asset blob cell.
- [ ] **/File Export BMP (`/FEP`):** Export snapshot of active view to uncompressed 24-bit `.bmp`.
- [ ] **/System Memory (`/SM`):** Display `#![no_std]` BumpArena allocation status, total memory region size, and high-water mark.
- [ ] **/System Substrate (`/SS`):** Monitor shared-memory mailbox partitions for inter-process communication or BareMetal OS exokernel handoffs.
