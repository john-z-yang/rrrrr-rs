use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set, Value},
    census::Census,
    ident::{ResolvedVar, Symbol},
    pass::census_collection,
    sexpr::SExpr,
};

pub(crate) fn propagate_consts(expr: Expr) -> Expr {
    let census = census_collection::collect_census(&expr);
    propagate_consts_expr(expr, &census, &mut HashMap::new())
}

fn propagate_consts_expr(
    expr: Expr,
    census: &Census,
    consts: &mut HashMap<Symbol, SExpr<Symbol>>,
) -> Expr {
    match expr {
        Expr::Let(Let { initializer, body }, span) => {
            let (symbol, rhs) = *initializer;
            let rhs = match rhs {
                Rhs::AExpr(aexpr) => Rhs::AExpr(propagate_consts_aexpr(aexpr, census, consts)),
                Rhs::CExpr(cexpr) => Rhs::CExpr(propagate_consts_cexpr(cexpr, census, consts)),
            };
            if let Rhs::AExpr(AExpr::Literal(sexpr)) = &rhs
                && !census.is_rebound(&symbol)
                && (sexpr.is_atomic() || census.use_count(&symbol) == 1)
            {
                consts.insert(symbol.clone(), sexpr.clone());
            }
            Expr::Let(
                Let {
                    initializer: Box::new((symbol, rhs)),
                    body: Box::new(propagate_consts_expr(*body, census, consts)),
                },
                span,
            )
        }
        Expr::AExpr(aexpr) => Expr::AExpr(propagate_consts_aexpr(aexpr, census, consts)),
        Expr::CExpr(cexpr) => Expr::CExpr(propagate_consts_cexpr(cexpr, census, consts)),
    }
}

fn propagate_consts_aexpr(
    aexpr: AExpr,
    census: &Census,
    consts: &mut HashMap<Symbol, SExpr<Symbol>>,
) -> AExpr {
    match aexpr {
        AExpr::Literal(..) | AExpr::Var(ResolvedVar::Free { .. }, _) => aexpr,
        AExpr::Var(ResolvedVar::Bound { symbol, binding }, span) => {
            if let Some(sexpr) = consts.get(&binding) {
                AExpr::Literal(sexpr.clone())
            } else {
                AExpr::Var(ResolvedVar::Bound { symbol, binding }, span)
            }
        }
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
                body: Box::new(propagate_consts_expr(*body, census, consts)),
            },
            span,
        ),
    }
}

fn propagate_consts_value(value: Value, consts: &mut HashMap<Symbol, SExpr<Symbol>>) -> Value {
    let Value::Var(ResolvedVar::Bound { symbol, binding }, span) = value else {
        return value;
    };
    if let Some(sexpr) = consts.get(&binding) {
        Value::Literal(sexpr.clone())
    } else {
        Value::Var(ResolvedVar::Bound { symbol, binding }, span)
    }
}

fn propagate_consts_cexpr(
    cexpr: CExpr,
    census: &Census,
    consts: &mut HashMap<Symbol, SExpr<Symbol>>,
) -> CExpr {
    match cexpr {
        CExpr::Application(Application { operand, args }, span) => CExpr::Application(
            Application {
                operand: Box::new(propagate_consts_value(*operand, consts)),
                args: args
                    .into_iter()
                    .map(|arg| propagate_consts_value(arg, consts))
                    .collect(),
            },
            span,
        ),
        CExpr::If(If { test, conseq, alt }, span) => CExpr::If(
            If {
                test: Box::new(propagate_consts_value(*test, consts)),
                conseq: Box::new(propagate_consts_expr(*conseq, census, consts)),
                alt: Box::new(propagate_consts_expr(*alt, census, consts)),
            },
            span,
        ),
        CExpr::Set(Set { var, aexpr }, span) => CExpr::Set(
            Set {
                var,
                aexpr: propagate_consts_value(aexpr, consts),
            },
            span,
        ),
    }
}
