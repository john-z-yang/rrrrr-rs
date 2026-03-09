use rrrrr_rs::{
    Session,
    compile::{
        compilation_error::{CompilationError, Result},
        sexpr::{Num, SExpr, Symbol},
        span::Span,
        util::{first, nth, rest},
    },
};

fn expand_source(source: &str) -> Result<SExpr> {
    let mut session = Session::new();
    let tokens = session.tokenize(source)?;
    let parsed = session.parse(&tokens)?;
    let introduced = session.introduce(&parsed);
    session.expand(&introduced)
}

fn expand_with_session(source: &str) -> (Session, Result<SExpr>) {
    let mut session = Session::new();
    let result = (|| {
        let tokens = session.tokenize(source)?;
        let parsed = session.parse(&tokens)?;
        let introduced = session.introduce(&parsed);
        session.expand(&introduced)
    })();
    (session, result)
}

fn session_expand(session: &mut Session, source: &str) -> Result<SExpr> {
    let tokens = session.tokenize(source)?;
    let parsed = session.parse(&tokens)?;
    let introduced = session.introduce(&parsed);
    session.expand(&introduced)
}

fn assert_generated_define_is_referenced(source: &str, expand_message: &str) {
    let (session, result) = expand_with_session(source);
    assert!(result.is_ok(), "{expand_message}, got: {:?}", result);
    let result = result.unwrap();
    let defined_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let body_ref = nth(&result, 3).unwrap();
    let SExpr::Id(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(body_ref, _) = body_ref else {
        panic!("Expected body reference to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to generated define"
    );
}

// --- Error-only tests ---

#[test]
fn test_expand_lambda_invalid_non_id_param() {
    assert!(expand_source("(lambda 42 x)").is_err());
}

#[test]
fn test_expand_lambda_invalid_dotted_param() {
    assert!(expand_source("(lambda (x . 42) x)").is_err());
}

#[test]
fn test_expand_top_level_unbound_id_reports_error() {
    assert!(
        expand_source("x").is_err(),
        "Expected unbound identifier at top level to be an error"
    );
}

#[test]
fn test_expand_set_unbound_identifier_reports_span() {
    assert!(
        matches!(
            expand_source("(set! x 1)"),
            Err(CompilationError {
                span: Span { lo: 6, hi: 7 },
                reason: _
            })
        ),
        "Expected set! on unbound identifier to report identifier span"
    );
}

#[test]
fn test_expand_set_core_binding_rejected_with_form_span() {
    assert!(
        matches!(
            expand_source("(set! cons 1)"),
            Err(CompilationError {
                span: Span { lo: 0, hi: 13 },
                reason: _
            })
        ),
        "Expected set! on core binding to report whole-form span"
    );
}

#[test]
fn test_expand_define_rhs_rejects_nested_define_in_expression_context() {
    assert!(
        matches!(
            expand_source("(define x (define y 1))"),
            Err(CompilationError { reason, .. }) if reason == "'define' is not allowed in an expression context"
        ),
        "Expected define RHS to be expanded in expression context"
    );
}

#[test]
fn test_expand_set_rhs_rejects_nested_define_in_expression_context() {
    assert!(
        matches!(
            expand_source("(begin (define x 1) (set! x (define y 2)) x)"),
            Err(CompilationError { reason, .. }) if reason == "'define' is not allowed in an expression context"
        ),
        "Expected set! RHS to be expanded in expression context"
    );
}

#[test]
fn test_expand_begin_in_expression_context_rejects_define_with_span() {
    assert!(
        matches!(
            expand_source("(lambda () (cons (begin (define x 1) x) 1))"),
            Err(CompilationError {
                span: Span { lo: 24, hi: 36 },
                reason: _
            })
        ),
        "Expected define within begin argument position to be rejected in expression context"
    );
}

#[test]
fn test_expand_lambda_non_leading_define_after_begin_expression_reports_span() {
    assert!(
        matches!(
            expand_source("(lambda () (begin 1) (define x 2))"),
            Err(CompilationError {
                span: Span { lo: 21, hi: 33 },
                reason: _
            })
        ),
        "Expected non-leading internal define to report define form span"
    );
}

#[test]
fn test_expand_lambda_begin_requires_expression() {
    assert!(
        expand_source("(lambda () (begin))").is_err(),
        "Expected begin with no body forms to fail"
    );
}

#[test]
fn test_expand_lambda_begin_improper_tail_reports_error_span() {
    assert!(
        matches!(
            expand_source("(lambda () (begin 1 . 2))"),
            Err(CompilationError {
                span: Span { lo: 11, hi: 24 },
                reason: _
            })
        ),
        "Expected improper begin in lambda body to report begin span"
    );
}

#[test]
fn test_expand_begin_improper_list_reports_error_span() {
    assert!(
        matches!(
            expand_source("(begin 1 . 2)"),
            Err(CompilationError {
                span: Span { lo: 0, hi: 13 },
                reason: _
            })
        ),
        "Expected improper top-level begin to report whole form span"
    );
}

#[test]
fn test_shadowed_syntax_rules_is_rejected() {
    let result = expand_source(
        r#"
        (lambda (syntax-rules)
          (letrec-syntax
            ((my-mac (syntax-rules ()
                       ((_) 1))))
            (my-mac)))
        "#,
    );
    assert!(
        result.is_err(),
        "Expected error when syntax-rules is shadowed by a lambda parameter"
    );
}

#[test]
fn test_expand_letrec_syntax_unbound_ellipsis_in_template_reports_error() {
    assert!(
        matches!(
            expand_source("(letrec-syntax ((m (syntax-rules () ((_ x) (...))))) (m 1))"),
            Err(CompilationError {
                span: Span { lo: 44, hi: 47 },
                reason
            }) if reason == "Unbound identifier: '...'"
        ),
        "Expected malformed ellipsis template usage to return a compilation error"
    );
}

#[test]
fn test_let_syntax_bindings_are_not_recursive() {
    let result = expand_source(
        r#"
        (let-syntax
          ((one (syntax-rules () ((_) 1)))
           (two (syntax-rules () ((_) (one)))))
          (two))
        "#,
    );
    assert!(
        result.is_err(),
        "Expected let-syntax bindings to not be recursive: (one) inside two's expansion should be unbound"
    );
}

#[test]
fn test_expand_lambda_macro_expanding_to_expression_ends_define_phase() {
    let result = expand_source(
        r#"
        (letrec-syntax
          ((expr (syntax-rules ()
                   ((_ x) x))))
          (lambda () (expr 1) (define y 2) y))
        "#,
    );
    assert!(
        result.is_err(),
        "Expected macro expanding to expression to end define phase, rejecting subsequent define"
    );
}

#[test]
fn test_expand_define_syntax_rejected_in_expression_context() {
    assert!(matches!(
        expand_source(
            r#"
            (list (define-syntax one
                    (syntax-rules ()
                      ((_) 1))))
            "#
        ),
        Err(CompilationError { reason, .. })
            if reason == "'define-syntax' is only allowed in the top level context"
    ));
}

#[test]
fn test_expand_define_syntax_rejected_in_body_context() {
    assert!(matches!(
        expand_source(
            r#"
            (lambda ()
              (define-syntax one
                (syntax-rules ()
                  ((_) 1)))
              (one))
            "#
        ),
        Err(CompilationError { reason, .. })
            if reason == "'define-syntax' is only allowed in the top level context"
    ));
}

#[test]
fn test_expand_define_syntax_invalid_form() {
    assert!(matches!(
        expand_source("(define-syntax)"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid 'define-syntax' form"
    ));
}

#[test]
fn test_expand_define_syntax_rejects_non_syntax_rules_transformer() {
    assert!(matches!(
        expand_source(
            r#"
            (define-syntax one (lambda (x) x))
            "#
        ),
        Err(CompilationError { reason, .. })
            if reason == "Expected a 'syntax-rules' transformer"
    ));
}

// --- Success-only / simple output tests ---

#[test]
fn test_expand_lambda_allows_unbound_id_in_body() {
    let result = expand_source("(lambda () x)");
    assert!(
        result.is_ok(),
        "Expected unbound identifier in lambda body to be allowed, got: {:?}",
        result
    );
}

#[test]
fn test_expand_lambda_allows_unbound_application_in_body() {
    let result = expand_source("(lambda () (f x))");
    assert!(
        result.is_ok(),
        "Expected unbound application in lambda body to be allowed, got: {:?}",
        result
    );
}

#[test]
fn test_expand_lambda_allows_set_on_unbound_id_in_body() {
    let result = expand_source("(lambda () (set! x 1))");
    assert!(
        result.is_ok(),
        "Expected set! on unbound identifier in lambda body to be allowed, got: {:?}",
        result
    );
}

#[test]
fn test_expand_let_syntax_to_num() {
    let result = expand_source(
        r#"
        (letrec-syntax
            ((one (syntax-rules ()
                    ((_) 1))))
          (one))
        "#,
    )
    .unwrap();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), span).without_spans()
    );
}

