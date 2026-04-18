use std::collections::{HashMap, VecDeque};

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, Folder, Lambda, Let, Rhs, Value},
    census::Census,
    compilation_error::{CompilationError, Result},
    ident::{ResolvedVar, Symbol},
    pass::{census_collection, dce},
    span::Span,
};

pub(crate) fn beta_reduce(expr: Expr) -> Result<Expr> {
    let expr = BetaReduceOptimizer {
        census: census_collection::collect_census(&expr),
        lambda_definitions: HashMap::new(),
    }
    .fold_expr(expr)?;
    Ok(dce::dce(expr))
}

struct BetaReduceOptimizer {
    census: Census,
    lambda_definitions: HashMap<Symbol, Lambda>,
}

impl BetaReduceOptimizer {
    fn build_inlined_expr(body: Expr, args: &mut VecDeque<(Symbol, Value)>, span: Span) -> Expr {
        let Some((sym, val)) = args.pop_front() else {
            return body;
        };
        Expr::Let(
            Let {
                initializer: Box::new((sym, Rhs::AExpr(val.into()))),
                body: Box::new(Self::build_inlined_expr(body, args, span)),
            },
            span,
        )
    }

    fn inline_lambda_app(lambda: Lambda, args: Vec<Value>, span: Span) -> Result<Expr> {
        let Lambda {
            args: arg_syms,
            body,
            ..
        } = lambda;
        if arg_syms.len() != args.len() {
            return Err(CompilationError {
                span,
                reason: format!(
                    "Invalid application: expected {} arguments, but got {}",
                    arg_syms.len(),
                    args.len()
                ),
            });
        }
        Ok(Self::build_inlined_expr(
            *body,
            &mut arg_syms.into_iter().zip(args).collect(),
            span,
        ))
    }
}

impl Folder for BetaReduceOptimizer {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        match expr {
            Expr::Let(Let { initializer, body }, span) => {
                let (symbol, rhs) = *initializer;
                let rhs = self.fold_rhs(rhs)?;
                if let Rhs::AExpr(AExpr::Lambda(lambda, _)) = &rhs
                    && lambda.var_arg.is_none()
                    && self.census.use_count(&symbol) == 1
                    && !self.census.is_rebound(&symbol)
                {
                    self.lambda_definitions
                        .insert(symbol.clone(), lambda.clone());
                }
                Ok(Expr::Let(
                    Let {
                        initializer: Box::new((symbol, rhs)),
                        body: Box::new(self.fold_expr(*body)?),
                    },
                    span,
                ))
            }
            Expr::CExpr(
                ref cexpr @ CExpr::Application(
                    Application {
                        ref operand,
                        ref args,
                    },
                    span,
                ),
            ) => {
                if let Value::Var(ResolvedVar::Bound { binding, .. }, _) = operand.as_ref()
                    && let Some(lambda) = self.lambda_definitions.get(binding)
                {
                    self.fold_expr(Self::inline_lambda_app(lambda.clone(), args.clone(), span)?)
                } else {
                    Ok(Expr::CExpr(self.fold_cexpr(cexpr.clone())?))
                }
            }
            Expr::AExpr(aexpr) => Ok(Expr::AExpr(self.fold_aexpr(aexpr)?)),
            Expr::CExpr(cexpr) => Ok(Expr::CExpr(self.fold_cexpr(cexpr)?)),
        }
    }

    fn fold_rhs(&mut self, rhs: Rhs) -> Result<Rhs> {
        if let Rhs::CExpr(CExpr::Application(Application { operand, args }, span)) = &rhs
            && let Value::Var(ResolvedVar::Bound { binding, .. }, _) = operand.as_ref()
            && let Some(lambda) = self.lambda_definitions.get(binding)
            && args.is_empty()
            && lambda.args.is_empty()
        {
            match Self::inline_lambda_app(lambda.clone(), args.clone(), *span)? {
                Expr::AExpr(aexpr) => return Ok(Rhs::AExpr(aexpr)),
                Expr::CExpr(cexpr) => return Ok(Rhs::CExpr(cexpr)),
                Expr::Let(..) => {}
            }
        }
        Ok(match rhs {
            Rhs::AExpr(aexpr) => Rhs::AExpr(self.fold_aexpr(aexpr)?),
            Rhs::CExpr(cexpr) => Rhs::CExpr(self.fold_cexpr(cexpr)?),
        })
    }
}
