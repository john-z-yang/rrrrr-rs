use std::collections::{HashMap, VecDeque};

use crate::compile::{
    anf::{AExpr, Application, CExpr, Expr, If, Lambda, Let, Rhs, Set},
    census::Census,
    compilation_error::{CompilationError, Result},
    ident::{ResolvedVar, Symbol},
    pass::{census_collection, dce},
    span::Span,
};

pub(crate) fn beta_contract(expr: Expr) -> Result<Expr> {
    let census = census_collection::collect_census(&expr);
    let expr = beta_contract_expr(expr, &census, &mut HashMap::new())?;
    Ok(dce::dce(expr))
}

fn build_let(body: Expr, args: &mut VecDeque<(Symbol, AExpr)>, span: Span) -> Expr {
    let Some((sym, val)) = args.pop_front() else {
        return body;
    };
    Expr::Let(
        Let {
            initializer: Box::new((sym, Rhs::AExpr(val))),
            body: Box::new(build_let(body, args, span)),
        },
        span,
    )
}

fn beta_contract_lambda_app(lambda: Lambda, args: Vec<AExpr>, span: Span) -> Result<Expr> {
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
    Ok(build_let(
        *body,
        &mut arg_syms.into_iter().zip(args).collect(),
        span,
    ))
}

fn beta_contract_expr(
    expr: Expr,
    census: &Census,
    lambda_defintions: &mut HashMap<Symbol, Lambda>,
) -> Result<Expr> {
    match expr {
        Expr::Let(Let { initializer, body }, span) => {
            let (symbol, rhs) = *initializer;
            if let Rhs::AExpr(AExpr::Lambda(lambda, _)) = &rhs {
                lambda_defintions.insert(symbol.clone(), lambda.clone());
            }
            let rhs = match rhs {
                Rhs::AExpr(aexpr) => {
                    Rhs::AExpr(beta_contract_aexpr(aexpr, census, lambda_defintions)?)
                }
                Rhs::CExpr(cexpr) => {
                    Rhs::CExpr(beta_contract_cexpr(cexpr, census, lambda_defintions)?)
                }
            };
            Ok(Expr::Let(
                Let {
                    initializer: Box::new((symbol, rhs)),
                    body: Box::new(beta_contract_expr(*body, census, lambda_defintions)?),
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
            if let AExpr::Var(ResolvedVar::Bound { binding, .. }, _) = operand.as_ref()
                && let Some(lambda) = lambda_defintions.get(binding)
                && lambda.var_arg.is_none()
                && census.use_count(binding) == 1
                && !census.is_rebound(binding)
            {
                beta_contract_expr(
                    beta_contract_lambda_app(lambda.clone(), args.clone(), span)?,
                    census,
                    lambda_defintions,
                )
            } else {
                Ok(Expr::CExpr(beta_contract_cexpr(
                    cexpr.clone(),
                    census,
                    lambda_defintions,
                )?))
            }
        }
        Expr::AExpr(aexpr) => Ok(Expr::AExpr(beta_contract_aexpr(
            aexpr,
            census,
            lambda_defintions,
        )?)),
        Expr::CExpr(cexpr) => Ok(Expr::CExpr(beta_contract_cexpr(
            cexpr,
            census,
            lambda_defintions,
        )?)),
    }
}

fn beta_contract_aexpr(
    aexpr: AExpr,
    census: &Census,
    lambda_defintions: &mut HashMap<Symbol, Lambda>,
) -> Result<AExpr> {
    match aexpr {
        AExpr::Literal(..) | AExpr::Var(..) => Ok(aexpr),
        AExpr::Lambda(
            Lambda {
                args,
                var_arg,
                body,
            },
            span,
        ) => Ok(AExpr::Lambda(
            Lambda {
                args,
                var_arg,
                body: Box::new(beta_contract_expr(*body, census, lambda_defintions)?),
            },
            span,
        )),
    }
}

fn beta_contract_cexpr(
    cexpr: CExpr,
    census: &Census,
    lambda_defintions: &mut HashMap<Symbol, Lambda>,
) -> Result<CExpr> {
    match cexpr {
        CExpr::Application(Application { operand, args }, span) => Ok(CExpr::Application(
            Application {
                operand,
                args: args
                    .into_iter()
                    .map(|arg| beta_contract_aexpr(arg, census, lambda_defintions))
                    .collect::<Result<Vec<_>>>()?,
            },
            span,
        )),
        CExpr::If(If { test, conseq, alt }, span) => Ok(CExpr::If(
            If {
                test,
                conseq: Box::new(beta_contract_expr(*conseq, census, lambda_defintions)?),
                alt: Box::new(beta_contract_expr(*alt, census, lambda_defintions)?),
            },
            span,
        )),
        CExpr::Set(Set { var, aexpr }, span) => Ok(CExpr::Set(
            Set {
                var,
                aexpr: beta_contract_aexpr(aexpr, census, lambda_defintions)?,
            },
            span,
        )),
    }
}
