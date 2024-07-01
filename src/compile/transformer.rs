use std::collections::{HashMap, HashSet};

use crate::{compile::util::for_each, match_sexpr};

use super::syntax::{Id, SExpr, Symbol};

// TODO:
//
// The technique used at https://youtu.be/Or_yKiI3Ha4?si desugars all IDs into gensym'ed values
// before evaluating the transformer. But it does not concern itself with the transformer spec
// capturing a free reference. I.e.
// (let ((x 1))
//   (let-syntax
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
//   (let-syntax
//       ((make-thing
//         (syntax-rules ()
//           ((_) x))))
//     (let ((x 2))
//       (make-thing))))
//
// Add scopes:
// (let (({x: 1} 1))
//   (let-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({_:1, 2}) {x: 1, 2}))))
//     (let (({x: 1, 3} 2))
//       (make-thing))))
//
// Evaluate the transformer:
// (let (({x: 1} 1))
//   (let-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({_:1, 2}) {x: 1, 2}))))
//     (let (({x: 1, 3} 2))
//       {x: 1})))
//
// I guess one thing I am not sure if this can work is whether we will ever run into a case like:
// (let (({x: 1} 1))
//   (let-syntax
//       ((make-thing
//         (syntax-rules ()
//           (({x:1, 3}) {x: 1, 2, 3, 4}))))
//     (let (({x: 1, 5} 2))
//       {x: 1})))
//
// Where {x: 1, 2, 4} is the result of expanding another macro (i.e. the original macro captures
// 1 and 2, we assign 4 during expansion).
// If the substitution is valid, then we must lower the input of the transformer into gensyms.
//
// Hmmm, I manage to create some examples here:
// (define x 1)
//
// (let-syntax
//     ((just-x
//       (syntax-rules ()
//         ((_) x))))
//   (let-syntax
//       ((make-thing
//         (syntax-rules ()
//           ((_ y) (just-x)))))
//     (let ((x 2))
//       (make-thing 0))))
// ==> 1
//
// (define x 1)
//
// (let-syntax
//     ((just-x
//       (syntax-rules ()
//         ((_) {x: 1}))))
//   (let-syntax
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
// (let-syntax
//     ((outer
//       (syntax-rules ()
//         ((_ {x: 1, 2})
//          (let-syntax
//              ((quote-thing
//                (syntax-rules ()
//                  ((_) {x: 1, 2, 3}))))
//            (quote-thing))))))
//     (outer 10))
// ==> (outer 10)
// ==> (let-syntax
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

pub struct Transformer {
    literals: HashSet<Symbol>,
    syntax_rules: Vec<SyntaxRule>,
}

impl SyntaxRule {
    fn _match_pattern(
        literals: &HashSet<Symbol>,
        pattern: &SExpr,
        sexpr: &SExpr,
        bindings: &mut HashMap<Id, SExpr>,
    ) -> Option<()> {
        match pattern {
            SExpr::Id(pattern) => {
                if literals.contains(&pattern.symbol) {
                    let SExpr::Id(Id { symbol, scopes: _ }) = sexpr else {
                        return None;
                    };
                    (pattern.symbol == *symbol).then_some(())
                } else {
                    bindings.insert(pattern.clone(), sexpr.clone());
                    Some(())
                }
            }
            SExpr::Cons(pattern) => {
                match &pattern.car {
                    SExpr::Id(id) if id.symbol.0 == "..." => {
                        bindings.insert(id.clone(), sexpr.clone());
                    }
                    _ => {
                        let SExpr::Cons(cons) = sexpr else {
                            return None;
                        };
                        Self::_match_pattern(literals, &pattern.car, &cons.car, bindings)?;
                        Self::_match_pattern(literals, &pattern.cdr, &cons.cdr, bindings)?;
                    }
                }
                Some(())
            }
            _ => (pattern == sexpr).then_some(()),
        }
    }

    fn match_pattern(
        &self,
        literals: &HashSet<Symbol>,
        sexpr: &SExpr,
    ) -> Option<HashMap<Id, SExpr>> {
        let mut bindings = HashMap::<Id, SExpr>::new();
        Self::_match_pattern(literals, &self.pattern, sexpr, &mut bindings).map(|_| bindings)
    }

