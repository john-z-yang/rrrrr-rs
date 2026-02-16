use std::collections::HashMap;

use super::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{SExpr, Symbol},
    transformer::Transformer,
    util::first,
};
use crate::{
    compile::{
        compilation_error::CompilationError,
        util::{try_for_each, try_map},
    },
    match_sexpr, template_sexpr,
};

type Env = HashMap<Symbol, Transformer>;

pub(crate) fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.add_scope(Bindings::CORE_SCOPE)
}

pub(crate) fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    if let SExpr::Nil(span) = sexpr {
        return Err(CompilationError {
            span: *span,
            reason: "Unexpected nil".to_owned(),
        });
    };
    if let SExpr::Id(..) = sexpr {
        return expand_id(sexpr, bindings);
    }
    match_sexpr! {(SExpr::Id(..), ..) = sexpr =>
        return expand_id_application(sexpr, bindings, env);
    };
    match_sexpr! {(..) = sexpr =>
        return expand_fn_application(sexpr, bindings, env);
    };
    Ok(sexpr.clone())
}

fn expand_id(sexpr: &SExpr, bindings: &mut Bindings) -> Result<SExpr> {
    let SExpr::Id(id, span) = sexpr else {
        unreachable!("expand_id is expecting an ID");
    };
    bindings.resolve(id).ok_or(CompilationError {
        span: *span,
        reason: format!("ID: {} is unbound", id),
    })?;
    Ok(sexpr.clone())
}

fn expand_id_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    let binding = match first(sexpr) {
        Some(SExpr::Id(id, span)) => bindings.resolve(&id).ok_or_else(|| CompilationError {
            span,
            reason: format!("ID: {} is unbound", id),
        })?,
        _ => unreachable!("first element of ID application must be an ID"),
    };

    match binding.0.as_str() {
        "quote" | "quote-syntax" => Ok(sexpr.clone()),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env),
        "lambda" => expand_lambda(sexpr, bindings, env),
        _ => {
            if let Some(transformer) = env.get(&binding) {
                let scope_id = bindings.new_scope_id();
                let sexpr = sexpr.add_scope(scope_id);
                let transformed_sexpr =
                    transformer
                        .transform(&sexpr)
                        .ok_or_else(|| CompilationError {
                            span: sexpr.get_span(),
                            reason: format!(
                                "Unable to apply transformer: {}, no rules match",
                                binding
                            ),
                        })?;
                expand(&transformed_sexpr.flip_scope(scope_id), bindings, env)
            } else {
                expand_fn_application(sexpr, bindings, env)
            }
        }
    }
}

fn expand_fn_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    try_map(|sub_sexpr| expand(sub_sexpr, bindings, env), sexpr)
}

fn expand_lambda(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {(lambda, (args @ ..), body @ ..) = sexpr =>
        let scope_id = bindings.new_scope_id();
        let args = args.add_scope(scope_id);

        try_for_each(
            |arg| {
                let SExpr::Id(id, _) = arg else {
                    return Err(CompilationError {
                        span: arg.get_span(),
                        reason: format!(
                            "Expected identifiers in function parameters, but got: {}",
                            arg
                        ),
                    });
                };
                let binding = bindings.gen_sym();
                bindings.add_binding(id, &binding);
                Ok(())
            },
            &args,
        )?;

        let body = try_map(|sexpr| expand(&sexpr.add_scope(scope_id), bindings, env), body)?;
        return Ok(template_sexpr!((lambda.clone(), args, ..body) => sexpr).unwrap());
    };
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid use of lambda form".to_owned(),
    })
}

fn expand_letrec_syntax(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {(sym("letrec-syntax"), ((keyword, transformer_spec)), body) = sexpr =>
        let scope_id = bindings.new_scope_id();
        let keyword = keyword.add_scope(scope_id);

        let SExpr::Id(id, _) = keyword else {
            return Err(CompilationError {
                span: keyword.get_span(),
                reason: format!("Expected identifiers in syntax keyword, but got: {}", keyword)
            });
        };
        let binding = bindings.gen_sym();
        bindings.add_binding(&id, &binding);

        let transformer = Transformer::new(&transformer_spec.add_scope(scope_id))?;
        env.insert(binding.clone(), transformer);

        let res = expand(&body.add_scope(scope_id), bindings, env);
        env.remove_entry(&binding);

        return res;
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid use of letrec-syntax form".to_owned(),
    })
}

#[cfg(test)]
mod tests {

    use crate::{
        compile::{
            lex::tokenize,
            parse::parse,
            sexpr::{Bool, Id, Num},
            span::Span,
        },
        sexpr,
    };

    fn last(sexpr: &SExpr) -> Option<SExpr> {
        match sexpr {
            SExpr::Cons(cons, _) if matches!(*cons.cdr, SExpr::Nil(_)) => {
                Some(cons.car.as_ref().clone())
            }
            SExpr::Cons(cons, _) => last(&cons.cdr),
            _ => None,
        }
    }

    fn nth(sexpr: &SExpr, idx: usize) -> Option<SExpr> {
        let SExpr::Cons(cons, _) = sexpr else {
            return None;
        };
        if idx == 0 {
            Some(cons.car.as_ref().clone())
        } else {
            nth(&cons.cdr, idx - 1)
        }
    }

    use super::*;

