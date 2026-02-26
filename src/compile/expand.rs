use std::{collections::HashMap, fmt};

use super::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{SExpr, Symbol},
    transformer::Transformer,
    util::try_first,
};
use crate::{
    compile::{
        compilation_error::CompilationError,
        sexpr::{Cons, Id},
        util::{append, first, is_proper_list, len, rest, try_dotted_tail, try_for_each, try_map},
    },
    if_let_sexpr, match_sexpr, template_sexpr,
};

type Env = HashMap<Symbol, Transformer>;

pub(crate) fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.add_scope(Bindings::CORE_SCOPE)
}

pub(crate) fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
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
            reason: "Unexpected empty list in expression position".to_owned(),
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
        unreachable!("expand_id expected an ID");
    };
    bindings.resolve_sym(id).ok_or(CompilationError {
        span: *span,
        reason: format!("Unbound identifier: '{}'", id),
    })?;
    Ok(sexpr.clone())
}

fn apply_transformer(
    sexpr: &SExpr,
    transformer: &Transformer,
    name: &Id,
    bindings: &mut Bindings,
) -> Result<SExpr> {
    let scope_id = bindings.new_scope_id();
    let scoped = sexpr.add_scope(scope_id);
    let transformed =
        transformer
            .transform(&scoped, bindings)
            .ok_or_else(|| CompilationError {
                span: scoped.get_span(),
                reason: format!("No matching rule for macro '{}'", name),
            })??;
    Ok(transformed.flip_scope(scope_id))
}

fn expand_id_application(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    let (id, binding) = match try_first(sexpr) {
        Some(SExpr::Id(id, span)) => {
            let binding = bindings.resolve_sym(&id).ok_or_else(|| CompilationError {
                span,
                reason: format!("Unbound identifier: '{}'", id),
            })?;
            (id, binding)
        }
        _ => unreachable!("expand_id_application expected first element to be an ID"),
    };

    match binding.0.as_str() {
        "quote" | "quote-syntax" => Ok(sexpr.clone()),
        "let-syntax" => expand_let_syntax(sexpr, bindings, env),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env),
        "lambda" => expand_lambda(sexpr, bindings, env),
        "define" => expand_define(sexpr, bindings, env, ctx),
        "set!" => expand_set(sexpr, bindings, env),
        "begin" => expand_begin(sexpr, bindings, env, ctx),
        _ => {
            if let Some(transformer) = env.get(&binding) {
                expand_sexpr(
                    &apply_transformer(sexpr, transformer, &id, bindings)?,
                    bindings,
                    env,
                    ctx,
                )
            } else {
                expand_fn_application(sexpr, bindings, env)
            }
        }
    }
}

fn expand_fn_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    try_map(sexpr, |sub_sexpr| {
        expand_sexpr(sub_sexpr, bindings, env, Context::Expression)
    })
}

fn expand_begin(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if !is_proper_list(sexpr) {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid 'begin' form: expected a proper list".to_owned(),
        });
    }
    if len(sexpr) == 1 {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid 'begin' form: expected at least one expression".to_owned(),
        });
    }
    try_map(sexpr, |sub_sexpr| {
        expand_sexpr(sub_sexpr, bindings, env, ctx)
    })
}

fn expand_set(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    if_let_sexpr! {(set, var @ SExpr::Id(id, span), exp) = sexpr =>
        let resolved = bindings.resolve_sym(id);
        let Some(resolved) = resolved else {
            return Err(CompilationError {
                span: *span,
                reason: format!("Unbound identifier: '{}'", id),
            })
        };
        if Bindings::CORE_BINDINGS.contains(&resolved.0.as_str()) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Cannot mutate core binding '{}'", id),
            })
        }
        let exp = expand_sexpr(exp, bindings, env, Context::Expression)?;
        return Ok(template_sexpr!((set.clone(), var.clone(), exp) => sexpr).unwrap());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'set!' form".to_owned(),
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
                reason: "'define' is not allowed in an expression context".to_owned(),
            });
        }
        if ctx == Context::TopLevel {
            let binding = bindings.gen_sym(id);
            bindings.add_binding(id, &binding);
        }
        let exp = expand_sexpr(exp, bindings, env, Context::Expression)?;
        return Ok(template_sexpr!((define.clone(), var.clone(), exp) => sexpr).unwrap());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'define' form".to_owned(),
    })
}