    fn _render_template(template: &SExpr, bindings: &HashMap<Id, SExpr>) -> SExpr {
        match template {
            SExpr::Id(pattern) => bindings.get(pattern).unwrap_or(template).clone(),
            SExpr::Cons(pattern) => match &pattern.car {
                SExpr::Id(id) if id.symbol.0 == "..." => bindings.get(id).unwrap().clone(),
                _ => SExpr::new_cons(
                    Self::_render_template(&pattern.car, bindings),
                    Self::_render_template(&pattern.cdr, bindings),
                ),
            },
            _ => template.clone(),
        }
    }

    fn render_template(&self, bindings: &HashMap<Id, SExpr>) -> SExpr {
        Self::_render_template(&self.template, bindings)
    }

    pub fn apply(&self, literals: &HashSet<Symbol>, sexpr: &SExpr) -> Option<SExpr> {
        let bindings = self.match_pattern(literals, sexpr)?;
        Some(self.render_template(&bindings))
    }
}

impl Transformer {
    pub fn new(spec: &SExpr) -> Self {
        match_sexpr! {(S(syntax-rules), (literals_list @ ..), rules @ ..) = spec =>
            let mut literals = HashSet::<Symbol>::new();
            for_each(|literal| {
                if let SExpr::Symbol(sym) = literal{
                    literals.insert(sym.clone());
                } else {
                    unreachable!("Expected symbols in syntax transformer literals");
                }
            }, &literals_list.coerce_to_datum());

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

    pub fn transform(&self, application: &SExpr) -> Option<SExpr> {
        self.syntax_rules
            .iter()
            .filter_map(|syntax_rule| syntax_rule.apply(&self.literals, application))
            .next()
    }
}

#[cfg(test)]
mod tests {
    use crate::{compile::expand::introduce, sexpr};

    use super::*;

    #[test]
    fn test_and_transformer_base_case() {
        #[rustfmt::skip]
        let transformer = Transformer::new(&introduce(&sexpr!(
            S(syntax-rules),
            (),
            ((S(_)), SExpr::new_bool(false))
        )));

        assert_eq!(
            transformer.transform(&introduce(&sexpr!(S(and)))).unwrap(),
            SExpr::new_bool(false)
        );

        #[rustfmt::skip]
        let transformer = Transformer::new(&introduce(&sexpr!(
            S(syntax-rules),
            (),
            ((S(_), S(e)), S(e))
        )));

        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(x))))
                .unwrap(),
            introduce(&SExpr::new_symbol("x"))
        );
    }

    #[test]
    fn test_and_transformer_recursive_case() {
        #[rustfmt::skip]
        let transformer = Transformer::new(&introduce(&sexpr!(
            S(syntax-rules),
            (),
            ((S(_), S(e1), S(e2), S(...)),
             (S(if), S(e1),
                     (S(and), S(e2), S(...)),
                     SExpr::new_bool(false)))
        )));

        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b)), SExpr::new_bool(false)))
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b), S(c))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b), S(c)), SExpr::new_bool(false)))
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b), S(c), S(d))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b), S(c), S(d)), SExpr::new_bool(false)))
        );
    }

    #[test]
    fn test_and_transformer() {
        #[rustfmt::skip]
        let transformer = Transformer::new(&introduce(&sexpr!(
            S(syntax-rules),
            (),
            ((S(_)), SExpr::new_bool(false)),
            ((S(_), S(e)), S(e)),
            ((S(_), S(e1), S(e2), S(...)),
             (S(if), S(e1),
                     (S(and), S(e2), S(...)),
                     SExpr::new_bool(false)))
        )));
        assert_eq!(
            transformer.transform(&introduce(&sexpr!(S(and)))).unwrap(),
            SExpr::new_bool(false)
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a))))
                .unwrap(),
            introduce(&SExpr::new_symbol("a"))
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b)), SExpr::new_bool(false)))
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b), S(c))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b), S(c)), SExpr::new_bool(false)))
        );
        assert_eq!(
            transformer
                .transform(&introduce(&sexpr!(S(and), S(a), S(b), S(c), S(d))))
                .unwrap(),
            introduce(&sexpr!(S(if), S(a), (S(and), S(b), S(c), S(d)), SExpr::new_bool(false)))
        );
    }
}
