use super::definition::{expand_define, expand_define_syntax, expand_set};
use super::expression::{
    expand_begin, expand_fn_application, expand_if, expand_lambda, expand_let_syntax,
    expand_letrec, expand_letrec_syntax,
};
use super::quote::{expand_quasiquote, expand_quote};
use super::transformer::Transformer;
use super::{Context, Env, MAX_MACRO_DEPTH};
use crate::compile::bindings::Id;
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::SExpr,
    util::first,
};
use crate::if_let_sexpr;

pub(super) fn expand_sexpr(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if let SExpr::Nil(span) = &sexpr {
        return Err(CompilationError {
            span: *span,
            reason: "Unexpected empty list".to_owned(),
        });
    };
    if let SExpr::Var(..) = sexpr {
        return expand_id(sexpr, bindings);
    }
    if_let_sexpr! {(SExpr::Var(..), ..) = &sexpr =>
        return expand_id_application(sexpr, bindings, env, ctx);
    };
    if_let_sexpr! {(..) = sexpr =>
        return expand_fn_application(sexpr, bindings, env, ctx);
    };
    Ok(sexpr)
}

fn expand_id(sexpr: SExpr<Id>, bindings: &mut Bindings) -> Result<SExpr<Id>> {
    let SExpr::Var(id, span) = &sexpr else {
        unreachable!("expand_id expected an ID");
    };
    match bindings.resolve_sym(id) {
        Some(symbol) if Bindings::CORE_FORMS.contains(&symbol.0.as_str()) => {
            Err(CompilationError {
                span: *span,
                reason: format!("Invalid '{}' form: not in parentheses", symbol),
            })
        }
        _ => Ok(sexpr),
    }
}

pub(super) fn apply_transformer(
    sexpr: SExpr<Id>,
    transformer: &Transformer,
    name: &Id,
    bindings: &mut Bindings,
    ctx: &mut Context,
) -> Result<SExpr<Id>> {
    ctx.increment_depth();
    if ctx.depth >= MAX_MACRO_DEPTH {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: format!(
                "Macro expansion depth limit exceeded ({MAX_MACRO_DEPTH}) while expanding '{name}'"
            ),
        });
    }
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
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    mut ctx: Context,
) -> Result<SExpr<Id>> {
    let (id, binding) = match first(&sexpr) {
        SExpr::Var(id, _) => match bindings.resolve_sym(id) {
            Some(binding) => (id.clone(), binding),
            None => {
                return expand_fn_application(sexpr, bindings, env, ctx);
            }
        },
        _ => unreachable!("expand_id_application expected first element to be an ID"),
    };

    match binding.0.as_str() {
        "quote" => expand_quote(sexpr),
        "quasiquote" => expand_quasiquote(sexpr, bindings, env, ctx),
        "unquote" | "unquote-splicing" => Err(CompilationError {
            span: sexpr.get_span(),
            reason: format!("Invalid '{}' form: not in 'quasiquote'", binding),
        }),
        "let-syntax" => expand_let_syntax(sexpr, bindings, env, ctx),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env, ctx),
        "lambda" => expand_lambda(sexpr, bindings, env, ctx),
        "define" => expand_define(sexpr, bindings, env, ctx),
        "define-syntax" => expand_define_syntax(sexpr, bindings, env, ctx),
        "letrec" => expand_letrec(sexpr, bindings, env, ctx),
        "set!" => expand_set(sexpr, bindings, env, ctx),
        "begin" => expand_begin(sexpr, bindings, env, ctx),
        "if" => expand_if(sexpr, bindings, env, ctx),
        _ => {
            if let Some(transformer) = env.get(&binding) {
                expand_sexpr(
                    apply_transformer(sexpr, transformer, &id, bindings, &mut ctx)?,
                    bindings,
                    env,
                    ctx,
                )
            } else {
                expand_fn_application(sexpr, bindings, env, ctx)
            }
        }
    }
}
