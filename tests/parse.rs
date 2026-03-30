use rrrrr_rs::compile::{
    compilation_error::CompilationError,
    ident::Symbol,
    read::{lex::tokenize, parse::parse, token::Token},
    sexpr::{Cons, Num, SExpr, Vector},
    span::Span,
};

#[test]
fn test_parse_nil() {
    let src = "(    )";
    let list = SExpr::Nil(Span { lo: 0, hi: 6 });

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_list_of_symbol() {
    let src = "(abc)";

    let list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(Symbol::new("abc"), Span { lo: 1, hi: 4 })),
            cdr: Box::new(SExpr::Nil(Span { lo: 4, hi: 5 })),
        },
        Span { lo: 0, hi: 5 },
    );

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_quote_datum() {
    let src = "'()";

    let list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(Symbol::new("quote"), Span { lo: 0, hi: 1 })),
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_unquote_splice_datum() {
    let src = ",@()";

    let list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(
                Symbol::new("unquote-splicing"),
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_unquote_splice_unquote_splice_datum() {
    let src = ",@,@()";

    let inner_list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(
                Symbol::new("unquote-splicing"),
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
            car: Box::new(SExpr::Var(
                Symbol::new("unquote-splicing"),
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_unquote_splice_quote_datum() {
    let src = ",@'()";

    let inner_list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(Symbol::new("quote"), Span { lo: 2, hi: 3 })),
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
            car: Box::new(SExpr::Var(
                Symbol::new("unquote-splicing"),
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
}

#[test]
fn test_parse_quote_unquote_splice_datum() {
    let src = "',@()";

    let inner_list = SExpr::Cons(
        Cons {
            car: Box::new(SExpr::Var(
                Symbol::new("unquote-splicing"),
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
            car: Box::new(SExpr::Var(Symbol::new("quote"), Span { lo: 0, hi: 1 })),
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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
    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![pair]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![list]);
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

    assert!(parse(&tokenize(src).unwrap()).unwrap() == vec![outer_pair]);
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
fn test_parse_empty_token_stream_panics_as_internal_error() {
    assert!(parse(&[]).is_err());
}

#[test]
fn test_parse_missing_eof_panics_as_internal_error() {
    let tokens = vec![
        Token::LParen(Span { lo: 0, hi: 1 }),
        Token::RParen(Span { lo: 1, hi: 2 }),
    ];
    assert!(parse(&tokens).is_err());
}

#[test]
fn test_parse_unclosed_list_without_eof_panics_as_internal_error() {
    let tokens = vec![Token::LParen(Span { lo: 0, hi: 1 })];
    assert!(parse(&tokens).is_err());
}
