use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set},
    census::Census,
    pass::collect_census::collect_census,
};

pub(crate) fn dce(expr: Expr) -> Expr {
    let mut census = collect_census(&expr);
    dce_expr(expr, &mut census)
}

fn dce_expr(expr: Expr, census: &mut Census) -> Expr {
    match expr {
        Expr::Let(Let { initializer, body }, span) => {
            let (symbol, rhs) = *initializer;
            if census.use_count(&symbol) == 0 && matches!(rhs, Rhs::AExpr(..)) {
                census.eliminate(&rhs);
                return dce_expr(*body, census);
            }

            let body = dce_expr(*body, census);
            if census.use_count(&symbol) == 0 && matches!(rhs, Rhs::AExpr(..)) {
                census.eliminate(&rhs);
                return body;
            }

            let rhs = match rhs {
                Rhs::AExpr(aexpr) => Rhs::AExpr(dce_aexpr(aexpr, census)),
                Rhs::CExpr(cexpr) => Rhs::CExpr(dce_cexpr(cexpr, census)),
            };
            Expr::Let(
                Let {
                    initializer: Box::new((symbol, rhs)),
                    body: Box::new(body),
                },
                span,
            )
        }
        Expr::AExpr(aexpr) => Expr::AExpr(dce_aexpr(aexpr, census)),
        Expr::CExpr(cexpr) => Expr::CExpr(dce_cexpr(cexpr, census)),
    }
}

fn dce_aexpr(aexpr: AExpr, census: &mut Census) -> AExpr {
    match aexpr {
        AExpr::Literal(..) | AExpr::Var(..) => aexpr,
        AExpr::Lambda(
            Lambda {
                args,
                var_arg,
                body,
            },
            span,
        ) => AExpr::Lambda(
            Lambda {
                args,
                var_arg,
                body: Box::new(dce_expr(*body, census)),
            },
            span,
        ),
    }
}

fn dce_cexpr(cexpr: CExpr, census: &mut Census) -> CExpr {
    match cexpr {
        CExpr::Application(Application { operand, args }, span) => CExpr::Application(
            Application {
                operand,
                args: args.into_iter().map(|arg| dce_aexpr(arg, census)).collect(),
            },
            span,
        ),
        CExpr::If(If { test, conseq, alt }, span) => CExpr::If(
            If {
                test,
                conseq: Box::new(dce_expr(*conseq, census)),
                alt: Box::new(dce_expr(*alt, census)),
            },
            span,
        ),
        CExpr::Set(Set { var, aexpr }, span) => CExpr::Set(
            Set {
                var,
                aexpr: dce_aexpr(aexpr, census),
            },
            span,
        ),
    }
}
