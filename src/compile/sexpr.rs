use std::collections::BTreeSet;
use std::fmt;

use super::{
    bindings::{ScopeId, Scopes},
    span::Span,
};

#[derive(PartialEq, Clone, Debug)]
pub enum SExpr {
    Id(Id, Span),
    Cons(Cons, Span),
    Nil(Span),
    Bool(Bool, Span),
    Num(Num, Span),
    Char(Char, Span),
    Str(Str, Span),
    Vector(Vector, Span),
}

impl SExpr {
    pub fn without_spans(&self) -> SExprWithoutSpans<'_> {
        SExprWithoutSpans(self)
    }

    pub fn get_span(&self) -> Span {
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

    pub fn update_span(&self, span: Span) -> Self {
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

    pub fn cons(car: SExpr, cdr: SExpr) -> Self {
        let start = car.get_span();
        let end = cdr.get_span();
        Self::Cons(Cons::new(car, cdr), start.combine(end))
    }

    fn adjust_scope<F>(&self, op: &F) -> Self
    where
        F: Fn(&Scopes) -> Scopes,
    {
        match self {
            Self::Id(id, span) => Self::Id(id.adjust_scope(op), *span),
            Self::Cons(cons, span) => Self::Cons(
                Cons::new(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
                *span,
            ),
            Self::Vector(vector, span) => Self::Vector(
                Vector(
                    vector
                        .0
                        .iter()
                        .map(|sexpr| sexpr.adjust_scope(op))
                        .collect(),
                ),
                *span,
            ),
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

#[derive(Clone, Copy)]
pub struct SExprWithoutSpans<'a>(&'a SExpr);

impl PartialEq for SExprWithoutSpans<'_> {
    fn eq(&self, other: &Self) -> bool {
        fn eq_sexpr_without_spans(left: &SExpr, right: &SExpr) -> bool {
            match (left, right) {
                (SExpr::Id(id, _), SExpr::Id(other, _)) => id == other,
                (SExpr::Cons(cons, _), SExpr::Cons(other, _)) => {
                    eq_sexpr_without_spans(&cons.car, &other.car)
                        && eq_sexpr_without_spans(&cons.cdr, &other.cdr)
                }
                (SExpr::Nil(_), SExpr::Nil(_)) => true,
                (SExpr::Bool(bool, _), SExpr::Bool(other, _)) => bool == other,
                (SExpr::Num(num, _), SExpr::Num(other, _)) => num == other,
                (SExpr::Char(char, _), SExpr::Char(other, _)) => char == other,
                (SExpr::Str(str, _), SExpr::Str(other, _)) => str == other,
                (SExpr::Vector(vector, _), SExpr::Vector(other, _)) => {
                    vector.0.len() == other.0.len()
                        && vector
                            .0
                            .iter()
                            .zip(other.0.iter())
                            .all(|(left, right)| eq_sexpr_without_spans(left, right))
                }
                _ => false,
            }
        }

        eq_sexpr_without_spans(self.0, other.0)
    }
}

impl fmt::Debug for SExprWithoutSpans<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SExpr::Id(id, _) => f.debug_tuple("Id").field(id).finish(),
            SExpr::Cons(cons, _) => f
                .debug_tuple("Cons")
                .field(&SExprWithoutSpans(cons.car.as_ref()))
                .field(&SExprWithoutSpans(cons.cdr.as_ref()))
                .finish(),
            SExpr::Nil(_) => f.write_str("Nil"),
            SExpr::Bool(bool, _) => f.debug_tuple("Bool").field(bool).finish(),
            SExpr::Num(num, _) => f.debug_tuple("Num").field(num).finish(),
            SExpr::Char(char, _) => f.debug_tuple("Char").field(char).finish(),
            SExpr::Str(str, _) => f.debug_tuple("Str").field(str).finish(),
            SExpr::Vector(vector, _) => {
                write!(f, "Vector(")?;
                let mut list = f.debug_list();
                for item in &vector.0 {
                    list.entry(&SExprWithoutSpans(item));
                }
                list.finish()?;
                write!(f, ")")
            }
        }
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub struct Id {
    pub symbol: Symbol,
    pub scopes: Scopes,
}

impl Id {
    pub fn new<const N: usize>(symbol: &str, scopes: [ScopeId; N]) -> Self {
        Id {
            symbol: Symbol::new(symbol),
            scopes: BTreeSet::from(scopes),
        }
    }

    pub fn adjust_scope<F>(&self, op: &F) -> Self
    where
        F: Fn(&Scopes) -> Scopes,
    {
        Id {
            symbol: self.symbol.clone(),
            scopes: op(&self.scopes),
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
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.symbol)
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

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(symbol: &str) -> Self {
        Symbol(symbol.to_string())
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct Cons {
    pub car: Box<SExpr>,
    pub cdr: Box<SExpr>,
}

impl Cons {
    pub fn new(car: SExpr, cdr: SExpr) -> Self {
        Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        }
    }

    pub fn try_into_vector(self, span: Span) -> Option<SExpr> {
        let mut vector = vec![*self.car];
        let mut cur = *self.cdr;
        while let SExpr::Cons(Cons { car, cdr }, _) = cur {
            vector.push(*car);
            cur = *cdr;
        }
        if let SExpr::Nil(_) = cur {
            Some(SExpr::Vector(Vector(vector), span))
        } else {
            None
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

impl fmt::Display for Cons {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        self.fmt_disp(f)
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

#[derive(Debug, PartialEq, Clone)]
pub struct Bool(pub bool);

impl fmt::Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", if self.0 { "#t" } else { "#f" })
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

#[derive(Debug, PartialEq, Clone)]
pub struct Num(pub f64);

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Char(pub char);

impl fmt::Display for Char {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 == ' ' {
            write!(f, "#\\space")
        } else if self.0 == '\n' {
            write!(f, "#\\newline")
        } else {
            write!(f, "#\\{}", self.0)
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Str(pub String);

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Vector(pub Vec<SExpr>);

impl Vector {
    pub fn into_cons_list(self, span: Span) -> SExpr {
        let mut prev = SExpr::Nil(Span {
            lo: span.hi - 1,
            hi: span.hi,
        });
        for e in self.0.into_iter().rev() {
            prev = SExpr::cons(e, prev);
        }
        prev
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

#[cfg(test)]
mod tests {
    use crate::{
        compile::{lex::tokenize, parse::parse},
        sexpr,
    };

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
            list.add_scope(0).add_scope(2).without_spans(),
            sexpr!(
                SExpr::Id(Id::new("a", [0, 1, 2]), span),
                (SExpr::Id(Id::new("b", [0, 1, 2]), span)),
                (SExpr::Id(Id::new("c", [0, 2]), span)),
                SExpr::Id(Id::new("d", [0, 1, 2]), span),
            )
            .without_spans()
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
            list.flip_scope(0).without_spans(),
            sexpr!(
                SExpr::Id(Id::new("a", [1, 0]), span),
                (SExpr::Id(Id::new("b", [1, 0]), span)),
                (SExpr::Id(Id::new("c", []), span)),
                SExpr::Id(Id::new("d", [1]), span),
            )
            .without_spans()
        )
    }

    #[test]
    fn test_add_scope_vector() {
        let span = Span { lo: 0, hi: 1 };
        let vector = SExpr::Vector(
            Vector(vec![
                SExpr::Id(Id::new("a", [1]), span),
                SExpr::Id(Id::new("b", [0]), span),
                SExpr::Num(Num(42.0), span),
            ]),
            span,
        );
        assert_eq!(
            vector.add_scope(2).without_spans(),
            SExpr::Vector(
                Vector(vec![
                    SExpr::Id(Id::new("a", [1, 2]), span),
                    SExpr::Id(Id::new("b", [0, 2]), span),
                    SExpr::Num(Num(42.0), span),
                ]),
                span,
            )
            .without_spans()
        )
    }

    #[test]
    fn test_flip_scope_vector() {
        let span = Span { lo: 0, hi: 1 };
        let vector = SExpr::Vector(
            Vector(vec![
                SExpr::Id(Id::new("a", [0, 1]), span),
                SExpr::Id(Id::new("b", [1]), span),
            ]),
            span,
        );
        assert_eq!(
            vector.flip_scope(1).without_spans(),
            SExpr::Vector(
                Vector(vec![
                    SExpr::Id(Id::new("a", [0]), span),
                    SExpr::Id(Id::new("b", []), span),
                ]),
                span,
            )
            .without_spans()
        )
    }

    #[test]
    fn test_add_scope_nested_vector() {
        let span = Span { lo: 0, hi: 1 };
        let nested = sexpr!(
            SExpr::Vector(Vector(vec![SExpr::Id(Id::new("x", [1]), span)]), span,),
            SExpr::Id(Id::new("y", [1]), span),
        );
        assert_eq!(
            nested.add_scope(2).without_spans(),
            sexpr!(
                SExpr::Vector(Vector(vec![SExpr::Id(Id::new("x", [1, 2]), span)]), span,),
                SExpr::Id(Id::new("y", [1, 2]), span),
            )
            .without_spans()
        )
    }

    #[test]
    fn test_vector_to_cons_list() {
        let SExpr::Vector(vector, span) = parse(&tokenize("#(1 2 3)").unwrap()).unwrap() else {
            unreachable!("Expected a vector")
        };

        assert_eq!(
            vector.into_cons_list(span),
            SExpr::Cons(
                Cons {
                    car: Box::new(SExpr::Num(Num(1.0), Span { lo: 2, hi: 3 })),
                    cdr: Box::new(SExpr::Cons(
                        Cons {
                            car: Box::new(SExpr::Num(Num(2.0), Span { lo: 4, hi: 5 })),
                            cdr: Box::new(SExpr::Cons(
                                Cons {
                                    car: Box::new(SExpr::Num(Num(3.0), Span { lo: 6, hi: 7 })),
                                    cdr: Box::new(SExpr::Nil(Span { lo: 7, hi: 8 })),
                                },
                                Span { lo: 6, hi: 8 },
                            )),
                        },
                        Span { lo: 4, hi: 8 },
                    )),
                },
                Span { lo: 2, hi: 8 },
            ),
        );
    }

    #[test]
    fn test_eq_includes_spans() {
        let left = SExpr::Num(Num(1.0), Span { lo: 0, hi: 1 });
        let right = SExpr::Num(Num(1.0), Span { lo: 3, hi: 4 });

        assert_ne!(left, right);
        assert_eq!(left.without_spans(), right.without_spans());
    }

    #[test]
    fn test_debug_without_spans_omits_span_fields() {
        let sexpr = SExpr::cons(
            SExpr::Num(Num(1.0), Span { lo: 1, hi: 2 }),
            SExpr::Vector(
                Vector(vec![SExpr::Id(Id::new("x", [1]), Span { lo: 3, hi: 4 })]),
                Span { lo: 5, hi: 6 },
            ),
        );

        let rendered = format!("{:?}", sexpr.without_spans());
        assert!(
            !rendered.contains("lo"),
            "debug output leaked span: {rendered}"
        );
        assert!(
            !rendered.contains("hi"),
            "debug output leaked span: {rendered}"
        );
    }
}
