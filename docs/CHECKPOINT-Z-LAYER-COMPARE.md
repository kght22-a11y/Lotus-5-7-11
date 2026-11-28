# Checkpoint: Compare two Z-layer views

**Date:** 2026-11-06

## What changed

Added the read-only CLI command:

```sh
lotus compare-z <file.lotus> <sheet_id> <from_z> <to_z>
```

It composes both selected views, recalculates formulas in each, and lists only cells that differ, sorted by column then row. Each row shows the before and after value. Additions and deletions are explicit. Formula changes include their recalculated result. Long text is shortened; BASIC and blob cells show byte length and a compact content hash rather than dumping their contents.

Example output shape:

```text
Z comparison: sheet 'main' 2 -> 5 (2 changed cell(s))
A1: number 3 -> number 9
B4: (absent) -> text "Review this change"
```

The command does not change or save the workbook. Selecting the same Z twice reports zero changed cells. Invalid layer numbers and unknown sheets produce an error message.

## Backup

The full pre-change source, workbook, and current documentation are in `checkpoint-backups/2026-11-05-pre-z-layer-compare/`.

## Verification

- `cargo test --workspace` — passed: 11 CLI, 29 core, and 2 edge tests (42 total).
- `cargo build -p lotus-cli` — passed.
- Smoke-tested `target/debug/lotus-cli compare-z demo.lotus main 0 0`; it reported zero changed cells.
- Unit coverage checks changed values, additions, deletions, formula-result differences, and binary summaries.

