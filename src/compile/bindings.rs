use super::syntax::{Id, Symbol};
use std::collections::{BTreeSet, HashMap};

pub type ScopeId = u64;
pub type Scopes = BTreeSet<ScopeId>;

pub struct Bindings {
    symbols: HashMap<Symbol, Vec<(Scopes, Symbol)>>,
    cur_scope: ScopeId,
}

impl Bindings {
    pub const CORE_SCOPE: ScopeId = 0;

    pub fn new() -> Self {
        let mut bindings = Bindings {
            symbols: HashMap::new(),
            cur_scope: Self::CORE_SCOPE,
        };
        for symbol in ["lambda", "list", "cons", "first", "second", "rest"] {
            bindings.add_binding(
                &Id::with_scope(&symbol, [Self::CORE_SCOPE]),
                &Symbol::new(symbol),
            )
        }
        bindings
    }

    pub fn add_binding(&mut self, id: &Id, symbol: &Symbol) {
        let binding = self.symbols.entry(id.symbol.clone()).or_insert(vec![]);
        binding.push((id.scopes.clone(), symbol.clone()));
    }

    pub fn resolve(&self, id: &Id) -> Option<Symbol> {
        self.symbols
            .get_key_value(&id.symbol)
            .map(|(_, candidates)| {
                candidates
                    .into_iter()
                    .filter(|(scopes, _)| {
                        for scope in &id.scopes {
                            if !scopes.contains(scope) {
                                return false;
                            }
                        }
                        return id.scopes.len() >= scopes.len();
                    })
                    .max_by(|(lhs, _), (rhs, _)| lhs.len().cmp(&rhs.len()))
            })
            .flatten()
            .map(|(_, symbol)| symbol.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn test_resolve_with_empty_bindings() {
        let bindings = Bindings::new();
        assert_eq!(
            bindings.resolve(&Id {
                symbol: Symbol::new("a"),
                scopes: BTreeSet::new(),
            }),
            None
        );
    }

    #[test]
    fn test_resolve_with_single_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::new("a"), &Symbol::new("1"));
        assert_eq!(bindings.resolve(&Id::new("a")), Some(Symbol::new("1")));
        assert_eq!(bindings.resolve(&Id::new("b")), None);
    }

    #[test]
    fn test_resolve_with_multiple_stacked_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::with_scope("a", [1, 2]), &Symbol::new("middle"));
        bindings.add_binding(&Id::with_scope("a", [1, 2, 3]), &Symbol::new("inner"));
        bindings.add_binding(&Id::with_scope("a", [1]), &Symbol::new("outer"));
        assert_eq!(
            bindings.resolve(&Id::with_scope("a", [1, 2])),
            Some(Symbol::new("middle"))
        );
        assert_eq!(
            bindings.resolve(&Id::with_scope("a", [1])),
            Some(Symbol::new("outer"))
        );
        assert_eq!(bindings.resolve(&Id::with_scope("a", [2])), None);
        assert_eq!(bindings.resolve(&Id::new("a")), None);
    }

    #[test]
    fn test_resolve_with_multiple_single_bindings() {
        let mut bindings = Bindings::new();
        bindings.add_binding(&Id::with_scope("a", [3]), &Symbol::new("3"));
        bindings.add_binding(&Id::with_scope("a", [2]), &Symbol::new("2"));
        bindings.add_binding(&Id::with_scope("a", [1]), &Symbol::new("1"));
        assert_eq!(bindings.resolve(&Id::with_scope("a", [1, 2])), None);
        assert_eq!(
            bindings.resolve(&Id::with_scope("a", [1])),
            Some(Symbol::new("1"))
        );
        assert_eq!(
            bindings.resolve(&Id::with_scope("a", [2])),
            Some(Symbol::new("2"))
        );
        assert_eq!(bindings.resolve(&Id::new("a")), None);
    }
}
