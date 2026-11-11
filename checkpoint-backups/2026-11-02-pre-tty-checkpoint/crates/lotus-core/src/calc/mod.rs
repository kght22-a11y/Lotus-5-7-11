pub mod ast;
pub mod engine;
pub mod eval;
pub mod lexer;
pub mod parser;

pub use ast::{BinaryOp, Expr, UnaryOp};
pub use engine::CalcEngine;
pub use eval::{EvalValue, Evaluator};
pub use lexer::{Lexer, Token};
pub use parser::{ParseError, Parser};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, CellValue, Coord, Sheet};

    #[test]
    fn test_formula_parsing_and_evaluation() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 10.0)); // A1 = 10
        sheet.set_cell(Cell::number(Coord::new(0, 1), 20.0)); // A2 = 20
        sheet.set_cell(Cell::number(Coord::new(0, 2), 30.0)); // A3 = 30

        // A4 = =A1 + A2 * 2 (Operator precedence: 10 + 40 = 50)
        sheet.set_cell(Cell::formula(Coord::new(0, 3), "=A1 + A2 * 2"));

        // A5 = =SUM(A1:A3) (10 + 20 + 30 = 60)
        sheet.set_cell(Cell::formula(Coord::new(0, 4), "=SUM(A1:A3)"));

        // A6 = =IF(A4 = 50, "PASS", "FAIL")
        sheet.set_cell(Cell::formula(Coord::new(0, 5), "=IF(A4 = 50, \"PASS\", \"FAIL\")"));

        let count = CalcEngine::recalculate_sheet(&mut sheet, None).unwrap();
        assert_eq!(count, 3);

        // Check A4
        match &sheet.get_cell(Coord::new(0, 3)).unwrap().value {
            CellValue::Formula { cached_num, .. } => assert_eq!(*cached_num, Some(50.0)),
            _ => panic!("Expected formula result"),
        }

        // Check A5
        match &sheet.get_cell(Coord::new(0, 4)).unwrap().value {
            CellValue::Formula { cached_num, .. } => assert_eq!(*cached_num, Some(60.0)),
            _ => panic!("Expected formula result"),
        }

        // Check A6
        match &sheet.get_cell(Coord::new(0, 5)).unwrap().value {
            CellValue::Formula { cached_text, .. } => assert_eq!(cached_text.as_deref(), Some("PASS")),
            _ => panic!("Expected formula result"),
        }
    }

    #[test]
    fn test_cycle_detection() {
        let mut sheet = Sheet::new("main", "Main");
        // A1 depends on B1
        sheet.set_cell(Cell::formula(Coord::new(0, 0), "=B1"));
        // B1 depends on A1
        sheet.set_cell(Cell::formula(Coord::new(1, 0), "=A1"));

        CalcEngine::recalculate_sheet(&mut sheet, None).unwrap();

        match &sheet.get_cell(Coord::new(0, 0)).unwrap().value {
            CellValue::Formula { cached_text, .. } => assert_eq!(cached_text.as_deref(), Some("#CYCLE!")),
            _ => panic!("Expected #CYCLE!"),
        }

        match &sheet.get_cell(Coord::new(1, 0)).unwrap().value {
            CellValue::Formula { cached_text, .. } => assert_eq!(cached_text.as_deref(), Some("#CYCLE!")),
            _ => panic!("Expected #CYCLE!"),
        }
    }
}
