use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set, Value},
    census::Census,
    ident::ResolvedVar,
};

pub(crate) fn collect_census(expr: &Expr) -> Census {
    let mut census = Census::default();
    collect_census_expr(expr, &mut census);
    census
}

fn collect_census_expr(expr: &Expr, census: &mut Census) {
    match expr {
        Expr::AExpr(aexpr) => collect_census_aexpr(aexpr, census),
        Expr::CExpr(cexpr) => collect_census_cexpr(cexpr, census),
        Expr::Let(Let { initializer, body }, _) => {
            match &initializer.as_ref().1 {
                Rhs::AExpr(aexpr) => collect_census_aexpr(aexpr, census),
                Rhs::CExpr(cexpr) => collect_census_cexpr(cexpr, census),
            }
            collect_census_expr(body, census);
        }
    }
}

fn collect_census_aexpr(aexpr: &AExpr, census: &mut Census) {
    match aexpr {
        AExpr::Var(ResolvedVar::Bound { binding, .. }, _) => {
            census.track_use(binding);
        }
        AExpr::Lambda(Lambda { body, .. }, _) => {
            collect_census_expr(body, census);
        }
        _ => {}
    }
}

fn collect_census_value(value: &Value, census: &mut Census) {
    if let Value::Var(ResolvedVar::Bound { binding, .. }, _) = value {
        census.track_use(binding);
    }
}

fn collect_census_cexpr(cexpr: &CExpr, census: &mut Census) {
    match cexpr {
        CExpr::Application(Application { operand, args }, _) => {
            collect_census_value(operand, census);
            for arg in args {
                collect_census_value(arg, census);
            }
        }
        CExpr::If(If { test, conseq, alt }, _) => {
            collect_census_value(test, census);
            collect_census_expr(conseq, census);
            collect_census_expr(alt, census);
        }
        CExpr::Set(Set { var, aexpr }, _) => {
            let ResolvedVar::Bound { binding, .. } = var else {
                return collect_census_value(aexpr, census);
            };
            census.track_use(binding);
            census.track_rebound(binding);
            collect_census_value(aexpr, census)
        }
    }
}
