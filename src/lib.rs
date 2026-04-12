pub mod compile;
pub mod prelude;

use compile::{
    bindings::Bindings, compilation_error::Result, pass::expand::Env, pass::read::token::Token,
    sexpr::SExpr,
};

use crate::{
    compile::{
        anf,
        bindings::Id,
        census::Census,
        core_expr,
        gensym::GenSym,
        ident::{Resolved, Symbol},
        pass::expand::introduce_scopes,
    },
    prelude::DERIVED_FORMS,
};

#[derive(Debug, Clone)]
pub struct Session {
    gen_sym: GenSym,
    bindings: Bindings,
    expander_env: Env,
    census: Census,
}

impl Session {
    pub fn new() -> Self {
        let gen_sym = GenSym::default();
        Self {
            bindings: Bindings::new(gen_sym.clone()),
            gen_sym,
            expander_env: Env::default(),
            census: Census::default(),
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

    pub fn resolve_sym(&self, id: &Id) -> Option<Symbol> {
        self.bindings.resolve_sym(id)
    }

    pub fn tokenize(&self, source: &str) -> Result<Vec<Token>> {
        compile::pass::read::lex::tokenize(source)
    }

    pub fn parse(&self, tokens: &[Token]) -> Result<Vec<SExpr<Symbol>>> {
        compile::pass::read::parse::parse(tokens)
    }

    pub fn introduce(&self, form: SExpr<Symbol>) -> SExpr<Id> {
        compile::pass::expand::introduce(form)
    }

    pub fn expand(&mut self, form: SExpr<Id>) -> Result<SExpr<Id>> {
        compile::pass::expand::expand(form, &mut self.bindings, &mut self.expander_env)
    }

    pub fn alpha_convert(&mut self, form: SExpr<Id>) -> SExpr<Resolved> {
        compile::pass::alpha_convert::alpha_convert(form, &mut self.bindings)
    }

    pub fn lower(&self, form: SExpr<Resolved>) -> core_expr::Expr {
        compile::pass::lower::lower(&self.gen_sym, form)
    }

    pub fn a_normalize(&mut self, form: core_expr::Expr) -> anf::Expr {
        let normalized = compile::pass::a_normalize::normalize(self.gen_sym.clone(), form);
        compile::pass::collect_census::collect(&normalized, &mut self.census);
        normalized
    }

    pub fn beta_contract(&mut self, form: anf::Expr) -> Result<anf::Expr> {
        let contracted = compile::pass::beta_contract::beta_contract(form, &self.census)?;
        compile::pass::collect_census::collect(&contracted, &mut self.census);
        Ok(contracted)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
