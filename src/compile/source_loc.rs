use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug)]
pub struct SourceLoc {
    pub line: usize,
    pub idx: usize,
    pub width: usize,
}

impl SourceLoc {
    pub fn combine(self, other: Self) -> Self {
        let (before, after) = if other < self {
            (other, self)
        } else {
            (self, other)
        };
        SourceLoc {
            line: before.line,
            idx: before.idx,
            width: after.idx - before.idx + after.width,
        }
    }
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "line: {}, idx: {}", self.line, self.idx)
    }
}
