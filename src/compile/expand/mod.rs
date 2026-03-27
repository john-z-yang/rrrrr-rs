mod body;
mod definition;
mod dispatch;
mod expression;
mod quote;
mod transformer;

#[cfg(test)]
mod tests;

use std::{collections::HashMap, mem, rc::Rc};

use self::transformer::Transformer;
use self::dispatch::{apply_transformer, expand_sexpr};
use super::{
    bindings::Bindings,
    compilation_error::Result,
    sexpr::{Id, SExpr, Symbol},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct Env {
    transformers: HashMap<Symbol, Rc<Transformer>>,
}

impl Env {
    fn get(&self, symbol: &Symbol) -> Option<&Transformer> {
        self.transformers.get(symbol).map(Rc::as_ref)
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.transformers.is_empty()
    }

    fn insert(&mut self, symbol: Symbol, transformer: Transformer) {
        self.transformers.insert(symbol, Rc::new(transformer));
    }
}

impl<const N: usize> From<[(Symbol, Transformer); N]> for Env {
    fn from(transformers: [(Symbol, Transformer); N]) -> Self {
        Self {
            transformers: transformers
                .into_iter()
                .map(|(symbol, transformer)| (symbol, Rc::new(transformer)))
                .collect(),
        }
    }
}

const MAX_MACRO_DEPTH: u16 = 1024;

pub fn introduce(sexpr: SExpr<Symbol>) -> SExpr<Id> {
    sexpr.map_var(&|symbol| Id {
        symbol,
        scopes: std::collections::BTreeSet::from([Bindings::CORE_SCOPE]),
    })
}

pub(crate) fn expand(
    sexpr: SExpr<Id>,
    bindings: &mut Bindings,
    env: &mut Env,
) -> Result<SExpr<Id>> {
    let mut bindings_snapshot = bindings.clone();
    let mut env_snapshot = env.clone();
    let result = expand_sexpr(sexpr, bindings, env, Context::new(SyntaxContext::TopLevel));
    if result.is_err() {
        mem::swap(&mut bindings_snapshot, bindings);
        mem::swap(&mut env_snapshot, env);
    }
    result
}

#[derive(PartialEq, Clone, Copy, Eq, Hash, Debug)]
struct Context {
    syntax_ctx: SyntaxContext,
    depth: u16,
}

impl Context {
    fn new(syntax_ctx: SyntaxContext) -> Self {
        Self {
            syntax_ctx,
            depth: 0,
        }
    }

    fn with_syntax_ctx(self, syntax_ctx: SyntaxContext) -> Self {
        Self { syntax_ctx, ..self }
    }

    fn increment_depth(&mut self) {
        self.depth += 1;
    }
}

#[derive(PartialEq, Clone, Copy, Eq, Hash, Debug)]
enum SyntaxContext {
    TopLevel,
    Expression,
    Body,
}

