use crate::compile::{
    anf::{Expr, Folder, Let, Rhs},
    census::Census,
    compilation_error::Result,
    pass::census_collection::collect_census,
};

pub(crate) fn dce(expr: Expr) -> Expr {
    DceOptimizer {
        census: collect_census(&expr),
    }
    .fold_expr(expr)
    .expect("no override produces Err")
}

struct DceOptimizer {
    census: Census,
}

impl Folder for DceOptimizer {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        Ok(match expr {
            Expr::Let(Let { initializer, body }, span) => {
                let (symbol, rhs) = *initializer;
                if self.census.use_count(&symbol) == 0 && matches!(rhs, Rhs::AExpr(..)) {
                    self.census.eliminate(&rhs);
                    return self.fold_expr(*body);
                }
                let body = self.fold_expr(*body)?;
                if self.census.use_count(&symbol) == 0 && matches!(rhs, Rhs::AExpr(..)) {
                    self.census.eliminate(&rhs);
                    return Ok(body);
                }
                let rhs = self.fold_rhs(rhs)?;
                Expr::Let(
                    Let {
                        initializer: Box::new((symbol, rhs)),
                        body: Box::new(body),
                    },
                    span,
                )
            }
            Expr::AExpr(aexpr) => Expr::AExpr(self.fold_aexpr(aexpr)?),
            Expr::CExpr(cexpr) => Expr::CExpr(self.fold_cexpr(cexpr)?),
        })
    }
}
