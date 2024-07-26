use std::collections::BTreeSet;
use std::fmt;

use super::bindings::{ScopeId, Scopes};

#[derive(PartialEq, Clone)]
pub enum SExpr {
    Id(Id),
    Cons(Cons),
    Nil,
    Bool(Bool),
    Num(Num),
    Char(Char),
    Str(Str),
    Vector(Vector),
}

#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Id {
    pub symbol: Symbol,
    pub scopes: Scopes,
}

#[derive(PartialEq, Clone)]
pub struct Cons {
    pub car: Box<SExpr>,
    pub cdr: Box<SExpr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

#[derive(Debug, PartialEq, Clone)]
pub struct Bool(pub bool);

#[derive(Debug, PartialEq, Clone)]
pub struct Num(pub f32);

#[derive(Debug, PartialEq, Clone)]
pub struct Char(pub char);

#[derive(Debug, PartialEq, Clone)]
pub struct Str(pub String);

#[derive(Debug, PartialEq, Clone)]
pub struct Vector(pub Vec<SExpr>);

impl Id {
    pub fn new<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Id {
            symbol: Symbol::new(symbol),
            scopes: BTreeSet::from(scopes),
        }
    }
}

impl Cons {
    pub fn new<T, U>(car: T, cdr: U) -> Self
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
            SExpr::Nil => {
                write!(f, ")")
            }
            SExpr::Cons(cons) => {
                write!(f, " ")?;
                cons.fmt_disp(f)
            }
            other => {
                write!(f, ". {})", other)
            }
        }
    }

    fn fmt_dbg(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.car)?;
        match self.cdr.as_ref() {
            SExpr::Nil => {
                write!(f, ")")
            }
            SExpr::Cons(cons) => {
                write!(f, " ")?;
                cons.fmt_dbg(f)
            }
            other => {
                write!(f, ". {})", other)
            }
        }
    }
}

impl Symbol {
    pub fn new(symbol: &str) -> Self {
        Symbol(symbol.to_string())
    }
}

impl Vector {
    pub fn new(slice: &[SExpr]) -> Self {
        Vector(slice.to_vec())
    }
}

impl fmt::Debug for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SExpr::Id(id) => {
                write!(f, "{:?}", id)
            }
            SExpr::Cons(cons) => {
                write!(f, "{:?}", cons)
            }
            SExpr::Nil => {
                write!(f, "()")
            }
            SExpr::Bool(bool) => {
                write!(f, "{:?}", bool)
            }
            SExpr::Num(num) => {
                write!(f, "{:?}", num)
            }
            SExpr::Char(char) => {
                write!(f, "'{:?}'", char)
            }
            SExpr::Str(string) => {
                write!(f, "\"{:?}\"", string)
            }
            SExpr::Vector(vector) => {
                write!(f, "\"{:?}\"", vector)
            }
        }
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Id {{ symbol: {}, scopes: {:?} }}",
            self.symbol, self.scopes
        )
    }
}

impl fmt::Debug for Cons {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        self.fmt_dbg(f)
    }
}

impl fmt::Display for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SExpr::Id(id) => {
                write!(f, "{}", id)
            }
            SExpr::Cons(cons) => {
                write!(f, "{}", cons)
            }
            SExpr::Nil => {
                write!(f, "()")
            }
            SExpr::Bool(bool) => {
                write!(f, "{}", bool)
            }
            SExpr::Num(num) => {
                write!(f, "{}", num)
            }
            SExpr::Char(char) => {
                write!(f, "'{:?}'", char)
            }
            SExpr::Str(str) => {
                write!(f, "{}", str)
            }
            SExpr::Vector(vector) => {
                write!(f, "#(")?;
                vector.0.iter().try_for_each(|e| write!(f, "{}", e))?;
                write!(f, ")")
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
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl fmt::Display for Vector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl From<Id> for SExpr {
    fn from(value: Id) -> Self {
        SExpr::Id(value)
    }
}

impl From<Cons> for SExpr {
    fn from(value: Cons) -> Self {
        SExpr::Cons(value)
    }
}

impl From<Symbol> for SExpr {
    fn from(value: Symbol) -> Self {
        SExpr::Id(Id {
            scopes: BTreeSet::from([]),
            symbol: value,
        })
    }
}

impl From<Bool> for SExpr {
    fn from(value: Bool) -> Self {
        SExpr::Bool(value)
    }
}

impl From<Num> for SExpr {
    fn from(value: Num) -> Self {
        SExpr::Num(value)
    }
}

impl From<Char> for SExpr {
    fn from(value: Char) -> Self {
        SExpr::Char(value)
    }
}

impl From<Str> for SExpr {
    fn from(value: Str) -> Self {
        SExpr::Str(value)
    }
}

impl From<Vector> for SExpr {
    fn from(value: Vector) -> Self {
        SExpr::Vector(value)
    }
}

impl TryFrom<SExpr> for Id {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Id(id) = value {
            Ok(id)
        } else {
            Err(())
        }
    }
}

