use std::{fmt, hash::Hash};

use crate::compile::bindings::Id;

use super::{
    bindings::{ScopeId, Scopes},
    span::Span,
};

#[derive(PartialEq, Clone, Debug, Hash)]
pub enum SExpr<T> {
    Var(T, Span),
    Cons(Cons<T>, Span),
    Nil(Span),
    Bool(Bool, Span),
    Num(Num, Span),
    Char(Char, Span),
    Str(Str, Span),
    Vector(Vector<T>, Span),
    Void(Span),
}

impl<T> SExpr<T> {
    pub fn without_spans(&self) -> SExprWithoutSpans<'_, T> {
        SExprWithoutSpans(self)
    }

    pub fn get_span(&self) -> Span {
        *match self {
            SExpr::Var(_, span) => span,
            SExpr::Cons(_, span) => span,
            SExpr::Nil(span) => span,
            SExpr::Bool(_, span) => span,
            SExpr::Num(_, span) => span,
            SExpr::Char(_, span) => span,
            SExpr::Str(_, span) => span,
            SExpr::Vector(_, span) => span,
            SExpr::Void(span) => span,
        }
    }

    pub fn update_span(&mut self, span: Span) {
        *match self {
            SExpr::Var(_, span) => span,
            SExpr::Cons(_, span) => span,
            SExpr::Nil(span) => span,
            SExpr::Bool(_, span) => span,
            SExpr::Num(_, span) => span,
            SExpr::Char(_, span) => span,
            SExpr::Str(_, span) => span,
            SExpr::Vector(_, span) => span,
            SExpr::Void(span) => span,
        } = span;
    }

    pub fn cons(car: SExpr<T>, cdr: SExpr<T>) -> Self {
        let start = car.get_span();
        let end = cdr.get_span();
        Self::Cons(Cons::new(car, cdr), start.combine(end))
    }

    pub fn map_var<U, F>(self, f: &F) -> SExpr<U>
    where
        F: Fn(T) -> U,
    {
        match self {
            SExpr::Var(var, span) => SExpr::Var(f(var), span),
            SExpr::Cons(cons, span) => {
                SExpr::Cons(Cons::new(cons.car.map_var(f), cons.cdr.map_var(f)), span)
            }
            SExpr::Nil(span) => SExpr::Nil(span),
            SExpr::Bool(bool, span) => SExpr::Bool(bool, span),
            SExpr::Num(num, span) => SExpr::Num(num, span),
            SExpr::Char(char, span) => SExpr::Char(char, span),
            SExpr::Str(str, span) => SExpr::Str(str, span),
            SExpr::Vector(vector, span) => SExpr::Vector(
                Vector(vector.0.into_iter().map(|sexpr| sexpr.map_var(f)).collect()),
                span,
            ),
            SExpr::Void(span) => SExpr::Void(span),
        }
    }

    pub fn is_atomic(&self) -> bool {
        !matches!(self, SExpr::Cons(..) | SExpr::Vector(..) | SExpr::Str(..))
    }
}

impl SExpr<Id> {
    fn adjust_scope<F>(self, op: &F) -> Self
    where
        F: Fn(&Scopes) -> Scopes,
    {
        match self {
            Self::Var(id, span) => Self::Var(id.adjust_scope(op), span),
            Self::Cons(cons, span) => Self::Cons(
                Cons::new(cons.car.adjust_scope(op), cons.cdr.adjust_scope(op)),
                span,
            ),
            Self::Vector(vector, span) => Self::Vector(
                Vector(
                    vector
                        .0
                        .into_iter()
                        .map(|sexpr| sexpr.adjust_scope(op))
                        .collect(),
                ),
                span,
            ),
            _ => self,
        }
    }

    pub fn add_scope(self, scope: ScopeId) -> Self {
        let op = |scopes: &Scopes| {
            let mut scopes = scopes.clone();
            scopes.insert(scope);
            scopes
        };
        self.adjust_scope(&op)
    }

