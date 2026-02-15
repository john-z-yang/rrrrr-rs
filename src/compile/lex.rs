use std::{iter::Peekable, str::Chars};

use crate::compile::{
    sexpr::{Bool, Char, Num, Str, Symbol},
    source_loc::SourceLoc,
};

use super::{compilation_error::CompilationError, token::Token};

pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>, CompilationError> {
    struct Lexer<'source> {
        it: Peekable<Chars<'source>>,
        cur: String,
        col: usize,
        line: usize,
        tokens: Vec<Token>,
    }

    impl Lexer<'_> {
        fn new(source: &'_ str) -> Lexer<'_> {
            Lexer {
                it: source.chars().peekable(),
                cur: String::new(),
                col: 0,
                line: 0,
                tokens: vec![],
            }
        }

        fn scan(&mut self) -> Result<Vec<Token>, CompilationError> {
            while self.look_ahead().is_some() {
                let res = self.scan_token()?;
                self.advance(res);
            }
            self.tokens.push(Token::EoF(self.get_source_loc()));
            Ok(self.tokens.clone())
        }

        fn scan_token(&mut self) -> Result<Option<Token>, CompilationError> {
            Ok(match self.consume() {
                ' ' | '\r' | '\t' | '\n' => None,
                ';' => {
                    self.consume_until(&|c| c == '\n');
                    None
                }
                '(' => Some(Token::LParen(self.get_source_loc())),
                ')' => Some(Token::RParen(self.get_source_loc())),
                '`' => Some(Token::QuasiQuote(self.get_source_loc())),
                '|' => Some(Token::Pipe(self.get_source_loc())),
                '\'' => Some(Token::Quote(self.get_source_loc())),
                '.' => Some(if !self.consume_if('.') {
                    Token::Dot(self.get_source_loc())
                } else if self.consume_if('.') {
                    Token::Id(Symbol::new("..."), self.get_source_loc())
                } else {
                    return Err(self.emit_err("Expecting '.' after '..'"));
                }),
                ',' => Some(if self.consume_if('@') {
                    Token::CommaAt(self.get_source_loc())
                } else {
                    Token::Comma(self.get_source_loc())
                }),
                '#' => Some(if self.consume_if('t') {
                    Token::Bool(Bool(true), self.get_source_loc())
                } else if self.consume_if('f') {
                    Token::Bool(Bool(false), self.get_source_loc())
                } else if self.consume_if('(') {
                    Token::HashLParen(self.get_source_loc())
                } else if self.consume_if('\\') && self.look_ahead().is_some() {
                    Token::Char(Char(self.consume()), self.get_source_loc())
                } else {
                    return Err(
                        self.emit_err("Expectin 't', 'f', '(' or character literal after '#'")
                    );
                }),
                '0'..='9' | '-' => Some(self.parse_num()?),
                '"' => Some(self.parse_string()?),
                c if Self::is_id_initial(c) => Some(self.parse_id()?),
                c => return Err(self.emit_err(&format!("Unexpeted character: '{}'", c))),
            })
        }

        fn parse_num(&mut self) -> Result<Token, CompilationError> {
            self.consume_until(&|c| !c.is_ascii_digit());

            if self.look_ahead() == Some('.') {
                self.consume();
                self.consume_until(&|c| !c.is_ascii_digit());
            }

            Ok(Token::Num(
                Num(self.cur.parse().map_err(|_| {
                    self.emit_err(&format!("Invalid number representation: {}", self.cur))
                })?),
                self.get_source_loc(),
            ))
        }

        fn parse_id(&mut self) -> Result<Token, CompilationError> {
            self.consume_until(&|c| !Self::is_id_subsequent(c));
            Ok(Token::Id(Symbol::new(&self.cur), self.get_source_loc()))
        }

        fn parse_string(&mut self) -> Result<Token, CompilationError> {
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
                return Err(self.emit_err("Unterminated string"));
            };
            self.consume();
            Ok(Token::Str(
                Str(self.cur[1..self.cur.len() - 1]
                    .replace("\\\\", "\\")
                    .replace("\\\"", "\"")),
                self.get_source_loc(),
            ))
        }

        fn consume_until<F>(&mut self, f: &F)
        where
            F: Fn(char) -> bool,
        {
            while let Some(c) = self.look_ahead() {
                match c {
                    _ if f(c) => break,
                    _ => (),
                }
                self.consume();
            }
        }

        fn consume_if(&mut self, c: char) -> bool {
            self.it
                .next_if(|next| c == *next)
                .map(|c| {
                    self.cur.push(c);
                    true
                })
                .unwrap_or(false)
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
            let num_lines = self.cur.chars().filter(|c| *c == '\n').count();
            self.line += num_lines;
            self.col += self.cur.len();
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

        fn get_source_loc(&self) -> SourceLoc {
            SourceLoc {
                line: self.line,
                idx: self.col,
                width: self.cur.len(),
            }
        }

        fn emit_err(&self, reason: &str) -> CompilationError {
            CompilationError {
                source_loc: self.get_source_loc(),
                reason: reason.to_owned(),
            }
        }
    }

    Lexer::new(source).scan()
}

#[cfg(test)]
mod tests {
    use crate::compile::{
        compilation_error::CompilationError,
        lex::tokenize,
        sexpr::{Bool, Char, Num, Str, Symbol},
        source_loc::SourceLoc,
        token::Token,
    };

