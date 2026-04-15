use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Expr, Folder, Let, Rhs},
    census::Census,
    compilation_error::Result,
    ident::{ResolvedVar, Symbol},
    pass::census_collection,
};

pub(crate) fn propagate_copies(expr: Expr) -> Expr {
    CopyPropOptimizer {
        census: census_collection::collect_census(&expr),
        aliases: HashMap::new(),
    }
    .fold_expr(expr)
    .expect("no override produces Err")
}

struct CopyPropOptimizer {
    census: Census,
    aliases: HashMap<Symbol, Symbol>,
}

impl Folder for CopyPropOptimizer {
    fn fold_resolved_var(&mut self, resolved_var: ResolvedVar) -> Result<ResolvedVar> {
        let ResolvedVar::Bound { symbol, binding } = resolved_var else {
            return Ok(resolved_var);
        };
        Ok(ResolvedVar::Bound {
            symbol,
            binding: self.aliases.get(&binding).unwrap_or(&binding).clone(),
        })
    }

    fn fold_let(&mut self, let_: Let) -> Result<Let> {
        let Let { initializer, body } = let_;
        let (symbol, rhs) = *initializer;
        let rhs = self.fold_rhs(rhs)?;
        if let Rhs::AExpr(AExpr::Var(ResolvedVar::Bound { binding, .. }, _)) = &rhs
            && !self.census.is_rebound(&symbol)
            && !self.census.is_rebound(binding)
        {
            self.aliases.insert(symbol.clone(), binding.clone());
        }
        Ok(Let {
            initializer: Box::new((symbol, rhs)),
            body: Box::new(self.fold_expr(*body)?),
        })
    }
}
