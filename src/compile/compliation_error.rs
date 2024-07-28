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

impl CompliationError {
    pub fn pprint_with_source(&self, source: &String) {
        println!("Error: {}", self.reason);
        println!("  --> {}:\n", self.source_loc);
        let mut line_iter = source.lines().skip(self.source_loc.line);
        let mut line_no = self.source_loc.line;
        let mut width_remaining = self.source_loc.width;
        while width_remaining > 0 {
            let line_text = line_iter.next().unwrap();
            let highlight = if line_no == self.source_loc.line {
                format!(
                    "{}{}",
                    " ".repeat(self.source_loc.col),
                    "^".repeat(line_text.len() - self.source_loc.col)
                )
            } else {
                "^".repeat(line_text.len())
            };
            println!("{} | {}\n    {}\n", line_no, line_text, highlight);
            line_no += 1;
            width_remaining -= highlight.chars().filter(|c| *c == '^').count();
        }
    }
}
