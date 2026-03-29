use crate::{
    compile::{
        read::{lex::tokenize, parse::parse},
        sexpr::{Bool, Id, Num},
        span::Span,
        util::{first, try_last, try_nth},
    },
    make_sexpr, template_sexpr,
};

fn expand_single_sexpr_src(src: &str, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr<Id>> {
    let sexpr = parse(&tokenize(src).unwrap()).unwrap().pop().unwrap();
    expand(introduce(sexpr), bindings, env)
}

fn introduce_single_sexpr_src(src: &str) -> SExpr<Id> {
    introduce(parse(&tokenize(src).unwrap()).unwrap().pop().unwrap())
}

use super::*;

#[test]
fn test_introduce() {
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        introduce_single_sexpr_src("(cons 0 1)").without_spans(),
        make_sexpr!(
            SExpr::Var(
                Id::new("cons", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            SExpr::Num(Num(0.0), span),
            SExpr::Num(Num(1.0), span),
        )
        .without_spans()
    );
}

#[test]
fn test_expand_lambda() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result =
        expand_single_sexpr_src("(lambda (x y) (cons x y))", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("lambda", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
            span
        ),
        (
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                span
            ),
            SExpr::Var(
                Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                span
            ),
        ),
        (
            SExpr::Var(
                Id::new(
                    "cons",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                ),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
            SExpr::Var(
                Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_maintains_span() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let src = "
        (lambda
          (x y)
          (cons
            x
            y
          )
        )";
    let result = expand_single_sexpr_src(src, &mut bindings, &mut env).unwrap();
    let expected = template_sexpr!(
        (
            SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]), Span { lo: 10, hi: 16 }),
            (
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]), Span { lo: 28, hi: 29 }),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]), Span { lo: 30, hi: 31 }),
            ),
            (
                SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]), Span { lo: 44, hi: 48 }),
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]), Span { lo: 61, hi: 62 }),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]), Span { lo: 75, hi: 76 }),
            )
        ) => &introduce_single_sexpr_src(src)
    )
    .unwrap();

    assert!(result == expected);
}

