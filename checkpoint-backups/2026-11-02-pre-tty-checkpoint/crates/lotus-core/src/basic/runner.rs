//! Transactional execution coordinator for embedded Modern BASIC.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String};

use super::{
    compiler::{CompileError, Compiler},
    vm::{Vm, VmError},
};
use crate::{
    calc::CalcEngine,
    model::{Coord, Sheet},
};

#[derive(Debug)]
pub enum BasicError {
    Compile(CompileError),
    Runtime(VmError),
    Recalc(String),
}

pub struct BasicRunner;

impl BasicRunner {
    /// Compiles and executes a Modern BASIC script against a worksheet.
    ///
    /// Transactional semantics:
    /// - All writes are staged in a sandbox.
    /// - If the script completes successfully within bounds and step limits,
    ///   the staged writes commit atomically, and all affected formulas recalculate.
    /// - If an error or budget violation occurs, staged writes are discarded and the sheet is unmodified.
    pub fn execute(
        sheet: &mut Sheet,
        source: &str,
        confinement: Option<(Coord, Coord)>,
        max_steps: Option<usize>,
    ) -> Result<usize, BasicError> {
        let code = Compiler::compile(source).map_err(BasicError::Compile)?;

        let mut vm = Vm::new(sheet, &code);
        if let Some((tl, br)) = confinement {
            vm = vm.with_confinement(tl, br);
        }
        if let Some(steps) = max_steps {
            vm = vm.with_step_limit(steps);
        }

        vm.run().map_err(BasicError::Runtime)?;

        // Atomic Commit of staged writes
        let num_writes = vm.staged_writes.len();
        for (coord, val) in vm.staged_writes {
            if let Some(cell) = sheet.cells.get_mut(&coord) {
                cell.value = val;
            } else {
                sheet.set_cell(crate::model::Cell::new(coord, val));
            }
        }

        // Trigger DAG recalculation
        CalcEngine::recalculate_sheet(sheet, None).map_err(BasicError::Recalc)?;

        Ok(num_writes)
    }
}
