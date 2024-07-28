use std::{iter::Peekable, str::Chars};

use crate::compile::{
    sexpr::{Bool, Char, Num, Str, Symbol},
    src_loc::SourceLoc,
};

use super::{compliation_error::CompliationError, token::Token};

pub fn tokenize(source: &str) -> Result<Vec<Token>, CompliationError> {
    struct Lexer<'source> {
        it: Peekable<Chars<'source>>,
        cur: String,
        col: usize,
        line: usize,
        tokens: Vec<Token>,
    }

    impl Lexer<'_> {
        fn new(source: &str) -> Lexer {
            Lexer {
                it: source.chars().peekable(),
                cur: String::new(),
                col: 0,
                line: 0,
                tokens: vec![],
            }
        }
        fn scan(&mut self) -> Result<Vec<Token>, CompliationError> {
            while self.look_ahead().is_some() {
                self.cur.clear();
                self.scan_token()?.map(|token| self.push_token(token));
            }
            self.cur.clear();
            self.tokens.push(Token::EoF(self.get_src_loc()));
            Ok(self.tokens.clone())
        }
        fn scan_token(&mut self) -> Result<Option<Token>, CompliationError> {
            Ok(match self.advance() {
                ' ' | '\r' | '\t' => {
                    self.parse_blank();
                    None
                }
                '\n' => {
                    self.parse_newline();
                    None
                }
                '(' => Some(Token::LParen(self.get_src_loc())),
                ')' => Some(Token::RParen(self.get_src_loc())),
                '`' => Some(Token::QuasiQuote(self.get_src_loc())),
                '|' => Some(Token::Pipe(self.get_src_loc())),
                '\'' => Some(Token::Quote(self.get_src_loc())),
                '.' => Some(if !self.advance_if('.') {
                    Token::Dot(self.get_src_loc())
                } else if self.advance_if('.') {
                    Token::Id(Symbol::new("..."), self.get_src_loc())
                } else {
                    return Err(self.emit_err("Expecting '.' after '..'"));
                }),
                ',' => Some(if self.advance_if('@') {
                    Token::CommaAt(self.get_src_loc())
                } else {
                    Token::Comma(self.get_src_loc())
                }),
                '#' => Some(if self.advance_if('t') {
                    Token::Bool(Bool(true), self.get_src_loc())
                } else if self.advance_if('f') {
                    Token::Bool(Bool(false), self.get_src_loc())
                } else if self.advance_if('(') {
                    Token::HashLParen(self.get_src_loc())
                } else if self.advance_if('\\') && self.look_ahead().is_some() {
                    Token::Char(Char(self.advance()), self.get_src_loc())
                } else {
                    return Err(
                        self.emit_err("Expectin 't', 'f', '(' or character literal after '#'")
                    );
                }),
                ';' => {
                    self.advance_until(&|c| c == '\n');
                    None
                }
                '0'..='9' | '-' => Some(self.parse_num()?),
                '"' => Some(self.parse_string()?),
                c if Self::is_id_initial(c) => Some(self.parse_id()?),
                c => return Err(self.emit_err(&format!("Unexpeted character: '{}'", c))),
            })
        }
        fn parse_num(&mut self) -> Result<Token, CompliationError> {
            self.advance_until(&|c| !c.is_ascii_digit());

            if self.look_ahead() == Some('.') {
                self.advance();
                self.advance_until(&|c| !c.is_ascii_digit());
            }

            Ok(Token::Num(
                Num(self.cur.parse().map_err(|_| {
                    self.emit_err(&format!("Invalid number representation: {}", self.cur))
                })?),
                self.get_src_loc(),
            ))
        }
        fn parse_id(&mut self) -> Result<Token, CompliationError> {
            self.advance_until(&|c| !Self::is_id_subsequent(c));
            Ok(Token::Id(Symbol::new(&self.cur), self.get_src_loc()))
        }
        fn parse_string(&mut self) -> Result<Token, CompliationError> {
            while let Some(c) = self.look_ahead() {
                match c {
                    '"' if !matches!(self.cur.chars().last(), Some('\\')) => break,
                    _ => (),
                }
                self.advance();
            }
            if self.look_ahead().is_none() {
                return Err(self.emit_err("Unterminated string"));
            };
            self.advance();
            Ok(Token::Str(
                Str(self.cur[1..self.cur.len() - 1].to_string()),
                self.get_src_loc(),
            ))
        }
        fn advance_until<F>(&mut self, f: &F)
        where
            F: Fn(char) -> bool,
        {
            while let Some(c) = self.look_ahead() {
                match c {
                    _ if f(c) => break,
                    _ => (),
                }
                self.advance();
            }
        }
        fn advance_if(&mut self, c: char) -> bool {
            self.it
                .next_if(|next| c == *next)
                .map(|c| {
                    self.cur.push(c);
                    true
                })
                .unwrap_or(false)
        }
        fn look_ahead(&mut self) -> Option<char> {
            self.it.peek().map(|c| c).copied()
        }
        fn advance(&mut self) -> char {
            let c = self.it.next().unwrap();
            self.cur.push(c);
            c
        }
        fn push_token(&mut self, token: Token) {
            if let Token::Str(_, _) = token {
                let num_lines = self.cur.lines().count() - 1;
                self.line += num_lines;
                if num_lines > 1 {
                    self.col = self.cur.lines().last().unwrap().len();
                } else {
                    self.col += self.cur.len();
                }
            } else {
                self.col += self.cur.len();
            }
            self.tokens.push(token);
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
        fn get_src_loc(&self) -> SourceLoc {
            SourceLoc {
                line: self.line,
                col: self.col,
                width: self.cur.len(),
            }
        }
        fn parse_blank(&mut self) {
            self.col += 1;
        }
        fn parse_newline(&mut self) {
            self.line += 1;
            self.col = 0;
        }
        fn emit_err(&self, reason: &str) -> CompliationError {
            CompliationError {
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
        sexpr::{Bool, Char, Num, Str, Symbol},
        src_loc::SourceLoc,
        token::Token,
    };

    #[test]
    fn test_tokenize_empty() {
        assert_eq!(
            tokenize("").unwrap(),
            vec![Token::EoF(SourceLoc {
                line: 0,
                col: 0,
                width: 0
            })]
        );
    }

    #[test]
    fn test_tokenize_multiline() {
        let src = "`(#())
; #
\"ab\" ; #
; #
\"\" 9.0001 0 -3 -42.00 -100 some-symbol <=? list->vector ;
2 #t #\\ (...) \"

  \"  \" 123
    456
\"";
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
                Token::Str(
                    Str("ab".to_string()),
                    SourceLoc {
                        line: 2,
                        col: 0,
                        width: 4
                    }
                ),
                Token::Str(
                    Str("".to_string()),
                    SourceLoc {
                        line: 4,
                        col: 0,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(9.0001),
                    SourceLoc {
                        line: 4,
                        col: 3,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(0.0),
                    SourceLoc {
                        line: 4,
                        col: 10,
                        width: 1
                    }
                ),
                Token::Num(
                    Num(-3.0),
                    SourceLoc {
                        line: 4,
                        col: 12,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(-42.0),
                    SourceLoc {
                        line: 4,
                        col: 15,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(-100.0),
                    SourceLoc {
                        line: 4,
                        col: 22,
                        width: 4
                    }
                ),
                Token::Id(
                    Symbol::new("some-symbol"),
                    SourceLoc {
                        line: 4,
                        col: 27,
                        width: 11
                    }
                ),
                Token::Id(
                    Symbol::new("<=?"),
                    SourceLoc {
                        line: 4,
                        col: 39,
                        width: 3
                    }
                ),
                Token::Id(
                    Symbol::new("list->vector"),
                    SourceLoc {
                        line: 4,
                        col: 43,
                        width: 12
                    }
                ),
                Token::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 5,
                        col: 0,
                        width: 1
                    }
                ),
                Token::Bool(
                    Bool(true),
                    SourceLoc {
                        line: 5,
                        col: 2,
                        width: 2
                    }
                ),
                Token::Char(
                    Char(' '),
                    SourceLoc {
                        line: 5,
                        col: 5,
                        width: 3
                    }
                ),
                Token::LParen(SourceLoc {
                    line: 5,
                    col: 8,
                    width: 1
                }),
                Token::Id(
                    Symbol::new("..."),
                    SourceLoc {
                        line: 5,
                        col: 9,
                        width: 3
                    }
                ),
                Token::RParen(SourceLoc {
                    line: 5,
                    col: 12,
                    width: 1
                }),
                Token::Str(
                    Str("\n\n  ".to_string()),
                    SourceLoc {
                        line: 5,
                        col: 14,
                        width: 6
                    }
                ),
                Token::Str(
                    Str(" 123\n    456\n".to_string()),
                    SourceLoc {
                        line: 7,
                        col: 5,
                        width: 15
                    }
                ),
                Token::EoF(SourceLoc {
                    line: 9,
                    col: 1,
                    width: 0
                })
            ]
        );
    }
}
