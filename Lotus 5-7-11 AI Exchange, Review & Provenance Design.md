# Lotus 5-7-11: AI Exchange, Review & Provenance Design

Design checkpoint, 2026-11-20. Covers a review of `lotus5711.zip` and the design decisions made in this session.

## Status at a glance

- **Sequencing decision:** finish the UI before starting AI integration. More UI is still to be hashed out.
- **Code verified:** the workspace builds and all 65 tests pass (14 CLI, 44 core, 2 edge, 5 vector raster). `lotus-gui` builds and has no tests of its own. The tests were run with an older cargo and a regenerated `Cargo.lock` in a scratch copy, because lock version 4 needs a newer cargo. The original lockfile was not touched.
- **Labels used below:** **Decided** means it was your call. **Proposed** means it is a suggestion that has not been confirmed. **Open** means it still needs a decision.

## Where the code stands

**Built since the 2026-11-10 progress report**

- Hershey stroke font, subpixel rasterizer and the BASIC turtle (11-15), all tested end to end.
- Cell overlays have started. `Cell` gained `col_span`, `row_span` and `overlay`. `Sheet` gained custom column widths and row heights, `cell_rect()` and `cell_obfuscates_data()` (the red line that signals data tucked under a large cell or curtain).
- `lotus-gui` crate with `GeoWorks::window()` in `view/gui.rs`. It renders a window and a triangle to `lotus_beta_ui.bmp`. One new integration test covers it.

**Gaps**

- The span, overlay and width fields are not written by the storage reader/writer, so they would be lost on save and reload. They are untested and nothing outside `cell.rs` and `sheet.rs` calls them.
- The GUI is a one-shot BMP export. There is no window (`winit` + `softbuffer`), no input, no AI focus cursor and no cell-to-document navigation yet.
- `HANDOVER.md` and `PROGRESS-REPORT.md` were last updated 11-15 and do not mention the overlays or the GUI.
- The 11-19 `pre-realign` backup has a flattened `src/` layout, but the live tree is still `crates/`. It is unclear whether the realign was rolled back or never started.
- The Linux build in `COMP_PLAN.md` is unchecked, and the Windows exe from 11-13 predates the vector work.

## AI integration: three lanes

**Decided**

1. **Manual copy/paste.** Generate Z-layer updates to paste into a chat interface, and provide an input to paste Z-layer updates back from the chat.
2. **Automatic file parser.** On the AI's turn, the file reloads every few seconds to pick up the AI's direct changes. On the human's turn, the human reviews the AI's updates and accepts or rejects them before making their own.
3. **API pipe (preferred).** A pipe to an API such as OpenAI's, or similar.

**Observation (proposed framing):** all three lanes deliver the same thing, a Z-layer batch. If the batch format and the review step are built once, the lanes differ only in transport.

**What exists today:** the viewer polls the file every 30 seconds (every 10 seconds for a minute after a save) and imports appended Z-layer batches automatically. It rejects any rewrite of the base. There is no review step, no turn tracking and no "generate a paste-ready update" path. An `import` subcommand exists and has not been examined.

## Turn flag

**Decided**

- The turn flag lives in the settings, not in the base workbook.
- The UI trigger is similar to Enter / Shift+Enter in a traditional AI chat interface.
- The AI can set the flag as a Z update, or directly when it manipulates the file.

**Code today:** there is no settings region in the file. The poll guard rejects any change to the title, sheets, documents or calc mode and only imports appended layers, so a flag written directly by the AI needs its own import path.

**Proposed**

- Record turn changes as appended records, where the last one wins.
- A pass is valid only from the side that currently holds the turn. That stops the AI grabbing the turn mid-edit and resolves simultaneous writes cleanly.
- Enter already commits a cell edit in the grid, so the hand-off needs a different chord (for example Ctrl+Enter) plus a slash-menu entry.

## Review step: dispositions

**Decided**

- The review needs more than accept / accept always / reject. It should work much like the "accept" / "accept always" prompts in agentic CLIs.
- A Z layer can be **kept** as part of a turn but not as part of the sheet going forward. It is still data that might need referencing later.
- Kept-but-unused layers go into a **bag** sorted by turn. A layer's status is flagged by addressing. When a layer is unused, its address comes off the active sequence and goes onto a list that serves as a quick reference for what to do with every Z layer (the manifest).

**What this means in the code**

- `composite_at_z` currently applies every layer from 1 up to the target Z, in order. Layers carry no turn, author or status, and Z is a stored position.
- With addressing, Z becomes a position in the active list, and each entry points at a layer ID (turn plus index). Composition and the consecutive-Z rule run over the active list.
- Incoming layers should arrive as pending, not be appended straight to history as the poll does now.
- The poll consults the manifest to know which layers it has already handled, so kept or rejected layers are not staged again.

