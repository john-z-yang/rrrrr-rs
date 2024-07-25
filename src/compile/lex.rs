use std::{
    iter::{Enumerate, Peekable},
    str::Chars,
};

use crate::compile::{
    sexpr::{Num, Symbol},
    src_loc::SourceLoc,
};

use super::{compliation_error::CompliationError, token::Token};

pub fn tokenize(source: &str) -> Result<Vec<Token>, CompliationError> {
    struct Lexer<'source> {
        source: &'source str,
        it: Peekable<Enumerate<Chars<'source>>>,
        start: usize,
        cur: usize,
        line: usize,
        tokens: Vec<Token>,
    }

    impl Lexer<'_> {
        fn new(source: &str) -> Lexer {
            Lexer {
                source,
                it: source.chars().enumerate().peekable(),
                start: 0,
                cur: 0,
                line: 0,
                tokens: vec![],
            }
        }
        fn scan(&mut self) -> Result<Vec<Token>, CompliationError> {
            while self.it.peek().is_some() {
                self.start = self.cur;
                self.scan_token()?;
            }
            self.tokens.push(Token::EoF());
            Ok(self.tokens.clone())
        }
        fn scan_token(&mut self) -> Result<(), CompliationError> {
            let c = self.advance();
            match c {
                ' ' | '\r' | '\t' => (),
                '\n' => self.line += 1,
                '(' => self.add_token(Token::LParen(self.get_src_loc())),
                ')' => self.add_token(Token::RParen(self.get_src_loc())),
                '`' => self.add_token(Token::QuasiQuote(self.get_src_loc())),
                '|' => self.add_token(Token::Pipe(self.get_src_loc())),
                '\'' => self.add_token(Token::Quote(self.get_src_loc())),
                '.' => {
                    if self.advance_if('.') {
                        if let Some('.') = self.look_ahead() {
                            self.add_token(Token::Id(Symbol::new("..."), self.get_src_loc()))
                        } else {
                            Err(self.emit_err("Expecting '.' after '..'"))?
                        }
                    } else {
                        self.add_token(Token::Dot(self.get_src_loc()))
                    }
                }
                ',' => {
                    if self.advance_if('@') {
                        self.add_token(Token::CommaAt(self.get_src_loc()))
                    } else {
                        self.add_token(Token::Comma(self.get_src_loc()))
                    }
                }
                '#' => self
                    .advance_if('(')
                    .then(|| self.add_token(Token::HashLParen(self.get_src_loc())))
                    .ok_or_else(|| self.emit_err("Expecting '(' after '#'"))?,
                ';' => {
                    self.advance_until(&|c| if c == '\n' { true } else { false });
                }
                '0'..='9' | '-' => self.parse_num()?,
                '"' => self.parse_string()?,
                c if Self::is_id_initial(c) => self.parse_id()?,
                c => Err(self.emit_err(&format!("Unexpeted character: '{}'", c)))?,
            };
            Ok(())
        }
        fn parse_id(&mut self) -> Result<(), CompliationError> {
            while let Some(c) = self.look_ahead() {
                if !Self::is_id_subsequent(c) {
                    break;
                }
                self.advance();
            }
            self.add_token(Token::Id(
                Symbol::new(&self.source[self.start..self.cur]),
                self.get_src_loc(),
            ));
            Ok(())
        }
        fn parse_num(&mut self) -> Result<(), CompliationError> {
            self.advance_until(&|c| match c {
                '0'..='9' => false,
                _ => true,
            });

            if self.look_ahead() == Some('.') {
                self.advance();
                self.advance_until(&|c| match c {
                    '0'..='9' => false,
                    _ => true,
                });
            }

            let sub_str = self.source[self.start..self.cur].to_string();
            self.add_token(Token::Num(
                Num(sub_str.parse().map_err(|_| {
                    self.emit_err(&format!("Invalid number representation: {}", sub_str))
                })?),
                self.get_src_loc(),
            ));
            Ok(())
        }
        fn parse_string(&mut self) -> Result<(), CompliationError> {
            self.advance_until(&|c| if c == '"' { true } else { false });
            if self.look_ahead().is_none() {
                Err(self.emit_err("Unterminated string"))?;
            };
            self.advance();
            self.add_token(Token::String(
                self.source[self.start + 1..self.cur - 1].to_string(),
                self.get_src_loc(),
            ));
            Ok(())
        }
        fn advance_until<F>(&mut self, f: &F)
        where
            F: Fn(char) -> bool,
        {
            while let Some(c) = self.look_ahead() {
                match c {
                    '\n' if !f(c) => self.line += 1,
                    _ if f(c) => break,
                    _ => (),
                }
                self.advance();
            }
        }
        fn advance_if(&mut self, c: char) -> bool {
            self.it
                .next_if(|(_, next)| c == *next)
                .map(|(pos, _)| {
                    self.cur = pos + 1;
                    true
                })
                .unwrap_or(false)
        }
        fn look_ahead(&mut self) -> Option<char> {
            self.it.peek().map(|(_, c)| c).copied()
        }
        fn advance(&mut self) -> char {
            let (pos, c) = self.it.next().unwrap();
            self.cur = pos + 1;
            c
        }
        fn add_token(&mut self, token: Token) {
            self.tokens.push(token);
        }
        fn get_src_loc(&self) -> SourceLoc {
            SourceLoc {
                line: self.line,
                col: self.start,
                width: self.cur - self.start,
            }
        }
        fn is_id_initial(c: char) -> bool {
            match c {
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
                | '-' => true,
                _ => false,
            }
        }
        fn is_id_subsequent(c: char) -> bool {
            match c {
                '0'..='9' | '+' | '-' | '.' | '@' => true,
                c => Self::is_id_initial(c),
            }
        }
        fn emit_err(&self, reason: &str) -> CompliationError {
            CompliationError {
                source: self.source.to_string(),
                source_loc: self.get_src_loc(),
                reason: reason.to_owned(),
            }
        }
    }

    return Lexer::new(source).scan();
}

