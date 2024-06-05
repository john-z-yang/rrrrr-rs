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
    (
        @
        $targ:expr,
        (.. $id:ident) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref $id) = $targ {
            $handler
        };
        match_sexpr! {
            @
            $targ,
            $($tail)*
        };
    };
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
    (
        @
        $targ:expr,
        (? $pat:pat $(, $($rest:tt)*)?) => $handler:expr;
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
    (
        @
        $targ:expr,
        (= $id:ident $(, $($rest:tt)*)?) => $handler:expr;
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
    (
        @
        $targ:expr,
        ($expr:expr $(, $($rest:tt)*)?) => $handler:expr;
        $($tail:tt)*
    ) => {
        if let SExpr::Cons(ref cons) = $targ {
            if cons.car == $expr {
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
    (
        $($tt:tt)*
    ) => {
        loop {
            match_sexpr! {
                @
                $($tt)*
            }
            break;
        }
    };
}
