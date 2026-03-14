use crate::{
    compile::{
        lex::tokenize,
        parse::parse,
        sexpr::{Bool, Id, Num},
        span::Span,
        util::{first, last, nth},
    },
    make_sexpr, template_sexpr,
};

fn expand_source(
    source: &str,
    bindings: &mut Bindings,
    env: &mut HashMap<Symbol, Rc<Transformer>>,
) -> Result<SExpr<Id>> {
    let expr = parse(&tokenize(source).unwrap()).unwrap();
    expand(introduce(expr), bindings, env)
}

use super::*;

#[test]
fn test_introduce() {
    let list = parse(&tokenize("(cons 0 1)").unwrap()).unwrap();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        introduce(list).without_spans(),
        make_sexpr!(
            SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE]), span),
            SExpr::Num(Num(0.0), span),
            SExpr::Num(Num(1.0), span),
        )
        .without_spans()
    );
}

#[test]
fn test_expand_lambda() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let lambda_expr = parse(&tokenize("(lambda (x y) (cons x y))").unwrap()).unwrap();
    let result = expand(introduce(lambda_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
        (
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
            SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1]), span),
        ),
        (
            SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_maintains_span() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let src = "
        (lambda
          (x y)
          (cons
            x
            y
          )
        )";
    let lambda_expr = parse(&tokenize(src).unwrap()).unwrap();
    let result = expand(introduce(lambda_expr), &mut bindings, &mut env).unwrap();
    let expected = template_sexpr!(
        (
            SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), Span { lo: 10, hi: 16 }),
            (
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1]), Span { lo: 28, hi: 29 }),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1]), Span { lo: 30, hi: 31 }),
            ),
            (
                SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 44, hi: 48 }),
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 61, hi: 62 }),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 75, hi: 76 }),
            )
        ) => &introduce(parse(&tokenize(src).unwrap()).unwrap())
    )
    .unwrap();

    assert!(result == expected);
}

