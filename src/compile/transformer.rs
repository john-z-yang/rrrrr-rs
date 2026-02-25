use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::compile::bindings::Bindings;

use crate::compile::util::{is_proper_list, len, try_dotted_tail, try_for_each};
use crate::if_let_sexpr;

use super::compilation_error::{CompilationError, Result};
use super::sexpr::{Id, SExpr, Symbol};

#[derive(Debug)]
struct SyntaxRule {
    pattern: SExpr,
    template: SExpr,
    literals: Arc<HashSet<Symbol>>,
}

#[derive(Debug, Clone)]
enum MatchedSExpr {
    One(SExpr),
    Many(Vec<MatchedSExpr>),
}

fn get_variables(sexpr: &SExpr, literals: &HashSet<Symbol>, vars: &mut HashSet<Symbol>) {
    match sexpr {
        SExpr::Id(Id { symbol, .. }, _)
            if symbol.0 != "..." && symbol.0 != "_" && !literals.contains(symbol) =>
        {
            vars.insert(symbol.clone());
        }
        SExpr::Cons(cons, _) => {
            get_variables(&cons.car, literals, vars);
            get_variables(&cons.cdr, literals, vars);
        }
        _ => {}
    }
}

fn collect_ellipses(cdr: &SExpr) -> (usize, &SExpr) {
    let mut count = 0;
    let mut cur = cdr;
    while let SExpr::Cons(cons, _) = cur {
        if matches!(&*cons.car, SExpr::Id(Id { symbol, .. }, _) if symbol.0 == "...") {
            count += 1;
            cur = &cons.cdr;
        } else {
            break;
        }
    }
    (count, cur)
}

impl SyntaxRule {
    fn new(pattern: SExpr, template: SExpr, literals: Arc<HashSet<Symbol>>) -> Result<Self> {
        fn validate_pattern(
            pattern: &SExpr,
            literals: &HashSet<Symbol>,
            seen_symbols: &mut HashSet<Symbol>,
        ) -> Result<()> {
            match pattern {
                SExpr::Id(Id { symbol, .. }, span) => {
                    if symbol.0 == "..." {
                        return Err(CompilationError {
                            span: *span,
                            reason: "'...' is not allowed as a pattern".into(),
                        });
                    }
                    if symbol.0 != "_"
                        && !literals.contains(symbol)
                        && !seen_symbols.insert(symbol.clone())
                    {
                        return Err(CompilationError {
                            span: *span,
                            reason: format!("duplicate pattern variable '{}'", symbol),
                        });
                    }
                    Ok(())
                }
                SExpr::Cons(_, _) => validate_list(pattern, literals, seen_symbols),
                _ => Ok(()),
            }
        }

        fn validate_list(
            pattern: &SExpr,
            literals: &HashSet<Symbol>,
            seen_symbols: &mut HashSet<Symbol>,
        ) -> Result<()> {
            let mut cur = pattern;
            while let SExpr::Cons(cons, _) = cur {
                let (ellipsis_count, rest) = collect_ellipses(&cons.cdr);
                if ellipsis_count > 0 {
                    if ellipsis_count > 1 {
                        return Err(CompilationError {
                            span: cons.cdr.get_span(),
                            reason: "Multiple consecutive '...' in pattern".into(),
                        });
                    }
                    validate_pattern(&cons.car, literals, seen_symbols)?;
                    if matches!(rest, SExpr::Cons(..)) {
                        return Err(CompilationError {
                            span: rest.get_span(),
                            reason: "Unexpected pattern element after '...'".into(),
                        });
                    }
                    return Ok(());
                }
                validate_pattern(&cons.car, literals, seen_symbols)?;
                cur = &cons.cdr;
            }
            match cur {
                SExpr::Nil(_) => Ok(()),
                _ => validate_pattern(cur, literals, seen_symbols),
            }
        }

        let mut seen = HashSet::new();
        validate_pattern(&pattern, &literals, &mut seen)?;
        Ok(SyntaxRule {
            pattern,
            template,
            literals,
        })
    }

