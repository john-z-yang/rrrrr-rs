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
        sexpr::Cons,
        util::{dotted_tail, len, rest, try_for_each, try_map},
    },
    if_let_sexpr, match_sexpr, template_sexpr,
};

type Env = HashMap<Symbol, Transformer>;

pub(crate) fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.add_scope(Bindings::CORE_SCOPE)
}

pub fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    expand_sexpr(sexpr, bindings, env, Context::TopLevel)
}

#[derive(PartialEq, Clone, Copy, Eq, Hash, Debug)]
enum Context {
    TopLevel,
    Expression,
    Body,
}

fn expand_sexpr(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if let SExpr::Nil(span) = sexpr {
        return Err(CompilationError {
            span: *span,
            reason: "Unexpected nil".to_owned(),
        });
    };
    if let SExpr::Id(..) = sexpr {
        return expand_id(sexpr, bindings);
    }
    if_let_sexpr! {(SExpr::Id(..), ..) = sexpr =>
        return expand_id_application(sexpr, bindings, env, ctx);
    };
    if_let_sexpr! {(..) = sexpr =>
        return expand_fn_application(sexpr, bindings, env);
    };
    Ok(sexpr.clone())
}

fn expand_id(sexpr: &SExpr, bindings: &mut Bindings) -> Result<SExpr> {
    let SExpr::Id(id, span) = sexpr else {
        unreachable!("expand_id is expecting an ID");
    };
    bindings.resolve_sym(id).ok_or(CompilationError {
        span: *span,
        reason: format!("ID: {} is unbound", id),
    })?;
    Ok(sexpr.clone())
}

fn expand_id_application(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    let binding = match first(sexpr) {
        Some(SExpr::Id(id, span)) => bindings.resolve_sym(&id).ok_or_else(|| CompilationError {
            span,
            reason: format!("ID: {} is unbound", id),
        })?,
        _ => unreachable!("first element of ID application must be an ID"),
    };

    match binding.0.as_str() {
        "quote" | "quote-syntax" => Ok(sexpr.clone()),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env, ctx),
        "lambda" => expand_lambda(sexpr, bindings, env),
        "define" => expand_define(sexpr, bindings, env, ctx),
        "set!" => expand_set(sexpr, bindings, env, ctx),
        "begin" => expand_begin(sexpr, bindings, env, ctx),
        _ => {
            if let Some(transformer) = env.get(&binding) {
                let scope_id = bindings.new_scope_id();
                let sexpr = sexpr.add_scope(scope_id);
                let transformed_sexpr =
                    transformer
                        .transform(&sexpr, bindings)
                        .ok_or_else(|| CompilationError {
                            span: sexpr.get_span(),
                            reason: format!(
                                "Unable to apply transformer: {}, no rules match",
                                binding
                            ),
                        })??;
                expand_sexpr(&transformed_sexpr.flip_scope(scope_id), bindings, env, ctx)
            } else {
                expand_fn_application(sexpr, bindings, env)
            }
        }
    }
}

fn expand_fn_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    try_map(
        |sub_sexpr| expand_sexpr(sub_sexpr, bindings, env, Context::Expression),
        sexpr,
    )
}

fn expand_begin(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if dotted_tail(sexpr).is_some() {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid use of begin form: expected a proper list".to_owned(),
        });
    }
    if len(sexpr) == 1 {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid use of begin form: must have at least 1 expression".to_owned(),
        });
    }
    try_map(
        |sub_sexpr| expand_sexpr(sub_sexpr, bindings, env, ctx),
        sexpr,
    )
}

fn expand_set(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if_let_sexpr! {(set, var @ SExpr::Id(id, span), exp) = sexpr =>
        let resolved = bindings.resolve_sym(id);
        let Some(resolved) = resolved else {
            return Err(CompilationError {
                span: *span,
                reason: format!("ID: {} is unbound", id),
            })
        };
        if Bindings::CORE_BINDINGS.contains(&resolved.0.as_str()) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Cannot mutate core binding: {}", id),
            })
        }
        let exp = expand_sexpr(exp, bindings, env, ctx)?;
        return Ok(template_sexpr!((set.clone(), var.clone(), exp) => sexpr).unwrap());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid use of set! form".to_owned(),
    })
}