#[test]
fn test_letrec_syntax_allows_multiple_transformer_bindings() {
    let result = expand_source(
        r#"
        (letrec-syntax
          ((one (syntax-rules () ((_) 1)))
           (two (syntax-rules () ((_) 2))))
          (one))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected multi-binding letrec-syntax to expand, got: {:?}",
        result
    );
    let result = result.unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), result.get_span()).without_spans()
    );
}

#[test]
fn test_expand_letrec_syntax_internal_define_in_expression_position() {
    let result = expand_source(
        r#"
        (list
          (letrec-syntax
            ((one (syntax-rules ()
                    ((_ x) x))))
            (define x 1)
            x))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected letrec-syntax body defines to expand in body context, got: {:?}",
        result
    );
}

#[test]
fn test_let_syntax_basic_expansion() {
    let result = expand_source(
        r#"
        (let-syntax
          ((one (syntax-rules ()
                  ((_) 1))))
          (one))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected let-syntax to expand, got: {:?}",
        result
    );
    let result = result.unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), result.get_span()).without_spans()
    );
}

#[test]
fn test_let_syntax_allows_multiple_transformer_bindings() {
    let result = expand_source(
        r#"
        (let-syntax
          ((one (syntax-rules () ((_) 1)))
           (two (syntax-rules () ((_) 2))))
          (two))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected multi-binding let-syntax to expand, got: {:?}",
        result
    );
    let result = result.unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(2.0), result.get_span()).without_spans()
    );
}

