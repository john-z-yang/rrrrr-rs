use crate::compile::sexpr::ListAccess;

use super::sexpr::{Cons, SExpr};

#[macro_export]
macro_rules! make_sexpr {
    (..$expr:expr $(,)?) => {
        $expr
    };
    (($($inner:tt)+) $(,)?) => {{
        let car = $crate::make_sexpr!($($inner)+);
        let span = car.get_span();
        $crate::compile::sexpr::SExpr::cons(
            car,
            $crate::compile::sexpr::SExpr::Nil(span),
        )
    }};
    (($($inner:tt)+), $($rest:tt)+) => {
        $crate::compile::sexpr::SExpr::cons(
            $crate::make_sexpr!($($inner)+),
            $crate::make_sexpr!($($rest)+),
        )
    };
    ($expr:expr $(,)?) => {{
        let car = $expr;
        let span = car.get_span();
        $crate::compile::sexpr::SExpr::cons(
            car,
            $crate::compile::sexpr::SExpr::Nil(span),
        )
    }};
    ($first:expr, $($rest:tt)+) => {
        $crate::compile::sexpr::SExpr::cons($first, $crate::make_sexpr!($($rest)+))
    };
}

#[macro_export]
macro_rules! if_let_sexpr {
    // 0: Empty list
    (
        () = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            $($handler)*
        }
    };

    // 1: Any list
    (
        (..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(_, _) = $targ {
            $($handler)*
        } else if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            $($handler)*
        };
    };


    // 2: Nested list with tail capture i.e. ((a, b), rest @ ..)
    (
        (($($inner:tt)*), $id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ) {
            $crate::if_let_sexpr! {($($inner)*) = car => {
                $crate::if_let_sexpr! {@tail_pos ($id @ ..) = cdr =>
                    $($handler)*
                }
            }}
        };
    };

    // 3: Nested list i.e. ((a, b), c, d)
    (
        (($($inner:tt)*) $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ) {
            $crate::if_let_sexpr! {($($inner)*) = car => {
                $crate::if_let_sexpr! {($($($rest)*)?) = cdr =>
                    $($handler)*
                }
            }}
        };
    };

    // 4: Assign id to structural pattern with tail capture i.e. (first @ (a, b, c), rest @ ..)
    (
        ($id:ident @ ($($pat:tt)*), $tail:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ) {
            let $id = car;
            $crate::if_let_sexpr! {($($pat)*) = &$id =>
                $crate::if_let_sexpr! {@tail_pos ($tail @ ..) = cdr =>
                    $($handler)*
                }
            }
        };
    };

    // 5: Assign id to structural pattern i.e. (first @ (a, b, c), ...)
    (
        ($id:ident @ ($($pat:tt)*) $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ) {
            let $id = car;
            $crate::if_let_sexpr! {($($pat)*) = &$id =>
                $crate::if_let_sexpr! {($($($rest)*)?) = cdr =>
                    $($handler)*
                }
            }
        };
    };

    // 6: Structural pattern with tail capture i.e. (a, rest @ ..)
    (
        ($pat:pat, $id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        #[allow(irrefutable_let_patterns)]
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ)
            && let $pat = car {
            $crate::if_let_sexpr! {@tail_pos ($id @ ..) = cdr =>
                $($handler)*
            }
        };
    };

    // 7: Structural pattern i.e. (a, b, c)
    (
        ($pat:pat $(, $($rest:tt)*)?) = $targ:expr => $($handler:tt)*
    ) => {
        #[allow(irrefutable_let_patterns)]
        if let Some((car, cdr)) = $crate::compile::sexpr::ListAccess::try_destruct($targ)
            && let $pat = car {
            $crate::if_let_sexpr! {($($($rest)*)?) = cdr =>
                $($handler)*
            }
        };
    };

    // 8: Internal rule: we came from (<some pattern>, $id:ident @ ..) and we just matched the
    // previous car. Now $id:ident @ .. should match anything
    (
        @tail_pos ($id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        let $id = $targ;
        $($handler)*
    };
}

