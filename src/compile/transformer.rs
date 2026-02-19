use std::collections::{HashMap, HashSet};

use crate::{
    compile::{
        compilation_error::{CompilationError, Result},
        util::try_for_each,
    },
    if_let_sexpr,
};

use super::{
    bindings::Bindings,
    sexpr::{Cons, Id, SExpr, Symbol},
    span::Span,
};

// TODO:
//
// The technique used at https://youtu.be/Or_yKiI3Ha4?si desugars all IDs into gensym'ed values
// before evaluating the transformer. But it does not concern itself with the transformer spec
// capturing a free reference. I.e.
// (let ((x 1))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           ((_) x))))
//     (let ((x 2))
//       (make-thing))))
// ==> 1
//
// According to r5rs:
// If a macro transformer inserts a free reference to an identifier, the reference refers to the
// binding that was visible where the transformer was specified, regardless of any local bindings
// that may surround the use of the macro.
//
// I am thinking since our syntax transformer is quite limited, and we're not using a full blown
// evaluator, we can get away with not using the deguaring step.
// So the tranformation look something like so:
//
// (let ((x 1))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           ((_) x))))
//     (let ((x 2))
//       (make-thing))))
//
// Add scopes:
// (let (({x: 1} 1))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({_:1, 2}) {x: 1, 2}))))
//     (let (({x: 1, 3} 2))
//       (make-thing))))
//
// Evaluate the transformer:
// (let (({x: 1} 1))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({_:1, 2}) {x: 1, 2}))))
//     (let (({x: 1, 3} 2))
//       {x: 1})))
//
// I guess one thing I am not sure if this can work is whether we will ever run into a case like:
// (let (({x: 1} 1))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({x:1, 3}) {x: 1, 2, 3, 4}))))
//     (let (({x: 1, 5} 2))
//       {x: 1})))
//
// Where {x: 1, 2, 4} is the result of expanding another macro (i.e. the original macro captures
// 1 and 2, we assign 4 during expansion).
// If the substitution is valid, then we must lower the input of the transformer into gensyms.
// Although I don't think this is possible because it doesn't seem to be a way to evaluate inside
// the syntax rule and are equavlent to a quote-syntax of some sort.
//
// Hmmm, I manage to create some examples here:
// (define x 1)
//
// (letrec-syntax
//     ((just-x
//       (syntax-rules ()
//         ((_) x))))
//   (letrec-syntax
//       ((make-thing
//         (syntax-rules ()
//           ((_ y) (just-x)))))
//     (let ((x 2))
//       (make-thing 0))))
// ==> 1
//
// (define x 1)
//
// (letrec-syntax
//     ((just-x
//       (syntax-rules ()
//         ((_) {x: 1}))))
//   (letrec-syntax
//       ((quote-thing
//         (syntax-rules ()
//           ((_ x) (just-x)))))
//     (quote-thing 10)))
//
// ==> (quote-thing 10)
// ==> (just-x)
// ==> {x: 1}
//
//
// (letrec-syntax
//     ((outer
//       (syntax-rules ()
//         ((_ {x: 1, 2})
//          (letrec-syntax
//              ((quote-thing
//                (syntax-rules ()
//                  ((_) {x: 1, 2, 3}))))
//            (quote-thing))))))
//     (outer 10))
// ==> (outer 10)
// ==> (letrec-syntax
//         ((quote-thing
//           (syntax-rules ()
//             ((_) x))))
//       (quote-thing))
// ==> {x: 1, 2, 3}
//

struct SyntaxRule {
    pattern: SExpr,
    template: SExpr,
}

pub(crate) struct Transformer {
    literals: HashSet<Symbol>,
    syntax_rules: Vec<SyntaxRule>,
}

