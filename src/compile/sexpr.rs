use std::collections::BTreeSet;
use std::fmt;

use super::{
    bindings::{ScopeId, Scopes},
    span::Span,
};

#[derive(Clone, Debug)]
pub(crate) enum SExpr {
    Id(Id, Span),
    Cons(Cons, Span),
    Nil(Span),
    Bool(Bool, Span),
    Num(Num, Span),
    Char(Char, Span),
    Str(Str, Span),
    Vector(Vector, Span),
}

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub(crate) struct Id {
    pub(crate) symbol: Symbol,
    pub(crate) scopes: Scopes,
}

#[derive(PartialEq, Clone, Debug)]
pub(crate) struct Cons {
    pub(crate) car: Box<SExpr>,
    pub(crate) cdr: Box<SExpr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub(crate) struct Symbol(pub(crate) String);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Bool(pub(crate) bool);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Num(pub(crate) f32);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Char(pub(crate) char);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Str(pub(crate) String);

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct Vector(pub(crate) Vec<SExpr>);

impl Id {
    pub(crate) fn new<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Id {
            symbol: Symbol::new(symbol),
            scopes: BTreeSet::from(scopes),
        }
    }
}

impl Cons {
    pub(crate) fn new(car: SExpr, cdr: SExpr) -> Self {
        Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        }
    }

    fn fmt_disp(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.car)?;
        match self.cdr.as_ref() {
            SExpr::Nil(_) => {
                write!(f, ")")
            }
            SExpr::Cons(cons, _) => {
                write!(f, " ")?;
                cons.fmt_disp(f)
            }
            other => {
                write!(f, " . {})", other)
            }
        }
    }
}

impl Symbol {
    pub(crate) fn new(symbol: &str) -> Self {
        Symbol(symbol.to_string())
    }
}

impl fmt::Display for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SExpr::Id(id, _) => {
                write!(f, "{}", id)
            }
            SExpr::Cons(cons, _) => {
                write!(f, "{}", cons)
            }
            SExpr::Nil(_) => {
                write!(f, "()")
            }
            SExpr::Bool(bool, _) => {
                write!(f, "{}", bool)
            }
            SExpr::Num(num, _) => {
                write!(f, "{}", num)
            }
            SExpr::Char(char, _) => {
                write!(f, "{}", char)
            }
            SExpr::Str(str, _) => {
                write!(f, "{}", str)
            }
            SExpr::Vector(vector, _) => {
                write!(f, "{}", vector)
            }
        }
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

impl fmt::Display for Cons {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        self.fmt_disp(f)
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", if self.0 { "#t" } else { "#f" })
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Char {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#\\{}", self.0)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for Vector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "#(")?;
        for e in self.0.iter().take(1) {
            write!(f, "{}", e)?;
        }
        for e in self.0.iter().skip(1) {
            write!(f, " {}", e)?;
        }
        write!(f, ")")
    }
}

impl TryFrom<SExpr> for Id {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Id(id, _) = value {
            Ok(id)
        } else {
            Err(())
        }
    }
}

impl TryFrom<SExpr> for Cons {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Cons(cons, _) = value {
            Ok(cons)
        } else {
            Err(())
        }
    }
}

impl TryFrom<SExpr> for Bool {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Bool(bool, _) = value {
            Ok(bool)
        } else {
            Err(())
        }
    }
}

impl PartialEq for SExpr {
    fn eq(&self, other: &Self) -> bool {
        match self {
            SExpr::Id(id, _) => {
                let SExpr::Id(other, _) = other else {
                    return false;
                };
                id == other
            }
            SExpr::Cons(cons, _) => {
                let SExpr::Cons(other, _) = other else {
                    return false;
                };
                cons == other
            }
            SExpr::Nil(_) => {
                matches!(other, Self::Nil(_))
            }
            SExpr::Bool(bool, _) => {
                let SExpr::Bool(other, _) = other else {
                    return false;
                };
                bool == other
            }
            SExpr::Num(num, _) => {
                let SExpr::Num(other, _) = other else {
                    return false;
                };
                num == other
            }
            SExpr::Char(char, _) => {
                let SExpr::Char(other, _) = other else {
                    return false;
                };
                char == other
            }
            SExpr::Str(str, _) => {
                let SExpr::Str(other, _) = other else {
                    return false;
                };
                str == other
            }
            SExpr::Vector(vector, _) => {
                let SExpr::Vector(other, _) = other else {
                    return false;
                };
                vector == other
            }
        }
    }
}