#[cfg(test)]
mod tests {
    use crate::compile::{
        lex::tokenize,
        sexpr::{Num, Symbol},
        src_loc::SourceLoc,
        token::Token,
    };

    #[test]
    fn test_tokenize_empty() {
        assert_eq!(tokenize("").unwrap(), vec![Token::EoF()]);
    }

    #[test]
    fn test_tokenize_multiline() {
        let src = "`(#())
; #
\"ab\" ; #
; #
\"\" 9.0001 0 -3 -42.00 -100 some-symbol <=? list->vector ;
2";
        assert_eq!(
            tokenize(src).unwrap(),
            vec![
                Token::QuasiQuote(SourceLoc {
                    line: 0,
                    col: 0,
                    width: 1
                }),
                Token::LParen(SourceLoc {
                    line: 0,
                    col: 1,
                    width: 1
                }),
                Token::HashLParen(SourceLoc {
                    line: 0,
                    col: 2,
                    width: 2
                }),
                Token::RParen(SourceLoc {
                    line: 0,
                    col: 4,
                    width: 1
                }),
                Token::RParen(SourceLoc {
                    line: 0,
                    col: 5,
                    width: 1
                }),
                Token::String(
                    "ab".to_string(),
                    SourceLoc {
                        line: 2,
                        col: 11,
                        width: 4
                    }
                ),
                Token::String(
                    "".to_string(),
                    SourceLoc {
                        line: 4,
                        col: 24,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(9.0001),
                    SourceLoc {
                        line: 4,
                        col: 27,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(0.0),
                    SourceLoc {
                        line: 4,
                        col: 34,
                        width: 1
                    }
                ),
                Token::Num(
                    Num(-3.0),
                    SourceLoc {
                        line: 4,
                        col: 36,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(-42.0),
                    SourceLoc {
                        line: 4,
                        col: 39,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(-100.0),
                    SourceLoc {
                        line: 4,
                        col: 46,
                        width: 4
                    }
                ),
                Token::Id(
                    Symbol::new("some-symbol"),
                    SourceLoc {
                        line: 4,
                        col: 51,
                        width: 11
                    }
                ),
                Token::Id(
                    Symbol::new("<=?"),
                    SourceLoc {
                        line: 4,
                        col: 63,
                        width: 3
                    }
                ),
                Token::Id(
                    Symbol::new("list->vector"),
                    SourceLoc {
                        line: 4,
                        col: 67,
                        width: 12
                    }
                ),
                Token::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 5,
                        col: 82,
                        width: 1
                    }
                ),
                Token::EoF()
            ]
        );
    }
}
