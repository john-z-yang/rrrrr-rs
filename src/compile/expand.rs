use std::{
    collections::{HashMap, HashSet},
    fmt, mem,
    sync::Arc,
};

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
        util::{
            append, first, is_proper_list, len, rest, try_dotted_tail, try_for_each, try_map,
            try_rest,
        },
    },
    if_let_sexpr, make_sexpr, match_sexpr, template_sexpr,
};

type Env = HashMap<Symbol, Arc<Transformer>>;

pub fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.add_scope(Bindings::CORE_SCOPE)
}

pub(crate) fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    let mut bindings_snapshot = bindings.clone();
    let mut env_snapshot = env.clone();
    let result = expand_sexpr(sexpr, bindings, env, Context::TopLevel);
    if result.is_err() {
        mem::swap(&mut bindings_snapshot, bindings);
        mem::swap(&mut env_snapshot, env);
    }
    result
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
            reason: "Unexpected empty list".to_owned(),
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
    match bindings.resolve_sym(id) {
        Some(symbol) if Bindings::CORE_FORMS.contains(&symbol.0.as_str()) => {
            Err(CompilationError {
                span: *span,
                reason: format!("Invalid '{}' form: not in parentheses", symbol),
            })
        }
        _ => Ok(sexpr.clone()),
    }
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
        Some(SExpr::Id(id, _)) => match bindings.resolve_sym(&id) {
            Some(binding) => (id, binding),
            None => {
                return expand_fn_application(sexpr, bindings, env);
            }
        },
        _ => unreachable!("expand_id_application expected first element to be an ID"),
    };

    match binding.0.as_str() {
        "quote" => Ok(sexpr.clone()),
        "quasiquote" => expand_quasiquote(sexpr, bindings, env, ctx),
        "unquote" | "unquote-splicing" => Err(CompilationError {
            span: sexpr.get_span(),
            reason: format!("Invalid '{}' form: not in 'quasiquote'", binding),
        }),
        "let-syntax" => expand_let_syntax(sexpr, bindings, env),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env),
        "lambda" => expand_lambda(sexpr, bindings, env),
        "define" => expand_define(sexpr, bindings, env, ctx),
        "define-syntax" => expand_define_syntax(sexpr, bindings, env, ctx),
        "set!" => expand_set(sexpr, bindings, env),
        "begin" => expand_begin(sexpr, bindings, env, ctx),
        "if" => expand_if(sexpr, bindings, env),
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

fn expand_if(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (iif, check, consequent, alternate) => {
            Ok(template_sexpr!((
                iif.clone(),
                expand_sexpr(&check.clone(), bindings, env, Context::Expression)?,
                expand_sexpr(&consequent.clone(), bindings, env, Context::Expression)?,
                expand_sexpr(&alternate.clone(), bindings, env, Context::Expression)?,
            ) => sexpr).unwrap())
        },

        (iif, check, consequent) => {
            Ok(template_sexpr!((
                iif.clone(),
                expand_sexpr(&check.clone(), bindings, env, Context::Expression)?,
                expand_sexpr(&consequent.clone(), bindings, env, Context::Expression)?,
            ) => sexpr).unwrap())
        },

        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'if' form: expected (if <test> <consequent> <alternate>) or (if <test> <consequent>)".to_owned(),
            })
        },
    }
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
    Ok(SExpr::cons(
        first(sexpr),
        try_map(&rest(sexpr), |sub_sexpr| {
            expand_sexpr(sub_sexpr, bindings, env, ctx)
        })?,
    )
    .update_span(sexpr.get_span()))
}

fn expand_set(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    if_let_sexpr! {(set, var @ SExpr::Id(id, _), exp) = sexpr =>
        if let Some(resolved) = bindings.resolve_sym(id)
            && (Bindings::CORE_FORMS.contains(&resolved.0.as_str()) || Bindings::CORE_PRIMITIVES.contains(&resolved.0.as_str())) {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: format!("Cannot mutate core forms/primatives '{}'", id),
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
    match_sexpr!(
        sexpr;

        (define, var @ SExpr::Id(id, _), exp) => {
            if matches!(ctx, Context::Expression) {
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
            Ok(template_sexpr!((define.clone(), var.clone(), exp) => sexpr).unwrap())
        },

        (define, (func_name @ SExpr::Id(..), args @ ..), body @ ..) => {
            expand_sexpr(
                &make_sexpr!(
                    define.clone(),
                    func_name.clone(),
                    (
                        SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        (*args).clone(),
                        ..(*body).clone(),
                    )
                ),
                bindings,
                env,
                ctx
            )
        },

        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'define' form".to_owned(),
            })
        },
    )
}

