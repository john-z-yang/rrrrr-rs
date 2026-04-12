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

    pub(crate) fn decrement_use(&mut self, rhs: &Rhs) {
        match rhs {
            Rhs::AExpr(aexpr) => self.decrement_use_aexpr(aexpr),
            Rhs::CExpr(cexpr) => self.decrement_use_cexpr(cexpr),
        }
    }

    pub(crate) fn decrement_use_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::AExpr(aexpr) => self.decrement_use_aexpr(aexpr),
            Expr::CExpr(cexpr) => self.decrement_use_cexpr(cexpr),
            Expr::Let(Let { initializer, body }, _) => {
                self.decrement_use(&initializer.1);
                self.decrement_use_expr(body);
            }
        }
    }

    pub(crate) fn decrement_use_aexpr(&mut self, aexpr: &AExpr) {
        match aexpr {
            AExpr::Var(Resolved::Bound { binding, .. }, _) => {
                self.data.get_mut(binding).unwrap().use_count -= 1;
            }
            AExpr::Lambda(Lambda { body, .. }, _) => {
                self.decrement_use_expr(body);
            }
            _ => {}
        }
    }

    pub(crate) fn decrement_use_cexpr(&mut self, cexpr: &CExpr) {
        match cexpr {
            CExpr::Application(Application { operand, args }, _) => {
                self.decrement_use_aexpr(operand);
                for arg in args {
                    self.decrement_use_aexpr(arg);
                }
            }
            CExpr::If(If { test, conseq, alt }, _) => {
                self.decrement_use_aexpr(test);
                self.decrement_use_expr(conseq);
                self.decrement_use_expr(alt);
            }
            CExpr::Set(Set { aexpr, .. }, _) => {
                self.decrement_use_aexpr(aexpr);
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
