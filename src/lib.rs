pub mod compile;

use std::collections::HashMap;

use compile::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{SExpr, Symbol},
    token::Token,
    transformer::Transformer,
};

#[derive(Debug, Clone)]
pub struct Session {
    bindings: Bindings,
    env: HashMap<Symbol, Transformer>,
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
        compile::sema::introduce(form)
    }

    pub fn expand(&mut self, form: &SExpr) -> Result<SExpr> {
        compile::sema::expand(form, &mut self.bindings, &mut self.env)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
