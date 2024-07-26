use std::fmt;

use super::src_loc::SourceLoc;

#[derive(Debug, Clone)]
pub(crate) struct CompliationError {
    pub(crate) source_loc: SourceLoc,
    pub(crate) reason: String,
}

impl fmt::Display for CompliationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.source_loc, self.reason)
    }
}
