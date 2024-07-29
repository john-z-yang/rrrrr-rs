use std::{cmp::min, fmt};

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
        let col = self.source_loc.idx
            - source
                .lines()
                .take(self.source_loc.line)
                .map(|line| line.len() + 1)
                .sum::<usize>();
        let lines_iter = source.lines().skip(self.source_loc.line);
        let mut line_no = self.source_loc.line;
        let mut width_remaining = self.source_loc.width;
        for line in lines_iter {
            let highlight = if line_no == self.source_loc.line {
                format!(
                    "{}{}",
                    " ".repeat(col),
                    "^".repeat(min(line.len() - col, width_remaining))
                )
            } else {
                "^".repeat(min(line.len() - col, width_remaining))
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
