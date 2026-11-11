//! Evaluator and built-in function engine for spreadsheet formulas.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use super::ast::{BinaryOp, Expr, UnaryOp};
use crate::model::{CellValue, Coord, Sheet, Workbook};

#[derive(Debug, Clone, PartialEq)]
pub enum EvalValue {
    Number(f64),
    Text(String),
    Boolean(bool),
    Empty,
    Error(String),
}

impl EvalValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            EvalValue::Number(n) => Some(*n),
            EvalValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            EvalValue::Empty => Some(0.0),
            EvalValue::Text(t) => t.parse::<f64>().ok(),
            EvalValue::Error(_) => None,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            EvalValue::Number(n) => format!("{}", n),
            EvalValue::Text(t) => t.clone(),
            EvalValue::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).into(),
            EvalValue::Empty => String::new(),
            EvalValue::Error(e) => e.clone(),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            EvalValue::Boolean(b) => *b,
            EvalValue::Number(n) => *n != 0.0,
            EvalValue::Text(t) => !t.is_empty(),
            EvalValue::Empty => false,
            EvalValue::Error(_) => false,
        }
    }
}

pub struct Evaluator<'a> {
    pub sheet: &'a Sheet,
    pub workbook: Option<&'a Workbook>,
}

impl<'a> Evaluator<'a> {
    pub fn new(sheet: &'a Sheet, workbook: Option<&'a Workbook>) -> Self {
        Self { sheet, workbook }
    }

    pub fn eval(&self, expr: &Expr) -> EvalValue {
        match expr {
            Expr::Number(n) => EvalValue::Number(*n),
            Expr::Text(t) => EvalValue::Text(t.clone()),
            Expr::Boolean(b) => EvalValue::Boolean(*b),

            Expr::CellRef(coord) => self.resolve_cell(*coord),

            Expr::NamedRef(name) => self.resolve_named(name),

            Expr::RangeRef { .. } => {
                // Standalone range reference without aggregation returns #VALUE!
                EvalValue::Error("#VALUE!".into())
            }

            Expr::Unary { op, expr } => {
                let val = self.eval(expr);
                match op {
                    UnaryOp::Neg => match val.as_f64() {
                        Some(n) => EvalValue::Number(-n),
                        None => EvalValue::Error("#VALUE!".into()),
                    },
                    UnaryOp::Not => EvalValue::Boolean(!val.is_truthy()),
                }
            }

            Expr::Binary { op, left, right } => {
                let l = self.eval(left);
                let r = self.eval(right);
                self.eval_binary(*op, l, r)
            }

            Expr::FunctionCall { name, args } => self.eval_function(name, args),
        }
    }

    fn resolve_cell(&self, coord: Coord) -> EvalValue {
        match self.sheet.get_cell(coord) {
            Some(cell) => match &cell.value {
                CellValue::Number(n) => EvalValue::Number(*n),
                CellValue::Text(t) => EvalValue::Text(t.clone()),
                CellValue::Boolean(b) => EvalValue::Boolean(*b),
                CellValue::Formula { cached_num, cached_text, .. } => {
                    if let Some(n) = cached_num {
                        EvalValue::Number(*n)
                    } else if let Some(t) = cached_text {
                        EvalValue::Text(t.clone())
                    } else {
                        EvalValue::Empty
                    }
                }
                CellValue::Empty => EvalValue::Empty,
                CellValue::Error(e) => EvalValue::Error(e.clone()),
                CellValue::Basic { .. } | CellValue::Blob { .. } => EvalValue::Error("#VALUE!".into()),
            },
            None => EvalValue::Empty,
        }
    }

    fn resolve_named(&self, name: &str) -> EvalValue {
        // First check current sheet
        for cell in self.sheet.cells.values() {
            if cell.name.as_deref() == Some(name) {
                return self.resolve_cell(cell.coord);
            }
        }

        // Then check full workbook if available
        if let Some(wb) = self.workbook {
            for sheet in &wb.sheets {
                for cell in sheet.cells.values() {
                    if cell.name.as_deref() == Some(name) {
                        let eval = Evaluator::new(sheet, Some(wb));
                        return eval.resolve_cell(cell.coord);
                    }
                }
            }
        }

        EvalValue::Error("#NAME?".into())
    }

