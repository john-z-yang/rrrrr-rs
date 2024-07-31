use std::collections::BTreeSet;
use std::fmt;

use super::{
    bindings::{ScopeId, Scopes},
    source_loc::SourceLoc,
};

#[derive(Clone, Debug)]
pub(crate) enum SExpr {
    Id(Id, SourceLoc),
    Cons(Cons, SourceLoc),
    Nil(SourceLoc),
    Bool(Bool, SourceLoc),
    Num(Num, SourceLoc),
    Char(Char, SourceLoc),
    Str(Str, SourceLoc),
    Vector(Vector, SourceLoc),
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
    pub(crate) fn new<T, U>(car: T, cdr: U) -> Self
    where
        T: Into<SExpr>,
        U: Into<SExpr>,
    {
        Cons {
            car: Box::new(car.into()),
            cdr: Box::new(cdr.into()),
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
    pub(crate) fn get_source_loc(&self) -> SourceLoc {
        *match self {
            SExpr::Id(_, source_loc) => source_loc,
            SExpr::Cons(_, source_loc) => source_loc,
            SExpr::Nil(source_loc) => source_loc,
            SExpr::Bool(_, source_loc) => source_loc,
            SExpr::Num(_, source_loc) => source_loc,
            SExpr::Char(_, source_loc) => source_loc,
            SExpr::Str(_, source_loc) => source_loc,
            SExpr::Vector(_, source_loc) => source_loc,
        }
    }

    pub(crate) fn update_source_loc(&self, source_loc: SourceLoc) -> Self {
        match self {
            SExpr::Id(id, _) => SExpr::Id(id.clone(), source_loc),
            SExpr::Cons(cons, _) => SExpr::Cons(cons.clone(), source_loc),
            SExpr::Nil(_) => SExpr::Nil(source_loc),
            SExpr::Bool(bool, _) => SExpr::Bool(bool.clone(), source_loc),
            SExpr::Num(num, _) => SExpr::Num(num.clone(), source_loc),
            SExpr::Char(char, _) => SExpr::Char(char.clone(), source_loc),
            SExpr::Str(str, _) => SExpr::Str(str.clone(), source_loc),
            SExpr::Vector(vector, _) => SExpr::Vector(vector.clone(), source_loc),
        }
    }

    pub(crate) fn cons(car: SExpr, cdr: SExpr) -> Self {
        let start = car.get_source_loc();
        let end = cdr.get_source_loc();
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
                source_loc,
            ) => Self::Id(
                Id {
                    symbol: symbol.clone(),
                    scopes: op(scope),
                },
                *source_loc,
            ),
            Self::Cons(cons, source_loc) => Self::Cons(
                Cons::new(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
                *source_loc,
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
        if self.get_source_loc() != other.get_source_loc() {
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
        let source_loc = SourceLoc {
            line: 0,
            idx: 0,
            width: 1,
        };
        let list = sexpr!(
            SExpr::Id(Id::new("a", [1]), source_loc),
            (SExpr::Id(Id::new("b", [1]), source_loc)),
            (SExpr::Id(Id::new("c", [0]), source_loc)),
            SExpr::Id(Id::new("d", [0, 1]), source_loc),
        );
        assert_eq!(
            list.add_scope(0).add_scope(2),
            sexpr!(
                SExpr::Id(Id::new("a", [0, 1, 2]), source_loc),
                (SExpr::Id(Id::new("b", [0, 1, 2]), source_loc)),
                (SExpr::Id(Id::new("c", [0, 2]), source_loc)),
                SExpr::Id(Id::new("d", [0, 1, 2]), source_loc),
            )
        )
    }

    #[test]
    fn test_flip_scope() {
        let source_loc = SourceLoc {
            line: 0,
            idx: 0,
            width: 1,
        };
        let list = sexpr!(
            SExpr::Id(Id::new("a", [1]), source_loc),
            (SExpr::Id(Id::new("b", [1]), source_loc)),
            (SExpr::Id(Id::new("c", [0]), source_loc)),
            SExpr::Id(Id::new("d", [0, 1]), source_loc),
        );
        assert_eq!(
            list.flip_scope(0),
            sexpr!(
                SExpr::Id(Id::new("a", [1, 0]), source_loc),
                (SExpr::Id(Id::new("b", [1, 0]), source_loc)),
                (SExpr::Id(Id::new("c", []), source_loc)),
                SExpr::Id(Id::new("d", [1]), source_loc),
            )
        )
    }
}
