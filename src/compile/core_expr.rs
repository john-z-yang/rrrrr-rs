use std::fmt::{Debug, Display};

use crate::compile::{
    ident::{Resolved, Symbol},
    sexpr::SExpr,
    span::Span,
};

#[derive(Clone, PartialEq)]
pub enum Expr {
    Literal(SExpr<Symbol>),
    Var(Resolved, Span),
    Lambda(Lambda, Span),
    Application(Application, Span),
    Letrec(Letrec, Span),
    If(If, Span),
    Define(Define, Span),
    Set(Set, Span),
    Begin(Begin, Span),
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Literal(sexpr) => {
                if matches!(sexpr, SExpr::Var(..) | SExpr::Cons(..) | SExpr::Nil(..)) {
                    write!(f, "'")?;
                }
                write!(f, "{}", sexpr)
            }
            Expr::Var(resolved, _) => write!(f, "{}", resolved),
            Expr::Lambda(lambda, _) => write!(f, "{}", lambda),
            Expr::Application(application, _) => write!(f, "{}", application),
            Expr::Letrec(letrec, _) => write!(f, "{}", letrec),
            Expr::If(iff, _) => write!(f, "{}", iff),
            Expr::Define(define, _) => write!(f, "{}", define),
            Expr::Set(set, _) => write!(f, "{}", set),
            Expr::Begin(begin, _) => write!(f, "{}", begin),
        }
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Literal(arg0) => write!(f, "Literal({})", arg0),
            Self::Var(arg0, arg1) => f.debug_tuple("Var").field(arg0).field(arg1).finish(),
            Self::Lambda(arg0, arg1) => f.debug_tuple("Lambda").field(arg0).field(arg1).finish(),
            Self::Application(arg0, arg1) => f
                .debug_tuple("Application")
                .field(arg0)
                .field(arg1)
                .finish(),
            Self::Letrec(arg0, arg1) => f.debug_tuple("Letrec").field(arg0).field(arg1).finish(),
            Self::If(arg0, arg1) => f.debug_tuple("If").field(arg0).field(arg1).finish(),
            Self::Define(arg0, arg1) => f.debug_tuple("Define").field(arg0).field(arg1).finish(),
            Self::Set(arg0, arg1) => f.debug_tuple("Set").field(arg0).field(arg1).finish(),
            Self::Begin(arg0, arg1) => f.debug_tuple("Begin").field(arg0).field(arg1).finish(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Lambda {
    pub args: Vec<Symbol>,
    pub var_arg: Option<Symbol>,
    pub body: Vec<Expr>,
}

impl Display for Lambda {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(λ ")?;
        match (self.args.is_empty(), &self.var_arg) {
            (true, None) => write!(f, "()")?,
            (true, Some(var_arg)) => write!(f, "{}", var_arg)?,
            (false, _) => {
                write!(f, "(")?;
                for (i, arg) in self.args.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                if let Some(var_arg) = &self.var_arg {
                    write!(f, " . {})", var_arg)?;
                } else {
                    write!(f, ")")?;
                }
            }
        }
        for expr in self.body.iter() {
            write!(f, " {}", expr)?;
        }
        write!(f, ")")
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Application {
    pub operand: Box<Expr>,
    pub args: Vec<Expr>,
}

impl Display for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}", self.operand)?;
        for arg in self.args.iter() {
            write!(f, " {}", arg)?;
        }
        write!(f, ")")
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Letrec {
    pub initializers: Vec<(Symbol, Expr)>,
    pub body: Vec<Expr>,
}

impl Display for Letrec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(letrec (")?;
        for (i, (symbol, expr)) in self.initializers.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "({} {})", symbol, expr)?;
        }
        write!(f, ")")?;
        for expr in self.body.iter() {
            write!(f, " {}", expr)?;
        }
        write!(f, ")")
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct If {
    pub test: Box<Expr>,
    pub conseq: Box<Expr>,
    pub alt: Box<Expr>,
}

impl Display for If {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(if {} {} {})", self.test, self.conseq, self.alt)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Define {
    pub var: Resolved,
    pub expr: Box<Expr>,
}

impl Display for Define {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(define {} {})", self.var, self.expr)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Set {
    pub var: Resolved,
    pub expr: Box<Expr>,
}

impl Display for Set {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(set! {} {})", self.var, self.expr)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Begin {
    pub body: Vec<Expr>,
}

impl Display for Begin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(begin")?;
        for expr in self.body.iter() {
            write!(f, " {}", expr)?;
        }
        write!(f, ")")
    }
}
