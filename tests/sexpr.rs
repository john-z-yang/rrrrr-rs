mod common;

use rrrrr_rs::{
    compile::{
        bindings::Id,
        sexpr::{Cons, Num, SExpr, Vector},
        span::Span,
    },
    make_sexpr,
};

#[test]
fn test_add_scope() {
    let span = Span { lo: 0, hi: 1 };
    let list = make_sexpr!(
        SExpr::Var(Id::new("a", [1]), span),
        (SExpr::Var(Id::new("b", [1]), span)),
        (SExpr::Var(Id::new("c", [0]), span)),
        SExpr::Var(Id::new("d", [0, 1]), span),
    );
    assert_eq!(
        list.add_scope(0).add_scope(2).without_spans(),
        make_sexpr!(
            SExpr::Var(Id::new("a", [0, 1, 2]), span),
            (SExpr::Var(Id::new("b", [0, 1, 2]), span)),
            (SExpr::Var(Id::new("c", [0, 2]), span)),
            SExpr::Var(Id::new("d", [0, 1, 2]), span),
        )
        .without_spans()
    )
}

#[test]
fn test_flip_scope() {
    let span = Span { lo: 0, hi: 1 };
    let list = make_sexpr!(
        SExpr::Var(Id::new("a", [1]), span),
        (SExpr::Var(Id::new("b", [1]), span)),
        (SExpr::Var(Id::new("c", [0]), span)),
        SExpr::Var(Id::new("d", [0, 1]), span),
    );
    assert_eq!(
        list.flip_scope(0).without_spans(),
        make_sexpr!(
            SExpr::Var(Id::new("a", [1, 0]), span),
            (SExpr::Var(Id::new("b", [1, 0]), span)),
            (SExpr::Var(Id::new("c", []), span)),
            SExpr::Var(Id::new("d", [1]), span),
        )
        .without_spans()
    )
}

#[test]
fn test_add_scope_vector() {
    let span = Span { lo: 0, hi: 1 };
    let vector = SExpr::Vector(
        Vector(vec![
            SExpr::Var(Id::new("a", [1]), span),
            SExpr::Var(Id::new("b", [0]), span),
            SExpr::Num(Num(42.0), span),
        ]),
        span,
    );
    assert_eq!(
        vector.add_scope(2).without_spans(),
        SExpr::Vector(
            Vector(vec![
                SExpr::Var(Id::new("a", [1, 2]), span),
                SExpr::Var(Id::new("b", [0, 2]), span),
                SExpr::Num(Num(42.0), span),
            ]),
            span,
        )
        .without_spans()
    )
}

#[test]
fn test_flip_scope_vector() {
    let span = Span { lo: 0, hi: 1 };
    let vector = SExpr::Vector(
        Vector(vec![
            SExpr::Var(Id::new("a", [0, 1]), span),
            SExpr::Var(Id::new("b", [1]), span),
        ]),
        span,
    );
    assert_eq!(
        vector.flip_scope(1).without_spans(),
        SExpr::Vector(
            Vector(vec![
                SExpr::Var(Id::new("a", [0]), span),
                SExpr::Var(Id::new("b", []), span),
            ]),
            span,
        )
        .without_spans()
    )
}

#[test]
fn test_add_scope_nested_vector() {
    let span = Span { lo: 0, hi: 1 };
    let nested = make_sexpr!(
        SExpr::Vector(Vector(vec![SExpr::Var(Id::new("x", [1]), span)]), span,),
        SExpr::Var(Id::new("y", [1]), span),
    );
    assert_eq!(
        nested.add_scope(2).without_spans(),
        make_sexpr!(
            SExpr::Vector(Vector(vec![SExpr::Var(Id::new("x", [1, 2]), span)]), span,),
            SExpr::Var(Id::new("y", [1, 2]), span),
        )
        .without_spans()
    )
}

#[test]
fn test_vector_to_cons_list() {
    let SExpr::Vector(vector, span) = common::parse_single_source("#(1 2 3)").unwrap() else {
        unreachable!("Expected a vector")
    };

    assert_eq!(
        vector.into_cons_list(span),
        SExpr::Cons(
            Cons {
                car: Box::new(SExpr::Num(Num(1.0), Span { lo: 2, hi: 3 })),
                cdr: Box::new(SExpr::Cons(
                    Cons {
                        car: Box::new(SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 })),
                        cdr: Box::new(SExpr::Cons(
                            Cons {
                                car: Box::new(SExpr::Num(Num(3.0), Span { lo: 6, hi: 7 })),
                                cdr: Box::new(SExpr::Nil(Span { lo: 7, hi: 8 })),
                            },
                            Span { lo: 6, hi: 8 },
                        )),
                    },
                    Span { lo: 4, hi: 8 },
                )),
            },
            Span { lo: 2, hi: 8 },
        ),
    );
}

#[test]
fn test_eq_includes_spans() {
    let left: SExpr<Id> = SExpr::Num(Num(1.0), Span { lo: 0, hi: 1 });
    let right: SExpr<Id> = SExpr::Num(Num(1.0), Span { lo: 3, hi: 4 });

    assert_ne!(left, right);
    assert_eq!(left.without_spans(), right.without_spans());
}

#[test]
fn test_debug_without_spans_omits_span_fields() {
    let sexpr = SExpr::cons(
        SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
        SExpr::Vector(
            Vector(vec![SExpr::Var(Id::new("x", [1]), Span { lo: 3, hi: 4 })]),
            Span { lo: 5, hi: 6 },
        ),
    );

    let rendered = format!("{:?}", sexpr.without_spans());
    assert!(
        !rendered.contains("lo"),
        "debug output leaked span: {rendered}"
    );
    assert!(
        !rendered.contains("hi"),
        "debug output leaked span: {rendered}"
    );
}
