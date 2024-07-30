use std::{iter::Peekable, slice::Iter};

use super::{compliation_error::CompliationError, sexpr::SExpr, token::Token};
use crate::compile::{
    sexpr::{Id, Vector},
    source_loc::SourceLoc,
};

pub fn parse(tokens: &[Token]) -> Result<SExpr, CompliationError> {
    struct Parser<'tokens> {
        it: Peekable<Iter<'tokens, Token>>,
        cur: &'tokens Token,
    }

    impl Parser<'_> {
        fn new(tokens: &[Token]) -> Parser {
            assert_ne!(tokens.len(), 0, "Token stream must have at least 1 token");
            Parser {
                it: tokens.iter().peekable(),
                cur: &tokens[0],
            }
        }

        fn parse(&mut self) -> Result<SExpr, CompliationError> {
            let res = self.parse_datum()?;
            match self.look_ahead() {
                Some(Token::EoF(_)) => Ok(res),
                Some(token) => Err(self.emit_err("Expecting EoF", token)),
                None => unreachable!("parse is expecting token stream to end with the EoF token"),
            }
        }

        fn parse_datum(&mut self) -> Result<SExpr, CompliationError> {
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
                Some(token) => Err(self.emit_err("Expecting a datum", token)),
                _ => unreachable!("parse_datum is expecting at least 1 token to look ahead"),
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
                "parse_atom is expecting a token that represents an atomic value",
            );

            match self.consume() {
                Token::Id(symbol, source_loc) => SExpr::Id(Id::new(&symbol.0, []), *source_loc),
                Token::Bool(bool, source_loc) => SExpr::Bool(bool.clone(), *source_loc),
                Token::Num(num, source_loc) => SExpr::Num(num.clone(), *source_loc),
                Token::Char(char, source_loc) => SExpr::Char(char.clone(), *source_loc),
                Token::Str(string, source_loc) => SExpr::Str(string.clone(), *source_loc),
                _ => unreachable!("parse_atom is only expecting tokens for atomic values"),
            }
        }

        fn parse_compound(&mut self) -> Result<SExpr, CompliationError> {
            match self.look_ahead() {
                Some(Token::HashLParen(_)) => self.parse_vector(),
                _ => self.parse_list(),
            }
        }

        fn parse_list(&mut self) -> Result<SExpr, CompliationError> {
            if matches!(
                self.look_ahead(),
                Some(Token::Quote(_) | Token::QuasiQuote(_) | Token::Comma(_) | Token::CommaAt(_))
            ) {
                return self.parse_abbreviation();
            }

            assert!(
                matches!(self.look_ahead(), Some(Token::LParen(_))),
                "parse_list is expecting a '(' token",
            );

            let start = self.consume().get_source_loc();
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
                Some(Token::Dot(_)) => {
                    elements.push(self.parse_dot_notation()?);
                    assert!(
                        matches!(self.look_ahead(), Some(Token::RParen(_))),
                        "parse_list is expecting a ')' token after parse_dot_notation returns",
                    );
                    Ok(Self::make_improper_list(
                        &elements,
                        start,
                        self.consume().get_source_loc(),
                    ))
                }
                Some(Token::RParen(end)) => {
                    self.consume();
                    Ok(Self::make_list(&elements, start, end))
                }
                Some(token) => Err(self.emit_err("Expected ')' to close '('", token)),
                None => {
                    unreachable!("parse_datum is expecting token stream to end with the EoF token")
                }
            }
        }

        fn parse_dot_notation(&mut self) -> Result<SExpr, CompliationError> {
            assert!(
                matches!(self.look_ahead(), Some(Token::Dot(_))),
                "parse_dot_notation is expecting the '.' token"
            );
            self.consume();
            let tail = self.parse_datum()?;
            match self.look_ahead() {
                Some(Token::RParen(_)) => Ok(tail),
                Some(token) => Err(self.emit_err("Expected ')' after dotted datum", token)),
                None => {
                    unreachable!("parse_list is expecting at least 1 token to look ahead")
                }
            }
        }

        fn parse_abbreviation(&mut self) -> Result<SExpr, CompliationError> {
            let elements = [self.parse_prefix(), self.parse_datum()?];
            Ok(Self::make_list(
                &elements,
                elements[0].get_source_loc(),
                elements[1].get_source_loc(),
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
                "parse_prefix is expecting either '(' or an abbreviation prefix",
            );

            match self.consume() {
                Token::Quote(source_loc) => SExpr::Id(Id::new("quote", []), *source_loc),
                Token::QuasiQuote(source_loc) => SExpr::Id(Id::new("quasiquote", []), *source_loc),
                Token::Comma(source_loc) => SExpr::Id(Id::new("unquote", []), *source_loc),
                Token::CommaAt(source_loc) => {
                    SExpr::Id(Id::new("unquote-splicing", []), *source_loc)
                }
                _ => unreachable!(
                    "parse_abbreviation is only expecting tokens for abbreviated prefix"
                ),
            }
        }

        fn parse_vector(&mut self) -> Result<SExpr, CompliationError> {
            assert!(
                matches!(self.look_ahead(), Some(Token::HashLParen(_))),
                "parse_vector is expecting the '#(' token"
            );
            let start = self.consume().get_source_loc();
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
                None => unreachable!("parse_datum is expecting at least 1 token to look ahead"),
            }
        }

        fn make_list(elements: &[SExpr], start: SourceLoc, end: SourceLoc) -> SExpr {
            let mut res = SExpr::Nil(end);
            for element in elements.iter().rev() {
                res = SExpr::cons(element.clone(), res);
            }
            res.update_source_loc(start.combine(res.get_source_loc()))
        }

        fn make_improper_list(slice: &[SExpr], start: SourceLoc, end: SourceLoc) -> SExpr {
            assert!(
                slice.len() >= 2,
                "improper list has to have more than 2 element"
            );
            let mut iter = slice.iter().rev();
            let cdr = iter.next().unwrap().clone();
            let car = iter.next().unwrap().clone();
            let mut res = SExpr::cons(car, cdr);
            res = res.update_source_loc(res.get_source_loc().combine(end));
            for element in iter {
                res = SExpr::cons(element.clone(), res);
            }
            res.update_source_loc(start.combine(res.get_source_loc()))
        }

        fn look_ahead(&mut self) -> Option<Token> {
            self.it.peek().copied().cloned()
        }

        fn consume(&mut self) -> &Token {
            let token = self.it.next().unwrap();
            self.cur = token;
            token
        }

        fn emit_err(&self, reason: &str, token: Token) -> CompliationError {
            CompliationError {
                source_loc: token.get_source_loc(),
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
        source_loc::SourceLoc,
    };

    use super::*;

    #[test]
    fn parse_nil() {
        let src = "(    )";
        let list = SExpr::Nil(SourceLoc {
            line: 0,
            idx: 0,
            width: 6,
        });

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_list_of_symbol() {
        let src = "(abc)";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("abc", []),
                    SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 3,
                    },
                )),
                cdr: Box::new(SExpr::Nil(SourceLoc {
                    line: 0,
                    idx: 4,
                    width: 1,
                })),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 5,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_quote_datum() {
        let src = "'()";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("quote", []),
                    SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 1,
                            width: 2,
                        })),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 1,
                            width: 2,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 2,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 3,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_unquote_splice_datum() {
        let src = ",@()";

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 2,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 2,
                            width: 2,
                        })),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 2,
                            width: 2,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 2,
                        width: 2,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 4,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_unquote_splice_unquote_splice_datum() {
        let src = ",@,@()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    SourceLoc {
                        line: 0,
                        idx: 2,
                        width: 2,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 4,
                            width: 2,
                        })),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 4,
                            width: 2,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 4,
                        width: 2,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 2,
                width: 4,
            },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 2,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 2,
                            width: 4,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 2,
                        width: 4,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 6,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_unquote_splice_quote_datum() {
        let src = ",@'()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("quote", []),
                    SourceLoc {
                        line: 0,
                        idx: 2,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 3,
                            width: 2,
                        })),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 3,
                            width: 2,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 3,
                        width: 2,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 2,
                width: 3,
            },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 2,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 2,
                            width: 3,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 2,
                        width: 3,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 5,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_quote_unquote_splice_datum() {
        let src = "',@()";

        let inner_list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("unquote-splicing", []),
                    SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 2,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 3,
                            width: 2,
                        })),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 3,
                            width: 2,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 3,
                        width: 2,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 1,
                width: 4,
            },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Id(
                    Id::new("quote", []),
                    SourceLoc {
                        line: 0,
                        idx: 0,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 0,
                            idx: 1,
                            width: 4,
                        })),
                    },
                    SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 4,
                    },
                )),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 5,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_simple_list() {
        let src = "(1.000)";
        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 5,
                    },
                )),
                cdr: Box::new(SExpr::Nil(SourceLoc {
                    line: 0,
                    idx: 6,
                    width: 1,
                })),
            },
            SourceLoc {
                line: 0,
                idx: 0,
                width: 7,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
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
                car: Box::new(SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 3,
                        idx: 10,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Num(
                            Num(2.0),
                            SourceLoc {
                                line: 4,
                                idx: 15,
                                width: 3,
                            },
                        )),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 5,
                            idx: 21,
                            width: 1,
                        })),
                    },
                    SourceLoc {
                        line: 4,
                        idx: 15,
                        width: 7,
                    },
                )),
            },
            SourceLoc {
                line: 2,
                idx: 5,
                width: 17,
            },
        );
        let list = SExpr::Cons(
            Cons {
                car: Box::new(inner_list),
                cdr: Box::new(SExpr::Nil(SourceLoc {
                    line: 6,
                    idx: 23,
                    width: 1,
                })),
            },
            SourceLoc {
                line: 1,
                idx: 1,
                width: 23,
            },
        );
        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
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
                SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 3,
                        idx: 12,
                        width: 1,
                    },
                ),
                SExpr::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 4,
                        idx: 17,
                        width: 3,
                    },
                ),
            ]),
            SourceLoc {
                line: 2,
                idx: 6,
                width: 18,
            },
        );

        let list = SExpr::Vector(
            Vector(vec![inner_list]),
            SourceLoc {
                line: 1,
                idx: 1,
                width: 25,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
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
                SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 3,
                        idx: 11,
                        width: 1,
                    },
                ),
                SExpr::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 4,
                        idx: 16,
                        width: 3,
                    },
                ),
            ]),
            SourceLoc {
                line: 2,
                idx: 5,
                width: 18,
            },
        );

        let inner_list_1 = SExpr::Vector(
            Vector(vec![
                SExpr::Num(
                    Num(3.0),
                    SourceLoc {
                        line: 7,
                        idx: 32,
                        width: 1,
                    },
                ),
                SExpr::Num(
                    Num(4.0),
                    SourceLoc {
                        line: 8,
                        idx: 37,
                        width: 3,
                    },
                ),
            ]),
            SourceLoc {
                line: 6,
                idx: 26,
                width: 18,
            },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(inner_list_0),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(inner_list_1),
                        cdr: Box::new(SExpr::Nil(SourceLoc {
                            line: 10,
                            idx: 45,
                            width: 1,
                        })),
                    },
                    SourceLoc {
                        line: 6,
                        idx: 26,
                        width: 20,
                    },
                )),
            },
            SourceLoc {
                line: 1,
                idx: 1,
                width: 45,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
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
                car: Box::new(SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 2,
                        idx: 11,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 4,
                        idx: 31,
                        width: 3,
                    },
                )),
            },
            SourceLoc {
                line: 1,
                idx: 1,
                width: 35,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), pair)
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
                car: Box::new(SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 3,
                        idx: 21,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 5,
                        idx: 41,
                        width: 3,
                    },
                )),
            },
            SourceLoc {
                line: 3,
                idx: 21,
                width: 25,
            },
        );

        let list = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(
                    Num(0.0),
                    SourceLoc {
                        line: 2,
                        idx: 11,
                        width: 1,
                    },
                )),
                cdr: Box::new(pair),
            },
            SourceLoc {
                line: 1,
                idx: 1,
                width: 45,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), list)
    }

    #[test]
    fn test_parse_nested_dotted_pairs() {
        let src = "
        ((1 . 2) .
         (3 . 4))";
        let inner_pair_0 = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(
                    Num(1.0),
                    SourceLoc {
                        line: 1,
                        idx: 11,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Num(
                    Num(2.0),
                    SourceLoc {
                        line: 1,
                        idx: 15,
                        width: 1,
                    },
                )),
            },
            SourceLoc {
                line: 1,
                idx: 10,
                width: 7,
            },
        );
        let inner_pair_1 = SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(
                    Num(3.0),
                    SourceLoc {
                        line: 2,
                        idx: 30,
                        width: 1,
                    },
                )),
                cdr: Box::new(SExpr::Num(
                    Num(4.0),
                    SourceLoc {
                        line: 2,
                        idx: 34,
                        width: 1,
                    },
                )),
            },
            SourceLoc {
                line: 2,
                idx: 29,
                width: 7,
            },
        );
        let outer_pair = SExpr::Cons(
            Cons {
                car: Box::new(inner_pair_0),
                cdr: Box::new(inner_pair_1),
            },
            SourceLoc {
                line: 1,
                idx: 9,
                width: 28,
            },
        );

        assert_eq!(parse(&tokenize(src).unwrap()).unwrap(), outer_pair)
    }

    #[test]
    fn test_parse_unclosed_nil() {
        let res = parse(&tokenize("(").unwrap());
        assert!(
            matches!(
                res,
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 0,
                        idx: 1,
                        width: 0,
                    },
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
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 1,
                        idx: 7,
                        width: 0,
                    },
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
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 1,
                        idx: 8,
                        width: 0,
                    },
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
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 1,
                        idx: 8,
                        width: 0,
                    },
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
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 0,
                        idx: 8,
                        width: 1,
                    },
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
                Err(CompliationError {
                    source_loc: SourceLoc {
                        line: 0,
                        idx: 8,
                        width: 1,
                    },
                    reason: _,
                })
            ),
            "{:?}",
            res
        )
    }
}