fn expand_define_syntax(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if_let_sexpr! {(_, SExpr::Id(id, _), transformer_spec) = sexpr =>
        if ctx != Context::TopLevel {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: "'define-syntax' is only allowed in the top level context".to_owned(),
            });
        }
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
        let binding = bindings.gen_sym(id);
        bindings.add_binding(id, &binding);
        env.insert(binding.clone(), Arc::new(transformer));

        return Ok(sexpr.clone());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'define-syntax' form".to_owned(),
    })
}

fn expand_lambda(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (lambda, (args @ ..), body @ ..) => {
            if len(body) == 0 {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: "Invalid 'lambda' form: expected at least one body expression".to_owned(),
                });
            }
            let scope_id = bindings.new_scope_id();
            let args = args.add_scope(scope_id);
            let mut seen = HashSet::new();

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
                if !seen.insert(id.symbol.clone()) {
                    return Err(CompilationError {
                        span: arg.get_span(),
                        reason: format!("Duplicate parameter: '{}'", id),
                    });
                }
                let binding = bindings.gen_sym(id);
                bindings.add_binding(id, &binding);
                Ok(())
            })?;

            match try_dotted_tail(&args) {
                None | Some(SExpr::Nil(_)) => {}
                Some(SExpr::Id(id, _)) => {
                    if !seen.insert(id.symbol.clone()) {
                        return Err(CompilationError {
                            span: sexpr.get_span(),
                            reason: format!("Duplicate parameter: '{}'", id),
                        });
                    }
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
            if len(body) == 0 {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: "Invalid 'lambda' form: expected at least one body expression".to_owned(),
                });
            }
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
    let (body, phase) = normalize_body(&body, bindings, env, NormalizationPhase::Define)?;
    if phase == NormalizationPhase::Define {
        return Err(CompilationError {
            span: body.get_span(),
            reason: "Invalid body: expected at least one expression after definitions".to_owned(),
        });
    }

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
    let (id, span) = match_sexpr! {
        sexpr;

        (_, SExpr::Id(id, span), _) => {
            (id, span)
        },

        (_, (SExpr::Id(id, span), _args @ ..), _) => {
            (id, span)
        },

        _ => {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'define' form".to_owned(),
            })
        },
    };

    let resolved = bindings.resolve_scopes(id);
    if let Some(resolved) = resolved
        && resolved == id.scopes
    {
        return Err(CompilationError {
            span: *span,
            reason: format!(
                "Duplicate definition: '{}' is already bound in this scope",
                id
            ),
        });
    }
    let binding = bindings.gen_sym(id);
    bindings.add_binding(id, &binding);

    Ok(())
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
    expand_syntax_binding(
        sexpr,
        bindings,
        &mut env.clone(),
        SyntaxBindingForm::LetSyntax,
    )
}

fn expand_letrec_syntax(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> Result<SExpr> {
    expand_syntax_binding(
        sexpr,
        bindings,
        &mut env.clone(),
        SyntaxBindingForm::LetrecSyntax,
    )
}

fn expand_syntax_binding(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    form: SyntaxBindingForm,
) -> Result<SExpr> {
    if_let_sexpr! {(_, (binding_pairs @ ..), body @ ..) = sexpr =>
        if !is_proper_list(binding_pairs) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Invalid '{form}' bindings: expected a proper list"),
            });
        }
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

        try_for_each(binding_pairs, |binding_pair| {
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
                env.insert(binding.clone(), Arc::new(transformer));
                return Ok(());
            }
            Err(CompilationError {
                span: binding_pair.get_span(),
                reason: format!("Invalid '{form}' binding pair: expected (identifier 'syntax-rules' transformer)"),
            })
        })?;

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

        return body;
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: format!("Invalid '{form}' form"),
    })
}

fn expand_quasiquote(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr> {
    if_let_sexpr! {(_, args) = sexpr => {
        return expand_sexpr(&expand_quasiquote_args(args, bindings, 0)?, bindings, env, ctx);
    }};
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'quasiquote': expected a single argument".to_owned(),
    })
}