fn expand_define(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if_let_sexpr! {(define, var @ SExpr::Id(id, _), exp) = sexpr =>
        if ctx == Context::Expression {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Cannot use define in an expression context".to_owned(),
            });
        }
        if ctx == Context::TopLevel {
            let binding = bindings.gen_sym(id);
            bindings.add_binding(id, &binding);
        }
        let exp = expand_sexpr(exp, bindings, env, ctx)?;
        return Ok(template_sexpr!((define.clone(), var.clone(), exp) => sexpr).unwrap());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid use of define form".to_owned(),
    })
}

fn expand_lambda(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (lambda, (args @ ..), body @ ..) => {
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
                    let binding = bindings.gen_sym(id);
                    bindings.add_binding(id, &binding);
                    Ok(())
                },
                &args,
            )?;

            match dotted_tail(&args) {
                None => {}
                Some(SExpr::Id(id, _)) => {
                    let binding = bindings.gen_sym(&id);
                    bindings.add_binding(&id, &binding);
                }
                Some(tail) => {
                    return Err(CompilationError {
                        span: tail.get_span(),
                        reason: format!(
                            "Expected identifiers in function parameters, but got: {}",
                            tail
                        ),
                    });
                }
            };

            let body = expand_body(&body.add_scope(scope_id), bindings, env)?;
            Ok(template_sexpr!((lambda.clone(), args, ..body) => sexpr).unwrap())
        },
        (lambda, arg @ SExpr::Id(..), body @ ..) => {
            let scope_id = bindings.new_scope_id();
            let arg = arg.add_scope(scope_id);
            let SExpr::Id(id, _) = &arg else {
                unreachable!("arg is already a SExpr::Id(..)")
            };
            let binding = bindings.gen_sym(id);
            bindings.add_binding(id, &binding);

            let body = expand_body(&body.add_scope(scope_id), bindings, env)?;
            Ok(template_sexpr!((lambda.clone(), arg, ..body) => sexpr).unwrap())
        },
        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid use of lambda form".to_owned(),
            })
        }
    }
}

fn expand_body(body: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    let body = body.add_scope(bindings.new_scope_id());
    let body = normalize_body(&body, bindings, NormalizationPhase::Define)?.0;

    try_map(
        |sexpr| expand_sexpr(sexpr, bindings, env, Context::Body),
        &body,
    )
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum NormalizationPhase {
    Define,
    Body,
}

fn normalize_body(
    body: &SExpr,
    bindings: &mut Bindings,
    phase: NormalizationPhase,
) -> Result<(SExpr, NormalizationPhase)> {
    let SExpr::Cons(cons, span) = body else {
        return Ok((body.clone(), phase));
    };

    if_let_sexpr! {((SExpr::Id(id, _), ..), remaining @ ..) = body =>
        if bindings.resolve(id).is_some_and(|id| id.symbol.0 == "define") {
            let define = first(body).unwrap();
            if phase != NormalizationPhase::Define {
                return Err(CompilationError {
                    span: define.get_span(),
                    reason: "Not allow to use define outside of define phase".to_owned(),
                });
            }
            collect_define(&define, bindings)?;
            let (cdr, next_phase) =
                normalize_body(&cons.cdr, bindings, NormalizationPhase::Define)?;
            return Ok((
                SExpr::Cons(Cons::new(*cons.car.clone(), cdr), *span),
                next_phase,
            ));
        }
        if bindings.resolve(id).is_some_and(|id| id.symbol.0 == "begin") {
            let begin = &first(body).unwrap();
            if dotted_tail(begin).is_some() {
                return Err(CompilationError {
                    span: begin.get_span(),
                    reason: "Invalid use of begin form: expected a proper list".to_owned(),
                });
            }
            if len(begin) == 1 {
                return Err(CompilationError {
                    span: begin.get_span(),
                    reason: "begin form must have at least 1 expression".to_owned(),
                });
            }
            let (head, next_phase) = normalize_body(
                &rest(begin).unwrap(),
                bindings,
                phase,
            )?;
            let (remaining, next_phase) = normalize_body(remaining, bindings, next_phase)?;
            return Ok((append_body_list(&head, &remaining)?, next_phase));
        }
    }

    let (cdr, _) = normalize_body(&cons.cdr, bindings, NormalizationPhase::Body)?;
    Ok((
        SExpr::Cons(Cons::new(*cons.car.clone(), cdr), *span),
        NormalizationPhase::Body,
    ))
}

fn append_body_list(head: &SExpr, tail: &SExpr) -> Result<SExpr> {
    if let SExpr::Nil(_) = head {
        return Ok(tail.clone());
    }
    let SExpr::Cons(cons, span) = head else {
        return Err(CompilationError {
            span: head.get_span(),
            reason: "Invalid body normalization: expected a proper list".to_owned(),
        });
    };
    Ok(SExpr::Cons(
        Cons::new(*cons.car.clone(), append_body_list(&cons.cdr, tail)?),
        *span,
    ))
}

fn collect_define(sexpr: &SExpr, bindings: &mut Bindings) -> Result<()> {
    if_let_sexpr! {(_, var @ SExpr::Id(id, span), _) = sexpr =>
        let resolved = bindings.resolve_scopes(id);
        if let Some(resolved) = resolved && resolved == id.scopes {
            return Err(CompilationError {
                span: *span,
                reason: format!("ID: {} is already bound within the same scope", var),
            })
        }
        let binding = bindings.gen_sym(id);
        bindings.add_binding(id, &binding);
        return Ok(());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid use of define form".to_owned(),
    })
}

