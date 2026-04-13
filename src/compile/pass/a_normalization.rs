use std::rc::Rc;

use crate::compile::{
    self,
    anf::{self, Rhs},
    core_expr,
    gensym::GenSym,
    ident::{ResolvedVar, Symbol},
};

type Continuation<T> = Box<dyn FnOnce(GenSym, T) -> anf::Expr>;

pub(crate) fn a_normalize(gen_sym: GenSym, expr: core_expr::Expr) -> anf::Expr {
    a_normalize_expr(gen_sym, expr, Box::new(|_, expr| expr))
}

fn a_normalize_expr(
    gen_sym: GenSym,
    expr: core_expr::Expr,
    k: Continuation<anf::Expr>,
) -> anf::Expr {
    match expr {
        core_expr::Expr::Literal(sexpr) => k(gen_sym, anf::Expr::AExpr(anf::AExpr::Literal(sexpr))),
        core_expr::Expr::Var(resolved, span) => {
            k(gen_sym, anf::Expr::AExpr(anf::AExpr::Var(resolved, span)))
        }
        core_expr::Expr::Lambda(
            core_expr::Lambda {
                args,
                var_arg,
                body,
            },
            span,
        ) => k(
            gen_sym.clone(),
            anf::Expr::AExpr(anf::AExpr::Lambda(
                anf::Lambda {
                    args,
                    var_arg,
                    body: Box::new(a_normalize(gen_sym, *body)),
                },
                span,
            )),
        ),
        core_expr::Expr::Application(core_expr::Application { operand, args }, span) => {
            a_normalize_name(
                gen_sym,
                *operand,
                Box::new(move |gen_sym, normalized_operand| {
                    a_normalize_names(
                        gen_sym.clone(),
                        Vec::with_capacity(args.len()),
                        Rc::new(args),
                        Box::new(move |_, normalized_args| {
                            k(
                                gen_sym,
                                compile::anf::Expr::CExpr(anf::CExpr::Application(
                                    anf::Application {
                                        operand: Box::new(normalized_operand),
                                        args: normalized_args,
                                    },
                                    span,
                                )),
                            )
                        }),
                    )
                }),
            )
        }
        core_expr::Expr::If(core_expr::If { test, conseq, alt }, span) => a_normalize_name(
            gen_sym,
            *test,
            Box::new(move |gen_sym, normalized_test| {
                k(
                    gen_sym.clone(),
                    anf::Expr::CExpr(anf::CExpr::If(
                        anf::If {
                            test: Box::new(normalized_test),
                            conseq: Box::new(a_normalize(gen_sym.clone(), *conseq)),
                            alt: Box::new(a_normalize(gen_sym, *alt)),
                        },
                        span,
                    )),
                )
            }),
        ),
        core_expr::Expr::Set(core_expr::Set { var, expr }, span) => a_normalize_name(
            gen_sym,
            *expr,
            Box::new(move |gen_sym, normalized| {
                k(
                    gen_sym.clone(),
                    anf::Expr::CExpr(anf::CExpr::Set(
                        anf::Set {
                            var,
                            aexpr: normalized,
                        },
                        span,
                    )),
                )
            }),
        ),
        core_expr::Expr::Begin(core_expr::Begin { body }, _) => a_normalize_names(
            gen_sym,
            Vec::with_capacity(body.len()),
            Rc::new(body),
            Box::new(|gen_sym, normalized_body| {
                k(
                    gen_sym,
                    normalized_body
                        .last()
                        .expect("Begin body has at least 1 expr")
                        .clone()
                        .into(),
                )
            }),
        ),
    }
}

fn a_normalize_name(
    gen_sym: GenSym,
    expr: core_expr::Expr,
    k: Continuation<anf::Value>,
) -> anf::Expr {
    a_normalize_expr(
        gen_sym,
        expr,
        Box::new(move |gen_sym, normalized| {
            let span = normalized.get_span();
            match normalized {
                anf::Expr::AExpr(anf::AExpr::Literal(sexpr)) => {
                    k(gen_sym, anf::Value::Literal(sexpr))
                }
                anf::Expr::AExpr(anf::AExpr::Var(resolved, span)) => {
                    k(gen_sym, anf::Value::Var(resolved, span))
                }
                anf::Expr::AExpr(aexpr) => {
                    let sym = gen_sym.fresh("anf");
                    anf::Expr::Let(
                        anf::Let {
                            initializer: Box::new((sym.clone(), Rhs::AExpr(aexpr))),
                            body: Box::new(k(
                                gen_sym,
                                anf::Value::Var(
                                    ResolvedVar::Bound {
                                        symbol: Symbol::new("anf"),
                                        binding: sym,
                                    },
                                    span,
                                ),
                            )),
                        },
                        span,
                    )
                }
                anf::Expr::CExpr(cexpr) => {
                    let sym = gen_sym.fresh("anf");
                    anf::Expr::Let(
                        anf::Let {
                            initializer: Box::new((sym.clone(), Rhs::CExpr(cexpr))),
                            body: Box::new(k(
                                gen_sym,
                                anf::Value::Var(
                                    ResolvedVar::Bound {
                                        symbol: Symbol::new("anf"),
                                        binding: sym,
                                    },
                                    span,
                                ),
                            )),
                        },
                        span,
                    )
                }
                _ => panic!("Unexpected normalized expr {}", normalized),
            }
        }),
    )
}

fn a_normalize_names(
    gen_sym: GenSym,
    mut acc: Vec<anf::Value>,
    exprs: Rc<Vec<core_expr::Expr>>,
    k: Continuation<Vec<anf::Value>>,
) -> anf::Expr {
    if exprs.is_empty() {
        k(gen_sym, acc)
    } else {
        a_normalize_name(
            gen_sym,
            exprs[acc.len()].clone(),
            Box::new(move |gen_sym, normalized| {
                acc.push(normalized);
                if acc.len() == exprs.len() {
                    k(gen_sym, acc)
                } else {
                    a_normalize_names(gen_sym, acc, exprs, k)
                }
            }),
        )
    }
}
