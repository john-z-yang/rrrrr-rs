use std::{iter::Peekable, slice::Iter};

use super::{compliation_error::CompliationError, sexpr::SExpr, token::Token};
use crate::compile::sexpr::Id;
use crate::sexpr;

pub fn parse(tokens: &[Token]) -> Result<SExpr, CompliationError> {
    struct Parser<'tokens> {
        it: Peekable<Iter<'tokens, Token>>,
        cur: &'tokens Token,
    }

    impl Parser<'_> {
        fn new(tokens: &[Token]) -> Parser {
            assert_ne!(tokens.len(), 0, "Token stream must have at least 1 token");
            Parser {
                it: tokens.iter().peekable(),
                cur: &tokens[0],
            }
        }
        fn parse(&mut self) -> Result<SExpr, CompliationError> {
            let res = self.parse_datum()?;
            match self.look_ahead() {
                Some(Token::EoF(_)) => Ok(res),
                Some(token) => Err(self.emit_err("Expecting EoF", token)),
                None => unreachable!("parse is expecting token stream to end with the EoF token"),
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
                Some(
                    Token::LParen(_)
                    | Token::HashLParen(_)
                    | Token::Quote(_)
                    | Token::QuasiQuote(_)
                    | Token::Comma(_)
                    | Token::CommaAt(_),
                ) => self.parse_compound(),
                Some(token) => Err(self.emit_err("Expecting a datum", token)),
                _ => unreachable!("parse_datum is expecting at least 1 token to look ahead"),
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
                "parse_list is expecting either '(' or an abbreviation prefix",
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
            match self.look_ahead() {
                Some(Token::Dot(_)) => {
                    elements.push(self.parse_dot_notation()?);
                    Ok(SExpr::make_improper_list(&elements))
                }
                Some(Token::RParen(_)) => {
                    self.advance();
                    Ok(SExpr::make_list(&elements))
                }
                Some(token) => Err(self.emit_err("Expected ')' to close '('", token)),
                None => {
                    unreachable!("parse_datum is expecting token stream to end with the EoF token")
                }
            }
        }
        fn parse_dot_notation(&mut self) -> Result<SExpr, CompliationError> {
            assert!(
                matches!(self.advance(), Token::Dot(_)),
                "parse_dot_notation is expecting the '.' token"
            );
            match self.look_ahead() {
                Some(Token::RParen(_)) => Ok(self.parse_datum()?),
                Some(token) => Err(self.emit_err("Expected ')' after dotted datum", token)),
                None => unreachable!("parse_datum is expecting at least 1 token to look ahead"),
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
            match self.look_ahead() {
                Some(Token::RParen(_)) => {
                    self.advance();
                    Ok(SExpr::from(&*elements))
                }
                Some(token) => Err(self.emit_err("Expected ')' to close '#('", token)),
                None => unreachable!("parse_datum is expecting at least 1 token to look ahead"),
            }
        }
        fn look_ahead(&mut self) -> Option<Token> {
            self.it.peek().copied().cloned()
        }
        fn advance(&mut self) -> &Token {
            let token = self.it.next().unwrap();
            self.cur = token;
            token
        }
        fn emit_err(&self, reason: &str, token: Token) -> CompliationError {
            CompliationError {
                source_loc: token.get_src_loc(),
                reason: format!("{}, but got: {}", reason.to_owned(), token),
            }
        }
    }
    Parser::new(tokens).parse()
}
