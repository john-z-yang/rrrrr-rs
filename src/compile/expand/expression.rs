use std::collections::HashSet;
use std::fmt;

use super::body::expand_body;
use super::dispatch::expand_sexpr;
use super::transformer::Transformer;
use super::{Context, Env, SyntaxContext};
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::{Id, SExpr, Symbol},
    util::{first, is_proper_list, len, rest, try_dotted_tail, try_first, try_for_each, try_map},
};
use crate::{if_let_sexpr, make_sexpr, match_sexpr, template_sexpr};

pub(super) fn expand_fn_application(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if let Some(tail) = try_dotted_tail(&sexpr)
        && !matches!(tail, SExpr::Nil(_))
    {
        return Err(CompilationError {
            span: tail.get_span(),
            reason: "Invalid application form: not a proper list".to_owned(),
        });
    }
    try_map(sexpr, |sub_sexpr| {
        expand_sexpr(
            sub_sexpr,
            bindings,
            env,
            ctx.with_syntax_ctx(SyntaxContext::Expression),
        )
    })
}

pub(super) fn expand_if(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    let expr_ctx = ctx.with_syntax_ctx(SyntaxContext::Expression);
    match_sexpr! {
        &sexpr;

        (iif, test, consequent, alternate) => {
            Ok(template_sexpr!((
                iif.clone(),
                expand_sexpr(test.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(consequent.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(alternate.clone(), bindings, env, expr_ctx)?,
            ) => &sexpr).unwrap())
        },

        (iif, test, consequent) => {
            Ok(make_sexpr!(
                iif.clone(),
                expand_sexpr(test.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(consequent.clone(), bindings, env, expr_ctx)?,
                SExpr::Void(sexpr.get_span()),
            ))
        },

        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'if' form: expected (if <test> <consequent> <alternate>) or (if <test> <consequent>)".to_owned(),
            })
        },
    }
}

pub(super) fn expand_begin(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if !is_proper_list(&sexpr) {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid 'begin' form: expected a proper list".to_owned(),
        });
    }
    if len(&sexpr) == 1 {
        return Err(CompilationError {
            span: sexpr.get_span(),
            reason: "Invalid 'begin' form: expected at least one expression".to_owned(),
        });
    }
    let mut res = SExpr::cons(
        first(&sexpr),
        try_map(rest(&sexpr), |sub_sexpr| {
            expand_sexpr(sub_sexpr, bindings, env, ctx)
        })?,
    );
    res.update_span(sexpr.get_span());
    Ok(res)
}

pub(super) fn expand_lambda(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    match_sexpr! {
        &sexpr;

        (lambda, args @ (..), body @ ..) => {
            let scope_id = bindings.new_scope_id();
            let args = args.clone().add_scope(scope_id);
            let body = body.clone().add_scope(scope_id);

            let mut arg_symbols = HashSet::new();

            try_for_each(&args, |arg| {
                let SExpr::Var(id, _) = arg else {
                    return Err(CompilationError {
                        span: arg.get_span(),
                        reason: format!(
                            "Expected an identifier in function parameters, but got: {}",
                            arg
                        ),
                    });
                };
                if !arg_symbols.insert(id.symbol.clone()) {
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
                Some(SExpr::Var(id, _)) => {
                    if !arg_symbols.insert(id.symbol.clone()) {
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

            let body = expand_body(body, bindings, env, ctx)?;
            Ok(template_sexpr!((lambda.clone(), args, ..body) => &sexpr).unwrap())
        },

        (lambda, arg @ SExpr::Var(..), body @ ..) => {
            let scope_id = bindings.new_scope_id();
            let arg = arg.clone().add_scope(scope_id);
            let body = body.clone().add_scope(scope_id);

            let SExpr::Var(id, _) = &arg else {
                unreachable!("arg is already a SExpr::Var(..)")
            };
            let binding = bindings.gen_sym(id);
            bindings.add_binding(id, &binding);

            let body = expand_body(body, bindings, env, ctx)?;
            Ok(template_sexpr!((lambda.clone(), arg, ..body) => &sexpr).unwrap())
        },

        _ => {
            Err(CompilationError {
                span: sexpr.get_span(),
                reason: "Invalid 'lambda' form".to_owned(),
            })
        }
    }
}

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
