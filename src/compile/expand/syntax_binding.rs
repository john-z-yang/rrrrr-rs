use std::fmt;

use super::body::expand_body;
use super::{Context, Env, transformer::Transformer};
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::{Id, SExpr, Symbol},
    util::{first, is_proper_list, len, try_first, try_for_each},
};
use crate::if_let_sexpr;

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

pub(super) fn expand_let_syntax(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    expand_syntax_binding(
        sexpr,
        bindings,
        &mut env.clone(),
        SyntaxBindingForm::LetSyntax,
        ctx,
    )
}

pub(super) fn expand_letrec_syntax(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    expand_syntax_binding(
        sexpr,
        bindings,
        &mut env.clone(),
        SyntaxBindingForm::LetrecSyntax,
        ctx,
    )
}

fn expand_syntax_binding(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    form: SyntaxBindingForm,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if_let_sexpr! {(_, binding_pairs @ (..), body @ ..) = &sexpr =>
        if !is_proper_list(binding_pairs) {
            return Err(CompilationError {
                span: sexpr.get_span(),
                reason: format!("Invalid '{form}' bindings: expected a proper list"),
            });
        }
        let scope_id = bindings.new_scope_id();

        try_for_each(binding_pairs, |binding_pair| {
            if_let_sexpr! {(SExpr::Var(id, _), transformer_spec) = binding_pair =>
                let id = id.add_scope(scope_id);
                let transformer_spec = match form {
                    SyntaxBindingForm::LetSyntax => transformer_spec,
                    SyntaxBindingForm::LetrecSyntax => &transformer_spec.clone().add_scope(scope_id)
                };

                let binding = bindings.gen_sym(&id);
                bindings.add_binding(&id, &binding);

                if !matches!(
                    try_first(transformer_spec),
                    Some(SExpr::Var(id, _)) if bindings.resolve_sym(&id) == Some(Symbol::new("syntax-rules"))
                ) {
                    return Err(CompilationError {
                        span: transformer_spec.get_span(),
                        reason: "Expected a 'syntax-rules' transformer".to_owned(),
                    });
                }
                let transformer = Transformer::new(transformer_spec)?;
                env.insert(binding, transformer);
                return Ok(());
            }
            Err(CompilationError {
                span: binding_pair.get_span(),
                reason: format!("Invalid '{form}' binding pair: expected (identifier 'syntax-rules' transformer)"),
            })
        })?;

        let body = expand_body(body.clone().add_scope(scope_id), bindings, env, ctx).map(|body| {
            if len(&body) == 1 {
                first(&body)
            } else {
                SExpr::cons(
                    SExpr::Var(Id::new("begin", [Bindings::CORE_SCOPE]), body.get_span()),
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
