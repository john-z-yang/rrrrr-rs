pub mod compile;

use std::{collections::HashMap, sync::Arc};

use compile::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{Id, SExpr, Symbol},
    token::Token,
    transformer::Transformer,
};

#[derive(Debug, Clone)]
pub struct Session {
    bindings: Bindings,
    env: HashMap<Symbol, Arc<Transformer>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            bindings: Bindings::new(),
            env: HashMap::new(),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn tokenize(&self, source: &str) -> Result<Vec<Token>> {
        compile::lex::tokenize(source)
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<SExpr> {
        compile::parse::parse(tokens)
    }

    pub fn introduce(&self, form: &SExpr) -> SExpr {
        compile::expand::introduce(form)
    }

    pub fn expand(&mut self, form: &SExpr) -> Result<SExpr> {
        compile::expand::expand(form, &mut self.bindings, &mut self.env)
    }

    pub fn resolve_sym(&self, id: &Id) -> Option<Symbol> {
        self.bindings.resolve_sym(id)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
