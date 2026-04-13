use std::collections::BTreeSet;

use crate::{
    compile::{
        bindings::{Bindings, Id},
        ident::{ResolvedSymbol, Symbol},
        sexpr::SExpr,
    },
    if_let_sexpr, make_sexpr, match_sexpr,
};

pub(crate) fn alpha_convert(sexpr: SExpr<Id>, bindings: &mut Bindings) -> SExpr<ResolvedSymbol> {
    match_sexpr! {
        &sexpr;

        (var @ SExpr::Var(id, _), rest @ ..) => {
            let resolved = bindings.resolve_sym(id);
            if resolved.as_ref().is_some_and(|resolved| resolved.0 == "quote") {
                alpha_convert_quote(sexpr.clone())
            } else {
                let var = var.clone().map_var(&make_resolver(bindings));
                make_sexpr!(
                    var,
                    ..alpha_convert(rest.clone(), bindings),
                )
            }
        },

        SExpr::Cons(cons, _) => {
            SExpr::cons(
                alpha_convert(*cons.car.clone(), bindings),
                alpha_convert(*cons.cdr.clone(), bindings),
            )
        },

        _ => {
            sexpr.clone().map_var(&make_resolver(bindings))
        },
    }
}

fn make_resolver(bindings: &Bindings) -> impl Fn(Id) -> ResolvedSymbol {
    |id| {
        let Some(Id {
            symbol: binding,
            scopes,
        }) = bindings.resolve(&id)
        else {
            return ResolvedSymbol::Free { symbol: id.symbol };
        };
        if scopes == BTreeSet::from([Bindings::CORE_SCOPE, Bindings::TOP_LEVEL_SCOPE]) {
            ResolvedSymbol::Free { symbol: id.symbol }
        } else {
            ResolvedSymbol::Bound {
                symbol: id.symbol,
                binding,
            }
        }
    }
}

fn alpha_convert_quote(sexpr: SExpr<Id>) -> SExpr<ResolvedSymbol> {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr) = sexpr => {
        return make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            sexpr
                .clone()
                .map_var(&|id| ResolvedSymbol::Literal { symbol: id.symbol }),
        );
    }};
    unreachable!("Invalid quote form")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile::{
            pass::expand::{Env, expand, introduce},
            pass::read::{lex::tokenize, parse::parse},
            sexpr::Num,
            span::Span,
        },
        make_sexpr,
    };

    fn alpha_convert_source(source: &str) -> SExpr<ResolvedSymbol> {
        let mut bindings = Bindings::new(Default::default());
        let mut env = Env::default();
        let sexpr = parse(&tokenize(source).unwrap()).unwrap().pop().unwrap();
        let expanded = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
        alpha_convert(expanded, &mut bindings)
    }

    #[test]
    fn test_alpha_convert_quote_keeps_quote_bound() {
        let result = alpha_convert_source("'x");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            SExpr::Var(
                ResolvedSymbol::Literal {
                    symbol: Symbol::new("x"),
                },
                span,
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_quote_literalizes_nested_payload_identifiers() {
        let result = alpha_convert_source("'(x y)");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Literal {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
                SExpr::Var(
                    ResolvedSymbol::Literal {
                        symbol: Symbol::new("y"),
                    },
                    span,
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_first_define_init_expr_has_free_self_reference() {
        let result = alpha_convert_source("(define x x)");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("define"),
                    binding: Symbol::new("define"),
                },
                span,
            ),
            SExpr::Var(
                ResolvedSymbol::Free {
                    symbol: Symbol::new("x"),
                },
                span,
            ),
            SExpr::Var(
                ResolvedSymbol::Free {
                    symbol: Symbol::new("x"),
                },
                span,
            ),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_set_after_define_uses_same_binding() {
        let result = alpha_convert_source("(begin (define x 1) (set! x 2))");
        let span = Span { lo: 0, hi: 0 };
        let x_binding = ResolvedSymbol::Free {
            symbol: Symbol::new("x"),
        };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("begin"),
                    binding: Symbol::new("begin"),
                },
                span,
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(x_binding.clone(), span),
                SExpr::Num(crate::compile::sexpr::Num(1.0), span),
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("set!"),
                        binding: Symbol::new("set!"),
                    },
                    span,
                ),
                SExpr::Var(x_binding, span),
                SExpr::Num(crate::compile::sexpr::Num(2.0), span),
            ),
        );
        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_shadowed_quote_is_not_treated_as_literal_form() {
        let result = alpha_convert_source("(lambda (quote) (quote x))");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("lambda"),
                    binding: Symbol::new("lambda"),
                },
                span,
            ),
            (SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote:1"),
                },
                span,
            )),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("quote"),
                        binding: Symbol::new("quote:1"),
                    },
                    span,
                ),
                SExpr::Var(
                    ResolvedSymbol::Free {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_inserted_vars_are_not_rebound() {
        let result = alpha_convert_source("(begin (define lambda 1) (define (x) 1))");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("begin"),
                    binding: Symbol::new("begin"),
                },
                span,
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(
                    ResolvedSymbol::Free {
                        symbol: Symbol::new("lambda")
                    },
                    span,
                ),
                SExpr::Num(Num(1.0), span),
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(
                    ResolvedSymbol::Free {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
                (
                    SExpr::Var(
                        ResolvedSymbol::Bound {
                            symbol: Symbol::new("lambda"),
                            binding: Symbol::new("lambda"),
                        },
                        span,
                    ),
                    SExpr::Nil(span),
                    SExpr::Num(Num(1.0), span),
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_convert_rebind_lambda_in_begin() {
        let result = alpha_convert_source("(begin (lambda () 1) (define lambda 1) lambda)");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                ResolvedSymbol::Bound {
                    symbol: Symbol::new("begin"),
                    binding: Symbol::new("begin"),
                },
                span,
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("lambda"),
                        binding: Symbol::new("lambda"),
                    },
                    span,
                ),
                SExpr::Nil(span),
                SExpr::Num(Num(1.0), span),
            ),
            (
                SExpr::Var(
                    ResolvedSymbol::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(
                    ResolvedSymbol::Free {
                        symbol: Symbol::new("lambda"),
                    },
                    span,
                ),
                SExpr::Num(Num(1.0), span),
            ),
            SExpr::Var(
                ResolvedSymbol::Free {
                    symbol: Symbol::new("lambda"),
                },
                span,
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }
}
