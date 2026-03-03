use std::{iter::Peekable, str::Chars};

use crate::compile::{
    compilation_error::Result,
    sexpr::{Bool, Char, Num, Str, Symbol},
    span::Span,
};

use super::{compilation_error::CompilationError, token::Token};

pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).scan()
}

struct Lexer<'source> {
    it: Peekable<Chars<'source>>,
    cur: String,
    offset: usize,
    tokens: Vec<Token>,
}

impl Lexer<'_> {
    fn new(source: &'_ str) -> Lexer<'_> {
        Lexer {
            it: source.chars().peekable(),
            cur: String::new(),
            offset: 0,
            tokens: vec![],
        }
    }

    fn scan(&mut self) -> Result<Vec<Token>> {
        while self.look_ahead().is_some() {
            let res = self.scan_token()?;
            self.advance(res);
        }
        self.tokens.push(Token::EoF(self.get_span()));
        Ok(self.tokens.clone())
    }

    fn scan_token(&mut self) -> Result<Option<Token>> {
        Ok(match self.consume() {
            ' ' | '\r' | '\t' | '\n' => None,
            ';' => {
                self.consume_until(&|c| c == '\n');
                None
            }
            '(' => Some(Token::LParen(self.get_span())),
            ')' => Some(Token::RParen(self.get_span())),
            '`' => Some(Token::QuasiQuote(self.get_span())),
            '|' => Some(Token::Pipe(self.get_span())),
            '\'' => Some(Token::Quote(self.get_span())),
            '.' => Some(if !self.consume_if(".") {
                Token::Dot(self.get_span())
            } else if self.consume_if(".") {
                Token::Id(Symbol::new("..."), self.get_span())
            } else {
                return Err(self.emit_err("Expected '.' after '..'"));
            }),
            ',' => Some(if self.consume_if("@") {
                Token::CommaAt(self.get_span())
            } else {
                Token::Comma(self.get_span())
            }),
            '#' => Some(if self.consume_if("t") {
                Token::Bool(Bool(true), self.get_span())
            } else if self.consume_if("f") {
                Token::Bool(Bool(false), self.get_span())
            } else if self.consume_if("(") {
                Token::HashLParen(self.get_span())
            } else if self.consume_if("\\space") {
                Token::Char(Char(' '), self.get_span())
            } else if self.consume_if("\\newline") {
                Token::Char(Char('\n'), self.get_span())
            } else if self.consume_if("\\") && self.look_ahead().is_some() {
                Token::Char(Char(self.consume()), self.get_span())
            } else {
                return Err(
                    self.emit_err("Expected '#t', '#f', '#(', or a character literal after '#'")
                );
            }),
            '-' => Some(self.parse_minus()?),
            '0'..='9' => Some(self.parse_num()?),
            '"' => Some(self.parse_string()?),
            c if Self::is_id_initial(c) => Some(self.parse_id()?),
            c => return Err(self.emit_err(&format!("Unexpected character: '{}'", c))),
        })
    }

    fn parse_minus(&mut self) -> Result<Token> {
        if self.look_ahead().is_none() {
            return Ok(Token::Id(Symbol::new(&self.cur), self.get_span()));
        };
        self.consume_until(&|c| !Self::is_id_subsequent(c) && !c.is_ascii_digit());
        if let Ok(num) = self.cur.parse() {
            Ok(Token::Num(Num(num), self.get_span()))
        } else {
            Ok(Token::Id(Symbol::new(&self.cur), self.get_span()))
        }
    }

    fn parse_num(&mut self) -> Result<Token> {
        self.consume_until(&|c| !c.is_ascii_digit());

        if self.look_ahead() == Some('.') {
            self.consume();
            self.consume_until(&|c| !c.is_ascii_digit());
        }

        Ok(Token::Num(
            Num(self
                .cur
                .parse()
                .map_err(|_| self.emit_err(&format!("Invalid number: '{}'", self.cur)))?),
            self.get_span(),
        ))
    }

    fn parse_id(&mut self) -> Result<Token> {
        self.consume_until(&|c| !Self::is_id_subsequent(c));
        Ok(Token::Id(Symbol::new(&self.cur), self.get_span()))
    }

    fn parse_string(&mut self) -> Result<Token> {
        let mut is_escaped = false;
        while let Some(c) = self.look_ahead() {
            match c {
                '"' if !is_escaped => break,
                '\\' => is_escaped = !is_escaped,
                _ => is_escaped = false,
            }
            self.consume();
        }
        if self.look_ahead().is_none() {
            return Err(self.emit_err("Unterminated string literal"));
        };
        self.consume();
        Ok(Token::Str(
            Str(self.cur[1..self.cur.len() - 1]
                .replace("\\\\", "\\")
                .replace("\\\"", "\"")),
            self.get_span(),
        ))
    }

    fn consume_until<F>(&mut self, f: &F)
    where
        F: Fn(char) -> bool,
    {
        while let Some(c) = self.look_ahead() {
            if f(c) {
                break;
            }
            self.consume();
        }
    }

    fn consume_if(&mut self, s: &str) -> bool {
        let mut it = self.it.clone();
        for target in s.chars() {
            let Some(cur) = it.next() else {
                return false;
            };
            if cur != target {
                return false;
            }
        }
        self.cur.push_str(s);
        for _ in s.chars() {
            let _ = self.it.next();
        }
        true
    }

    fn look_ahead(&mut self) -> Option<char> {
        self.it.peek().copied()
    }

    fn consume(&mut self) -> char {
        let c = self.it.next().unwrap();
        self.cur.push(c);
        c
    }

    fn advance(&mut self, token: Option<Token>) {
        if let Some(token) = token {
            self.tokens.push(token)
        }
        self.offset += self.cur.len();
        self.cur.clear();
    }

    fn is_id_initial(c: char) -> bool {
        matches!(c,
            'A'..='Z'
            | 'a'..='z'
            | '!'
            | '$'
            | '%'
            | '&'
            | '*'
            | '/'
            | ':'
            | '<'
            | '='
            | '>'
            | '?'
            | '^'
            | '_'
            | '~'
            | '+'
            | '-'
        )
    }

    fn is_id_subsequent(c: char) -> bool {
        match c {
            '0'..='9' | '+' | '-' | '.' | '@' => true,
            c => Self::is_id_initial(c),
        }
    }

    fn get_span(&self) -> Span {
        Span {
            lo: self.offset,
            hi: self.offset + self.cur.len(),
        }
    }

    fn emit_err(&self, reason: &str) -> CompilationError {
        CompilationError {
            span: self.get_span(),
            reason: reason.to_owned(),
        }
    }
}