impl TryFrom<SExpr> for Cons {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Cons(cons) = value {
            Ok(cons)
        } else {
            Err(())
        }
    }
}

impl TryFrom<SExpr> for Bool {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Bool(bool) = value {
            Ok(bool)
        } else {
            Err(())
        }
    }
}

impl SExpr {
    pub fn id<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Self::Id(Id {
            symbol: Symbol::new(symbol),
            scopes: Scopes::from(scopes),
        })
    }

    pub fn cons<T, U>(car: T, cdr: U) -> Self
    where
        T: Into<SExpr>,
        U: Into<SExpr>,
    {
        Self::Cons(Cons::new(car, cdr))
    }

    pub fn bool(val: bool) -> Self {
        Self::Bool(Bool(val))
    }

    pub fn num(val: f32) -> Self {
        Self::Num(Num(val))
    }

    pub fn vector(val: &[Self]) -> Self {
        Self::Vector(Vector::new(val))
    }

    fn adjust_scope<F>(&self, op: &F) -> Self
    where
        F: Fn(&Scopes) -> Scopes,
    {
        match self {
            Self::Id(Id {
                symbol,
                scopes: scope,
            }) => Self::Id(Id {
                symbol: symbol.clone(),
                scopes: op(scope),
            }),
            Self::Cons(cons) => Self::cons(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
            _ => self.clone(),
        }
    }

    pub fn add_scope(&self, scope: ScopeId) -> Self {
        let op = |scopes: &Scopes| {
            let mut scopes = scopes.clone();
            scopes.insert(scope);
            scopes
        };
        self.adjust_scope(&op)
    }

    pub fn flip_scope(&self, scope: ScopeId) -> Self {
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

    pub fn make_list(slice: &[Self]) -> Self {
        if slice.is_empty() {
            Self::Nil
        } else {
            Self::cons(slice[0].clone(), SExpr::make_list(&slice[1..]))
        }
    }

    pub fn make_improper_list(slice: &[Self]) -> Self {
        assert!(
            slice.len() > 1,
            "improper list has to have more than 1 elements"
        );
        if slice.len() == 2 {
            Self::cons(slice[0].clone(), slice[1].clone())
        } else {
            Self::cons(slice[0].clone(), SExpr::make_improper_list(&slice[1..]))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::sexpr;

    use super::*;

    #[test]
    fn test_add_scope() {
        let list = sexpr!(
            SExpr::id("a", [1]),
            (SExpr::id("b", [0, 1])),
            (SExpr::id("c", [0])),
            SExpr::id("d", [0, 1]),
        );
        assert_eq!(
            list.add_scope(0).add_scope(2),
            sexpr!(
                SExpr::id("a", [0, 1, 2]),
                (SExpr::id("b", [0, 1, 2])),
                (SExpr::id("c", [0, 2])),
                SExpr::id("d", [0, 1, 2])
            )
        )
    }

    #[test]
    fn test_flip_scope() {
        let list = sexpr!(
            SExpr::id("a", [1]),
            (SExpr::id("b", [0, 1])),
            (SExpr::id("c", [0])),
            SExpr::id("d", [0, 1]),
        );
        assert_eq!(
            list.flip_scope(0),
            sexpr!(
                SExpr::id("a", [0, 1]),
                (SExpr::id("b", [1])),
                (SExpr::id("c", [])),
                SExpr::id("d", [1]),
            )
        )
    }
}
