use std::{iter::Peekable, slice::Iter};

use super::{compliation_error::CompliationError, sexpr::SExpr, token::Token};
use crate::compile::sexpr::Id;
use crate::compile::src_loc::SourceLoc;
use crate::sexpr;

pub fn parse(tokens: &[Token]) -> Result<SExpr, CompliationError> {
    struct Parser<'tokens> {
        it: Peekable<Iter<'tokens, Token>>,
    }

    impl Parser<'_> {
        fn new(tokens: &[Token]) -> Parser {
            Parser {
                it: tokens.iter().peekable(),
            }
        }
        fn parse(&mut self) -> Result<SExpr, CompliationError> {
            let res = self.parse_datum()?;
            match self.advance() {
                Token::EoF(_) => Ok(res),
                _ => Err(self.emit_err("Expected EoF")),
            }
        }
        fn parse_datum(&mut self) -> Result<SExpr, CompliationError> {
            match self.look_ahead() {
                Some(
                    Token::Id(_, _)
                    | Token::Bool(_, _)
                    | Token::Num(_, _)
                    | Token::Char(_, _)
                    | Token::Str(_, _),
                ) => Ok(self.parse_atom()),
                _ => self.parse_compound(),
            }
        }
        fn parse_atom(&mut self) -> SExpr {
            match self.advance() {
                Token::Id(symbol, _) => symbol.clone().into(),
                Token::Bool(bool, _) => bool.clone().into(),
                Token::Num(num, _) => num.clone().into(),
                Token::Char(char, _) => char.clone().into(),
                Token::Str(string, _) => string.clone().into(),
                _ => unreachable!("parse_atom is only expecting tokens for atomic values"),
            }
        }
        fn parse_compound(&mut self) -> Result<SExpr, CompliationError> {
            match self.look_ahead() {
                Some(Token::HashLParen(_)) => self.parse_vector(),
                _ => self.parse_list(),
            }
        }
        fn parse_list(&mut self) -> Result<SExpr, CompliationError> {
            assert!(
                matches!(
                    self.look_ahead(),
                    Some(
                        Token::LParen(_)
                            | Token::Quote(_)
                            | Token::QuasiQuote(_)
                            | Token::Comma(_)
                            | Token::CommaAt(_)
                    )
                ),
                "parse_list is expecting either '(' or an abbreviation prefix, but got: {:?}",
                self.look_ahead()
            );

            if matches!(
                self.look_ahead(),
                Some(Token::Quote(_) | Token::QuasiQuote(_) | Token::Comma(_) | Token::CommaAt(_))
            ) {
                return self.parse_abbreviation();
            }

            self.advance();
            let mut elements: Vec<SExpr> = vec![];
            while let Some(t) = self.look_ahead() {
                if matches!(t, Token::RParen(_)) || matches!(t, Token::Dot(_)) {
                    break;
                }
                elements.push(self.parse_datum()?);
            }
            if matches!(self.look_ahead(), Some(Token::Dot(_))) {
                self.advance();
                elements.push(self.parse_datum()?);
                if !matches!(self.look_ahead(), Some(Token::RParen(_))) {
                    Err(self.emit_err("Expected ')' after dotted datum"))?
                }
                Ok(SExpr::make_improper_list(&elements))
            } else if matches!(self.look_ahead(), Some(Token::RParen(_))) {
                self.advance();
                Ok(SExpr::make_list(&elements))
            } else {
                Err(self.emit_err("Expected ')' to close '('"))?
            }
        }
        fn parse_abbreviation(&mut self) -> Result<SExpr, CompliationError> {
            let op = self.parse_prefix();
            Ok(sexpr!(op, self.parse_datum()?))
        }
        fn parse_prefix(&mut self) -> SExpr {
            match self.advance() {
                Token::Quote(_) => SExpr::from(Id::new("quote", [])),
                Token::QuasiQuote(_) => SExpr::from(Id::new("quasiquote", [])),
                Token::Comma(_) => SExpr::from(Id::new("unquote", [])),
                Token::CommaAt(_) => SExpr::from(Id::new("unquote-splicing", [])),
                _ => unreachable!(
                    "parse_abbreviation is only expecting tokens for abbreviated prefix"
                ),
            }
        }
        fn parse_vector(&mut self) -> Result<SExpr, CompliationError> {
            assert!(
                matches!(self.advance(), Token::HashLParen(_)),
                "parse_vector is expecting the '#(' token"
            );
            let mut elements: Vec<SExpr> = vec![];
            while let Some(t) = self.look_ahead() {
                if matches!(t, Token::RParen(_)) {
                    break;
                }
                elements.push(self.parse_datum()?);
            }
            if matches!(self.look_ahead(), Some(Token::RParen(_))) {
                self.advance();
                Ok(SExpr::from(&*elements))
            } else {
                Err(self.emit_err("Expected ')' to close '#('"))?
            }
        }
        fn look_ahead(&mut self) -> Option<&Token> {
            self.it.peek().copied()
        }
        fn advance(&mut self) -> &Token {
            self.it.next().unwrap()
        }
        fn get_src_loc(&mut self) -> SourceLoc {
            (*self.it.peek().unwrap()).get_src_loc()
        }
        fn emit_err(&mut self, reason: &str) -> CompliationError {
            CompliationError {
                source_loc: self.get_src_loc(),
                reason: reason.to_owned(),
            }
        }
    }
    Parser::new(tokens).parse()
}
