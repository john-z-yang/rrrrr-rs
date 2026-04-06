use std::{cell::Cell, rc::Rc};

use crate::compile::ident::Symbol;

#[derive(Clone, Debug, Default)]
pub(crate) struct GenSym {
    counter: Rc<Cell<u64>>,
}

impl GenSym {
    pub(crate) fn fresh(&self, hint: &str) -> Symbol {
        self.counter.set(self.counter.get() + 1);
        Symbol(format!("{}:{}", hint, self.counter.get()))
    }
}
