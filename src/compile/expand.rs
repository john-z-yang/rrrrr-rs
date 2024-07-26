use std::collections::HashMap;

use super::{
    bindings::Bindings,
    sexpr::{Id, SExpr, Symbol},
    transformer::Transformer,
    util::{first, map},
};
use crate::{compile::util::for_each, match_sexpr, sexpr};

type Env = HashMap<Symbol, Transformer>;

pub fn introduce(sexpr: &SExpr) -> SExpr {
    sexpr.add_scope(Bindings::CORE_SCOPE)
}

pub fn expand(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    if let SExpr::Nil = sexpr {
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
    assert!(bindings.resolve(id).is_some(), "ID must have a binding");
    SExpr::Id(id.clone())
}

fn expand_id_application(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    let binding = match first(sexpr) {
        Some(SExpr::Id(id)) => bindings.resolve(&id).unwrap(),
        _ => unreachable!("first element of ID application must be an ID"),
    };

    match binding.0.as_str() {
        "quote" | "quote-syntax" => sexpr.clone(),
        "letrec-syntax" => expand_letrec_syntax(sexpr, bindings, env),
        "lambda" => expand_lambda(sexpr, bindings, env),
        _ => {
            if let Some(transformer) = env.get(&binding) {
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
            let SExpr::Id(id) = arg else {
                unreachable!("Expected identifiers in function parameters");
            };
            let binding = bindings.gen_sym();
            bindings.add_binding(id, &binding);
        }, &args);

        let body = map(|sexpr| expand(&sexpr.add_scope(scope_id), bindings, env), body);
        return sexpr!(lambda.clone(), args, ..body);
    };
    unreachable!("Invalid use of lambda form: {}", sexpr);
}

fn expand_letrec_syntax(sexpr: &SExpr, bindings: &mut Bindings, env: &mut Env) -> SExpr {
    match_sexpr! {(#"letrec-syntax", ((keyword, transformer_spec)), body) = sexpr =>
        let scope_id = bindings.new_scope_id();
        let keyword = keyword.add_scope(scope_id);

        let SExpr::Id(id) = keyword else {
            unreachable!("Expected identifiers in syntax keyword");
        };
        let binding = bindings.gen_sym();
        bindings.add_binding(&id, &binding);

        let transformer = Transformer::new(&transformer_spec.add_scope(scope_id));
        env.insert(binding.clone(), transformer);

        let res = expand(&body.add_scope(scope_id), bindings, env);
        env.remove_entry(&binding);

        return res;
    }
    unreachable!("Invalid use of let_syntax form: {}", sexpr);
}

#[cfg(test)]
mod tests {

    use crate::{
        compile::util::{last, nth},
        sexpr,
    };

    use super::*;

    #[test]
    fn test_introduce() {
        let list = sexpr!(SExpr::id("cons", []), SExpr::num(0.0), SExpr::num(1.0));
        assert_eq!(
            introduce(&list),
            sexpr!(
                SExpr::id("cons", [Bindings::CORE_SCOPE]),
                SExpr::num(0.0),
                SExpr::num(1.0),
            )
        );
    }

    #[test]
    fn test_expand_lambda() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(#"lambda", (#"x", #"y"), (#"cons", #"x", #"y"),);
        let left = expand(&introduce(&lambda_expr), &mut bindings, &mut env);
        let right = sexpr!(
            SExpr::id("lambda", [Bindings::CORE_SCOPE]),
            (
                SExpr::id("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::id("y", [Bindings::CORE_SCOPE, 1]),
            ),
            (
                SExpr::id("cons", [Bindings::CORE_SCOPE, 1]),
                SExpr::id("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::id("y", [Bindings::CORE_SCOPE, 1]),
            ),
        );
        assert_eq!(left, right);
    }

    #[test]
    fn test_expand_lambda_recursive() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(
            #"lambda",
            (#"x"),
            (#"lambda", (#"y"), (#"cons", #"x", #"y")),
            (#"cons", #"x", #"x")
        );
        let result = expand(&introduce(&lambda_expr), &mut bindings, &mut env);
        let expected = sexpr!(
            SExpr::id("lambda", [Bindings::CORE_SCOPE]),
            (SExpr::id("x", [Bindings::CORE_SCOPE, 1])),
            (
                SExpr::id("lambda", [Bindings::CORE_SCOPE, 1]),
                (SExpr::id("y", [Bindings::CORE_SCOPE, 1, 2])),
                (
                    SExpr::id("cons", [Bindings::CORE_SCOPE, 1, 2]),
                    SExpr::id("x", [Bindings::CORE_SCOPE, 1, 2]),
                    SExpr::id("y", [Bindings::CORE_SCOPE, 1, 2]),
                )
            ),
            (
                SExpr::id("cons", [Bindings::CORE_SCOPE, 1]),
                SExpr::id("x", [Bindings::CORE_SCOPE, 1]),
                SExpr::id("x", [Bindings::CORE_SCOPE, 1]),
            ),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_atoms() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let lambda_expr = sexpr!(SExpr::bool(false));
        assert_eq!(
            expand(&introduce(&lambda_expr), &mut bindings, &mut env),
            sexpr!(SExpr::bool(false))
        );
    }

    #[test]
    fn test_expand_and_macro_0_arg() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_"), SExpr::bool(false)),
            ((#"_", #"e"), #"e"),
            ((#"_", #"e1", #"e2", #"..."),
             (#"if", #"e1",
                     (#"and", #"e2", #"..."),
                     SExpr::bool(false))),
        )));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = sexpr!(#"and");
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);
        let expected = SExpr::bool(false);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_1_arg() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_"), SExpr::bool(false)),
            ((#"_", #"e"), #"e"),
            ((#"_", #"e1", #"e2", #"..."),
             (#"if", #"e1",
                     (#"and", #"e2", #"..."),
                     SExpr::bool(false))),
        )));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = introduce(&sexpr!(#"and", #"list"));
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);
        let expected = SExpr::id("list", [Bindings::CORE_SCOPE]);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_2_args() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_"), SExpr::bool(false)),
            ((#"_", #"e"), #"e"),
            ((#"_", #"e1", #"e2", #"..."),
             (#"if", #"e1",
                     (#"and", #"e2", #"..."),
                     SExpr::bool(false))),
        )));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = sexpr!(#"and", #"list", #"list");
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);
        let expected = sexpr!(
            SExpr::id("if", [Bindings::CORE_SCOPE, 1]),
            SExpr::id("list", [Bindings::CORE_SCOPE]),
            SExpr::id("list", [Bindings::CORE_SCOPE]),
            SExpr::bool(false),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_and_macro_4_args() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("and", [Bindings::CORE_SCOPE]), &Symbol::new("and"));

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_"), SExpr::bool(false)),
            ((#"_", #"e"), #"e"),
            ((#"_", #"e1", #"e2", #"..."),
             (#"if", #"e1",
                     (#"and", #"e2", #"..."),
                     SExpr::bool(false))),
        )));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("and", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = sexpr!(
            #"and",
            SExpr::bool(true),
            SExpr::bool(true),
            SExpr::bool(true),
            SExpr::bool(true),
        );
        // (and t t t t)
        // (if t (and t t t) f)
        // (if t (if t (and t t) f) f)
        // (if t (if t (if t (and t) f) f) f)
        // (if t (if t (if t t f) f) f) f)
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);
        let expected = sexpr!(
            SExpr::id("if", [Bindings::CORE_SCOPE, 1]),
            SExpr::bool(true),
            (
                SExpr::id("if", [Bindings::CORE_SCOPE, 2]),
                SExpr::bool(true),
                (
                    SExpr::id("if", [Bindings::CORE_SCOPE, 3]),
                    SExpr::bool(true),
                    SExpr::bool(true),
                    SExpr::bool(false),
                ),
                SExpr::bool(false),
            ),
            SExpr::bool(false),
        );
        assert_eq!(result, expected);
        assert_eq!(
            bindings
                .resolve(&(first(&result).unwrap().try_into().unwrap()))
                .unwrap(),
            Symbol::new("if")
        );
    }

    #[test]
    fn test_expand_simple_macro_hygiene() {
        let mut bindings = Bindings::new();

        bindings.add_binding(&Id::new("x", [Bindings::CORE_SCOPE]), &Symbol::new("x"));
        bindings.add_binding(
            &Id::new("my-macro", [Bindings::CORE_SCOPE]),
            &Symbol::new("my-macro"),
        );

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_", #"body"), (#"lambda", (#"x"), #"body")),
        )));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("my-macro", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = sexpr!(#"my-macro", #"x");
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);
        let expected = sexpr!(
            SExpr::id("lambda", [Bindings::CORE_SCOPE, 1]),
            (SExpr::id("x", [Bindings::CORE_SCOPE, 1, 2])),
            SExpr::id("x", [Bindings::CORE_SCOPE, 2]),
        );
        assert_eq!(result, expected);
        assert_ne!(
            bindings
                .resolve(
                    &first(&nth(&result, 1).unwrap())
                        .unwrap()
                        .try_into()
                        .unwrap()
                )
                .unwrap(),
            bindings
                .resolve(&last(&result).unwrap().try_into().unwrap())
                .unwrap(),
        );
        assert_eq!(
            bindings
                .resolve(&Id::new("x", [Bindings::CORE_SCOPE]))
                .unwrap(),
            bindings
                .resolve(&last(&result).unwrap().try_into().unwrap())
                .unwrap(),
        )
    }

    #[test]
    fn test_expand_or_macro_hygiene() {
        let mut bindings = Bindings::new();

        bindings.add_binding(
            &Id::new("my-or", [Bindings::CORE_SCOPE]),
            &Symbol::new("my-or"),
        );

        let transformer = Transformer::new(&introduce(&sexpr!(
            #"syntax-rules",
            (),
            ((#"_"), SExpr::bool(false)),
            ((#"_", #"e"), #"e"),
            ((#"_", #"e1", #"e2", #"..."),
             ((#"lambda", (#"temp"),
                (#"if", #"temp", #"temp", (#"my-or", #"e2", #"..."))), #"e1"),
        ))));

        let mut env = HashMap::from([(
            bindings
                .resolve(&Id::new("my-or", [Bindings::CORE_SCOPE]))
                .unwrap(),
            transformer,
        )]);

        let sexpr = sexpr!(
            (
                #"lambda",
                (#"temp"),
                (#"my-or", SExpr::bool(false), #"temp")
            ),
            SExpr::bool(true),
        );
        let result = expand(&introduce(&sexpr), &mut bindings, &mut env);

        let expected = sexpr!(
            (
                SExpr::id("lambda", [Bindings::CORE_SCOPE]),
                (SExpr::id("temp", [Bindings::CORE_SCOPE, 1])),
                (
                    (
                        SExpr::id("lambda", [Bindings::CORE_SCOPE, 2]),
                        (SExpr::id("temp", [Bindings::CORE_SCOPE, 2, 3])),
                        (
                            SExpr::id("if", [Bindings::CORE_SCOPE, 2, 3]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 0, 2, 3]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 0, 2, 3]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 3]),
                        )
                    ),
                    SExpr::bool(false)
                )
            ),
            SExpr::bool(true),
        );

        assert_eq!(result, expected);

        let outer_temp_id = first(&nth(&first(&result).unwrap(), 1).unwrap()).unwrap();
        let inner_temp_id = first(
            &nth(
                &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        let if_expr = nth(
            &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
            2,
        )
        .unwrap();

        assert_ne!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_ne!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );
    }

    #[test]
    fn test_expand_let_syntax_to_num() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = sexpr!(
            #"letrec-syntax",
                ((#"one",
                    (#"syntax-rules", (),
                        ((#"_"), SExpr::num(1.0))))),
                (#"one")
        );
        let result = expand(&introduce(&let_syntax_expr), &mut bindings, &mut env);
        let expected = SExpr::num(1.0);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_expand_let_syntax_via_or_macro() {
        let mut bindings = Bindings::new();
        let mut env = HashMap::<Symbol, Transformer>::new();
        let let_syntax_expr = sexpr!(
            #"letrec-syntax",
                ((#"or",
                    (#"syntax-rules", (),
                    ((#"_"), SExpr::bool(false)),
                    ((#"_", #"e"), #"e"),
                    ((#"_", #"e1", #"e2", #"..."),
                    ((#"lambda", (#"temp"),
                        (#"if", #"temp", #"temp", (#"or", #"e2", #"..."))), #"e1"))))),
                    ((#"lambda",
                        (#"temp"),
                        (#"or", SExpr::bool(false), #"temp")),
                    SExpr::bool(true)),
        );
        let result = expand(&introduce(&let_syntax_expr), &mut bindings, &mut env);
        let expected = sexpr!(
            (
                SExpr::id("lambda", [Bindings::CORE_SCOPE, 1]),
                (SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 2])),
                (
                    (
                        SExpr::id("lambda", [Bindings::CORE_SCOPE, 1, 3]),
                        (SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 3, 4])),
                        (
                            SExpr::id("if", [Bindings::CORE_SCOPE, 1, 3, 4]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 3, 4]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 3, 4]),
                            SExpr::id("temp", [Bindings::CORE_SCOPE, 1, 2, 4])
                        )
                    ),
                    SExpr::bool(false)
                ),
            ),
            SExpr::bool(true),
        );
        assert_eq!(result, expected);

        let outer_temp_id = first(&nth(&first(&result).unwrap(), 1).unwrap()).unwrap();
        let inner_temp_id = first(
            &nth(
                &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
                1,
            )
            .unwrap(),
        )
        .unwrap();
        let if_expr = nth(
            &first(&nth(&first(&result).unwrap(), 2).unwrap()).unwrap(),
            2,
        )
        .unwrap();

        assert_ne!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_ne!(
            bindings
                .resolve(&(nth(&if_expr, 1).unwrap()).try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&inner_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 2).unwrap()).try_into().unwrap())
                .unwrap(),
        );

        assert_eq!(
            bindings
                .resolve(&outer_temp_id.clone().try_into().unwrap())
                .unwrap(),
            bindings
                .resolve(&(nth(&if_expr, 3).unwrap()).try_into().unwrap())
                .unwrap(),
        );
    }
}
