use super::{Context, Env, expand_sexpr};
use crate::compile::{
    bindings::{Bindings, Id},
    compilation_error::{CompilationError, Result},
    sexpr::SExpr,
    util::{first, try_rest},
};
use crate::{if_let_sexpr, make_sexpr, match_sexpr};

pub(super) fn expand_quote(sexpr: SExpr<Id>) -> Result<SExpr<Id>> {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, _) = &sexpr => {
        return Ok(sexpr);
    }}
    Err(CompilationError {
        span,
        reason: "Invalid 'quote' form: expected a single argument".to_owned(),
    })
}

pub(super) fn expand_quasiquote(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, arg) = sexpr => {
        return expand_sexpr(expand_quasiquote_args(arg, bindings, 0)?, bindings, env, ctx);
    }};
    Err(CompilationError {
        span,
        reason: "Invalid 'quasiquote' form: expected a single argument".to_owned(),
    })
}

fn expand_quasiquote_args_list(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    depth: u32,
) -> Result<SExpr<Id>> {
    match_sexpr! {
        &sexpr;

        (car, cdr @ ..) => {
            if let SExpr::Var(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && binding.symbol.0 == "quasiquote"
            {
                Ok(make_sexpr!(
                    SExpr::Var(Id::new("list", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Var(Id::new("cons", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                            SExpr::Var(Id::new("quasiquote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                        ),
                        expand_quasiquote_args(cdr.clone(), bindings, depth + 1)?,
                    ),
                ))
            } else if let SExpr::Var(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && (binding.symbol.0 == "unquote" || binding.symbol.0 == "unquote-splicing")
            {
                if depth > 0 {
                    Ok(make_sexpr!(
                        SExpr::Var(Id::new("list", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Var(Id::new("cons", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                            (
                                SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                                car.clone(),
                            ),
                            expand_quasiquote_args(cdr.clone(), bindings, depth - 1)?,
                        ),
                    ))
                } else if binding.symbol.0 == "unquote" {
                    Ok(make_sexpr!(
                        SExpr::Var(Id::new("list", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        ..cdr.clone(),
                    ))
                } else {
                    Ok(make_sexpr!(
                        SExpr::Var(Id::new("append", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        ..cdr.clone(),
                    ))
                }
            } else {
                Ok(make_sexpr!(
                    SExpr::Var(Id::new("list", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Var(Id::new("append", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        expand_quasiquote_args_list(car.clone(), bindings, depth)?,
                        expand_quasiquote_args(cdr.clone(), bindings, depth)?,
                    ),
                ))
            }
        },

        SExpr::Vector(vector, span) => {
            Ok(make_sexpr!(
                SExpr::Var(Id::new("list", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                (
                    SExpr::Var(Id::new("list->vector", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                    expand_quasiquote_args(vector.clone().into_cons_list(*span), bindings, depth)?,
                ),
            ))
        },

        _ => {
            Ok(make_sexpr!(
                SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                (sexpr),
            ))
        },
    }
}

fn expand_quasiquote_args(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    depth: u32,
) -> Result<SExpr<Id>> {
    match_sexpr! {
        &sexpr;

        (car, cdr @ ..) => {
            if let SExpr::Var(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && binding.symbol.0 == "quasiquote"
            {
                Ok(make_sexpr!(
                    SExpr::Var(Id::new("cons", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                    (
                        SExpr::Var(
                            Id::new("quote", [Bindings::CORE_SCOPE]),
                            sexpr.get_span(),
                        ),
                        SExpr::Var(
                            Id::new("quasiquote", [Bindings::CORE_SCOPE]),
                            sexpr.get_span(),
                        ),
                    ),
                    expand_quasiquote_args(cdr.clone(), bindings, depth + 1)?,
                ))
            } else if let SExpr::Var(id, _) = car
                && let Some(binding) = bindings.resolve(id)
                && (binding.symbol.0 == "unquote" || binding.symbol.0 == "unquote-splicing")
            {
                if depth > 0 {
                    Ok(make_sexpr!(
                        SExpr::Var(Id::new("cons", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                        (
                            SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                            car.clone(),
                        ),
                        expand_quasiquote_args(cdr.clone(), bindings, depth - 1)?,
                    ))
                } else if binding.symbol.0 == "unquote" && matches!(try_rest(cdr), Some(SExpr::Nil(..))) {
                    Ok(first(cdr).clone())
                } else {
                    Err(CompilationError {
                        span: car.get_span().combine(cdr.get_span()),
                        reason: format!("Invalid '{}' form", id),
                    })
                }
            } else {
                Ok(make_sexpr!(
                    SExpr::Var(Id::new("append", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                    expand_quasiquote_args_list(car.clone(), bindings, depth)?,
                    expand_quasiquote_args(cdr.clone(), bindings, depth)?,
                ))
            }
        },

        SExpr::Vector(vector, span) => {
            Ok(make_sexpr!(
                SExpr::Var(Id::new("list->vector", [Bindings::Q_QUOTE_SCOPE]), sexpr.get_span()),
                expand_quasiquote_args(vector.clone().into_cons_list(*span), bindings, depth)?,
            ))
        },

        _ => {
            Ok(make_sexpr!(
                SExpr::Var(Id::new("quote", [Bindings::CORE_SCOPE]), sexpr.get_span()),
                sexpr,
            ))
        },
    }
}
