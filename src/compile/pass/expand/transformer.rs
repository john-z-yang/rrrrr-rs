use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::compile::bindings::Id;
use crate::compile::ident::Symbol;
use crate::compile::{
    bindings::Bindings,
    compilation_error::{CompilationError, Result},
    sexpr::{SExpr, Vector},
    util::{is_proper_list, len, try_dotted_tail, try_for_each},
};
use crate::if_let_sexpr;

#[derive(Debug, Clone)]
pub(super) struct Transformer {
    syntax_rules: Vec<SyntaxRule>,
}

#[derive(Debug, Clone)]
struct SyntaxRule {
    pattern: SExpr<Id>,
    template: SExpr<Id>,
    literals: Rc<HashSet<Symbol>>,
}

#[derive(Debug, Clone)]
enum CapturedSExpr {
    One(SExpr<Id>),
    Many(Vec<CapturedSExpr>),
}

fn collect_capture_variables(
    sexpr: &SExpr<Id>,
    literals: &HashSet<Symbol>,
    vars: &mut HashSet<Symbol>,
) {
    match sexpr {
        SExpr::Var(Id { symbol, .. }, _)
            if symbol.0 != "..." && symbol.0 != "_" && !literals.contains(symbol) =>
        {
            vars.insert(symbol.clone());
        }
        SExpr::Cons(cons, _) => {
            collect_capture_variables(&cons.car, literals, vars);
            collect_capture_variables(&cons.cdr, literals, vars);
        }
        SExpr::Vector(vector, _) => {
            for sexpr in &vector.0 {
                collect_capture_variables(sexpr, literals, vars);
            }
        }
        _ => {}
    }
}

fn consume_ellipsis(cdr: &SExpr<Id>) -> (usize, &SExpr<Id>) {
    let mut count = 0;
    let mut cur = cdr;
    while let SExpr::Cons(cons, _) = cur {
        if matches!(
            &*cons.car,
            SExpr::Var(Id { symbol, .. }, _) if symbol.0 == "..."
        ) {
            count += 1;
            cur = &cons.cdr;
        } else {
            break;
        }
    }
    (count, cur)
}

fn validate_pattern(
    pattern: &SExpr<Id>,
    literals: &HashSet<Symbol>,
    symbols_seen: &mut HashSet<Symbol>,
) -> Result<()> {
    match pattern {
        SExpr::Var(Id { symbol, .. }, span) => {
            if symbol.0 == "..." {
                return Err(CompilationError {
                    span: *span,
                    reason: "'...' is not allowed in this position".to_owned(),
                });
            }
            if symbol.0 != "_" && !literals.contains(symbol) && !symbols_seen.insert(symbol.clone())
            {
                return Err(CompilationError {
                    span: *span,
                    reason: format!("Duplicate pattern variable '{}'", symbol),
                });
            }
            Ok(())
        }
        SExpr::Cons(_, _) => {
            let mut cur = pattern;
            while let SExpr::Cons(cons, _) = cur {
                let (ellipsis_count, rest) = consume_ellipsis(&cons.cdr);
                if ellipsis_count > 0 {
                    if ellipsis_count > 1 {
                        return Err(CompilationError {
                            span: cons.cdr.get_span(),
                            reason: "Multiple consecutive '...' in pattern".to_owned(),
                        });
                    }
                    validate_pattern(&cons.car, literals, symbols_seen)?;
                    if matches!(rest, SExpr::Cons(..)) {
                        return Err(CompilationError {
                            span: rest.get_span(),
                            reason: "Unexpected pattern element after '...'".to_owned(),
                        });
                    }
                    if !matches!(rest, SExpr::Nil(_)) {
                        validate_pattern(rest, literals, symbols_seen)?;
                    }
                    return Ok(());
                }
                validate_pattern(&cons.car, literals, symbols_seen)?;
                cur = &cons.cdr;
            }
            if let SExpr::Nil(_) = cur {
                Ok(())
            } else {
                validate_pattern(cur, literals, symbols_seen)
            }
        }
        SExpr::Vector(vector, span) => validate_pattern(
            &vector.clone().into_cons_list(*span),
            literals,
            symbols_seen,
        ),
        _ => Ok(()),
    }
}

