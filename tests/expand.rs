use rrrrr_rs::{
    Session,
    compile::{
        compilation_error::{CompilationError, Result},
        sexpr::{Id, Num, SExpr, Symbol},
        span::Span,
        util::{first, rest, try_nth},
    },
};

fn expand_source(source: &str) -> Result<SExpr<Id>> {
    let mut session = Session::new();
    let tokens = session.tokenize(source)?;
    let parsed = session.parse(&tokens)?.pop().unwrap();
    let introduced = session.introduce(parsed);
    session.expand(introduced)
}

fn expand_with_session(source: &str) -> (Session, Result<SExpr<Id>>) {
    let mut session = Session::new();
    let result = (|| {
        let tokens = session.tokenize(source)?;
        let parsed = session.parse(&tokens)?.pop().unwrap();
        let introduced = session.introduce(parsed);
        session.expand(introduced)
    })();
    (session, result)
}

fn session_expand(session: &mut Session, source: &str) -> Result<SExpr<Id>> {
    let tokens = session.tokenize(source)?;
    let parsed = session.parse(&tokens)?.pop().unwrap();
    let introduced = session.introduce(parsed);
    session.expand(introduced)
}

fn assert_generated_define_is_referenced(source: &str, expand_message: &str) {
    let (session, result) = expand_with_session(source);
    assert!(result.is_ok(), "{expand_message}, got: {:?}", result);
    let result = result.unwrap();
    // Body is now a letrec: (lambda () (letrec ((var init)) body))
    let letrec = try_nth(&result, 2).unwrap();
    let first_init = first(&try_nth(&letrec, 1).unwrap());
    let defined_var = first(&first_init);
    let body_ref = try_nth(&letrec, 2).unwrap();
    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body reference to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to generated define"
    );
}

#[test]
fn test_expand_lambda_invalid_non_id_param() {
    assert!(expand_source("(lambda 42 x)").is_err());
}

#[test]
fn test_expand_lambda_invalid_dotted_param() {
    assert!(expand_source("(lambda (x . 42) x)").is_err());
}

#[test]
fn test_expand_lambda_rejects_duplicate_params() {
    assert!(matches!(
        expand_source("(lambda (x x) x)"),
        Err(CompilationError { reason, .. }) if reason == "Duplicate parameter: 'x'"
    ));
}

#[test]
fn test_expand_lambda_rejects_duplicate_rest_param() {
    assert!(matches!(
        expand_source("(lambda (x . x) x)"),
        Err(CompilationError { reason, .. }) if reason == "Duplicate parameter: 'x'"
    ));
}