#[macro_export]
macro_rules! match_sexpr {
    // Entry point: bind target, start processing arms
    ($targ:expr; $($arms:tt)*) => {{
        let targ = $targ;
        match_sexpr!(@arms targ, $($arms)*)
    }};

    // Default arm (base case)
    (@arms $targ:ident, _ => $default:block $(,)?) => {
        $default
    };

    // Regular arm followed by more arms
    (@arms $targ:ident, ($($pat:tt)*) => $handler:block, $($rest:tt)*) => {{
        let mut result = None;
        $crate::if_let_sexpr! { ($($pat)*) = $targ =>
            result = Some($handler);
        }
        match result {
            Some(val) => val,
            None => match_sexpr!(@arms $targ, $($rest)*)
        }
    }};

    // Bare pattern arm followed by more arms
    (@arms $targ:ident, $pat:pat => $handler:block, $($rest:tt)*) => {{
        let mut result = None;
        #[allow(irrefutable_let_patterns)]
        if let $pat = $targ {
            result = Some($handler);
        }
        match result {
            Some(val) => val,
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
        if matches!($targ, $crate::compile::sexpr::SExpr::Nil(_)) {
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
            let (car, cdr) = cons.into();
            if let Some(car) = template_sexpr!(($($inner)*) => car) {
                if let Some(cdr) = template_sexpr!(($($($rest)*)?) => cdr) {
                    Some($crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            car,
                            cdr,
                        ),
                        span.clone()
                    ))
                } else {
                    None
                }
            } else {
                None
            }
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
            let (_, cdr) = cons.into();
            if let Some(rest) = template_sexpr!(($($($rest)*)?) => cdr) {
                Some($crate::compile::sexpr::SExpr::Cons(
                    $crate::compile::sexpr::Cons::new(
                        $first,
                        rest,
                    ),
                    span.clone(),
                ))
            } else {
                None
            }
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

pub fn try_first<S: ListAccess>(sexpr: S) -> Option<S> {
    sexpr.try_destruct().map(|(car, _)| car)
}

pub fn first<S: ListAccess>(sexpr: S) -> S {
    try_first(sexpr).expect("first expected parameter to be a cons")
}

pub fn try_rest<S: ListAccess>(sexpr: S) -> Option<S> {
    sexpr.try_destruct().map(|(_, cdr)| cdr)
}

pub fn rest<S: ListAccess>(sexpr: S) -> S {
    try_rest(sexpr).expect("rest expected parameter to be a cons")
}

pub fn len<T>(sexpr: &SExpr<T>) -> usize {
    let mut res = 0;
    let mut cur = sexpr;
    while let SExpr::Cons(Cons { cdr, .. }, _) = cur {
        res += 1;
        cur = cdr;
    }
    res
}

pub fn try_dotted_tail<T>(sexpr: &SExpr<T>) -> Option<&SExpr<T>> {
    let SExpr::Cons(Cons { cdr: cur, .. }, _) = sexpr else {
        return None;
    };
    let mut cur: &SExpr<T> = cur.as_ref();
    while let SExpr::Cons(Cons { cdr, .. }, _) = cur {
        cur = cdr;
    }
    Some(cur)
}

pub fn is_proper_list<T>(sexpr: &SExpr<T>) -> bool {
    if let SExpr::Nil(_) = sexpr {
        return true;
    }
    try_dotted_tail(sexpr).is_some_and(|tail| matches!(tail, SExpr::Nil(_)))
}

pub fn append<T>(head: SExpr<T>, tail: SExpr<T>) -> SExpr<T> {
    if let SExpr::Nil(_) = head {
        return tail;
    }
    let SExpr::Cons(mut cons, span) = head else {
        unreachable!("append expected a proper list")
    };
    let mut cur = &mut *cons.cdr;
    while let SExpr::Cons(cons, _) = cur {
        cur = &mut *cons.cdr;
    }
    *cur = tail;
    SExpr::Cons(cons, span)
}

pub fn try_nth<S: ListAccess>(sexpr: S, idx: usize) -> Option<S> {
    let (car, cdr) = sexpr.try_destruct()?;
    if idx == 0 {
        Some(car)
    } else {
        try_nth(cdr, idx - 1)
    }
}

pub fn try_last<S: ListAccess>(sexpr: S) -> Option<S> {
    let (mut last, mut cur) = sexpr.try_destruct()?;
    while let Some((car, cdr)) = cur.try_destruct() {
        last = car;
        cur = cdr;
    }
    Some(last)
}

pub fn for_each<S, F>(sexpr: S, mut op: F)
where
    S: ListAccess,
    F: FnMut(S),
{
    let mut cur = sexpr;
    while let Some((car, cdr)) = cur.try_destruct() {
        op(car);
        cur = cdr;
    }
}

pub fn try_for_each<S: ListAccess, F, E>(sexpr: S, mut op: F) -> Result<(), E>
where
    F: FnMut(S) -> Result<(), E>,
{
    let mut cur = sexpr;
    while let Some((car, cdr)) = cur.try_destruct() {
        op(car)?;
        cur = cdr;
    }
    Ok(())
}

pub fn try_map<T, F, E>(sexpr: SExpr<T>, mut op: F) -> Result<SExpr<T>, E>
where
    F: FnMut(SExpr<T>) -> Result<SExpr<T>, E>,
{
    if let SExpr::Cons(cons, span) = sexpr {
        return Ok(SExpr::Cons(
            Cons::new(op(*cons.car)?, try_map(*cons.cdr, op)?),
            span,
        ));
    }
    Ok(sexpr)
}

pub fn split<T>(list: SExpr<T>, n: usize) -> (SExpr<T>, SExpr<T>) {
    if n == 0 {
        let span = list.get_span();
        return (SExpr::Nil(span), list);
    }
    let SExpr::Cons(cons, _) = list else {
        panic!("split expected a cons list")
    };
    let (car, cdr) = cons.into();
    if n == 1 {
        let span = car.get_span();
        return (SExpr::cons(car, SExpr::Nil(span)), cdr);
    }
    let (head, rest) = split(cdr, n - 1);
    (SExpr::cons(car, head), rest)
}
