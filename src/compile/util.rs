use super::sexpr::{Cons, SExpr};

#[macro_export]
macro_rules! sexpr {
    () => {
        $crate::compile::sexpr::SExpr::Nil(SourceLoc {
            line: 0,
            idx: 0,
            width: 1,
        })
    };
    (..$expr:expr) => {
        $expr
    };
    (..#$symbol:literal) => {
        $crate::compile::sexpr::SExpr::Id(Id::new($symbol), SourceLoc {
            line: 0,
            idx: 0,
            width: 1,
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
        if let $crate::compile::sexpr::SExpr::Cons(ref cons, _) = $targ {
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
        if let $crate::compile::sexpr::SExpr::Cons(ref cons) = $targ {
            match_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                $($handler)*
            }
        };
    };

    // Compare if the first element is an exact symbol or id i.e. `('lambda, ...)`
    (
        (# $symbol:literal $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(ref cons, _) = $targ {
            let symbol = $crate::compile::sexpr::Symbol::new($symbol);
            if let $crate::compile::sexpr::SExpr::Id(ref id, _) = cons.car.as_ref() {
                if id.symbol == symbol {
                    match_sexpr! {($($($rest)*)?) = cons.cdr.as_ref() =>
                        $($handler)*
                    }
                }
            };
        };
    };

    // Match a structual pattern for first element in a list i.e. `(Symbol(var_name), 'b, 'c)`
    (
        ($pat:pat $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(ref cons, _) = $targ {
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
        if let $crate::compile::sexpr::SExpr::Cons(ref cons, ref source_loc) = $targ {
            template_sexpr!(($($inner)*) => cons.car.as_ref()).and_then(|car| {
                Some(
                    $crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            car,
                            template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref())?),
                        *source_loc
                ))
            })
        } else {
            println!("nested");
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
        if let $crate::compile::sexpr::SExpr::Cons(ref cons, ref source_loc) = $targ {
            template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref()).and_then(
                |rest| {
                    Some($crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            $first,
                            rest
                        ),
                        *source_loc,
                    ))
                }
            )
        } else {
            println!("elem");
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

pub(crate) fn for_each<F>(mut op: F, sexpr: &SExpr)
where
    F: FnMut(&SExpr),
{
    if let SExpr::Cons(cons, _) = sexpr {
        op(&cons.car);
        for_each(op, &cons.cdr);
    }
}

pub(crate) fn map<F>(mut op: F, sexpr: &SExpr) -> SExpr
where
    F: FnMut(&SExpr) -> SExpr,
{
    match sexpr {
        SExpr::Nil(source_loc) => SExpr::Nil(*source_loc),
        SExpr::Cons(cons, source_loc) => {
            SExpr::Cons(Cons::new(op(&cons.car), map(op, &cons.cdr)), *source_loc)
        }
        _ => op(sexpr),
    }
}

#[cfg(test)]
mod tests {
    use crate::compile::{lex::tokenize, parse::parse, sexpr::Num, source_loc::SourceLoc};

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
                SExpr::Num(Num(1.0), SourceLoc { line: 0, idx: 1, width: 1 })
            ) => original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_double() {
        let original = parse(&tokenize("(0 1)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), SourceLoc { line: 0, idx: 1, width: 1 }),
                SExpr::Num(Num(2.0), SourceLoc { line: 0, idx: 3, width: 1 })
            ) => original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 2)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_first() {
        let original = parse(&tokenize("((0) 1)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                (SExpr::Num(Num(1.0), SourceLoc { line: 0, idx: 2, width: 1 })),
                SExpr::Num(Num(2.0), SourceLoc { line: 0, idx: 5, width: 1 })
            ) => original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("((1) 2)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_middle() {
        let original = parse(&tokenize("(0 (1) 2)").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), SourceLoc { line: 0, idx: 1, width: 1 }),
                (SExpr::Num(Num(2.0), SourceLoc { line: 0, idx: 4, width: 1 })),
                SExpr::Num(Num(3.0), SourceLoc { line: 0, idx: 7, width: 1 })
            ) => original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 (2) 3)").unwrap()).unwrap()));
    }

    #[test]
    fn test_template_sexpr_nested_list_last() {
        let original = parse(&tokenize("(0 (1))").unwrap()).unwrap();
        let templated = template_sexpr!(
            (
                SExpr::Num(Num(1.0), SourceLoc { line: 0, idx: 1, width: 1 }),
                (SExpr::Num(Num(2.0), SourceLoc { line: 0, idx: 4, width: 1 }))
            ) => original)
        .unwrap();
        assert!(templated.is_idential(&parse(&tokenize("(1 (2))").unwrap()).unwrap()));
    }
}
