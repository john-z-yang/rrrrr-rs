use super::dispatch::expand_sexpr;
use super::transformer::Transformer;
use super::{Context, Env, SyntaxContext};
use crate::compile::bindings::Id;
use crate::compile::ident::Symbol;
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::SExpr,
    util::try_first,
};
use crate::{if_let_sexpr, make_sexpr, match_sexpr, template_sexpr};

pub(super) fn expand_set(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if_let_sexpr! {(set, var @ SExpr::Var(id, _), exp) = &sexpr =>
        if let Some(resolved) = bindings.resolve_sym(id)
            && Bindings::CORE_FORMS.contains(&resolved.0.as_str()) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Cannot mutate core forms/primatives '{}'", id),
            })
        }
        let exp = expand_sexpr(exp.clone(), bindings, env, ctx.with_syntax_ctx(SyntaxContext::Expression))?;
        return Ok(template_sexpr!((set.clone(), var.clone(), exp) => &sexpr).unwrap());
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'set!' form".to_owned(),
    })
}

pub(super) fn expand_define(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    match_sexpr! {
        &sexpr;

        (define, var @ SExpr::Var(..), exp) => {
            if matches!(ctx.syntax_ctx, SyntaxContext::Expression) {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: "'define' is not allowed in an expression context".to_owned(),
                });
            }
            assert_eq!(ctx.syntax_ctx, SyntaxContext::TopLevel);
            let exp = expand_sexpr(exp.clone(), bindings, env, ctx.with_syntax_ctx(SyntaxContext::Expression))?;
            Ok(template_sexpr!((define.clone(), var.clone(), exp) => &sexpr).unwrap())
        },

        (define, (func_name @ SExpr::Var(..), args @ ..), body @ ..) => {
            expand_sexpr(
                make_sexpr!(
                    define.clone(),
                    func_name.clone(),
                    (
                        SExpr::Var(Id::new("lambda", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        args.clone(),
                        ..body.clone(),
                    )
                ),
                bindings,
                env,
                ctx,
            )
        },

        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'define' form".to_owned(),
            })
        },
    }
}

pub(super) fn expand_define_syntax(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if_let_sexpr! {(_, SExpr::Var(id, _), transformer_spec) = &sexpr =>
        if ctx.syntax_ctx != SyntaxContext::TopLevel {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: "'define-syntax' is only allowed in the top level context".to_owned(),
            });
        }
        if !matches!(
            try_first(transformer_spec),
            Some(SExpr::Var(id, _)) if bindings.resolve_sym(id) == Some(Symbol::new("syntax-rules"))
        ) {
            return Err(CompilationError {
                span: transformer_spec.get_span(),
                reason: "Expected a 'syntax-rules' transformer".to_owned(),
            });
        }

        let transformer = Transformer::new(transformer_spec)?;
        let binding = bindings.gen_sym(id);
        bindings.add_binding(id, &binding);
        env.insert(binding, transformer);

        return Ok(SExpr::Void(sexpr.get_span()));
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'define-syntax' form".to_owned(),
    })
}
