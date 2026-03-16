use std::collections::HashSet;

use crate::{
    compile::{
        bindings::Bindings,
        compilation_error::{CompilationError, Result},
        expand::{Context, Env, SyntaxContext, expand_body, expand_sexpr},
        sexpr::{Id, SExpr},
        util::{is_proper_list, try_for_each, try_map},
    },
    if_let_sexpr, template_sexpr,
};

pub(super) fn expand_letrec(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if_let_sexpr! {(letrec, initializers @ (..), body @ ..) = &sexpr => {
        if !is_proper_list(initializers) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'letrec' form: expected initializers to be a proper list".to_owned(),
            });
        }

        let scope_id = bindings.new_scope_id();
        let initializers = initializers.clone().add_scope(scope_id);
        let body = body.clone().add_scope(scope_id);

        let mut initializer_symbols = HashSet::new();

        try_for_each(&initializers, |initializer| {
            if_let_sexpr! {(SExpr::Var(id, span), _) = initializer => {
                if !initializer_symbols.insert(id.symbol.clone()) {
                    return Err(CompilationError {
                        span: *span,
                        reason: format!("Duplicate id: '{}'", id),
                    });
                }
                let binding = bindings.gen_sym(id);
                bindings.add_binding(id, &binding);
                return Ok(());
            }};
            Err(CompilationError {
                span: initializer.get_span(),
                reason: "Invalid 'letrec' form: expected initializer to be in the form of (var expr)".to_owned(),
            })
        })?;

        let initializers = try_map(initializers, |initializer| {
            if_let_sexpr! {(var, sexpr) = initializer.clone() => {
                return Ok(template_sexpr!((
                    var,
                    expand_sexpr(
                        sexpr,
                        bindings,
                        env,
                        ctx.with_syntax_ctx(SyntaxContext::Expression),
                    )?,
                ) => initializer).unwrap());
            }};
            unreachable!("Initializer shape is already validated")
        })?;

        return Ok(template_sexpr!((
            letrec.clone(),
            initializers,
            ..expand_body(body, bindings, env, ctx)?
        ) => &sexpr).unwrap());
    }};

    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'letrec' form".to_owned(),
    })
}
