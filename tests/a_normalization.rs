use rrrrr_rs::{
    Session,
    compile::{
        anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set, Value},
        ident::{ResolvedVar, Symbol},
        sexpr::{Bool, Num, SExpr},
        span::Span,
    },
};

fn a_normalize_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    session.a_normalize(lowered)
}

#[test]
fn test_a_normalize_literal() {
    assert_eq!(
        a_normalize_source("42"),
        Expr::AExpr(AExpr::Literal(SExpr::Num(Num(42.0), Span { lo: 0, hi: 2 })))
    );
}

#[test]
fn test_a_normalize_free_variable() {
    assert_eq!(
        a_normalize_source("x"),
        Expr::AExpr(AExpr::Var(
            ResolvedVar::Free {
                symbol: Symbol::new("x"),
            },
            Span { lo: 0, hi: 1 },
        ))
    );
}

#[test]
fn test_a_normalize_lambda() {
    assert_eq!(
        a_normalize_source("(lambda (x) x)"),
        Expr::AExpr(AExpr::Lambda(
            Lambda {
                args: vec![Symbol::new("x:1")],
                var_arg: None,
                body: Box::new(Expr::AExpr(AExpr::Var(
                    ResolvedVar::Bound {
                        symbol: Symbol::new("x"),
                        binding: Symbol::new("x:1"),
                    },
                    Span { lo: 12, hi: 13 },
                ))),
            },
            Span { lo: 1, hi: 14 },
        ))
    );
}

#[test]
fn test_a_normalize_application() {
    assert_eq!(
        a_normalize_source("(f 1 2)"),
        Expr::CExpr(CExpr::Application(
            Application {
                operand: Box::new(Value::Var(
                    ResolvedVar::Free {
                        symbol: Symbol::new("f"),
                    },
                    Span { lo: 1, hi: 2 },
                )),
                args: vec![
                    Value::Literal(SExpr::Num(Num(1.0), Span { lo: 3, hi: 4 })),
                    Value::Literal(SExpr::Num(Num(2.0), Span { lo: 5, hi: 6 })),
                ],
            },
            Span { lo: 1, hi: 7 },
        ))
    );
}

#[test]
fn test_a_normalize_application_names_complex_arg() {
    // (f (g 1))
    // →
    // (let ((anf:1 (g 1)))
    //   (f anf:1))
    assert_eq!(
        a_normalize_source("(f (g 1))"),
        Expr::Let(
            Let {
                initializer: Box::new((
                    Symbol::new("anf:1"),
                    Rhs::CExpr(CExpr::Application(
                        Application {
                            operand: Box::new(Value::Var(
                                ResolvedVar::Free {
                                    symbol: Symbol::new("g"),
                                },
                                Span { lo: 4, hi: 5 },
                            )),
                            args: vec![Value::Literal(
                                SExpr::Num(Num(1.0), Span { lo: 6, hi: 7 },)
                            )],
                        },
                        Span { lo: 4, hi: 8 },
                    )),
                )),
                body: Box::new(Expr::CExpr(CExpr::Application(
                    Application {
                        operand: Box::new(Value::Var(
                            ResolvedVar::Free {
                                symbol: Symbol::new("f"),
                            },
                            Span { lo: 1, hi: 2 },
                        )),
                        args: vec![Value::Var(
                            ResolvedVar::Bound {
                                symbol: Symbol::new("anf"),
                                binding: Symbol::new("anf:1"),
                            },
                            Span { lo: 4, hi: 8 },
                        )],
                    },
                    Span { lo: 1, hi: 9 },
                ))),
            },
            Span { lo: 4, hi: 8 },
        )
    );
}

#[test]
fn test_a_normalize_if() {
    assert_eq!(
        a_normalize_source("(if #t 1 2)"),
        Expr::CExpr(CExpr::If(
            If {
                test: Box::new(Value::Literal(SExpr::Bool(
                    Bool(true),
                    Span { lo: 4, hi: 6 },
                ))),
                conseq: Box::new(Expr::AExpr(AExpr::Literal(SExpr::Num(
                    Num(1.0),
                    Span { lo: 7, hi: 8 },
                )))),
                alt: Box::new(Expr::AExpr(AExpr::Literal(SExpr::Num(
                    Num(2.0),
                    Span { lo: 9, hi: 10 },
                )))),
            },
            Span { lo: 1, hi: 11 },
        ))
    );
}

