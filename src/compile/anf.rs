use std::{
    fmt::{Debug, Display},
    hash::{DefaultHasher, Hash, Hasher},
};

use crate::compile::{
    ident::{ResolvedVar, Symbol},
    sexpr::SExpr,
    span::Span,
};

#[derive(Clone, PartialEq, Debug, Hash)]
pub enum Expr {
    AExpr(AExpr),
    CExpr(CExpr),
    Let(Let, Span),
}

impl Expr {
    pub fn get_span(&self) -> Span {
        match self {
            Expr::AExpr(AExpr::Literal(sexpr)) => sexpr.get_span(),
            Expr::AExpr(AExpr::Var(_, span)) => *span,
            Expr::AExpr(AExpr::Lambda(_, span)) => *span,
            Expr::CExpr(CExpr::Application(_, span)) => *span,
            Expr::CExpr(CExpr::If(_, span)) => *span,
            Expr::CExpr(CExpr::Set(_, span)) => *span,
            Expr::Let(_, span) => *span,
        }
    }

    pub fn calculate_hash(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.hash(&mut s);
        s.finish()
    }
}

impl From<Value> for Expr {
    fn from(value: Value) -> Self {
        Expr::AExpr(value.into())
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", ExprPrettyPrinter(self))
    }
}

#[derive(Clone, PartialEq, Hash)]
pub enum AExpr {
    Literal(SExpr<Symbol>),
    Var(ResolvedVar, Span),
    Lambda(Lambda, Span),
}

impl From<Value> for AExpr {
    fn from(value: Value) -> Self {
        match value {
            Value::Literal(sexpr) => AExpr::Literal(sexpr),
            Value::Var(resolved, span) => AExpr::Var(resolved, span),
        }
    }
}