    fn match_pattern(
        &self,
        target: &SExpr,
        bindings: &Bindings,
    ) -> Option<HashMap<Symbol, MatchedSExpr>> {
        fn _match_ellipsis(
            repeat: &SExpr,
            target: &SExpr,
            literals: &HashSet<Symbol>,
            bindings: &Bindings,
            matches: &mut HashMap<Symbol, MatchedSExpr>,
        ) -> Option<()> {
            let mut collected: HashMap<Symbol, Vec<MatchedSExpr>> = HashMap::new();
            let mut cur = target;
            while let SExpr::Cons(cons, _) = cur {
                let mut sub = HashMap::new();
                _match(repeat, &cons.car, literals, bindings, &mut sub)?;
                for (k, v) in sub {
                    collected.entry(k).or_default().push(v);
                }
                cur = &cons.cdr;
            }
            let mut vars = HashSet::new();
            get_variables(repeat, literals, &mut vars);
            for var in vars {
                collected.entry(var).or_default();
            }
            for (k, vs) in collected {
                matches.insert(k, MatchedSExpr::Many(vs));
            }
            Some(())
        }

        fn _match_list(
            pattern: &SExpr,
            target: &SExpr,
            literals: &HashSet<Symbol>,
            bindings: &Bindings,
            matches: &mut HashMap<Symbol, MatchedSExpr>,
        ) -> Option<()> {
            let mut pattern_iter = pattern;
            let mut target_iter = target;
            loop {
                match pattern_iter {
                    SExpr::Cons(pattern, _) if collect_ellipses(&pattern.cdr).0 > 0 => {
                        _match_ellipsis(&pattern.car, target_iter, literals, bindings, matches)?;
                        return _match(
                            &try_dotted_tail(pattern_iter).expect("pattern is a list"),
                            &try_dotted_tail(target_iter).unwrap_or(target_iter.clone()),
                            literals,
                            bindings,
                            matches,
                        );
                    }
                    SExpr::Cons(pattern, _) => {
                        let SExpr::Cons(target, _) = target_iter else {
                            return None;
                        };
                        _match(&pattern.car, &target.car, literals, bindings, matches)?;
                        pattern_iter = &pattern.cdr;
                        target_iter = &target.cdr;
                    }
                    _ => return _match(pattern_iter, target_iter, literals, bindings, matches),
                }
            }
        }

        fn _match(
            pattern: &SExpr,
            target: &SExpr,
            literals: &HashSet<Symbol>,
            bindings: &Bindings,
            matches: &mut HashMap<Symbol, MatchedSExpr>,
        ) -> Option<()> {
            match (pattern, target) {
                (SExpr::Id(pat_id, _), _) if literals.contains(&pat_id.symbol) => {
                    let SExpr::Id(tgt_id, _) = target else {
                        return None;
                    };
                    match (bindings.resolve(pat_id), bindings.resolve(tgt_id)) {
                        (Some(resolved_pat), Some(resolved_tgt)) => {
                            (resolved_pat == resolved_tgt).then_some(())
                        }
                        (None, None) => (pat_id.symbol == tgt_id.symbol).then_some(()),
                        _ => None,
                    }
                }
                (SExpr::Id(Id { symbol, .. }, _), _) if symbol.0 == "_" => Some(()),
                (SExpr::Id(Id { symbol, .. }, _), _) => {
                    matches.insert(symbol.clone(), MatchedSExpr::One(target.clone()));
                    Some(())
                }
                (SExpr::Cons(_, _), _) => _match_list(pattern, target, literals, bindings, matches),
                _ if pattern.without_spans() == target.without_spans() => Some(()),
                _ => None,
            }
        }

        let mut matches = HashMap::new();
        _match(
            &self.pattern,
            target,
            &self.literals,
            bindings,
            &mut matches,
        )?;
        Some(matches)
    }

    fn render_template(&self, matches: &HashMap<Symbol, MatchedSExpr>) -> Result<SExpr> {
        fn expand_repeated(
            template: &SExpr,
            level: usize,
            matches: &HashMap<Symbol, MatchedSExpr>,
        ) -> Result<Vec<SExpr>> {
            if level == 0 {
                return Ok(vec![render(template, matches)?]);
            }

            let mut symbols = HashSet::new();
            get_variables(template, &HashSet::new(), &mut symbols);
            let packed: Vec<_> = symbols
                .iter()
                .filter_map(|sym| match matches.get(sym) {
                    Some(MatchedSExpr::Many(items)) => Some((sym, items)),
                    _ => None,
                })
                .collect();

            if packed.is_empty() {
                return Err(CompilationError {
                    span: template.get_span(),
                    reason: "At least one variable needs to be a repeated capture".to_owned(),
                });
            }

            let len = packed[0].1.len();
            for &(sym, items) in &packed[1..] {
                if items.len() != len {
                    return Err(CompilationError {
                        span: template.get_span(),
                        reason: format!(
                            "Incompatible ellipsis match counts for '{}' ({}) and '{}' ({})",
                            packed[0].0,
                            len,
                            sym,
                            items.len()
                        ),
                    });
                }
            }

            (0..len).try_fold(Vec::new(), |mut acc, i| {
                let mut sub_matches = matches.clone();
                for &(sym, items) in &packed {
                    sub_matches.insert(sym.clone(), items[i].clone());
                }
                acc.extend(expand_repeated(template, level - 1, &sub_matches)?);
                Ok(acc)
            })
        }

        fn render(template: &SExpr, matches: &HashMap<Symbol, MatchedSExpr>) -> Result<SExpr> {
            match template {
                SExpr::Id(Id { symbol, .. }, span) => match matches.get(symbol) {
                    Some(MatchedSExpr::One(sexpr)) => Ok(sexpr.clone()),
                    Some(MatchedSExpr::Many(_)) => Err(CompilationError {
                        span: *span,
                        reason: format!(
                            "'{}' is followed by ellipsis in pattern but not in template",
                            symbol
                        ),
                    }),
                    None => Ok(template.clone()),
                },
                SExpr::Cons(_, _) => render_list(template, matches),
                _ => Ok(template.clone()),
            }
        }

        fn render_list(template: &SExpr, matches: &HashMap<Symbol, MatchedSExpr>) -> Result<SExpr> {
            match template {
                SExpr::Cons(cons, _) => {
                    let (level, rest) = collect_ellipses(&cons.cdr);
                    if level > 0 {
                        let expanded = expand_repeated(&cons.car, level, matches)?;
                        let tail = render_list(rest, matches)?;
                        Ok(expanded
                            .into_iter()
                            .rfold(tail, |acc, item| SExpr::cons(item, acc)))
                    } else {
                        let car = render(&cons.car, matches)?;
                        let cdr = render_list(&cons.cdr, matches)?;
                        Ok(SExpr::cons(car, cdr))
                    }
                }
                SExpr::Nil(_) => Ok(template.clone()),
                _ => render(template, matches),
            }
        }

        render(&self.template, matches)
    }
}

