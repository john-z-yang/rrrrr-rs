use std::collections::HashMap;

use super::{
    bindings::Bindings,
    syntax::{Id, SExpr, Symbol},
    transformer::Transformer,
    util::{first, map},
};
use crate::{compile::util::for_each, match_sexpr, sexpr};

type Env = HashMap<Symbol, Transformer>;

fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.coerce_to_syntax().add_scope(Bindings::CORE_SCOPE)
}

fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    if let SExpr::Symbol(_) | SExpr::Nil = sexpr {
        panic!("Bad syntax");
    };
    if let SExpr::Id(id) = sexpr {
        return expand_id(id, bindings);
    }
    match_sexpr! {(SExpr::Id(_), ..) = sexpr =>
        return expand_id_application(sexpr, bindings, env);
    };
    match_sexpr! {(..) = sexpr =>
        return expand_fn_application(sexpr, bindings, env);
    };
    sexpr.clone()
}

fn expand_id(id: &Id, bindings: &mut Bindings) -> SExpr {
    bindings.resolve(id).map(SExpr::Id).unwrap();
    SExpr::Id(id.clone())
}

fn expand_id_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    let binding = match first(sexpr) {
        SExpr::Id(id) => bindings.resolve(&id).unwrap(),
        _ => unreachable!("ID must have a binding during expansion of ID application"),
    };

    match binding.symbol.0.as_str() {
        "quote" | "quote-syntax" => sexpr.clone(),
        "let-syntax" => expand_let_syntax(sexpr, bindings, env),
        "lambda" => expand_lambda(sexpr, bindings, env),
        _ => {
            if let Some(transformer) = env.get(&binding.symbol) {
                let scope_id = bindings.new_scope_id();
                let sexpr = sexpr.add_scope(scope_id);
                let transformed_sexpr = transformer.transform(&sexpr).unwrap();
                expand(&transformed_sexpr.flip_scope(scope_id), bindings, env)
            } else {
                expand_fn_application(sexpr, bindings, env)
            }
        }
    }
}

fn expand_fn_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    map(|sub_sexpr| expand(sub_sexpr, bindings, env), sexpr)
}

fn expand_lambda(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    match_sexpr! {(lambda, (args @ ..), body @ ..) = sexpr =>
        let scope_id = bindings.new_scope_id();
        let args = args.add_scope(scope_id);

        for_each(|arg| {
            if let SExpr::Id(id) = arg{
                let binding = bindings.gen_sym();
                bindings.add_binding(id, &binding);
            } else {
                unreachable!("Expected identifiers in function parameters");
            }
        }, &args);

        let body = map(|sexpr| expand(&sexpr.add_scope(scope_id), bindings, env), body);
        return sexpr!(lambda.clone(), args, ..body);
    };
    unreachable!("Invalid use of lambda form: {}", sexpr);
}

fn expand_let_syntax(_sexpr: &SExpr, _bindings: &mut Bindings, _env: &mut Env) -> SExpr {
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
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(S(lambda), (S(x), S(y)), (S(cons), S(x), S(y)),);
        let left = expand(
            &introduce(&lambda_expr.coerce_to_syntax()),
            &mut bindings,
            &mut env,
        );
        let right = sexpr!(
            SExpr::new_id_with_scope("lambda", [Bindings::CORE_SCOPE]),
            (
                SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1])
            ),
            (
                SExpr::new_id_with_scope("cons", [Bindings::CORE_SCOPE, 1]),
                SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1])
            ),
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_expand_lambda_recursive() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(
            S(lambda),
            (S(x)),
            (S(lambda), (S(y)), (S(cons), S(x), S(y))),
            (S(cons), S(x), S(x))
        );
        let left = expand(
            &introduce(&lambda_expr.coerce_to_syntax()),
            &mut bindings,
            &mut env,
        );
        let right = sexpr!(
            SExpr::new_id_with_scope("lambda", [Bindings::CORE_SCOPE]),
            (SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1])),
            (
                SExpr::new_id_with_scope("lambda", [Bindings::CORE_SCOPE, 1]),
                (SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1, 2]),),
                (
                    SExpr::new_id_with_scope("cons", [Bindings::CORE_SCOPE, 1, 2]),
                    SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1, 2]),
                    SExpr::new_id_with_scope("y", [Bindings::CORE_SCOPE, 1, 2]),
                )
            ),
            (
                SExpr::new_id_with_scope("cons", [Bindings::CORE_SCOPE, 1]),
                SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::new_id_with_scope("x", [Bindings::CORE_SCOPE, 1]),
            )
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_expand_atoms() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(SExpr::new_bool(false));
        assert_eq!(
            expand(
                &introduce(&lambda_expr.coerce_to_syntax()),
                &mut bindings,
                &mut env
            ),
            sexpr!(SExpr::new_bool(false))
        );
    }
}
