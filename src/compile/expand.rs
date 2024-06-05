use super::{bindings::Bindings, syntax::SExpr};

fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.coerce_to_syntax().add_scope(Bindings::CORE_SCOPE)
}

pub fn expand(sexpr: &SExpr, bindings: &Bindings) -> SExpr {
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
            sexpr!(
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
            )
        );
    }
}
