pub mod compile;
pub mod prelude;

use compile::{
    bindings::Bindings, compilation_error::Result, expand::Env, read::token::Token, sexpr::SExpr,
};

use crate::{
    compile::{
        ast::Expr,
        bindings::Id,
        expand::introduce_scopes,
        ident::{Resolved, Symbol},
    },
    prelude::DERIVED_FORMS,
};

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
        self.tokenize(DERIVED_FORMS)
            .and_then(|tokens| self.parse(&tokens))
            .and_then(|sexprs| {
                sexprs
                    .into_iter()
                    .map(|sexpr| self.expand(introduce_scopes(sexpr, [Bindings::CORE_SCOPE])))
                    .collect::<Result<Vec<_>>>()
            })
            .expect("Unable to load prelude");
    }

    pub fn tokenize(&self, source: &str) -> Result<Vec<Token>> {
        compile::read::lex::tokenize(source)
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<Vec<SExpr<Symbol>>> {
        compile::read::parse::parse(tokens)
    }

    pub fn introduce(&self, form: SExpr<Symbol>) -> SExpr<Id> {
        compile::expand::introduce(form)
    }

    pub fn expand(&mut self, form: SExpr<Id>) -> Result<SExpr<Id>> {
        compile::expand::expand(form, &mut self.bindings, &mut self.expander_env)
    }

    pub fn alpha_reduce(&mut self, form: SExpr<Id>) -> SExpr<Resolved> {
        compile::alpha_reduce::alpha_reduce(form, &self.bindings)
    }

    pub fn resolve_sym(&self, id: &Id) -> Option<Symbol> {
        self.bindings.resolve_sym(id)
    }

    pub fn lower(&self, form: SExpr<Resolved>) -> Expr {
        compile::lower::lower(form)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
