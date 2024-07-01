use super::syntax::SExpr;

#[macro_export]
macro_rules! sexpr {
    () => {
        $crate::compile::syntax::SExpr::Nil
    };
    (..$expr:expr) => {
        $expr
    };
    (($($inner:tt)*) $(, $($rest:tt)*)?) => {
        $crate::compile::syntax::SExpr::new_cons(
            sexpr!($($inner)*),
            sexpr!($($($rest)*)?)
        )
    };
    (S($($symbol:tt)*) $(, $($rest:tt)*)?) => {{
        let symbol = stringify!($($symbol)*).chars();
        $crate::compile::syntax::SExpr::new_cons(
            $crate::compile::syntax::SExpr::new_symbol(symbol.as_str()),
            sexpr!($($($rest)*)?)
        )
    }};
    ($first:expr $(, $($rest:tt)*)?) => {
        $crate::compile::syntax::SExpr::new_cons($first, sexpr!($($($rest)*)?))
    };
}

#[macro_export]
macro_rules! match_sexpr {
    // Empty list aka Nil
    (
        () = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Nil = $targ {
            $($handler)*
        }
    };

    // Handles nested lists i.e. `(('a, 'b, 'c))``
    (
        (($($inner:tt)*) $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Cons(ref cons) = $targ {
            match_sexpr! {($($inner)*) = cons.car => {
                match_sexpr! {($($($rest)*)?) = cons.cdr =>
                    $($handler)*
                }
            }}
        };
    };

    // Matches any list
    (
        (..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Cons(_) = $targ {
            $($handler)*
        } else if let $crate::compile::syntax::SExpr::Nil = $targ {
            $($handler)*
        };
    };

    // Matches any list, assign the list to an identifier
    (
        ($id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Cons(_) = $targ {
            let $id = &$targ;
            $($handler)*
        } else if let $crate::compile::syntax::SExpr::Nil = $targ {
            let $id = &$targ;
            $($handler)*
        }
    };

    // Wildcard pattern `_` for first element in a list
    (
        (_ $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Cons(ref cons) = $targ {
            match_sexpr! {($($($rest)*)?) = cons.cdr =>
                $($handler)*
            }
        };
    };

    // Compare if the first element is an exact symbol or id i.e. `('lambda, ...)`
    (
        (S($($symbol:tt)*) $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::syntax::SExpr::Cons(ref cons) = $targ {
            let symbol = stringify!($($symbol)*).chars();
            let symbol = $crate::compile::syntax::Symbol::new(symbol.as_str());
            if let $crate::compile::syntax::SExpr::Symbol(ref sym) = &cons.car {
                if *sym == symbol {
                    match_sexpr! {($($($rest)*)?) = cons.cdr =>
                        $($handler)*
                    }
                }
            } else if let $crate::compile::syntax::SExpr::Id(ref id) = &cons.car {
                if id.symbol == symbol {
                    match_sexpr! {($($($rest)*)?) = cons.cdr =>
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
        if let $crate::compile::syntax::SExpr::Cons(ref cons) = $targ {
            #[allow(irrefutable_let_patterns)]
            if let $pat = &cons.car {
                match_sexpr! {($($($rest)*)?) = cons.cdr =>
                    $($handler)*
                }
            }
        };
    };
}

pub fn first(sexpr: &SExpr) -> SExpr {
    match sexpr {
        SExpr::Cons(cons) => cons.car.clone(),
        _ => sexpr.clone(),
    }
}

pub fn last(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons) if matches!(cons.cdr, SExpr::Nil) => Some(cons.car.clone()),
        SExpr::Cons(cons) => last(&cons.cdr),
        _ => None,
    }
}

pub fn nth(sexpr: &SExpr, idx: usize) -> Option<SExpr> {
    let SExpr::Cons(cons) = sexpr else {
        return None;
    };
    if idx == 0 {
        Some(cons.car.clone())
    } else {
        nth(&cons.cdr, idx - 1)
    }
}

pub fn for_each<F>(mut op: F, sexpr: &SExpr)
where
    F: FnMut(&SExpr),
{
    if let SExpr::Cons(cons) = sexpr {
        op(&cons.car);
        for_each(op, &cons.cdr);
    }
}

pub fn map<F>(mut op: F, sexpr: &SExpr) -> SExpr
where
    F: FnMut(&SExpr) -> SExpr,
{
    match sexpr {
        SExpr::Nil => SExpr::Nil,
        SExpr::Cons(cons) => SExpr::new_cons(op(&cons.car), map(op, &cons.cdr)),
        _ => op(sexpr),
    }
}