fn match_repetition(
    repeated_pattern: &SExpr<Id>,
    target: &SExpr<Id>,
    literals: &HashSet<Symbol>,
    bindings: &Bindings,
    captures: &mut HashMap<Symbol, CapturedSExpr>,
) -> Option<()> {
    let mut repeated_captures: HashMap<Symbol, Vec<CapturedSExpr>> = HashMap::new();
    let mut cur = target;
    while let SExpr::Cons(cons, _) = cur {
        let mut sub_captures = HashMap::new();
        match_subpattern(
            repeated_pattern,
            &cons.car,
            literals,
            bindings,
            &mut sub_captures,
        )?;
        for (k, v) in sub_captures {
            repeated_captures.entry(k).or_default().push(v);
        }
        cur = &cons.cdr;
    }
    let mut vars = HashSet::new();
    collect_capture_variables(repeated_pattern, literals, &mut vars);
    for var in vars {
        repeated_captures.entry(var).or_default();
    }
    for (k, vs) in repeated_captures {
        captures.insert(k, CapturedSExpr::Many(vs));
    }
    Some(())
}

fn match_subpatterns(
    patterns: &SExpr<Id>,
    target: &SExpr<Id>,
    literals: &HashSet<Symbol>,
    bindings: &Bindings,
    captures: &mut HashMap<Symbol, CapturedSExpr>,
) -> Option<()> {
    let mut cur_pattern = patterns;
    let mut cur_target = target;
    loop {
        match cur_pattern {
            SExpr::Cons(pattern, _) if consume_ellipsis(&pattern.cdr).0 > 0 => {
                match_repetition(&pattern.car, cur_target, literals, bindings, captures)?;
                return match_subpattern(
                    try_dotted_tail(cur_pattern).expect("pattern is a list"),
                    try_dotted_tail(cur_target).unwrap_or(cur_target),
                    literals,
                    bindings,
                    captures,
                );
            }
            SExpr::Cons(pattern, _) => {
                let SExpr::Cons(target, _) = cur_target else {
                    return None;
                };
                match_subpattern(&pattern.car, &target.car, literals, bindings, captures)?;
                cur_pattern = &pattern.cdr;
                cur_target = &target.cdr;
            }
            _ => return match_subpattern(cur_pattern, cur_target, literals, bindings, captures),
        }
    }
}

