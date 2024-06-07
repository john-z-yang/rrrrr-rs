#[macro_export]
macro_rules! sexpr {
    () => {
        SExpr::Nil
    };
    (($($inner:tt)*) $(, $($rest:tt)*)?) => {
        SExpr::new_cons(sexpr!($($inner)*), sexpr!($($($rest)*)?))
    };
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
    // Compare if the first element is an exact symbol i.e. `('lambda, ...)`
    (
        @
        $targ:expr,
        ($symbol:lifetime $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            let mut symbol = stringify!($symbol).chars();
            symbol.next();
            if cons.car == SExpr::new_symbol(symbol.as_str()) {
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
