use rrrrr_rs::{
    Session,
    compile::{
        ast::{Application, Begin, Define, Expr, If, Lambda, Letrec, Set},
        ident::{Resolved, Symbol},
        sexpr::{Bool, Cons, Num, SExpr, Str},
        span::Span,
    },
};

fn lower_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let reduced = session.alpha_reduce(expanded);
    session.lower(reduced)
}

#[test]
fn test_lower_number_literal() {
    assert_eq!(
        lower_source("42"),
        Expr::Literal(SExpr::Num(Num(42.0), Span { lo: 0, hi: 2 }))
    );
}

#[test]
fn test_lower_boolean_literal() {
    assert_eq!(
        lower_source("#f"),
        Expr::Literal(SExpr::Bool(Bool(false), Span { lo: 0, hi: 2 }))
    );
}

#[test]
fn test_lower_string_literal() {
    assert_eq!(
        lower_source("\"hello\""),
        Expr::Literal(SExpr::Str(Str("hello".to_string()), Span { lo: 0, hi: 7 }))
    );
}

#[test]
fn test_lower_quoted_symbol() {
    assert_eq!(
        lower_source("'x"),
        Expr::Literal(SExpr::Var(Symbol::new("x"), Span { lo: 1, hi: 2 }))
    );
}

#[test]
fn test_lower_quoted_list() {
    assert_eq!(
        lower_source("'(a b c)"),
        Expr::Literal(SExpr::Cons(
            Cons::new(
                SExpr::Var(Symbol::new("a"), Span { lo: 2, hi: 3 }),
                SExpr::Cons(
                    Cons::new(
                        SExpr::Var(Symbol::new("b"), Span { lo: 4, hi: 5 }),
                        SExpr::Cons(
                            Cons::new(
                                SExpr::Var(Symbol::new("c"), Span { lo: 6, hi: 7 }),
                                SExpr::Nil(Span { lo: 7, hi: 8 }),
                            ),
                            Span { lo: 6, hi: 8 },
                        ),
                    ),
                    Span { lo: 4, hi: 8 },
                ),
            ),
            Span { lo: 1, hi: 8 },
        ))
    );
}

#[test]
fn test_lower_quoted_nil() {
    assert_eq!(
        lower_source("'()"),
        Expr::Literal(SExpr::Nil(Span { lo: 1, hi: 3 }))
    );
}

#[test]
fn test_lower_free_variable() {
    assert_eq!(
        lower_source("x"),
        Expr::Var(
            Resolved::Free {
                symbol: Symbol::new("x")
            },
            Span { lo: 0, hi: 1 },
        )
    );
}

#[test]
fn test_lower_lambda() {
    assert_eq!(
        lower_source("(lambda (x y) x)"),
        Expr::Lambda(
            Lambda {
                args: vec![Symbol::new("x:1"), Symbol::new("y:2")],
                var_arg: None,
                body: vec![Expr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("x"),
                        binding: Symbol::new("x:1"),
                    },
                    Span { lo: 14, hi: 15 },
                )],
            },
            Span { lo: 1, hi: 16 },
        )
    );
}

#[test]
fn test_lower_lambda_rest_param() {
    assert_eq!(
        lower_source("(lambda (x . rest) x)"),
        Expr::Lambda(
            Lambda {
                args: vec![Symbol::new("x:1")],
                var_arg: Some(Symbol::new("rest:2")),
                body: vec![Expr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("x"),
                        binding: Symbol::new("x:1"),
                    },
                    Span { lo: 19, hi: 20 },
                )],
            },
            Span { lo: 1, hi: 21 },
        )
    );
}

#[test]
fn test_lower_lambda_single_rest_param() {
    assert_eq!(
        lower_source("(lambda args args)"),
        Expr::Lambda(
            Lambda {
                args: vec![],
                var_arg: Some(Symbol::new("args:1")),
                body: vec![Expr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("args"),
                        binding: Symbol::new("args:1"),
                    },
                    Span { lo: 13, hi: 17 },
                )],
            },
            Span { lo: 1, hi: 18 },
        )
    );
}

#[test]
fn test_lower_application() {
    assert_eq!(
        lower_source("(f 1 2)"),
        Expr::Application(
            Application {
                operand: Box::new(Expr::Var(
                    Resolved::Free {
                        symbol: Symbol::new("f")
                    },
                    Span { lo: 1, hi: 2 },
                )),
                args: vec![
                    Expr::Literal(SExpr::Num(Num(1.0), Span { lo: 3, hi: 4 })),
                    Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 5, hi: 6 })),
                ],
            },
            Span { lo: 1, hi: 7 },
        )
    );
}

