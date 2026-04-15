use rrrrr_rs::{Session, compile::anf::Expr};

fn propagate_copies_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let reduced = session.beta_reduce(normalized).unwrap();
    session.propagate_copies(reduced)
}

fn propagate_copies_source_with_prelude(source: &str) -> Expr {
    let mut session = Session::with_prelude();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let reduced = session.beta_reduce(normalized).unwrap();
    session.propagate_copies(reduced)
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_propagation() {
    assert_eq!(pp(propagate_copies_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_literal_rhs_not_propagated() {
    assert_eq!(
        pp(propagate_copies_source("((lambda (x) x) 1)")),
        r#"
(let ((x:1 1))
  x:1)
        "#
        .trim()
    );
}

#[test]
fn test_simple_copy_eliminated() {
    assert_eq!(
        pp(propagate_copies_source_with_prelude(
            "(let ((y 10)) ((lambda (x) x) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 y:8))
    y:8))
        "#
        .trim()
    );
}

#[test]
fn test_copy_propagated_into_body() {
    assert_eq!(
        pp(propagate_copies_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (+ x 1)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 y:8))
    (+:free y:8 1)))
        "#
        .trim()
    );
}

#[test]
fn test_copy_propagated_to_multiple_uses() {
    assert_eq!(
        pp(propagate_copies_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (+ x x)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 y:8))
    (+:free y:8 y:8)))
        "#
        .trim()
    );
}

#[test]
fn test_multi_arg_copies_propagated() {
    assert_eq!(
        pp(propagate_copies_source_with_prelude(
            "(let ((a 1) (b 2)) ((lambda (x y) (+ x y)) a b))"
        )),
        r#"
(let ((a:8 1))
  (let ((b:9 2))
    (let ((x:10 a:8))
      (let ((y:11 b:9))
        (+:free a:8 b:9)))))
        "#
        .trim()
    );
}

#[test]
fn test_mutated_binding_preserved() {
    assert_eq!(
        pp(propagate_copies_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (begin (set! x 5) x)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 y:8))
    (let ((anf:10 (set! x:9 5)))
      x:9)))
        "#
        .trim()
    );
}