#[test]
fn test_expand_lambda_recursive() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let lambda_expr = parse(
        &tokenize(
            r#"
            (lambda (x)
              (lambda (y) (cons x y))
              (cons x x))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(lambda_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
        (SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1]), span)),
        (
            SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
            (SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3]), span)),
            (
                SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
            )
        ),
        (
            SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_lambda_dotted_params() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let lambda_expr = parse(&tokenize("(lambda (x y . z) (cons x z))").unwrap()).unwrap();
    let result = expand(introduce(lambda_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
        (
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
            SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1]), span),
            ..SExpr::Var(Id::new("z", [Bindings::CORE_SCOPE, 1]), span)
        ),
        (
            SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("z", [Bindings::CORE_SCOPE, 1, 2]), span),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_lambda_symbol_param() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let lambda_expr = parse(&tokenize("(lambda x (cons x x))").unwrap()).unwrap();
    let result = expand(introduce(lambda_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
        SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
        (
            SExpr::Var(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
        ),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_atoms() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let sexpr = parse(
        &tokenize(
            r#"
            (#f)
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let span = Span { lo: 0, hi: 0 };
    assert_eq!(
        expand(introduce(sexpr), &mut bindings, &mut env)
            .unwrap()
            .without_spans(),
        make_sexpr!(SExpr::Bool(Bool(false), span)).without_spans()
    );
}

#[test]
fn test_expand_and_macro_0_arg() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                    (syntax-rules ()
                      ((_) #f)
                      ((_ e) e)
                      ((_ e1 e2 ...)
                       (if e1 (and e2 ...) #f)))
                "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = parse(&tokenize("(and)").unwrap()).unwrap();
    let result = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
    let expected = introduce(parse(&tokenize("#f").unwrap()).unwrap());
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_1_arg() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                    (syntax-rules ()
                      ((_) #f)
                      ((_ e) e)
                      ((_ e1 e2 ...)
                       (if e1 (and e2 ...) #f)))
                "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = introduce(parse(&tokenize("(and list)").unwrap()).unwrap());
    let result = expand(sexpr, &mut bindings, &mut env).unwrap();
    let expected = introduce(parse(&tokenize("list").unwrap()).unwrap());
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_2_args() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   (if e1 (and e2 ...) #f)))
            "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = parse(&tokenize("(and list list)").unwrap()).unwrap();
    let result = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 1]), span),
        SExpr::Var(Id::new("list", [Bindings::CORE_SCOPE]), span),
        SExpr::Var(Id::new("list", [Bindings::CORE_SCOPE]), span),
        SExpr::Bool(Bool(false), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_and_macro_4_args() {
    let mut bindings = Bindings::new();

    bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   (if e1 (and e2 ...) #f)))
            "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = parse(&tokenize("(and #t #t #t #t)").unwrap()).unwrap();
    // (and t t t t)
    // (if t (and t t t) f)
    // (if t (if t (and t t) f) f)
    // (if t (if t (if t (and t) f) f) f)
    // (if t (if t (if t t f) f) f) f)
    let result = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 1]), span),
        SExpr::Bool(Bool(true), span),
        (
            SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 2]), span),
            SExpr::Bool(Bool(true), span),
            (
                SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 3]), span),
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
            .resolve_sym(&(first(&result).try_into().unwrap()))
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

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                (syntax-rules ()
                  ((_ body) (lambda (x) body)))
            "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("my-macro", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = parse(&tokenize("(my-macro x)").unwrap()).unwrap();
    let result = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1]), span),
        (SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span)),
        SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 2, 3]), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
    assert_ne!(
        bindings
            .resolve_sym(&first(&nth(&result, 1).unwrap()).try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&last(&result).unwrap().try_into().unwrap())
            .unwrap(),
    );
    assert_eq!(
        bindings
            .resolve_sym(&Id::new("x", [Bindings::CORE_SCOPE]))
            .unwrap(),
        bindings
            .resolve_sym(&last(&result).unwrap().try_into().unwrap())
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

    let transformer = Transformer::new(&introduce(
        parse(
            &tokenize(
                r#"
                (syntax-rules ()
                  ((_) #f)
                  ((_ e) e)
                  ((_ e1 e2 ...)
                   ((lambda (temp) (if temp temp (my-or e2 ...))) e1)))
            "#,
            )
            .unwrap(),
        )
        .unwrap(),
    ))
    .unwrap();

    let mut env = HashMap::from([(
        bindings
            .resolve_sym(&Id::new("my-or", [Bindings::CORE_SCOPE]))
            .unwrap(),
        Rc::new(transformer),
    )]);

    let sexpr = parse(&tokenize("((lambda (temp) (my-or #f temp)) #t)").unwrap()).unwrap();
    let result = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };

    let expected = make_sexpr!(
        (
            SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
            (SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1]), span)),
            (
                (
                    SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 3]), span),
                    (SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4]), span)),
                    (
                        SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                        SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                        SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                        SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 4, 5]), span),
                    ),
                ),
                SExpr::Bool(Bool(false), span),
            ),
        ),
        SExpr::Bool(Bool(true), span),
    );

    assert_eq!(result.without_spans(), expected.without_spans());

    let outer_temp_id = first(&nth(&first(&result), 1).unwrap());
    let inner_temp_id = first(&nth(&first(&nth(&first(&result), 2).unwrap()), 1).unwrap());
    let if_expr = nth(&first(&nth(&first(&result), 2).unwrap()), 2).unwrap();

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
            .resolve_sym(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_ne!(
        bindings
            .resolve_sym(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
            .unwrap(),
    );
}

#[test]
fn test_expand_let_syntax_via_or_macro() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let let_syntax_expr = parse(
        &tokenize(
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
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        (
            SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
            (SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 3]), span)),
            (
                (
                    SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 5]), span),
                    (SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6]), span)),
                    (
                        SExpr::Var(Id::new("if", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                        SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                        SExpr::Var(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                        SExpr::Var(
                            Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 3, 4, 6, 7]),
                            span,
                        ),
                    ),
                ),
                SExpr::Bool(Bool(false), span),
            ),
        ),
        SExpr::Bool(Bool(true), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());

    let outer_temp_id = first(&nth(&first(&result), 1).unwrap());
    let inner_temp_id = first(&nth(&first(&nth(&first(&result), 2).unwrap()), 1).unwrap());
    let if_expr = nth(&first(&nth(&first(&result), 2).unwrap()), 2).unwrap();

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
            .resolve_sym(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_ne!(
        bindings
            .resolve_sym(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&inner_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
            .unwrap(),
    );

    assert_eq!(
        bindings
            .resolve_sym(&outer_temp_id.clone().try_into().unwrap())
            .unwrap(),
        bindings
            .resolve_sym(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
            .unwrap(),
    );
}

#[test]
fn test_expand_let_syntax_has_body_ctx() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let let_syntax_expr = parse(
        &tokenize(
            r#"
            (letrec-syntax
                ((one (syntax-rules ()
                        ((_) 1))))
            (define x 1)
            x)
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    expand(introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
    assert!(
        bindings
            .resolve(&Id::new("x", [Bindings::CORE_SCOPE]))
            .is_none()
    );
}

#[test]
fn test_expand_let_syntax_multiple_body_exprs_recursive_defn() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let let_syntax_expr = parse(
        &tokenize(
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
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    let expected = make_sexpr!(
        SExpr::Var(Id::new("begin", [Bindings::CORE_SCOPE]), span),
        (
            SExpr::Var(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            (
                SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Nil(span),
                SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
            ),
        ),
        (
            SExpr::Var(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
            SExpr::Var(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
            (
                SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Nil(span),
                SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2, 5, 6]), span),
            ),
        ),
        SExpr::Var(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
    );
    assert_eq!(result.without_spans(), expected.without_spans());
}

#[test]
fn test_expand_let_syntax_multiple_body_exprs_() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let let_syntax_expr = parse(
        &tokenize(
            r#"
            (letrec-syntax
                ((one (syntax-rules ()
                        ((_) 1)))
                (two (syntax-rules ()
                        ((_) 2))))
            (one)
            (two))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let expr = parse(
        &tokenize(
            r#"
            (letrec-syntax
              ((one (syntax-rules ()
                       ((_) 1))))
              (one))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(expr), &mut bindings, &mut env);
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let expr = parse(
        &tokenize(
            r#"
            (letrec-syntax
              ((one (syntax-rules ()
                       ((_) 1)))
               (bad 42))
              (one))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(expr), &mut bindings, &mut env);
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let expr = parse(
        &tokenize(
            r#"
            (let-syntax
              ((one (syntax-rules ()
                       ((_) 1))))
              (one))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(expr), &mut bindings, &mut env);
    assert!(result.is_ok(), "Expected let-syntax expression to expand");
    assert!(
        env.is_empty(),
        "Expected let-syntax to remove temporary transformer bindings from env"
    );
}

#[test]
fn test_let_syntax_cleans_env_on_transformer_spec_error() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let expr = parse(
        &tokenize(
            r#"
            (let-syntax
              ((one (syntax-rules ()
                       ((_) 1)))
               (bad 42))
              (one))
            "#,
        )
        .unwrap(),
    )
    .unwrap();
    let result = expand(introduce(expr), &mut bindings, &mut env);
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();

    // A begin with a define followed by an invalid form.
    // The define will mutate bindings before the error is hit.
    let result = expand_source("(begin (define x 1) ())", &mut bindings, &mut env);
    assert!(result.is_err());

    // x should not be resolvable since the expansion failed
    assert_eq!(
        bindings.resolve_sym(&Id::new("x", [Bindings::CORE_SCOPE])),
        None
    );
}

#[test]
fn test_expand_failed_define_syntax_does_not_persist_transformer() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();

    // define-syntax followed by a use of the macro in a begin that also has an error
    let result = expand_source(
        "(begin (define-syntax my-id (syntax-rules () ((_ x) x))) (my-id ()))",
        &mut bindings,
        &mut env,
    );
    assert!(result.is_err());

    // my-id should not be resolvable
    assert_eq!(
        bindings.resolve_sym(&Id::new("my-id", [Bindings::CORE_SCOPE])),
        None
    );
    // env should be empty (no transformer persisted)
    assert!(env.is_empty());
}

// --- Quasiquote unit tests ---

#[test]
fn test_expand_quasiquote_atom() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("`42", &mut bindings, &mut env).unwrap();
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("`()", &mut bindings, &mut env).unwrap();
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("`(1 2)", &mut bindings, &mut env).unwrap();
    let span = Span { lo: 0, hi: 0 };
    // `(1 2) => (append (quote (1)) (append (quote (2)) (quote ())))
    // Note: (quote (1)) wraps the element in a list for append
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
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("(lambda (x) `(1 ,x))", &mut bindings, &mut env).unwrap();
    // Focus on the body: (append (quote (1)) (append (list x) (quote ())))
    let body = nth(&result, 2).unwrap();
    // The body head should be `append`
    let head: Id = first(&body).try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&head),
        Some(Symbol::new("append")),
        "Body head should resolve to 'append'"
    );
    // Second element of body is (quote (1))
    let quote_1 = nth(&body, 1).unwrap();
    let quote_head: Id = first(&quote_1).try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&quote_head),
        Some(Symbol::new("quote")),
    );
    // Third element contains (list x)
    let inner_append = nth(&body, 2).unwrap();
    let list_call = nth(&inner_append, 1).unwrap();
    let list_head: Id = first(&list_call).try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&list_head),
        Some(Symbol::new("list")),
        "Unquoted element should be wrapped in 'list'"
    );
}

#[test]
fn test_expand_quasiquote_with_unquote_splicing() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("(lambda (xs) `(1 ,@xs))", &mut bindings, &mut env).unwrap();
    // Body: (append (quote (1)) (append (append xs) (quote ())))
    let body = nth(&result, 2).unwrap();
    let inner_append = nth(&body, 2).unwrap();
    // The splice call should be (append xs) — append wrapping the spliced var
    let splice_call = nth(&inner_append, 1).unwrap();
    let splice_head: Id = first(&splice_call).try_into().unwrap();
    assert_eq!(
        bindings.resolve_sym(&splice_head),
        Some(Symbol::new("append")),
        "Spliced element should be wrapped in 'append'"
    );
}