    #[test]
    fn test_tokenize_empty() {
        assert_eq!(
            tokenize("").unwrap(),
            vec![Token::EoF(SourceLoc {
                line: 0,
                idx: 0,
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
                    idx: 0,
                    width: 1
                }),
                Token::LParen(SourceLoc {
                    line: 0,
                    idx: 1,
                    width: 1
                }),
                Token::HashLParen(SourceLoc {
                    line: 0,
                    idx: 2,
                    width: 2
                }),
                Token::RParen(SourceLoc {
                    line: 0,
                    idx: 4,
                    width: 1
                }),
                Token::RParen(SourceLoc {
                    line: 0,
                    idx: 5,
                    width: 1
                }),
                Token::Str(
                    Str("ab".to_string()),
                    SourceLoc {
                        line: 2,
                        idx: 11,
                        width: 4
                    }
                ),
                Token::Str(
                    Str("".to_string()),
                    SourceLoc {
                        line: 4,
                        idx: 24,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(9.0001),
                    SourceLoc {
                        line: 4,
                        idx: 27,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(0.0),
                    SourceLoc {
                        line: 4,
                        idx: 34,
                        width: 1
                    }
                ),
                Token::Num(
                    Num(-3.0),
                    SourceLoc {
                        line: 4,
                        idx: 36,
                        width: 2
                    }
                ),
                Token::Num(
                    Num(-42.0),
                    SourceLoc {
                        line: 4,
                        idx: 39,
                        width: 6
                    }
                ),
                Token::Num(
                    Num(-100.0),
                    SourceLoc {
                        line: 4,
                        idx: 46,
                        width: 4
                    }
                ),
                Token::Id(
                    Symbol::new("some-symbol"),
                    SourceLoc {
                        line: 4,
                        idx: 51,
                        width: 11
                    }
                ),
                Token::Id(
                    Symbol::new("<=?"),
                    SourceLoc {
                        line: 4,
                        idx: 63,
                        width: 3
                    }
                ),
                Token::Id(
                    Symbol::new("list->vector"),
                    SourceLoc {
                        line: 4,
                        idx: 67,
                        width: 12
                    }
                ),
                Token::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 5,
                        idx: 82,
                        width: 1
                    }
                ),
                Token::Bool(
                    Bool(true),
                    SourceLoc {
                        line: 5,
                        idx: 84,
                        width: 2
                    }
                ),
                Token::Char(
                    Char(' '),
                    SourceLoc {
                        line: 5,
                        idx: 87,
                        width: 3
                    }
                ),
                Token::LParen(SourceLoc {
                    line: 5,
                    idx: 90,
                    width: 1
                }),
                Token::Id(
                    Symbol::new("..."),
                    SourceLoc {
                        line: 5,
                        idx: 91,
                        width: 3
                    }
                ),
                Token::RParen(SourceLoc {
                    line: 5,
                    idx: 94,
                    width: 1
                }),
                Token::Str(
                    Str("\n\n  ".to_string()),
                    SourceLoc {
                        line: 5,
                        idx: 96,
                        width: 6
                    }
                ),
                Token::Str(
                    Str(" 123\n    456\n".to_string()),
                    SourceLoc {
                        line: 7,
                        idx: 104,
                        width: 15
                    }
                ),
                Token::EoF(SourceLoc {
                    line: 9,
                    idx: 119,
                    width: 0
                })
            ]
        );
    }

    #[test]
    fn test_tokenize_escape_double_quote() {
        let result = tokenize(
            r#"

        "\""

        "#,
        )
        .unwrap();
        assert_eq!(
            result[0],
            Token::Str(
                Str("\"".to_string()),
                SourceLoc {
                    line: 2,
                    idx: 10,
                    width: 4
                }
            )
        );
    }

    #[test]
    fn test_tokenize_escape_slashes() {
        let result = tokenize(
            r#"

        "\\"

        "#,
        )
        .unwrap();
        assert_eq!(
            result[0],
            Token::Str(
                Str("\\".to_string()),
                SourceLoc {
                    line: 2,
                    idx: 10,
                    width: 4
                }
            )
        );
    }

    #[test]
    fn test_tokenize_escape_multiple_slashes() {
        let result = tokenize(
            r#"

        "\\\""

        "#,
        )
        .unwrap();
        assert_eq!(
            result[0],
            Token::Str(
                Str("\\\"".to_string()),
                SourceLoc {
                    line: 2,
                    idx: 10,
                    width: 6
                }
            )
        );
    }

    #[test]
    fn test_tokenize_escape_multiple_slashes_and_double_quote() {
        let result = tokenize(
            r#"

        "\\\"\\\"\\"

        "#,
        )
        .unwrap();
        assert_eq!(
            result[0],
            Token::Str(
                Str("\\\"\\\"\\".to_string()),
                SourceLoc {
                    line: 2,
                    idx: 10,
                    width: 12
                }
            )
        );
    }

    #[test]
    fn test_tokenize_unterminated_single_line_string() {
        let res = tokenize("\"");
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    source_loc: SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 1
                    },
                    reason: _
                })
            ),
            "{:?}",
            res
        );

        let res = tokenize("1   \"");
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    source_loc: SourceLoc {
                        line: 0,
                        idx: 4,
                        width: 1
                    },
                    reason: _
                })
            ),
            "{:?}",
            res
        );
    }

    #[test]
    fn test_tokenize_unterminated_multiline_string() {
        let res = tokenize("\"\n123\n456\n\" \"\n123\n456\n");
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    source_loc: SourceLoc {
                        line: 3,
                        idx: 12,
                        width: 10
                    },
                    reason: _
                })
            ),
            "{:?}",
            res
        );
    }
}
