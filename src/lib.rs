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
        core_expr,
        gensym::GenSym,
        ident::{ResolvedSymbol, Symbol},
        pass::expand::introduce_scopes,
    },
    prelude::DERIVED_FORMS,
};

#[derive(Debug, Clone)]
pub struct Session {
    gen_sym: GenSym,
    bindings: Bindings,
    expander_env: Env,
}

impl Session {
    pub fn new() -> Self {
        let gen_sym = GenSym::default();
        Self {
            bindings: Bindings::new(gen_sym.clone()),
            gen_sym,
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

    pub fn alpha_convert(&mut self, form: SExpr<Id>) -> SExpr<ResolvedSymbol> {
        compile::pass::alpha_conversion::alpha_convert(form, &mut self.bindings)
    }

    pub fn lower(&self, form: SExpr<ResolvedSymbol>) -> core_expr::Expr {
        compile::pass::lower::lower(&self.gen_sym, form)
    }

    pub fn a_normalize(&self, form: core_expr::Expr) -> anf::Expr {
        compile::pass::a_normalization::a_normalize(self.gen_sym.clone(), form)
    }

    pub fn propagate_copies(&self, form: anf::Expr) -> anf::Expr {
        compile::pass::copy_propagation::propagate_copies(form)
    }

    pub fn beta_contract(&self, form: anf::Expr) -> Result<anf::Expr> {
        compile::pass::beta_contraction::beta_contract(form)
    }

    pub fn dce(&self, form: anf::Expr) -> anf::Expr {
        compile::pass::dce::dce(form)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