#[test]
fn test_expand_lambda_requires_body_expression() {
    assert!(matches!(
        expand_source("(lambda (x))"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid body: expected at least one body expression"
    ));
}

#[test]
fn test_expand_lambda_requires_expression_after_internal_definitions() {
    assert!(matches!(
        expand_source("(lambda () (define x 1))"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid body: expected at least one expression after definitions"
    ));
}

#[test]
fn test_expand_top_level_unbound_id_is_allowed() {
    assert!(
        expand_source("x").is_ok(),
        "Expected unbound identifier at top level to be allowed"
    );
}

#[test]
fn test_expand_set_unbound_identifier_is_allowed() {
    assert!(
        expand_source("(set! x 1)").is_ok(),
        "Expected set! on unbound identifier to be allowed"
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
fn test_expand_begin_preserves_outer_form_span() {
    let result = expand_source("(begin 1 2)").unwrap();

    assert_eq!(result.get_span(), Span { lo: 0, hi: 11 });
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
        result.is_ok(),
        "Expected let-syntax with non-recursive reference to be allowed (scoping still non-recursive, just not enforced at expansion time)"
    );
}

#[test]
fn test_expand_let_syntax_rejects_improper_binding_list() {
    assert!(matches!(
        expand_source("(let-syntax ((one (syntax-rules () ((_) 1))) . 42) (one))"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid 'let-syntax' bindings: expected a proper list"
    ));
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

#[test]
fn test_expand_define_syntax_basic() {
    let mut session = Session::new();

    let result = session_expand(
        &mut session,
        r#"
            (define-syntax one
              (syntax-rules ()
                ((_) 1)))
            "#,
    );
    assert!(
        result.is_ok(),
        "Expected define-syntax to expand, got: {:?}",
        result
    );

    let result = session_expand(&mut session, "(one)").unwrap();
    assert_eq!(
        result.without_spans(),
        SExpr::Num(Num(1.0), result.get_span()).without_spans()
    );
}

#[test]
fn test_expand_define_syntax_returns_void() {
    let mut session = Session::new();

    let result = session_expand(
        &mut session,
        r#"
        (define-syntax one
          (syntax-rules ()
            ((_) 1)))
        "#,
    )
    .unwrap();

    assert_eq!(
        result.without_spans(),
        SExpr::Void(Span { lo: 0, hi: 0 }).without_spans()
    );
}

#[test]
fn test_expand_define_syntax_multiple_definitions() {
    let mut session = Session::new();

    session_expand(
        &mut session,
        r#"
            (define-syntax one
              (syntax-rules ()
                ((_) 1)))
            "#,
    )
    .unwrap();

    session_expand(
        &mut session,
        r#"
            (define-syntax two
              (syntax-rules ()
                ((_) 2)))
            "#,
    )
    .unwrap();

    let result = session_expand(&mut session, "(list (one) (two))").unwrap();

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

    let body = try_nth(&result, 2).unwrap();
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

    // Body is now: (lambda () (letrec ((x 1)) x))
    let letrec = try_nth(&result, 2).unwrap();
    assert!(
        try_nth(&result, 3).is_none(),
        "Expected begin to be spliced and desugared into a single letrec"
    );
    let first_init = first(&try_nth(&letrec, 1).unwrap());
    let defined_var = first(&first_init);
    let last_body_expr = try_nth(&letrec, 2).unwrap();

    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Var(last_body_expr, _) = last_body_expr else {
        panic!("Expected final body expression to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&last_body_expr).unwrap(),
        "Expected body reference to resolve to internal define from spliced begin"
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

    // Body is now: (lambda () (letrec ((x 1) (y 2)) y))
    let letrec = try_nth(&result, 2).unwrap();
    assert!(
        try_nth(&result, 3).is_none(),
        "Expected body to be a single letrec form"
    );
    let initializers = try_nth(&letrec, 1).unwrap();
    let second_init = try_nth(&initializers, 1).unwrap();
    let defined_var_y = first(&second_init);
    let final_expr = try_nth(&letrec, 2).unwrap();

    let SExpr::Var(defined_var_y, _) = defined_var_y else {
        panic!("Expected second define variable to be an identifier");
    };
    let SExpr::Var(final_expr, _) = final_expr else {
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

    // Body is now: (lambda () (letrec ((x 1) (y 2)) y))
    let letrec = try_nth(&result, 2).unwrap();
    assert!(
        try_nth(&result, 3).is_none(),
        "Expected body to be a single letrec form"
    );
    let initializers = try_nth(&letrec, 1).unwrap();
    let second_init = try_nth(&initializers, 1).unwrap();
    let defined_var_y = first(&second_init);
    let final_expr = try_nth(&letrec, 2).unwrap();

    let SExpr::Var(defined_var_y, _) = defined_var_y else {
        panic!("Expected second define variable to be an identifier");
    };
    let SExpr::Var(final_expr, _) = final_expr else {
        panic!("Expected final body expression to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var_y).unwrap(),
        session.resolve_sym(&final_expr).unwrap(),
        "Expected final y reference to resolve to second internal define"
    );
}

#[test]
fn test_expand_lambda_local_binding_shadows_transformer_after_body_boundary() {
    let (session, result) = expand_with_session(
        r#"
        (let-syntax
          ((m (syntax-rules () ((_) 1))))
          (lambda ()
            (define m (lambda () 2))
            0
            (m)))
        "#,
    );
    let result = result.unwrap();

    let letrec = try_nth(&result, 2).unwrap();
    let init = first(&try_nth(&letrec, 1).unwrap());
    let defined_var = first(&init);
    let final_expr = try_nth(&letrec, 3).unwrap();
    let call_head = first(&final_expr);

    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected internal define variable to be an identifier");
    };
    let SExpr::Var(call_head, _) = call_head else {
        panic!("Expected final expression to remain a function application");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&call_head).unwrap(),
        "Expected post-boundary call to resolve to the local binding"
    );
}

#[test]
fn test_expand_lambda_local_binding_shadows_begin_after_body_boundary() {
    let (session, result) =
        expand_with_session("(lambda () (define begin (lambda x x)) 0 (begin 1 2))");
    let result = result.unwrap();

    let letrec = try_nth(&result, 2).unwrap();
    let init = first(&try_nth(&letrec, 1).unwrap());
    let defined_var = first(&init);
    let final_expr = try_nth(&letrec, 3).unwrap();
    let call_head = first(&final_expr);

    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected internal define variable to be an identifier");
    };
    let SExpr::Var(call_head, _) = call_head else {
        panic!("Expected shadowed begin to remain a function application");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&call_head).unwrap(),
        "Expected post-boundary begin form to resolve to the local binding"
    );
}

#[test]
fn test_expand_lambda_macro_expanding_to_define_after_body_boundary_is_rejected() {
    let result = expand_source(
        r#"
        (let-syntax
          ((def (syntax-rules ()
                  ((_ x v) (define x v)))))
          (lambda ()
            0
            (def y 42)
            y))
        "#,
    );
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason == "'define' must appear at the beginning of a body"
        ),
        "Expected macro-generated define after body boundary to be rejected, got: {:?}",
        result
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
        matches!(&list_id, SExpr::Var(id, _) if session.resolve_sym(id) == Some(Symbol::new("list")))
    );
    assert_eq!(
        try_nth(&result, 1).unwrap().without_spans(),
        SExpr::Num(Num(5.0), Span { lo: 0, hi: 0 }).without_spans()
    );
    assert_eq!(
        try_nth(&result, 2).unwrap().without_spans(),
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
        matches!(&list_id, SExpr::Var(id, _) if session.resolve_sym(id) == Some(Symbol::new("list")))
    );
    assert_eq!(
        try_nth(&result, 1).unwrap().without_spans(),
        SExpr::Num(Num(1.0), span).without_spans()
    );
    assert_eq!(
        try_nth(&result, 2).unwrap().without_spans(),
        SExpr::Num(Num(2.0), span).without_spans()
    );
    assert_eq!(
        try_nth(&result, 3).unwrap().without_spans(),
        SExpr::Num(Num(3.0), span).without_spans()
    );
}

// --- Quasiquote tests ---

#[test]
fn test_quasiquote_constant_list() {
    let result = expand_source("`(1 2 3)").unwrap();
    assert_eq!(
        format!("{result}"),
        "(append (quote (1)) (append (quote (2)) (append (quote (3)) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_empty_list() {
    let result = expand_source("`()").unwrap();
    assert_eq!(format!("{result}"), "(quote ())");
}

#[test]
fn test_quasiquote_atom_number() {
    let result = expand_source("`42").unwrap();
    assert_eq!(format!("{result}"), "(quote 42)");
}

#[test]
fn test_quasiquote_atom_symbol() {
    let result = expand_source("`hello").unwrap();
    assert_eq!(format!("{result}"), "(quote hello)");
}

#[test]
fn test_quasiquote_with_unquote() {
    let result = expand_source("(lambda (x) `(1 ,x 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (append (quote (1)) (append (list x) (append (quote (3)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_unquote_first_position() {
    let result = expand_source("(lambda (x) `(,x 2 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (append (list x) (append (quote (2)) (append (quote (3)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_multiple_unquotes() {
    let result = expand_source("(lambda (a b) `(,a ,b))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (a b) (append (list a) (append (list b) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_unquote_splicing() {
    let result = expand_source("(lambda (xs) `(1 ,@xs 5))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (append (quote (1)) (append (append xs) (append (quote (5)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_unquote_splicing_first_position() {
    let result = expand_source("(lambda (xs) `(,@xs 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (append (append xs) (append (quote (3)) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_unquote_splicing_only_element() {
    let result = expand_source("(lambda (xs) `(,@xs))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (append (append xs) (quote ())))"
    );
}

#[test]
fn test_quasiquote_mixed_unquote_and_splicing() {
    let result = expand_source("(lambda (x ys) `(,x ,@ys 4))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x ys) (append (list x) (append (append ys) (append (quote (4)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_nested_list_with_unquote() {
    let result = expand_source("(lambda (x) `((,x 2) 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (append (list (append (list x) (append (quote (2)) (quote ())))) (append (quote (3)) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_mixed_types() {
    let result = expand_source(r#"`(#t "hello" 42)"#).unwrap();
    assert_eq!(
        format!("{result}"),
        r#"(append (quote (#t)) (append (quote ("hello")) (append (quote (42)) (quote ()))))"#
    );
}

#[test]
fn test_quasiquote_nested_preserves_inner_quasiquote() {
    let result = expand_source("(lambda (x) `(a `(b ,,x)))").unwrap();
    // The inner quasiquote stays as syntax; only the outer ,x is expanded
    let output = format!("{result}");
    assert!(
        output.contains("(quote quasiquote)"),
        "Nested quasiquote should preserve inner quasiquote keyword"
    );
    assert!(
        output.contains("(quote unquote)"),
        "Nested quasiquote should preserve inner unquote keyword"
    );
}

#[test]
fn test_quasiquote_in_macro_template() {
    let result = expand_source(
        r#"
        (letrec-syntax ((make-pair (syntax-rules ()
                                     ((_ a b) `(,a ,b)))))
          (make-pair 1 2))
        "#,
    )
    .unwrap();
    assert_eq!(
        format!("{result}"),
        "(append (list 1) (append (list 2) (quote ())))"
    );
}

// --- Vector quasiquote tests ---

#[test]
fn test_quasiquote_constant_vector() {
    let result = expand_source("`#(1 2 3)").unwrap();
    assert_eq!(
        format!("{result}"),
        "(list->vector (append (quote (1)) (append (quote (2)) (append (quote (3)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_empty_vector() {
    let result = expand_source("`#()").unwrap();
    assert_eq!(format!("{result}"), "(list->vector (quote ()))");
}

#[test]
fn test_quasiquote_vector_mixed_types() {
    let result = expand_source(r#"`#(#t "hello" 42)"#).unwrap();
    assert_eq!(
        format!("{result}"),
        r#"(list->vector (append (quote (#t)) (append (quote ("hello")) (append (quote (42)) (quote ())))))"#
    );
}

#[test]
fn test_quasiquote_vector_with_unquote() {
    let result = expand_source("(lambda (x) `#(1 ,x 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (list->vector (append (quote (1)) (append (list x) (append (quote (3)) (quote ()))))))"
    );
}

#[test]
fn test_quasiquote_vector_multiple_unquotes() {
    let result = expand_source("(lambda (x y) `#(,x ,y))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x y) (list->vector (append (list x) (append (list y) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_vector_with_unquote_splicing() {
    let result = expand_source("(lambda (xs) `#(1 ,@xs 4))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (list->vector (append (quote (1)) (append (append xs) (append (quote (4)) (quote ()))))))"
    );
}

#[test]
fn test_quasiquote_vector_unquote_only_element() {
    let result = expand_source("(lambda (x) `#(,x))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (list->vector (append (list x) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_vector_splice_only_element() {
    let result = expand_source("(lambda (xs) `#(,@xs))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (list->vector (append (append xs) (quote ()))))"
    );
}

#[test]
fn test_quasiquote_vector_mixed_unquote_and_splicing() {
    let result = expand_source("(lambda (x ys) `#(,x ,@ys 4))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x ys) (list->vector (append (list x) (append (append ys) (append (quote (4)) (quote ()))))))"
    );
}

#[test]
fn test_quasiquote_vector_nested_in_list() {
    let result = expand_source("(lambda (x) `(a #(1 ,x 3) b))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (append (quote (a)) (append (list (list->vector (append (quote (1)) (append (list x) (append (quote (3)) (quote ())))))) (append (quote (b)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_vector_with_splicing_nested_in_list() {
    let result = expand_source("(lambda (xs) `(a #(1 ,@xs 4) b))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (xs) (append (quote (a)) (append (list (list->vector (append (quote (1)) (append (append xs) (append (quote (4)) (quote ())))))) (append (quote (b)) (quote ())))))"
    );
}

#[test]
fn test_quasiquote_vector_nested_in_vector() {
    let result = expand_source("(lambda (x) `#(#(1 ,x) 3))").unwrap();
    assert_eq!(
        format!("{result}"),
        "(lambda (x) (list->vector (append (list (list->vector (append (quote (1)) (append (list x) (quote ()))))) (append (quote (3)) (quote ())))))"
    );
}

// --- Quasiquote error tests ---

#[test]
fn test_unquote_outside_quasiquote_is_error() {
    let result = expand_source(",x");
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason == "Invalid 'unquote' form: not in 'quasiquote'"
        ),
        "Expected unquote outside quasiquote to be an error, got: {:?}",
        result
    );
}

#[test]
fn test_unquote_splicing_outside_quasiquote_is_error() {
    let result = expand_source(",@x");
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason == "Invalid 'unquote-splicing' form: not in 'quasiquote'"
        ),
        "Expected unquote-splicing outside quasiquote to be an error, got: {:?}",
        result
    );
}

#[test]
fn test_expand_define_function_shorthand() {
    assert_eq!(
        session_expand(&mut Session::new(), "(define (foo x) x)")
            .unwrap()
            .without_spans(),
        session_expand(&mut Session::new(), "(define foo (lambda (x) x))")
            .unwrap()
            .without_spans(),
    );
}

#[test]
fn test_expand_define_function_shorthand_dotted_pair() {
    assert_eq!(
        session_expand(&mut Session::new(), "(define (foo . x) x)")
            .unwrap()
            .without_spans(),
        session_expand(&mut Session::new(), "(define foo (lambda x x))")
            .unwrap()
            .without_spans(),
    );
}

#[test]
fn test_expand_define_function_shorthand_no_args() {
    assert_eq!(
        session_expand(&mut Session::new(), "(define (foo) 1)")
            .unwrap()
            .without_spans(),
        session_expand(&mut Session::new(), "(define foo (lambda () 1))")
            .unwrap()
            .without_spans(),
    );
}

#[test]
fn test_expand_define_function_shorthand_non_id_errors() {
    assert!(expand_source("(define (42 x) x)").is_err());
}

#[test]
fn test_expand_define_function_shorthand_in_expression_context_errors() {
    assert!(matches!(
        expand_source("(define x (define (foo) 1))"),
        Err(CompilationError { reason, .. })
            if reason == "'define' is not allowed in an expression context"
    ));
}

#[test]
fn test_expand_define_function_shorthand_duplicate_errors() {
    assert!(expand_source("(lambda () (define (foo x) x) (define (foo y) y) (foo 1))").is_err());
}

#[test]
fn test_expand_lambda_internal_define_function_shorthand() {
    let (session, result) = expand_with_session("(lambda () (define (foo x) x) (foo 1))");
    let result = result.unwrap();

    // Body is now: (lambda () (letrec ((foo (lambda (x) x))) (foo 1)))
    let letrec = try_nth(&result, 2).unwrap();
    let first_init = first(&try_nth(&letrec, 1).unwrap());
    let defined_var = first(&first_init);
    let body_ref = first(&try_nth(&letrec, 2).unwrap());

    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define variable to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body function reference to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to function shorthand define"
    );
}

#[test]
fn test_expand_macro_expansion_depth_limit() {
    assert!(matches!(
        expand_source(
"
(letrec-syntax
    ((foo (syntax-rules ()
            ((_) (foo)))))
  (foo))
"),
        Err(CompilationError { reason, .. })
            if reason == "Macro expansion depth limit exceeded (1024) while expanding 'foo'"
    ));
}

#[test]
fn test_expand_macro_expansion_mutual_depth_limit() {
    assert!(matches!(
        expand_source(
"
(letrec-syntax
    ((foo (syntax-rules ()
            ((_) (bar))))
     (bar (syntax-rules ()
            ((_) (foo)))))
  (foo))
"),
        Err(CompilationError { reason, .. })
            if reason == "Macro expansion depth limit exceeded (1024) while expanding 'bar'"
    ));
}

#[test]
fn test_expand_macro_expansion_depth_limit_via_body() {
    // Each depth level adds many Rust frames (lambda wrapping), so use a larger stack.
    let result = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            expand_source(
                "
(letrec-syntax
    ((loop (syntax-rules ()
             ((_) ((lambda () (loop)))))))
  (loop))
",
            )
        })
        .unwrap()
        .join()
        .unwrap();

    assert!(matches!(
        result,
        Err(CompilationError { reason, .. })
            if reason == "Macro expansion depth limit exceeded (1024) while expanding 'loop'"
    ));
}

// --- Core letrec form tests ---

#[test]
fn test_expand_letrec_basic() {
    let result = expand_source("(letrec ((x 1)) x)");
    assert!(
        result.is_ok(),
        "Expected basic letrec to expand, got: {:?}",
        result
    );
}

#[test]
fn test_expand_letrec_bindings_are_visible_in_body() {
    let (session, result) = expand_with_session("(letrec ((x 1)) x)");
    assert!(
        result.is_ok(),
        "Expected letrec to expand, got: {:?}",
        result
    );
    let result = result.unwrap();

    let initializers = try_nth(&result, 1).unwrap();
    let first_init = first(&initializers);
    let bound_var = first(&first_init);
    let body_ref = try_nth(&result, 2).unwrap();
    let SExpr::Var(bound_var, _) = bound_var else {
        panic!("Expected bound variable to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body reference to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&bound_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to the letrec-bound variable"
    );
}

#[test]
fn test_expand_letrec_bindings_are_visible_in_init_expressions() {
    // In letrec, init expressions can see all bindings (for mutual recursion)
    let (session, result) = expand_with_session("(letrec ((f (lambda () g)) (g (lambda () f))) f)");
    assert!(
        result.is_ok(),
        "Expected letrec to expand, got: {:?}",
        result
    );
    let result = result.unwrap();
    // Structure: (letrec ((f' (lambda () g')) (g' (lambda () f'))) f')
    let initializers = try_nth(&result, 1).unwrap();

    // Get the bound variable 'g' from second initializer
    let second_init = first(&rest(&initializers));
    let g_bound = first(&second_init);
    let SExpr::Var(g_bound, _) = g_bound else {
        panic!("Expected g bound var to be an identifier");
    };

    // Get the reference to 'g' inside f's lambda body
    let first_init = first(&initializers);
    let f_lambda = try_nth(&first_init, 1).unwrap(); // (lambda () g')
    let f_lambda_body = try_nth(&f_lambda, 2).unwrap(); // g'
    let SExpr::Var(g_ref, _) = f_lambda_body else {
        panic!("Expected g reference to be an identifier");
    };

    assert_eq!(
        session.resolve_sym(&g_bound).unwrap(),
        session.resolve_sym(&g_ref).unwrap(),
        "Expected reference to 'g' inside f's init to resolve to letrec-bound 'g'"
    );
}

#[test]
fn test_expand_letrec_multiple_bindings() {
    let result = expand_source("(letrec ((x 1) (y 2) (z 3)) (list x y z))");
    assert!(
        result.is_ok(),
        "Expected letrec with multiple bindings to expand, got: {:?}",
        result
    );
}

#[test]
fn test_expand_letrec_empty_bindings() {
    let result = expand_source("(letrec () 1)");
    assert!(
        result.is_ok(),
        "Expected letrec with empty bindings to expand, got: {:?}",
        result
    );
}

#[test]
fn test_expand_letrec_body_with_internal_defines() {
    let result = expand_source("(letrec ((x 1)) (define y 2) y)");
    assert!(
        result.is_ok(),
        "Expected letrec body to allow internal defines, got: {:?}",
        result
    );
}

#[test]
fn test_expand_letrec_rejects_duplicate_bindings() {
    assert!(matches!(
        expand_source("(letrec ((x 1) (x 2)) x)"),
        Err(CompilationError { reason, .. }) if reason == "Duplicate id: 'x'"
    ));
}

#[test]
fn test_expand_letrec_rejects_missing_body() {
    assert!(matches!(
        expand_source("(letrec ((x 1)))"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid body: expected at least one body expression"
    ));
}

#[test]
fn test_expand_letrec_rejects_invalid_initializer_shape() {
    assert!(matches!(
        expand_source("(letrec (x) x)"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid 'letrec' form: expected initializer to be in the form of (var expr)"
    ));
}

#[test]
fn test_expand_letrec_rejects_non_id_in_initializer() {
    assert!(matches!(
        expand_source("(letrec ((42 1)) 1)"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid 'letrec' form: expected initializer to be in the form of (var expr)"
    ));
}

#[test]
fn test_expand_letrec_rejects_define_in_init_expression() {
    assert!(matches!(
        expand_source("(letrec ((x (define y 1))) x)"),
        Err(CompilationError { reason, .. })
            if reason == "'define' is not allowed in an expression context"
    ));
}

#[test]
fn test_expand_letrec_invalid_form() {
    assert!(matches!(
        expand_source("(letrec)"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid 'letrec' form"
    ));
}

#[test]
fn test_expand_letrec_body_only_defines_rejected() {
    assert!(matches!(
        expand_source("(letrec ((x 1)) (define y 2))"),
        Err(CompilationError { reason, .. })
            if reason == "Invalid body: expected at least one expression after definitions"
    ));
}

#[test]
fn test_expand_letrec_used_by_named_let() {
    // Named let desugars to letrec; verify it still works
    let result = expand_source("(let loop ((i 0)) (if i 1 (loop 0)))");
    assert!(
        result.is_ok(),
        "Expected named let (which uses letrec) to expand, got: {:?}",
        result
    );
}

#[test]
fn test_expand_nested_lambda_with_inner_defines() {
    // Outer lambda body has defines lowered to letrec;
    // inner lambda body also has defines lowered to its own letrec
    let (session, result) =
        expand_with_session("(lambda () (define x 1) (lambda () (define y x) y))");
    let result = result.unwrap();

    // Structure: (lambda () (letrec ((x 1)) (lambda () (letrec ((y x')) y'))))
    let outer_letrec = try_nth(&result, 2).unwrap();
    let outer_init = first(&try_nth(&outer_letrec, 1).unwrap());
    let outer_var = first(&outer_init);

    let inner_lambda = try_nth(&outer_letrec, 2).unwrap();
    let inner_letrec = try_nth(&inner_lambda, 2).unwrap();
    let inner_init = first(&try_nth(&inner_letrec, 1).unwrap());
    let inner_var = first(&inner_init);
    let inner_init_expr = try_nth(&inner_init, 1).unwrap();
    let inner_body_ref = try_nth(&inner_letrec, 2).unwrap();

    let SExpr::Var(outer_var, _) = outer_var else {
        panic!("Expected outer define var to be an identifier");
    };
    let SExpr::Var(inner_var, _) = inner_var else {
        panic!("Expected inner define var to be an identifier");
    };
    let SExpr::Var(inner_init_expr, _) = inner_init_expr else {
        panic!("Expected inner init expr to be an identifier");
    };
    let SExpr::Var(inner_body_ref, _) = inner_body_ref else {
        panic!("Expected inner body ref to be an identifier");
    };

    // inner y's init references outer x
    assert_eq!(
        session.resolve_sym(&outer_var).unwrap(),
        session.resolve_sym(&inner_init_expr).unwrap(),
        "Expected inner define init to reference outer define"
    );
    // inner body references inner y
    assert_eq!(
        session.resolve_sym(&inner_var).unwrap(),
        session.resolve_sym(&inner_body_ref).unwrap(),
        "Expected inner body to reference inner define"
    );
}

#[test]
fn test_expand_inner_lambda_with_defines_but_outer_without() {
    // Outer lambda body has no defines (no letrec);
    // inner lambda body does have defines (gets its own letrec)
    let (session, result) = expand_with_session("(lambda () (lambda () (define x 1) x))");
    let result = result.unwrap();

    // Structure: (lambda () (lambda () (letrec ((x 1)) x)))
    assert!(
        try_nth(&result, 3).is_none(),
        "Expected outer lambda to have a single body expression"
    );
    let inner_lambda = try_nth(&result, 2).unwrap();
    let inner_letrec = try_nth(&inner_lambda, 2).unwrap();
    let inner_init = first(&try_nth(&inner_letrec, 1).unwrap());
    let defined_var = first(&inner_init);
    let body_ref = try_nth(&inner_letrec, 2).unwrap();

    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define var to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body ref to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected inner body reference to resolve to inner define"
    );
}

#[test]
fn test_expand_let_syntax_body_with_defines_lowered_to_letrec() {
    // let-syntax body is a body context, so defines should be lowered to letrec
    let (session, result) = expand_with_session(
        r#"
        (let-syntax
          ((one (syntax-rules () ((_) 1))))
          (define x (one))
          x)
        "#,
    );
    let result = result.unwrap();

    // The let-syntax body expands to: (letrec ((x 1)) x)
    // (let-syntax itself disappears, its body is the result)
    let SExpr::Cons(..) = &result else {
        panic!("Expected result to be a list (letrec form)");
    };
    let head = first(&result);
    let SExpr::Var(head_id, _) = head else {
        panic!("Expected result head to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&head_id),
        Some(Symbol::new("letrec")),
        "Expected let-syntax body with defines to lower to letrec"
    );

    let init = first(&try_nth(&result, 1).unwrap());
    let defined_var = first(&init);
    let body_ref = try_nth(&result, 2).unwrap();
    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define var to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body ref to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to define within let-syntax body"
    );
}

#[test]
fn test_expand_letrec_body_with_defines_creates_nested_letrec() {
    // letrec body is a body context; internal defines create a nested letrec
    let (session, result) = expand_with_session("(letrec ((x 1)) (define y x) y)");
    let result = result.unwrap();

    // Structure: (letrec ((x 1)) (letrec ((y x')) y'))
    let outer_init = first(&try_nth(&result, 1).unwrap());
    let outer_var = first(&outer_init);
    let inner_letrec = try_nth(&result, 2).unwrap();

    let inner_head = first(&inner_letrec);
    let SExpr::Var(inner_head_id, _) = inner_head else {
        panic!("Expected inner form to be letrec");
    };
    assert_eq!(
        session.resolve_sym(&inner_head_id),
        Some(Symbol::new("letrec")),
        "Expected inner defines to produce nested letrec"
    );

    let inner_init = first(&try_nth(&inner_letrec, 1).unwrap());
    let inner_var = first(&inner_init);
    let inner_init_expr = try_nth(&inner_init, 1).unwrap();
    let inner_body_ref = try_nth(&inner_letrec, 2).unwrap();

    let SExpr::Var(outer_var, _) = outer_var else {
        panic!("Expected outer var to be an identifier");
    };
    let SExpr::Var(inner_var, _) = inner_var else {
        panic!("Expected inner var to be an identifier");
    };
    let SExpr::Var(inner_init_expr, _) = inner_init_expr else {
        panic!("Expected inner init expr to be an identifier");
    };
    let SExpr::Var(inner_body_ref, _) = inner_body_ref else {
        panic!("Expected inner body ref to be an identifier");
    };

    assert_eq!(
        session.resolve_sym(&outer_var).unwrap(),
        session.resolve_sym(&inner_init_expr).unwrap(),
        "Expected inner define init to reference outer letrec binding"
    );
    assert_eq!(
        session.resolve_sym(&inner_var).unwrap(),
        session.resolve_sym(&inner_body_ref).unwrap(),
        "Expected inner body to reference inner define"
    );
}

#[test]
fn test_expand_multiple_define_function_shorthands_in_body() {
    let (session, result) =
        expand_with_session("(lambda () (define (f x) x) (define (g y) (f y)) (g 1))");
    let result = result.unwrap();

    // Structure: (lambda () (letrec ((f (lambda (x) x)) (g (lambda (y) (f y)))) (g 1)))
    let letrec = try_nth(&result, 2).unwrap();
    let initializers = try_nth(&letrec, 1).unwrap();

    let f_init = first(&initializers);
    let f_var = first(&f_init);
    let f_lambda = try_nth(&f_init, 1).unwrap();

    let g_init = try_nth(&initializers, 1).unwrap();
    let g_var = first(&g_init);
    let g_lambda = try_nth(&g_init, 1).unwrap();

    // f's lambda body references its own param
    let SExpr::Var(f_var_id, _) = &f_var else {
        panic!("Expected f to be an identifier");
    };
    let SExpr::Var(g_var_id, _) = &g_var else {
        panic!("Expected g to be an identifier");
    };
    assert_ne!(
        session.resolve_sym(f_var_id).unwrap(),
        session.resolve_sym(g_var_id).unwrap(),
        "Expected f and g to have distinct bindings"
    );

    // g's lambda body calls f — check f reference resolves to f's define
    let g_body = try_nth(&g_lambda, 2).unwrap(); // (f y)
    let f_ref_in_g = first(&g_body);
    let SExpr::Var(f_ref_in_g, _) = f_ref_in_g else {
        panic!("Expected f reference in g's body to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(f_var_id).unwrap(),
        session.resolve_sym(&f_ref_in_g).unwrap(),
        "Expected g's body to reference f from the same letrec"
    );

    // body expression calls g
    let body_call = try_nth(&letrec, 2).unwrap(); // (g 1)
    let g_ref = first(&body_call);
    let SExpr::Var(g_ref, _) = g_ref else {
        panic!("Expected g reference in body to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(g_var_id).unwrap(),
        session.resolve_sym(&g_ref).unwrap(),
        "Expected body to reference g from the letrec"
    );

    // verify f and g init expressions are lambdas
    let f_head = first(&f_lambda);
    let g_head = first(&g_lambda);
    let SExpr::Var(f_head_id, _) = f_head else {
        panic!("Expected f init to be a lambda");
    };
    let SExpr::Var(g_head_id, _) = g_head else {
        panic!("Expected g init to be a lambda");
    };
    assert_eq!(session.resolve_sym(&f_head_id), Some(Symbol::new("lambda")));
    assert_eq!(session.resolve_sym(&g_head_id), Some(Symbol::new("lambda")));
}

#[test]
fn test_expand_duplicate_define_across_begin_splices_errors() {
    let result = expand_source("(lambda () (begin (define x 1)) (define x 2) x)");
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason.contains("Duplicate definition")
        ),
        "Expected duplicate define across begin splices to be rejected, got: {:?}",
        result
    );
}

#[test]
fn test_expand_duplicate_define_within_single_begin_errors() {
    let result = expand_source("(lambda () (begin (define x 1) (define x 2) x))");
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason.contains("Duplicate definition")
        ),
        "Expected duplicate defines within begin to be rejected, got: {:?}",
        result
    );
}

#[test]
fn test_expand_lambda_with_let_syntax_and_defines_in_body() {
    // let-syntax inside a lambda body expression — the let-syntax itself
    // has its own body with defines
    let (session, result) = expand_with_session(
        r#"
        (lambda ()
          (define x 1)
          (let-syntax
            ((inc (syntax-rules () ((_ v) (+ v 1)))))
            (define y (inc x))
            y))
        "#,
    );
    let result = result.unwrap();

    // Outer: (lambda () (letrec ((x 1)) (let-syntax-body...)))
    let outer_letrec = try_nth(&result, 2).unwrap();
    let outer_init = first(&try_nth(&outer_letrec, 1).unwrap());
    let outer_var = first(&outer_init);

    let SExpr::Var(..) = &outer_var else {
        panic!("Expected outer define var to be an identifier");
    };

    // Inner let-syntax body should produce a nested letrec
    let inner_letrec = try_nth(&outer_letrec, 2).unwrap();
    let inner_head = first(&inner_letrec);
    let SExpr::Var(inner_head_id, _) = inner_head else {
        panic!("Expected inner form head to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&inner_head_id),
        Some(Symbol::new("letrec")),
        "Expected let-syntax body with defines to produce letrec"
    );
}

#[test]
fn test_expand_define_init_is_expanded_in_expression_context() {
    // define's init expression must be in expression context,
    // so nested define inside init should error
    let result = expand_source("(lambda () (define x (define y 1)) x)");
    assert!(
        matches!(
            &result,
            Err(CompilationError { reason, .. })
                if reason == "'define' is not allowed in an expression context"
        ),
        "Expected define in init expression to be rejected, got: {:?}",
        result
    );
}

#[test]
fn test_expand_letrec_lowering_preserves_mutual_recursion() {
    // Classic mutually recursive internal defines
    let (session, result) = expand_with_session(
        r#"
        (lambda ()
          (define (even? n) (if n (odd? n) #t))
          (define (odd? n) (if n (even? n) #f))
          (even? 1))
        "#,
    );
    let result = result.unwrap();

    // Structure: (lambda () (letrec ((even? ...) (odd? ...)) (even? 1)))
    let letrec = try_nth(&result, 2).unwrap();
    let initializers = try_nth(&letrec, 1).unwrap();

    let even_init = first(&initializers);
    let even_var = first(&even_init);
    let odd_init = try_nth(&initializers, 1).unwrap();
    let odd_var = first(&odd_init);

    let SExpr::Var(even_var_id, _) = &even_var else {
        panic!("Expected even? to be an identifier");
    };
    let SExpr::Var(odd_var_id, _) = &odd_var else {
        panic!("Expected odd? to be an identifier");
    };

    // even?'s lambda body should reference odd?
    let even_lambda = try_nth(&even_init, 1).unwrap();
    let even_body = try_nth(&even_lambda, 2).unwrap(); // (if n (odd? n) #t)
    let odd_ref_in_even = first(&try_nth(&even_body, 2).unwrap()); // odd? in (odd? n)
    let SExpr::Var(odd_ref_id, _) = odd_ref_in_even else {
        panic!("Expected odd? reference in even?'s body to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(odd_var_id).unwrap(),
        session.resolve_sym(&odd_ref_id).unwrap(),
        "Expected even?'s body to reference odd? from the same letrec"
    );

    // odd?'s lambda body should reference even?
    let odd_lambda = try_nth(&odd_init, 1).unwrap();
    let odd_body = try_nth(&odd_lambda, 2).unwrap(); // (if n (even? n) #f)
    let even_ref_in_odd = first(&try_nth(&odd_body, 2).unwrap()); // even? in (even? n)
    let SExpr::Var(even_ref_id, _) = even_ref_in_odd else {
        panic!("Expected even? reference in odd?'s body to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(even_var_id).unwrap(),
        session.resolve_sym(&even_ref_id).unwrap(),
        "Expected odd?'s body to reference even? from the same letrec"
    );
}

#[test]
fn test_expand_macro_expanding_to_define_in_let_syntax_body() {
    let (session, result) = expand_with_session(
        r#"
        (let-syntax
          ((def (syntax-rules ()
                  ((_ x v) (define x v)))))
          (def y 42)
          y)
        "#,
    );
    let result = result.unwrap();

    // let-syntax body: (letrec ((y 42)) y)
    let head = first(&result);
    let SExpr::Var(head_id, _) = head else {
        panic!("Expected result head to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&head_id),
        Some(Symbol::new("letrec")),
        "Expected macro-generated define in let-syntax body to produce letrec"
    );

    let init = first(&try_nth(&result, 1).unwrap());
    let defined_var = first(&init);
    let body_ref = try_nth(&result, 2).unwrap();
    let SExpr::Var(defined_var, _) = defined_var else {
        panic!("Expected define var to be an identifier");
    };
    let SExpr::Var(body_ref, _) = body_ref else {
        panic!("Expected body ref to be an identifier");
    };
    assert_eq!(
        session.resolve_sym(&defined_var).unwrap(),
        session.resolve_sym(&body_ref).unwrap(),
        "Expected body reference to resolve to macro-generated define"
    );
}

#[test]
fn test_expand_begin_with_defines_inside_let_syntax_body() {
    let result = expand_source(
        r#"
        (let-syntax
          ((m (syntax-rules () ((_) 1))))
          (begin (define x (m)))
          x)
        "#,
    );
    assert!(
        result.is_ok(),
        "Expected begin with defines inside let-syntax body to expand, got: {:?}",
        result
    );
}

#[test]
fn test_expand_multiple_body_expressions_after_defines() {
    // Multiple expressions after defines — all should be in the letrec body
    let (session, result) = expand_with_session("(lambda () (define x 1) x x x)");
    let result = result.unwrap();

    // Structure: (lambda () (letrec ((x 1)) x x x))
    let letrec = try_nth(&result, 2).unwrap();
    assert!(
        try_nth(&result, 3).is_none(),
        "Expected single letrec form in lambda body"
    );
    // letrec should have initializer list + 3 body expressions
    assert!(try_nth(&letrec, 2).is_some(), "Expected first body expr");
    assert!(try_nth(&letrec, 3).is_some(), "Expected second body expr");
    assert!(try_nth(&letrec, 4).is_some(), "Expected third body expr");
    assert!(try_nth(&letrec, 5).is_none(), "Expected only 3 body exprs");

    // All body exprs should reference x
    let init = first(&try_nth(&letrec, 1).unwrap());
    let x_var = first(&init);
    let SExpr::Var(x_var_id, _) = &x_var else {
        panic!("Expected x to be an identifier");
    };

    for i in 2..=4 {
        let expr = try_nth(&letrec, i).unwrap();
        let SExpr::Var(ref expr_id, _) = expr else {
            panic!("Expected body expr {} to be an identifier", i - 1);
        };
        assert_eq!(
            session.resolve_sym(x_var_id).unwrap(),
            session.resolve_sym(expr_id).unwrap(),
            "Expected body expr {} to reference the defined x",
            i - 1
        );
    }
}

#[test]
fn test_expand_body_with_only_expressions_no_letrec() {
    // A body with no defines should NOT produce a letrec wrapper
    let result = expand_source("(lambda () 1 2 3)").unwrap();

    // Structure: (lambda () 1 2 3) — no letrec wrapper
    let first_expr = try_nth(&result, 2).unwrap();
    assert!(
        matches!(first_expr, SExpr::Num(..)),
        "Expected first body expression to be a number, not a letrec"
    );
    assert!(try_nth(&result, 3).is_some(), "Expected second body expr");
    assert!(try_nth(&result, 4).is_some(), "Expected third body expr");
}