    #[test]
    fn test_introduce() {
        let list = parse(&tokenize("(cons 0 1)").unwrap()).unwrap();
        let span = Span { lo: 0, hi: 0 };
        assert_eq!(
            introduce(&list),
            sexpr!(
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), span),
                SExpr::Num(Num(0.0), span),
                SExpr::Num(Num(1.0), span),
            )
        );
    }

    #[test]
    fn test_expand_lambda() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = parse(&tokenize("(lambda (x y) (cons x y))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1]), span),
            ),
            (
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1]), span),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_maintains_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let src = "
        (lambda
          (x y)
          (cons
            x
            y
          )
        )";
        let lambda_expr = parse(&tokenize(src).unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let expected = template_sexpr!(
            (
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), Span { lo: 10, hi: 16 }),
                (
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), Span { lo: 28, hi: 29 }),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1]), Span { lo: 30, hi: 31 }),
                ),
                (
                    SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1]), Span { lo: 44, hi: 48 }),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), Span { lo: 61, hi: 62 }),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1]), Span { lo: 75, hi: 76 }),
                )
            ) => &parse(&tokenize(src).unwrap()).unwrap()
        )
        .unwrap();

        assert!(result.is_idential(&expected));
    }

    #[test]
    fn test_expand_lambda_recursive() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
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
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
            (SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span)),
            (
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1]), span),
                (SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span)),
                (
                    SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
                )
            ),
            (
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_atoms() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
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
            expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap(),
            sexpr!(SExpr::Bool(Bool(false), span))
        );
    }

    #[test]
    fn test_expand_and_macro_0_arg() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(
            &parse(
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
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("(and)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let expected = parse(&tokenize("#f").unwrap()).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_1_arg() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(
            &parse(
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
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = introduce(&parse(&tokenize("(and list)").unwrap()).unwrap());
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let expected = introduce(&parse(&tokenize("list").unwrap()).unwrap());
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_2_args() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(
            &parse(
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
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("(and list list)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 1]), span),
            SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), span),
            SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), span),
            SExpr::Bool(Bool(false), span),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_4_args() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(
            &parse(
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
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("(and #t #t #t #t)").unwrap()).unwrap();
        // (and t t t t)
        // (if t (and t t t) f)
        // (if t (if t (and t t) f) f)
        // (if t (if t (if t (and t) f) f) f)
        // (if t (if t (if t t f) f) f) f)
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 1]), span),
            SExpr::Bool(Bool(true), span),
            (
                SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 2]), span),
                SExpr::Bool(Bool(true), span),
                (
                    SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 3]), span),
                    SExpr::Bool(Bool(true), span),
                    SExpr::Bool(Bool(true), span),
                    SExpr::Bool(Bool(false), span),
                ),
                SExpr::Bool(Bool(false), span),
            ),
            SExpr::Bool(Bool(false), span),
        );
        assert_eq!(result, expected);
        assert_eq!(
            bindings
                .resolve(&(first(&result).unwrap().try_into().unwrap()))
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
            &parse(
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
                .resolve(&Id::new("my-macro", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("(my-macro x)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1]), span),
            (SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span)),
            SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 2]), span),
        );
        assert_eq!(result, expected);
        assert_ne!(
            bindings
                .resolve(
                    &first(&nth(&result, 1).unwrap())
                        .unwrap()
                        .try_into()
                        .unwrap()
                )
                .unwrap(),
            bindings
                .resolve(&last(&result).unwrap().try_into().unwrap())
                .unwrap(),
        );
        assert_eq!(
            bindings
                .resolve(&Id::new("x", [Bindings::CORE_SCOPE]))
                .unwrap(),
            bindings
                .resolve(&last(&result).unwrap().try_into().unwrap())
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
            &parse(
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
                .resolve(&Id::new("my-or", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("((lambda (temp) (my-or #f temp)) #t)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };

        let expected = sexpr!(
            (
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
                (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1]), span)),
                (
                    (
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 2]), span),
                        (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 2, 3]), span)),
                        (
                            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 2, 3]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 0, 2, 3]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 0, 2, 3]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 3]), span),
                        )
                    ),
                    SExpr::Bool(Bool(false), span)
                )
            ),
            SExpr::Bool(Bool(true), span),
        );

        assert_eq!(result, expected);

        let outer_temp_id = first(&nth(&first(&result).unwrap(), 1).unwrap()).unwrap();
        let inner_temp_id = first(
            &nth(
                &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        let if_expr = nth(
            &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
            2,
        )
        .unwrap();

        assert_ne!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_ne!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );
    }

    #[test]
    fn test_expand_let_syntax_to_num() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
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
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();

        let span = Span { lo: 0, hi: 0 };
        let expected = SExpr::Num(Num(1.0), span);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_let_syntax_via_or_macro() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
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
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            (
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1]), span),
                (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2]), span)),
                (
                    (
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 3]), span),
                        (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 3, 4]), span)),
                        (
                            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 1, 3, 4]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 3, 4]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 3, 4]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 4]), span)
                        )
                    ),
                    SExpr::Bool(Bool(false), span)
                ),
            ),
            SExpr::Bool(Bool(true), span),
        );
        assert_eq!(result, expected);

        let outer_temp_id = first(&nth(&first(&result).unwrap(), 1).unwrap()).unwrap();
        let inner_temp_id = first(
            &nth(
                &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        let if_expr = nth(
            &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
            2,
        )
        .unwrap();

        assert_ne!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_ne!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );
    }

    #[test]
    fn test_expand_let_syntax_or_macro_0_arg_maintains_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
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
                   (or))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let expected = SExpr::Bool(Bool(false), Span { lo: 420, hi: 424 });

        assert!(
            result.is_idential(&expected),
            "result: {}\nexpected: {}",
            result,
            expected
        );
    }

    #[test]
    fn test_expand_let_syntax_or_macro_1_arg_maintains_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
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
                   (or 1))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let expected = SExpr::Num(Num(1.0), Span { lo: 424, hi: 425 });

        assert!(
            result.is_idential(&expected),
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }
}
