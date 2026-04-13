use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set, Value},
    census::Census,
    ident::{ResolvedVar, Symbol},
    pass::census_collection,
};

pub(crate) fn propagate_copies(expr: Expr) -> Expr {
    let census = census_collection::collect_census(&expr);
    propagate_copies_expr(expr, &census, &mut HashMap::new())
}

fn propagate_copies_expr(
    expr: Expr,
    census: &Census,
    aliases: &mut HashMap<Symbol, Symbol>,
) -> Expr {
    match expr {
        Expr::Let(Let { initializer, body }, span) => {
            let (symbol, rhs) = *initializer;
            let rhs = match rhs {
                Rhs::AExpr(aexpr) => Rhs::AExpr(propagate_copies_aexpr(aexpr, census, aliases)),
                Rhs::CExpr(cexpr) => Rhs::CExpr(propagate_copies_cexpr(cexpr, census, aliases)),
            };
            if let Rhs::AExpr(AExpr::Var(ResolvedVar::Bound { binding, .. }, _)) = &rhs
                && !census.is_rebound(&symbol)
                && !census.is_rebound(binding)
            {
                aliases.insert(symbol.clone(), binding.clone());
            }
            Expr::Let(
                Let {
                    initializer: Box::new((symbol, rhs)),
                    body: Box::new(propagate_copies_expr(*body, census, aliases)),
                },
                span,
            )
        }
        Expr::AExpr(aexpr) => Expr::AExpr(propagate_copies_aexpr(aexpr, census, aliases)),
        Expr::CExpr(cexpr) => Expr::CExpr(propagate_copies_cexpr(cexpr, census, aliases)),
    }
}

fn propagate_copies_aexpr(
    aexpr: AExpr,
    census: &Census,
    aliases: &mut HashMap<Symbol, Symbol>,
) -> AExpr {
    match aexpr {
        AExpr::Literal(..) | AExpr::Var(ResolvedVar::Free { .. }, _) => aexpr,
        AExpr::Var(ResolvedVar::Bound { symbol, binding }, span) => AExpr::Var(
            ResolvedVar::Bound {
                symbol,
                binding: aliases.get(&binding).unwrap_or(&binding).clone(),
            },
            span,
        ),
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
                body: Box::new(propagate_copies_expr(*body, census, aliases)),
            },
            span,
        ),
    }
}

fn propagate_copies_value(value: Value, aliases: &mut HashMap<Symbol, Symbol>) -> Value {
    let Value::Var(ResolvedVar::Bound { symbol, binding }, span) = value else {
        return value;
    };
    Value::Var(
        ResolvedVar::Bound {
            symbol,
            binding: aliases.get(&binding).unwrap_or(&binding).clone(),
        },
        span,
    )
}

fn propagate_copies_cexpr(
    cexpr: CExpr,
    census: &Census,
    aliases: &mut HashMap<Symbol, Symbol>,
) -> CExpr {
    match cexpr {
        CExpr::Application(Application { operand, args }, span) => CExpr::Application(
            Application {
                operand: Box::new(propagate_copies_value(*operand, aliases)),
                args: args
                    .into_iter()
                    .map(|arg| propagate_copies_value(arg, aliases))
                    .collect(),
            },
            span,
        ),
        CExpr::If(If { test, conseq, alt }, span) => CExpr::If(
            If {
                test: Box::new(propagate_copies_value(*test, aliases)),
                conseq: Box::new(propagate_copies_expr(*conseq, census, aliases)),
                alt: Box::new(propagate_copies_expr(*alt, census, aliases)),
            },
            span,
        ),
        CExpr::Set(Set { var, aexpr }, span) => CExpr::Set(
            Set {
                var,
                aexpr: propagate_copies_value(aexpr, aliases),
            },
            span,
        ),
    }
}
