use crate::{
    compile::{
        core_expr::{Application, Begin, Define, Expr, If, Lambda, Letrec, Set},
        ident::Resolved,
        sexpr::SExpr,
        util::{first, for_each, len, rest, try_dotted_tail},
    },
    if_let_sexpr,
};

pub(crate) fn lower(sexpr: SExpr<Resolved>) -> Expr {
    if let SExpr::Var(resolved, span) = sexpr {
        return Expr::Var(resolved, span);
    }
    if_let_sexpr! {(SExpr::Var(..), ..) = &sexpr =>
        return lower_id_application(sexpr);
    };
    if_let_sexpr! {(..) = sexpr =>
        return lower_fn_application(sexpr);
    };
    Expr::Literal(sexpr.map_var(&From::from))
}

fn lower_id_application(sexpr: SExpr<Resolved>) -> Expr {
    let binding = match first(&sexpr) {
        SExpr::Var(Resolved::Bound { binding, .. }, _) => binding.clone(),
        SExpr::Var(Resolved::Free { .. }, _) => return lower_fn_application(sexpr),
        _ => unreachable!("expand_id_application expected first element to be a bound/free ID"),
    };

    match binding.0.as_str() {
        "quote" => Expr::Literal(first(rest(sexpr)).map_var(&From::from)),
        "lambda" => lower_lambda(sexpr),
        "letrec" => lower_letrec(sexpr),
        "if" => lower_if(sexpr),
        "define" => lower_define(sexpr),
        "set!" => lower_set(sexpr),
        "begin" => lower_begin(sexpr),
        _ => lower_fn_application(sexpr),
    }
}

fn lower_lambda(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr_arg, sexpr_body @ ..) = sexpr => {
        let mut args = vec![];
        let mut var_arg = None;
        match sexpr_arg {
            SExpr::Var(Resolved::Bound { binding, .. }, _) => var_arg = Some(binding),
            SExpr::Cons(..) => {
                var_arg = try_dotted_tail(&sexpr_arg).and_then(|tail| {
                    match tail {
                        SExpr::Var(Resolved::Bound { binding, .. }, _) => Some(binding.clone()),
                        SExpr::Nil(_) => None,
                        _ => unreachable!("Bad rest parameter")
                    }
                });
                for_each(sexpr_arg, |arg| {
                    let SExpr::Var(Resolved::Bound { binding, .. }, _) = arg else {
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
                body: Box::new(lower_body(sexpr_body)),
            },
            span,
        );
    }}
    unreachable!("Invalid lambda form")
}

fn lower_letrec(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr_initializers @ (..), sexpr_body @ ..) = sexpr => {
        let mut initializers = vec![];
        for_each(sexpr_initializers, |initializer| {
            if_let_sexpr! {(SExpr::Var(Resolved::Bound { binding, .. }, _), exp) = initializer => {
                initializers.push((binding, lower(exp)));
                return;
            }};
            unreachable!("Invalid letrec initializer")
        });
        return Expr::Letrec(
            Letrec {
                initializers,
                body: Box::new(lower_body(sexpr_body)),
            },
            span,
        );
    }}
    unreachable!("Invalid letrec form")
}

fn lower_body(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if len(&sexpr) > 1 {
        let mut body = vec![];
        for_each(sexpr, |sexpr| {
            body.push(lower(sexpr));
        });
        Expr::Begin(Begin { body }, span)
    } else {
        lower(first(sexpr))
    }
}

fn lower_fn_application(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(first, rest @ ..) = sexpr => {
        let operand = lower(first);
        let mut args = vec![];
        for_each(rest, |sexpr| {
            args.push(lower(sexpr));
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

fn lower_if(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, test, consequent, alternate) = sexpr => {
        return Expr::If(
            If {
                test: Box::new(lower(test)),
                conseq: Box::new(lower(consequent)),
                alt: Box::new(lower(alternate)),
            },
            span,
        );
    }}
    unreachable!("Invalid if form")
}

fn lower_set(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, SExpr::Var(var, _), exp) = sexpr => {
        return Expr::Set(
            Set {
                var,
                expr: Box::new(lower(exp)),
            },
            span,
        );
    }};
    unreachable!("Invalid set form")
}

fn lower_define(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, SExpr::Var(var, _), exp) = sexpr => {
        return Expr::Define(
            Define {
                var,
                expr: Box::new(lower(exp)),
            },
            span,
        );
    }};
    unreachable!("Invalid define form")
}

fn lower_begin(sexpr: SExpr<Resolved>) -> Expr {
    let span = sexpr.get_span();
    if_let_sexpr! {(_,  rest @ ..) = sexpr => {
        let mut body = vec![];
        for_each(rest, |sexpr| {
            body.push(lower(sexpr));
        });
        return Expr::Begin(
            Begin { body },
            span,
        );
    }};
    unreachable!("Invalid begin form")
}