#[test]
fn test_expand_quasiquote_unquote_resolves_to_lambda_param() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source("(lambda (x) `(,x))", &mut bindings, &mut env).unwrap();
    // lambda param
    let param = first(&nth(&result, 1).unwrap());
    let param_id: Id = param.try_into().unwrap();
    let param_sym = bindings.resolve_sym(&param_id).unwrap();
    // body is (append (list x) (quote ()))
    let body = nth(&result, 2).unwrap();
    let list_call = nth(&body, 1).unwrap();
    let x_ref: Id = nth(&list_call, 1).unwrap().try_into().unwrap();
    let x_sym = bindings.resolve_sym(&x_ref).unwrap();
    assert_eq!(
        param_sym, x_sym,
        "Unquoted x should resolve to the lambda parameter"
    );
}

#[test]
fn test_expand_quasiquote_nested_preserves_inner() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    // `(1 `(2 3)) — no unquotes, just nested quasiquote
    let result = expand_source("`(1 `(2 3))", &mut bindings, &mut env).unwrap();
    let output = format!("{result}");
    assert!(
        output.contains("(quote quasiquote)"),
        "Nested quasiquote should keep inner quasiquote as data: got {output}"
    );
}

#[test]
fn test_expand_unquote_outside_quasiquote_errors() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source(",x", &mut bindings, &mut env);
    assert!(result.is_err());
}

#[test]
fn test_expand_unquote_splicing_outside_quasiquote_errors() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::<Symbol, Rc<Transformer>>::new();
    let result = expand_source(",@x", &mut bindings, &mut env);
    assert!(result.is_err());
}
