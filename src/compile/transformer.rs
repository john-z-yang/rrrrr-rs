use std::collections::{HashMap, HashSet};

use crate::{compile::util::for_each, match_sexpr};

use super::{
    sexpr::{Cons, Id, SExpr, Symbol},
    source_loc::SourceLoc,
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
        literals: &HashSet<Symbol>,
        sexpr: &SExpr,
    ) -> Option<HashMap<Id, SExpr>> {
        fn _match_pattern(
            literals: &HashSet<Symbol>,
            pattern: &SExpr,
            sexpr: &SExpr,
            bindings: &mut HashMap<Id, SExpr>,
        ) -> Option<()> {
            match pattern {
                SExpr::Id(pattern, _) => {
                    if literals.contains(&pattern.symbol) {
                        let SExpr::Id(Id { symbol, scopes: _ }, _) = sexpr else {
                            return None;
                        };
                        (pattern.symbol == *symbol).then_some(())
                    } else {
                        bindings.insert(pattern.clone(), sexpr.clone());
                        Some(())
                    }
                }
                SExpr::Cons(pattern, _) => {
                    match pattern.car.as_ref() {
                        SExpr::Id(id, _) if id.symbol.0 == "..." => {
                            bindings.insert(id.clone(), sexpr.clone());
                        }
                        _ => {
                            let SExpr::Cons(cons, _) = sexpr else {
                                return None;
                            };
                            _match_pattern(literals, &pattern.car, &cons.car, bindings)?;
                            _match_pattern(literals, &pattern.cdr, &cons.cdr, bindings)?;
                        }
                    }
                    Some(())
                }
                _ => (pattern == sexpr).then_some(()),
            }
        }

        let mut bindings = HashMap::<Id, SExpr>::new();
        _match_pattern(literals, &self.pattern, sexpr, &mut bindings).map(|_| bindings)
    }

    fn render_template(
        &self,
        bindings: &HashMap<Id, SExpr>,
        application_source_loc: SourceLoc,
    ) -> SExpr {
        fn _render_template(
            template: &SExpr,
            bindings: &HashMap<Id, SExpr>,
            application_source_loc: SourceLoc,
        ) -> SExpr {
            match template {
                SExpr::Id(pattern, _) => bindings
                    .get(pattern)
                    .unwrap_or(&template.update_source_loc(application_source_loc))
                    .clone(),
                SExpr::Cons(pattern, _) => match pattern.car.as_ref() {
                    SExpr::Id(id, _) if id.symbol.0 == "..." => bindings.get(id).unwrap().clone(),
                    _ => SExpr::Cons(
                        Cons::new(
                            _render_template(&pattern.car, bindings, application_source_loc),
                            _render_template(&pattern.cdr, bindings, application_source_loc),
                        ),
                        application_source_loc,
                    ),
                },
                _ => template.update_source_loc(application_source_loc),
            }
        }

        _render_template(&self.template, bindings, application_source_loc)
    }

    pub(crate) fn apply(&self, literals: &HashSet<Symbol>, application: &SExpr) -> Option<SExpr> {
        let bindings = self.match_pattern(literals, application)?;
        Some(self.render_template(&bindings, application.get_source_loc()))
    }
}

impl Transformer {
    pub(crate) fn new(spec: &SExpr) -> Self {
        match_sexpr! {(#"syntax-rules", (literals_list @ ..), rules @ ..) = spec =>
            let mut literals = HashSet::<Symbol>::new();
            for_each(|literal| {
                if let SExpr::Id(Id { symbol, scopes: _ }, _) = literal{
                    literals.insert(symbol.clone());
                } else {
                    unreachable!("Expected symbols in syntax transformer literals");
                }
            }, literals_list);

            let mut syntax_rules = Vec::<SyntaxRule>::new();
            for_each(|rule_pair| {
                match_sexpr! {(pattern, template) = rule_pair =>
                    syntax_rules.push(SyntaxRule { pattern: pattern.clone(), template: template.clone() });
                }
            }, rules);

            return Self { literals, syntax_rules }
        }
        unreachable!("Unrecognized syntax for syntax transformer")
    }

    pub(crate) fn transform(&self, application: &SExpr) -> Option<SExpr> {
        self.syntax_rules
            .iter()
            .filter_map(|syntax_rule| syntax_rule.apply(&self.literals, application))
            .next()
    }
}

#[cfg(test)]
mod tests {
    use crate::compile::{
        expand::introduce,
        lex::tokenize,
        parse::parse,
        sexpr::{Bool, Num},
        source_loc::SourceLoc,
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
        ));

