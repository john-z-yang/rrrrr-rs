use super::sexpr::{Cons, SExpr};

#[macro_export]
macro_rules! sexpr {
    () => {
        $crate::compile::sexpr::SExpr::Nil(Span {
            lo: 0,
            hi: 1,
        })
    };
    (..$expr:expr) => {
        $expr
    };
    (..#$symbol:literal) => {
        $crate::compile::sexpr::SExpr::Id(Id::new($symbol), Span {
            lo: 0,
            hi: 1,
        })
    };
    (($($inner:tt)*) $(, $($rest:tt)*)?) => {
        $crate::compile::sexpr::SExpr::cons(
            sexpr!($($inner)*),
            sexpr!($($($rest)*)?)
        )
    };
    ($first:expr $(, $($rest:tt)*)?) => {
        $crate::compile::sexpr::SExpr::cons($first, sexpr!($($($rest)*)?))
    };
}

#[macro_export]
macro_rules! match_sexpr {
    // Empty list aka Nil
    (
        () = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            $($handler)*
        }
    };

    // Handles nested lists i.e. (('a, 'b, 'c))
    (
        (($($inner:tt)*) $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, _) = $targ {
            match_sexpr! {($($inner)*) = cons.car.as_ref() => {
                match_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                    $($handler)*
                }
            }}
        };
    };

    // Matches any list
    (
        (..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(_, _) = $targ {
            $($handler)*
        } else if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            $($handler)*
        };
    };

    // Matches any list, assign the list to an identifier
    (
        ($id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(_, _) = $targ {
            let $id = &$targ;
            $($handler)*
        } else if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            let $id = &$targ;
            $($handler)*
        }
    };

    // Wildcard pattern `_` for first element in a list
    (
        (_ $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, _) = $targ {
            match_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                $($handler)*
            }
        };
    };

    // Match a structural pattern for first element in a list i.e. `(Symbol(var_name), 'b, 'c)`
    (
        ($pat:pat $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, _) = $targ {
            #[allow(irrefutable_let_patterns)]
            if let $pat = cons.car.as_ref() {
                match_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                    $($handler)*
                }
            }
        };
    };
}

#[macro_export]
macro_rules! template_sexpr {
    // Empty list aka Nil i.e. ()
    (@
        () => $targ:ident
    ) => {
        if matches!($targ, SExpr::Nil(_)) {
            Some($targ.clone())
        } else {
            None
        }
    };

    // Nested lists i.e. ((a, b, c))
    (@
        (($($inner:tt)*) $(, $($rest:tt)*)?) => $targ:ident
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, span) = $targ {
            template_sexpr!(($($inner)*) => cons.car.as_ref()).and_then(|car| {
                Some(
                    $crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            car,
                            template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref())?),
                        *span
                ))
            })
        } else {
            None
        }
    };

    // Splat i.e. (.. 3)
    (@
        (..$rest:expr) => $targ:ident
    ) => {
        Some($rest.clone())
    };

    // First element in a list i.e. (a b c)
    (@
        ($first:expr $(, $($rest:tt)*)?) => $targ:expr
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, span) = $targ {
            template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref()).and_then(
                |rest| {
                    Some($crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            $first,
                            rest
                        ),
                        *span,
                    ))
                }
            )
        } else {
            None
        }
    };


    (($($template:tt)*) => $targ:expr) => {{
        #[allow(unused_variables)]
        let res = $targ;
        template_sexpr!(@ ($($template)*) => res)
    }};
}

pub(crate) fn first(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) => Some((*cons.car).clone()),
        _ => None,
    }
}

pub(crate) fn try_for_each<F, E>(mut op: F, sexpr: &SExpr) -> Result<(), E>
where
    F: FnMut(&SExpr) -> Result<(), E>,
{
    if let SExpr::Cons(cons, _) = sexpr {
        op(&cons.car)?;
        try_for_each(op, &cons.cdr)?;
    }
    Ok(())
}

pub(crate) fn try_map<F, E>(mut op: F, sexpr: &SExpr) -> Result<SExpr, E>
where
    F: FnMut(&SExpr) -> Result<SExpr, E>,
{
    match sexpr {
        SExpr::Nil(span) => Ok(SExpr::Nil(*span)),
        SExpr::Cons(cons, span) => Ok(SExpr::Cons(
            Cons::new(op(&cons.car)?, try_map(op, &cons.cdr)?),
            *span,
        )),
        _ => op(sexpr),
    }
}

#[cfg(test)]
mod tests {
    use crate::compile::{lex::tokenize, parse::parse, sexpr::Num, span::Span};

    use super::*;

    #[test]
    fn test_template_sexpr_nil() {
        let original = parse(&tokenize("()").unwrap()).unwrap();
        let templated = template_sexpr!(() => original).unwrap();
        assert!(templated.is_idential(&parse(&tokenize("()").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_single() {
        let original = parse(&tokenize("(0)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), Span {lo: 1, hi: 2 })
            ) => &original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_double() {
        let original = parse(&tokenize("(0 1)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
                SExpr::Num(Num(2.0), Span { lo: 3, hi: 4 })
            ) => &original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 2)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_first() {
        let original = parse(&tokenize("((0) 1)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                (SExpr::Num(Num(1.0), Span { lo: 2, hi: 3 })),
                SExpr::Num(Num(2.0), Span { lo: 5, hi: 6 })
            ) => &original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("((1) 2)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_middle() {
        let original = parse(&tokenize("(0 (1) 2)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
                (SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 })),
                SExpr::Num(Num(3.0), Span { lo: 7, hi: 8 })
            ) => &original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 (2) 3)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_last() {
        let original = parse(&tokenize("(0 (1))").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
                (SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 }))
            ) => &original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 (2))").unwrap()).unwrap()));
    }
}
