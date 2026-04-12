use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(symbol: &str) -> Self {
        Symbol(symbol.to_string())
    }
}

impl From<ResolvedSymbol> for Symbol {
    fn from(value: ResolvedSymbol) -> Self {
        match value {
            ResolvedSymbol::Bound { symbol, .. } => symbol,
            ResolvedSymbol::Free { symbol } => symbol,
            ResolvedSymbol::Literal { symbol } => symbol,
        }
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub enum ResolvedSymbol {
    Bound { symbol: Symbol, binding: Symbol },
    Free { symbol: Symbol },
    Literal { symbol: Symbol },
}

impl fmt::Display for ResolvedSymbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolvedSymbol::Bound { binding, .. } => write!(f, "{}", binding),
            ResolvedSymbol::Free { symbol } => write!(f, "{}:free", symbol),
            ResolvedSymbol::Literal { symbol } => write!(f, "{}", symbol),
        }
    }
}

#[derive(PartialEq, Clone, Eq, Hash, Debug)]
pub enum ResolvedVar {
    Bound { symbol: Symbol, binding: Symbol },
    Free { symbol: Symbol },
}

impl TryFrom<ResolvedSymbol> for ResolvedVar {
    type Error = ResolvedSymbol;

    fn try_from(value: ResolvedSymbol) -> Result<Self, Self::Error> {
        match value {
            ResolvedSymbol::Bound { symbol, binding } => Ok(ResolvedVar::Bound { symbol, binding }),
            ResolvedSymbol::Free { symbol } => Ok(ResolvedVar::Free { symbol }),
            ResolvedSymbol::Literal { .. } => Err(value),
        }
    }
}

impl fmt::Display for ResolvedVar {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResolvedVar::Bound { binding, .. } => write!(f, "{}", binding),
            ResolvedVar::Free { symbol } => write!(f, "{}:free", symbol),
        }
    }
}
