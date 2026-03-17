use std::{iter::Peekable, slice::Iter};

use super::token::Token;
use crate::compile::{
    compilation_error::{CompilationError, Result},
    sexpr::{SExpr, Symbol, Vector},
    span::Span,
};

pub fn parse(tokens: &[Token]) -> Result<SExpr<Symbol>> {
    Parser::new(tokens)?.parse()
}

struct Parser<'tokens> {
    it: Peekable<Iter<'tokens, Token>>,
    cur: &'tokens Token,
}

impl Parser<'_> {
    fn new(tokens: &'_ [Token]) -> Result<Parser<'_>> {
        if tokens.is_empty() {
            Err(CompilationError {
                span: Span { lo: 0, hi: 0 },
                reason: "Token stream must have at least 1 token".to_owned(),
            })
        } else if !matches!(tokens.last().unwrap(), Token::EoF(_)) {
            Err(CompilationError {
                span: tokens.last().unwrap().get_span(),
                reason: "Token stream must end with the EOF token".to_owned(),
            })
        } else {
            Ok(Parser {
                it: tokens.iter().peekable(),
                cur: &tokens[0],
            })
        }
    }

    fn parse(&mut self) -> Result<SExpr<Symbol>> {
        let res = self.parse_datum()?;
        match self.look_ahead() {
            Some(Token::EoF(_)) => Ok(res),
            Some(token) => Err(self.emit_err("Expected end of input", token)),
            None => unreachable!("parse expected token stream to end with the EoF token"),
        }
    }

    fn parse_datum(&mut self) -> Result<SExpr<Symbol>> {
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
            Some(token) => Err(self.emit_err("Expected a datum", token)),
            _ => unreachable!("parse_datum expected at least 1 token to look ahead"),
        }
    }

    fn parse_atom(&mut self) -> SExpr<Symbol> {
        assert!(
            matches!(
                self.look_ahead(),
                Some(
                    Token::Id(..)
                        | Token::Bool(..)
                        | Token::Num(..)
                        | Token::Char(..)
                        | Token::Str(..)
                )
            ),
            "parse_atom expected a token that represents an atomic value",
        );

        match self.consume() {
            Token::Id(symbol, span) => SExpr::Var(symbol.clone(), *span),
            Token::Bool(bool, span) => SExpr::Bool(bool.clone(), *span),
            Token::Num(num, span) => SExpr::Num(num.clone(), *span),
            Token::Char(char, span) => SExpr::Char(char.clone(), *span),
            Token::Str(string, span) => SExpr::Str(string.clone(), *span),
            _ => unreachable!("parse_atom expected only tokens for atomic values"),
        }
    }

    fn parse_compound(&mut self) -> Result<SExpr<Symbol>> {
        match self.look_ahead() {
            Some(Token::HashLParen(_)) => self.parse_vector(),
            _ => self.parse_list(),
        }
    }

    fn parse_list(&mut self) -> Result<SExpr<Symbol>> {
        if matches!(
            self.look_ahead(),
            Some(Token::Quote(_) | Token::QuasiQuote(_) | Token::Comma(_) | Token::CommaAt(_))
        ) {
            return self.parse_abbreviation();
        }

        assert!(
            matches!(self.look_ahead(), Some(Token::LParen(_))),
            "parse_list expected a '(' token",
        );

        let start = self.consume().get_span();
        let mut elements: Vec<SExpr<Symbol>> = vec![];
        while let Some(t) = self.look_ahead() {
            if matches!(t, Token::RParen(_))
                || matches!(t, Token::Dot(_))
                || matches!(t, Token::EoF(_))
            {
                break;
            }
            elements.push(self.parse_datum()?);
        }
        match self.look_ahead() {
            Some(dot @ Token::Dot(_)) => {
                if elements.is_empty() {
                    return Err(self.emit_err("Expected a datum after '('", dot));
                }
                elements.push(self.parse_dot_notation()?);
                assert!(
                    matches!(self.look_ahead(), Some(Token::RParen(_))),
                    "parse_list expected a ')' token after parse_dot_notation returns",
                );
                Ok(Self::make_improper_list(
                    &elements,
                    start,
                    self.consume().get_span(),
                ))
            }
            Some(Token::RParen(end)) => {
                self.consume();
                Ok(Self::make_list(&elements, start, end))
            }
            Some(token) => Err(self.emit_err("Expected ')' to close '('", token)),
            None => {
                unreachable!("parse_datum expected token stream to end with the EoF token")
            }
        }
    }

    fn parse_dot_notation(&mut self) -> Result<SExpr<Symbol>> {
        assert!(
            matches!(self.look_ahead(), Some(Token::Dot(_))),
            "parse_dot_notation expected the '.' token"
        );
        self.consume();
        let tail = self.parse_datum()?;
        match self.look_ahead() {
            Some(Token::RParen(_)) => Ok(tail),
            Some(token) => Err(self.emit_err("Expected ')' after dotted datum", token)),
            None => {
                unreachable!("parse_list expected at least 1 token to look ahead")
            }
        }
    }

    fn parse_abbreviation(&mut self) -> Result<SExpr<Symbol>> {
        let elements = [self.parse_prefix(), self.parse_datum()?];
        Ok(Self::make_list(
            &elements,
            elements[0].get_span(),
            elements[1].get_span(),
        ))
    }

    fn parse_prefix(&mut self) -> SExpr<Symbol> {
        assert!(
            matches!(
                self.look_ahead(),
                Some(Token::Quote(_) | Token::QuasiQuote(_) | Token::Comma(_) | Token::CommaAt(_))
            ),
            "parse_prefix expected either '(' or an abbreviation prefix",
        );

        match self.consume() {
            Token::Quote(span) => SExpr::Var(Symbol::new("quote"), *span),
            Token::QuasiQuote(span) => SExpr::Var(Symbol::new("quasiquote"), *span),
            Token::Comma(span) => SExpr::Var(Symbol::new("unquote"), *span),
            Token::CommaAt(span) => SExpr::Var(Symbol::new("unquote-splicing"), *span),
            _ => unreachable!("parse_abbreviation expected only tokens for abbreviated prefix"),
        }
    }

    fn parse_vector(&mut self) -> Result<SExpr<Symbol>> {
        assert!(
            matches!(self.look_ahead(), Some(Token::HashLParen(_))),
            "parse_vector expected the '#(' token"
        );
        let start = self.consume().get_span();
        let mut elements: Vec<SExpr<Symbol>> = vec![];
        while let Some(t) = self.look_ahead() {
            if matches!(t, Token::RParen(_)) || matches!(t, Token::EoF(_)) {
                break;
            }
            elements.push(self.parse_datum()?);
        }
        match self.look_ahead() {
            Some(Token::RParen(end)) => {
                self.consume();
                Ok(SExpr::Vector(Vector(elements), start.combine(end)))
            }
            Some(token) => Err(self.emit_err("Expected ')' to close '#('", token)),
            None => unreachable!("parse_datum expected at least 1 token to look ahead"),
        }
    }

    fn make_list(elements: &[SExpr<Symbol>], start: Span, end: Span) -> SExpr<Symbol> {
        let mut res = SExpr::Nil(end);
        for element in elements.iter().rev() {
            res = SExpr::cons(element.clone(), res);
        }
        res.update_span(start.combine(res.get_span()));
        res
    }

    fn make_improper_list(slice: &[SExpr<Symbol>], start: Span, end: Span) -> SExpr<Symbol> {
        assert!(
            slice.len() >= 2,
            "improper list has to have more than 2 elements"
        );
        let mut iter = slice.iter().rev();
        let cdr = iter.next().unwrap().clone();
        let car = iter.next().unwrap().clone();
        let mut res = SExpr::cons(car, cdr);
        res.update_span(res.get_span().combine(end));
        for element in iter {
            res = SExpr::cons(element.clone(), res);
        }
        res.update_span(start.combine(res.get_span()));
        res
    }

    fn look_ahead(&mut self) -> Option<Token> {
        self.it.peek().copied().cloned()
    }

    fn consume(&mut self) -> &Token {
        let token = self.it.next().unwrap();
        self.cur = token;
        token
    }

    fn emit_err(&self, reason: &str, token: Token) -> CompilationError {
        CompilationError {
            span: token.get_span(),
            reason: format!("{}, but got: {}", reason, token),
        }
    }
}