impl SyntaxRule {
    fn match_pattern(
        &self,
        sexpr: &SExpr,
        literals: &HashSet<Symbol>,
        bindings: &Bindings,
    ) -> Option<HashMap<Id, SExpr>> {
        fn _match_pattern(
            literals: &HashSet<Symbol>,
            resolver: &Bindings,
            pattern: &SExpr,
            sexpr: &SExpr,
            matches: &mut HashMap<Id, SExpr>,
        ) -> Option<()> {
            match pattern {
                SExpr::Id(pattern, _) => {
                    if literals.contains(&pattern.symbol) {
                        let SExpr::Id(id, _) = sexpr else {
                            return None;
                        };
                        match (resolver.resolve_sym(pattern), resolver.resolve_sym(id)) {
                            (Some(p), Some(i)) => (p == i).then_some(()),
                            (None, None) => (pattern.symbol == id.symbol).then_some(()),
                            _ => None,
                        }
                    } else {
                        matches.insert(pattern.clone(), sexpr.clone());
                        Some(())
                    }
                }
                SExpr::Cons(pattern, _) => {
                    match pattern.car.as_ref() {
                        SExpr::Id(id, _) if id.symbol.0 == "..." => {
                            matches.insert(id.clone(), sexpr.clone());
                        }
                        _ => {
                            let SExpr::Cons(cons, _) = sexpr else {
                                return None;
                            };
                            _match_pattern(literals, resolver, &pattern.car, &cons.car, matches)?;
                            _match_pattern(literals, resolver, &pattern.cdr, &cons.cdr, matches)?;
                        }
                    }
                    Some(())
                }
                _ => (pattern == sexpr).then_some(()),
            }
        }

        let mut matches = HashMap::<Id, SExpr>::new();
        _match_pattern(literals, bindings, &self.pattern, sexpr, &mut matches).map(|_| matches)
    }

    fn render_template(&self, matches: &HashMap<Id, SExpr>, application_span: Span) -> SExpr {
        fn _render_template(
            template: &SExpr,
            matches: &HashMap<Id, SExpr>,
            application_span: Span,
        ) -> SExpr {
            match template {
                SExpr::Id(pattern, _) => matches
                    .get(pattern)
                    .unwrap_or(&template.update_span(application_span))
                    .clone(),
                SExpr::Cons(pattern, _) => match pattern.car.as_ref() {
                    SExpr::Id(id, _) if id.symbol.0 == "..." => matches.get(id).unwrap().clone(),
                    _ => SExpr::Cons(
                        Cons::new(
                            _render_template(&pattern.car, matches, application_span),
                            _render_template(&pattern.cdr, matches, application_span),
                        ),
                        application_span,
                    ),
                },
                _ => template.update_span(application_span),
            }
        }

        _render_template(&self.template, matches, application_span)
    }

    pub(crate) fn apply(
        &self,
        application: &SExpr,
        literals: &HashSet<Symbol>,
        resolver: &Bindings,
    ) -> Option<SExpr> {
        let bindings = self.match_pattern(application, literals, resolver)?;
        Some(self.render_template(&bindings, application.get_span()))
    }
}

impl Transformer {
    pub(crate) fn new(spec: &SExpr) -> Result<Self> {
        if_let_sexpr! {(_, (literals_list @ ..), rules @ ..) = spec =>
            let mut literals = HashSet::<Symbol>::new();
            try_for_each(
                |literal| {
                    if let SExpr::Id(Id { symbol, scopes: _ }, _) = literal {
                        literals.insert(symbol.clone());
                        Ok(())
                    } else {
                        Err(CompilationError {
                            span: literal.get_span(),
                            reason: format!(
                                "Expected symbols in syntax transformer literals, but got: {}",
                                literal
                            ),
                        })
                    }
                },
                literals_list,
            )?;

            let mut syntax_rules = Vec::<SyntaxRule>::new();
            try_for_each(
                |rule_pair| {
                    if_let_sexpr! {(pattern, template) = rule_pair =>
                        syntax_rules.push(SyntaxRule { pattern: pattern.clone(), template: template.clone() });
                        return Ok(());
                    }
                    Err(CompilationError {
                        span: rule_pair.get_span(),
                        reason: "Unrecognized syntax for syntax transformer rule pair".to_owned(),
                    })
                },
                rules,
            )?;

            return Ok(Self { literals, syntax_rules })
        }
        Err(CompilationError {
            span: spec.get_span(),
            reason: "Unrecognized syntax for syntax transformer".to_owned(),
        })
    }

