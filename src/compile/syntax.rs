use std::collections::BTreeSet;
use std::fmt;

use super::bindings::{ScopeId, Scopes};

#[derive(Debug, PartialEq, Clone)]
pub enum SExpr {
    Id(Id),
    Cons(Cons),
    Symbol(Symbol),
    Nil,
    Bool(Bool),
    Num(Num),
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct Id {
    pub symbol: Symbol,
    pub scopes: Scopes,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Cons {
    pub car: Box<SExpr>,
    pub cdr: Box<SExpr>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

#[derive(Debug, PartialEq, Clone)]
pub struct Bool(bool);

#[derive(Debug, PartialEq, Clone)]
pub struct Num(u32);

impl fmt::Display for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SExpr::Id(id) => {
                write!(f, "{}", id)
            }
            SExpr::Cons(cons) => {
                write!(f, "{}", cons)
            }
            SExpr::Symbol(symbol) => {
                write!(f, "{}", symbol)
            }
            SExpr::Nil => {
                write!(f, "Nil")
            }
            SExpr::Bool(bool) => {
                write!(f, "{}", bool)
            }
            SExpr::Num(num) => {
                write!(f, "{}", num)
            }
        }
    }
}

impl Id {
    pub fn new<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Id {
            symbol: Symbol::new(symbol),
            scopes: BTreeSet::from(scopes),
        }
    }
}

impl Cons {
    pub fn new(car: SExpr, cdr: SExpr) -> Self {
        Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        }
    }

    fn fmt_list(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.car)?;
        match self.cdr.as_ref() {
            SExpr::Nil => {
                write!(f, ")")
            }
            SExpr::Cons(cons) => {
                write!(f, " ")?;
                cons.fmt_list(f)
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

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

impl fmt::Display for Cons {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        self.fmt_list(f)
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
        SExpr::Symbol(value)
    }
}

impl From<Bool> for SExpr {
    fn from(value: Bool) -> Self {
        SExpr::Bool(value)
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

impl TryFrom<SExpr> for Symbol {
    type Error = ();
    fn try_from(value: SExpr) -> Result<Self, Self::Error> {
        if let SExpr::Symbol(symbol) = value {
            Ok(symbol)
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
    pub fn new_id<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Self::Id(Id {
            symbol: Symbol::new(symbol),
            scopes: Scopes::from(scopes),
        })
    }

    pub fn new_cons(car: SExpr, cdr: SExpr) -> Self {
        Self::Cons(Cons::new(car, cdr))
    }

    pub fn new_symbol(symbol: &str) -> Self {
        Self::Symbol(Symbol::new(symbol))
    }

    pub fn new_bool(val: bool) -> Self {
        Self::Bool(Bool(val))
    }

    pub fn new_num(val: u32) -> Self {
        Self::Num(Num(val))
    }

    pub fn coerce_to_syntax(&self) -> Self {
        match self {
            Self::Symbol(Symbol(symbol)) => SExpr::new_id(symbol, []),
            Self::Cons(cons) => {
                Self::new_cons(cons.car.coerce_to_syntax(), cons.cdr.coerce_to_syntax())
            }
            _ => self.clone(),
        }
    }

    pub fn coerce_to_datum(&self) -> Self {
        match self {
            Self::Id(Id { symbol, scopes: _ }) => Self::Symbol(symbol.clone()),
            Self::Cons(cons) => {
                Self::new_cons(cons.car.coerce_to_datum(), cons.cdr.coerce_to_datum())
            }
            _ => self.clone(),
        }
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
            Self::Cons(cons) => {
                Self::new_cons(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op))
            }
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
}

#[cfg(test)]
mod tests {
    use crate::sexpr;

    use super::*;

    #[test]
    fn test_add_scope() {
        let list = sexpr!(
            SExpr::new_id("a", [1]),
            SExpr::new_num(0),
            (SExpr::new_num(1), (SExpr::new_id("b", [0, 1]))),
            SExpr::new_num(2),
            (SExpr::new_id("c", [0])),
            SExpr::new_id("d", [0, 1]),
        );
        assert_eq!(
            list.add_scope(0).add_scope(2),
            sexpr!(
                SExpr::new_id("a", [0, 1, 2]),
                SExpr::new_num(0),
                (SExpr::new_num(1), (SExpr::new_id("b", [0, 1, 2])),),
                SExpr::new_num(2),
                (SExpr::new_id("c", [0, 2])),
                SExpr::new_id("d", [0, 1, 2])
            )
        )
    }

    #[test]
    fn test_flip_scope() {
        let list = sexpr!(
            SExpr::new_id("a", [1]),
            SExpr::new_num(0),
            (SExpr::new_num(1), (SExpr::new_id("b", [0, 1]))),
            SExpr::new_num(2),
            (SExpr::new_id("c", [0])),
            SExpr::new_id("d", [0, 1]),
        );
        assert_eq!(
            list.flip_scope(0),
            sexpr!(
                SExpr::new_id("a", [0, 1]),
                SExpr::new_num(0),
                (SExpr::new_num(1), (SExpr::new_id("b", [1]))),
                SExpr::new_num(2),
                (SExpr::new_id("c", [])),
                SExpr::new_id("d", [1]),
            )
        )
    }

    #[test]
    fn test_syntax_coercion() {
        assert_eq!(
            SExpr::new_symbol("a").coerce_to_syntax(),
            SExpr::new_id("a", []),
        );
        let list = sexpr!(
            SExpr::new_symbol("a"),
            SExpr::new_num(0),
            (SExpr::new_num(1), (SExpr::new_symbol("b"))),
            SExpr::new_num(2),
            (SExpr::new_symbol("c")),
            SExpr::new_id("d", [0, 1]),
        );
        assert_eq!(
            list.coerce_to_syntax(),
            sexpr!(
                SExpr::new_id("a", []),
                SExpr::new_num(0),
                (SExpr::new_num(1), (SExpr::new_id("b", []))),
                SExpr::new_num(2),
                (SExpr::new_id("c", [])),
                SExpr::new_id("d", [0, 1])
            )
        )
    }

    #[test]
    fn test_datum_coercion() {
        let list = sexpr!(
            SExpr::new_id("a", []),
            SExpr::new_num(0),
            (SExpr::new_num(1), (SExpr::new_id("b", []))),
            SExpr::new_num(2),
            (SExpr::new_id("c", [])),
            SExpr::new_id("d", [0, 1])
        );
        assert_eq!(
            list.coerce_to_datum(),
            sexpr!(
                SExpr::new_symbol("a"),
                SExpr::new_num(0),
                (SExpr::new_num(1), (SExpr::new_symbol("b"))),
                SExpr::new_num(2),
                (SExpr::new_symbol("c")),
                SExpr::new_symbol("d")
            )
        )
    }
}
