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

    // Handles nested lists i.e. `(('a, 'b, 'c))``
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

pub fn first(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) => Some((*cons.car).clone()),
        _ => None,
    }
}

pub fn for_each<F>(mut op: F, sexpr: &SExpr)
where
    F: FnMut(&SExpr),
{
    if let SExpr::Cons(cons, _) = sexpr {
        op(&cons.car);
        for_each(op, &cons.cdr);
    }
}

pub fn map<F>(mut op: F, sexpr: &SExpr) -> SExpr
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
