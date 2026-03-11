use super::sexpr::{Cons, SExpr};

#[macro_export]
macro_rules! make_sexpr {
    (..$expr:expr $(,)?) => {
        $expr
    };
    (($($inner:tt)+) $(,)?) => {{
        let car = make_sexpr!($($inner)+);
        let span = car.get_span();
        $crate::compile::sexpr::SExpr::cons(
            car,
            $crate::compile::sexpr::SExpr::Nil(span),
        )
    }};
    (($($inner:tt)+), $($rest:tt)+) => {
        $crate::compile::sexpr::SExpr::cons(
            make_sexpr!($($inner)+),
            make_sexpr!($($rest)+),
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
        $crate::compile::sexpr::SExpr::cons($first, make_sexpr!($($rest)+))
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

    // Any list
    (
        (..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(_, _) = $targ {
            $($handler)*
        } else if let $crate::compile::sexpr::SExpr::Nil(_) = $targ {
            $($handler)*
        };
    };

    // Any list, assign the list to an identifier
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


    // Nested list with tail capture i.e. ((a, b), rest @ ..)
    (
        (($($inner:tt)*), $id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, _) = $targ {
            if_let_sexpr! {($($inner)*) = cons.car.as_ref() => {
                if_let_sexpr! {@tail_pos ($id @ ..) = cons.cdr.as_ref() =>
                    $($handler)*
                }
            }}
        };
    };

    // Nested list i.e. ((a, b), c, d)
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

    // Structural pattern with tail capture i.e. (a, rest @ ..)
    (
        ($pat:pat, $id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        if let $crate::compile::sexpr::SExpr::Cons(cons, _) = $targ {
            #[allow(irrefutable_let_patterns)]
            if let $pat = cons.car.as_ref() {
                if_let_sexpr! {@tail_pos ($id @ ..) = cons.cdr.as_ref() =>
                    $($handler)*
                }
            }
        };
    };

    // Structural pattern i.e. (a, b, c)
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

    // Internal rule: we came from (<some pattern>, $id:ident @ ..) and we just matched the
    // previous car. Now $id:ident @ .. should match anything
    (
        @tail_pos ($id:ident @ ..) = $targ:expr => $($handler:tt)*
    ) => {
        let $id = &$targ;
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
        if_let_sexpr! { ($($pat)*) = $targ =>
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
            if let Some(car) = template_sexpr!(($($inner)*) => cons.car.as_ref()) {
                if let Some(cdr) = template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref()) {
                    Some($crate::compile::sexpr::SExpr::Cons(
                        $crate::compile::sexpr::Cons::new(
                            car,
                            cdr,
                        ),
                        *span
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
            if let Some(rest) = template_sexpr!(($($($rest)*)?) => cons.cdr.as_ref()) {
                Some($crate::compile::sexpr::SExpr::Cons(
                    $crate::compile::sexpr::Cons::new(
                        $first,
                        rest,
                    ),
                    *span,
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

pub fn try_first(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) => Some(*cons.car.clone()),
        _ => None,
    }
}

pub fn first(sexpr: &SExpr) -> SExpr {
    try_first(sexpr).expect("first expected parameter to be a cons")
}

pub fn try_rest(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) => Some(*cons.cdr.clone()),
        _ => None,
    }
}

pub fn rest(sexpr: &SExpr) -> SExpr {
    try_rest(sexpr).expect("rest expected parameter to be a cons")
}

pub fn len(sexpr: &SExpr) -> usize {
    let mut res = 0;
    let mut cur = sexpr;
    while let SExpr::Cons(Cons { cdr, .. }, _) = cur {
        res += 1;
        cur = cdr;
    }
    res
}

pub fn try_dotted_tail(sexpr: &SExpr) -> Option<SExpr> {
    let SExpr::Cons(Cons { cdr: cur, .. }, _) = sexpr else {
        return None;
    };
    let mut cur: &SExpr = cur.as_ref();
    while let SExpr::Cons(Cons { cdr, .. }, _) = cur {
        cur = cdr;
    }
    Some(cur.clone())
}

pub fn is_proper_list(sexpr: &SExpr) -> bool {
    if let SExpr::Nil(_) = sexpr {
        return true;
    }
    try_dotted_tail(sexpr).is_some_and(|tail| matches!(tail, SExpr::Nil(_)))
}

pub fn append(head: &SExpr, tail: &SExpr) -> SExpr {
    match head {
        SExpr::Nil(_) => tail.clone(),
        SExpr::Cons(cons, span) => {
            SExpr::Cons(Cons::new(*cons.car.clone(), append(&cons.cdr, tail)), *span)
        }
        _ => unreachable!("append expected a proper list"),
    }
}

pub fn nth(sexpr: &SExpr, idx: usize) -> Option<SExpr> {
    let SExpr::Cons(cons, _) = sexpr else {
        return None;
    };
    if idx == 0 {
        Some(cons.car.as_ref().clone())
    } else {
        nth(&cons.cdr, idx - 1)
    }
}

pub fn last(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons, _) if matches!(*cons.cdr, SExpr::Nil(_)) => {
            Some(cons.car.as_ref().clone())
        }
        SExpr::Cons(cons, _) => last(&cons.cdr),
        _ => None,
    }
}

pub fn try_for_each<F, E>(sexpr: &SExpr, mut op: F) -> Result<(), E>
where
    F: FnMut(&SExpr) -> Result<(), E>,
{
    let mut cur = sexpr;
    while let SExpr::Cons(Cons { car, cdr }, _) = cur {
        op(car)?;
        cur = cdr;
    }
    Ok(())
}

pub fn try_map<F, E>(sexpr: &SExpr, mut op: F) -> Result<SExpr, E>
where
    F: FnMut(&SExpr) -> Result<SExpr, E>,
{
    if let SExpr::Cons(cons, span) = sexpr {
        return Ok(SExpr::Cons(
            Cons::new(op(&cons.car)?, try_map(&cons.cdr, op)?),
            *span,
        ));
    }
    Ok(sexpr.clone())
}