fn expand_letrec_syntax(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if_let_sexpr! {(_, (specs @ ..), body) = sexpr =>
        let scope_id = bindings.new_scope_id();
        let mut transformer_bindings = vec![];

        if let Err(e) = try_for_each(|spec| {
            if_let_sexpr! {(keyword, transformer_spec) = spec =>
                let keyword = keyword.add_scope(scope_id);

                let SExpr::Id(id, _) = keyword else {
                    return Err(CompilationError {
                        span: keyword.get_span(),
                        reason: format!(
                            "Expected identifiers in syntax keyword, but got: {}",
                            keyword
                        ),
                    });
                };
                let binding = bindings.gen_sym(&id);
                bindings.add_binding(&id, &binding);

                let transformer_spec = transformer_spec.add_scope(scope_id);
                if !matches!(
                    first(&transformer_spec),
                    Some(SExpr::Id(id, _)) if bindings.resolve_sym(&id) == Some(Symbol::new("syntax-rules"))
                ) {
                    return Err(CompilationError {
                        span: transformer_spec.get_span(),
                        reason: "Expected syntax-rules transformer spec".to_owned(),
                    });
                }
                let transformer = Transformer::new(&transformer_spec)?;
                env.insert(binding.clone(), transformer);
                transformer_bindings.push(binding);
            }
            Ok(())
        }, specs) {
            transformer_bindings.iter().for_each(|transformer_binding| {
                env.remove_entry(transformer_binding);
            });
            return Err(e);
        }

        let res = expand_sexpr(&body.add_scope(scope_id), bindings, env, ctx);
        transformer_bindings.iter().for_each(|transformer_binding| {
            env.remove_entry(transformer_binding);
        });

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
            compilation_error::CompilationError,
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
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
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
                    SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 44, hi: 48 }),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 61, hi: 62 }),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), Span { lo: 75, hi: 76 }),
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
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                (SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3]), span)),
                (
                    SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                )
            ),
            (
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_lambda_dotted_params() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = parse(&tokenize("(lambda (x y . z) (cons x z))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1]), span),
                ..SExpr::Id(Id::new("z", [Bindings::CORE_SCOPE, 1]), span)
            ),
            (
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("z", [Bindings::CORE_SCOPE, 1, 2]), span),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_lambda_symbol_param() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = parse(&tokenize("(lambda x (cons x x))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), span),
            SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1]), span),
            (
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_lambda_invalid_non_id_param() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = parse(&tokenize("(lambda 42 x)").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_lambda_invalid_dotted_param() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = parse(&tokenize("(lambda (x . 42) x)").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_set_unbound_identifier_reports_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(set! x 1)").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
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
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(set! cons 1)").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 0, hi: 13 },
                    reason: _
                })
            ),
            "Expected set! on core binding to report whole-form span"
        );
    }

    #[test]
    fn test_expand_begin_in_expression_context_rejects_define_with_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(lambda () (cons (begin (define x 1) x) 1))").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
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
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(lambda () (begin 1) (define x 2))").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 21, hi: 33 },
                    reason: _
                })
            ),
            "Expected non-leading internal define to report define form span"
        );
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
                .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
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
                .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
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
                .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
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
                .resolve_sym(&Id::new("and", [Bindings::CORE_SCOPE]))
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
                .resolve_sym(&(first(&result).unwrap().try_into().unwrap()))
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
                .resolve_sym(&Id::new("my-macro", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = parse(&tokenize("(my-macro x)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1]), span),
            (SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span)),
            SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 2, 3]), span),
        );
        assert_eq!(result, expected);
        assert_ne!(
            bindings
                .resolve_sym(
                    &first(&nth(&result, 1).unwrap())
                        .unwrap()
                        .try_into()
                        .unwrap()
                )
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
                .resolve_sym(&Id::new("my-or", [Bindings::CORE_SCOPE]))
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
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 3]), span),
                        (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4]), span)),
                        (
                            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 3, 4, 5]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 4, 5]), span),
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
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 4]), span),
                        (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 4, 5]), span)),
                        (
                            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 1, 4, 5, 6]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 4, 5, 6]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 4, 5, 6]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 3, 5, 6]), span)
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

    #[test]
    fn test_shadowed_syntax_rules_is_rejected() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize(
                r#"
                (lambda (syntax-rules)
                  (letrec-syntax
                    ((my-mac (syntax-rules ()
                               ((_) 1))))
                    (my-mac)))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
        assert!(
            result.is_err(),
            "Expected error when syntax-rules is shadowed by a lambda parameter"
        );
    }

    #[test]
    fn test_literal_matching_respects_lexical_binding() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize(
                r#"
                (letrec-syntax
                  ((my-mac (syntax-rules (list)
                             ((_ list) 1)
                             ((_ x) 2))))
                  (lambda (list) (my-mac list)))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

        let body = nth(&result, 2).unwrap();
        assert_eq!(body, SExpr::Num(Num(2.0), body.get_span()));
    }

    #[test]
    fn test_letrec_syntax_allows_multiple_transformer_bindings() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize(
                r#"
                (letrec-syntax
                  ((one (syntax-rules () ((_) 1)))
                   (two (syntax-rules () ((_) 2))))
                  (one))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
        assert!(
            result.is_ok(),
            "Expected multi-binding letrec-syntax to expand, got: {:?}",
            result
        );
        let result = result.unwrap();
        assert_eq!(result, SExpr::Num(Num(1.0), result.get_span()));
    }

    #[test]
    fn test_expand_letrec_syntax_cleans_env_after_success() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
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
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
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
        let mut env = HashMap::<Symbol, Transformer>::new();
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
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
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
    fn test_expand_letrec_syntax_unbound_ellipsis_in_template_reports_error() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(letrec-syntax ((m (syntax-rules () ((_ x) (...))))) (m 1))").unwrap())
                .unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 44, hi: 47 },
                    reason
                }) if reason == "Unbound '...'"
            ),
            "Expected malformed ellipsis template usage to return a compilation error"
        );
    }

    #[test]
    fn test_expand_top_level_begin_define_persists_binding_for_following_expand() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let begin_expr = parse(&tokenize("(begin (define x 1) x)").unwrap()).unwrap();
        let id_expr = parse(&tokenize("x").unwrap()).unwrap();

        let first_result = expand(&introduce(&begin_expr), &mut bindings, &mut env);
        assert!(
            first_result.is_ok(),
            "Expected top-level begin with define to expand successfully"
        );
        let second_result = expand(&introduce(&id_expr), &mut bindings, &mut env);
        assert!(
            second_result.is_ok(),
            "Expected identifier defined inside top-level begin to remain bound for later expansion"
        );
    }

    #[test]
    fn test_expand_lambda_internal_define_inside_begin() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(lambda () (begin (define x 1) x))").unwrap()).unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

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
            bindings.resolve_sym(&defined_var).unwrap(),
            bindings.resolve_sym(&last_body_expr).unwrap(),
            "Expected body reference to resolve to internal define from spliced begin"
        );
    }

    #[test]
    fn test_expand_lambda_begin_requires_expression() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(lambda () (begin))").unwrap()).unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
        assert!(result.is_err(), "Expected begin with no body forms to fail");
    }

    #[test]
    fn test_expand_lambda_shadowed_begin_is_not_spliced() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(lambda () (define begin (lambda x x)) (begin 1 2 3))").unwrap())
                .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

        let defined_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
        let begin_call = nth(&result, 3).unwrap();
        let begin_head = first(&begin_call).unwrap();
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
            bindings.resolve_sym(&defined_var).unwrap(),
            bindings.resolve_sym(&begin_head).unwrap(),
            "Expected begin call to resolve to shadowing local binding"
        );
    }

    #[test]
    fn test_expand_lambda_begin_binding_defined_inside_spliced_begin_shadows_nested_begin() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize("(lambda () (begin (define begin (lambda x x)) (begin 1 2)))").unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

        let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
        let nested_begin_call = nth(&result, 3).unwrap();
        let nested_begin_head = first(&nested_begin_call).unwrap();
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
        let define_sym = bindings.resolve_sym(&define_begin_var).unwrap();
        let nested_head_sym = bindings.resolve_sym(&nested_begin_head).unwrap();
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
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize("(lambda () (begin (define begin (lambda x x))) (begin 1 2))").unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

        let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
        let following_begin_call = nth(&result, 3).unwrap();
        let following_begin_head = first(&following_begin_call).unwrap();
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
        let define_sym = bindings.resolve_sym(&define_begin_var).unwrap();
        let following_head_sym = bindings.resolve_sym(&following_begin_head).unwrap();
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
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(
            &tokenize("(lambda () (begin (define begin (lambda x x)) begin (begin 1 2)))").unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env).unwrap();

        let define_begin_var = nth(&nth(&result, 2).unwrap(), 1).unwrap();
        let begin_reference = nth(&result, 3).unwrap();
        let begin_call = nth(&result, 4).unwrap();
        let begin_call_head = first(&begin_call).unwrap();
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
        let define_sym = bindings.resolve_sym(&define_begin_var).unwrap();
        let reference_sym = bindings.resolve_sym(&begin_reference).unwrap();
        let call_head_sym = bindings.resolve_sym(&begin_call_head).unwrap();
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
    fn test_expand_lambda_begin_improper_tail_reports_error_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(lambda () (begin 1 . 2))").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 11, hi: 24 },
                    reason: _
                })
            ),
            "Expected improper begin in lambda body to report begin span"
        );
    }

    #[test]
    fn test_expand_lambda_define_after_spliced_begin_is_collected() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(lambda () (begin (define x 1)) (define y 2) y)").unwrap()).unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
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
            bindings.resolve_sym(&defined_var_y).unwrap(),
            bindings.resolve_sym(&final_expr).unwrap(),
            "Expected final y reference to resolve to collected internal define"
        );
    }

    #[test]
    fn test_expand_lambda_multiple_begin_define_groups_stay_in_define_phase() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(lambda () (begin (define x 1)) (begin (define y 2)) y)").unwrap())
                .unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
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
            bindings.resolve_sym(&defined_var_y).unwrap(),
            bindings.resolve_sym(&final_expr).unwrap(),
            "Expected final y reference to resolve to second internal define"
        );
    }

    #[test]
    fn test_expand_begin_improper_list_reports_error_span() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(begin 1 . 2)").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 0, hi: 13 },
                    reason: _
                })
            ),
            "Expected improper top-level begin to report whole form span"
        );
    }
}