    fn eval_binary(&self, op: BinaryOp, l: EvalValue, r: EvalValue) -> EvalValue {
        match op {
            BinaryOp::Add => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Number(a + b),
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Sub => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Number(a - b),
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Mul => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Number(a * b),
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Div => match (l.as_f64(), r.as_f64()) {
                (Some(_), Some(b)) if b == 0.0 => EvalValue::Error("#DIV/0!".into()),
                (Some(a), Some(b)) => EvalValue::Number(a / b),
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Mod => match (l.as_f64(), r.as_f64()) {
                (Some(_), Some(b)) if b == 0.0 => EvalValue::Error("#DIV/0!".into()),
                (Some(a), Some(b)) => EvalValue::Number(a % b),
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Pow => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => {
                    #[cfg(feature = "std")]
                    return EvalValue::Number(a.powf(b));
                    #[cfg(not(feature = "std"))]
                    return EvalValue::Number(libm::pow(a, b));
                }
                _ => EvalValue::Error("#VALUE!".into()),
            },
            BinaryOp::Concat => EvalValue::Text(format!("{}{}", l.as_string(), r.as_string())),
            BinaryOp::Eq => EvalValue::Boolean(l == r),
            BinaryOp::NotEq => EvalValue::Boolean(l != r),
            BinaryOp::Lt => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Boolean(a < b),
                _ => EvalValue::Boolean(l.as_string() < r.as_string()),
            },
            BinaryOp::LtEq => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Boolean(a <= b),
                _ => EvalValue::Boolean(l.as_string() <= r.as_string()),
            },
            BinaryOp::Gt => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Boolean(a > b),
                _ => EvalValue::Boolean(l.as_string() > r.as_string()),
            },
            BinaryOp::GtEq => match (l.as_f64(), r.as_f64()) {
                (Some(a), Some(b)) => EvalValue::Boolean(a >= b),
                _ => EvalValue::Boolean(l.as_string() >= r.as_string()),
            },
        }
    }

    fn collect_numbers_from_args(&self, args: &[Expr]) -> Vec<f64> {
        let mut numbers = Vec::new();
        for arg in args {
            match arg {
                Expr::RangeRef { top_left, bottom_right } => {
                    for col in top_left.col..=bottom_right.col {
                        for row in top_left.row..=bottom_right.row {
                            let val = self.resolve_cell(Coord::new(col, row));
                            if let Some(n) = val.as_f64() {
                                numbers.push(n);
                            }
                        }
                    }
                }
                _ => {
                    let val = self.eval(arg);
                    if let Some(n) = val.as_f64() {
                        numbers.push(n);
                    }
                }
            }
        }
        numbers
    }

    fn eval_function(&self, name: &str, args: &[Expr]) -> EvalValue {
        match name {
            "SUM" => {
                let nums = self.collect_numbers_from_args(args);
                let sum: f64 = nums.iter().sum();
                EvalValue::Number(sum)
            }
            "AVERAGE" | "AVG" => {
                let nums = self.collect_numbers_from_args(args);
                if nums.is_empty() {
                    EvalValue::Error("#DIV/0!".into())
                } else {
                    let sum: f64 = nums.iter().sum();
                    EvalValue::Number(sum / nums.len() as f64)
                }
            }
            "MIN" => {
                let nums = self.collect_numbers_from_args(args);
                if let Some(min) = nums.into_iter().reduce(f64::min) {
                    EvalValue::Number(min)
                } else {
                    EvalValue::Number(0.0)
                }
            }
            "MAX" => {
                let nums = self.collect_numbers_from_args(args);
                if let Some(max) = nums.into_iter().reduce(f64::max) {
                    EvalValue::Number(max)
                } else {
                    EvalValue::Number(0.0)
                }
            }
            "COUNT" => {
                let nums = self.collect_numbers_from_args(args);
                EvalValue::Number(nums.len() as f64)
            }
            "IF" => {
                if args.len() < 2 {
                    return EvalValue::Error("#VALUE!".into());
                }
                let cond = self.eval(&args[0]);
                if cond.is_truthy() {
                    self.eval(&args[1])
                } else if args.len() >= 3 {
                    self.eval(&args[2])
                } else {
                    EvalValue::Boolean(false)
                }
            }
            "ABS" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) => EvalValue::Number(n.abs()),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "ROUND" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) => {
                            let decimals = args.get(1).and_then(|a| self.eval(a).as_f64()).unwrap_or(0.0) as i32;
                            let factor = 10f64.powi(decimals);
                            EvalValue::Number((n * factor).round() / factor)
                        }
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            // =============================
            // Scientific & Math Functions
            // =============================
            "SQRT" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) if n >= 0.0 => EvalValue::Number(n.sqrt()),
                        Some(_) => EvalValue::Error("#NUM!".into()),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "SIN" => self.eval_single_math(args, f64::sin),
            "COS" => self.eval_single_math(args, f64::cos),
            "TAN" => self.eval_single_math(args, f64::tan),
            "LN" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) if n > 0.0 => EvalValue::Number(n.ln()),
                        Some(_) => EvalValue::Error("#NUM!".into()),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "LOG" => {
                // LOG(number) = log10, LOG(number, base) = log_base
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) if n > 0.0 => {
                            let base = args.get(1)
                                .and_then(|a| self.eval(a).as_f64())
                                .unwrap_or(10.0);
                            if base <= 0.0 || base == 1.0 {
                                EvalValue::Error("#NUM!".into())
                            } else {
                                EvalValue::Number(n.ln() / base.ln())
                            }
                        }
                        Some(_) => EvalValue::Error("#NUM!".into()),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "LOG10" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) if n > 0.0 => EvalValue::Number(n.log10()),
                        Some(_) => EvalValue::Error("#NUM!".into()),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "EXP" => self.eval_single_math(args, f64::exp),
            "POW" | "POWER" => {
                if args.len() >= 2 {
                    match (self.eval(&args[0]).as_f64(), self.eval(&args[1]).as_f64()) {
                        (Some(base), Some(exp)) => EvalValue::Number(base.powf(exp)),
                        _ => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "MOD" => {
                if args.len() >= 2 {
                    match (self.eval(&args[0]).as_f64(), self.eval(&args[1]).as_f64()) {
                        (Some(_), Some(d)) if d == 0.0 => EvalValue::Error("#DIV/0!".into()),
                        (Some(n), Some(d)) => {
                            // Excel-compatible MOD: result has same sign as divisor
                            let r = n - d * (n / d).floor();
                            EvalValue::Number(r)
                        }
                        _ => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "INT" => self.eval_single_math(args, f64::floor),
            "CEILING" | "CEIL" => self.eval_single_math(args, f64::ceil),
            "FLOOR" => self.eval_single_math(args, f64::floor),
            "PI" => EvalValue::Number(core::f64::consts::PI),
            "SIGN" => {
                if let Some(arg) = args.first() {
                    match self.eval(arg).as_f64() {
                        Some(n) if n > 0.0 => EvalValue::Number(1.0),
                        Some(n) if n < 0.0 => EvalValue::Number(-1.0),
                        Some(_) => EvalValue::Number(0.0),
                        None => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }

            // =============================
            // Logical Functions
            // =============================
            "AND" => {
                if args.is_empty() {
                    return EvalValue::Error("#VALUE!".into());
                }
                for arg in args {
                    if !self.eval(arg).is_truthy() {
                        return EvalValue::Boolean(false);
                    }
                }
                EvalValue::Boolean(true)
            }
            "OR" => {
                if args.is_empty() {
                    return EvalValue::Error("#VALUE!".into());
                }
                for arg in args {
                    if self.eval(arg).is_truthy() {
                        return EvalValue::Boolean(true);
                    }
                }
                EvalValue::Boolean(false)
            }
            "NOT" => {
                if let Some(arg) = args.first() {
                    EvalValue::Boolean(!self.eval(arg).is_truthy())
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }

            // =============================
            // String Functions
            // =============================
            "LEN" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    EvalValue::Number(s.len() as f64)
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "LEFT" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    let count = args.get(1).and_then(|a| self.eval(a).as_f64()).unwrap_or(1.0) as usize;
                    let result: String = s.chars().take(count).collect();
                    EvalValue::Text(result)
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "RIGHT" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    let count = args.get(1).and_then(|a| self.eval(a).as_f64()).unwrap_or(1.0) as usize;
                    let chars: Vec<char> = s.chars().collect();
                    let start = chars.len().saturating_sub(count);
                    let result: String = chars[start..].iter().collect();
                    EvalValue::Text(result)
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "MID" => {
                if args.len() >= 3 {
                    let s = self.eval(&args[0]).as_string();
                    let start = self.eval(&args[1]).as_f64().unwrap_or(1.0) as usize;
                    let count = self.eval(&args[2]).as_f64().unwrap_or(0.0) as usize;
                    // MID is 1-based
                    let start_idx = start.saturating_sub(1);
                    let result: String = s.chars().skip(start_idx).take(count).collect();
                    EvalValue::Text(result)
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "CONCATENATE" | "CONCAT" => {
                let mut result = String::new();
                for arg in args {
                    result.push_str(&self.eval(arg).as_string());
                }
                EvalValue::Text(result)
            }
            "TRIM" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    // Excel TRIM: collapse all runs of spaces to single space, strip leading/trailing
                    let mut result = String::new();
                    let mut prev_space = true; // treat start as if preceded by space to strip leading
                    for c in s.chars() {
                        if c == ' ' {
                            if !prev_space {
                                result.push(' ');
                            }
                            prev_space = true;
                        } else {
                            result.push(c);
                            prev_space = false;
                        }
                    }
                    // strip trailing space
                    if result.ends_with(' ') {
                        result.pop();
                    }
                    EvalValue::Text(result)
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "UPPER" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    EvalValue::Text(s.to_ascii_uppercase())
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "LOWER" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    EvalValue::Text(s.to_ascii_lowercase())
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "REPT" | "REPEAT" => {
                if args.len() >= 2 {
                    let s = self.eval(&args[0]).as_string();
                    let count = self.eval(&args[1]).as_f64().unwrap_or(0.0) as usize;
                    EvalValue::Text(s.repeat(count))
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "SUBSTITUTE" => {
                // SUBSTITUTE(text, old, new, [instance_num])
                if args.len() >= 3 {
                    let text = self.eval(&args[0]).as_string();
                    let old = self.eval(&args[1]).as_string();
                    let new = self.eval(&args[2]).as_string();
                    if let Some(instance_arg) = args.get(3) {
                        let instance = self.eval(instance_arg).as_f64().unwrap_or(1.0) as usize;
                        // Replace only the Nth occurrence
                        let mut result = String::new();
                        let mut count = 0usize;
                        let mut remaining = text.as_str();
                        while let Some(pos) = remaining.find(&old) {
                            count += 1;
                            if count == instance {
                                result.push_str(&remaining[..pos]);
                                result.push_str(&new);
                                result.push_str(&remaining[pos + old.len()..]);
                                return EvalValue::Text(result);
                            } else {
                                result.push_str(&remaining[..pos + old.len()]);
                                remaining = &remaining[pos + old.len()..];
                            }
                        }
                        result.push_str(remaining);
                        EvalValue::Text(result)
                    } else {
                        // Replace all occurrences
                        EvalValue::Text(text.replace(&old, &new))
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "VALUE" => {
                if let Some(arg) = args.first() {
                    let s = self.eval(arg).as_string();
                    match s.trim().parse::<f64>() {
                        Ok(n) => EvalValue::Number(n),
                        Err(_) => EvalValue::Error("#VALUE!".into()),
                    }
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }
            "TEXT" => {
                // Simplified TEXT(value, format) - supports basic number formatting
                if args.len() >= 2 {
                    let val = self.eval(&args[0]).as_f64().unwrap_or(0.0);
                    let fmt = self.eval(&args[1]).as_string();
                    // Count decimal places from format like "0.00"
                    let decimals = if let Some(dot_pos) = fmt.find('.') {
                        fmt[dot_pos + 1..].chars().take_while(|c| *c == '0' || *c == '#').count()
                    } else {
                        0
                    };
                    EvalValue::Text(format!("{:.prec$}", val, prec = decimals))
                } else {
                    EvalValue::Error("#VALUE!".into())
                }
            }

            // =============================
            // Lookup Functions
            // =============================
            "VLOOKUP" => self.eval_vlookup(args),
            "HLOOKUP" => self.eval_hlookup(args),
            "INDEX" => self.eval_index(args),
            "MATCH" => self.eval_match(args),

            _ => EvalValue::Error(format!("#UNKNOWN_FN:{}!", name)),
        }
    }

    /// Helper for single-argument math functions (SIN, COS, etc.)
    fn eval_single_math(&self, args: &[Expr], f: fn(f64) -> f64) -> EvalValue {
        if let Some(arg) = args.first() {
            match self.eval(arg).as_f64() {
                Some(n) => EvalValue::Number(f(n)),
                None => EvalValue::Error("#VALUE!".into()),
            }
        } else {
            EvalValue::Error("#VALUE!".into())
        }
    }

    /// VLOOKUP(lookup_value, range, col_index, [range_lookup])
    fn eval_vlookup(&self, args: &[Expr]) -> EvalValue {
        if args.len() < 3 {
            return EvalValue::Error("#VALUE!".into());
        }
        let lookup = self.eval(&args[0]);
        let (top_left, bottom_right) = match &args[1] {
            Expr::RangeRef { top_left, bottom_right } => (*top_left, *bottom_right),
            _ => return EvalValue::Error("#VALUE!".into()),
        };
        let col_index = match self.eval(&args[2]).as_f64() {
            Some(n) => n as usize,
            None => return EvalValue::Error("#VALUE!".into()),
        };
        if col_index < 1 || col_index > (bottom_right.col - top_left.col + 1) as usize {
            return EvalValue::Error("#REF!".into());
        }
        let exact = if let Some(arg) = args.get(3) {
            !self.eval(arg).is_truthy() // FALSE = exact match
        } else {
            false // default is approximate (TRUE)
        };

        // Search down the first column
        for row in top_left.row..=bottom_right.row {
            let cell_val = self.resolve_cell(Coord::new(top_left.col, row));
            let found = if exact {
                cell_val == lookup
            } else {
                // Approximate: find last value <= lookup_value (assumes sorted)
                match (cell_val.as_f64(), lookup.as_f64()) {
                    (Some(a), Some(b)) => a <= b,
                    _ => cell_val.as_string() <= lookup.as_string(),
                }
            };
            if exact && found {
                let result_col = top_left.col + (col_index as u32 - 1);
                return self.resolve_cell(Coord::new(result_col, row));
            }
            if !exact && !found {
                // We've gone past; use previous row
                if row > top_left.row {
                    let result_col = top_left.col + (col_index as u32 - 1);
                    return self.resolve_cell(Coord::new(result_col, row - 1));
                } else {
                    return EvalValue::Error("#N/A".into());
                }
            }
        }
        if exact {
            EvalValue::Error("#N/A".into())
        } else {
            // Largest value in range
            let result_col = top_left.col + (col_index as u32 - 1);
            self.resolve_cell(Coord::new(result_col, bottom_right.row))
        }
    }

    /// HLOOKUP(lookup_value, range, row_index, [range_lookup])
    fn eval_hlookup(&self, args: &[Expr]) -> EvalValue {
        if args.len() < 3 {
            return EvalValue::Error("#VALUE!".into());
        }
        let lookup = self.eval(&args[0]);
        let (top_left, bottom_right) = match &args[1] {
            Expr::RangeRef { top_left, bottom_right } => (*top_left, *bottom_right),
            _ => return EvalValue::Error("#VALUE!".into()),
        };
        let row_index = match self.eval(&args[2]).as_f64() {
            Some(n) => n as usize,
            None => return EvalValue::Error("#VALUE!".into()),
        };
        if row_index < 1 || row_index > (bottom_right.row - top_left.row + 1) as usize {
            return EvalValue::Error("#REF!".into());
        }
        let exact = if let Some(arg) = args.get(3) {
            !self.eval(arg).is_truthy()
        } else {
            false
        };

        for col in top_left.col..=bottom_right.col {
            let cell_val = self.resolve_cell(Coord::new(col, top_left.row));
            let found = if exact {
                cell_val == lookup
            } else {
                match (cell_val.as_f64(), lookup.as_f64()) {
                    (Some(a), Some(b)) => a <= b,
                    _ => cell_val.as_string() <= lookup.as_string(),
                }
            };
            if exact && found {
                let result_row = top_left.row + (row_index as u32 - 1);
                return self.resolve_cell(Coord::new(col, result_row));
            }
            if !exact && !found {
                if col > top_left.col {
                    let result_row = top_left.row + (row_index as u32 - 1);
                    return self.resolve_cell(Coord::new(col - 1, result_row));
                } else {
                    return EvalValue::Error("#N/A".into());
                }
            }
        }
        if exact {
            EvalValue::Error("#N/A".into())
        } else {
            let result_row = top_left.row + (row_index as u32 - 1);
            self.resolve_cell(Coord::new(bottom_right.col, result_row))
        }
    }

    /// INDEX(range, row_num, [col_num])
    fn eval_index(&self, args: &[Expr]) -> EvalValue {
        if args.is_empty() {
            return EvalValue::Error("#VALUE!".into());
        }
        let (top_left, _bottom_right) = match &args[0] {
            Expr::RangeRef { top_left, bottom_right } => (*top_left, *bottom_right),
            _ => return EvalValue::Error("#VALUE!".into()),
        };
        let row_num = if args.len() > 1 {
            self.eval(&args[1]).as_f64().unwrap_or(1.0) as u32
        } else {
            1
        };
        let col_num = if args.len() > 2 {
            self.eval(&args[2]).as_f64().unwrap_or(1.0) as u32
        } else {
            1
        };
        if row_num < 1 || col_num < 1 {
            return EvalValue::Error("#REF!".into());
        }
        let target = Coord::new(
            top_left.col + col_num - 1,
            top_left.row + row_num - 1,
        );
        self.resolve_cell(target)
    }

    /// MATCH(lookup_value, lookup_range, [match_type])
    fn eval_match(&self, args: &[Expr]) -> EvalValue {
        if args.len() < 2 {
            return EvalValue::Error("#VALUE!".into());
        }
        let lookup = self.eval(&args[0]);
        let (top_left, bottom_right) = match &args[1] {
            Expr::RangeRef { top_left, bottom_right } => (*top_left, *bottom_right),
            _ => return EvalValue::Error("#VALUE!".into()),
        };
        let match_type = if args.len() > 2 {
            self.eval(&args[2]).as_f64().unwrap_or(1.0) as i32
        } else {
            1
        };

        // Determine if horizontal or vertical range
        let is_row = top_left.row == bottom_right.row;
        let count = if is_row {
            (bottom_right.col - top_left.col + 1) as usize
        } else {
            (bottom_right.row - top_left.row + 1) as usize
        };

        let get_val = |i: usize| -> EvalValue {
            if is_row {
                self.resolve_cell(Coord::new(top_left.col + i as u32, top_left.row))
            } else {
                self.resolve_cell(Coord::new(top_left.col, top_left.row + i as u32))
            }
        };

        match match_type {
            0 => {
                // Exact match
                for i in 0..count {
                    if get_val(i) == lookup {
                        return EvalValue::Number((i + 1) as f64);
                    }
                }
                EvalValue::Error("#N/A".into())
            }
            1 => {
                // Largest value <= lookup (sorted ascending)
                let mut last_match = None;
                for i in 0..count {
                    let v = get_val(i);
                    match (v.as_f64(), lookup.as_f64()) {
                        (Some(a), Some(b)) if a <= b => last_match = Some(i),
                        (Some(a), Some(b)) if a > b => break,
                        _ => {}
                    }
                }
                match last_match {
                    Some(i) => EvalValue::Number((i + 1) as f64),
                    None => EvalValue::Error("#N/A".into()),
                }
            }
            -1 => {
                // Smallest value >= lookup (sorted descending)
                let mut last_match = None;
                for i in 0..count {
                    let v = get_val(i);
                    match (v.as_f64(), lookup.as_f64()) {
                        (Some(a), Some(b)) if a >= b => last_match = Some(i),
                        (Some(a), Some(b)) if a < b => break,
                        _ => {}
                    }
                }
                match last_match {
                    Some(i) => EvalValue::Number((i + 1) as f64),
                    None => EvalValue::Error("#N/A".into()),
                }
            }
            _ => EvalValue::Error("#VALUE!".into()),
        }
    }
}
