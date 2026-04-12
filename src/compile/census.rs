use std::collections::HashMap;

use crate::compile::ident::Symbol;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Census {
    data: HashMap<Symbol, VarMeta>,
}

impl Census {
    pub(crate) fn track_use(&mut self, symbol: &Symbol) {
        self.data.entry(symbol.clone()).or_default().use_count += 1;
    }

    pub(crate) fn track_rebound(&mut self, symbol: &Symbol) {
        self.data.entry(symbol.clone()).or_default().is_rebound = true;
    }

    pub(crate) fn use_count(&self, symbol: &Symbol) -> usize {
        self.data
            .get(symbol)
            .map(|var_meta| var_meta.use_count)
            .unwrap_or_default()
    }

    pub(crate) fn is_rebound(&self, symbol: &Symbol) -> bool {
        self.data
            .get(symbol)
            .map(|var_meta| var_meta.is_rebound)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct VarMeta {
    pub(crate) use_count: usize,
    pub(crate) is_rebound: bool,
}
