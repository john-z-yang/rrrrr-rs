use super::sexpr::{Id, Symbol};
use std::collections::{BTreeSet, HashMap};

pub(crate) type ScopeId = u64;
pub(crate) type Scopes = BTreeSet<ScopeId>;

pub(crate) struct Bindings {
    symbols: HashMap<Symbol, Vec<(Scopes, Symbol)>>,
    scope_counter: ScopeId,
    gen_sym_counter: u64,
}

impl Bindings {
    pub(crate) const CORE_SCOPE: ScopeId = 0;

    pub(crate) const CORE_BINDINGS: &[&str] = &[
        "letrec-syntax",
        "quote",
        "quote-syntax",
        "if",
        "lambda",
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

    pub(crate) fn gen_sym(&mut self) -> Symbol {
        self.gen_sym_counter += 1;
        Symbol(format!("gensym:{0}", self.gen_sym_counter))
    }

    pub(crate) fn add_binding(&mut self, id: &Id, symbol: &Symbol) {
        let binding = self.symbols.entry(id.symbol.clone()).or_default();
        binding.push((id.scopes.clone(), symbol.clone()));
    }

    pub(crate) fn resolve(&self, id: &Id) -> Option<Symbol> {
        self.symbols
            .get_key_value(&id.symbol)
            .and_then(|(_, candidates)| {
                candidates
                    .iter()
                    .filter(|(candidate_scopes, _)| candidate_scopes.is_subset(&id.scopes))
                    .max_by(|(lhs, _), (rhs, _)| lhs.len().cmp(&rhs.len()))
            })
            .map(|(_, symbol)| symbol.clone())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_resolve_with_empty_bindings() {
        let bindings = Bindings::new();
        assert_eq!(bindings.resolve(&Id::new("a", [])), None);
    }

    #[test]
    fn test_resolve_with_single_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", []), &Symbol::new("1"));
        assert_eq!(bindings.resolve(&Id::new("a", [])), Some(Symbol::new("1")));
        assert_eq!(bindings.resolve(&Id::new("b", [])), None);
    }

    #[test]
    fn test_resolve_with_multiple_stacked_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", [1, 2]), &Symbol::new("middle"));
        bindings.add_binding(&Id::new("a", [1, 2, 3]), &Symbol::new("inner"));
        bindings.add_binding(&Id::new("a", [1]), &Symbol::new("outer"));
        assert_eq!(
            bindings.resolve(&Id::new("a", [1, 2, 4])),
            Some(Symbol::new("middle"))
        );
        assert_eq!(
            bindings.resolve(&Id::new("a", [1])),
            Some(Symbol::new("outer"))
        );
        assert_eq!(bindings.resolve(&Id::new("a", [2])), None);
        assert_eq!(bindings.resolve(&Id::new("a", [])), None);
    }

    #[test]
    fn test_resolve_with_multiple_single_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a", [3]), &Symbol::new("3"));
        bindings.add_binding(&Id::new("a", [2]), &Symbol::new("2"));
        bindings.add_binding(&Id::new("a", [1]), &Symbol::new("1"));
        assert_eq!(
            bindings.resolve(&Id::new("a", [1, 2])),
            Some(Symbol::new("1"))
        );
        assert_eq!(bindings.resolve(&Id::new("a", [1])), Some(Symbol::new("1")));
        assert_eq!(bindings.resolve(&Id::new("a", [2])), Some(Symbol::new("2")));
        assert_eq!(bindings.resolve(&Id::new("a", [])), None);
    }

    #[test]
    fn test_resolve_with_core_bindings() {
        let bindings = Bindings::new();
        for core_binding in Bindings::CORE_BINDINGS {
            assert_eq!(
                bindings.resolve(&Id::new(core_binding, [Bindings::CORE_SCOPE])),
                Some(Symbol::new(core_binding))
            );
        }
    }
}
