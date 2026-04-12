use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set},
    ident::{Resolved, Symbol},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Census {
    data: HashMap<Symbol, VarMeta>,
}

impl Census {
    pub(crate) fn track_use(&mut self, symbol: &Symbol) {
        self.data.entry(symbol.clone()).or_default().use_count += 1;
    }

    pub(crate) fn eliminate(&mut self, rhs: &Rhs) {
        match rhs {
            Rhs::AExpr(aexpr) => self.eliminate_aexpr(aexpr),
            Rhs::CExpr(cexpr) => self.eliminate_cexpr(cexpr),
        }
    }

    pub(crate) fn eliminate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::AExpr(aexpr) => self.eliminate_aexpr(aexpr),
            Expr::CExpr(cexpr) => self.eliminate_cexpr(cexpr),
            Expr::Let(Let { initializer, body }, _) => {
                self.eliminate(&initializer.1);
                self.eliminate_expr(body);
            }
        }
    }

    pub(crate) fn eliminate_aexpr(&mut self, aexpr: &AExpr) {
        match aexpr {
            AExpr::Var(Resolved::Bound { binding, .. }, _) => {
                self.data.get_mut(binding).unwrap().use_count -= 1;
            }
            AExpr::Lambda(Lambda { body, .. }, _) => {
                self.eliminate_expr(body);
            }
            _ => {}
        }
    }

    pub(crate) fn eliminate_cexpr(&mut self, cexpr: &CExpr) {
        match cexpr {
            CExpr::Application(Application { operand, args }, _) => {
                self.eliminate_aexpr(operand);
                for arg in args {
                    self.eliminate_aexpr(arg);
                }
            }
            CExpr::If(If { test, conseq, alt }, _) => {
                self.eliminate_aexpr(test);
                self.eliminate_expr(conseq);
                self.eliminate_expr(alt);
            }
            CExpr::Set(Set { aexpr, .. }, _) => {
                self.eliminate_aexpr(aexpr);
            }
        }
    }

    pub(crate) fn track_rebound(&mut self, symbol: &Symbol) {
        self.data.entry(symbol.clone()).or_default().is_rebound = true;
    }

    pub(crate) fn use_count(&self, symbol: &Symbol) -> usize {
        self.data
            .get(symbol)
            .map(|var_meta| var_meta.use_count)
            .unwrap_or_default()
    }

    pub(crate) fn is_rebound(&self, symbol: &Symbol) -> bool {
        self.data
            .get(symbol)
            .map(|var_meta| var_meta.is_rebound)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct VarMeta {
    pub(crate) use_count: usize,
    pub(crate) is_rebound: bool,
}
