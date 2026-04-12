use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set},
    census::Census,
    ident::Resolved,
};

pub(crate) fn collect(expr: &Expr, census: &mut Census) {
    collect_expr(expr, census);
}

fn collect_expr(expr: &Expr, census: &mut Census) {
    match expr {
        Expr::AExpr(aexpr) => collect_aexpr(aexpr, census),
        Expr::CExpr(cexpr) => collect_cexpr(cexpr, census),
        Expr::Let(Let { initializer, body }, _) => {
            match &initializer.as_ref().1 {
                Rhs::AExpr(aexpr) => collect_aexpr(aexpr, census),
                Rhs::CExpr(cexpr) => collect_cexpr(cexpr, census),
            }
            collect_expr(body, census);
        }
    }
}

fn collect_aexpr(aexpr: &AExpr, census: &mut Census) {
    match aexpr {
        AExpr::Var(Resolved::Bound { binding, .. }, _) => {
            census.track_use(binding);
        }
        AExpr::Var(..) => {}
        AExpr::Literal(..) => {}
        AExpr::Lambda(Lambda { body, .. }, _) => {
            collect_expr(body, census);
        }
    }
}

fn collect_cexpr(cexpr: &CExpr, census: &mut Census) {
    match cexpr {
        CExpr::Application(Application { operand, args }, _) => {
            collect_aexpr(operand, census);
            for arg in args {
                collect_aexpr(arg, census);
            }
        }
        CExpr::If(If { test, conseq, alt }, _) => {
            collect_aexpr(test, census);
            collect_expr(conseq, census);
            collect_expr(alt, census);
        }
        CExpr::Set(Set { var, aexpr }, _) => {
            let Resolved::Bound { binding, .. } = var else {
                return collect_aexpr(aexpr, census);
            };
            census.track_rebound(binding);
            collect_aexpr(aexpr, census)
        }
    }
}