fn expand_lambda(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (lambda, (args @ ..), body @ ..) => {
            let scope_id = bindings.new_scope_id();
            let args = args.add_scope(scope_id);

            try_for_each(&args, |arg| {
                let SExpr::Id(id, _) = arg else {
                    return Err(CompilationError {
                        span: arg.get_span(),
                        reason: format!(
                            "Expected an identifier in function parameters, but got: {}",
                            arg
                        ),
                    });
                };
                let binding = bindings.gen_sym(id);
                bindings.add_binding(id, &binding);
                Ok(())
            })?;

            match try_dotted_tail(&args) {
                None | Some(SExpr::Nil(_)) => {}
                Some(SExpr::Id(id, _)) => {
                    let binding = bindings.gen_sym(&id);
                    bindings.add_binding(&id, &binding);
                }
                Some(tail) => {
                    return Err(CompilationError {
                        span: tail.get_span(),
                        reason: format!(
                            "Expected an identifier as rest parameter, but got: {}",
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
                reason: "Invalid 'lambda' form".to_owned(),
            })
        }
    }
}

fn expand_body(body: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    let body = body.add_scope(bindings.new_scope_id());
    let body = normalize_body(&body, bindings, env, NormalizationPhase::Define)?.0;

    try_map(&body, |sexpr| {
        expand_sexpr(sexpr, bindings, env, Context::Body)
    })
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum NormalizationPhase {
    Define,
    Body,
}

enum BodyFormKind {
    Define,
    Begin,
    Other,
}

fn partial_expand_body(
    form: &SExpr,
    bindings: &mut Bindings,
    env: &Env,
) -> Result<(SExpr, BodyFormKind)> {
    let mut form = form.clone();
    loop {
        let Some(SExpr::Id(id, _)) = try_first(&form) else {
            return Ok((form, BodyFormKind::Other));
        };
        let Some(binding) = bindings.resolve_sym(&id) else {
            return Ok((form, BodyFormKind::Other));
        };
        match binding.0.as_str() {
            "define" => return Ok((form, BodyFormKind::Define)),
            "begin" => return Ok((form, BodyFormKind::Begin)),
            _ => {
                let Some(transformer) = env.get(&binding) else {
                    return Ok((form, BodyFormKind::Other));
                };
                form = apply_transformer(&form, transformer, &id, bindings)?;
            }
        }
    }
}

fn normalize_body(
    body: &SExpr,
    bindings: &mut Bindings,
    env: &Env,
    phase: NormalizationPhase,
) -> Result<(SExpr, NormalizationPhase)> {
    let SExpr::Cons(cons, span) = body else {
        return Ok((body.clone(), phase));
    };

    let (expanded_car, kind) = partial_expand_body(&cons.car, bindings, env)?;

    match kind {
        BodyFormKind::Define => {
            if phase != NormalizationPhase::Define {
                return Err(CompilationError {
                    span: expanded_car.get_span(),
                    reason: "'define' must appear at the beginning of a body".to_owned(),
                });
            }
            collect_define(&expanded_car, bindings)?;
            let (cdr, next_phase) =
                normalize_body(&cons.cdr, bindings, env, NormalizationPhase::Define)?;
            Ok((SExpr::Cons(Cons::new(expanded_car, cdr), *span), next_phase))
        }
        BodyFormKind::Begin => {
            if !is_proper_list(&expanded_car) {
                return Err(CompilationError {
                    span: expanded_car.get_span(),
                    reason: "Invalid 'begin' form: expected a proper list".to_owned(),
                });
            }
            if len(&expanded_car) == 1 {
                return Err(CompilationError {
                    span: expanded_car.get_span(),
                    reason: "Invalid 'begin' form: expected at least one expression".to_owned(),
                });
            }
            let (head, next_phase) = normalize_body(&rest(&expanded_car), bindings, env, phase)?;
            let (remaining, next_phase) = normalize_body(&cons.cdr, bindings, env, next_phase)?;
            Ok((append(&head, &remaining), next_phase))
        }
        BodyFormKind::Other => {
            let (cdr, _) = normalize_body(&cons.cdr, bindings, env, NormalizationPhase::Body)?;
            Ok((
                SExpr::Cons(Cons::new(expanded_car, cdr), *span),
                NormalizationPhase::Body,
            ))
        }
    }
}

fn collect_define(sexpr: &SExpr, bindings: &mut Bindings) -> Result<()> {
    if_let_sexpr! {(_, var @ SExpr::Id(id, span), _) = sexpr =>
        let resolved = bindings.resolve_scopes(id);
        if let Some(resolved) = resolved && resolved == id.scopes {
            return Err(CompilationError {
                span: *span,
                reason: format!("Duplicate definition: '{}' is already bound in this scope", var),
            })
        }
        let binding = bindings.gen_sym(id);
        bindings.add_binding(id, &binding);
        return Ok(());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'define' form".to_owned(),
    })
}

enum SyntaxBindingForm {
    LetSyntax,
    LetrecSyntax,
}

impl fmt::Display for SyntaxBindingForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SyntaxBindingForm::LetSyntax => "let-syntax",
                SyntaxBindingForm::LetrecSyntax => "letrec-syntax",
            }
        )
    }
}

