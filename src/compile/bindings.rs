use super::syntax::{Id, Symbol};
use std::collections::{BTreeSet, HashMap};

pub type ScopeId = u64;
pub type Scopes = BTreeSet<ScopeId>;

pub struct Bindings {
    symbols: HashMap<Symbol, Vec<(Scopes, Symbol)>>,
    scope_counter: ScopeId,
    gen_sym_counter: u64,
}

impl Bindings {
    pub const CORE_SCOPE: ScopeId = 0;

    pub fn new() -> Self {
        let mut bindings = Bindings {
            symbols: HashMap::new(),
            scope_counter: Self::CORE_SCOPE,
            gen_sym_counter: 0,
        };
        for symbol in ["if", "lambda", "list", "cons", "first", "second", "rest"] {
            bindings.add_binding(&Id::new(symbol, [Self::CORE_SCOPE]), &Symbol::new(symbol))
        }
        bindings
    }

    pub fn new_scope_id(&mut self) -> ScopeId {
        self.scope_counter += 1;
        self.scope_counter
    }

    pub fn gen_sym(&mut self) -> Symbol {
        self.gen_sym_counter += 1;
        Symbol(format!("gensym:{0}", self.gen_sym_counter))
    }

    pub fn add_binding(&mut self, id: &Id, symbol: &Symbol) {
        let binding = self.symbols.entry(id.symbol.clone()).or_default();
        binding.push((id.scopes.clone(), symbol.clone()));
    }

    pub fn resolve(&self, id: &Id) -> Option<Id> {
        self.symbols
            .get_key_value(&id.symbol)
            .and_then(|(_, candidates)| {
                candidates
                    .iter()
                    .filter(|(scopes, _)| {
                        for scope in scopes {
                            if !id.scopes.contains(scope) {
                                return false;
                            }
                        }
                        id.scopes.len() >= scopes.len()
                    })
                    .max_by(|(lhs, _), (rhs, _)| lhs.len().cmp(&rhs.len()))
            })
            .map(|(scopes, symbol)| Id {
                scopes: scopes.clone(),
                symbol: symbol.clone(),
            })
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
        assert_eq!(bindings.resolve(&Id::new("a", [])), Some(Id::new("1", [])));
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
            Some(Id::new("middle", [1, 2]))
        );
        assert_eq!(
            bindings.resolve(&Id::new("a", [1])),
            Some(Id::new("outer", [1]))
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
            Some(Id::new("1", [1]))
        );
        assert_eq!(
            bindings.resolve(&Id::new("a", [1])),
            Some(Id::new("1", [1]))
        );
        assert_eq!(
            bindings.resolve(&Id::new("a", [2])),
            Some(Id::new("2", [2]))
        );
        assert_eq!(bindings.resolve(&Id::new("a", [])), None);
    }

    #[test]
    fn test_resolve_with_core_bindings() {
        let bindings = Bindings::new();
        assert_eq!(
            bindings.resolve(&Id::new("lambda", [Bindings::CORE_SCOPE])),
            Some(Id::new("lambda", [Bindings::CORE_SCOPE]))
        );
    }
}
