use rrrrr_rs::{Session, compile::anf::Expr};

fn dce_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    let reduced = session.beta_reduce(normalized).unwrap();
    session.dce(reduced)
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_elimination() {
    assert_eq!(pp(dce_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_dead_pure_binding() {
    assert_eq!(pp(dce_source("((lambda () 42))")), "42");
}

#[test]
fn test_dead_binding_after_reduction() {
    assert_eq!(
        pp(dce_source("((lambda (x) (+ x 1)) 1)")),
        r#"
(let ((x:1 1))
  (+:free x:1 1))
        "#
        .trim()
    );
}

#[test]
fn test_nested_dead_bindings() {
    assert_eq!(
        pp(dce_source("((lambda (x) ((lambda (y) (+ x y)) 2)) 1)")),
        r#"
(let ((x:1 1))
  (let ((y:2 2))
    (+:free x:1 y:2)))
        "#
        .trim()
    );
}

#[test]
fn test_side_effectful_binding_preserved() {
    assert_eq!(
        pp(dce_source("((lambda (x) 42) (f 1))")),
        r#"
(let ((anf:3 (f:free 1)))
  42)
        "#
        .trim()
    );
}

#[test]
fn test_cascading_through_dead_lambda() {
    assert_eq!(
        pp(dce_source(
            "((lambda (x) ((lambda (y) 2) x)) (lambda () 1))"
        )),
        "2"
    );
}

#[test]
fn test_live_conditional_initializer_is_folded() {
    assert_eq!(
        pp(dce_source("((lambda (f) f) (if #t ((lambda (x) 1) 2) 3))")),
        r#"
(let ((anf:5 (if #t 1 3)))
  (let ((f:1 anf:5))
    f:1))
        "#
        .trim()
    );
}

#[test]
fn test_if_branch_is_eliminated() {
    assert_eq!(
        pp(dce_source("((lambda () (if 1 (if #f 0 1))))")),
        r#"
1
        "#
        .trim()
    );
}

#[test]
fn test_if_branch_elimination_truthy_values() {
    assert_eq!(
        pp(dce_source("(if '() 1 2)")),
        r#"
1
        "#
        .trim()
    );
}
