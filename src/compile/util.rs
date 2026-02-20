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
macro_rules! if_let_sexpr {
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
            if_let_sexpr! {($($inner)*) = cons.car.as_ref() => {
                if_let_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
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
            if_let_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
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
                if_let_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                    $($handler)*
                }
            }
        };
    };
}

#[macro_export]
macro_rules! match_sexpr {
    // Entry point: bind target, start processing arms
    ($targ:expr; $($arms:tt)*) => {{
        let __targ = $targ;
        match_sexpr!(@arms __targ, $($arms)*)
    }};

    // Default arm (base case)
    (@arms $targ:ident, _ => $default:block $(,)?) => {
        $default
    };

    // Regular arm followed by more arms
    (@arms $targ:ident, ($($pat:tt)*) => $handler:block, $($rest:tt)*) => {{
        let mut __result = None;
        if_let_sexpr! { ($($pat)*) = $targ => { __result = Some($handler); } }
        match __result {
            Some(__val) => __val,
            None => match_sexpr!(@arms $targ, $($rest)*)
        }
    }};
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

pub(crate) fn try_first(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) => Some(*cons.car.clone()),
        _ => None,
    }
}

pub(crate) fn first(sexpr: &SExpr) -> SExpr {
    let SExpr::Cons(cons, _) = sexpr else {
        unreachable!("Expecting parameter to be a cons")
    };
    *cons.car.clone()
}

pub(crate) fn rest(sexpr: &SExpr) -> SExpr {
    let SExpr::Cons(cons, _) = sexpr else {
        unreachable!("Expecting parameter to be a cons")
    };
    *cons.cdr.clone()
}

pub(crate) fn len(sexpr: &SExpr) -> usize {
    let mut res = 0;
    let mut cur = sexpr;
    while let SExpr::Cons(Cons { cdr, .. }, _) = cur {
        res += 1;
        cur = cdr;
    }
    res
}

pub(crate) fn try_dotted_tail(sexpr: &SExpr) -> Option<SExpr> {
    if let SExpr::Nil(_) = sexpr {
        None
    } else if let SExpr::Cons(cons, _) = sexpr {
        try_dotted_tail(&cons.cdr)
    } else {
        Some(sexpr.clone())
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
    if let SExpr::Cons(cons, span) = sexpr {
        return Ok(SExpr::Cons(
            Cons::new(op(&cons.car)?, try_map(op, &cons.cdr)?),
            *span,
        ));
    }
    Ok(sexpr.clone())
}

#[cfg(test)]
mod tests {
    use crate::compile::{lex::tokenize, parse::parse, sexpr::Num, span::Span};

    use super::*;

    #[test]
    fn test_multi_match_sexpr() {
        let nil = parse(&tokenize("()").unwrap()).unwrap();
        let list = parse(&tokenize("(1 2 3)").unwrap()).unwrap();
        let num = parse(&tokenize("42").unwrap()).unwrap();

        let classify = |sexpr: &SExpr| -> &str {
            match_sexpr! {
                sexpr;

                () => { "nil" },
                (..) => { "list" },
                _ => { "other" },
            }
        };

        assert_eq!(classify(&nil), "nil");
        assert_eq!(classify(&list), "list");
        assert_eq!(classify(&num), "other");
    }

    #[test]
    fn test_multi_match_sexpr_arm_priority() {
        let list = parse(&tokenize("(1 2)").unwrap()).unwrap();

        // First matching arm wins — (_, _) matches before (..)
        let result: &str = match_sexpr! {
            &list;

            (_, _) => { "two" },
            (..) => { "any-list" },
            _ => { "other" },
        };
        assert_eq!(result, "two");

        // Single-element list should skip (_, _) and match (..)
        let single = parse(&tokenize("(1)").unwrap()).unwrap();
        let result: &str = match_sexpr! {
            &single;

            (_, _) => { "two" },
            (..) => { "any-list" },
            _ => { "other" },
        };
        assert_eq!(result, "any-list");
    }

    #[test]
    fn test_multi_match_sexpr_nested_list() {
        let nested = parse(&tokenize("((a b) c)").unwrap()).unwrap();
        let flat = parse(&tokenize("(a b c)").unwrap()).unwrap();

        let classify = |sexpr: &SExpr| -> &str {
            match_sexpr! {
                sexpr;

                ((_first, _), _) => { "nested-pair" },
                (_, _, _) => { "three" },
                _ => { "other" },
            }
        };

        assert_eq!(classify(&nested), "nested-pair");
        assert_eq!(classify(&flat), "three");
    }

    #[test]
    fn test_multi_match_sexpr_with_try_operator() {
        fn extract_second(sexpr: &SExpr) -> Result<&SExpr, &str> {
            match_sexpr! {
                sexpr;

                (_, second, _) => { Ok(second) },
                _ => { Err("expected a 3-element list") },
            }
        }

        let list = parse(&tokenize("(1 2 3)").unwrap()).unwrap();
        let short = parse(&tokenize("(1)").unwrap()).unwrap();

        assert!(matches!(extract_second(&list), Ok(SExpr::Num(Num(2.0), _))));
        assert!(extract_second(&short).is_err());
    }

    #[test]
    fn test_multi_match_sexpr_default_arm() {
        let num = parse(&tokenize("42").unwrap()).unwrap();
        let result: i32 = match_sexpr! {
            &num;
            () => { 0 },
            (..) => { 1 },
            _ => { 2 },
        };
        assert_eq!(result, 2);
    }

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
