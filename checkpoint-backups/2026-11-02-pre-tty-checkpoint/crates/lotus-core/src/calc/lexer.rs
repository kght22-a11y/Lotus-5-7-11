//! Lexer for spreadsheet formulas.

#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
#[cfg(feature = "std")]
use std::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    StringLit(String),
    Ident(String),
    Colon,
    Comma,
    LParen,
    RParen,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Ampersand,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

pub struct Lexer<'a> {
    chars: core::str::CharIndices<'a>,
    peeked: Option<(usize, char)>,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let input = input.trim().strip_prefix('=').unwrap_or(input.trim());
        let mut chars = input.char_indices();
        let peeked = chars.next();
        Self {
            chars,
            peeked,
        }
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let current = self.peeked;
        self.peeked = self.chars.next();
        current
    }

    fn peek(&self) -> Option<char> {
        self.peeked.map(|(_, c)| c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace();
        let (_start_idx, ch) = self.advance()?;

        let token = match ch {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '^' => Token::Caret,
            '&' => Token::Ampersand,
            ':' => Token::Colon,
            ',' => Token::Comma,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '=' => Token::Eq,
            '<' => {
                if self.peek() == Some('>') {
                    self.advance();
                    Token::NotEq
                } else if self.peek() == Some('=') {
                    self.advance();
                    Token::LtEq
                } else {
                    Token::Lt
                }
            }
            '>' => {
                if self.peek() == Some('=') {
                    self.advance();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            '"' => {
                let mut s = String::new();
                while let Some((_, c)) = self.advance() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                Token::StringLit(s)
            }
            c if c.is_ascii_digit() || c == '.' => {
                let mut num_str = String::from(c);
                while let Some(peek_ch) = self.peek() {
                    if peek_ch.is_ascii_digit() || peek_ch == '.' {
                        num_str.push(peek_ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let num = num_str.parse::<f64>().unwrap_or(0.0);
                Token::Number(num)
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut ident = String::from(c);
                while let Some(peek_ch) = self.peek() {
                    if peek_ch.is_alphanumeric() || peek_ch == '_' || peek_ch == '$' {
                        ident.push(peek_ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                Token::Ident(ident)
            }
            _ => return None,
        };

        Some(token)
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while let Some(tok) = self.next_token() {
            tokens.push(tok);
        }
        tokens
    }
}
