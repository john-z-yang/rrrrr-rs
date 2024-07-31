use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug)]
pub(crate) struct SourceLoc {
    pub(crate) line: usize,
    pub(crate) idx: usize,
    pub(crate) width: usize,
}

impl SourceLoc {
    pub(crate) fn combine(self, other: Self) -> Self {
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