    pub(crate) fn transform(&self, application: &SExpr, bindings: &Bindings) -> Option<SExpr> {
        self.syntax_rules
            .iter()
            .filter_map(|syntax_rule| syntax_rule.apply(application, &self.literals, bindings))
            .next()
    }
}

#[cfg(test)]
mod tests {
    use crate::compile::{
        bindings::Bindings,
        expand::introduce,
        lex::tokenize,
        parse::parse,
        sexpr::{Bool, Num},
        span::Span,
    };

    use super::*;

    #[test]
    fn test_transformer_improper_list_in_template() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    "
                    (syntax-rules ()
                        ((_ a) (1 2 . a)))",
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        let src = "
        (
          mac 3
        )";
        let callsite_src_loc = Span { lo: 9, hi: 36 };

        let result = transformer
            .transform(
                &introduce(&parse(&tokenize(src).unwrap()).unwrap()),
                &Bindings::new(),
            )
            .unwrap();
        let expected = &SExpr::Cons(
            Cons::new(
                SExpr::Num(Num(1.0), callsite_src_loc),
                SExpr::Cons(
                    Cons::new(
                        SExpr::Num(Num(2.0), callsite_src_loc),
                        SExpr::Num(Num(3.0), Span { lo: 25, hi: 26 }),
                    ),
                    callsite_src_loc,
                ),
            ),
            callsite_src_loc,
        );

        assert!(
            result.is_idential(expected),
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }

    #[test]
    fn test_and_transformer_literal_in_template() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    "
                    (syntax-rules ()
                      ((_) #f))
                ",
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        let result = transformer
            .transform(
                &introduce(&parse(&tokenize("(and)").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .unwrap();
        let expected = &SExpr::Bool(Bool(false), Span { lo: 0, hi: 5 });

        assert!(
            result.is_idential(expected),
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }

    #[test]
    fn test_and_transformer_literal_in_pattern() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    "
                    (syntax-rules ()
                      ((_ 1 x) x))
                ",
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        let result = transformer
            .transform(
                &introduce(&parse(&tokenize("(macro 1 a)").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .unwrap();
        let expected = SExpr::Id(Id::new("a", [0]), Span { lo: 9, hi: 10 });

        assert!(
            result.is_idential(&expected),
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }

    #[test]
    fn test_and_transformer_id() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    r#"
                    (syntax-rules ()
                      ((_ e) e))
                "#,
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        let result = transformer
            .transform(
                &introduce(&parse(&tokenize("(and x)").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .unwrap();
        let expected = introduce(&SExpr::Id(Id::new("x", []), Span { lo: 5, hi: 6 }));

        assert!(
            result.is_idential(&expected),
            "result: {:?}\nexpected: {:?}",
            result,
            expected
        );
    }

    #[test]
    fn test_and_transformer_recursive_case() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    r#"
                    (syntax-rules ()
                      ((_ e1 e2 ...) (if e1 (and e2 ...) #f)))
                "#,
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b c)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b c d)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c d) #f)").unwrap()).unwrap())
        );
    }

    #[test]
    fn test_and_transformer() {
        let transformer = Transformer::new(&introduce(
            &parse(
                &tokenize(
                    r#"
                    (syntax-rules ()
                      ((_) #f)
                      ((_ e) e)
                      ((_ e1 e2 ...)
                       (if e1 (and e2 ...) #f)))
                "#,
                )
                .unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();

        assert!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap()
                .is_idential(&SExpr::Bool(Bool(false), Span { lo: 0, hi: 5 }))
        );
        assert!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and x)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap()
                .is_idential(&introduce(&SExpr::Id(
                    Id::new("x", []),
                    Span { lo: 5, hi: 6 }
                )))
        );
        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b c)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(
                    &introduce(&parse(&tokenize("(and a b c d)").unwrap()).unwrap()),
                    &Bindings::new(),
                )
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c d) #f)").unwrap()).unwrap())
        );
    }
}
