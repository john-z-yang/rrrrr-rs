use std::collections::HashMap;

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, Folder, Lambda, Let, Rhs, Value},
    census::Census,
    compilation_error::Result,
    ident::{ResolvedVar, Symbol},
    pass::census_collection,
};

pub(crate) fn eta_reduce(expr: Expr) -> Expr {
    EtaReduceOptimizer {
        census: census_collection::collect_census(&expr),
        aliases: HashMap::new(),
    }
    .fold_expr(expr)
    .expect("no override produces Err")
}

struct EtaReduceOptimizer {
    census: Census,
    aliases: HashMap<Symbol, Symbol>,
}

impl EtaReduceOptimizer {
    fn try_eta(lambda: &Lambda) -> Option<Symbol> {
        let Lambda {
            args: lambda_args,
            var_arg,
            body,
        } = lambda;
        let Expr::CExpr(CExpr::Application(
            Application {
                operand,
                args: app_args,
            },
            _,
        )) = body.as_ref()
        else {
            return None;
        };

        let Value::Var(
            ResolvedVar::Bound {
                binding: operand, ..
            },
            _,
        ) = operand.as_ref()
        else {
            return None;
        };

        if var_arg.is_some()
            || lambda_args.contains(operand)
            || lambda_args.len() != app_args.len()
            || lambda_args
                .iter()
                .zip(app_args.iter())
                .any(|(lambda_arg, app_arg)| {
                    let Value::Var(ResolvedVar::Bound { binding, .. }, _) = app_arg else {
                        return true;
                    };
                    lambda_arg != binding
                })
        {
            return None;
        }
        Some(operand.clone())
    }
}

impl Folder for EtaReduceOptimizer {
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
        if !self.census.is_rebound(&symbol)
            && let Rhs::AExpr(AExpr::Lambda(lambda, _)) = &rhs
            && let Some(reduced) = Self::try_eta(lambda)
            && !self.census.is_rebound(&reduced)
        {
            self.aliases.insert(symbol.clone(), reduced.clone());
        };
        Ok(Let {
            initializer: Box::new((symbol, rhs)),
            body: Box::new(self.fold_expr(*body)?),
        })
    }
}
