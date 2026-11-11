# Checkpoint: Bidirectional cell/document links

**Date:** 2026-11-06

## What changed

Cells now store outgoing document links in the optional `documentRefs` attribute. It is a comma-separated list of document IDs, with optional Markdown section fragments:

```xml
<cell ref="A1" name="revenue" type="number" documentRefs="briefing#inputs,methodology">8000</cell>
```

Documents already supported explicit live references with `{{cell:target}}`. They now expose those references as graph edges, and can resolve a qualified target such as `{{cell:detail!A1}}` against another worksheet. Unqualified targets use the sheet passed to the renderer.

The new read-only command inspects both directions at the latest layer, or at a chosen Z layer:

```sh
lotus links <file.lotus> <sheet_id> <cell_or_name> [z]
```

Outgoing links are shown with `->`; documents that reference the cell are shown with `<-`. Unresolved outgoing document IDs are reported. Existing files without `documentRefs` load with an empty link list. Z-layer edits preserve and compare this metadata.

## Current limits

Section fragments are stored and displayed but are not checked against Markdown headings or navigated yet. The command reports graph edges; interactive cell-to-document navigation, snapshots, and dependency-triggered document refresh remain future work.

## Backup

The full pre-change source, demo workbook, handover, and recent checkpoint notes are in `checkpoint-backups/2026-11-06-pre-bidirectional-document-links/`.

## Verification

- `cargo test --workspace` — passed: 12 CLI, 31 core, and 2 edge tests (45 total).
- `cargo build -p lotus-cli` — passed.
- `target/debug/lotus-cli links demo.lotus main A1` found the demo briefing’s reverse reference to the `revenue` cell.
- Tests cover attribute round-trip, forward and reverse graph lookup, cross-sheet live values, historical links, and recording link edits in Z layers.

