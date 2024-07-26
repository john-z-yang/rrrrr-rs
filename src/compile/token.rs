use super::{
    sexpr::{Bool, Char, Num, Str, Symbol},
    src_loc::SourceLoc,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Id(Symbol, SourceLoc),
    Bool(Bool, SourceLoc),
    Num(Num, SourceLoc),
    Char(Char, SourceLoc),
    Str(Str, SourceLoc),
    HashLParen(SourceLoc),
    CommaAt(SourceLoc),
    Comma(SourceLoc),
    LParen(SourceLoc),
    RParen(SourceLoc),
    Quote(SourceLoc),
    Dot(SourceLoc),
    QuasiQuote(SourceLoc),
    Pipe(SourceLoc),
    EoF(SourceLoc),
}

impl Token {
    pub fn get_src_loc(&self) -> SourceLoc {
        match self {
            Token::Id(_, source_loc) => source_loc,
            Token::Bool(_, source_loc) => source_loc,
            Token::Num(_, source_loc) => source_loc,
            Token::Char(_, source_loc) => source_loc,
            Token::Str(_, source_loc) => source_loc,
            Token::HashLParen(source_loc) => source_loc,
            Token::CommaAt(source_loc) => source_loc,
            Token::Comma(source_loc) => source_loc,
            Token::LParen(source_loc) => source_loc,
            Token::RParen(source_loc) => source_loc,
            Token::Quote(source_loc) => source_loc,
            Token::Dot(source_loc) => source_loc,
            Token::QuasiQuote(source_loc) => source_loc,
            Token::Pipe(source_loc) => source_loc,
            Token::EoF(source_loc) => source_loc,
        }
        .clone()
    }
}
