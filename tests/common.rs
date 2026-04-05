use rrrrr_rs::compile::{
    compilation_error::Result,
    ident::Symbol,
    pass::read::{lex::tokenize, parse::parse},
    sexpr::SExpr,
};

pub fn parse_single_source(src: &str) -> Result<SExpr<Symbol>> {
    parse(&tokenize(src)?).map(|mut vec| {
        assert_eq!(
            vec.len(),
            1,
            "parse_single_source: expected 1 datum, got {}",
            vec.len()
        );
        vec.pop().unwrap()
    })
}