fn expand_let_syntax(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    expand_syntax_binding(sexpr, bindings, env, SyntaxBindingForm::LetSyntax)
}

fn expand_letrec_syntax(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    expand_syntax_binding(sexpr, bindings, env, SyntaxBindingForm::LetrecSyntax)
}

fn expand_syntax_binding(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    form: SyntaxBindingForm,
) -> Result<SExpr> {
    if_let_sexpr! {(_, (binding_pairs @ ..), body @ ..) = sexpr =>
        if !is_proper_list(body) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Invalid '{form}' body: expected a proper list"),
            });
        }
        if len(body) == 0 {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Invalid '{form}' body: expected at least one expression"),
            });
        }
        let scope_id = bindings.new_scope_id();
        let mut transformer_bindings = vec![];

        if let Err(e) = try_for_each(binding_pairs, |binding_pair| {
            if_let_sexpr! {(SExpr::Id(id, _), transformer_spec) = binding_pair =>
                let id = id.add_scope(scope_id);
                let transformer_spec = match form {
                    SyntaxBindingForm::LetSyntax => transformer_spec,
                    SyntaxBindingForm::LetrecSyntax => &transformer_spec.add_scope(scope_id)
                };

                let binding = bindings.gen_sym(&id);
                bindings.add_binding(&id, &binding);

                if !matches!(
                    try_first(transformer_spec),
                    Some(SExpr::Id(id, _)) if bindings.resolve_sym(&id) == Some(Symbol::new("syntax-rules"))
                ) {
                    return Err(CompilationError {
                        span: transformer_spec.get_span(),
                        reason: "Expected a 'syntax-rules' transformer".to_owned(),
                    });
                }
                let transformer = Transformer::new(transformer_spec)?;
                env.insert(binding.clone(), transformer);
                transformer_bindings.push(binding);
                return Ok(());
            }
            Err(CompilationError {
                span: binding_pair.get_span(),
                reason: format!("Invalid '{form}' binding pair: expected (identifier 'syntax-rules' transformer)"),
            })
        }) {
            transformer_bindings.iter().for_each(|transformer_binding| {
                env.remove_entry(transformer_binding);
            });
            return Err(e);
        }

        let body = expand_body(&body.add_scope(scope_id), bindings, env).map(|body| {
            if len(&body) == 1 {
                first(&body)
            } else {
                SExpr::cons(
                    SExpr::Id(Id::new("begin", [Bindings::CORE_SCOPE]), body.get_span()),
                    body,
                )
            }
        });

        transformer_bindings.iter().for_each(|transformer_binding| {
            env.remove_entry(transformer_binding);
        });

        return body;
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: format!("Invalid '{form}' form"),
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
            util::first,
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

    fn expand_source_with_fresh_state(source: &str) -> (Bindings, Result<SExpr>) {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize(source).unwrap()).unwrap();
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
        (bindings, result)
    }

    fn assert_generated_define_is_referenced(source: &str, expand_message: &str) {
        let (bindings, result) = expand_source_with_fresh_state(source);
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
            bindings.resolve_sym(&defined_var).unwrap(),
            bindings.resolve_sym(&body_ref).unwrap(),
            "Expected body reference to resolve to generated define"
        );
    }

    use super::*;

    #[test]
    fn test_introduce() {
        let list = parse(&tokenize("(cons 0 1)").unwrap()).unwrap();
        let span = Span { lo: 0, hi: 0 };
        assert_eq!(
            introduce(&list).without_spans(),
            sexpr!(
                SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), span),
                SExpr::Num(Num(0.0), span),
                SExpr::Num(Num(1.0), span),
            )
            .without_spans()
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
        assert_eq!(result.without_spans(), expected.without_spans());
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

        assert!(result == expected);
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
    fn test_expand_define_rhs_rejects_nested_define_in_expression_context() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr = parse(&tokenize("(define x (define y 1))").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError { reason, .. }) if reason == "'define' is not allowed in an expression context"
            ),
            "Expected define RHS to be expanded in expression context"
        );
    }

    #[test]
    fn test_expand_set_rhs_rejects_nested_define_in_expression_context() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let expr =
            parse(&tokenize("(begin (define x 1) (set! x (define y 2)) x)").unwrap()).unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError { reason, .. }) if reason == "'define' is not allowed in an expression context"
            ),
            "Expected set! RHS to be expanded in expression context"
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
            expand(&introduce(&sexpr), &mut bindings, &mut env)
                .unwrap()
                .without_spans(),
            sexpr!(SExpr::Bool(Bool(false), span)).without_spans()
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(result.without_spans(), expected.without_spans());
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
                SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 3]), span)),
                (
                    (
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 5]), span),
                        (SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6]), span)),
                        (
                            SExpr::Id(Id::new("if", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                            SExpr::Id(Id::new("temp", [Bindings::CORE_SCOPE, 1, 5, 6, 7]), span),
                            SExpr::Id(
                                Id::new("temp", [Bindings::CORE_SCOPE, 1, 2, 3, 4, 6, 7]),
                                span
                            )
                        )
                    ),
                    SExpr::Bool(Bool(false), span)
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
        let expected = SExpr::Bool(Bool(false), Span { lo: 105, hi: 107 });

        assert!(
            result == expected,
            "result: {:?}\nexpected: {:?}",
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
            result == expected,
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }

    #[test]
    fn test_expand_let_syntax_has_body_ctx() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
            &tokenize(
                r#"
                (letrec-syntax
                    ((one (syntax-rules ()
                            ((_) 1))))
                (define x 1))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        assert!(
            bindings
                .resolve(&Id::new("x", [Bindings::CORE_SCOPE]))
                .is_none()
        );
    }

    #[test]
    fn test_expand_let_syntax_multiple_body_exprs_recursive_defn() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
            &tokenize(
                r#"
                (letrec-syntax
                    ((one (syntax-rules ()
                            ((_) 1)))
                    (two (syntax-rules ()
                            ((_) 2))))
                (define x (lambda () y))
                (define y (lambda () x)))
                "#,
            )
            .unwrap(),
        )
        .unwrap();
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("begin", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Id(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                (
                    SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                    (),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                )
            ),
            (
                SExpr::Id(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
                (
                    SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                    (),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2, 5, 6]), span),
                )
            )
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_let_syntax_multiple_body_exprs_() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = &parse(
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
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = sexpr!(
            SExpr::Id(Id::new("begin", [Bindings::CORE_SCOPE]), span),
            SExpr::Num(Num(1.0), span),
            SExpr::Num(Num(2.0), span),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
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
        assert_eq!(
            body.without_spans(),
            SExpr::Num(Num(2.0), body.get_span()).without_spans()
        );
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
        assert_eq!(
            result.without_spans(),
            SExpr::Num(Num(1.0), result.get_span()).without_spans()
        );
    }

    #[test]
    fn test_expand_letrec_syntax_internal_define_in_expression_position() {
        let (_, result) = expand_source_with_fresh_state(
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
        let expr = parse(
            &tokenize("(letrec-syntax ((m (syntax-rules () ((_ x) (...))))) (m 1))").unwrap(),
        )
        .unwrap();
        assert!(
            matches!(
                expand(&introduce(&expr), &mut bindings, &mut env),
                Err(CompilationError {
                    span: Span { lo: 44, hi: 47 },
                    reason
                }) if reason == "Unbound identifier: '...'"
            ),
            "Expected malformed ellipsis template usage to return a compilation error"
        );
    }

    #[test]
    fn test_let_syntax_basic_expansion() {
        let (_, result) = expand_source_with_fresh_state(
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
        let (_, result) = expand_source_with_fresh_state(
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
    fn test_let_syntax_bindings_are_not_recursive() {
        // In let-syntax, `two`'s transformer references `one`, but `one` is not
        // visible to sibling specs. When `(two)` expands to `(one)`, `one` is
        // unbound, producing an error.
        let (_, result) = expand_source_with_fresh_state(
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
    fn test_letrec_syntax_bindings_are_recursive() {
        let (_, result) = expand_source_with_fresh_state(
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
        // In letrec-syntax, (one) IS visible to two's transformer
        assert_eq!(
            result.without_spans(),
            SExpr::Num(Num(1.0), result.get_span()).without_spans(),
            "Expected letrec-syntax bindings to be recursive: (one) inside two's template should expand to 1"
        );
    }

    #[test]
    fn test_let_syntax_cleans_env_after_success() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
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
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
        assert!(result.is_ok(), "Expected let-syntax expression to expand");
        assert!(
            env.is_empty(),
            "Expected let-syntax to remove temporary transformer bindings from env"
        );
    }

    #[test]
    fn test_let_syntax_cleans_env_on_transformer_spec_error() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
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
        let result = expand(&introduce(&expr), &mut bindings, &mut env);
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
    fn test_let_syntax_body_expansion() {
        let (_, result) = expand_source_with_fresh_state(
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

    #[test]
    fn test_expand_lambda_macro_expanding_to_expression_ends_define_phase() {
        let (_, result) = expand_source_with_fresh_state(
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
}