#[test]
fn test_a_normalize_set() {
    assert_eq!(
        a_normalize_source("(set! x 1)"),
        Expr::CExpr(CExpr::Set(
            Set {
                var: ResolvedVar::Free {
                    symbol: Symbol::new("x"),
                },
                value: Value::Literal(SExpr::Num(Num(1.0), Span { lo: 8, hi: 9 })),
            },
            Span { lo: 1, hi: 10 },
        ))
    );
}

#[test]
fn test_a_normalize_begin_returns_last() {
    assert_eq!(
        a_normalize_source("(begin 1 2 3)"),
        Expr::AExpr(AExpr::Literal(SExpr::Num(
            Num(3.0),
            Span { lo: 11, hi: 12 },
        )))
    );
}

#[test]
fn test_a_normalize_begin_preserves_side_effects() {
    // (begin (f 1) 2)
    // →
    // (let ((anf:1 (f 1)))
    //   2)
    assert_eq!(
        a_normalize_source("(begin (f 1) 2)"),
        Expr::Let(
            Let {
                initializer: Box::new((
                    Symbol::new("anf:1"),
                    Rhs::CExpr(CExpr::Application(
                        Application {
                            operand: Box::new(Value::Var(
                                ResolvedVar::Free {
                                    symbol: Symbol::new("f"),
                                },
                                Span { lo: 8, hi: 9 },
                            )),
                            args: vec![Value::Literal(SExpr::Num(
                                Num(1.0),
                                Span { lo: 10, hi: 11 },
                            ))],
                        },
                        Span { lo: 8, hi: 12 },
                    )),
                )),
                body: Box::new(Expr::AExpr(AExpr::Literal(SExpr::Num(
                    Num(2.0),
                    Span { lo: 13, hi: 14 },
                )))),
            },
            Span { lo: 8, hi: 12 },
        )
    );
}

#[test]
fn test_a_normalize_nested_application_in_arg() {
    // (+ ((lambda () 1)))
    // →
    // (let ((anf:1 (lambda () 1)))
    //   (let ((anf:2 (anf:1)))
    //     (+:free anf:2)))
    assert_eq!(
        a_normalize_source("(+ ((lambda () 1)))"),
        Expr::Let(
            Let {
                initializer: Box::new((
                    Symbol::new("anf:1"),
                    Rhs::AExpr(AExpr::Lambda(
                        Lambda {
                            args: vec![],
                            var_arg: None,
                            body: Box::new(Expr::AExpr(AExpr::Literal(SExpr::Num(
                                Num(1.0),
                                Span { lo: 15, hi: 16 }
                            ))))
                        },
                        Span { lo: 5, hi: 17 }
                    )),
                )),
                body: Box::new(Expr::Let(
                    Let {
                        initializer: Box::new((
                            Symbol::new("anf:2"),
                            Rhs::CExpr(CExpr::Application(
                                Application {
                                    operand: Box::new(Value::Var(
                                        ResolvedVar::Bound {
                                            symbol: Symbol::new("anf"),
                                            binding: Symbol::new("anf:1")
                                        },
                                        Span { lo: 5, hi: 17 }
                                    )),
                                    args: vec![],
                                },
                                Span { lo: 5, hi: 18 }
                            ))
                        )),
                        body: Box::new(Expr::CExpr(CExpr::Application(
                            Application {
                                operand: Box::new(Value::Var(
                                    ResolvedVar::Free {
                                        symbol: Symbol::new("+")
                                    },
                                    Span { lo: 1, hi: 2 }
                                )),
                                args: vec![Value::Var(
                                    ResolvedVar::Bound {
                                        symbol: Symbol::new("anf"),
                                        binding: Symbol::new("anf:2")
                                    },
                                    Span { lo: 5, hi: 18 }
                                )],
                            },
                            Span { lo: 1, hi: 19 }
                        )))
                    },
                    Span { lo: 5, hi: 18 }
                )),
            },
            Span { lo: 5, hi: 17 },
        )
    );
}

#[test]
fn test_pretty_print() {
    assert_eq!(
        format!(
            "{}",
            a_normalize_source("(define (f x y) (if (= x 0) y (f (- x 1) (* x y))))")
        ),
        r#"
(let ((anf:7
       (λ (x:1 y:2)
         (let ((anf:4 (=:free x:1 0)))
           (if anf:4
               y:2
               (let ((anf:5 (-:free x:1 1)))
                 (let ((anf:6 (*:free x:1 y:2)))
                   (f:free anf:5 anf:6))))))))
  (set! f:free anf:7))
        "#
        .trim()
    );
}
