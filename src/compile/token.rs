use super::{
    sexpr::{Bool, Char, Num, Symbol},
    src_loc::SourceLoc,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Id(Symbol, SourceLoc),
    Bool(Bool, SourceLoc),
    Num(Num, SourceLoc),
    Char(Char, SourceLoc),
    String(String, SourceLoc),
    HashLParen(SourceLoc),
    CommaAt(SourceLoc),
    Comma(SourceLoc),
    LParen(SourceLoc),
    RParen(SourceLoc),
    Quote(SourceLoc),
    Dot(SourceLoc),
    QuasiQuote(SourceLoc),
    Pipe(SourceLoc),
    EoF(),
}
