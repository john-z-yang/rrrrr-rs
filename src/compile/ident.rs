use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(symbol: &str) -> Self {
        Symbol(symbol.to_string())
    }
}

impl From<Resolved> for Symbol {
    fn from(value: Resolved) -> Self {
        match value {
            Resolved::Bound { symbol, .. } => symbol,
            Resolved::Free { symbol } => symbol,
            Resolved::Literal { symbol } => symbol,
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub enum Resolved {
    Bound { symbol: Symbol, binding: Symbol },
    Free { symbol: Symbol },
    Literal { symbol: Symbol },
}

impl fmt::Display for Resolved {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Resolved::Bound { binding, .. } => write!(f, "{}", binding),
            Resolved::Free { symbol } => write!(f, "{}", symbol),
            Resolved::Literal { symbol } => write!(f, "{}", symbol),
        }
    }
}