#[test]
fn test_lower_if() {
    assert_eq!(
        lower_source("(if #t 1 2)"),
        Expr::If(
            If {
                test: Box::new(Expr::Literal(SExpr::Bool(
                    Bool(true),
                    Span { lo: 4, hi: 6 }
                ))),
                conseq: Box::new(Expr::Literal(SExpr::Num(Num(1.0), Span { lo: 7, hi: 8 }))),
                alt: Box::new(Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 9, hi: 10 }))),
            },
            Span { lo: 1, hi: 11 },
        )
    );
}

#[test]
fn test_lower_define() {
    assert_eq!(
        lower_source("(define x 42)"),
        Expr::Define(
            Define {
                var: Resolved::Free {
                    symbol: Symbol::new("x"),
                },
                expr: Box::new(Expr::Literal(SExpr::Num(
                    Num(42.0),
                    Span { lo: 10, hi: 12 }
                ))),
            },
            Span { lo: 1, hi: 13 },
        )
    );
}

#[test]
fn test_lower_set() {
    assert_eq!(
        lower_source("(set! x 2)"),
        Expr::Set(
            Set {
                var: Resolved::Free {
                    symbol: Symbol::new("x"),
                },
                expr: Box::new(Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 8, hi: 9 }))),
            },
            Span { lo: 1, hi: 10 },
        )
    );
}

#[test]
fn test_lower_begin() {
    assert_eq!(
        lower_source("(begin 1 2 3)"),
        Expr::Begin(
            Begin {
                body: vec![
                    Expr::Literal(SExpr::Num(Num(1.0), Span { lo: 7, hi: 8 })),
                    Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 9, hi: 10 })),
                    Expr::Literal(SExpr::Num(Num(3.0), Span { lo: 11, hi: 12 })),
                ],
            },
            Span { lo: 1, hi: 13 },
        )
    );
}

#[test]
fn test_lower_letrec() {
    assert_eq!(
        lower_source("(letrec ((x 1) (y 2)) x)"),
        Expr::Letrec(
            Letrec {
                initializers: vec![
                    (
                        Symbol::new("x:1"),
                        Expr::Literal(SExpr::Num(Num(1.0), Span { lo: 12, hi: 13 })),
                    ),
                    (
                        Symbol::new("y:2"),
                        Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 18, hi: 19 })),
                    ),
                ],
                body: vec![Expr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("x"),
                        binding: Symbol::new("x:1"),
                    },
                    Span { lo: 22, hi: 23 },
                )],
            },
            Span { lo: 1, hi: 24 },
        )
    );
}

#[test]
fn test_lower_lambda_with_internal_defines() {
    let result = lower_source("(lambda () (define x 1) (define y 2) (+ x y))");
    let Expr::Lambda(
        Lambda {
            args,
            var_arg,
            mut body,
        },
        _,
    ) = result
    else {
        panic!("Expected Lambda, got {:?}", result);
    };
    assert!(args.is_empty());
    assert!(var_arg.is_none());
    assert_eq!(body.len(), 1);
    let Some(Expr::Letrec(Letrec { initializers, body }, _)) = body.pop() else {
        panic!(
            "Expected Lambda to be transformed into Letrec, got {:?}",
            body[0]
        )
    };
    assert_eq!(
        initializers,
        vec![
            (
                Symbol::new("x:1"),
                Expr::Literal(SExpr::Num(Num(1.0), Span { lo: 21, hi: 22 }))
            ),
            (
                Symbol::new("y:2"),
                Expr::Literal(SExpr::Num(Num(2.0), Span { lo: 34, hi: 35 }))
            )
        ]
    );
    assert_eq!(
        body,
        vec![Expr::Application(
            Application {
                operand: Box::new(Expr::Var(
                    Resolved::Free {
                        symbol: Symbol::new("+")
                    },
                    Span { lo: 38, hi: 39 }
                )),
                args: vec![
                    Expr::Var(
                        Resolved::Bound {
                            symbol: Symbol::new("x"),
                            binding: Symbol::new("x:1")
                        },
                        Span { lo: 40, hi: 41 },
                    ),
                    Expr::Var(
                        Resolved::Bound {
                            symbol: Symbol::new("y"),
                            binding: Symbol::new("y:2")
                        },
                        Span { lo: 42, hi: 43 },
                    )
                ],
            },
            Span { lo: 38, hi: 44 }
        )]
    )
}
