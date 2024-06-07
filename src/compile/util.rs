use super::syntax::SExpr;

#[macro_export]
macro_rules! sexpr {
    () => {
        SExpr::Nil
    };
    (..$expr:expr) => {
        $expr
    };
    (($($inner:tt)*) $(, $($rest:tt)*)?) => {
        SExpr::new_cons(sexpr!($($inner)*), sexpr!($($($rest)*)?))
    };
    ($first:lifetime $(, $($rest:tt)*)?) => {{
        let mut symbol = stringify!($first).chars();
        symbol.next();
        SExpr::new_cons(SExpr::new_symbol(symbol.as_str()), sexpr!($($($rest)*)?))
    }};
    ($first:expr $(, $($rest:tt)*)?) => {
        SExpr::new_cons($first, sexpr!($($($rest)*)?))
    };
}

#[macro_export]
macro_rules! match_sexpr {
    // No branches remaining, we must have gotten here via a list, verify that the target is a Nil
    (
        @
        $targ:expr,
    ) => {};
    (
        @
        $targ:expr,
        () => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Nil = $targ {
            $handler
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Handles nested lists i.e. `(('a, 'b, 'c))``
    (
        @
        $targ:expr,
        (($($inner:tt)*) $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            match_sexpr! {
                @
                cons.car,
                ($($inner)*) => {
                    match_sexpr! {
                        @
                        cons.cdr,
                        ($($($rest)*)?) => $handler;
                    }
                };
            }
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Matches any list
    (
        @
        $targ:expr,
        (..) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(_) = $targ {
            $handler
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Matches any list, assign the list to an identifier
    (
        @
        $targ:expr,
        (.. $id:ident) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(_) = $targ {
            let $id = &$targ;
            $handler
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Wildcard pattern `_` for first element in a list
    (
        @
        $targ:expr,
        (_ $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            match_sexpr! {
                @
                cons.cdr,
                ($($($rest)*)?) => $handler;
            }
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Compare if the first element is an exact symbol or id i.e. `('lambda, ...)`
    (
        @
        $targ:expr,
        ($symbol:lifetime $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            let mut symbol = stringify!($symbol).chars();
            symbol.next();
            let symbol = Symbol::new(symbol.as_str());
            if let SExpr::Symbol(ref sym) = &cons.car {
                if *sym == symbol {
                    match_sexpr! {
                        @
                        cons.cdr,
                        ($($($rest)*)?) => $handler;
                    }
                }
            } else if let SExpr::Id(ref id) = &cons.car {
                if id.symbol == symbol {
                    match_sexpr! {
                        @
                        cons.cdr,
                        ($($($rest)*)?) => $handler;
                    }
                }
            };
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Binds an identifier to the first element in a list i.e. `(my_var, 'b, 'c)``
    (
        @
        $targ:expr,
        ($id:ident $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            let $id = &cons.car;
            match_sexpr! {
                @
                cons.cdr,
                ($($($rest)*)?) => $handler;
            }
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Match a structual pattern for first element in a list i.e. `(Symbol(var_name), 'b, 'c)`
    (
        @
        $targ:expr,
        ($pat:pat $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            if let $pat = &cons.car {
                match_sexpr! {
                    @
                    cons.cdr,
                    ($($($rest)*)?) => $handler;
                }
            }
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Wildcard for any single entity
    (
        @
        $targ:expr,
        _ => $handler:expr;
        $($tail:tt)*
    ) => {
        $handler;
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Match a structual pattern for any single entity
    (
        @
        $targ:expr,
        $pat:pat => $handler:expr;
        $($tail:tt)*
    ) => {
        if let $pat = $targ {
            $handler;
        }
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
    // Main entry point
    (
        $($tt:tt)*
    ) => {
        match_sexpr! {
            @
            $($tt)*
        }
    };
}

pub fn first(sexpr: &SExpr) -> SExpr {
    match sexpr {
        SExpr::Cons(cons) => cons.car.clone(),
        _ => sexpr.clone(),
    }
}

pub fn last(sexpr: &SExpr) -> SExpr {
    match sexpr {
        SExpr::Cons(cons) => last(&cons.cdr),
        _ => sexpr.clone(),
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
