use rrrrr_rs::{Session, compile::anf::Expr};

fn beta_contract_source(source: &str) -> Expr {
    let mut session = Session::new();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    session.beta_contract(normalized).unwrap()
}

fn beta_contract_source_with_prelude(source: &str) -> Expr {
    let mut session = Session::with_prelude();
    let tokens = session.tokenize(source).unwrap();
    let parsed = session.parse(&tokens).unwrap().pop().unwrap();
    let introduced = session.introduce(parsed);
    let expanded = session.expand(introduced).unwrap();
    let converted = session.alpha_convert(expanded);
    let lowered = session.lower(converted);
    let normalized = session.a_normalize(lowered);
    session.beta_contract(normalized).unwrap()
}

fn pp(expr: Expr) -> String {
    format!("{}", expr)
}

#[test]
fn test_no_contraction() {
    assert_eq!(pp(beta_contract_source("(f 1)")), "(f:free 1)");
}

#[test]
fn test_zero_arg_contraction() {
    assert_eq!(
        pp(beta_contract_source("((lambda () 42))")),
        r#"42
        "#
        .trim()
    );
}

#[test]
fn test_single_arg_contraction() {
    assert_eq!(
        pp(beta_contract_source("((lambda (x) (+ x 1)) 1)")),
        r#"
(let ((x:1 1))
  (+:free x:1 1))
        "#
        .trim()
    );
}

#[test]
fn test_multi_arg_contraction() {
    assert_eq!(
        pp(beta_contract_source("((lambda (x y) (+ x y)) 1 2)")),
        r#"
(let ((x:1 1))
  (let ((y:2 2))
    (+:free x:1 y:2)))
        "#
        .trim()
    );
}

#[test]
fn test_nested_contraction() {
    assert_eq!(
        pp(beta_contract_source(
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
        pp(beta_contract_source_with_prelude(
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