#[test]
fn test_letrec_syntax_bindings_are_recursive() {
    let result = expand_source(
        r#"
        (letrec-syntax
          ((one (syntax-rules () ((_) 1)))
           (two (syntax-rules () ((_) (one)))))
          (two))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected letrec-syntax to expand, got: {:?}",
        result
    );
    let result = result.unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), result.get_span()).without_spans(),
        "Expected letrec-syntax bindings to be recursive: (one) inside two's template should expand to 1"
    );
}

#[test]
fn test_let_syntax_body_expansion() {
    let result = expand_source(
        r#"
        (list
          (let-syntax
            ((one (syntax-rules ()
                    ((_ x) x))))
            (define x 1)
            x))
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected let-syntax body defines to expand in body context, got: {:?}",
        result
    );
}

// --- Stateful session tests ---

#[test]
fn test_expand_top_level_begin_define_persists_binding_for_following_expand() {
    let mut session = Session::new();

    let tokens = session.tokenize("(begin (define x 1) x)").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let first_result = session.expand(&introduced);
    assert!(
        first_result.is_ok(),
        "Expected top-level begin with define to expand successfully"
    );

    let tokens = session.tokenize("x").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let second_result = session.expand(&introduced);
    assert!(
        second_result.is_ok(),
        "Expected identifier defined inside top-level begin to remain bound for later expansion"
    );
}

#[test]
fn test_expand_successful_expansion_persists_bindings() {
    let mut session = Session::new();

    let tokens = session.tokenize("(define x 1)").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let result = session.expand(&introduced);
    assert!(result.is_ok());

    let tokens = session.tokenize("x").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let result = session.expand(&introduced);
    assert!(result.is_ok());
}

#[test]
fn test_expand_define_syntax_basic() {
    let mut session = Session::new();

    let tokens = session
        .tokenize(
            r#"
            (define-syntax one
              (syntax-rules ()
                ((_) 1)))
            "#,
        )
        .unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let result = session.expand(&introduced);
    assert!(
        result.is_ok(),
        "Expected define-syntax to expand, got: {:?}",
        result
    );

    let tokens = session.tokenize("(one)").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let result = session.expand(&introduced).unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), result.get_span()).without_spans()
    );
}