#[derive(Debug)]
pub(crate) struct Transformer {
    syntax_rules: Vec<SyntaxRule>,
}

impl Transformer {
    pub(crate) fn new(spec: &SExpr) -> Result<Self> {
        if_let_sexpr! {(_, (literals_list @ ..), rules @ ..) = spec =>
            let mut literals = HashSet::<Symbol>::new();
            if len(rules) == 0 {
                return Err(CompilationError {
                    span: rules.get_span(),
                    reason: "Expected syntax transformer to have at least 1 rule".to_owned(),
                });
            }
            if !is_proper_list(literals_list) {
                return Err(CompilationError {
                    span: literals_list.get_span(),
                    reason: "Expected literals in syntax transformer to be proper list".to_owned(),
                });
            }
            if !is_proper_list(rules) {
                return Err(CompilationError {
                    span: rules.get_span(),
                    reason: "Expected rules in syntax transformer to be proper list".to_owned(),
                });
            }
            try_for_each(
                |literal| {
                    let SExpr::Id(Id { symbol, scopes: _ }, _) = literal else {
                        return Err(CompilationError {
                            span: literal.get_span(),
                            reason: format!(
                                "Expected symbols in syntax transformer literals, got: {}",
                                literal
                            ),
                        });
                    };
                    if symbol.0 == "..." || symbol.0 == "_" {
                        return Err(CompilationError {
                            span: literal.get_span(),
                            reason: format!(
                                "{} is not allowed in syntax transformer literals",
                                literal
                            ),
                        });
                    }
                    literals.insert(symbol.clone());
                    Ok(())
                },
                literals_list,
            )?;

            let literals = Arc::new(literals);
            let mut syntax_rules = Vec::<SyntaxRule>::new();
            try_for_each(
                |rule_pair| {
                    if_let_sexpr! {(pattern, template) = rule_pair =>
                        let SExpr::Cons(pattern, _) = pattern else {
                            return Err(CompilationError {
                                span: pattern.get_span(),
                                reason: "Syntax transformer pattern must be a list".to_owned(),
                            });
                        };
                        if !matches!(*pattern.car, SExpr::Id(..)) {
                            return Err(CompilationError {
                                span: pattern.car.get_span(),
                                reason: format!(
                                    "Syntax transformer pattern must start with an identifier, got {}",
                                    pattern.car
                                ),
                            });
                        }
                        syntax_rules.push(SyntaxRule::new(
                            pattern.cdr.as_ref().clone(),
                            template.clone(),
                            Arc::clone(&literals),
                        )?);
                        return Ok(());
                    }
                    Err(CompilationError {
                        span: rule_pair.get_span(),
                        reason: "Unrecognized syntax for syntax transformer rule pair".to_owned(),
                    })
                },
                rules,
            )?;

            return Ok(Self { syntax_rules })
        }
        Err(CompilationError {
            span: spec.get_span(),
            reason: "Unrecognized syntax for syntax transformer".to_owned(),
        })
    }

