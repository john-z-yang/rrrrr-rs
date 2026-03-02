use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub lo: usize,
    pub hi: usize,
}

impl Span {
    pub(crate) fn combine(self, other: Self) -> Self {
        Span {
            lo: std::cmp::min(self.lo, other.lo),
            hi: std::cmp::max(self.hi, other.hi),
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "lo: {}, hi: {}", self.lo, self.hi)
    }
}
