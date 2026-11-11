# Checkpoint: BASIC Cell Access Boundary

**Date:** 2026-11-02  
**Status:** Implemented; `cargo check -p lotus-cli` passes  
**Pre-change backup:** `checkpoint-backups/2026-11-02-pre-runtime-boundary/`

## Goal

Make a BASIC procedure cell-scoped by default. Grant formula/input/output access narrowly and explicitly, while keeping writes transactional.

## Runtime policy implemented

- A procedure is executed with the coordinate of the cell that contains its BASIC source.
- That owning cell is readable and writable by default.
- `GET_CELL` on any other cell requires an explicit read grant.
- `SET_CELL` on any other cell requires an explicit write grant.
- Read and write grants are separate. Granting a read does not grant a write.
- A denied access returns `VmError::CellAccessDenied`, and staged writes are discarded because the runner commits only after a successful VM run.
- After successful writes, the existing formula recalculation pass runs.
- The step budget remains in place (100,000 when the CLI launches the runtime).
- No general filesystem or operating-system I/O instructions exist in the BASIC VM yet. The grant lists cover workbook cells only.

`BasicRunner::execute` now requires an owner coordinate and separate read/write grant slices. This intentionally replaces the old optional rectangular write confinement API, which allowed unrestricted access when no bounds were supplied and did not check reads.

## CLI usage

No grants means own-cell access only:

```sh
cargo run -p lotus-cli -- run-basic demo.lotus sim.bas
```

The demo script reads `revenue` and `tax_rate`, and writes `tax_amount`, so run it with:

```sh
cargo run -p lotus-cli -- run-basic demo.lotus sim.bas --read revenue,tax_rate --write tax_amount
```

Coordinates such as `A1` and names such as `revenue` are accepted. Grants resolve on the script's sheet. The interactive menu runner passes no extra grants, so a script that accesses other cells is denied by default there.

## Backup inventory

The pre-change snapshot contains the BASIC VM, runner, module and unit-test source, CLI entry point, handover/checkpoint docs, and `demo.lotus`. It preserves the original behavior before this access-policy change.

## Verification and follow-up

- `cargo check -p lotus-cli` passed after updating all call sites.
- Tests have not been run.
- The CLI grant syntax is cell-target based; future work can persist grant declarations with the procedure cell in the workbook format.
- Decide how formula inputs and resource I/O should be represented as capabilities. This checkpoint does not add filesystem or host I/O.