    pub(crate) fn transform(
        &self,
        application: &SExpr,
        bindings: &Bindings,
    ) -> Option<Result<SExpr>> {
        let SExpr::Cons(app_cons, _) = application else {
            return None;
        };
        self.syntax_rules
            .iter()
            .filter_map(|rule| {
                let matches = rule.match_pattern(&app_cons.cdr, bindings)?;
                Some(rule.render_template(&matches))
            })
            .next()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::compile::{expand::introduce, lex::tokenize, parse::parse, span::Span};

    use super::*;

    impl MatchedSExpr {
        fn eq_ignoring_spans(&self, other: &MatchedSExpr) -> bool {
            match (self, other) {
                (MatchedSExpr::One(a), MatchedSExpr::One(b)) => {
                    a.without_spans() == b.without_spans()
                }
                (MatchedSExpr::Many(a), MatchedSExpr::Many(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignoring_spans(y))
                }
                _ => false,
            }
        }
    }

    fn p(src: &str) -> SExpr {
        parse(&tokenize(src).unwrap()).unwrap()
    }

    fn one(src: &str) -> MatchedSExpr {
        MatchedSExpr::One(p(src))
    }

    fn many(vals: Vec<MatchedSExpr>) -> MatchedSExpr {
        MatchedSExpr::Many(vals)
    }

    fn do_match(pattern: &str, target: &str) -> Option<HashMap<Symbol, MatchedSExpr>> {
        do_match_with_literals(pattern, target, &[])
    }

    fn do_match_with_literals(
        pattern: &str,
        target: &str,
        literals: &[&str],
    ) -> Option<HashMap<Symbol, MatchedSExpr>> {
        let rule = SyntaxRule::new(
            p(pattern),
            nil(),
            Arc::new(literals.iter().map(|s| Symbol::new(s)).collect()),
        )
        .expect("invalid pattern in test");
        rule.match_pattern(&p(target), &Bindings::new())
    }

    fn assert_binding(
        matches: &HashMap<Symbol, MatchedSExpr>,
        name: &str,
        expected: &MatchedSExpr,
    ) {
        let sym = Symbol::new(name);
        let actual = matches
            .get(&sym)
            .unwrap_or_else(|| panic!("missing binding for '{}'", name));
        assert!(
            actual.eq_ignoring_spans(expected),
            "binding for '{}': got {:?}, expected {:?}",
            name,
            actual,
            expected
        );
    }

    // match(1, 1) == {}
    #[test]
    fn test_match_literal_equal() {
        let result = do_match("1", "1");
        let matches = result.expect("should match");
        assert!(matches.is_empty());
    }

    // match(2, 1) is None
    #[test]
    fn test_match_literal_not_equal() {
        assert!(do_match("2", "1").is_none());
    }

    // match("x", "a") == {"x": One("a")}
    #[test]
    fn test_match_pattern_variable() {
        let matches = do_match("x", "a").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "x", &one("a"));
    }