**Proposed**

- A model reading the file can read the manifest first and know what is live, kept or rejected without opening any layer.
- The bag can sit in the binary end of the file, marked not to parse unless asked, and be reachable through pointer cells.

## Merge down

**Decided.** The file merges down like a Photoshop PSD, on request, in three ways:

- **Checkpoint.** A prerendered point in the history. It does not change the Z layers at all. It holds the fully calculated state as a starting point, so loading starts there and only runs the layers after it. All active layers are retained. Meant to run automatically every hundred or more turns, saving a lot of compute over time.
- **Lock-in.** Locks into a new base state and keeps the history as provenance. This is the most common manual merge. Afterwards a layer can no longer be switched off; changes are made as direct edits to the book. This also keeps dead links out of the file.
- **Fresh book.** Merges into a fresh book with no history, like a template or a log sheet. This is how a new base workbook starts.

**Proposed details**

- A checkpoint records the chain head it was built from. The loader compares it to the chain and rebuilds on a mismatch.
- Lock-in moves a base marker instead of renumbering. Layers below the marker keep their addresses so references still resolve. The old base or an earlier checkpoint is kept for browsing earlier states.
- A fresh book is a new file and leaves the old one intact (assumed, not confirmed).

**Open:** layers still pending at merge time were written against the old base. Either block the merge until they are reviewed, or carry them over with a note.

## Provenance, trust and deletion

**Decided**

- Z layers will be Merkle chained, if they are not already. An experiential AI cannot be built if it cannot trust its memory.
- Once a layer is accepted, or kept but unused, it can only be tombstoned. There are two kinds:
  - **Soft tombstone:** keeps the data and needs no chain patch. It is just saying goodbye to the layer for now.
  - **Hard tombstone:** for sensitive data that has to be deleted. A chain patch is left in place and the data is overwritten in place.
- Chain hashes will be **salted**. This replaces the earlier idea of a 128-bit minimum on its own, because hash length does not stop brute-forcing of small inputs. An attacker guesses the input and compares hashes, so a short input stays guessable at any hash size.

**Proposed**

- A random per-layer salt of at least 128 bits, stored inside the layer and overwritten with the data on a hard delete. Salt every layer (about 16 bytes each), since it is not known in advance which will hold something sensitive.
- 256-bit hash output. A 128-bit hash gives only about 64 bits of collision resistance, which is the property a tamper-evident chain relies on.
- Decisions (accept, keep, tombstone) are appended as chained records. The manifest is then a derived cache of those decisions, so it cannot be edited without leaving a trace.
- A hard delete must explicitly invalidate any checkpoint covering that layer. The chain head hash is unchanged by design, so the checkpoint check alone would still pass. It must also scrub derived copies: later layers, cached formula results and rendered `{{cell:...}}` references in documents. The dependency graph and document links can list them.
- Lotus cannot reach copies outside the file: old backup folders, text pasted into a chat in lane 1, or anything sent to an API in lane 3.
- Hard delete is a human-only operation, so the AI can never trigger it.

## Design principles

- Nothing is built for one thing. Every piece doubles as context management, and nothing is wasted.
- The AI never edits the workbook itself. It submits Z-layer updates and the user side incorporates them. Direct file writes by the AI are limited to layer submission and the turn flag.
- Any model that can read the file can use it, with nothing else brought to the table. The workbook (cells, pointers, collars, table of contents) is the context management.

## Open questions

1. **Accept always:** what does it cover (a sheet, a cell range, all AI layers), and how is it revoked? A standing grant stored in settings is proposed. BASIC's per-cell grants are the precedent in the code, though they are tied to procedures.
2. **Reject vs keep:** is reject different from keep? Proposed: reject is hidden by default and recorded so the AI does not propose it again.
3. **Turn grouping:** does it live in the manifest or in each layer's address?
4. **Pending layers at merge-down:** block the merge or carry them over?
5. **Turn hand-off key:** which chord, given that Enter commits a cell?
6. **Hash output size:** 128-bit or 256-bit.

## Suggested order of work

This is a proposal and follows your decision to finish the UI first.

**UI**

1. Persist the span, overlay and width fields in storage, with roundtrip tests, and wire `cell_rect` into rendering.
2. Build the live window (`winit` + `softbuffer`) over the rasterizer, with input.
3. Leave layout room for a pending-changes view, a turn indicator and a paste pane, using the Z-layer compare that already exists.

**AI integration**

1. Layer address and turn model, plus the manifest.
2. Settings region and turn flag, with their own import path through the poll guard.
3. Staged import in place of the current auto-accept, then the three lanes. Lane 2 has the most already built.
4. Merge-down, the Merkle chain, tombstones and salting.