#[test]
fn test_expand_lambda_recursive() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (lambda (x)
              (lambda (y) (cons x y))
              (cons x x))
            "#,
        &mut bindings,
        &mut env,
    )
    .unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("lambda", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE,]),
            span
        ),
        (SExpr::Var(
            Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
            span
        )),
        (
            SExpr::Var(
                Id::new(
                    "lambda",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                ),
                span
            ),
            (SExpr::Var(
                Id::new(
                    "y",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 4]
                ),
                span
            )),
            (
                SExpr::Var(
                    Id::new(
                        "cons",
                        [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 4, 5]
                    ),
                    span
                ),
                SExpr::Var(
                    Id::new(
                        "x",
                        [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 4, 5]
                    ),
                    span
                ),
                SExpr::Var(
                    Id::new(
                        "y",
                        [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 4, 5]
                    ),
                    span
                ),
            )
        ),
        (
            SExpr::Var(
                Id::new(
                    "cons",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                ),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_lambda_dotted_params() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result =
        expand_single_sexpr_src("(lambda (x y . z) (cons x z))", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("lambda", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
            span
        ),
        (
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                span
            ),
            SExpr::Var(
                Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                span
            ),
            ..SExpr::Var(
                Id::new("z", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                span
            )
        ),
        (
            SExpr::Var(
                Id::new(
                    "cons",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                ),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
            SExpr::Var(
                Id::new("z", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_lambda_symbol_param() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("(lambda x (cons x x))", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("lambda", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
            span
        ),
        SExpr::Var(
            Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
            span
        ),
        (
            SExpr::Var(
                Id::new(
                    "cons",
                    [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                ),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
            SExpr::Var(
                Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                span
            ),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_atoms() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        expand_single_sexpr_src("(#f)", &mut bindings, &mut env)
            .unwrap()
            .without_spans(),
        make_sexpr!(SExpr::Bool(Bool(false), span)).without_spans()
    );
}

#[test]
fn test_expand_define_function_shorthand() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        expand_single_sexpr_src("(define (foo x) x)", &mut bindings, &mut env)
            .unwrap()
            .without_spans(),
        make_sexpr!(
            SExpr::Var(
                Id::new("define", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            SExpr::Var(
                Id::new("foo", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            (
                SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
                (SExpr::Var(
                    Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                    span
                ),),
                SExpr::Var(
                    Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                    span
                ),
            )
        )
        .without_spans()
    );
}

#[test]
fn test_expand_define_function_shorthand_dotted_pair() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        expand_single_sexpr_src("(define (foo . x) x)", &mut bindings, &mut env)
            .unwrap()
            .without_spans(),
        make_sexpr!(
            SExpr::Var(
                Id::new("define", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            SExpr::Var(
                Id::new("foo", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            (
                SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
                SExpr::Var(
                    Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
                    span
                ),
                SExpr::Var(
                    Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                    span
                ),
            )
        )
        .without_spans()
    );
}

#[test]
fn test_expand_define_function_shorthand_no_args() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        expand_single_sexpr_src("(define (foo) 1)", &mut bindings, &mut env)
            .unwrap()
            .without_spans(),
        make_sexpr!(
            SExpr::Var(
                Id::new("define", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            SExpr::Var(
                Id::new("foo", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
                span
            ),
            (
                SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
                SExpr::Nil(span),
                SExpr::Num(Num(1.0), span),
            )
        )
        .without_spans()
    );
}

#[test]
fn test_expand_and_macro_0_arg() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                    (syntax-rules ()
                      ((_) #f)
                      ((_ e) e)
                      ((_ e1 e2 ...)
                       (if e1 (and e2 ...) #f)))
                "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    let result = expand_single_sexpr_src("(and)", &mut bindings, &mut env).unwrap();
    let expected = introduce_single_sexpr_src("#f");
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_1_arg() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                    (syntax-rules ()
                      ((_) #f)
                      ((_ e) e)
                      ((_ e1 e2 ...)
                       (if e1 (and e2 ...) #f)))
                "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    let result = expand_single_sexpr_src("(and list)", &mut bindings, &mut env).unwrap();
    let expected = introduce_single_sexpr_src("list");
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_2_args() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   (if e1 (and e2 ...) #f)))
            "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    let result = expand_single_sexpr_src("(and list list)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
            span
        ),
        SExpr::Var(
            Id::new("list", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE,]),
            span
        ),
        SExpr::Var(
            Id::new("list", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE,]),
            span
        ),
        SExpr::Bool(Bool(false), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_4_args() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   (if e1 (and e2 ...) #f)))
            "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    // (and t t t t)
    // (if t (and t t t) f)
    // (if t (if t (and t t) f) f)
    // (if t (if t (if t (and t) f) f) f)
    // (if t (if t (if t t f) f) f) f)
    let result = expand_single_sexpr_src("(and #t #t #t #t)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]),
            span
        ),
        SExpr::Bool(Bool(true), span),
        (
            SExpr::Var(
                Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 3]),
                span
            ),
            SExpr::Bool(Bool(true), span),
            (
                SExpr::Var(
                    Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 4]),
                    span
                ),
                SExpr::Bool(Bool(true), span),
                SExpr::Bool(Bool(true), span),
                SExpr::Bool(Bool(false), span),
            ),
            SExpr::Bool(Bool(false), span),
        ),
        SExpr::Bool(Bool(false), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
    assert_eq!(
        bindings
            .resolve_sym(&(first(&result).clone().try_into().unwrap()))
            .unwrap(),
        Symbol::new("if")
    );
}

#[test]
fn test_expand_simple_macro_hygiene() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("x", [Bindings::CORE_SCOPE]), &Symbol::new("x"));
    bindings.add_binding(
        &Id::new("my-macro", [Bindings::CORE_SCOPE]),
        &Symbol::new("my-macro"),
    );

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                (syntax-rules ()
                  ((_ body) (lambda (x) body)))
            "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("my-macro", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    let result = expand_single_sexpr_src("(my-macro x)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new(
                "lambda",
                [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2]
            ),
            span
        ),
        (SExpr::Var(
            Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
            span
        )),
        SExpr::Var(
            Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 3, 4]),
            span
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
    assert_ne!(
        bindings
            .resolve_sym(
                &first(try_nth(&result, 1).unwrap())
                    .clone()
                    .try_into()
                    .unwrap()
            )
            .unwrap(),
        bindings
            .resolve_sym(&try_last(&result).unwrap().clone().try_into().unwrap())
            .unwrap(),
    );
    assert_eq!(
        bindings
            .resolve_sym(&Id::new("x", [Bindings::CORE_SCOPE]))
            .unwrap(),
        bindings
            .resolve_sym(&try_last(&result).unwrap().clone().try_into().unwrap())
            .unwrap(),
    )
}

#[test]
fn test_expand_or_macro_hygiene() {
    let mut bindings = Bindings::new();

    bindings.add_binding(
        &Id::new("my-or", [Bindings::CORE_SCOPE]),
        &Symbol::new("my-or"),
    );

    let transformer = Transformer::new(&introduce_single_sexpr_src(
        r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   ((lambda (temp) (if temp temp (my-or e2 ...))) e1)))
            "#,
    ))
    .unwrap();

    let mut env = Env::from([(
        bindings
            .resolve_sym(&Id::new("my-or", [Bindings::CORE_SCOPE]))
            .unwrap(),
        transformer,
    )]);

    let result = expand_single_sexpr_src(
        "((lambda (temp) (my-or #f temp)) #t)",
        &mut bindings,
        &mut env,
    )
    .unwrap();
    let outer_lambda = first(&result);
    let inner_application = try_nth(outer_lambda, 2).unwrap();
    let inner_lambda = first(inner_application);
    let if_expr = try_nth(inner_lambda, 2).unwrap();

    assert_eq!(
        bindings.resolve_sym(&first(outer_lambda).clone().try_into().unwrap()),
        Some(Symbol::new("lambda"))
    );
    assert_eq!(
        bindings.resolve_sym(&first(inner_lambda).clone().try_into().unwrap()),
        Some(Symbol::new("lambda"))
    );
    assert_eq!(
        bindings.resolve_sym(&first(if_expr).clone().try_into().unwrap()),
        Some(Symbol::new("if"))
    );

    let outer_temp_id = first(try_nth(first(&result), 1).unwrap());
    let inner_temp_id = first(try_nth(first(try_nth(first(&result), 2).unwrap()), 1).unwrap());
    let if_expr = try_nth(first(try_nth(first(&result), 2).unwrap()), 2).unwrap();

    assert_ne!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&(try_nth(if_expr, 1).unwrap()).clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 2).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_ne!(
        bindings
            .resolve_sym(&(try_nth(if_expr, 1).unwrap()).clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 3).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 2).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 3).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );
}

#[test]
fn test_expand_let_syntax_via_or_macro() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
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
               ((lambda (temp) (or #f temp)) #t))
            "#,
        &mut bindings,
        &mut env,
    )
    .unwrap();
    let outer_lambda = first(&result);
    let inner_application = try_nth(outer_lambda, 2).unwrap();
    let inner_lambda = first(inner_application);
    let if_expr = try_nth(inner_lambda, 2).unwrap();

    assert_eq!(
        bindings.resolve_sym(&first(outer_lambda).clone().try_into().unwrap()),
        Some(Symbol::new("lambda"))
    );
    assert_eq!(
        bindings.resolve_sym(&first(inner_lambda).clone().try_into().unwrap()),
        Some(Symbol::new("lambda"))
    );
    assert_eq!(
        bindings.resolve_sym(&first(if_expr).clone().try_into().unwrap()),
        Some(Symbol::new("if"))
    );

    let outer_temp_id = first(try_nth(first(&result), 1).unwrap());
    let inner_temp_id = first(try_nth(first(try_nth(first(&result), 2).unwrap()), 1).unwrap());
    let if_expr = try_nth(first(try_nth(first(&result), 2).unwrap()), 2).unwrap();

    assert_ne!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&(try_nth(if_expr, 1).unwrap()).clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 2).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_ne!(
        bindings
            .resolve_sym(&(try_nth(if_expr, 1).unwrap()).clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 3).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 2).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(try_nth(if_expr, 3).unwrap()).clone().try_into().unwrap())
            .unwrap(),
    );
}

#[test]
fn test_expand_let_syntax_has_body_ctx() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    expand_single_sexpr_src(
        r#"
            (letrec-syntax
                ((one (syntax-rules ()
                        ((_) 1))))
            (define x 1)
            x)
            "#,
        &mut bindings,
        &mut env,
    )
    .unwrap();
    assert!(
        bindings
            .resolve(&Id::new("x", [Bindings::CORE_SCOPE]))
            .is_none()
    );
}

#[test]
fn test_expand_let_syntax_multiple_body_exprs_recursive_defn() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (letrec-syntax
                ((one (syntax-rules ()
                        ((_) 1)))
                (two (syntax-rules ()
                        ((_) 2))))
            (define x (lambda () y))
            (define y (lambda () x))
            x)
            "#,
        &mut bindings,
        &mut env,
    )
    .unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("letrec", [Bindings::CORE_SCOPE]), span),
        (
            (
                SExpr::Var(
                    Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                    span
                ),
                (
                    SExpr::Var(
                        Id::new(
                            "lambda",
                            [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                        ),
                        span
                    ),
                    SExpr::Nil(span),
                    SExpr::Var(
                        Id::new(
                            "y",
                            [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 4, 5]
                        ),
                        span
                    ),
                ),
            ),
            (
                SExpr::Var(
                    Id::new("y", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
                    span
                ),
                (
                    SExpr::Var(
                        Id::new(
                            "lambda",
                            [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]
                        ),
                        span
                    ),
                    SExpr::Nil(span),
                    SExpr::Var(
                        Id::new(
                            "x",
                            [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3, 6, 7]
                        ),
                        span
                    ),
                ),
            ),
        ),
        SExpr::Var(
            Id::new("x", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE, 2, 3]),
            span
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_let_syntax_multiple_body_exprs_() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (letrec-syntax
                ((one (syntax-rules ()
                        ((_) 1)))
                (two (syntax-rules ()
                        ((_) 2))))
            (one)
            (two))
            "#,
        &mut bindings,
        &mut env,
    )
    .unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("begin", [Bindings::CORE_SCOPE]), span),
        SExpr::Num(Num(1.0), span),
        SExpr::Num(Num(2.0), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_letrec_syntax_cleans_env_after_success() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (letrec-syntax
              ((one (syntax-rules ()
                       ((_) 1))))
              (one))
            "#,
        &mut bindings,
        &mut env,
    );
    assert!(
        result.is_ok(),
        "Expected letrec-syntax expression to expand"
    );
    assert!(
        env.is_empty(),
        "Expected letrec-syntax to remove temporary transformer bindings from env"
    );
}

#[test]
fn test_expand_letrec_syntax_cleans_env_on_transformer_spec_error() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (letrec-syntax
              ((one (syntax-rules ()
                       ((_) 1)))
               (bad 42))
              (one))
            "#,
        &mut bindings,
        &mut env,
    );
    assert!(
        result.is_err(),
        "Expected invalid letrec-syntax transformer spec to fail"
    );
    assert!(
        env.is_empty(),
        "Expected letrec-syntax error path to remove inserted transformer bindings from env"
    );
}

#[test]
fn test_let_syntax_cleans_env_after_success() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (let-syntax
              ((one (syntax-rules ()
                       ((_) 1))))
              (one))
            "#,
        &mut bindings,
        &mut env,
    );
    assert!(result.is_ok(), "Expected let-syntax expression to expand");
    assert!(
        env.is_empty(),
        "Expected let-syntax to remove temporary transformer bindings from env"
    );
}

