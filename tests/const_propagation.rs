use rrrrr_rs::{Session, compile::anf::Expr};

fn propagate_consts_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let contracted = session.beta_contract(normalized).unwrap();
    let cleaned = session.dce(contracted);
    session.propagate_consts(cleaned)
}

fn propagate_consts_source_with_prelude(source: &str) -> Expr {
    let mut session = Session::with_prelude();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let contracted = session.beta_contract(normalized).unwrap();
    let cleaned = session.dce(contracted);
    session.propagate_consts(cleaned)
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_propagation() {
    assert_eq!(pp(propagate_consts_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_atomic_literal_propagated_into_body() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (+ x 1)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 10))
    (+:free 10 1)))
        "#
        .trim()
    );
}

#[test]
fn test_atomic_literal_propagated_to_multiple_uses() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (+ x x)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 10))
    (+:free 10 10)))
        "#
        .trim()
    );
}

#[test]
fn test_compound_literal_single_use_propagated() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((p '(1 2))) ((lambda (x) (car x)) p))"
        )),
        r#"
(let ((p:8 '(1 2)))
  (let ((x:9 '(1 2)))
    (car:free '(1 2))))
        "#
        .trim()
    );
}

#[test]
fn test_compound_literal_multiple_uses_not_propagated() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((p '(1 2))) ((lambda (x) (cons x x)) p))"
        )),
        r#"
(let ((p:8 '(1 2)))
  (let ((x:9 '(1 2)))
    (cons:free x:9 x:9)))
        "#
        .trim()
    );
}

#[test]
fn test_string_literal_multiple_uses_not_propagated() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((s \"hi\")) ((lambda (x) (cons x x)) s))"
        )),
        r#"
(let ((s:8 "hi"))
  (let ((x:9 "hi"))
    (cons:free x:9 x:9)))
        "#
        .trim()
    );
}

#[test]
fn test_string_literal_single_use_propagated() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((s \"hi\")) ((lambda (x) (car x)) s))"
        )),
        r#"
(let ((s:8 "hi"))
  (let ((x:9 "hi"))
    (car:free "hi")))
        "#
        .trim()
    );
}

#[test]
fn test_mutated_binding_not_propagated() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (begin (set! x 5) x)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 10))
    (let ((anf:10 (set! x:9 5)))
      x:9)))
        "#
        .trim()
    );
}

#[test]
fn test_propagation_through_if() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((y 10)) ((lambda (x) (if x x x)) y))"
        )),
        r#"
(let ((y:8 10))
  (let ((x:9 10))
    (if 10 10 10)))
        "#
        .trim()
    );
}

#[test]
fn test_propagation_into_lambda_body() {
    assert_eq!(
        pp(propagate_consts_source_with_prelude(
            "(let ((y 10)) (lambda () y))"
        )),
        r#"
(let ((y:8 10))
  (λ () 10))
        "#
        .trim()
    );
}
