use std::fmt;

use super::span::Span;

#[derive(Debug, Clone)]
pub(crate) struct CompilationError {
    pub(crate) span: Span,
    pub(crate) reason: String,
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.span, self.reason)
    }
}

impl CompilationError {
    pub(crate) fn pprint_with_source(&self, source: &str) {
        println!("Error: {}", self.reason);
        println!(" --> {}:", self.span);
        println!("    |");
        let mut offset = 0;
        for (line_no, line) in source.lines().enumerate() {
            if offset + line.len() <= self.span.lo {
                offset += line.len() + 1;
                continue;
            }
            let mut highlight = String::with_capacity(line.len());
            for c in line.chars() {
                highlight.push(if offset >= self.span.lo && offset < self.span.hi {
                    '^'
                } else {
                    ' '
                });
                offset += c.len_utf8();
            }
            offset += 1;
            println!("{:>3} | {}\n    | {}", line_no + 1, line, highlight);
            if offset >= self.span.hi {
                break;
            }
        }
        println!();
    }
}