        let src = "
        (
          mac 3
        )";
        let callsite_src_loc = SourceLoc {
            line: 1,
            idx: 9,
            width: 27,
        };

        let result = transformer
            .transform(&introduce(&parse(&tokenize(src).unwrap()).unwrap()))
            .unwrap();
        let expected = &SExpr::Cons(
            Cons::new(
                SExpr::Num(Num(1.0), callsite_src_loc),
                SExpr::Cons(
                    Cons::new(
                        SExpr::Num(Num(2.0), callsite_src_loc),
                        SExpr::Num(
                            Num(3.0),
                            SourceLoc {
                                line: 2,
                                idx: 25,
                                width: 1,
                            },
                        ),
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
        ));

        let result = transformer
            .transform(&introduce(&parse(&tokenize("(and)").unwrap()).unwrap()))
            .unwrap();
        let expected = &SExpr::Bool(
            Bool(false),
            SourceLoc {
                line: 0,
                idx: 0,
                width: 5,
            },
        );

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
        ));

        let result = transformer
            .transform(&introduce(
                &parse(&tokenize("(macro 1 a)").unwrap()).unwrap(),
            ))
            .unwrap();
        let expected = SExpr::Id(
            Id::new("a", [0]),
            SourceLoc {
                line: 0,
                idx: 9,
                width: 1,
            },
        );

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
        ));

        let result = transformer
            .transform(&introduce(&parse(&tokenize("(and x)").unwrap()).unwrap()))
            .unwrap();
        let expected = introduce(&SExpr::Id(
            Id::new("x", []),
            SourceLoc {
                line: 0,
                idx: 5,
                width: 1,
            },
        ));

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
        ));

        assert_eq!(
            transformer
                .transform(&introduce(&parse(&tokenize("(and a b)").unwrap()).unwrap()))
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(&introduce(
                    &parse(&tokenize("(and a b c)").unwrap()).unwrap()
                ))
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(&introduce(
                    &parse(&tokenize("(and a b c d)").unwrap()).unwrap()
                ))
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
        ));

        assert!(transformer
            .transform(&introduce(&parse(&tokenize("(and)").unwrap()).unwrap()))
            .unwrap()
            .is_idential(&SExpr::Bool(
                Bool(false),
                SourceLoc {
                    line: 0,
                    idx: 0,
                    width: 5
                }
            )));
        assert!(transformer
            .transform(&introduce(&parse(&tokenize("(and x)").unwrap()).unwrap()))
            .unwrap()
            .is_idential(&introduce(&SExpr::Id(
                Id::new("x", []),
                SourceLoc {
                    line: 0,
                    idx: 5,
                    width: 1
                }
            ))));
        assert_eq!(
            transformer
                .transform(&introduce(&parse(&tokenize("(and a b)").unwrap()).unwrap()))
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(&introduce(
                    &parse(&tokenize("(and a b c)").unwrap()).unwrap()
                ))
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c) #f)").unwrap()).unwrap())
        );
        assert_eq!(
            transformer
                .transform(&introduce(
                    &parse(&tokenize("(and a b c d)").unwrap()).unwrap()
                ))
                .unwrap(),
            introduce(&parse(&tokenize("(if a (and b c d) #f)").unwrap()).unwrap())
        );
    }
}