#[test]
fn test_let_syntax_cleans_env_on_transformer_spec_error() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(
        r#"
            (let-syntax
              ((one (syntax-rules ()
                       ((_) 1)))
               (bad 42))
              (one))
            "#,
        &mut bindings,
        &mut env,
    );
    assert!(
        result.is_err(),
        "Expected invalid let-syntax transformer spec to fail"
    );
    assert!(
        env.is_empty(),
        "Expected let-syntax error path to remove inserted transformer bindings from env"
    );
}

#[test]
fn test_expand_failed_expansion_does_not_affect_bindings_or_env() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    let result = expand_single_sexpr_src("(begin (define x 1) ())", &mut bindings, &mut env);

    assert!(result.is_err());
    assert_eq!(
        bindings.resolve_sym(&Id::new("x", [Bindings::CORE_SCOPE])),
        None
    );
}

#[test]
fn test_expand_failed_define_syntax_does_not_persist_transformer() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    let result = expand_single_sexpr_src(
        "(begin (define-syntax my-id (syntax-rules () ((_ x) x))) (my-id ()))",
        &mut bindings,
        &mut env,
    );

    assert!(result.is_err());
    assert_eq!(
        bindings.resolve_sym(&Id::new("my-id", [Bindings::CORE_SCOPE])),
        None
    );
    assert!(env.is_empty());
}

