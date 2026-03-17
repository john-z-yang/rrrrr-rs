mod body;
mod letrec;
mod quasiquote;
mod syntax_binding;
mod transformer;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    mem,
    rc::Rc,
};

use self::transformer::Transformer;
use super::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::{Id, SExpr, Symbol},
    util::{try_first, try_map},
};
use crate::{
    compile::{
        expand::{
            body::expand_body,
            letrec::expand_letrec,
            quasiquote::expand_quasiquote,
            syntax_binding::{expand_let_syntax, expand_letrec_syntax},
        },
        util::{first, is_proper_list, len, rest, try_dotted_tail, try_for_each},
    },
    if_let_sexpr, make_sexpr, match_sexpr, template_sexpr,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Env {
    transformers: HashMap<Symbol, Rc<Transformer>>,
}

impl Env {
    fn get(&self, symbol: &Symbol) -> Option<&Transformer> {
        self.transformers.get(symbol).map(Rc::as_ref)
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.transformers.is_empty()
    }

    fn insert(&mut self, symbol: Symbol, transformer: Transformer) {
        self.transformers.insert(symbol, Rc::new(transformer));
    }
}

impl<const N: usize> From<[(Symbol, Transformer); N]> for Env {
    fn from(transformers: [(Symbol, Transformer); N]) -> Self {
        Self {
            transformers: transformers
                .into_iter()
                .map(|(symbol, transformer)| (symbol, Rc::new(transformer)))
                .collect(),
        }
    }
}

const MAX_MACRO_DEPTH: u16 = 1024;

pub fn introduce(sexpr: SExpr<Symbol>) -> SExpr<Id> {
    sexpr.map_var(&|symbol, _| Id {
        symbol,
        scopes: std::collections::BTreeSet::from([Bindings::CORE_SCOPE]),
    })
}

pub(crate) fn expand(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
) -> Result<SExpr<Id>> {
    let mut bindings_snapshot = bindings.clone();
    let mut env_snapshot = env.clone();
    let result = expand_sexpr(sexpr, bindings, env, Context::new(SyntaxContext::TopLevel));
    if result.is_err() {
        mem::swap(&mut bindings_snapshot, bindings);
        mem::swap(&mut env_snapshot, env);
    }
    result
}

#[derive(PartialEq, Clone, Copy, Eq, Hash, Debug)]
struct Context {
    syntax_ctx: SyntaxContext,
    depth: u16,
}

impl Context {
    fn new(syntax_ctx: SyntaxContext) -> Self {
        Self {
            syntax_ctx,
            depth: 0,
        }
    }

    fn with_syntax_ctx(self, syntax_ctx: SyntaxContext) -> Self {
        Self { syntax_ctx, ..self }
    }

    fn increment_depth(&mut self) {
        self.depth += 1;
    }
}

#[derive(PartialEq, Clone, Copy, Eq, Hash, Debug)]
enum SyntaxContext {
    TopLevel,
    Expression,
    Body,
}

fn expand_sexpr(
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

fn apply_transformer(
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
    let (id, binding) = match try_first(&sexpr) {
        Some(SExpr::Var(id, _)) => match bindings.resolve_sym(&id) {
            Some(binding) => (id, binding),
            None => {
                return expand_fn_application(sexpr, bindings, env, ctx);
            }
        },
        _ => unreachable!("expand_id_application expected first element to be an ID"),
    };

    match binding.0.as_str() {
        "quote" => Ok(sexpr),
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

fn expand_fn_application(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    try_map(sexpr, |sub_sexpr| {
        expand_sexpr(
            sub_sexpr,
            bindings,
            env,
            ctx.with_syntax_ctx(SyntaxContext::Expression),
        )
    })
}

fn expand_if(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    let expr_ctx = ctx.with_syntax_ctx(SyntaxContext::Expression);
    match_sexpr! {
        &sexpr;

        (iif, check, consequent, alternate) => {
            Ok(template_sexpr!((
                iif.clone(),
                expand_sexpr(check.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(consequent.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(alternate.clone(), bindings, env, expr_ctx)?,
            ) => &sexpr).unwrap())
        },

        (iif, check, consequent) => {
            Ok(template_sexpr!((
                iif.clone(),
                expand_sexpr(check.clone(), bindings, env, expr_ctx)?,
                expand_sexpr(consequent.clone(), bindings, env, expr_ctx)?,
            ) => &sexpr).unwrap())
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

fn expand_set(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    if_let_sexpr! {(set, var @ SExpr::Var(id, _), exp) = &sexpr =>
        if let Some(resolved) = bindings.resolve_sym(id)
            && (Bindings::CORE_FORMS.contains(&resolved.0.as_str()) || Bindings::CORE_PRIMITIVES.contains(&resolved.0.as_str())) {
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

fn expand_define(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
    ctx: Context,
) -> Result<SExpr<Id>> {
    match_sexpr!(
        &sexpr;

        (define, var @ SExpr::Var(id, _), exp) => {
            if matches!(ctx.syntax_ctx, SyntaxContext::Expression) {
                return Err(CompilationError {
                    span: sexpr.get_span(),
                    reason: "'define' is not allowed in an expression context".to_owned(),
                });
            }
            if ctx.syntax_ctx == SyntaxContext::TopLevel {
                let binding = bindings.gen_sym(id);
                bindings.add_binding(id, &binding);
            }
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
            Some(SExpr::Var(id, _)) if bindings.resolve_sym(&id) == Some(Symbol::new("syntax-rules"))
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

        return Ok(sexpr);
    }
    Err(CompilationError {
        span: sexpr.get_span(),
        reason: "Invalid 'define-syntax' form".to_owned(),
    })
}

fn expand_lambda(
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
