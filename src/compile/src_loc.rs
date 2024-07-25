use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub struct SourceLoc {
    pub line: usize,
    pub col: usize,
    pub width: usize,
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line: {}, col: {}", self.line, self.col)
    }
}