#[test]
fn test_expand_if_three_arms() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("(if #t 1 2)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
            span
        ),
        SExpr::Bool(Bool(true), span),
        SExpr::Num(Num(1.0), span),
        SExpr::Num(Num(2.0), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_if_two_arms_normalizes_to_three() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("(if #t 1)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(
            Id::new("if", [Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]),
            span
        ),
        SExpr::Bool(Bool(true), span),
        SExpr::Num(Num(1.0), span),
        SExpr::Void(span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_quasiquote_atom() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("`42", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), span),
        SExpr::Num(Num(42.0), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_quasiquote_empty_list() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("`()", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), span),
        SExpr::Nil(span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_quasiquote_constant_list() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src("`(1 2)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("append", [Bindings::CORE_SCOPE]), span),
        (
            SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), span),
            (SExpr::Num(Num(1.0), span)),
        ),
        (
            SExpr::Var(Id::new("append", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), span),
                (SExpr::Num(Num(2.0), span)),
            ),
            (
                SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), span),
                SExpr::Nil(span),
            ),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_quasiquote_with_unquote() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    // Expands into (lambda (x) (append (quote (1)) (append (list x) (quote ()))))
    let result = expand_single_sexpr_src("(lambda (x) `(1 ,x))", &mut bindings, &mut env).unwrap();

    let body = try_nth(&result, 2).unwrap();
    let head: Id = first(body).clone().try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&head),
        Some(Symbol::new("append")),
        "Body head should resolve to 'append'"
    );
    let quote_1 = try_nth(body, 1).unwrap();
    let quote_head: Id = first(quote_1).clone().try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&quote_head),
        Some(Symbol::new("quote")),
    );
    let inner_append = try_nth(body, 2).unwrap();
    let list_call = try_nth(inner_append, 1).unwrap();
    let list_head: Id = first(list_call).clone().try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&list_head),
        Some(Symbol::new("list")),
        "Unquoted element should be wrapped in 'list'"
    );
}

