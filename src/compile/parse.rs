use std::{iter::Peekable, slice::Iter};

use super::{compilation_error::CompilationError, sexpr::SExpr, token::Token};
use crate::compile::{
    compilation_error::Result,
    sexpr::{Id, Vector},
    span::Span,
};

pub(crate) fn parse(tokens: &[Token]) -> Result<SExpr> {
    struct Parser<'tokens> {
        it: Peekable<Iter<'tokens, Token>>,
        cur: &'tokens Token,
    }

    impl Parser<'_> {
        fn new(tokens: &'_ [Token]) -> Parser<'_> {
            assert_ne!(tokens.len(), 0, "Token stream must have at least 1 token");
            Parser {
                it: tokens.iter().peekable(),
                cur: &tokens[0],
            }
        }

        fn parse(&mut self) -> Result<SExpr> {
            let res = self.parse_datum()?;
            match self.look_ahead() {
                Some(Token::EoF(_)) => Ok(res),
                Some(token) => Err(self.emit_err("Expected end of input", token)),
                None => unreachable!("parse expected token stream to end with the EoF token"),
            }
        }

        fn parse_datum(&mut self) -> Result<SExpr> {
            match self.look_ahead() {
                Some(
                    Token::Id(_, _)
                    | Token::Bool(_, _)
                    | Token::Num(_, _)
                    | Token::Char(_, _)
                    | Token::Str(_, _),
                ) => Ok(self.parse_atom()),
                Some(
                    Token::LParen(_)
                    | Token::HashLParen(_)
                    | Token::Quote(_)
                    | Token::QuasiQuote(_)
                    | Token::Comma(_)
                    | Token::CommaAt(_),
                ) => self.parse_compound(),
                Some(token) => Err(self.emit_err("Expected a datum", token)),
                _ => unreachable!("parse_datum expected at least 1 token to look ahead"),
            }
        }

        fn parse_atom(&mut self) -> SExpr {
            assert!(
                matches!(
                    self.look_ahead(),
                    Some(
                        Token::Id(..)
                            | Token::Bool(..)
                            | Token::Num(..)
                            | Token::Char(..)
                            | Token::Str(..)
                    )
                ),
                "parse_atom expected a token that represents an atomic value",
            );

            match self.consume() {
                Token::Id(symbol, span) => SExpr::Id(Id::new(&symbol.0, []), *span),
                Token::Bool(bool, span) => SExpr::Bool(bool.clone(), *span),
                Token::Num(num, span) => SExpr::Num(num.clone(), *span),
                Token::Char(char, span) => SExpr::Char(char.clone(), *span),
                Token::Str(string, span) => SExpr::Str(string.clone(), *span),
                _ => unreachable!("parse_atom expected only tokens for atomic values"),
            }
        }

        fn parse_compound(&mut self) -> Result<SExpr> {
            match self.look_ahead() {
                Some(Token::HashLParen(_)) => self.parse_vector(),
                _ => self.parse_list(),
            }
        }

        fn parse_list(&mut self) -> Result<SExpr> {
            if matches!(
                self.look_ahead(),
                Some(Token::Quote(_) | Token::QuasiQuote(_) | Token::Comma(_) | Token::CommaAt(_))
            ) {
                return self.parse_abbreviation();
            }

            assert!(
                matches!(self.look_ahead(), Some(Token::LParen(_))),
                "parse_list expected a '(' token",
            );

            let start = self.consume().get_span();
            let mut elements: Vec<SExpr> = vec![];
            while let Some(t) = self.look_ahead() {
                if matches!(t, Token::RParen(_))
                    || matches!(t, Token::Dot(_))
                    || matches!(t, Token::EoF(_))
                {
                    break;
                }
                elements.push(self.parse_datum()?);
            }
            match self.look_ahead() {
                Some(dot @ Token::Dot(_)) => {
                    if elements.is_empty() {
                        return Err(self.emit_err("Expected a datum after '('", dot));
                    }
                    elements.push(self.parse_dot_notation()?);
                    assert!(
                        matches!(self.look_ahead(), Some(Token::RParen(_))),
                        "parse_list expected a ')' token after parse_dot_notation returns",
                    );
                    Ok(Self::make_improper_list(
                        &elements,
                        start,
                        self.consume().get_span(),
                    ))
                }
                Some(Token::RParen(end)) => {
                    self.consume();
                    Ok(Self::make_list(&elements, start, end))
                }
                Some(token) => Err(self.emit_err("Expected ')' to close '('", token)),
                None => {
                    unreachable!("parse_datum expected token stream to end with the EoF token")
                }
            }
        }

        fn parse_dot_notation(&mut self) -> Result<SExpr> {
            assert!(
                matches!(self.look_ahead(), Some(Token::Dot(_))),
                "parse_dot_notation expected the '.' token"
            );
            self.consume();
            let tail = self.parse_datum()?;
            match self.look_ahead() {
                Some(Token::RParen(_)) => Ok(tail),
                Some(token) => Err(self.emit_err("Expected ')' after dotted datum", token)),
                None => {
                    unreachable!("parse_list expected at least 1 token to look ahead")
                }
            }
        }

        fn parse_abbreviation(&mut self) -> Result<SExpr> {
            let elements = [self.parse_prefix(), self.parse_datum()?];
            Ok(Self::make_list(
                &elements,
                elements[0].get_span(),
                elements[1].get_span(),
            ))
        }

        fn parse_prefix(&mut self) -> SExpr {
            assert!(
                matches!(
                    self.look_ahead(),
                    Some(
                        Token::Quote(_)
                            | Token::QuasiQuote(_)
                            | Token::Comma(_)
                            | Token::CommaAt(_)
                    )
                ),
                "parse_prefix expected either '(' or an abbreviation prefix",
            );

            match self.consume() {
                Token::Quote(span) => SExpr::Id(Id::new("quote", []), *span),
                Token::QuasiQuote(span) => SExpr::Id(Id::new("quasiquote", []), *span),
                Token::Comma(span) => SExpr::Id(Id::new("unquote", []), *span),
                Token::CommaAt(span) => SExpr::Id(Id::new("unquote-splicing", []), *span),
                _ => unreachable!("parse_abbreviation expected only tokens for abbreviated prefix"),
            }
        }

        fn parse_vector(&mut self) -> Result<SExpr> {
            assert!(
                matches!(self.look_ahead(), Some(Token::HashLParen(_))),
                "parse_vector expected the '#(' token"
            );
            let start = self.consume().get_span();
            let mut elements: Vec<SExpr> = vec![];
            while let Some(t) = self.look_ahead() {
                if matches!(t, Token::RParen(_)) || matches!(t, Token::EoF(_)) {
                    break;
                }
                elements.push(self.parse_datum()?);
            }
            match self.look_ahead() {
                Some(Token::RParen(end)) => {
                    self.consume();
                    Ok(SExpr::Vector(Vector(elements), start.combine(end)))
                }
                Some(token) => Err(self.emit_err("Expected ')' to close '#('", token)),
                None => unreachable!("parse_datum expected at least 1 token to look ahead"),
            }
        }

        fn make_list(elements: &[SExpr], start: Span, end: Span) -> SExpr {
            let mut res = SExpr::Nil(end);
            for element in elements.iter().rev() {
                res = SExpr::cons(element.clone(), res);
            }
            res.update_span(start.combine(res.get_span()))
        }

        fn make_improper_list(slice: &[SExpr], start: Span, end: Span) -> SExpr {
            assert!(
                slice.len() >= 2,
                "improper list has to have more than 2 element"
            );
            let mut iter = slice.iter().rev();
            let cdr = iter.next().unwrap().clone();
            let car = iter.next().unwrap().clone();
            let mut res = SExpr::cons(car, cdr);
            res = res.update_span(res.get_span().combine(end));
            for element in iter {
                res = SExpr::cons(element.clone(), res);
            }
            res.update_span(start.combine(res.get_span()))
        }

        fn look_ahead(&mut self) -> Option<Token> {
            self.it.peek().copied().cloned()
        }

        fn consume(&mut self) -> &Token {
            let token = self.it.next().unwrap();
            self.cur = token;
            token
        }

        fn emit_err(&self, reason: &str, token: Token) -> CompilationError {
            CompilationError {
                span: token.get_span(),
                reason: format!("{}, but got: {}", reason.to_owned(), token),
            }
        }
    }

    Parser::new(tokens).parse()
}