fn expand_quasiquote_args_list(
    sexpr: &SExpr,
    bindings: &mut Bindings,
    depth: u32,
) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (car, cdr @ ..) => {
            if let SExpr::Id(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && binding.symbol.0 == "quasiquote"
            {
                Ok(make_sexpr!(
                    SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                            SExpr::Id(Id::new("quasiquote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        ),
                        expand_quasiquote_args(cdr, bindings, depth + 1)?,
                    ),
                ))
            } else if let SExpr::Id(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && (binding.symbol.0 == "unquote" || binding.symbol.0 == "unquote-splicing")
            {
                if depth > 0 {
                    Ok(make_sexpr!(
                        SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                            (
                                SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                                car.clone(),
                            ),
                            expand_quasiquote_args(cdr, bindings, depth - 1)?,
                        ),
                    ))
                } else if binding.symbol.0 == "unquote" {
                    Ok(make_sexpr!(
                        SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        ..(*cdr).clone(),
                    ))
                } else {
                    Ok(make_sexpr!(
                        SExpr::Id(Id::new("append", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        ..(*cdr).clone(),
                    ))
                }
            } else {
                Ok(make_sexpr!(
                    SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Id(Id::new("append", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        expand_quasiquote_args_list(car, bindings, depth)?,
                        expand_quasiquote_args(cdr, bindings, depth)?,
                    ),
                ))
            }
        },

        SExpr::Vector(vector, span) => {
            Ok(make_sexpr!(
                SExpr::Id(Id::new("list", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                (
                    SExpr::Id(Id::new("list->vector", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                    expand_quasiquote_args(&vector.clone().into_cons_list(*span), bindings, depth)?,
                ),
            ))
        },

        _ => {
            Ok(make_sexpr!(
                SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                (
                    sexpr.clone(),
                ),
            ))
        },
    }
}

fn expand_quasiquote_args(sexpr: &SExpr, bindings: &mut Bindings, depth: u32) -> Result<SExpr> {
    match_sexpr! {
        sexpr;

        (car, cdr @ ..) => {
            if let SExpr::Id(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && binding.symbol.0 == "quasiquote"
            {
                Ok(make_sexpr!(
                    SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Id(
                            Id::new("quote", [Bindings::CORE_SCOPE]),
                            sexpr.get_span(),
                        ),
                        SExpr::Id(
                            Id::new("quasiquote", [Bindings::CORE_SCOPE]),
                            sexpr.get_span(),
                        ),
                    ),
                    expand_quasiquote_args(cdr, bindings, depth + 1)?,
                ))
            } else if let SExpr::Id(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && (binding.symbol.0 == "unquote" || binding.symbol.0 == "unquote-splicing")
            {
                if depth > 0 {
                    Ok(make_sexpr!(
                        SExpr::Id(Id::new("cons", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                            car.clone(),
                        ),
                        expand_quasiquote_args(cdr, bindings, depth - 1)?,
                    ))
                } else if binding.symbol.0 == "unquote" && matches!(try_rest(cdr), Some(SExpr::Nil(..))) {
                    Ok(first(cdr))
                } else {
                    Err(CompilationError {
                        span: car.get_span().combine(cdr.get_span()),
                        reason: format!("Invalid '{}' form", id),
                    })
                }
            } else {
                Ok(make_sexpr!(
                    SExpr::Id(Id::new("append", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                    expand_quasiquote_args_list(car, bindings, depth)?,
                    expand_quasiquote_args(cdr, bindings, depth)?,
                ))
            }
        },

        SExpr::Vector(vector, span) => {
            Ok(make_sexpr!(
                SExpr::Id(Id::new("list->vector", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                expand_quasiquote_args(&vector.clone().into_cons_list(*span), bindings, depth)?,
            ))
        },

        _ => {
            Ok(make_sexpr!(
                SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                sexpr.clone(),
            ))
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        compile::{
            lex::tokenize,
            parse::parse,
            sexpr::{Bool, Id, Num},
            span::Span,
            util::{first, last, nth},
        },
        make_sexpr,
    };

    fn expand_source(
        source: &str,
        bindings: &mut Bindings,
        env: &mut HashMap<Symbol, Arc<Transformer>>,
    ) -> Result<SExpr> {
        let expr = parse(&tokenize(source).unwrap()).unwrap();
        expand(&introduce(&expr), bindings, env)
    }

    use super::*;

    #[test]
    fn test_introduce() {
        let list = parse(&tokenize("(cons 0 1)").unwrap()).unwrap();
        let span = Span { lo: 0, hi: 0 };
        assert_eq!(
            introduce(&list).without_spans(),
            make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let lambda_expr = parse(&tokenize("(lambda (x y) (cons x y))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let expected = make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let lambda_expr = parse(&tokenize("(lambda (x y . z) (cons x z))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let lambda_expr = parse(&tokenize("(lambda x (cons x x))").unwrap()).unwrap();
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
    fn test_expand_atoms() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
            make_sexpr!(SExpr::Bool(Bool(false), span)).without_spans()
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
            Arc::new(transformer),
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
            Arc::new(transformer),
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
            Arc::new(transformer),
        )]);

        let sexpr = parse(&tokenize("(and list list)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
            Arc::new(transformer),
        )]);

        let sexpr = parse(&tokenize("(and #t #t #t #t)").unwrap()).unwrap();
        // (and t t t t)
        // (if t (and t t t) f)
        // (if t (if t (and t t) f) f)
        // (if t (if t (if t (and t) f) f) f)
        // (if t (if t (if t t f) f) f) f)
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
            Arc::new(transformer),
        )]);

        let sexpr = parse(&tokenize("(my-macro x)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
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
            Arc::new(transformer),
        )]);

        let sexpr = parse(&tokenize("((lambda (temp) (my-or #f temp)) #t)").unwrap()).unwrap();
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };

        let expected = make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let expected = make_sexpr!(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let let_syntax_expr = &parse(
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let let_syntax_expr = &parse(
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
        let result = expand(&introduce(let_syntax_expr), &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Id(Id::new("begin", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Id(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
                (
                    SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                    SExpr::Nil(span),
                    SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2, 3, 4]), span),
                ),
            ),
            (
                SExpr::Id(Id::new("define", [Bindings::CORE_SCOPE, 1, 2]), span),
                SExpr::Id(Id::new("y", [Bindings::CORE_SCOPE, 1, 2]), span),
                (
                    SExpr::Id(Id::new("lambda", [Bindings::CORE_SCOPE, 1, 2]), span),
                    SExpr::Nil(span),
                    SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2, 5, 6]), span),
                ),
            ),
            SExpr::Id(Id::new("x", [Bindings::CORE_SCOPE, 1, 2]), span),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_let_syntax_multiple_body_exprs_() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let expected = make_sexpr!(
            SExpr::Id(Id::new("begin", [Bindings::CORE_SCOPE]), span),
            SExpr::Num(Num(1.0), span),
            SExpr::Num(Num(2.0), span),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_letrec_syntax_cleans_env_after_success() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
    fn test_let_syntax_cleans_env_after_success() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
    fn test_expand_failed_expansion_does_not_affect_bindings_or_env() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();

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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();

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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let result = expand_source("`42", &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), span),
            SExpr::Num(Num(42.0), span),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_quasiquote_empty_list() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let result = expand_source("`()", &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), span),
            SExpr::Nil(span),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_quasiquote_constant_list() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let result = expand_source("`(1 2)", &mut bindings, &mut env).unwrap();
        let span = Span { lo: 0, hi: 0 };
        // `(1 2) => (append (quote (1)) (append (quote (2)) (quote ())))
        // Note: (quote (1)) wraps the element in a list for append
        let expected = make_sexpr!(
            SExpr::Id(Id::new("append", [Bindings::CORE_SCOPE]), span),
            (
                SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), span),
                (SExpr::Num(Num(1.0), span)),
            ),
            (
                SExpr::Id(Id::new("append", [Bindings::CORE_SCOPE]), span),
                (
                    SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), span),
                    (SExpr::Num(Num(2.0), span)),
                ),
                (
                    SExpr::Id(Id::new("quote", [Bindings::CORE_SCOPE]), span),
                    SExpr::Nil(span),
                ),
            ),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_expand_quasiquote_with_unquote() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
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
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let result = expand_source(",x", &mut bindings, &mut env);
        assert!(result.is_err());
    }

    #[test]
    fn test_expand_unquote_splicing_outside_quasiquote_errors() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Arc<Transformer>>::new();
        let result = expand_source(",@x", &mut bindings, &mut env);
        assert!(result.is_err());
    }
}