#[test]
fn test_expand_quasiquote_with_unquote_splicing() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    // Expands into (lambda (xs) (append (quote (1)) (append (append xs) (quote ()))))
    let result =
        expand_single_sexpr_src("(lambda (xs) `(1 ,@xs))", &mut bindings, &mut env).unwrap();

    let body = try_nth(&result, 2).unwrap();
    let inner_append = try_nth(body, 2).unwrap();

    let splice_call = try_nth(inner_append, 1).unwrap();
    let splice_head: Id = first(splice_call).clone().try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&splice_head),
        Some(Symbol::new("append")),
        "Spliced element should be wrapped in 'append'"
    );
}

#[test]
fn test_expand_quasiquote_unquote_resolves_to_lambda_param() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    // Expands into (lambda (x) (append (list x) (quote ())))
    let result = expand_single_sexpr_src("(lambda (x) `(,x))", &mut bindings, &mut env).unwrap();

    let param = first(try_nth(&result, 1).unwrap());
    let param_id: Id = param.clone().try_into().unwrap();
    let param_sym = bindings.resolve_sym(&param_id).unwrap();

    let body = try_nth(&result, 2).unwrap();
    let list_call = try_nth(body, 1).unwrap();
    let x_ref: Id = try_nth(list_call, 1).unwrap().clone().try_into().unwrap();
    let x_sym = bindings.resolve_sym(&x_ref).unwrap();
    assert_eq!(
        param_sym, x_sym,
        "Unquoted x should resolve to the lambda parameter"
    );
}

#[test]
fn test_expand_quasiquote_nested_preserves_inner() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();

    let result = expand_single_sexpr_src("`(1 `(2 3))", &mut bindings, &mut env).unwrap();
    let output = format!("{result}");
    assert!(
        output.contains("(quote quasiquote)"),
        "Nested quasiquote should keep inner quasiquote as data: got {output}"
    );
}

#[test]
fn test_expand_unquote_outside_quasiquote_errors() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(",x", &mut bindings, &mut env);
    assert!(result.is_err());
}

#[test]
fn test_expand_unquote_splicing_outside_quasiquote_errors() {
    let mut bindings = Bindings::new();
    let mut env = Env::default();
    let result = expand_single_sexpr_src(",@x", &mut bindings, &mut env);
    assert!(result.is_err());
}