#[cfg(test)]
mod tests {
    use crate::compile::{
        lex::tokenize,
        sexpr::{Cons, Num},
        span::Span,
    };

    use super::*;

    #[test]
    fn parse_nil() {
        let src = "(    )";
        let list = SExpr::Nil(Span { lo: 0, hi: 6 });

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_list_of_symbol() {
        let src = "(abc)";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(Id::new("abc", []), Span { lo: 1, hi: 4 })),
                cdr: Box::new(SExpr::Nil(Span { lo: 4, hi: 5 })),
            },
            Span { lo: 0, hi: 5 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_quote_datum() {
        let src = "'()";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(Id::new("quote", []), Span { lo: 0, hi: 1 })),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(Span { lo: 1, hi: 3 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 1, hi: 3 })),
                    },
                    Span { lo: 1, hi: 3 },
                )),
            },
            Span { lo: 0, hi: 3 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_unquote_splice_datum() {
        let src = ",@()";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    Span { lo: 0, hi: 2 },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(Span { lo: 2, hi: 4 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 2, hi: 4 })),
                    },
                    Span { lo: 2, hi: 4 },
                )),
            },
            Span { lo: 0, hi: 4 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_unquote_splice_unquote_splice_datum() {
        let src = ",@,@()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    Span { lo: 2, hi: 4 },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(Span { lo: 4, hi: 6 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 4, hi: 6 })),
                    },
                    Span { lo: 4, hi: 6 },
                )),
            },
            Span { lo: 2, hi: 6 },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    Span { lo: 0, hi: 2 },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(Span { lo: 2, hi: 6 })),
                    },
                    Span { lo: 2, hi: 6 },
                )),
            },
            Span { lo: 0, hi: 6 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_unquote_splice_quote_datum() {
        let src = ",@'()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(Id::new("quote", []), Span { lo: 2, hi: 3 })),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(Span { lo: 3, hi: 5 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 3, hi: 5 })),
                    },
                    Span { lo: 3, hi: 5 },
                )),
            },
            Span { lo: 2, hi: 5 },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    Span { lo: 0, hi: 2 },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(Span { lo: 2, hi: 5 })),
                    },
                    Span { lo: 2, hi: 5 },
                )),
            },
            Span { lo: 0, hi: 5 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_quote_unquote_splice_datum() {
        let src = "',@()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    Span { lo: 1, hi: 3 },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(Span { lo: 3, hi: 5 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 3, hi: 5 })),
                    },
                    Span { lo: 3, hi: 5 },
                )),
            },
            Span { lo: 1, hi: 5 },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(Id::new("quote", []), Span { lo: 0, hi: 1 })),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(Span { lo: 1, hi: 5 })),
                    },
                    Span { lo: 1, hi: 5 },
                )),
            },
            Span { lo: 0, hi: 5 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_simple_list() {
        let src = "(1.000)";
        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 1, hi: 6 })),
                cdr: Box::new(SExpr::Nil(Span { lo: 6, hi: 7 })),
            },
            Span { lo: 0, hi: 7 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_nested_list() {
        let src = "
(
  (
   1
   2.0
  )
)
";
        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 10, hi: 11 })),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Num(Num(2.0), Span { lo: 15, hi: 18 })),
                        cdr: Box::new(SExpr::Nil(Span { lo: 21, hi: 22 })),
                    },
                    Span { lo: 15, hi: 22 },
                )),
            },
            Span { lo: 5, hi: 22 },
        );
        let list = SExpr::Cons(
            Cons {
                car: Box::new(inner_list),
                cdr: Box::new(SExpr::Nil(Span { lo: 23, hi: 24 })),
            },
            Span { lo: 1, hi: 24 },
        );
        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_nested_vector() {
        let src = "
#(
  #(
   1
   2.0
  )
)
";
        let inner_list = SExpr::Vector(
            Vector(vec![
                SExpr::Num(Num(1.0), Span { lo: 12, hi: 13 }),
                SExpr::Num(Num(2.0), Span { lo: 17, hi: 20 }),
            ]),
            Span { lo: 6, hi: 24 },
        );

        let list = SExpr::Vector(Vector(vec![inner_list]), Span { lo: 1, hi: 26 });

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_vector_in_list() {
        let src = "
(
  #(
   1
   2.0
  )
  #(
   3
   4.0
  )
)
";
        let inner_list_0 = SExpr::Vector(
            Vector(vec![
                SExpr::Num(Num(1.0), Span { lo: 11, hi: 12 }),
                SExpr::Num(Num(2.0), Span { lo: 16, hi: 19 }),
            ]),
            Span { lo: 5, hi: 23 },
        );

        let inner_list_1 = SExpr::Vector(
            Vector(vec![
                SExpr::Num(Num(3.0), Span { lo: 32, hi: 33 }),
                SExpr::Num(Num(4.0), Span { lo: 37, hi: 40 }),
            ]),
            Span { lo: 26, hi: 44 },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(inner_list_0),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list_1),
                        cdr: Box::new(SExpr::Nil(Span { lo: 45, hi: 46 })),
                    },
                    Span { lo: 26, hi: 46 },
                )),
            },
            Span { lo: 1, hi: 46 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_dotted_pair() {
        let src = "
(
        1
        .
        2.0
)
";
        let pair = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 11, hi: 12 })),
                cdr: Box::new(SExpr::Num(Num(2.0), Span { lo: 31, hi: 34 })),
            },
            Span { lo: 1, hi: 36 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&pair));
    }

    #[test]
    fn test_parse_improper_list() {
        let src = "
(
        0
        1
        .
        2.0
)
";
        let pair = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 21, hi: 22 })),
                cdr: Box::new(SExpr::Num(Num(2.0), Span { lo: 41, hi: 44 })),
            },
            Span { lo: 21, hi: 46 },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(0.0), Span { lo: 11, hi: 12 })),
                cdr: Box::new(pair),
            },
            Span { lo: 1, hi: 46 },
        );

        assert!(parse(&tokenize(src).unwrap()).unwrap().is_idential(&list));
    }

    #[test]
    fn test_parse_nested_dotted_pairs() {
        let src = "
        ((1 . 2) .
         (3 . 4))";
        let inner_pair_0 = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 11, hi: 12 })),
                cdr: Box::new(SExpr::Num(Num(2.0), Span { lo: 15, hi: 16 })),
            },
            Span { lo: 10, hi: 17 },
        );
        let inner_pair_1 = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(3.0), Span { lo: 30, hi: 31 })),
                cdr: Box::new(SExpr::Num(Num(4.0), Span { lo: 34, hi: 35 })),
            },
            Span { lo: 29, hi: 36 },
        );
        let outer_pair = SExpr::Cons(
            Cons {
                car: Box::new(inner_pair_0),
                cdr: Box::new(inner_pair_1),
            },
            Span { lo: 9, hi: 37 },
        );

        assert!(
            parse(&tokenize(src).unwrap())
                .unwrap()
                .is_idential(&outer_pair)
        );
    }

    #[test]
    fn test_parse_unclosed_nil() {
        let res = parse(&tokenize("(").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 1, hi: 1 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_unclosed_list() {
        let res = parse(&tokenize("( 1 2 \n").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 7, hi: 7 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_unclosed_vector() {
        let res = parse(&tokenize("#( 1 2 \n").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 8, hi: 8 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_unclosed_dotted_pair() {
        let res = parse(&tokenize("( 1 . 2\n").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 8, hi: 8 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_dotted_pair_with_extra_dot() {
        let res = parse(&tokenize("( 1 . 2 .").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 8, hi: 9 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_dotted_pair_with_extra_element() {
        let res = parse(&tokenize("( 1 . 2 . 3 )").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 8, hi: 9 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_dotted_pair_without_head_datum() {
        let res = parse(&tokenize("( . 1 )").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 2, hi: 3 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    fn test_parse_dotted_pair_without_head_datum_and_tail() {
        let res = parse(&tokenize("( . )").unwrap());
        assert!(
            matches!(
                res,
                Err(CompilationError {
                    span: Span { lo: 2, hi: 3 },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }

    #[test]
    #[should_panic(expected = "Token stream must have at least 1 token")]
    fn test_parse_empty_token_stream_panics_as_internal_error() {
        let _ = parse(&[]);
    }

    #[test]
    #[should_panic(expected = "parse expected token stream to end with the EoF token")]
    fn test_parse_missing_eof_panics_as_internal_error() {
        let tokens = vec![
            Token::LParen(Span { lo: 0, hi: 1 }),
            Token::RParen(Span { lo: 1, hi: 2 }),
        ];
        let _ = parse(&tokens);
    }

    #[test]
    #[should_panic(expected = "parse_datum expected token stream to end with the EoF token")]
    fn test_parse_unclosed_list_without_eof_panics_as_internal_error() {
        let tokens = vec![Token::LParen(Span { lo: 0, hi: 1 })];
        let _ = parse(&tokens);
    }
}
