mod common;

use rrrrr_rs::{
    compile::{
        ident::Symbol,
        sexpr::{Num, SExpr, Str},
        span::Span,
        util::{is_proper_list, len},
    },
    if_let_sexpr, make_sexpr, match_sexpr, template_sexpr,
};

#[test]
fn test_multi_match_sexpr() {
    let nil = common::parse_single_source("()").unwrap();
    let list = common::parse_single_source("(1 2 3)").unwrap();
    let num = common::parse_single_source("42").unwrap();

    let classify = |sexpr: &SExpr<Symbol>| -> &str {
        match_sexpr! {
            sexpr;

            () => { "nil" },
            (..) => { "list" },
            _ => { "other" },
        }
    };

    assert_eq!(classify(&nil), "nil");
    assert_eq!(classify(&list), "list");
    assert_eq!(classify(&num), "other");
}

#[test]
fn test_multi_match_sexpr_arm_priority() {
    let list = common::parse_single_source("(1 2)").unwrap();

    // First matching arm wins — (_, _) matches before (..)
    let result: &str = match_sexpr! {
        &list;

        (_, _) => { "two" },
        (..) => { "any-list" },
        _ => { "other" },
    };
    assert_eq!(result, "two");

    // Single-element list should skip (_, _) and match (..)
    let single = common::parse_single_source("(1)").unwrap();
    let result: &str = match_sexpr! {
        &single;

        (_, _) => { "two" },
        (..) => { "any-list" },
        _ => { "other" },
    };
    assert_eq!(result, "any-list");
}

#[test]
fn test_multi_match_sexpr_nested_list() {
    let nested = common::parse_single_source("((a b) c)").unwrap();
    let flat = common::parse_single_source("(a b c)").unwrap();

    let classify = |sexpr: &SExpr<Symbol>| -> &str {
        match_sexpr! {
            sexpr;

            ((_first, _), _) => { "nested-pair" },
            (_, _, _) => { "three" },
            _ => { "other" },
        }
    };

    assert_eq!(classify(&nested), "nested-pair");
    assert_eq!(classify(&flat), "three");
}

#[test]
fn test_multi_match_sexpr_with_try_operator() {
    fn extract_second(sexpr: &SExpr<Symbol>) -> Result<&SExpr<Symbol>, &str> {
        match_sexpr! {
            sexpr;

            (_, second, _) => { Ok(second) },
            _ => { Err("expected a 3-element list") },
        }
    }

    let list = common::parse_single_source("(1 2 3)").unwrap();
    let short = common::parse_single_source("(1)").unwrap();

    assert!(matches!(extract_second(&list), Ok(SExpr::Num(Num(2.0), _))));
    assert!(extract_second(&short).is_err());
}

#[test]
fn test_multi_match_sexpr_default_arm() {
    let num = common::parse_single_source("42").unwrap();
    let result: i32 = match_sexpr! {
        &num;
        () => { 0 },
        (..) => { 1 },
        _ => { 2 },
    };
    assert_eq!(result, 2);
}

#[test]
fn test_template_sexpr_nil() {
    let original = common::parse_single_source("()").unwrap();
    let templated = template_sexpr!(() => original).unwrap();
    assert!(templated == common::parse_single_source("()").unwrap());
}

#[test]
fn test_template_sexpr_single() {
    let original = common::parse_single_source("(0)").unwrap();
    let templated = template_sexpr!(
        (
            SExpr::Num(Num(1.0), Span {lo: 1, hi: 2 })
        ) => &original)
    .unwrap();
    assert!(templated == common::parse_single_source("(1)").unwrap());
}

#[test]
fn test_template_sexpr_double() {
    let original = common::parse_single_source("(0 1)").unwrap();
    let templated = template_sexpr!(
        (
            SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
            SExpr::Num(Num(2.0), Span { lo: 3, hi: 4 })
        ) => &original)
    .unwrap();
    assert!(templated == common::parse_single_source("(1 2)").unwrap());
}

#[test]
fn test_template_sexpr_nested_list_first() {
    let original = common::parse_single_source("((0) 1)").unwrap();
    let templated = template_sexpr!(
        (
            (SExpr::Num(Num(1.0), Span { lo: 2, hi: 3 })),
            SExpr::Num(Num(2.0), Span { lo: 5, hi: 6 })
        ) => &original)
    .unwrap();
    assert!(templated == common::parse_single_source("((1) 2)").unwrap());
}

