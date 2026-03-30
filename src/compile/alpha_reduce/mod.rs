use crate::{
    compile::{
        bindings::{Bindings, Id},
        ident::{Resolved, Symbol},
        sexpr::SExpr,
    },
    if_let_sexpr, make_sexpr, match_sexpr,
};

pub(crate) fn alpha_reduce(sexpr: SExpr<Id>, bindings: &Bindings) -> SExpr<Resolved> {
    match_sexpr! {
        &sexpr;

        (var @ SExpr::Var(id, _), rest @ ..) => {
            let resolved = bindings.resolve_sym(id);
            if resolved.as_ref().is_some_and(|resolved| resolved.0 == "quote") {
                alpha_reduce_quote(sexpr.clone())
            } else {
                make_sexpr!(
                    var.clone().map_var(&|id| match bindings.resolve_sym(&id) {
                        Some(binding) => Resolved::Bound {
                            symbol: id.symbol,
                            binding,
                        },
                        None => Resolved::Free { symbol: id.symbol },
                    }),
                    ..alpha_reduce(rest.clone(), bindings),
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
                None => Resolved::Free { symbol: id.symbol }
            })
        },
    }
}

fn alpha_reduce_quote(sexpr: SExpr<Id>) -> SExpr<Resolved> {
    let span = sexpr.get_span();
    if_let_sexpr! {(_, sexpr) = sexpr => {
        return make_sexpr!(
            SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("quote"),
                    binding: Symbol::new("quote"),
                },
                span,
            ),
            sexpr
                .clone()
                .map_var(&|id| Resolved::Literal { symbol: id.symbol }),
        );
    }};
    unreachable!("Invalid quote form")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        compile::{
            expand::{Env, expand, introduce},
            read::{lex::tokenize, parse::parse},
            sexpr::Num,
            span::Span,
        },
        make_sexpr,
    };

    fn alpha_reduce_source(source: &str) -> SExpr<Resolved> {
        let mut bindings = Bindings::new();
        let mut env = Env::default();
        let sexpr = parse(&tokenize(source).unwrap()).unwrap().pop().unwrap();
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
                    Resolved::Free {
                        symbol: Symbol::new("x"),
                    },
                    span,
                ),
            ),
        );

        assert_eq!(result.without_spans(), expected.without_spans());
    }

    #[test]
    fn test_alpha_reduce_inserted_vars_are_not_rebound() {
        let result = alpha_reduce_source("(begin (define lambda 1) (define (x) 1))");
        let span = Span { lo: 0, hi: 0 };
        let expected = make_sexpr!(
            SExpr::Var(
                Resolved::Bound {
                    symbol: Symbol::new("begin"),
                    binding: Symbol::new("begin"),
                },
                span,
            ),
            (
                SExpr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("lambda"),
                        binding: Symbol::new("lambda"),
                    },
                    span,
                ),
                SExpr::Num(Num(1.0), span),
            ),
            (
                SExpr::Var(
                    Resolved::Bound {
                        symbol: Symbol::new("define"),
                        binding: Symbol::new("define"),
                    },
                    span,
                ),
                SExpr::Var(
                    Resolved::Free {
                        symbol: Symbol::new("x")
                    },
                    span,
                ),
                (
                    SExpr::Var(
                        Resolved::Bound {
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
}
