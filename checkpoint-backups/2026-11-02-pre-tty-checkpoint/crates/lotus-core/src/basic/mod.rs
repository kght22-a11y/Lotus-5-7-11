pub mod bytecode;
pub mod compiler;
pub mod runner;
pub mod vm;

pub use bytecode::{Opcode, Value};
pub use compiler::{CompileError, Compiler};
pub use runner::{BasicError, BasicRunner};
pub use vm::{Vm, VmError};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calc::CalcEngine;
    use crate::model::{Cell, CellValue, Coord, Sheet};

    #[test]
    fn test_basic_execution_and_formula_recalc() {
        let mut sheet = Sheet::new("main", "Main");
        // A1 = 100.0
        sheet.set_cell(Cell::number(Coord::new(0, 0), 100.0).with_name("revenue"));
        // A2 = =A1 * 1.5
        sheet.set_cell(Cell::formula(Coord::new(0, 1), "=A1 * 1.5"));

        // Recalculate initial state
        CalcEngine::recalculate_sheet(&mut sheet, None).unwrap();
        match &sheet.get_cell(Coord::new(0, 1)).unwrap().value {
            CellValue::Formula { cached_num, .. } => assert_eq!(*cached_num, Some(150.0)),
            _ => panic!("Expected formula"),
        }

        // Run BASIC script that modifies A1
        let script = r#"
        SUB Simulate()
            LET cur = GET_CELL("revenue")
            SET_CELL("A1", cur + 200)
        END SUB
        "#;

        let writes = BasicRunner::execute(&mut sheet, script, None, None).unwrap();
        assert_eq!(writes, 1);

        // Verify A1 was updated to 300.0
        match &sheet.get_cell(Coord::new(0, 0)).unwrap().value {
            CellValue::Number(n) => assert_eq!(*n, 300.0),
            _ => panic!("Expected number"),
        }

        // Verify dependent formula A2 automatically recalculated to 450.0 (300 * 1.5)!
        match &sheet.get_cell(Coord::new(0, 1)).unwrap().value {
            CellValue::Formula { cached_num, .. } => assert_eq!(*cached_num, Some(450.0)),
            _ => panic!("Expected recalculated formula"),
        }
    }

    #[test]
    fn test_cell_confinement_sandbox() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 10.0));

        // Attempt to write outside assigned bounding box (A1..B2) to Z99
        let hostile_script = r#"
        LET cur = GET_CELL("A1")
        SET_CELL("Z99", cur * 10)
        "#;

        let confinement = Some((Coord::new(0, 0), Coord::new(1, 1))); // A1 to B2
        let result = BasicRunner::execute(&mut sheet, hostile_script, confinement, None);

        match result {
            Err(BasicError::Runtime(VmError::CellConfinementViolation(coord))) => {
                assert_eq!(coord, Coord::parse_a1("Z99").unwrap());
            }
            _ => panic!("Expected CellConfinementViolation, got {:?}", result),
        }

        // Verify sheet was untouched
        assert_eq!(sheet.cells.len(), 1);
        assert!(sheet.get_cell(Coord::parse_a1("Z99").unwrap()).is_none());
    }

    #[test]
    fn test_step_budget_infinite_loop() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 10.0));

        // Infinite loop
        let runaway_script = r#"
        LET i = 0
        WHILE 1
            LET i = i + 1
        WEND
        "#;

        let result = BasicRunner::execute(&mut sheet, runaway_script, None, Some(500));

        match result {
            Err(BasicError::Runtime(VmError::StepBudgetExceeded)) => {}
            _ => panic!("Expected StepBudgetExceeded, got {:?}", result),
        }
    }
}