#[test]
fn test_template_sexpr_nested_list_middle() {
    let original = common::parse_single_source("(0 (1) 2)").unwrap();
    let templated = template_sexpr!(
        (
            SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
            (SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 })),
            SExpr::Num(Num(3.0), Span { lo: 7, hi: 8 })
        ) => &original)
    .unwrap();
    assert!(templated == common::parse_single_source("(1 (2) 3)").unwrap());
}

#[test]
fn test_template_sexpr_nested_list_last() {
    let original = common::parse_single_source("(0 (1))").unwrap();
    let templated = template_sexpr!(
        (
            SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
            (SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 }))
        ) => &original)
    .unwrap();
    assert!(templated == common::parse_single_source("(1 (2))").unwrap());
}

#[test]
fn test_if_let_sexpr_tail_capture_proper_list() {
    // (a, rest @ ..) on proper list (foo x y) — rest should be (x y)
    let sexpr = common::parse_single_source("(foo x y)").unwrap();
    let mut matched = false;
    if_let_sexpr! {(SExpr::Var(..), rest @ ..) = &sexpr => {
        matched = true;
        assert!(matches!(rest, SExpr::Cons(..)));
    }}
    assert!(matched);
}

#[test]
fn test_if_let_sexpr_tail_capture_dotted_pair() {
    // (a, rest @ ..) on dotted pair (foo . x) — rest should be the symbol x
    let sexpr = common::parse_single_source("(foo . x)").unwrap();
    let mut matched = false;
    if_let_sexpr! {(SExpr::Var(..), rest @ ..) = &sexpr => {
        matched = true;
        assert!(matches!(rest, SExpr::Var(Symbol(s), _) if s == "x"));
    }}
    assert!(matched);
}

#[test]
fn test_if_let_sexpr_tail_capture_nil() {
    // (a, rest @ ..) on single-element list (foo) — rest should be nil
    let sexpr = common::parse_single_source("(foo)").unwrap();
    let mut matched = false;
    if_let_sexpr! {(SExpr::Var(..), rest @ ..) = &sexpr => {
        matched = true;
        assert!(matches!(rest, SExpr::Nil(..)));
    }}
    assert!(matched);
}

#[test]
fn test_if_let_sexpr_capture_and_assign_id() {
    // (a @ (...), rest @ ...)
    let sexpr = common::parse_single_source("((\"str\" 0) 1)").unwrap();
    let mut matched = false;
    if_let_sexpr! {(inner @ (first @ SExpr::Str(_, _), second @ SExpr::Num(_, _)), third @ SExpr::Num(_, _)) = sexpr => {
        matched = true;
        assert_eq!(len(&inner), 2);
        assert!(is_proper_list(&inner));
        let span = Span{ lo: 0, hi: 0 };
        assert_eq!(
            inner.without_spans(),
            make_sexpr!(
                SExpr::Str(Str("str".to_owned()), span),
                SExpr::Num(Num(0.0), span),
            ).without_spans()
        );
        assert_eq!(first.without_spans(), SExpr::Str(Str("str".to_owned()), span).without_spans());
        assert_eq!(second.without_spans(), SExpr::Num(Num(0.0), span).without_spans());
        assert_eq!(third.without_spans(), SExpr::Num(Num(1.0), span).without_spans());
    }}
    assert!(matched);
}

#[test]
fn test_if_let_sexpr_nested_list_tail_capture_dotted() {
    // ((inner), rest @ ..) on ((a b) . x) — rest should be the symbol x
    let sexpr = common::parse_single_source("((a b) . x)").unwrap();
    let mut matched = false;
    if_let_sexpr! {((..), rest @ ..) = &sexpr => {
        matched = true;
        assert!(matches!(rest, SExpr::Var(Symbol(s), _) if s == "x"));
    }}
    assert!(matched);
}

#[test]
fn test_match_sexpr_tail_capture_dotted() {
    let proper = common::parse_single_source("(define (foo x y) body)").unwrap();
    let dotted = common::parse_single_source("(define (foo . x) body)").unwrap();

    let extract_args = |sexpr: &SExpr<Symbol>| -> &str {
        match_sexpr! {
            sexpr;

            (_, (SExpr::Var(..), args @ ..), _) => {
                if matches!(args, SExpr::Var(..)) {
                    "atom"
                } else {
                    "list"
                }
            },
            _ => { "no-match" },
        }
    };

    assert_eq!(extract_args(&proper), "list");
    assert_eq!(extract_args(&dotted), "atom");
}
