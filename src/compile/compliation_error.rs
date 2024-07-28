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
    pub fn pprint_with_source(&self, source: &str) {
        println!("Error: {}", self.reason);
        println!(" --> {}:", self.source_loc);
        println!("  |");
        let lines_iter = source.lines().skip(self.source_loc.line);
        let mut line_no = self.source_loc.line;
        let mut width_remaining = self.source_loc.width;
        for line in lines_iter {
            let highlight = if line_no == self.source_loc.line {
                format!(
                    "{}{}",
                    " ".repeat(self.source_loc.col),
                    "^".repeat(line.len() - self.source_loc.col)
                )
            } else {
                "^".repeat(line.len())
            };
            println!("{} | {}\n  | {}", line_no + 1, line, highlight);
            line_no += 1;
            width_remaining -= highlight.chars().filter(|c| *c == '^').count();
            if width_remaining == 0 {
                break;
            }
            width_remaining -= 1;
        }
        println!();
    }
}
