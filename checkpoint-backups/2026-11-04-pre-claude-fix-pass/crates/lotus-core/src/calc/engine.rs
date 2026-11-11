//! Dependency Graph (DAG) and Incremental Topological Recalculator.

#[cfg(not(feature = "std"))]
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
#[cfg(feature = "std")]
use std::collections::{BTreeMap, BTreeSet};

use super::{
    ast::Expr,
    eval::{EvalValue, Evaluator},
    parser::Parser,
};
use crate::model::{CellValue, Coord, Sheet, Workbook};

pub struct CalcEngine;

impl CalcEngine {
    /// Recalculates all formulas across a sheet using topological dependency ordering.
    /// Detects circular references and marks them with `#CYCLE!`.
    pub fn recalculate_sheet(sheet: &mut Sheet, workbook: Option<&Workbook>) -> Result<usize, String> {
        // 1. Parse all formula cells and extract dependencies
        let mut parsed_formulas: BTreeMap<Coord, (Expr, BTreeSet<Coord>)> = BTreeMap::new();

        for cell in sheet.cells.values() {
            if let CellValue::Formula { expr, .. } = &cell.value {
                if let Ok(ast) = Parser::parse_str(expr) {
                    let mut deps = BTreeSet::new();
                    collect_dependencies(&ast, &mut deps);
                    parsed_formulas.insert(cell.coord, (ast, deps));
                }
            }
        }

        let formula_cells: BTreeSet<Coord> = parsed_formulas.keys().copied().collect();
        let mut dependents: BTreeMap<Coord, BTreeSet<Coord>> = BTreeMap::new();
        let mut in_degree: BTreeMap<Coord, usize> = BTreeMap::new();

        for (coord, (_, deps)) in &parsed_formulas {
            let mut formula_deps_count = 0;
            for dep in deps {
                if formula_cells.contains(dep) {
                    dependents.entry(*dep).or_default().insert(*coord);
                    formula_deps_count += 1;
                }
            }
            in_degree.insert(*coord, formula_deps_count);
        }

        // 2. Cycle Detection using Kahn's algorithm / Topological sort
        let mut queue: Vec<Coord> = Vec::new();
        let mut remaining_in_degree = in_degree.clone();

        for (coord, deg) in &in_degree {
            if *deg == 0 {
                queue.push(*coord);
            }
        }

        let mut eval_order: Vec<Coord> = Vec::new();
        while let Some(coord) = queue.pop() {
            eval_order.push(coord);

            if let Some(downstream) = dependents.get(&coord) {
                for next in downstream {
                    if let Some(deg) = remaining_in_degree.get_mut(next) {
                        *deg = deg.saturating_sub(1);
                        if *deg == 0 {
                            queue.push(*next);
                        }
                    }
                }
            }
        }

        // If eval_order does not contain all formulas, cycles exist!
        let mut cycle_cells = BTreeSet::new();
        for (coord, deg) in &remaining_in_degree {
            if *deg > 0 {
                cycle_cells.insert(*coord);
            }
        }

        // Mark cycle cells with #CYCLE!
        for coord in &cycle_cells {
            if let Some(cell) = sheet.cells.get_mut(coord) {
                if let CellValue::Formula { ref mut cached_num, ref mut cached_text, .. } = cell.value {
                    *cached_num = None;
                    *cached_text = Some("#CYCLE!".into());
                }
            }
        }

        // 3. Evaluate in topological order
        let mut count = 0;
        for coord in eval_order {
            if let Some((ast, _)) = parsed_formulas.get(&coord) {
                let eval = Evaluator::new(sheet, workbook);
                let result = eval.eval(ast);

                if let Some(cell) = sheet.cells.get_mut(&coord) {
                    if let CellValue::Formula { ref mut cached_num, ref mut cached_text, .. } = cell.value {
                        match result {
                            EvalValue::Number(n) => {
                                *cached_num = Some(n);
                                *cached_text = None;
                            }
                            EvalValue::Text(t) => {
                                *cached_num = None;
                                *cached_text = Some(t);
                            }
                            EvalValue::Boolean(b) => {
                                *cached_num = None;
                                *cached_text = Some(if b { "TRUE".into() } else { "FALSE".into() });
                            }
                            EvalValue::Empty => {
                                *cached_num = None;
                                *cached_text = None;
                            }
                            EvalValue::Error(e) => {
                                *cached_num = None;
                                *cached_text = Some(e);
                            }
                        }
                    }
                }
                count += 1;
            }
        }

        Ok(count)
    }

    /// Recalculates all sheets in the entire workbook.
    pub fn recalculate_workbook(wb: &mut Workbook) -> Result<usize, String> {
        let mut total = 0;
        let num_sheets = wb.sheets.len();
        for i in 0..num_sheets {
            let mut sheet = wb.sheets[i].clone();
            let count = Self::recalculate_sheet(&mut sheet, None)?;
            wb.sheets[i] = sheet;
            total += count;
        }
        Ok(total)
    }
}

fn collect_dependencies(expr: &Expr, deps: &mut BTreeSet<Coord>) {
    match expr {
        Expr::CellRef(c) => {
            deps.insert(*c);
        }
        Expr::RangeRef { top_left, bottom_right } => {
            for col in top_left.col..=bottom_right.col {
                for row in top_left.row..=bottom_right.row {
                    deps.insert(Coord::new(col, row));
                }
            }
        }
        Expr::Unary { expr, .. } => collect_dependencies(expr, deps),
        Expr::Binary { left, right, .. } => {
            collect_dependencies(left, deps);
            collect_dependencies(right, deps);
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                collect_dependencies(arg, deps);
            }
        }
        _ => {}
    }
}