impl SExpr {
    pub(crate) fn get_span(&self) -> Span {
        *match self {
            SExpr::Id(_, span) => span,
            SExpr::Cons(_, span) => span,
            SExpr::Nil(span) => span,
            SExpr::Bool(_, span) => span,
            SExpr::Num(_, span) => span,
            SExpr::Char(_, span) => span,
            SExpr::Str(_, span) => span,
            SExpr::Vector(_, span) => span,
        }
    }

    pub(crate) fn update_span(&self, span: Span) -> Self {
        match self {
            SExpr::Id(id, _) => SExpr::Id(id.clone(), span),
            SExpr::Cons(cons, _) => SExpr::Cons(cons.clone(), span),
            SExpr::Nil(_) => SExpr::Nil(span),
            SExpr::Bool(bool, _) => SExpr::Bool(bool.clone(), span),
            SExpr::Num(num, _) => SExpr::Num(num.clone(), span),
            SExpr::Char(char, _) => SExpr::Char(char.clone(), span),
            SExpr::Str(str, _) => SExpr::Str(str.clone(), span),
            SExpr::Vector(vector, _) => SExpr::Vector(vector.clone(), span),
        }
    }

    pub(crate) fn cons(car: SExpr, cdr: SExpr) -> Self {
        let start = car.get_span();
        let end = cdr.get_span();
        Self::Cons(Cons::new(car, cdr), start.combine(end))
    }

    fn adjust_scope<F>(&self, op: &F) -> Self
    where
        F: Fn(&Scopes) -> Scopes,
    {
        match self {
            Self::Id(
                Id {
                    symbol,
                    scopes: scope,
                },
                span,
            ) => Self::Id(
                Id {
                    symbol: symbol.clone(),
                    scopes: op(scope),
                },
                *span,
            ),
            Self::Cons(cons, span) => Self::Cons(
                Cons::new(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
                *span,
            ),
            _ => self.clone(),
        }
    }

    pub(crate) fn add_scope(&self, scope: ScopeId) -> Self {
        let op = |scopes: &Scopes| {
            let mut scopes = scopes.clone();
            scopes.insert(scope);
            scopes
        };
        self.adjust_scope(&op)
    }

    pub(crate) fn flip_scope(&self, scope: ScopeId) -> Self {
        let op = |scopes: &Scopes| {
            let mut scopes = scopes.clone();
            if scopes.contains(&scope) {
                scopes.remove(&scope);
            } else {
                scopes.insert(scope);
            }
            scopes
        };
        self.adjust_scope(&op)
    }

    #[cfg(test)]
    pub(crate) fn is_idential(&self, other: &Self) -> bool {
        if self.get_span() != other.get_span() {
            return false;
        }
        if let (Self::Cons(self_cons, _), Self::Cons(other_cons, _)) = (self, other) {
            self_cons.car.is_idential(&other_cons.car) && self_cons.cdr.is_idential(&other_cons.cdr)
        } else {
            self == other
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sexpr;

    use super::*;

    #[test]
    fn test_add_scope() {
        let span = Span { lo: 0, hi: 1 };
        let list = sexpr!(
            SExpr::Id(Id::new("a", [1]), span),
            (SExpr::Id(Id::new("b", [1]), span)),
            (SExpr::Id(Id::new("c", [0]), span)),
            SExpr::Id(Id::new("d", [0, 1]), span),
        );
        assert_eq!(
            list.add_scope(0).add_scope(2),
            sexpr!(
                SExpr::Id(Id::new("a", [0, 1, 2]), span),
                (SExpr::Id(Id::new("b", [0, 1, 2]), span)),
                (SExpr::Id(Id::new("c", [0, 2]), span)),
                SExpr::Id(Id::new("d", [0, 1, 2]), span),
            )
        )
    }

    #[test]
    fn test_flip_scope() {
        let span = Span { lo: 0, hi: 1 };
        let list = sexpr!(
            SExpr::Id(Id::new("a", [1]), span),
            (SExpr::Id(Id::new("b", [1]), span)),
            (SExpr::Id(Id::new("c", [0]), span)),
            SExpr::Id(Id::new("d", [0, 1]), span),
        );
        assert_eq!(
            list.flip_scope(0),
            sexpr!(
                SExpr::Id(Id::new("a", [1, 0]), span),
                (SExpr::Id(Id::new("b", [1, 0]), span)),
                (SExpr::Id(Id::new("c", []), span)),
                SExpr::Id(Id::new("d", [1]), span),
            )
        )
    }
}
