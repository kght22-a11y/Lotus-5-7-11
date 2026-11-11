//! Single-pass compiler from Modern BASIC source to compact bytecode.

#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec};

use super::bytecode::{Opcode, Value};

#[derive(Debug, PartialEq)]
pub enum CompileError {
    UnexpectedToken(String),
    UnexpectedEof,
    SyntaxError(String),
}

pub struct Compiler<'a> {
    tokens: Vec<Token>,
    pos: usize,
    code: Vec<Opcode>,
    _phantom: core::marker::PhantomData<&'a ()>,
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    StringLit(String),
    Plus,
    Minus,
    Star,
    Slash,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    LParen,
    RParen,
    Comma,
    Newline,
}

impl<'a> Compiler<'a> {
    pub fn compile(source: &str) -> Result<Vec<Opcode>, CompileError> {
        let tokens = tokenize_basic(source);
        let mut compiler = Self {
            tokens,
            pos: 0,
            code: Vec::new(),
            _phantom: core::marker::PhantomData,
        };
        compiler.parse_program()?;
        compiler.code.push(Opcode::Halt);
        Ok(compiler.code)
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

    fn skip_newlines(&mut self) {
        while let Some(Token::Newline) = self.peek() {
            self.advance();
        }
    }

    fn parse_program(&mut self) -> Result<(), CompileError> {
        self.skip_newlines();
        while self.pos < self.tokens.len() {
            self.parse_statement()?;
            self.skip_newlines();
        }
        Ok(())
    }

    fn parse_statement(&mut self) -> Result<(), CompileError> {
        self.skip_newlines();
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if let Token::Ident(word) = tok {
            let upper = word.to_ascii_uppercase();
            match upper.as_str() {
                "SUB" => {
                    self.advance(); // consume SUB
                    // consume name and optional ()
                    if let Some(Token::Ident(_)) = self.peek() {
                        self.advance();
                    }
                    if self.peek() == Some(&Token::LParen) {
                        self.advance();
                        if self.peek() == Some(&Token::RParen) {
                            self.advance();
                        }
                    }
                    // Parse until END SUB
                    while let Some(t) = self.peek() {
                        if let Token::Ident(w) = t {
                            if w.to_ascii_uppercase() == "END" {
                                self.advance();
                                if let Some(Token::Ident(s)) = self.peek() {
                                    if s.to_ascii_uppercase() == "SUB" {
                                        self.advance();
                                        break;
                                    }
                                }
                            }
                        }
                        self.parse_statement()?;
                        self.skip_newlines();
                    }
                    Ok(())
                }
                "LET" => {
                    self.advance(); // consume LET
                    let var_name = match self.advance() {
                        Some(Token::Ident(name)) => name,
                        _ => return Err(CompileError::SyntaxError("Expected variable name after LET".into())),
                    };
                    match self.advance() {
                        Some(Token::Eq) => {}
                        _ => return Err(CompileError::SyntaxError("Expected '=' after variable name in LET".into())),
                    }
                    self.parse_expr()?;
                    self.code.push(Opcode::Store(var_name));
                    Ok(())
                }
                "SET_CELL" => {
                    self.advance(); // consume SET_CELL
                    match self.advance() {
                        Some(Token::LParen) => {}
                        _ => return Err(CompileError::SyntaxError("Expected '(' after SET_CELL".into())),
                    }
                    // Arg 1: Target (coord or name)
                    self.parse_expr()?;
                    match self.advance() {
                        Some(Token::Comma) => {}
                        _ => return Err(CompileError::SyntaxError("Expected ',' between SET_CELL arguments".into())),
                    }
                    // Arg 2: Value
                    self.parse_expr()?;
                    match self.advance() {
                        Some(Token::RParen) => {}
                        _ => return Err(CompileError::SyntaxError("Expected ')' after SET_CELL arguments".into())),
                    }
                    self.code.push(Opcode::SetCell);
                    Ok(())
                }
                "PRINT" => {
                    self.advance(); // consume PRINT
                    self.parse_expr()?;
                    self.code.push(Opcode::Print);
                    Ok(())
                }
                "IF" => {
                    self.advance(); // consume IF
                    self.parse_expr()?;
                    if let Some(Token::Ident(w)) = self.peek() {
                        if w.to_ascii_uppercase() == "THEN" {
                            self.advance();
                        }
                    }

                    let jump_false_idx = self.code.len();
                    self.code.push(Opcode::JumpIfFalse(0)); // Placeholder

                    // Body
                    self.skip_newlines();
                    while let Some(t) = self.peek() {
                        if let Token::Ident(w) = t {
                            let u = w.to_ascii_uppercase();
                            if u == "END" || u == "ELSE" {
                                break;
                            }
                        }
                        self.parse_statement()?;
                        self.skip_newlines();
                    }

                    // Check for ELSE
                    let mut has_else = false;
                    let mut jump_end_idx = 0;
                    if let Some(Token::Ident(w)) = self.peek() {
                        if w.to_ascii_uppercase() == "ELSE" {
                            self.advance(); // consume ELSE
                            has_else = true;
                            jump_end_idx = self.code.len();
                            self.code.push(Opcode::Jump(0)); // Placeholder to jump over ELSE body
                            // Patch jump_false to point here
                            self.code[jump_false_idx] = Opcode::JumpIfFalse(self.code.len());

                            self.skip_newlines();
                            while let Some(t) = self.peek() {
                                if let Token::Ident(w) = t {
                                    if w.to_ascii_uppercase() == "END" {
                                        break;
                                    }
                                }
                                self.parse_statement()?;
                                self.skip_newlines();
                            }
                        }
                    }

                    // Expect END IF
                    if let Some(Token::Ident(w)) = self.advance() {
                        if w.to_ascii_uppercase() == "END" {
                            if let Some(Token::Ident(sub)) = self.advance() {
                                if sub.to_ascii_uppercase() != "IF" {
                                    return Err(CompileError::SyntaxError("Expected 'END IF'".into()));
                                }
                            }
                        }
                    }

                    if has_else {
                        self.code[jump_end_idx] = Opcode::Jump(self.code.len());
                    } else {
                        self.code[jump_false_idx] = Opcode::JumpIfFalse(self.code.len());
                    }

                    Ok(())
                }
                "WHILE" => {
                    self.advance(); // consume WHILE
                    let loop_start = self.code.len();
                    self.parse_expr()?;
                    let jump_false_idx = self.code.len();
                    self.code.push(Opcode::JumpIfFalse(0)); // Placeholder

                    self.skip_newlines();
                    while let Some(t) = self.peek() {
                        if let Token::Ident(w) = t {
                            if w.to_ascii_uppercase() == "WEND" {
                                self.advance(); // consume WEND
                                break;
                            }
                        }
                        self.parse_statement()?;
                        self.skip_newlines();
                    }

                    // Jump back to start
                    self.code.push(Opcode::Jump(loop_start));
                    self.code[jump_false_idx] = Opcode::JumpIfFalse(self.code.len());
                    Ok(())
                }
                _ => {
                    // Implicit LET: var = expr
                    self.advance(); // consume var name
                    if self.peek() == Some(&Token::Eq) {
                        self.advance(); // consume '='
                        self.parse_expr()?;
                        self.code.push(Opcode::Store(word));
                        Ok(())
                    } else {
                        Err(CompileError::SyntaxError(word))
                    }
                }
            }
        } else {
            self.advance();
            Ok(())
        }
    }

    fn parse_expr(&mut self) -> Result<(), CompileError> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<(), CompileError> {
        self.parse_addition()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Eq => Opcode::Eq,
                Token::NotEq => Opcode::NotEq,
                Token::Lt => Opcode::Lt,
                Token::LtEq => Opcode::LtEq,
                Token::Gt => Opcode::Gt,
                Token::GtEq => Opcode::GtEq,
                _ => break,
            };
            self.advance();
            self.parse_addition()?;
            self.code.push(op);
        }
        Ok(())
    }

    fn parse_addition(&mut self) -> Result<(), CompileError> {
        self.parse_multiplication()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Plus => Opcode::Add,
                Token::Minus => Opcode::Sub,
                _ => break,
            };
            self.advance();
            self.parse_multiplication()?;
            self.code.push(op);
        }
        Ok(())
    }

    fn parse_multiplication(&mut self) -> Result<(), CompileError> {
        self.parse_primary()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Star => Opcode::Mul,
                Token::Slash => Opcode::Div,
                _ => break,
            };
            self.advance();
            self.parse_primary()?;
            self.code.push(op);
        }
        Ok(())
    }

    fn parse_primary(&mut self) -> Result<(), CompileError> {
        let tok = self.advance().ok_or(CompileError::UnexpectedEof)?;
        match tok {
            Token::Number(n) => {
                self.code.push(Opcode::Push(Value::Number(n)));
                Ok(())
            }
            Token::StringLit(s) => {
                self.code.push(Opcode::Push(Value::StringLit(s)));
                Ok(())
            }
            Token::LParen => {
                self.parse_expr()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(()),
                    _ => Err(CompileError::SyntaxError("Expected ')'".into())),
                }
            }
            Token::Ident(name) => {
                let upper = name.to_ascii_uppercase();
                if upper == "GET_CELL" {
                    match self.advance() {
                        Some(Token::LParen) => {}
                        _ => return Err(CompileError::SyntaxError("Expected '(' after GET_CELL".into())),
                    }
                    self.parse_expr()?;
                    match self.advance() {
                        Some(Token::RParen) => {}
                        _ => return Err(CompileError::SyntaxError("Expected ')' after GET_CELL argument".into())),
                    }
                    self.code.push(Opcode::GetCell);
                    Ok(())
                } else if upper == "TRUE" {
                    self.code.push(Opcode::Push(Value::Boolean(true)));
                    Ok(())
                } else if upper == "FALSE" {
                    self.code.push(Opcode::Push(Value::Boolean(false)));
                    Ok(())
                } else {
                    self.code.push(Opcode::Load(name));
                    Ok(())
                }
            }
            _ => Err(CompileError::UnexpectedToken("Unexpected primary expression".into())),
        }
    }
}

