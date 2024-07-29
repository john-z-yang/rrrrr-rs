use std::collections::BTreeSet;
use std::fmt;

use super::{
    bindings::{ScopeId, Scopes},
    src_loc::SourceLoc,
};

#[derive(PartialEq, Clone, Debug)]
pub enum SExpr {
    Id(Id, SourceLoc),
    Cons(Cons, SourceLoc),
    Nil(SourceLoc),
    Bool(Bool, SourceLoc),
    Num(Num, SourceLoc),
    Char(Char, SourceLoc),
    Str(Str, SourceLoc),
    Vector(Vector, SourceLoc),
}

#[derive(PartialEq, Clone, Eq, Hash)]
pub struct Id {
    pub symbol: Symbol,
    pub scopes: Scopes,
}

#[derive(PartialEq, Clone, Debug)]
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
            SExpr::Nil(_) => {
                write!(f, ")")
            }
            SExpr::Cons(cons, _) => {
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
            SExpr::Nil(_) => {
                write!(f, ")")
            }
            SExpr::Cons(cons, _) => {
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

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Id {{ symbol: {}, scopes: {:?} }}",
            self.symbol, self.scopes
        )
    }
}

// impl fmt::Debug for Cons {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         write!(f, "(")?;
//         self.fmt_dbg(f)
//     }
// }

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

impl SExpr {
    pub fn get_src_loc(&self) -> SourceLoc {
        match self {
            SExpr::Id(_, src_loc) => src_loc,
            SExpr::Cons(_, src_loc) => src_loc,
            SExpr::Nil(src_loc) => src_loc,
            SExpr::Bool(_, src_loc) => src_loc,
            SExpr::Num(_, src_loc) => src_loc,
            SExpr::Char(_, src_loc) => src_loc,
            SExpr::Str(_, src_loc) => src_loc,
            SExpr::Vector(_, src_loc) => src_loc,
        }
        .clone()
    }

    pub fn update_src_loc(&self, source_loc: SourceLoc) -> Self {
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

    pub fn cons(car: SExpr, cdr: SExpr) -> Self {
        let start = car.get_src_loc();
        let end = cdr.get_src_loc();
        Self::Cons(Cons::new(car, cdr), start.combine(&end))
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
                source_loc.clone(),
            ),
            Self::Cons(cons, _) => Self::cons(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
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

    pub fn make_list(elements: &[Self], start: &SourceLoc, end: &SourceLoc) -> Self {
        let mut res = Self::Nil(end.clone());
        for element in elements.iter().rev() {
            res = Self::cons(element.clone(), res);
        }
        res.update_src_loc(start.clone().combine(&res.get_src_loc()))
    }

    pub fn make_improper_list(slice: &[Self], start: &SourceLoc, end: &SourceLoc) -> Self {
        assert!(
            slice.len() >= 2,
            "improper list has to have more than 2 element"
        );
        let mut iter = slice.iter().rev();
        let cdr = iter.next().unwrap().clone();
        let car = iter.next().unwrap().clone();
        let mut res = Self::cons(car, cdr);
        res = res.update_src_loc(res.get_src_loc().combine(end));
        for element in iter {
            res = Self::cons(element.clone(), res);
        }
        res.update_src_loc(start.clone().combine(&res.get_src_loc()))
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::sexpr;

//     use super::*;

//     #[test]
//     fn test_add_scope() {
//         let list = sexpr!(
//             SExpr::Id(
//                 Id::new("a", [1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 0,
//                     width: 1
//                 }
//             ),
//             (SExpr::Id(
//                 Id::new("b", [1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 2,
//                     width: 1
//                 }
//             )),
//             (SExpr::Id(
//                 Id::new("c", [0]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 4,
//                     width: 1
//                 }
//             )),
//             SExpr::Id(
//                 Id::new("d", [0, 1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 6,
//                     width: 1
//                 }
//             ),
//         );
//         assert_eq!(
//             list.add_scope(0).add_scope(2),
//             sexpr!(
//                 SExpr::Id(
//                     Id::new("a", [0, 1, 2]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 0,
//                         width: 1
//                     }
//                 ),
//                 (SExpr::Id(
//                     Id::new("b", [0, 1, 2]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 2,
//                         width: 1
//                     }
//                 )),
//                 (SExpr::Id(
//                     Id::new("c", [0, 1, 2]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 4,
//                         width: 1
//                     }
//                 )),
//                 SExpr::Id(
//                     Id::new("d", [0, 1, 2]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 6,
//                         width: 1
//                     }
//                 ),
//             )
//         )
//     }

//     #[test]
//     fn test_flip_scope() {
//         let list = sexpr!(
//             SExpr::Id(
//                 Id::new("a", [1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 0,
//                     width: 1
//                 }
//             ),
//             (SExpr::Id(
//                 Id::new("b", [1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 2,
//                     width: 1
//                 }
//             )),
//             (SExpr::Id(
//                 Id::new("c", [0]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 4,
//                     width: 1
//                 }
//             )),
//             SExpr::Id(
//                 Id::new("d", [0, 1]),
//                 SourceLoc {
//                     line: 0,
//                     idx: 6,
//                     width: 1
//                 }
//             ),
//         );
//         assert_eq!(
//             list.flip_scope(0),
//             sexpr!(
//                 SExpr::Id(
//                     Id::new("a", [1, 0]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 0,
//                         width: 1
//                     }
//                 ),
//                 (SExpr::Id(
//                     Id::new("b", [1, 0]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 2,
//                         width: 1
//                     }
//                 )),
//                 (SExpr::Id(
//                     Id::new("c", []),
//                     SourceLoc {
//                         line: 0,
//                         idx: 4,
//                         width: 1
//                     }
//                 )),
//                 SExpr::Id(
//                     Id::new("d", [1]),
//                     SourceLoc {
//                         line: 0,
//                         idx: 6,
//                         width: 1
//                     }
//                 ),
//             )
//         )
//     }
// }
