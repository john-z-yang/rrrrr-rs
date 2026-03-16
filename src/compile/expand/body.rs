use super::{Context, Env, SyntaxContext, apply_transformer, expand_sexpr};
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::{Cons, Id, SExpr},
    util::{append, is_proper_list, len, rest, try_first, try_map},
};
use crate::{if_let_sexpr, match_sexpr};

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
    let (body, phase) = normalize_body(body, bindings, env, NormalizationPhase::Define, &mut ctx)?;
    if phase == NormalizationPhase::Define {
        return Err(CompilationError {
            span: body.get_span(),
            reason: "Invalid body: expected at least one expression after definitions".to_owned(),
        });
    }

    try_map(body, |sexpr| {
        expand_sexpr(
            sexpr,
            bindings,
            env,
            ctx.with_syntax_ctx(SyntaxContext::Body),
        )
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

fn normalize_body(
    body: SExpr<Id>,
    bindings: &mut Bindings,
    env: &Env,
    phase: NormalizationPhase,
    ctx: &mut Context,
) -> Result<(SExpr<Id>, NormalizationPhase)> {
    let SExpr::Cons(cons, span) = body else {
        return Ok((body, phase));
    };

    let (expanded_car, kind) = partial_expand_body(*cons.car, bindings, env, ctx)?;

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
                normalize_body(*cons.cdr, bindings, env, NormalizationPhase::Define, ctx)?;
            Ok((SExpr::Cons(Cons::new(expanded_car, cdr), span), next_phase))
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
            let (head, next_phase) =
                normalize_body(rest(&expanded_car), bindings, env, phase, ctx)?;
            let (remaining, next_phase) =
                normalize_body(*cons.cdr, bindings, env, next_phase, ctx)?;
            Ok((append(head, remaining), next_phase))
        }
        BodyFormKind::Other => {
            let (cdr, _) = normalize_body(*cons.cdr, bindings, env, NormalizationPhase::Body, ctx)?;
            Ok((
                SExpr::Cons(Cons::new(expanded_car, cdr), span),
                NormalizationPhase::Body,
            ))
        }
    }
}

fn partial_expand_body(
    form: SExpr<Id>,
    bindings: &mut Bindings,
    env: &Env,
    ctx: &mut Context,
) -> Result<(SExpr<Id>, BodyFormKind)> {
    let mut form: SExpr<Id> = form;
    loop {
        let Some(SExpr::Var(id, _)) = try_first(&form) else {
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
                form = apply_transformer(form, transformer, &id, bindings, ctx)?;
            }
        }
    }
}

fn collect_define(sexpr: &SExpr<Id>, bindings: &mut Bindings) -> Result<()> {
    let (id, span) = match_sexpr! {
        &sexpr;

        (_, SExpr::Var(id, span), _) => {
            (id, span)
        },

        (_, (SExpr::Var(id, span), _args @ ..), _) => {
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