impl Debug for AExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Literal(arg0) => write!(f, "Literal({})", arg0),
            Self::Var(arg0, arg1) => f.debug_tuple("Var").field(arg0).field(arg1).finish(),
            Self::Lambda(arg0, arg1) => f.debug_tuple("Lambda").field(arg0).field(arg1).finish(),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub enum CExpr {
    Application(Application, Span),
    If(If, Span),
    Set(Set, Span),
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub enum Rhs {
    AExpr(AExpr),
    CExpr(CExpr),
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub enum Value {
    Literal(SExpr<Symbol>),
    Var(ResolvedVar, Span),
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct Let {
    pub initializer: Box<(Symbol, Rhs)>,
    pub body: Box<Expr>,
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct Begin {
    pub body: Vec<Expr>,
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct Lambda {
    pub args: Vec<Symbol>,
    pub var_arg: Option<Symbol>,
    pub body: Box<Expr>,
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct Application {
    pub operand: Box<Value>,
    pub args: Vec<Value>,
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct If {
    pub test: Box<Value>,
    pub conseq: Box<Expr>,
    pub alt: Box<Expr>,
}

#[derive(Clone, PartialEq, Debug, Hash)]
pub struct Set {
    pub var: ResolvedVar,
    pub aexpr: Value,
}

struct ExprPrettyPrinter<'a>(&'a Expr);

impl<'a> ExprPrettyPrinter<'a> {
    fn is_multilined_aexpr(aexpr: &AExpr) -> bool {
        !matches!(aexpr, AExpr::Literal(_) | AExpr::Var(..))
    }

    fn is_multilined_cexpr(cexpr: &CExpr) -> bool {
        match cexpr {
            CExpr::Application(..) | CExpr::Set(..) => false,
            CExpr::If(If { conseq, alt, .. }, _) => {
                Self::is_multilined_expr(conseq) || Self::is_multilined_expr(alt)
            }
        }
    }

    fn is_multilined_rhs(rhs: &Rhs) -> bool {
        match rhs {
            Rhs::AExpr(aexpr) => Self::is_multilined_aexpr(aexpr),
            Rhs::CExpr(cexpr) => Self::is_multilined_cexpr(cexpr),
        }
    }

    fn is_multilined_expr(expr: &Expr) -> bool {
        match expr {
            Expr::AExpr(aexpr) => Self::is_multilined_aexpr(aexpr),
            Expr::CExpr(cexpr) => Self::is_multilined_cexpr(cexpr),
            Expr::Let(..) => true,
        }
    }

    fn write_indent(f: &mut std::fmt::Formatter<'_>, n: usize) -> std::fmt::Result {
        write!(f, "{}", " ".repeat(n))
    }

    fn fmt_expr(
        expr: &Expr,
        f: &mut std::fmt::Formatter<'_>,
        indent_level: usize,
    ) -> std::fmt::Result {
        match expr {
            Expr::AExpr(aexpr) => Self::fmt_aexpr(aexpr, f, indent_level),
            Expr::CExpr(cexpr) => Self::fmt_cexpr(cexpr, f, indent_level),
            Expr::Let(Let { initializer, body }, _) => {
                Self::write_indent(f, indent_level)?;
                let (sym, rhs) = initializer.as_ref();
                write!(f, "(let (({}", sym)?;
                if Self::is_multilined_rhs(rhs) {
                    writeln!(f)?;
                    match rhs {
                        Rhs::AExpr(aexpr) => Self::fmt_aexpr(aexpr, f, indent_level + 7)?,
                        Rhs::CExpr(cexpr) => Self::fmt_cexpr(cexpr, f, indent_level + 7)?,
                    }
                } else {
                    write!(f, " ")?;
                    match rhs {
                        Rhs::AExpr(aexpr) => Self::fmt_aexpr(aexpr, f, 0)?,
                        Rhs::CExpr(cexpr) => Self::fmt_cexpr(cexpr, f, 0)?,
                    }
                }
                writeln!(f, "))")?;
                Self::fmt_expr(body, f, indent_level + 2)?;
                write!(f, ")")
            }
        }
    }

    fn fmt_value(
        value: &Value,
        f: &mut std::fmt::Formatter<'_>,
        indent_level: usize,
    ) -> std::fmt::Result {
        Self::write_indent(f, indent_level)?;
        match value {
            Value::Literal(sexpr) => {
                if matches!(sexpr, SExpr::Var(..) | SExpr::Cons(..) | SExpr::Nil(..)) {
                    write!(f, "'")?;
                }
                write!(f, "{}", sexpr)
            }
            Value::Var(resolved, _) => write!(f, "{}", resolved),
        }
    }

    fn fmt_aexpr(
        aexpr: &AExpr,
        f: &mut std::fmt::Formatter<'_>,
        indent_level: usize,
    ) -> std::fmt::Result {
        Self::write_indent(f, indent_level)?;
        match aexpr {
            AExpr::Literal(sexpr) => {
                if matches!(sexpr, SExpr::Var(..) | SExpr::Cons(..) | SExpr::Nil(..)) {
                    write!(f, "'")?;
                }
                write!(f, "{}", sexpr)
            }
            AExpr::Var(resolved, _) => write!(f, "{}", resolved),
            AExpr::Lambda(
                Lambda {
                    args,
                    var_arg,
                    body,
                },
                _,
            ) => {
                write!(f, "(λ ")?;
                match (args.is_empty(), var_arg) {
                    (true, None) => write!(f, "()")?,
                    (true, Some(var_arg)) => write!(f, "{}", var_arg)?,
                    (false, _) => {
                        write!(f, "(")?;
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                write!(f, " ")?;
                            }
                            write!(f, "{}", arg)?;
                        }
                        if let Some(var_arg) = var_arg {
                            write!(f, " . {})", var_arg)?;
                        } else {
                            write!(f, ")")?;
                        }
                    }
                }
                if Self::is_multilined_expr(body.as_ref()) {
                    writeln!(f)?;
                    Self::fmt_expr(body, f, indent_level + 2)?;
                } else {
                    write!(f, " ")?;
                    Self::fmt_expr(body, f, 0)?;
                }
                write!(f, ")")
            }
        }
    }

    fn fmt_cexpr(
        cexpr: &CExpr,
        f: &mut std::fmt::Formatter<'_>,
        indent_level: usize,
    ) -> std::fmt::Result {
        Self::write_indent(f, indent_level)?;
        match cexpr {
            CExpr::Application(Application { operand, args }, _) => {
                write!(f, "(")?;
                Self::fmt_value(operand, f, 0)?;
                for arg in args.iter() {
                    write!(f, " ")?;
                    Self::fmt_value(arg, f, 0)?;
                }
                write!(f, ")")
            }
            CExpr::If(If { test, conseq, alt }, _) => {
                if Self::is_multilined_expr(conseq) || Self::is_multilined_expr(alt) {
                    write!(f, "(if ")?;
                    Self::fmt_value(test, f, 0)?;
                    writeln!(f)?;
                    Self::fmt_expr(conseq, f, indent_level + 4)?;
                    writeln!(f)?;
                    Self::fmt_expr(alt, f, indent_level + 4)?;
                    write!(f, ")")
                } else {
                    write!(f, "(if ")?;
                    Self::fmt_value(test, f, 0)?;
                    write!(f, " ")?;
                    Self::fmt_expr(conseq, f, 0)?;
                    write!(f, " ")?;
                    Self::fmt_expr(alt, f, 0)?;
                    write!(f, ")")
                }
            }
            CExpr::Set(Set { var, aexpr }, _) => {
                write!(f, "(set! {}", var)?;
                write!(f, " ")?;
                Self::fmt_value(aexpr, f, 0)?;
                write!(f, ")")
            }
        }
    }
}

impl<'a> Display for ExprPrettyPrinter<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        ExprPrettyPrinter::fmt_expr(self.0, f, 0)
    }
}
