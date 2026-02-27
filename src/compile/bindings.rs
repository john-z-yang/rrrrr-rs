use super::sexpr::{Id, Symbol};
use std::collections::{BTreeSet, HashMap};

pub(crate) type ScopeId = u64;
pub(crate) type Scopes = BTreeSet<ScopeId>;

#[derive(Debug, Clone)]
pub(crate) struct Bindings {
    symbols: HashMap<Symbol, HashMap<Scopes, Symbol>>,
    scope_counter: ScopeId,
    gen_sym_counter: u64,
}

impl Bindings {
    pub(crate) const CORE_SCOPE: ScopeId = 0;

    pub(crate) const CORE_BINDINGS: &[&str] = &[
        "let-syntax",
        "letrec-syntax",
        "syntax-rules",
        "quote",
        "quote-syntax",
        "if",
        "lambda",
        "define",
        "define-syntax",
        "set!",
        "begin",
        "list",
        "cons",
        "first",
        "second",
        "rest",
    ];

    pub(crate) fn new() -> Self {
        let mut bindings = Bindings {
            symbols: HashMap::new(),
            scope_counter: Self::CORE_SCOPE,
            gen_sym_counter: 0,
        };
        for symbol in Self::CORE_BINDINGS {
            bindings.add_binding(&Id::new(symbol, [Self::CORE_SCOPE]), &Symbol::new(symbol))
        }
        bindings
    }

    pub(crate) fn new_scope_id(&mut self) -> ScopeId {
        self.scope_counter += 1;
        self.scope_counter
    }

    pub(crate) fn gen_sym(&mut self, hint: &Id) -> Symbol {
        self.gen_sym_counter += 1;
        Symbol(format!("gensym_{}:{}", self.gen_sym_counter, hint))
    }

    pub(crate) fn add_binding(&mut self, id: &Id, symbol: &Symbol) {
        self.symbols
            .entry(id.symbol.clone())
            .or_default()
            .insert(id.scopes.clone(), symbol.clone());
    }

    pub(crate) fn resolve(&self, id: &Id) -> Option<Id> {
        self.symbols
            .get_key_value(&id.symbol)
            .and_then(|(_, candidates)| {
                candidates
                    .iter()
                    .filter(|(candidate_scopes, _)| candidate_scopes.is_subset(&id.scopes))
                    .max_by(|(lhs, _), (rhs, _)| lhs.len().cmp(&rhs.len()))
            })
            .map(|(scopes, symbol)| Id {
                symbol: symbol.clone(),
                scopes: scopes.clone(),
            })
    }

    pub(crate) fn resolve_scopes(&self, id: &Id) -> Option<Scopes> {
        self.resolve(id).map(|id| id.scopes)
    }

    pub(crate) fn resolve_sym(&self, id: &Id) -> Option<Symbol> {
        self.resolve(id).map(|id| id.symbol)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_resolve_with_empty_bindings() {
        let bindings = Bindings::new();
        assert_eq!(bindings.resolve_sym(&Id::new("a", [])), None);
    }

    #[test]
    fn test_resolve_with_single_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", []), &Symbol::new("1"));
        assert_eq!(
            bindings.resolve_sym(&Id::new("a", [])),
            Some(Symbol::new("1"))
        );
        assert_eq!(bindings.resolve_sym(&Id::new("b", [])), None);
    }

    #[test]
    fn test_resolve_with_multiple_stacked_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", [1, 2]), &Symbol::new("middle"));
        bindings.add_binding(&Id::new("a", [1, 2, 3]), &Symbol::new("inner"));
        bindings.add_binding(&Id::new("a", [1]), &Symbol::new("outer"));
        assert_eq!(
            bindings.resolve_sym(&Id::new("a", [1, 2, 4])),
            Some(Symbol::new("middle"))
        );
        assert_eq!(
            bindings.resolve_sym(&Id::new("a", [1])),
            Some(Symbol::new("outer"))
        );
        assert_eq!(bindings.resolve_sym(&Id::new("a", [2])), None);
        assert_eq!(bindings.resolve_sym(&Id::new("a", [])), None);
    }

    #[test]
    fn test_gen_sym() {
        let mut bindings = Bindings::new();
        let gen_sym_1 = bindings.gen_sym(&Id::new("foo", [1]));
        let gen_sym_2 = bindings.gen_sym(&Id::new("bar", [1]));
        let gen_sym_3 = bindings.gen_sym(&Id::new("foo", [1, 2]));
        assert_eq!(gen_sym_1, Symbol::new("gensym_1:foo"));
        assert_eq!(gen_sym_2, Symbol::new("gensym_2:bar"));
        assert_eq!(gen_sym_3, Symbol::new("gensym_3:foo"));
    }

    #[test]
    fn test_add_binding_overwrites_with_same_scopes() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", [1, 2]), &Symbol::new("first"));
        assert_eq!(
            bindings.resolve_sym(&Id::new("a", [1, 2])),
            Some(Symbol::new("first"))
        );
        bindings.add_binding(&Id::new("a", [1, 2]), &Symbol::new("second"));
        assert_eq!(
            bindings.resolve_sym(&Id::new("a", [1, 2])),
            Some(Symbol::new("second"))
        );
    }

    #[test]
    fn test_resolve_with_core_bindings() {
        let bindings = Bindings::new();
        for core_binding in Bindings::CORE_BINDINGS {
            assert_eq!(
                bindings.resolve_sym(&Id::new(core_binding, [Bindings::CORE_SCOPE])),
                Some(Symbol::new(core_binding))
            );
        }
    }
}
