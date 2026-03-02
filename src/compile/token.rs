use std::fmt;

use super::{
    sexpr::{Bool, Char, Num, Str, Symbol},
    span::Span,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Id(Symbol, Span),
    Bool(Bool, Span),
    Num(Num, Span),
    Char(Char, Span),
    Str(Str, Span),
    HashLParen(Span),
    CommaAt(Span),
    Comma(Span),
    LParen(Span),
    RParen(Span),
    Quote(Span),
    Dot(Span),
    QuasiQuote(Span),
    Pipe(Span),
    EoF(Span),
}

impl Token {
    pub fn get_span(&self) -> Span {
        *match self {
            Token::Id(_, span) => span,
            Token::Bool(_, span) => span,
            Token::Num(_, span) => span,
            Token::Char(_, span) => span,
            Token::Str(_, span) => span,
            Token::HashLParen(span) => span,
            Token::CommaAt(span) => span,
            Token::Comma(span) => span,
            Token::LParen(span) => span,
            Token::RParen(span) => span,
            Token::Quote(span) => span,
            Token::Dot(span) => span,
            Token::QuasiQuote(span) => span,
            Token::Pipe(span) => span,
            Token::EoF(span) => span,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Id(symbol, _) => write!(f, "{}", symbol),
            Token::Bool(bool, _) => write!(f, "{}", bool),
            Token::Num(num, _) => write!(f, "{}", num),
            Token::Char(char, _) => write!(f, "{}", char),
            Token::Str(str, _) => write!(f, "{}", str),
            Token::HashLParen(_) => write!(f, "#("),
            Token::CommaAt(_) => write!(f, ",@"),
            Token::Comma(_) => write!(f, ","),
            Token::LParen(_) => write!(f, "("),
            Token::RParen(_) => write!(f, ")"),
            Token::Quote(_) => write!(f, "'"),
            Token::Dot(_) => write!(f, "."),
            Token::QuasiQuote(_) => write!(f, "`"),
            Token::Pipe(_) => write!(f, "|"),
            Token::EoF(_) => write!(f, "EoF"),
        }
    }
}
