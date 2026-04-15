use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Expr, Folder, Let, Rhs, Value},
    census::Census,
    compilation_error::Result,
    ident::{ResolvedVar, Symbol},
    pass::census_collection,
    sexpr::SExpr,
};

pub(crate) fn propagate_consts(expr: Expr) -> Expr {
    ConstPropOptimizer {
        census: census_collection::collect_census(&expr),
        consts: HashMap::new(),
    }
    .fold_expr(expr)
    .expect("no override produces Err")
}

struct ConstPropOptimizer {
    census: Census,
    consts: HashMap<Symbol, SExpr<Symbol>>,
}

impl Folder for ConstPropOptimizer {
    fn fold_aexpr(&mut self, aexpr: AExpr) -> Result<AExpr> {
        Ok(match aexpr {
            AExpr::Literal(..) | AExpr::Var(ResolvedVar::Free { .. }, _) => aexpr,
            AExpr::Lambda(lambda, span) => AExpr::Lambda(self.fold_lambda(lambda)?, span),
            AExpr::Var(ResolvedVar::Bound { symbol, binding }, span) => {
                if let Some(sexpr) = self.consts.get(&binding) {
                    AExpr::Literal(sexpr.clone())
                } else {
                    AExpr::Var(ResolvedVar::Bound { symbol, binding }, span)
                }
            }
        })
    }

    fn fold_value(&mut self, value: Value) -> Result<Value> {
        Ok(match value {
            Value::Var(ResolvedVar::Bound { symbol, binding }, span) => {
                if let Some(sexpr) = self.consts.get(&binding) {
                    Value::Literal(sexpr.clone())
                } else {
                    Value::Var(ResolvedVar::Bound { symbol, binding }, span)
                }
            }
            Value::Literal(..) | Value::Var(ResolvedVar::Free { .. }, _) => value,
        })
    }

    fn fold_let(&mut self, let_: Let) -> Result<Let> {
        let Let { initializer, body } = let_;
        let (symbol, rhs) = *initializer;
        let rhs = self.fold_rhs(rhs)?;
        if let Rhs::AExpr(AExpr::Literal(sexpr)) = &rhs
            && !self.census.is_rebound(&symbol)
            && (sexpr.is_atomic() || self.census.use_count(&symbol) == 1)
        {
            self.consts.insert(symbol.clone(), sexpr.clone());
        }
        Ok(Let {
            initializer: Box::new((symbol, rhs)),
            body: Box::new(self.fold_expr(*body)?),
        })
    }
}