#[test]
fn test_expand_define_syntax_multiple_definitions() {
    let mut session = Session::new();

    let tokens = session
        .tokenize(
            r#"
            (define-syntax one
              (syntax-rules ()
                ((_) 1)))
            "#,
        )
        .unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    session.expand(&introduced).unwrap();

    let tokens = session
        .tokenize(
            r#"
            (define-syntax two
              (syntax-rules ()
                ((_) 2)))
            "#,
        )
        .unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    session.expand(&introduced).unwrap();

    let tokens = session.tokenize("(list (one) (two))").unwrap();
    let parsed = session.parse(&tokens).unwrap();
    let introduced = session.introduce(&parsed);
    let result = session.expand(&introduced).unwrap();

    let span = Span { lo: 0, hi: 0 };
    let args = rest(&result);
    assert_eq!(
        first(&args).without_spans(),
        SExpr::Num(Num(1.0), span).without_spans()
    );
    assert_eq!(
        first(&rest(&args)).without_spans(),
        SExpr::Num(Num(2.0), span).without_spans()
    );
}

// --- Tests that were missed in the first migration pass ---

#[test]
fn test_expand_let_syntax_or_macro_0_arg_maintains_span() {
    let result = expand_source(
        r#"
                (letrec-syntax
                  ((or (syntax-rules ()
                            ((_) #f)
                            ((_ e) e)
                            ((_ e1 e2 ...)
                             ((lambda (temp)
                               (if temp
                                  temp
                                   (or e2 ...)))
                              e1)))))
                   (or))
                "#,
    )
    .unwrap();
    let expected = SExpr::Bool(
        rrrrr_rs::compile::sexpr::Bool(false),
        Span { lo: 105, hi: 107 },
    );

    assert!(
        result == expected,
        "result: {:?}\nexpected: {:?}",
        result,
        expected
    );
}

#[test]
fn test_expand_let_syntax_or_macro_1_arg_maintains_span() {
    let result = expand_source(
        r#"
                (letrec-syntax
                  ((or (syntax-rules ()
                            ((_) #f)
                            ((_ e) e)
                            ((_ e1 e2 ...)
                             ((lambda (temp)
                               (if temp
                                  temp
                                   (or e2 ...)))
                              e1)))))
                   (or 1))
                "#,
    )
    .unwrap();
    let expected = SExpr::Num(Num(1.0), Span { lo: 424, hi: 425 });

    assert!(
        result == expected,
        "result: {:?}\nexpected: {:?}",
        result,
        expected
    );
}

#[test]
fn test_literal_matching_respects_lexical_binding() {
    let result = expand_source(
        r#"
                (letrec-syntax
                  ((my-mac (syntax-rules (list)
                             ((_ list) 1)
                             ((_ x) 2))))
                  (lambda (list) (my-mac list)))
                "#,
    )
    .unwrap();

    let body = nth(&result, 2).unwrap();
    assert_eq!(
        body.without_spans(),
        SExpr::Num(Num(2.0), body.get_span()).without_spans()
    );
}

// --- Tests using resolve_sym on expansion output ---

#[test]
fn test_expand_lambda_internal_define_inside_begin() {
    let (session, result) = expand_with_session("(lambda () (begin (define x 1) x))");
    let result = result.unwrap();

    let defined_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let last_body_expr = nth(&result, 3).unwrap();
    assert!(
        nth(&result, 4).is_none(),
        "Expected begin to be spliced into lambda body"
    );

    let SExpr::Id(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(last_body_expr, _) = last_body_expr else {
        panic!("Expected final body expression to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&last_body_expr).unwrap(),
        "Expected body reference to resolve to internal define from spliced begin"
    );
}

#[test]
fn test_expand_lambda_shadowed_begin_is_not_spliced() {
    let (session, result) =
        expand_with_session("(lambda () (define begin (lambda x x)) (begin 1 2 3))");
    let result = result.unwrap();

    let defined_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let begin_call = nth(&result, 3).unwrap();
    let begin_head = first(&begin_call);
    assert!(
        nth(&result, 4).is_none(),
        "Expected shadowed begin call to remain as a single body form"
    );

    let SExpr::Id(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(begin_head, _) = begin_head else {
        panic!("Expected begin call head to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&begin_head).unwrap(),
        "Expected begin call to resolve to shadowing local binding"
    );
}

#[test]
fn test_expand_lambda_begin_binding_defined_inside_spliced_begin_shadows_nested_begin() {
    let (session, result) =
        expand_with_session("(lambda () (begin (define begin (lambda x x)) (begin 1 2)))");
    let result = result.unwrap();

    let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let nested_begin_call = nth(&result, 3).unwrap();
    let nested_begin_head = first(&nested_begin_call);
    assert!(
        nth(&result, 4).is_none(),
        "Expected begin wrapper to splice and keep nested begin call as a single form"
    );

    let SExpr::Id(define_begin_var, _) = define_begin_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(nested_begin_head, _) = nested_begin_head else {
        panic!("Expected nested begin call head to be an identifier");
    };
    let define_sym = session.resolve_sym(&define_begin_var).unwrap();
    let nested_head_sym = session.resolve_sym(&nested_begin_head).unwrap();
    assert_eq!(
        define_sym, nested_head_sym,
        "Expected nested begin call to resolve to locally-defined begin"
    );
    assert_ne!(
        define_sym,
        Symbol::new("begin"),
        "Expected nested begin call not to resolve to core begin"
    );
}

#[test]
fn test_expand_lambda_begin_binding_defined_in_begin_group_shadows_following_begin_form() {
    let (session, result) =
        expand_with_session("(lambda () (begin (define begin (lambda x x))) (begin 1 2))");
    let result = result.unwrap();

    let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let following_begin_call = nth(&result, 3).unwrap();
    let following_begin_head = first(&following_begin_call);
    assert!(
        nth(&result, 4).is_none(),
        "Expected following begin to remain a call form after begin is rebound"
    );

    let SExpr::Id(define_begin_var, _) = define_begin_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(following_begin_head, _) = following_begin_head else {
        panic!("Expected begin call head to be an identifier");
    };
    let define_sym = session.resolve_sym(&define_begin_var).unwrap();
    let following_head_sym = session.resolve_sym(&following_begin_head).unwrap();
    assert_eq!(
        define_sym, following_head_sym,
        "Expected following begin form to resolve to locally-defined begin"
    );
    assert_ne!(
        define_sym,
        Symbol::new("begin"),
        "Expected following begin form not to resolve to core begin"
    );
}

#[test]
fn test_expand_lambda_rebound_begin_reference_and_call_share_local_binding() {
    let (session, result) =
        expand_with_session("(lambda () (begin (define begin (lambda x x)) begin (begin 1 2)))");
    let result = result.unwrap();

    let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
    let begin_reference = nth(&result, 3).unwrap();
    let begin_call = nth(&result, 4).unwrap();
    let begin_call_head = first(&begin_call);
    assert!(
        nth(&result, 5).is_none(),
        "Expected body to contain define, begin reference, and begin call"
    );

    let SExpr::Id(define_begin_var, _) = define_begin_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Id(begin_reference, _) = begin_reference else {
        panic!("Expected begin reference to be an identifier");
    };
    let SExpr::Id(begin_call_head, _) = begin_call_head else {
        panic!("Expected begin call head to be an identifier");
    };
    let define_sym = session.resolve_sym(&define_begin_var).unwrap();
    let reference_sym = session.resolve_sym(&begin_reference).unwrap();
    let call_head_sym = session.resolve_sym(&begin_call_head).unwrap();
    assert_eq!(
        define_sym, reference_sym,
        "Expected begin reference to resolve to locally-defined begin"
    );
    assert_eq!(
        define_sym, call_head_sym,
        "Expected begin call head to resolve to locally-defined begin"
    );
    assert_ne!(
        define_sym,
        Symbol::new("begin"),
        "Expected rebound begin usages not to resolve to core begin"
    );
}

#[test]
fn test_expand_lambda_define_after_spliced_begin_is_collected() {
    let (session, result) = expand_with_session("(lambda () (begin (define x 1)) (define y 2) y)");
    assert!(
        result.is_ok(),
        "Expected define after leading begin to be normalized and collected"
    );
    let result = result.unwrap();

    let defined_var_y = nth(&nth(&result, 3).unwrap(), 1).unwrap();
    let final_expr = nth(&result, 4).unwrap();
    assert!(
        nth(&result, 5).is_none(),
        "Expected exactly 3 body forms after expansion"
    );

    let SExpr::Id(defined_var_y, _) = defined_var_y else {
        panic!("Expected second define variable to be an identifier");
    };
    let SExpr::Id(final_expr, _) = final_expr else {
        panic!("Expected final body expression to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var_y).unwrap(),
        session.resolve_sym(&final_expr).unwrap(),
        "Expected final y reference to resolve to collected internal define"
    );
}

#[test]
fn test_expand_lambda_multiple_begin_define_groups_stay_in_define_phase() {
    let (session, result) =
        expand_with_session("(lambda () (begin (define x 1)) (begin (define y 2)) y)");
    assert!(
        result.is_ok(),
        "Expected subsequent begin-wrapped defines to remain in define phase"
    );
    let result = result.unwrap();

    let defined_var_y = nth(&nth(&result, 3).unwrap(), 1).unwrap();
    let final_expr = nth(&result, 4).unwrap();
    assert!(
        nth(&result, 5).is_none(),
        "Expected exactly 3 body forms after expansion"
    );

    let SExpr::Id(defined_var_y, _) = defined_var_y else {
        panic!("Expected second define variable to be an identifier");
    };
    let SExpr::Id(final_expr, _) = final_expr else {
        panic!("Expected final body expression to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var_y).unwrap(),
        session.resolve_sym(&final_expr).unwrap(),
        "Expected final y reference to resolve to second internal define"
    );
}

#[test]
fn test_expand_lambda_macro_expanding_to_define() {
    assert_generated_define_is_referenced(
        r#"
            (letrec-syntax
              ((def (syntax-rules ()
                      ((_ x v) (define x v)))))
              (lambda () (def y 42) y))
            "#,
        "Expected macro expanding to define to work in lambda body",
    );
}

#[test]
fn test_expand_lambda_macro_expanding_to_begin_with_define() {
    assert_generated_define_is_referenced(
        r#"
            (letrec-syntax
              ((def-begin (syntax-rules ()
                            ((_ x v) (begin (define x v))))))
              (lambda () (def-begin y 42) y))
            "#,
        "Expected macro expanding to begin-wrapped define to work",
    );
}

#[test]
fn test_expand_lambda_nested_macro_expanding_to_define() {
    assert_generated_define_is_referenced(
        r#"
            (letrec-syntax
              ((def (syntax-rules ()
                      ((_ x v) (define x v))))
               (def2 (syntax-rules ()
                       ((_ x v) (def x v)))))
              (lambda () (def2 y 42) y))
            "#,
        "Expected chained macros expanding to define to work",
    );
}

// --- define-syntax output tests ---

#[test]
fn test_expand_define_syntax_with_pattern_variable() {
    let mut session = Session::new();

    session_expand(
        &mut session,
        r#"
                (define-syntax double
                  (syntax-rules ()
                    ((_ x) (list x x))))
                "#,
    )
    .unwrap();

    let result = session_expand(&mut session, "(double 5)").unwrap();

    let list_id = first(&result);
    assert!(
        matches!(&list_id, SExpr::Id(id, _) if session.resolve_sym(id) == Some(Symbol::new("list")))
    );
    assert_eq!(
        nth(&result, 1).unwrap().without_spans(),
        SExpr::Num(Num(5.0), Span { lo: 0, hi: 0 }).without_spans()
    );
    assert_eq!(
        nth(&result, 2).unwrap().without_spans(),
        SExpr::Num(Num(5.0), Span { lo: 0, hi: 0 }).without_spans()
    );
}

#[test]
fn test_expand_define_syntax_with_ellipsis() {
    let mut session = Session::new();

    session_expand(
        &mut session,
        r#"
                (define-syntax my-list
                  (syntax-rules ()
                    ((_ x ...) (list x ...))))
                "#,
    )
    .unwrap();

    let result = session_expand(&mut session, "(my-list 1 2 3)").unwrap();
    let span = Span { lo: 0, hi: 0 };

    let list_id = first(&result);
    assert!(
        matches!(&list_id, SExpr::Id(id, _) if session.resolve_sym(id) == Some(Symbol::new("list")))
    );
    assert_eq!(
        nth(&result, 1).unwrap().without_spans(),
        SExpr::Num(Num(1.0), span).without_spans()
    );
    assert_eq!(
        nth(&result, 2).unwrap().without_spans(),
        SExpr::Num(Num(2.0), span).without_spans()
    );
    assert_eq!(
        nth(&result, 3).unwrap().without_spans(),
        SExpr::Num(Num(3.0), span).without_spans()
    );
}
