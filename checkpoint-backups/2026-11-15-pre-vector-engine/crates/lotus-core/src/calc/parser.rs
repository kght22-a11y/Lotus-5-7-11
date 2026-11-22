//! Recursive-descent Pratt parser for spreadsheet formula expressions.

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, string::String, vec::Vec};

use super::{
    ast::{BinaryOp, Expr, UnaryOp},
    lexer::{Lexer, Token},
};
use crate::model::Coord;

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedToken(String),
    UnexpectedEof,
    InvalidReference(String),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_str(input: &str) -> Result<Expr, ParseError> {
        let tokens = Lexer::new(input).tokenize();
        let mut parser = Self::new(tokens);
        let expr = parser.parse_expr(0)?;
        if !parser.is_eof() {
            return Err(ParseError::UnexpectedToken("Trailing tokens".into()));
        }
        Ok(expr)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_primary()?;

        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                Some(Token::Percent) => BinaryOp::Mod,
                Some(Token::Caret) => BinaryOp::Pow,
                Some(Token::Ampersand) => BinaryOp::Concat,
                Some(Token::Eq) => BinaryOp::Eq,
                Some(Token::NotEq) => BinaryOp::NotEq,
                Some(Token::Lt) => BinaryOp::Lt,
                Some(Token::LtEq) => BinaryOp::LtEq,
                Some(Token::Gt) => BinaryOp::Gt,
                Some(Token::GtEq) => BinaryOp::GtEq,
                _ => break,
            };

            let (l_bp, r_bp) = infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }

            self.advance(); // consume operator
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Binary {
                op,
                left: Box::new(lhs),
                right: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let tok = self.advance().ok_or(ParseError::UnexpectedEof)?;

        match tok {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::StringLit(s) => Ok(Expr::Text(s)),
            Token::Minus => {
                let expr = self.parse_expr(10)?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(expr),
                })
            }
            Token::LParen => {
                let inner = self.parse_expr(0)?;
                match self.advance() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err(ParseError::UnexpectedToken("Expected ')'".into())),
                }
            }
            Token::Ident(name) => {
                let upper = name.to_ascii_uppercase();

                // Boolean literals
                if upper == "TRUE" {
                    return Ok(Expr::Boolean(true));
                }
                if upper == "FALSE" {
                    return Ok(Expr::Boolean(false));
                }

                // Function call: IDENT '(' args ')'
                if self.peek() == Some(&Token::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.parse_expr(0)?);
                            match self.peek() {
                                Some(Token::Comma) => {
                                    self.advance();
                                }
                                Some(Token::RParen) => break,
                                _ => return Err(ParseError::UnexpectedToken("Expected ',' or ')' in argument list".into())),
                            }
                        }
                    }
                    self.advance(); // consume ')'
                    return Ok(Expr::FunctionCall { name: upper, args });
                }

                // Range reference: IDENT ':' IDENT (e.g. A1:B10)
                if self.peek() == Some(&Token::Colon) {
                    self.advance(); // consume ':'
                    let right_tok = self.advance().ok_or(ParseError::UnexpectedEof)?;
                    if let Token::Ident(right_name) = right_tok {
                        let c1 = Coord::parse_a1(&strip_dollar(&name))
                            .ok_or_else(|| ParseError::InvalidReference(name.clone()))?;
                        let c2 = Coord::parse_a1(&strip_dollar(&right_name))
                            .ok_or_else(|| ParseError::InvalidReference(right_name.clone()))?;

                        let top_left = Coord::new(c1.col.min(c2.col), c1.row.min(c2.row));
                        let bottom_right = Coord::new(c1.col.max(c2.col), c1.row.max(c2.row));

                        return Ok(Expr::RangeRef { top_left, bottom_right });
                    } else {
                        return Err(ParseError::UnexpectedToken("Expected coordinate after ':'".into()));
                    }
                }

                // Single Cell Reference or Named Reference
                let cleaned = strip_dollar(&name);
                if let Some(coord) = Coord::parse_a1(&cleaned) {
                    Ok(Expr::CellRef(coord))
                } else {
                    Ok(Expr::NamedRef(name))
                }
            }
            _ => Err(ParseError::UnexpectedToken("Invalid primary expression".into())),
        }
    }
}

fn strip_dollar(s: &str) -> String {
    s.replace('$', "")
}

fn infix_binding_power(op: BinaryOp) -> (u8, u8) {
    match op {
        BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => (1, 2),
        BinaryOp::Concat => (3, 4),
        BinaryOp::Add | BinaryOp::Sub => (5, 6),
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => (7, 8),
        BinaryOp::Pow => (10, 9), // Right-associative
    }
}