    // match([1, "x", 1], [1, "a", 1]) == {"x": One("a")}
    #[test]
    fn test_match_list_with_literals() {
        let matches = do_match("(1 x 1)", "(1 a 1)").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "x", &one("a"));
    }

    // match([1, "x", 2], [1, "a", 1]) is None
    #[test]
    fn test_match_list_literal_mismatch() {
        assert!(do_match("(1 x 2)", "(1 a 1)").is_none());
    }

    // match([1, "x"], [1, "a", 1]) is None — length mismatch
    #[test]
    fn test_match_list_too_short_pattern() {
        assert!(do_match("(1 x)", "(1 a 1)").is_none());
    }

    // match([1, "x", 1], [1, "a"]) is None — length mismatch
    #[test]
    fn test_match_list_too_long_pattern() {
        assert!(do_match("(1 x 1)", "(1 a)").is_none());
    }

    // match(["a", "..."], ["x", "y"]) == {"a": Many([One("x"), One("y")])}
    #[test]
    fn test_match_simple_ellipsis() {
        let matches = do_match("(a ...)", "(x y)").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "a", &many(vec![one("x"), one("y")]));
    }

    // match(["a", "..."], []) — zero repetitions, variable tracked as empty Many
    #[test]
    fn test_match_ellipsis_zero() {
        let matches = do_match("(a ...)", "()").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "a", &many(vec![]));
    }

    // match(["_", "a", "..."], ["mac"]) — ellipsis with prefix, zero reps
    #[test]
    fn test_match_ellipsis_with_prefix() {
        let matches = do_match("(_ a ...)", "(mac)").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "a", &many(vec![]));
    }

    // Zero repetitions with multiple variables in repeat pattern
    #[test]
    fn test_match_ellipsis_zero_multi_var() {
        let matches = do_match("((a b) ...)", "()").expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "a", &many(vec![]));
        assert_binding(&matches, "b", &many(vec![]));
    }

    // Zero repetitions with nested ellipsis
    #[test]
    fn test_match_nested_ellipsis_zero() {
        let matches = do_match("((a ...) ...)", "()").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "a", &many(vec![]));
    }

    // match(["_", "e1", "e2", "..."], ["and", "a", "b", "c"])
    #[test]
    fn test_match_ellipsis_with_prefix_and_elements() {
        let matches = do_match("(_ e1 e2 ...)", "(and a b c)").expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "e1", &one("a"));
        assert_binding(&matches, "e2", &many(vec![one("b"), one("c")]));
    }

    // match([["a", "..."], "..."], [["x", "y"], ["z"]])
    //   == {"a": Many([Many([One("x"), One("y")]), Many([One("z")])])}
    #[test]
    fn test_match_nested_ellipsis() {
        let matches = do_match("((a ...) ...)", "((x y) (z))").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(
            &matches,
            "a",
            &many(vec![many(vec![one("x"), one("y")]), many(vec![one("z")])]),
        );
    }

    // match([[[1, "a"], "..."], "..."], [[[1, "x"], [1, "y"]], [[1, "z"]]])
    #[test]
    fn test_match_nested_ellipsis_with_literals() {
        let matches =
            do_match("(((1 a) ...) ...)", "(((1 x) (1 y)) ((1 z)))").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(
            &matches,
            "a",
            &many(vec![many(vec![one("x"), one("y")]), many(vec![one("z")])]),
        );
    }

    // match([[[1, "a"], "..."], "..."], [[[2, "x"], [1, "y"]], [[1, "z"]]]) is None
    #[test]
    fn test_match_nested_ellipsis_literal_mismatch() {
        assert!(do_match("(((1 a) ...) ...)", "(((2 x) (1 y)) ((1 z)))").is_none());
    }

    // Wildcard matches anything
    #[test]
    fn test_match_wildcard() {
        let matches = do_match("_", "42").expect("should match");
        assert!(matches.is_empty());

        let matches = do_match("_", "(a b c)").expect("should match");
        assert!(matches.is_empty());
    }

    // Pattern variable captures a list
    #[test]
    fn test_match_variable_captures_list() {
        let matches = do_match("x", "(a b)").expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "x", &one("(a b)"));
    }

    // Ellipsis with complex repeat pattern
    #[test]
    fn test_match_ellipsis_complex_repeat() {
        let matches = do_match("((a b) ...)", "((1 2) (3 4))").expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "a", &many(vec![one("1"), one("3")]));
        assert_binding(&matches, "b", &many(vec![one("2"), one("4")]));
    }

    // The `and` macro pattern from existing tests
    #[test]
    fn test_match_and_macro_pattern() {
        // (_ e1 e2 ...) against (and a b)
        let matches = do_match("(_ e1 e2 ...)", "(and a b)").expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "e1", &one("a"));
        assert_binding(&matches, "e2", &many(vec![one("b")]));

        // (_ e1 e2 ...) against (and a b c d)
        let matches = do_match("(_ e1 e2 ...)", "(and a b c d)").expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "e1", &one("a"));
        assert_binding(&matches, "e2", &many(vec![one("b"), one("c"), one("d")]));
    }

    // Non-list target against list pattern
    #[test]
    fn test_match_list_pattern_atom_target() {
        assert!(do_match("(a b)", "42").is_none());
    }

    // Ellipsis pattern against non-list target
    #[test]
    fn test_match_ellipsis_pattern_atom_target() {
        assert!(do_match("(a ...)", "42").is_none());
    }

    // --- render tests ---

    fn assert_renders_to(pattern: &str, template: &str, target: &str, expected: &str) {
        let rule = SyntaxRule::new(p(pattern), p(template), Arc::new(HashSet::new()))
            .expect("invalid pattern in test");
        let matches = rule
            .match_pattern(&p(target), &Bindings::new())
            .expect("pattern should match target");
        let result = rule
            .render_template(&matches)
            .expect("render should succeed");
        assert_eq!(
            result.without_spans(),
            p(expected).without_spans(),
            "render({template}, {matches:?}) = {result}, expected {expected}"
        );
    }

    #[test]
    fn test_render_atom_passthrough() {
        assert_renders_to("(_)", "#f", "(mac)", "#f");
    }

    #[test]
    fn test_render_variable_substitution() {
        assert_renders_to("(_ e)", "e", "(mac x)", "x");
    }

    #[test]
    fn test_render_literal_in_template() {
        assert_renders_to("(_ x)", "(f x)", "(mac 1)", "(f 1)");
    }

    #[test]
    fn test_render_simple_ellipsis() {
        assert_renders_to("(_ x ...)", "(x ...)", "(mac 1 2 3)", "(1 2 3)");
    }

    #[test]
    fn test_render_zero_repetition() {
        assert_renders_to("(_ x ...)", "(x ...)", "(mac)", "()");
    }

    #[test]
    fn test_render_ellipsis_with_prefix() {
        assert_renders_to(
            "(_ e1 e2 ...)",
            "(if e1 (and e2 ...) #f)",
            "(and a b c)",
            "(if a (and b c) #f)",
        );
    }

    #[test]
    fn test_render_ellipsis_with_two_args() {
        assert_renders_to(
            "(_ e1 e2 ...)",
            "(if e1 (and e2 ...) #f)",
            "(and a b)",
            "(if a (and b) #f)",
        );
    }

    #[test]
    fn test_render_literal_in_repeated_template() {
        assert_renders_to("(_ x ...)", "((f x) ...)", "(mac 1 2)", "((f 1) (f 2))");
    }

    #[test]
    fn test_render_nested_ellipsis() {
        assert_renders_to(
            "(_ (a ...) ...)",
            "((a ...) ...)",
            "(mac (x y) (z))",
            "((x y) (z))",
        );
    }

    #[test]
    fn test_render_double_ellipsis_flatten() {
        assert_renders_to(
            "(_ (a ...) ...)",
            "(a ... ...)",
            "(mac (1 2) (3))",
            "(1 2 3)",
        );
    }

    #[test]
    fn test_render_dotted_tail_datum_after_ellipsis() {
        assert_renders_to("(_ a ... . b)", "(a ... . b)", "(mac . 1)", "1");
        assert_renders_to("(_ a ... . b)", "(a ... . b)", "(mac)", "()");
        assert_renders_to("(_ a ... . b)", "(b a ...)", "(mac 1 2 3)", "(() 1 2 3)");
        assert_renders_to("(_ a ... . b)", "(b a ...)", "(mac 1 . 0)", "(0 1)");
        assert_renders_to("(_ a ... . b)", "(b a ...)", "(mac 1 2 3 . 0)", "(0 1 2 3)");
    }

    #[test]
    fn test_render_complex_repeat() {
        assert_renders_to(
            "(_ (a b) ...)",
            "((b a) ...)",
            "(mac (1 2) (3 4))",
            "((2 1) (4 3))",
        );
    }

    #[test]
    fn test_render_improper_list_template() {
        assert_renders_to("(_ a)", "(1 2 . a)", "(mac 3)", "(1 2 . 3)");
    }

    #[test]
    fn test_render_zero_repetition_multi_var() {
        assert_renders_to("(_ (a b) ...)", "((b a) ...)", "(mac)", "()");
    }

    #[test]
    fn test_render_zero_repetition_nested() {
        assert_renders_to("(_ (a ...) ...)", "(a ... ...)", "(mac)", "()");
    }

    // --- validation tests (SyntaxRule::new) ---

    fn nil() -> SExpr {
        SExpr::Nil(Span { lo: 0, hi: 0 })
    }

    fn make_rule(pattern: &str) -> Result<SyntaxRule> {
        SyntaxRule::new(p(pattern), nil(), Arc::new(HashSet::new()))
    }

    fn make_rule_with_literals(pattern: &str, literals: &[&str]) -> Result<SyntaxRule> {
        SyntaxRule::new(
            p(pattern),
            nil(),
            Arc::new(literals.iter().map(|s| Symbol::new(s)).collect()),
        )
    }

    #[test]
    fn test_new_valid_simple_pattern() {
        assert!(make_rule("(_ x)").is_ok());
    }

    #[test]
    fn test_new_valid_ellipsis_at_end() {
        assert!(make_rule("(_ x ...)").is_ok());
    }

    #[test]
    fn test_new_valid_nested_ellipsis() {
        assert!(make_rule("(_ (a ...) ...)").is_ok());
    }

    #[test]
    fn test_new_duplicate_pattern_variable() {
        let err = make_rule("(_ a a)").unwrap_err();
        assert!(err.reason.contains("duplicate pattern variable 'a'"));
    }

    #[test]
    fn test_new_duplicate_pattern_variable_nested() {
        let err = make_rule("(_ (a b) a)").unwrap_err();
        assert!(err.reason.contains("duplicate pattern variable 'a'"));
    }

    #[test]
    fn test_new_duplicate_wildcard_allowed() {
        assert!(make_rule("(_ _ _)").is_ok());
    }

    #[test]
    fn test_new_dotted_tail_pattern_after_ellipses_allowed() {
        assert!(make_rule("(_ a ... . b)").is_ok());
    }

    #[test]
    fn test_new_duplicate_literal_allowed() {
        assert!(make_rule_with_literals("(_ foo foo)", &["foo"]).is_ok());
    }

    #[test]
    fn test_new_ellipsis_not_at_end() {
        let err = make_rule("(_ a ... b)").unwrap_err();
        assert!(
            err.reason
                .contains("Unexpected pattern element after '...'")
        );
    }

    #[test]
    fn test_new_bare_ellipsis() {
        let err = make_rule("...").unwrap_err();
        assert!(err.reason.contains("'...' is not allowed as a pattern"));
    }

    #[test]
    fn test_new_ellipsis_as_first_element() {
        let err = make_rule("(... a)").unwrap_err();
        assert!(err.reason.contains("'...' is not allowed as a pattern"));
    }

    #[test]
    fn test_new_valid_atom_pattern() {
        assert!(make_rule("x").is_ok());
    }

    #[test]
    fn test_new_valid_complex_pattern() {
        assert!(make_rule("(_ (a b ...) c)").is_ok());
    }

    #[test]
    fn test_new_consecutive_ellipsis_rejected() {
        let err = make_rule("(_ a ... ...)").unwrap_err();
        assert!(err.reason.contains("Multiple consecutive '...' in pattern"));
    }

    #[test]
    fn test_new_triple_consecutive_ellipsis_rejected() {
        let err = make_rule("(_ a ... ... ...)").unwrap_err();
        assert!(err.reason.contains("Multiple consecutive '...' in pattern"));
    }

    #[test]
    fn test_new_nested_consecutive_ellipsis_rejected() {
        let err = make_rule("(_ (a ... ...) b)").unwrap_err();
        assert!(err.reason.contains("Multiple consecutive '...' in pattern"));
    }

    // --- literal identifier tests ---

    // Unbound literal in pattern matches same unbound literal in target
    #[test]
    fn test_match_literal_identifier_same_name() {
        let matches =
            do_match_with_literals("(_ foo e)", "(mac foo 42)", &["foo"]).expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "e", &one("42"));
    }

    // Unbound literal in pattern does not match different unbound identifier
    #[test]
    fn test_match_literal_identifier_different_name() {
        assert!(do_match_with_literals("(_ foo e)", "(mac bar 42)", &["foo"]).is_none());
    }

    // Literal in pattern does not match non-identifier target
    #[test]
    fn test_match_literal_identifier_vs_non_identifier() {
        assert!(do_match_with_literals("(_ foo e)", "(mac 42 x)", &["foo"]).is_none());
    }

    // Literal in pattern does not match list target
    #[test]
    fn test_match_literal_identifier_vs_list() {
        assert!(do_match_with_literals("(_ foo e)", "(mac (a b) x)", &["foo"]).is_none());
    }

    // Non-literal identifier is still captured as a pattern variable
    #[test]
    fn test_match_non_literal_still_captures() {
        let matches =
            do_match_with_literals("(_ foo e)", "(mac bar 42)", &["other"]).expect("should match");
        assert_eq!(matches.len(), 2);
        assert_binding(&matches, "foo", &one("bar"));
        assert_binding(&matches, "e", &one("42"));
    }

    // Literal with ellipsis — only matching identifiers are consumed
    #[test]
    fn test_match_literal_in_ellipsis_subpattern() {
        let matches =
            do_match_with_literals("((_ foo e) ...)", "((mac foo 1) (mac foo 2))", &["foo"])
                .expect("should match");
        assert_eq!(matches.len(), 1);
        assert_binding(&matches, "e", &many(vec![one("1"), one("2")]));
    }

    // Literal with ellipsis — mismatch in one repetition fails the whole match
    #[test]
    fn test_match_literal_in_ellipsis_mismatch() {
        assert!(
            do_match_with_literals("((_ foo e) ...)", "((mac foo 1) (mac bar 2))", &["foo"])
                .is_none()
        );
    }

    // --- Transformer tests ---

    fn make_transformer(src: &str) -> Transformer {
        Transformer::new(&introduce(&parse(&tokenize(src).unwrap()).unwrap())).unwrap()
    }

    fn transform(transformer: &Transformer, src: &str) -> SExpr {
        transformer
            .transform(
                &introduce(&parse(&tokenize(src).unwrap()).unwrap()),
                &Bindings::new(),
            )
            .unwrap()
            .unwrap()
    }

    #[test]
    fn test_transformer_single_rule_literal_output() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_) #f))",
        );
        let result = transform(&t, "(and)");
        assert_eq!(
            result.without_spans(),
            introduce(&parse(&tokenize("#f").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_single_rule_identity() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ e) e))",
        );
        let result = transform(&t, "(mac x)");
        assert_eq!(
            result.without_spans(),
            introduce(&parse(&tokenize("x").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_ellipsis_rule() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ e1 e2 ...) (if e1 (and e2 ...) #f)))",
        );
        assert_eq!(
            transform(&t, "(and a b)").without_spans(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap()).without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c)").without_spans(),
            introduce(&parse(&tokenize("(if a (and b c) #f)").unwrap()).unwrap()).without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c d)").without_spans(),
            introduce(&parse(&tokenize("(if a (and b c d) #f)").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_multiple_rules() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_) #f)
               ((_ e) e)
               ((_ e1 e2 ...) (if e1 (and e2 ...) #f)))",
        );
        assert_eq!(
            transform(&t, "(and)").without_spans(),
            introduce(&parse(&tokenize("#f").unwrap()).unwrap()).without_spans()
        );
        assert_eq!(
            transform(&t, "(and x)").without_spans(),
            introduce(&parse(&tokenize("x").unwrap()).unwrap()).without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b)").without_spans(),
            introduce(&parse(&tokenize("(if a (and b) #f)").unwrap()).unwrap()).without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c d)").without_spans(),
            introduce(&parse(&tokenize("(if a (and b c d) #f)").unwrap()).unwrap()).without_spans()
        );
    }

    // Keyword name in the pattern is ignored — any name at application head works
    #[test]
    fn test_transformer_keyword_ignored() {
        let t = make_transformer(
            "(syntax-rules ()
               ((foo e) e))",
        );
        // Even though pattern says "foo", application uses "bar"
        assert_eq!(
            transform(&t, "(bar x)").without_spans(),
            introduce(&parse(&tokenize("x").unwrap()).unwrap()).without_spans()
        );
    }

    // Non-list application returns None
    #[test]
    fn test_transformer_non_list_application() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x) x))",
        );
        assert!(
            t.transform(
                &introduce(&parse(&tokenize("42").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .is_none()
        );
    }

    // Zero-rep ellipsis through the Transformer (end-to-end)
    #[test]
    fn test_transformer_zero_repetition_ellipsis() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x ...) (begin x ...)))",
        );
        assert_eq!(
            transform(&t, "(mac)").without_spans(),
            introduce(&parse(&tokenize("(begin)").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_no_matching_rule() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ a b) (b a)))",
        );
        assert!(
            t.transform(
                &introduce(&parse(&tokenize("(mac x)").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .is_none()
        );
    }

    #[test]
    fn test_transformer_first_matching_rule_wins() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x) 1)
               ((_ x) 2))",
        );
        assert_eq!(
            transform(&t, "(mac a)").without_spans(),
            introduce(&parse(&tokenize("1").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_with_literals() {
        let t = make_transformer(
            "(syntax-rules (=>)
               ((_ a => b) (b a)))",
        );
        assert_eq!(
            transform(&t, "(mac x => f)").without_spans(),
            introduce(&parse(&tokenize("(f x)").unwrap()).unwrap()).without_spans()
        );
        // Non-matching: `=>` in literal position doesn't match different identifier
        assert!(
            t.transform(
                &introduce(&parse(&tokenize("(mac x y f)").unwrap()).unwrap()),
                &Bindings::new(),
            )
            .is_none()
        );
    }

    #[test]
    fn test_transformer_improper_list_in_template() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ a) (1 2 . a)))",
        );
        assert_eq!(
            transform(&t, "(mac 3)").without_spans(),
            introduce(&parse(&tokenize("(1 2 . 3)").unwrap()).unwrap()).without_spans()
        );
    }

    #[test]
    fn test_transformer_new_invalid_spec() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules)").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
    }

    #[test]
    fn test_transformer_new_non_proper_list_of_rules() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (a b c) ((_ x) x) . 3)").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected rules in syntax transformer to be proper list")
        );
    }

    #[test]
    fn test_transformer_new_non_proper_list_of_literals() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (a b . c) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected literals in syntax transformer to be proper list")
        );
    }

    #[test]
    fn test_transformer_new_pattern_without_symbol_start() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (a b c) ((1 x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Syntax transformer pattern must start with an identifier, got")
        );
    }

    #[test]
    fn test_transformer_new_no_rules() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (a b c) )").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected syntax transformer to have at least 1 rule")
        );
    }

    #[test]
    fn test_transformer_new_non_symbol_literal() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (42) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected symbols in syntax transformer literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_duplicate_pattern_var() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules () ((_ a a) a))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("duplicate pattern variable")
        );
    }

    #[test]
    fn test_transformer_new_rejects_ellipsis_in_literals() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (...) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in syntax transformer literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_underscore_in_literals() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (_) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in syntax transformer literals")
        );
    }

    // ... mixed with valid literals still rejected
    #[test]
    fn test_transformer_new_rejects_ellipsis_among_valid_literals() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (=> ...) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in syntax transformer literals")
        );
    }

    // _ mixed with valid literals still rejected
    #[test]
    fn test_transformer_new_rejects_underscore_among_valid_literals() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules (=> _) ((_ x) x))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in syntax transformer literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_ellipsis_not_at_end() {
        let result = Transformer::new(&introduce(
            &parse(&tokenize("(syntax-rules () ((_ a ... b) a))").unwrap()).unwrap(),
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Unexpected pattern element after '...'")
        );
    }
}
