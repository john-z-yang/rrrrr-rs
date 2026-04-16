use rrrrr_rs::{Session, compile::anf::Expr};

fn eta_reduce_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let reduced = session.beta_reduce(normalized).unwrap();
    session.eta_reduce(reduced)
}

fn eta_reduce_source_with_prelude(source: &str) -> Expr {
    let mut session = Session::with_prelude();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let reduced = session.beta_reduce(normalized).unwrap();
    session.eta_reduce(reduced)
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_reduction() {
    assert_eq!(pp(eta_reduce_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_basic_eta_reduction() {
    // (let ((f identity)) (let ((g (lambda (x) (f x)))) (g 42)))
    // anf:6 wraps f:1 — eta reduces anf:6 -> f:1, so g:2's RHS becomes f:1
    assert_eq!(
        pp(eta_reduce_source(
            "((lambda (f) ((lambda (g) (g 42)) (lambda (x) (f x)))) (lambda (z) z))"
        )),
        r#"
(let ((anf:8
       (λ (z:4) z:4)))
  (let ((f:1 anf:8))
    (let ((anf:6
           (λ (x:3) (f:1 x:3))))
      (let ((g:2 f:1))
        (g:2 42)))))
        "#
        .trim()
    );
}

#[test]
fn test_multi_arg_eta_reduction() {
    assert_eq!(
        pp(eta_reduce_source(
            "((lambda (f) ((lambda (g) (g 1 2)) (lambda (x y) (f x y)))) (lambda (a b) (+ a b)))"
        )),
        r#"
(let ((anf:10
       (λ (a:5 b:6) (+:free a:5 b:6))))
  (let ((f:1 anf:10))
    (let ((anf:8
           (λ (x:3 y:4) (f:1 x:3 y:4))))
      (let ((g:2 f:1))
        (g:2 1 2)))))
        "#
        .trim()
    );
}

#[test]
fn test_no_reduction_free_operand() {
    // f is free — try_eta requires Bound operand
    assert_eq!(
        pp(eta_reduce_source(
            "((lambda (g) (g 42)) (lambda (x) (f x)))"
        )),
        r#"
(let ((anf:4
       (λ (x:2) (f:free x:2))))
  (let ((g:1 anf:4))
    (g:1 42)))
        "#
        .trim()
    );
}

#[test]
fn test_no_reduction_operand_is_param() {
    // (lambda (f x) (f x)) — operand f is a lambda param, not free
    assert_eq!(
        pp(eta_reduce_source(
            "((lambda (h) (h (lambda (z) z) 42)) (lambda (f x) (f x)))"
        )),
        r#"
(let ((anf:7
       (λ (f:3 x:4) (f:3 x:4))))
  (let ((h:1 anf:7))
    (let ((anf:5
           (λ (z:2) z:2)))
      (h:1 anf:5 42))))
        "#
        .trim()
    );
}

#[test]
fn test_no_reduction_arg_order_mismatch() {
    // (lambda (x y) (f y x)) — args are swapped, not eta
    assert_eq!(
        pp(eta_reduce_source(
            "((lambda (f) ((lambda (g) (g 1 2)) (lambda (x y) (f y x)))) (lambda (a b) (+ a b)))"
        )),
        r#"
(let ((anf:10
       (λ (a:5 b:6) (+:free a:5 b:6))))
  (let ((f:1 anf:10))
    (let ((anf:8
           (λ (x:3 y:4) (f:1 y:4 x:3))))
      (let ((g:2 anf:8))
        (g:2 1 2)))))
        "#
        .trim()
    );
}

#[test]
fn test_no_reduction_rebound_operand() {
    assert_eq!(
        pp(eta_reduce_source_with_prelude(
            "
(let ((f (lambda (x) x)))
  (let ((g (lambda (x) (f x))))
    (let ((a (set! f (lambda (x) 0))))
      (g 1))))"
        )),
        r#"
(let ((anf:20
       (λ (x:13) x:13)))
  (let ((f:8 anf:20))
    (let ((anf:18
           (λ (x:12) (f:8 x:12))))
      (let ((g:9 anf:18))
        (let ((anf:15
               (λ (x:11) 0)))
          (let ((anf:16 (set! f:8 anf:15)))
            (g:9 1)))))))
        "#
        .trim()
    );
}