    pub fn flip_scope(self, scope: ScopeId) -> Self {
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

impl TryFrom<SExpr<Id>> for Id {
    type Error = ();
    fn try_from(value: SExpr<Id>) -> Result<Self, Self::Error> {
        if let SExpr::Var(id, _) = value {
            Ok(id)
        } else {
            Err(())
        }
    }
}

impl<T: fmt::Display> fmt::Display for SExpr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SExpr::Var(var, _) => {
                write!(f, "{}", var)
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
            SExpr::Void(_) => {
                write!(f, "#<void>")
            }
        }
    }
}

pub trait ListAccess
where
    Self: std::marker::Sized,
{
    fn try_destruct(self) -> Option<(Self, Self)>;
}

impl<T> ListAccess for SExpr<T> {
    fn try_destruct(self) -> Option<(Self, Self)> {
        match self {
            SExpr::Cons(Cons { car, cdr }, _) => Some((*car, *cdr)),
            _ => None,
        }
    }
}

impl<T> ListAccess for &SExpr<T> {
    fn try_destruct(self) -> Option<(Self, Self)> {
        match self {
            SExpr::Cons(Cons { car, cdr }, _) => Some((car, cdr)),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SExprWithoutSpans<'a, T>(&'a SExpr<T>);

impl<T: PartialEq> PartialEq for SExprWithoutSpans<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        fn eq_sexpr_without_spans<T: PartialEq>(left: &SExpr<T>, right: &SExpr<T>) -> bool {
            match (left, right) {
                (SExpr::Var(var, _), SExpr::Var(other, _)) => var == other,
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
                (SExpr::Void(_), SExpr::Void(_)) => true,
                _ => false,
            }
        }

        eq_sexpr_without_spans(self.0, other.0)
    }
}

impl<T: fmt::Debug> fmt::Debug for SExprWithoutSpans<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SExpr::Var(var, _) => f.debug_tuple("Var").field(var).finish(),
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
            SExpr::Void(_) => f.write_str("#<void>"),
        }
    }
}

#[derive(PartialEq, Clone, Debug, Hash)]
pub struct Cons<T> {
    pub car: Box<SExpr<T>>,
    pub cdr: Box<SExpr<T>>,
}

impl<T> Cons<T> {
    pub fn new(car: SExpr<T>, cdr: SExpr<T>) -> Self {
        Cons {
            car: Box::new(car),
            cdr: Box::new(cdr),
        }
    }

    pub fn try_into_vector(self, span: Span) -> Option<SExpr<T>> {
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
}

impl<T> From<Cons<T>> for (SExpr<T>, SExpr<T>) {
    fn from(value: Cons<T>) -> Self {
        (*value.car, *value.cdr)
    }
}

impl<'a, T> From<&'a Cons<T>> for (&'a SExpr<T>, &'a SExpr<T>) {
    fn from(value: &'a Cons<T>) -> Self {
        (value.car.as_ref(), value.cdr.as_ref())
    }
}

impl<T: fmt::Display> Cons<T> {
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

impl<T: fmt::Display> fmt::Display for Cons<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        self.fmt_disp(f)
    }
}

#[derive(Debug, PartialEq, Clone, Hash)]
pub struct Bool(pub bool);

impl fmt::Display for Bool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", if self.0 { "#t" } else { "#f" })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Num(pub f64);

impl Hash for Num {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Clone, Hash)]
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

#[derive(Debug, PartialEq, Clone, Hash)]
pub struct Str(pub String);

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

#[derive(Debug, PartialEq, Clone, Hash)]
pub struct Vector<T>(pub Vec<SExpr<T>>);

impl<T> Vector<T> {
    pub fn into_cons_list(self, span: Span) -> SExpr<T> {
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

impl<T: fmt::Display> fmt::Display for Vector<T> {
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
