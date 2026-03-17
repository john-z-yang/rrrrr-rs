pub mod compile;

use compile::{
    bindings::Bindings,
    compilation_error::Result,
    expand::Env,
    read::token::Token,
    sexpr::{Id, SExpr, Symbol},
};

use crate::compile::prelude::PRELUDE;

#[derive(Debug, Clone)]
pub struct Session {
    bindings: Bindings,
    expander_env: Env,
}

impl Session {
    pub fn new() -> Self {
        Self {
            bindings: Bindings::new(),
            expander_env: Env::default(),
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
        compile::read::lex::tokenize(source)
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<SExpr<Symbol>> {
        compile::read::parse::parse(tokens)
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
