# Checkpoint: File-Based Z-Layer Updates

**Date:** 2026-11-02  
**Status:** Implemented; `cargo check -p lotus-cli` passes  
**Pre-change backup:** `checkpoint-backups/2026-11-02-pre-z-layer-serialization/`

## Goal

Use the `.lotus` file itself as the AI exchange artifact. An AI can submit a proposed change as a new Z layer; Lotus loads, validates, stores, and composes that layer without a separate AI API.

## File representation

The base worksheet is Z=0. A top-level `<history>` element stores per-sheet deltas. Each `<layer>` has a sheet ID and the next consecutive Z value. Changed cells use the ordinary cell representation; deletions use a tombstone.

```xml
<history>
  <layer sheet="main" z="1">
    <cell ref="A1" type="number">9000</cell>
    <delete ref="B2" />
  </layer>
</history>
```

### Appended update batches

An AI can submit a batch by appending a second `<history>` block after the workbook’s closing `</workbook>` tag. The block can contain one layer or many consecutive layers; the reader validates the complete batch before the viewer accepts it. For example, the AI can append twelve `<layer>` blocks in one post. The file reader scans Lotus tags as a stream, so this append trailer is part of the `.lotus` format even though the combined file is not a single-root XML document.

```xml
</workbook>
<history>
  <layer sheet="main" z="4">
    <cell ref="A1" type="number">9000</cell>
  </layer>
  <layer sheet="main" z="5">
    <cell ref="B1" type="text">Review this change</cell>
  </layer>
</history>
<cursor sheet="main" x="0" y="0" z="5" />
```

The cursor line is reserved for a future AI focus marker: a workbook sheet and X, Y, Z location where the UI should direct attention and begin following links. The current viewer ignores this tag; cursor navigation and link following are deferred.

The UI polls the file every 30 seconds. After the user saves an update, it polls every 10 seconds for one minute, then returns to the 30-second interval. It hashes each read and parses only when the file changes. New history layers are imported without rewriting the base sheet. Concurrent batches are placed after already present local layers. A changed base workbook or malformed/incomplete batch is held back; Ctrl-S is blocked until the file is corrected or explicitly reloaded.

Cell types, formulas, BASIC source, and raw blob payloads use the same serialization rules as base-sheet cells.

## Validation and behavior

- Layers must start at Z=1 and be consecutive for each sheet.
- Every layer must refer to an existing worksheet.
- A cell coordinate may appear only once in a layer; a cell's stored coordinate must match its history record.
- A deletion must be a self-closing `<delete ref="..." />` inside a layer.
- The base sheet is not mutated. `Workbook::sheet_at_z` composes the base with changes through a selected layer; Z=0 returns the base.
- Viewing an earlier layer is the non-destructive way to reverse/review an update. The CLI does not rewrite the base or remove later history.
- `history <file> [sheet_id]` lists layer contents. `view-z <file> <sheet_id> <z>` renders a composed historical view.
- The writer preserves layer order and tombstones when it saves a workbook.

## Verification

- `cargo check -p lotus-cli` passes after wiring history through the workbook model, reader, writer, and CLI.
- Regression coverage was added on 2026-11-03; `cargo test --workspace` passes 21 tests, including Z-layer round-trip, tombstone composition, raw binary history payloads, and invalid ordering.
- `.lotus` files without a `<history>` element continue to load as Z=0 workbooks.

## Remaining small follow-ups

- Add a simple editor operation that records a human or AI proposed cell patch as the next layer.
- Decide whether a selected viewing Z belongs in the file or remains a UI choice. Current CLI selection is per command.
- Exercise binary and document payloads inside history layers in normal use.
