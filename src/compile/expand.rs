use crate::match_sexpr;

use super::{
    bindings::Bindings,
    syntax::{Id, SExpr, Symbol},
};

fn first(sexpr: &SExpr) -> Option<SExpr> {
    match sexpr {
        SExpr::Cons(cons) => Some(cons.car.clone()),
        _ => None,
    }
}

fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.coerce_to_syntax().add_scope(Bindings::CORE_SCOPE)
}

pub fn expand(sexpr: &SExpr, bindings: &Bindings) -> Option<SExpr> {
    match_sexpr!(
        sexpr,
        SExpr::Id(id) => {
            return expand_id(id, bindings);
        };
        (SExpr::Id(_), ..) => {
            return expand_id_application(sexpr, bindings);
        };
        (..) => {
            return expand_application(sexpr, bindings);
        };
        SExpr::Symbol(_) | SExpr::Nil => {
            return None;
        };
        _ => {
            return Some(sexpr.clone());
        };
    );
}

fn expand_id(id: &Id, bindings: &Bindings) -> Option<SExpr> {
    bindings.resolve(id).map(|id| SExpr::Id(id))
}

fn expand_id_application(sexpr: &SExpr, bindings: &Bindings) -> Option<SExpr> {
    let binding = match first(sexpr)? {
        SExpr::Id(id) => bindings.resolve(&id),
        _ => unreachable!(),
    }?;
    match binding {
        Id {
            symbol: Symbol(symbol),
            scopes: _,
        } => match symbol.as_str() {
            "lambda" => expand_lambda(sexpr, bindings),
            "let-syntax" => expand_let_syntax(sexpr, bindings),
            "quote" | "quote-syntax" => Some(sexpr.clone()),
            _ => {
                // TODO: check if this is a macro via some table, if so, apply the macro and expand the result
                expand_application(sexpr, bindings)
            }
        },
    };
    None
}

fn expand_application(sexpr: &SExpr, bindings: &Bindings) -> Option<SExpr> {
    todo!()
}

fn expand_lambda(sexpr: &SExpr, bindings: &Bindings) -> Option<SExpr> {
    todo!()
}

fn expand_let_syntax(sexpr: &SExpr, bindings: &Bindings) -> Option<SExpr> {
    todo!()
}

#[cfg(test)]
mod tests {

    use crate::sexpr;

    use super::*;

    #[test]
    fn test_introduce() {
        let list = sexpr!(
            SExpr::new_symbol("cons"),
            SExpr::new_num(0),
            SExpr::new_num(1)
        );
        assert_eq!(
            introduce(&list),
            sexpr!(
                SExpr::new_id_with_scope("cons", [Bindings::CORE_SCOPE]),
                SExpr::new_num(0),
                SExpr::new_num(1)
            )
        );
    }

    #[test]
    fn test_expand_lambda() {
        let bindings = Bindings::new();
        let lambda_expr = sexpr!(
            SExpr::new_symbol("lambda"),
            (SExpr::new_symbol("x"), SExpr::new_symbol("y")),
            (
                SExpr::new_symbol("+"),
                SExpr::new_symbol("x"),
                SExpr::new_symbol("y")
            ),
        );
        assert_eq!(
            expand(&introduce(&lambda_expr.coerce_to_syntax()), &bindings),
            Some(sexpr!(
                SExpr::new_id_with_scope("lambda", [Bindings::CORE_SCOPE]),
                (
                    SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
                    SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1])
                ),
                (
                    SExpr::new_id_with_scope("+", [Bindings::CORE_SCOPE]),
                    SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
                    SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1])
                ),
            ))
        );
    }
}
