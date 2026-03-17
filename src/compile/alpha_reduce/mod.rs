use crate::{
    compile::{
        bindings::Bindings,
        sexpr::{Id, Resolved, SExpr, Symbol},
    },
    make_sexpr, match_sexpr,
};

pub(crate) fn alpha_reduce(sexpr: SExpr<Id>, bindings: &Bindings) -> SExpr<Resolved> {
    match_sexpr! {
        &sexpr;

        (var @ SExpr::Var(id, span), sexpr) => {
            if bindings.resolve_sym(id).is_some_and(|resolved| resolved.0 == "quote") {
                make_sexpr!(
                    SExpr::Var(
                        Resolved::Bound {
                            symbol: id.symbol.clone(),
                            binding: Symbol::new("quote"),
                        },
                        *span,
                    ),
                    sexpr
                        .clone()
                        .map_var(&|id| Resolved::Literal { symbol: id.symbol }),
                )
            } else {
                make_sexpr!(
                    var.clone().map_var(&|id| match bindings.resolve_sym(&id) {
                        Some(binding) => Resolved::Bound {
                            symbol: id.symbol,
                            binding,
                        },
                        None => Resolved::Unbound { symbol: id.symbol },
                    }),
                    alpha_reduce(sexpr.clone(), bindings)
                )
            }
        },

        SExpr::Cons(cons, _) => {
            SExpr::cons(
                alpha_reduce(*cons.car.clone(), bindings),
                alpha_reduce(*cons.cdr.clone(), bindings),
            )
        },

        _ => {
            sexpr.clone().map_var(&|id| match bindings.resolve_sym(&id) {
                Some(binding) => Resolved::Bound { symbol: id.symbol, binding },
                None => Resolved::Unbound { symbol: id.symbol }
            })
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile::{
            expand::{Env, expand, introduce},
            read::{lex::tokenize, parse::parse},
            span::Span,
        },
        make_sexpr,
    };

    fn alpha_reduce_source(source: &str) -> SExpr<Resolved> {
        let mut bindings = Bindings::new();
        let mut env = Env::default();
        let sexpr = parse(&tokenize(source).unwrap()).unwrap();
        let expanded = expand(introduce(sexpr), &mut bindings, &mut env).unwrap();
        alpha_reduce(expanded, &bindings)
    }

    #[test]
    fn test_alpha_reduce_quote_keeps_quote_bound() {
        let result = alpha_reduce_source("'x");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            SExpr::Var(
                Resolved::Literal {
                    symbol: Symbol::new("x"),
                },
                span,
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_reduce_quote_literalizes_nested_payload_identifiers() {
        let result = alpha_reduce_source("'(x y)");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            (
                SExpr::Var(
                    Resolved::Literal {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
                SExpr::Var(
                    Resolved::Literal {
                        symbol: Symbol::new("y"),
                    },
                    span,
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_reduce_shadowed_quote_is_not_treated_as_literal_form() {
        let result = alpha_reduce_source("(lambda (quote) (quote x))");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("lambda"),
                    binding: Symbol::new("lambda"),
                },
                span,
            ),
            (SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote:1"),
                },
                span,
            )),
            (
                SExpr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("quote"),
                        binding: Symbol::new("quote:1"),
                    },
                    span,
                ),
                SExpr::Var(
                    Resolved::Unbound {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }
}