fn tokenize_basic(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = source.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_whitespace() {
            if c == '\n' {
                tokens.push(Token::Newline);
            }
            i += 1;
            continue;
        }

        match c {
            '+' => tokens.push(Token::Plus),
            '-' => tokens.push(Token::Minus),
            '*' => tokens.push(Token::Star),
            '/' => tokens.push(Token::Slash),
            '(' => tokens.push(Token::LParen),
            ')' => tokens.push(Token::RParen),
            ',' => tokens.push(Token::Comma),
            '=' => tokens.push(Token::Eq),
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    tokens.push(Token::NotEq);
                    i += 1;
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::LtEq);
                    i += 1;
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    tokens.push(Token::GtEq);
                    i += 1;
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '"' => {
                i += 1;
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    s.push(chars[i]);
                    i += 1;
                }
                tokens.push(Token::StringLit(s));
            }
            ch if ch.is_ascii_digit() || ch == '.' => {
                let mut num_str = String::from(ch);
                while i + 1 < chars.len() && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '.') {
                    i += 1;
                    num_str.push(chars[i]);
                }
                let val = num_str.parse::<f64>().unwrap_or(0.0);
                tokens.push(Token::Number(val));
            }
            ch if ch.is_alphabetic() || ch == '_' => {
                let mut ident = String::from(ch);
                while i + 1 < chars.len() && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_') {
                    i += 1;
                    ident.push(chars[i]);
                }
                tokens.push(Token::Ident(ident));
            }
            _ => {}
        }
        i += 1;
    }

    tokens
}
