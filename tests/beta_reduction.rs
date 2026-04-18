use rrrrr_rs::{Session, compile::anf::Expr};

fn beta_reduce_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    session.beta_reduce(normalized).unwrap()
}

fn beta_reduce_source_with_prelude(source: &str) -> Expr {
    let mut session = Session::with_prelude();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    session.beta_reduce(normalized).unwrap()
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_reduction() {
    assert_eq!(pp(beta_reduce_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_zero_arg_reduction() {
    assert_eq!(
        pp(beta_reduce_source("((lambda () 42))")),
        r#"42
        "#
        .trim()
    );
}

#[test]
fn test_single_arg_reduction() {
    assert_eq!(
        pp(beta_reduce_source("((lambda (x) (+ x 1)) 1)")),
        r#"
(let ((x:1 1))
  (+:free x:1 1))
        "#
        .trim()
    );
}

#[test]
fn test_multi_arg_reduction() {
    assert_eq!(
        pp(beta_reduce_source("((lambda (x y) (+ x y)) 1 2)")),
        r#"
(let ((x:1 1))
  (let ((y:2 2))
    (+:free x:1 y:2)))
        "#
        .trim()
    );
}

#[test]
fn test_nested_reduction() {
    assert_eq!(
        pp(beta_reduce_source(
            "((lambda (x) ((lambda (y) (+ x y)) 2)) 1)"
        )),
        r#"
(let ((x:1 1))
  (let ((y:2 2))
    (+:free x:1 y:2)))
        "#
        .trim()
    );
}

#[test]
fn test_let_form() {
    assert_eq!(
        pp(beta_reduce_source_with_prelude(
            "(let ((x 1) (y 2)) (+ x y))"
        )),
        r#"
(let ((x:8 1))
  (let ((y:9 2))
    (+:free x:8 y:9)))
        "#
        .trim()
    );
}

#[test]
fn test_reduction_inside_if_conseq() {
    assert_eq!(
        pp(beta_reduce_source("(if x ((lambda (x) x) 1) 2)")),
        r#"
(if x:free
    (let ((x:1 1))
      x:1)
    2)
        "#
        .trim()
    );
}

#[test]
fn test_reduction_inside_if_alt() {
    assert_eq!(
        pp(beta_reduce_source("(if x 1 ((lambda (x) x) 2))")),
        r#"
(if x:free
    1
    (let ((x:1 2))
      x:1))
        "#
        .trim()
    );
}

#[test]
fn test_reduction_inside_nested_if() {
    assert_eq!(
        pp(beta_reduce_source("(if x (if y ((lambda (x) x) 1) 2) 3)")),
        r#"
(if x:free
    (if y:free
        (let ((x:1 1))
          x:1)
        2)
    3)
        "#
        .trim()
    );
}
