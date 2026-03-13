pub mod compile;

use std::{collections::HashMap, rc::Rc};

use compile::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{Id, SExpr, Symbol},
    token::Token,
    transformer::Transformer,
};

use crate::compile::prelude::PRELUDE;

#[derive(Debug, Clone)]
pub struct Session {
    bindings: Bindings,
    expander_env: HashMap<Symbol, Rc<Transformer>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            bindings: Bindings::new(),
            expander_env: HashMap::new(),
        }
    }

    pub fn with_prelude() -> Self {
        let mut session = Self::new();
        session.load_prelude();
        session
    }

    pub fn reset(&mut self) {
        *self = Self::with_prelude();
    }

    fn load_prelude(&mut self) {
        self.tokenize(PRELUDE)
            .and_then(|tokens| self.parse(&tokens))
            .and_then(|sexpr| self.expand(self.introduce(sexpr)))
            .expect("Unable to load prelude");
    }

    pub fn tokenize(&self, source: &str) -> Result<Vec<Token>> {
        compile::lex::tokenize(source)
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<SExpr<Symbol>> {
        compile::parse::parse(tokens)
    }

    pub fn introduce(&self, form: SExpr<Symbol>) -> SExpr<Id> {
        compile::expand::introduce(form)
    }

    pub fn expand(&mut self, form: SExpr<Id>) -> Result<SExpr<Id>> {
        compile::expand::expand(form, &mut self.bindings, &mut self.expander_env)
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
