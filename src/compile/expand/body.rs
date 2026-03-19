use super::{Context, Env, SyntaxContext, apply_transformer, expand_sexpr};
use crate::{
    compile::{
        bindings::Bindings,
        compilation_error::{CompilationError, Result},
        sexpr::{Cons, Id, SExpr},
        span::Span,
        util::{append, is_proper_list, len, rest, split, try_first, try_map},
    },
    template_sexpr,
};
use crate::{if_let_sexpr, make_sexpr, match_sexpr};

pub(super) fn expand_body(
    body: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    mut ctx: Context,
) -> Result<SExpr<Id>> {
    if len(&body) == 0 {
        return Err(CompilationError {
            span: body.get_span(),
            reason: "Invalid body: expected at least one body expression".to_owned(),
        });
    }
    if !is_proper_list(&body) {
        return Err(CompilationError {
            span: body.get_span(),
            reason: "Invalid body: expected it to be a proper list".to_owned(),
        });
    }

    let body = body.add_scope(bindings.new_scope_id());
    let body = normalize_body(body, bindings, env, &mut ctx)?;

    let (initializers, expressions) = extract_initializers(body, bindings)?;

    if len(&initializers) == 0 {
        try_map(expressions, |sexpr| {
            expand_sexpr(
                sexpr,
                bindings,
                env,
                ctx.with_syntax_ctx(SyntaxContext::Body),
            )
        })
    } else {
        transform_to_letrec(initializers, expressions, bindings, env, ctx)
    }
}

enum BodySExprKind {
    Define,
    Begin,
    Other,
}

fn normalize_body(
    body: SExpr<Id>,
    bindings: &mut Bindings,
    env: &Env,
    ctx: &mut Context,
) -> Result<SExpr<Id>> {
    let SExpr::Cons(cons, span) = body else {
        return Ok(body);
    };

    let (expanded_car, kind) = partial_expand_sexpr(*cons.car, bindings, env, ctx)?;

    match kind {
        BodySExprKind::Define => {
            register_define(&expanded_car, bindings)?;
            let rest = normalize_body(*cons.cdr, bindings, env, ctx)?;
            Ok(SExpr::Cons(Cons::new(expanded_car, rest), span))
        }
        BodySExprKind::Begin => {
            if !is_proper_list(&expanded_car) {
                return Err(CompilationError {
                    span: expanded_car.get_span(),
                    reason: "Invalid 'begin' form: expected a proper list".to_owned(),
                });
            }
            if len(&expanded_car) == 1 {
                return Err(CompilationError {
                    span: expanded_car.get_span(),
                    reason: "Invalid 'begin' form: expected at least one non-define expression"
                        .to_owned(),
                });
            }
            let spliced = normalize_body(rest(&expanded_car), bindings, env, ctx)?;
            let remaining = normalize_body(*cons.cdr, bindings, env, ctx)?;
            Ok(append(spliced, remaining))
        }
        BodySExprKind::Other => {
            let rest = normalize_body(*cons.cdr, bindings, env, ctx)?;
            Ok(SExpr::Cons(Cons::new(expanded_car, rest), span))
        }
    }
}

fn register_define(define: &SExpr<Id>, bindings: &mut Bindings) -> Result<()> {
    let ((id, span), init) = extract_define(define)?;
    let resolved = bindings.resolve_scopes(&id);
    if let Some(resolved) = resolved
        && resolved == id.scopes
    {
        return Err(CompilationError {
            span: span.combine(init.get_span()),
            reason: format!(
                "Duplicate definition: '{}' is already bound in this scope",
                id
            ),
        });
    }

    let binding = bindings.gen_sym(&id);
    bindings.add_binding(&id, &binding);
    Ok(())
}

fn partial_expand_sexpr(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &Env,
    ctx: &mut Context,
) -> Result<(SExpr<Id>, BodySExprKind)> {
    let mut form: SExpr<Id> = sexpr;
    loop {
        let Some(SExpr::Var(id, _)) = try_first(&form) else {
            return Ok((form, BodySExprKind::Other));
        };
        let Some(binding) = bindings.resolve_sym(&id) else {
            return Ok((form, BodySExprKind::Other));
        };
        match binding.0.as_str() {
            "define" => return Ok((form, BodySExprKind::Define)),
            "begin" => return Ok((form, BodySExprKind::Begin)),
            _ => {
                let Some(transformer) = env.get(&binding) else {
                    return Ok((form, BodySExprKind::Other));
                };
                form = apply_transformer(form, transformer, &id, bindings, ctx)?;
            }
        }
    }
}

fn extract_initializers(
    body: SExpr<Id>,
    bindings: &mut Bindings,
) -> Result<(SExpr<Id>, SExpr<Id>)> {
    let mut num_defines = 0;
    let mut num_expressions = 0;

    let body = try_map(body, |sexpr| {
        if let Some(SExpr::Var(id, _)) = try_first(&sexpr)
            && bindings.resolve_sym(&id).is_some_and(|s| s.0 == "define")
        {
            if num_expressions > 0 {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: "'define' must appear at the beginning of a body".to_owned(),
                });
            }

            let ((id, span), init) = extract_define(&sexpr)?;

            num_defines += 1;

            Ok(make_sexpr!(SExpr::Var(id, span), init))
        } else {
            num_expressions += 1;
            Ok(sexpr)
        }
    })?;

    if num_expressions == 0 {
        return Err(CompilationError {
            span: body.get_span(),
            reason: "Invalid body: expected at least one expression after definitions".to_owned(),
        });
    }

    Ok(split(body, num_defines))
}

fn extract_define(define: &SExpr<Id>) -> Result<((Id, Span), SExpr<Id>)> {
    match_sexpr! {
        define;

        (_, SExpr::Var(id, span), expr) => {
            Ok(((id.clone(), *span), expr.clone()))
        },

        (_, (SExpr::Var(func_id, span), args @ ..), body @ ..) => {
            Ok((
                (func_id.clone(), *span),
                make_sexpr!(
                    SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), define.get_span()),
                    args.clone(),
                    ..body.clone(),
                ),
            ))
        },

        _ => {
            Err(CompilationError {
                span: define.get_span(),
                reason: "Invalid 'define' form".to_owned(),
            })
        },
    }
}

fn transform_to_letrec(
    initializers: SExpr<Id>,
    expressions: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    let initializer_list = try_map(initializers, |initializer| {
        if_let_sexpr! {(var @ SExpr::Var(..), init) = initializer.clone() => {
            return Ok(template_sexpr!((
                var,
                expand_sexpr(
                    init,
                    bindings,
                    env,
                    ctx.with_syntax_ctx(SyntaxContext::Expression),
                )?,
            ) => initializer).unwrap());
        }};
        unreachable!("bad initializer")
    })?;

    let body = try_map(expressions, |sexpr| {
        expand_sexpr(
            sexpr,
            bindings,
            env,
            ctx.with_syntax_ctx(SyntaxContext::Body),
        )
    })?;

    Ok(make_sexpr!((
        SExpr::Var(
            Id::new("letrec", [Bindings::CORE_SCOPE]),
            initializer_list.get_span(),
        ),
        initializer_list,
        ..body,
    )))
}
