use crate::{
    compile::{
        core_expr::{Application, Begin, Expr, If, Lambda, Set},
        gensym::GenSym,
        ident::{ResolvedSymbol, ResolvedVar, Symbol},
        sexpr::SExpr,
        util::{first, for_each, len, rest, try_dotted_tail},
    },
    if_let_sexpr,
};

pub(crate) fn lower(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    if let SExpr::Var(resolved, span) = sexpr {
        return Expr::Var(
            resolved
                .try_into()
                .expect("symbol should not be literal in this context"),
            span,
        );
    }
    if_let_sexpr! {(SExpr::Var(..), ..) = &sexpr =>
        return lower_id_application(gen_sym, sexpr);
    };
    if_let_sexpr! {(..) = sexpr =>
        return lower_fn_application(gen_sym, sexpr);
    };
    Expr::Literal(sexpr.map_var(&From::from))
}

fn lower_id_application(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let binding = match first(&sexpr) {
        SExpr::Var(ResolvedSymbol::Bound { binding, .. }, _) => binding.clone(),
        SExpr::Var(ResolvedSymbol::Free { .. }, _) => return lower_fn_application(gen_sym, sexpr),
        _ => unreachable!("expand_id_application expected first element to be a bound/free ID"),
    };

    match binding.0.as_str() {
        "quote" => Expr::Literal(first(rest(sexpr)).map_var(&From::from)),
        "lambda" => lower_lambda(gen_sym, sexpr),
        "letrec" => lower_letrec(gen_sym, sexpr),
        "if" => lower_if(gen_sym, sexpr),
        "define" => lower_define(gen_sym, sexpr),
        "set!" => lower_set(gen_sym, sexpr),
        "begin" => lower_begin(gen_sym, sexpr),
        _ => lower_fn_application(gen_sym, sexpr),
    }
}

fn lower_lambda(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr_arg, sexpr_body @ ..) = sexpr => {
        let mut args = vec![];
        let mut var_arg = None;
        match sexpr_arg {
            SExpr::Var(ResolvedSymbol::Bound { binding, .. }, _) => var_arg = Some(binding),
            SExpr::Cons(..) => {
                var_arg = try_dotted_tail(&sexpr_arg).and_then(|tail| {
                    match tail {
                        SExpr::Var(ResolvedSymbol::Bound { binding, .. }, _) => Some(binding.clone()),
                        SExpr::Nil(_) => None,
                        _ => unreachable!("Bad rest parameter")
                    }
                });
                for_each(sexpr_arg, |arg| {
                    let SExpr::Var(ResolvedSymbol::Bound { binding, .. }, _) = arg else {
                        unreachable!("Bad parameter")
                    };
                    args.push(binding);
                });
            }
            SExpr::Nil(_) => {}
            _ => unreachable!("Invalid lambda form"),
        };

        return Expr::Lambda(
            Lambda {
                args,
                var_arg,
                body: Box::new(lower_body(gen_sym, sexpr_body)),
            },
            span,
        );
    }}
    unreachable!("Invalid lambda form")
}

fn lower_letrec(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr_initializers @ (..), sexpr_body @ ..) = sexpr => {
        let mut vars = vec![];
        let mut rhs_exprs = vec![];
        let mut temps = vec![];
        for_each(sexpr_initializers, |initializer| {
            if_let_sexpr! {(SExpr::Var(ResolvedSymbol::Bound { symbol, binding }, _), exp) = initializer => {
                vars.push((symbol, binding));
                rhs_exprs.push(lower(gen_sym, exp));
                temps.push(gen_sym.fresh("temp"));
                return;
            }};
            unreachable!("Invalid letrec initializer")
        });

        if vars.is_empty() {
            return Expr::Application(
                Application {
                    operand: Box::new(Expr::Lambda(
                        Lambda {
                            args: vec![],
                            var_arg: None,
                            body: Box::new(lower_body(gen_sym, sexpr_body)),
                        },
                        span,
                    )),
                    args: vec![],
                },
                span,
            );
        }

        let mut body: Vec<Expr> = vars
            .iter()
            .cloned()
            .zip(temps.iter().cloned())
            .map(|((symbol, binding), temp)| {
                Expr::Set(
                    Set {
                        var: ResolvedVar::Bound { symbol, binding },
                        expr: Box::new(Expr::Var(
                            ResolvedVar::Bound {
                                symbol: Symbol::new("temp"),
                                binding: temp,
                            },
                            span,
                        )),
                    },
                    span,
                )
            })
            .collect();

        for_each(sexpr_body, |sexpr| {
            body.push(lower(gen_sym, sexpr));
        });

        let inner_binding = Expr::Application(
            Application {
                operand: Box::new(Expr::Lambda(
                    Lambda {
                        args: temps,
                        var_arg: None,
                        body: Box::new(Expr::Begin(Begin { body }, span)),
                    },
                    span,
                )),
                args: rhs_exprs,
            },
            span,
        );

        return Expr::Application(
            Application {
                args: vec![Expr::Literal(SExpr::Void(span)); vars.len()],
                operand: Box::new(Expr::Lambda(
                    Lambda {
                        args: vars.into_iter().map(|(_, binding)| binding).collect(),
                        var_arg: None,
                        body: Box::new(inner_binding),
                    },
                    span,
                )),
            },
            span,
        );
    }}
    unreachable!("Invalid letrec form")
}

fn lower_body(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if len(&sexpr) > 1 {
        let mut body = vec![];
        for_each(sexpr, |sexpr| {
            body.push(lower(gen_sym, sexpr));
        });
        Expr::Begin(Begin { body }, span)
    } else {
        lower(gen_sym, first(sexpr))
    }
}

fn lower_fn_application(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(first, rest @ ..) = sexpr => {
        let operand = lower(gen_sym, first);
        let mut args = vec![];
        for_each(rest, |sexpr| {
            args.push(lower(gen_sym, sexpr));
        });

        return Expr::Application(
            Application {
                operand: Box::new(operand),
                args,
            },
            span,
        );
    }}
    unreachable!("Invalid fn application form")
}

fn lower_if(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, test, consequent, alternate) = sexpr => {
        return Expr::If(
            If {
                test: Box::new(lower(gen_sym, test)),
                conseq: Box::new(lower(gen_sym, consequent)),
                alt: Box::new(lower(gen_sym, alternate)),
            },
            span,
        );
    }}
    unreachable!("Invalid if form")
}

fn lower_set(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, SExpr::Var(var, _), exp) = sexpr => {
        return Expr::Set(
            Set {
                var: var.try_into().expect("symbol should not be literal in set"),
                expr: Box::new(lower(gen_sym, exp)),
            },
            span,
        );
    }};
    unreachable!("Invalid set form")
}

fn lower_define(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, SExpr::Var(var, _), exp) = sexpr => {
        let var = var
            .try_into()
            .expect("symbol should not be literal in define");
        assert!(matches!(var, ResolvedVar::Free { .. }));
        return Expr::Set(
            Set {
                var,
                expr: Box::new(lower(gen_sym, exp)),
            },
            span,
        );
    }};
    unreachable!("Invalid define form")
}

fn lower_begin(gen_sym: &GenSym, sexpr: SExpr<ResolvedSymbol>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_,  rest @ ..) = sexpr => {
        let mut body = vec![];
        for_each(rest, |sexpr| {
            body.push(lower(gen_sym, sexpr));
        });
        return Expr::Begin(
            Begin { body },
            span,
        );
    }};
    unreachable!("Invalid begin form")
}