fn match_subpattern(
    pattern: &SExpr<Id>,
    target: &SExpr<Id>,
    literals: &HashSet<Symbol>,
    bindings: &Bindings,
    captures: &mut HashMap<Symbol, CapturedSExpr>,
) -> Option<()> {
    match (pattern, target) {
        (SExpr::Var(pat_id, _), _) if literals.contains(&pat_id.symbol) => {
            let SExpr::Var(tgt_id, _) = target else {
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
        (SExpr::Var(Id { symbol, .. }, _), _) if symbol.0 == "_" => Some(()),
        (SExpr::Var(Id { symbol, .. }, _), _) => {
            captures.insert(symbol.clone(), CapturedSExpr::One(target.clone()));
            Some(())
        }
        (SExpr::Cons(_, _), _) => match_subpatterns(pattern, target, literals, bindings, captures),
        (SExpr::Vector(pattern, pattern_span), SExpr::Vector(target, target_span)) => {
            match_subpatterns(
                &pattern.clone().into_cons_list(*pattern_span),
                &target.clone().into_cons_list(*target_span),
                literals,
                bindings,
                captures,
            )
        }
        _ if pattern.without_spans() == target.without_spans() => Some(()),
        _ => None,
    }
}

fn render_template_repetition(
    template: &SExpr<Id>,
    level: usize,
    captures: &HashMap<Symbol, CapturedSExpr>,
) -> Result<Vec<SExpr<Id>>> {
    if level == 0 {
        return Ok(vec![render_template(template, captures)?]);
    }

    let mut symbols = HashSet::new();
    collect_capture_variables(template, &HashSet::new(), &mut symbols);
    let packed: Vec<_> = symbols
        .iter()
        .filter_map(|sym| match captures.get(sym) {
            Some(CapturedSExpr::Many(items)) => Some((sym, items)),
            _ => None,
        })
        .collect();

    if packed.is_empty() {
        return Err(CompilationError {
            span: template.get_span(),
            reason: "Expected at least one repeated capture variable in ellipsis template"
                .to_owned(),
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
        let mut sub_captures = captures.clone();
        for &(sym, items) in &packed {
            sub_captures.insert(sym.clone(), items[i].clone());
        }
        acc.extend(render_template_repetition(
            template,
            level - 1,
            &sub_captures,
        )?);
        Ok(acc)
    })
}

fn render_template(
    template: &SExpr<Id>,
    captures: &HashMap<Symbol, CapturedSExpr>,
) -> Result<SExpr<Id>> {
    match template {
        SExpr::Var(Id { symbol, .. }, span) => match captures.get(symbol) {
            Some(CapturedSExpr::One(sexpr)) => Ok(sexpr.clone()),
            Some(CapturedSExpr::Many(_)) => Err(CompilationError {
                span: *span,
                reason: format!(
                    "'{}' is followed by ellipsis in pattern but not in template",
                    symbol
                ),
            }),
            None => Ok(template.clone()),
        },
        SExpr::Cons(_, _) => render_templates(template, captures),
        SExpr::Vector(vector, span) => Ok(
            match render_templates(&vector.clone().into_cons_list(*span), captures)? {
                SExpr::Cons(cons, span) => cons.try_into_vector(span).expect("Expected Cons list to be proper after rendering template converted from Vector"),
                SExpr::Nil(span) => SExpr::Vector(Vector(vec![]), span),
                _ => unreachable!(
                    "Expected Cons or Nil after rendering template converted from Vector"
                ),
            },
        ),
        _ => Ok(template.clone()),
    }
}

fn render_templates(
    templates: &SExpr<Id>,
    captures: &HashMap<Symbol, CapturedSExpr>,
) -> Result<SExpr<Id>> {
    match templates {
        SExpr::Cons(cons, _) => {
            let (level, rest) = consume_ellipsis(&cons.cdr);
            if level > 0 {
                let expanded = render_template_repetition(&cons.car, level, captures)?;
                let tail = render_template(rest, captures)?;
                Ok(expanded
                    .into_iter()
                    .rfold(tail, |acc, item| SExpr::cons(item, acc)))
            } else {
                let car = render_template(&cons.car, captures)?;
                let cdr = render_template(&cons.cdr, captures)?;
                Ok(SExpr::cons(car, cdr))
            }
        }
        SExpr::Nil(_) => Ok(templates.clone()),
        _ => unreachable!(
            "render_templates expected a list as template, but got {}",
            templates
        ),
    }
}

impl SyntaxRule {
    fn new(pattern: SExpr<Id>, template: SExpr<Id>, literals: Rc<HashSet<Symbol>>) -> Result<Self> {
        let mut symbols_seen = HashSet::new();
        validate_pattern(&pattern, &literals, &mut symbols_seen)?;
        Ok(SyntaxRule {
            pattern,
            template,
            literals,
        })
    }

    fn match_pattern(
        &self,
        target: &SExpr<Id>,
        bindings: &Bindings,
    ) -> Option<HashMap<Symbol, CapturedSExpr>> {
        let mut captures = HashMap::new();
        match_subpattern(
            &self.pattern,
            target,
            &self.literals,
            bindings,
            &mut captures,
        )?;
        Some(captures)
    }

    fn render_template(&self, captures: &HashMap<Symbol, CapturedSExpr>) -> Result<SExpr<Id>> {
        render_template(&self.template, captures)
    }
}

impl Transformer {
    pub(super) fn new(spec: &SExpr<Id>) -> Result<Self> {
        if_let_sexpr! {(_, literals_list @ (..), rules @ ..) = spec =>
            let mut literals = HashSet::<Symbol>::new();
            if len(rules) == 0 {
                return Err(CompilationError {
                    span: rules.get_span(),
                    reason: "Expected 'syntax-rules' to have at least one rule".to_owned(),
                });
            }
            if !is_proper_list(literals_list) {
                return Err(CompilationError {
                    span: literals_list.get_span(),
                    reason: "Expected 'syntax-rules' literals to be a proper list".to_owned(),
                });
            }
            if !is_proper_list(rules) {
                return Err(CompilationError {
                    span: rules.get_span(),
                    reason: "Expected 'syntax-rules' rules to be a proper list".to_owned(),
                });
            }
            try_for_each(literals_list, |literal| {
                let SExpr::Var(Id { symbol, scopes: _ }, _) = literal else {
                    return Err(CompilationError {
                        span: literal.get_span(),
                        reason: format!(
                            "Expected an identifier in 'syntax-rules' literals, but got: {}",
                            literal
                        ),
                    });
                };
                if symbol.0 == "..." || symbol.0 == "_" {
                    return Err(CompilationError {
                        span: literal.get_span(),
                        reason: format!(
                            "'{}' is not allowed in 'syntax-rules' literals",
                            literal
                        ),
                    });
                }
                literals.insert(symbol.clone());
                Ok(())
            })?;

            let literals = Rc::new(literals);
            let mut syntax_rules = Vec::<SyntaxRule>::new();
            try_for_each(rules, |rule_pair| {
                if_let_sexpr! {(pattern, template) = rule_pair =>
                    let SExpr::Cons(pattern, _) = pattern else {
                        return Err(CompilationError {
                            span: pattern.get_span(),
                            reason: "'syntax-rules' pattern must be a list".to_owned(),
                        });
                    };
                    if !matches!(pattern.car.as_ref(), SExpr::Var(..)) {
                        return Err(CompilationError {
                            span: pattern.car.get_span(),
                            reason: format!(
                                "'syntax-rules' pattern must start with an identifier, but got: {}",
                                pattern.car
                            ),
                        });
                    }
                    syntax_rules.push(SyntaxRule::new(
                        pattern.cdr.as_ref().clone(),
                        template.clone(),
                        Rc::clone(&literals),
                    )?);
                    return Ok(());
                }
                Err(CompilationError {
                    span: rule_pair.get_span(),
                    reason: "Invalid 'syntax-rules' rule: expected (pattern template)".to_owned(),
                })
            })?;

            return Ok(Self { syntax_rules });
        }
        Err(CompilationError {
            span: spec.get_span(),
            reason: "Invalid 'syntax-rules' form".to_owned(),
        })
    }

    pub(super) fn transform(
        &self,
        application: &SExpr<Id>,
        bindings: &Bindings,
    ) -> Option<Result<SExpr<Id>>> {
        let SExpr::Cons(app_cons, _) = application else {
            return None;
        };
        self.syntax_rules
            .iter()
            .filter_map(|rule| {
                let captures = rule.match_pattern(&app_cons.cdr, bindings)?;
                Some(rule.render_template(&captures))
            })
            .next()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::compile::{
        pass::{
            expand::introduce,
            read::{lex::tokenize, parse::parse},
        },
        span::Span,
    };

    use super::*;

    impl CapturedSExpr {
        fn eq_ignoring_spans(&self, other: &CapturedSExpr) -> bool {
            match (self, other) {
                (CapturedSExpr::One(a), CapturedSExpr::One(b)) => {
                    a.without_spans() == b.without_spans()
                }
                (CapturedSExpr::Many(a), CapturedSExpr::Many(b)) => {
                    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignoring_spans(y))
                }
                _ => false,
            }
        }
    }

    fn introduce_single_sexpr_src(src: &str) -> SExpr<Id> {
        introduce(parse(&tokenize(src).unwrap()).unwrap().pop().unwrap())
    }

    fn one(src: &str) -> CapturedSExpr {
        CapturedSExpr::One(introduce_single_sexpr_src(src))
    }

    fn many(vals: Vec<CapturedSExpr>) -> CapturedSExpr {
        CapturedSExpr::Many(vals)
    }

    fn do_match(pattern: &str, target: &str) -> Option<HashMap<Symbol, CapturedSExpr>> {
        do_match_with_literals(pattern, target, &[])
    }

    fn do_match_with_literals(
        pattern: &str,
        target: &str,
        literals: &[&str],
    ) -> Option<HashMap<Symbol, CapturedSExpr>> {
        let rule = SyntaxRule::new(
            introduce_single_sexpr_src(pattern),
            nil(),
            Rc::new(literals.iter().map(|s| Symbol::new(s)).collect()),
        )
        .expect("invalid pattern in test");
        rule.match_pattern(
            &introduce_single_sexpr_src(target),
            &Bindings::new(Default::default()),
        )
    }

    fn assert_binding(
        captures: &HashMap<Symbol, CapturedSExpr>,
        name: &str,
        expected: &CapturedSExpr,
    ) {
        let sym = Symbol::new(name);
        let actual = captures
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

    #[test]
    fn test_match_literal_equal() {
        let result = do_match("1", "1");
        let captures = result.expect("should match");
        assert!(captures.is_empty());
    }

    #[test]
    fn test_match_literal_not_equal() {
        assert!(do_match("2", "1").is_none());
    }

    #[test]
    fn test_match_pattern_variable() {
        let captures = do_match("x", "a").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "x", &one("a"));
    }

    #[test]
    fn test_match_list_with_literals() {
        let captures = do_match("(1 x 1)", "(1 a 1)").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "x", &one("a"));
    }

    #[test]
    fn test_match_list_literal_mismatch() {
        assert!(do_match("(1 x 2)", "(1 a 1)").is_none());
    }

    #[test]
    fn test_match_list_too_short_pattern() {
        assert!(do_match("(1 x)", "(1 a 1)").is_none());
    }

    #[test]
    fn test_match_list_too_long_pattern() {
        assert!(do_match("(1 x 1)", "(1 a)").is_none());
    }

    #[test]
    fn test_match_simple_ellipsis() {
        let captures = do_match("(a ...)", "(x y)").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![one("x"), one("y")]));
    }

    #[test]
    fn test_match_ellipsis_zero() {
        let captures = do_match("(a ...)", "()").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![]));
    }

    #[test]
    fn test_match_ellipsis_with_prefix() {
        let captures = do_match("(_ a ...)", "(mac)").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![]));
    }

    #[test]
    fn test_match_ellipsis_zero_multi_var() {
        let captures = do_match("((a b) ...)", "()").expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "a", &many(vec![]));
        assert_binding(&captures, "b", &many(vec![]));
    }

    #[test]
    fn test_match_nested_ellipsis_zero() {
        let captures = do_match("((a ...) ...)", "()").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![]));
    }

    #[test]
    fn test_match_ellipsis_with_prefix_and_elements() {
        let captures = do_match("(_ e1 e2 ...)", "(and a b c)").expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "e1", &one("a"));
        assert_binding(&captures, "e2", &many(vec![one("b"), one("c")]));
    }

    #[test]
    fn test_match_nested_ellipsis() {
        let captures = do_match("((a ...) ...)", "((x y) (z))").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(
            &captures,
            "a",
            &many(vec![many(vec![one("x"), one("y")]), many(vec![one("z")])]),
        );
    }

    #[test]
    fn test_match_nested_ellipsis_with_literals() {
        let captures =
            do_match("(((1 a) ...) ...)", "(((1 x) (1 y)) ((1 z)))").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(
            &captures,
            "a",
            &many(vec![many(vec![one("x"), one("y")]), many(vec![one("z")])]),
        );
    }

    #[test]
    fn test_match_nested_ellipsis_literal_mismatch() {
        assert!(do_match("(((1 a) ...) ...)", "(((2 x) (1 y)) ((1 z)))").is_none());
    }

    #[test]
    fn test_match_wildcard() {
        let captures = do_match("_", "42").expect("should match");
        assert!(captures.is_empty());

        let captures = do_match("_", "(a b c)").expect("should match");
        assert!(captures.is_empty());
    }

    #[test]
    fn test_match_variable_captures_list() {
        let captures = do_match("x", "(a b)").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "x", &one("(a b)"));
    }

    #[test]
    fn test_match_ellipsis_complex_repeat() {
        let captures = do_match("((a b) ...)", "((1 2) (3 4))").expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "a", &many(vec![one("1"), one("3")]));
        assert_binding(&captures, "b", &many(vec![one("2"), one("4")]));
    }

    #[test]
    fn test_match_and_macro_pattern() {
        // (_ e1 e2 ...) against (and a b)
        let captures = do_match("(_ e1 e2 ...)", "(and a b)").expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "e1", &one("a"));
        assert_binding(&captures, "e2", &many(vec![one("b")]));

        // (_ e1 e2 ...) against (and a b c d)
        let captures = do_match("(_ e1 e2 ...)", "(and a b c d)").expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "e1", &one("a"));
        assert_binding(&captures, "e2", &many(vec![one("b"), one("c"), one("d")]));
    }

    #[test]
    fn test_match_list_pattern_atom_target() {
        assert!(do_match("(a b)", "42").is_none());
    }

    #[test]
    fn test_match_ellipsis_pattern_atom_target() {
        assert!(do_match("(a ...)", "42").is_none());
    }

    #[test]
    fn test_match_vector_ellipsis() {
        let captures = do_match("#(a ...)", "#(1 2 3)").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![one("1"), one("2"), one("3")]));
    }

    #[test]
    fn test_match_vector_ellipsis_zero() {
        let captures = do_match("#(a ...)", "#()").expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "a", &many(vec![]));
    }

    #[test]
    fn test_match_vector_and_list_shape_mismatch() {
        assert!(do_match("#(a b)", "(1 2)").is_none());
        assert!(do_match("(a b)", "#(1 2)").is_none());
    }

    fn assert_renders_to(pattern: &str, template: &str, target: &str, expected: &str) {
        let rule = SyntaxRule::new(
            introduce_single_sexpr_src(pattern),
            introduce_single_sexpr_src(template),
            Rc::new(HashSet::new()),
        )
        .expect("invalid pattern in test");
        let captures = rule
            .match_pattern(
                &introduce_single_sexpr_src(target),
                &Bindings::new(Default::default()),
            )
            .expect("pattern should match target");
        let result = rule
            .render_template(&captures)
            .expect("render should succeed");
        assert_eq!(
            result.without_spans(),
            introduce_single_sexpr_src(expected).without_spans(),
            "render({template}, {captures:?}) = {result}, expected {expected}"
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
    fn test_render_vector_template_repetition_reproducer() {
        assert_renders_to("(_ x ...)", "(#(x) ...)", "(mac 1 2)", "(#(1) #(2))");
    }

    #[test]
    fn test_render_vector_internal_ellipsis() {
        assert_renders_to("(_ x ...)", "#(x ...)", "(mac 1 2 3)", "#(1 2 3)");
    }

    #[test]
    fn test_render_vector_internal_ellipsis_zero() {
        assert_renders_to("(_ x ...)", "#(x ...)", "(mac)", "#()");
    }

    #[test]
    fn test_render_vector_nested_ellipsis() {
        assert_renders_to(
            "(_ (x ...) ...)",
            "#((x ...) ...)",
            "(mac (1 2) (3))",
            "#((1 2) (3))",
        );
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

    fn nil() -> SExpr<Id> {
        SExpr::Nil(Span { lo: 0, hi: 0 })
    }

    fn make_rule(pattern: &str) -> Result<SyntaxRule> {
        SyntaxRule::new(
            introduce_single_sexpr_src(pattern),
            nil(),
            Rc::new(HashSet::new()),
        )
    }

    fn make_rule_with_literals(pattern: &str, literals: &[&str]) -> Result<SyntaxRule> {
        SyntaxRule::new(
            introduce_single_sexpr_src(pattern),
            nil(),
            Rc::new(literals.iter().map(|s| Symbol::new(s)).collect()),
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
        assert!(err.reason.contains("Duplicate pattern variable 'a'"));
    }

    #[test]
    fn test_new_duplicate_pattern_variable_nested() {
        let err = make_rule("(_ (a b) a)").unwrap_err();
        assert!(err.reason.contains("Duplicate pattern variable 'a'"));
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
    fn test_new_ellipsis_as_dotted_tail_after_ellipsis_rejected() {
        let err = make_rule("(_ a ... . ...)").unwrap_err();
        assert!(err.reason.contains("'...' is not allowed in this position"),);
    }

    #[test]
    fn test_new_duplicate_var_in_dotted_tail_after_ellipsis_rejected() {
        let err = make_rule("(_ a ... . a)").unwrap_err();
        assert!(err.reason.contains("Duplicate pattern variable 'a'"));
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
        assert!(err.reason.contains("'...' is not allowed in this position"));
    }

    #[test]
    fn test_new_ellipsis_as_first_element() {
        let err = make_rule("(... a)").unwrap_err();
        assert!(err.reason.contains("'...' is not allowed in this position"));
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

    #[test]
    fn test_match_literal_identifier_same_name() {
        let captures =
            do_match_with_literals("(_ foo e)", "(mac foo 42)", &["foo"]).expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "e", &one("42"));
    }

    #[test]
    fn test_match_literal_identifier_different_name() {
        assert!(do_match_with_literals("(_ foo e)", "(mac bar 42)", &["foo"]).is_none());
    }

    #[test]
    fn test_match_literal_identifier_vs_non_identifier() {
        assert!(do_match_with_literals("(_ foo e)", "(mac 42 x)", &["foo"]).is_none());
    }

    #[test]
    fn test_match_literal_identifier_vs_list() {
        assert!(do_match_with_literals("(_ foo e)", "(mac (a b) x)", &["foo"]).is_none());
    }

    #[test]
    fn test_match_non_literal_still_captures() {
        let captures =
            do_match_with_literals("(_ foo e)", "(mac bar 42)", &["other"]).expect("should match");
        assert_eq!(captures.len(), 2);
        assert_binding(&captures, "foo", &one("bar"));
        assert_binding(&captures, "e", &one("42"));
    }

    #[test]
    fn test_match_literal_in_ellipsis_subpattern() {
        let captures =
            do_match_with_literals("((_ foo e) ...)", "((mac foo 1) (mac foo 2))", &["foo"])
                .expect("should match");
        assert_eq!(captures.len(), 1);
        assert_binding(&captures, "e", &many(vec![one("1"), one("2")]));
    }

    #[test]
    fn test_match_literal_in_ellipsis_mismatch() {
        assert!(
            do_match_with_literals("((_ foo e) ...)", "((mac foo 1) (mac bar 2))", &["foo"])
                .is_none()
        );
    }

    fn make_transformer(src: &str) -> Transformer {
        Transformer::new(&introduce_single_sexpr_src(src)).unwrap()
    }

    fn transform(transformer: &Transformer, src: &str) -> SExpr<Id> {
        transformer
            .transform(
                &introduce_single_sexpr_src(src),
                &Bindings::new(Default::default()),
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
            introduce_single_sexpr_src("#f").without_spans()
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
            introduce_single_sexpr_src("x").without_spans()
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
            introduce_single_sexpr_src("(if a (and b) #f)").without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c)").without_spans(),
            introduce_single_sexpr_src("(if a (and b c) #f)").without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c d)").without_spans(),
            introduce_single_sexpr_src("(if a (and b c d) #f)").without_spans()
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
            introduce_single_sexpr_src("#f").without_spans()
        );
        assert_eq!(
            transform(&t, "(and x)").without_spans(),
            introduce_single_sexpr_src("x").without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b)").without_spans(),
            introduce_single_sexpr_src("(if a (and b) #f)").without_spans()
        );
        assert_eq!(
            transform(&t, "(and a b c d)").without_spans(),
            introduce_single_sexpr_src("(if a (and b c d) #f)").without_spans()
        );
    }

    #[test]
    fn test_transformer_keyword_ignored() {
        let t = make_transformer(
            "(syntax-rules ()
               ((foo e) e))",
        );
        // Even though pattern says "foo", application uses "bar"
        assert_eq!(
            transform(&t, "(bar x)").without_spans(),
            introduce_single_sexpr_src("x").without_spans()
        );
    }

    #[test]
    fn test_transformer_non_list_application() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x) x))",
        );
        assert!(
            t.transform(
                &introduce_single_sexpr_src("42"),
                &Bindings::new(Default::default()),
            )
            .is_none()
        );
    }

    #[test]
    fn test_transformer_zero_repetition_ellipsis() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x ...) (begin x ...)))",
        );
        assert_eq!(
            transform(&t, "(mac)").without_spans(),
            introduce_single_sexpr_src("(begin)").without_spans()
        );
    }

    #[test]
    fn test_transformer_vector_template_with_ellipsis() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ x ...) #(x ...)))",
        );
        assert_eq!(
            transform(&t, "(mac 1 2 3)").without_spans(),
            introduce_single_sexpr_src("#(1 2 3)").without_spans()
        );
        assert_eq!(
            transform(&t, "(mac)").without_spans(),
            introduce_single_sexpr_src("#()").without_spans()
        );
    }

    #[test]
    fn test_transformer_vector_pattern_matches_only_vectors() {
        let t = make_transformer(
            "(syntax-rules ()
               ((_ #(x ...)) (x ...)))",
        );
        assert_eq!(
            transform(&t, "(mac #(1 2 3))").without_spans(),
            introduce_single_sexpr_src("(1 2 3)").without_spans()
        );
        assert!(
            t.transform(
                &introduce_single_sexpr_src("(mac (1 2 3))"),
                &Bindings::new(Default::default()),
            )
            .is_none()
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
                &introduce_single_sexpr_src("(mac x)"),
                &Bindings::new(Default::default()),
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
            introduce_single_sexpr_src("1").without_spans()
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
            introduce_single_sexpr_src("(f x)").without_spans()
        );
        // Non-matching: `=>` in literal position doesn't match different identifier
        assert!(
            t.transform(
                &introduce_single_sexpr_src("(mac x y f)"),
                &Bindings::new(Default::default()),
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
            introduce_single_sexpr_src("(1 2 . 3)").without_spans()
        );
    }

    #[test]
    fn test_transformer_new_invalid_spec() {
        let result = Transformer::new(&introduce_single_sexpr_src("(syntax-rules)"));
        assert!(result.is_err());
    }

    #[test]
    fn test_transformer_new_non_proper_list_of_rules() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (a b c) ((_ x) x) . 3)",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected 'syntax-rules' rules to be a proper list")
        );
    }

    #[test]
    fn test_transformer_new_non_proper_list_of_literals() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (a b . c) ((_ x) x))",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected 'syntax-rules' literals to be a proper list")
        );
    }

    #[test]
    fn test_transformer_new_pattern_without_symbol_start() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (a b c) ((1 x) x))",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("'syntax-rules' pattern must start with an identifier, but got")
        );
    }

    #[test]
    fn test_transformer_new_no_rules() {
        let result = Transformer::new(&introduce_single_sexpr_src("(syntax-rules (a b c) )"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected 'syntax-rules' to have at least one rule")
        );
    }

    #[test]
    fn test_transformer_new_non_symbol_literal() {
        let result = Transformer::new(&introduce_single_sexpr_src("(syntax-rules (42) ((_ x) x))"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Expected an identifier in 'syntax-rules' literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_duplicate_pattern_var() {
        let result = Transformer::new(&introduce_single_sexpr_src("(syntax-rules () ((_ a a) a))"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("Duplicate pattern variable")
        );
    }

    #[test]
    fn test_transformer_new_rejects_ellipsis_in_literals() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (...) ((_ x) x))",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in 'syntax-rules' literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_underscore_in_literals() {
        let result = Transformer::new(&introduce_single_sexpr_src("(syntax-rules (_) ((_ x) x))"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in 'syntax-rules' literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_ellipsis_among_valid_literals() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (=> ...) ((_ x) x))",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in 'syntax-rules' literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_underscore_among_valid_literals() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules (=> _) ((_ x) x))",
        ));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .reason
                .contains("is not allowed in 'syntax-rules' literals")
        );
    }

    #[test]
    fn test_transformer_new_rejects_ellipsis_not_at_end() {
        let result = Transformer::new(&introduce_single_sexpr_src(
            "(syntax-rules () ((_ a ... b) a))",
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
