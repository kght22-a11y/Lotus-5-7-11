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
- The runtime has no access to the filesystem, network, processes, or operating-system services, and this is a permanent design boundary. External data must be brought into the workbook by the user before a procedure runs. Runtime grants cover workbook cells only; host-side import and export are separate operations.
- The workbook file is the AI exchange artifact; AI changes are proposed as Z-layer updates. Lotus must validate those updates as workbook changes. Embedded BASIC procedures separately use cell-level grants. Neither mechanism gives the runtime host access; tools supplied outside Lotus remain governed by the host.

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
- As of 2026-11-03, `cargo test --workspace` passes 21 tests, including explicit cross-cell read/write grant and transaction rollback coverage.
- The CLI grant syntax is cell-target based; future work can persist grant declarations with the procedure cell in the workbook format.
- Define workbook-native access to embedded blob payloads if procedures need to consume or produce binary resources. Such access must remain inside the workbook boundary; importing and ejecting files remain explicit host operations.
