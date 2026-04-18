use crate::compile::{
    anf::{CExpr, Expr, Folder, If, Let, Rhs, Value},
    census::Census,
    compilation_error::Result,
    pass::census_collection::collect_census,
    sexpr::{Bool, SExpr},
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
            Expr::CExpr(CExpr::If(If { test, conseq, alt }, _))
                if matches!(*test, Value::Literal(SExpr::Bool(Bool(false), _))) =>
            {
                self.census.eliminate_expr(conseq.as_ref());
                self.fold_expr(*alt)?
            }
            Expr::CExpr(CExpr::If(If { test, conseq, alt }, _))
                if matches!(*test, Value::Literal(_)) =>
            {
                self.census.eliminate_expr(alt.as_ref());
                self.fold_expr(*conseq)?
            }
            Expr::AExpr(aexpr) => Expr::AExpr(self.fold_aexpr(aexpr)?),
            Expr::CExpr(cexpr) => Expr::CExpr(self.fold_cexpr(cexpr)?),
        })
    }
}
