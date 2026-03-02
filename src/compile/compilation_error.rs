use std::fmt;

use super::span::Span;

pub type Result<T> = std::result::Result<T, CompilationError>;

#[derive(Debug, Clone)]
pub struct CompilationError {
    pub span: Span,
    pub reason: String,
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.span, self.reason)
    }
}

impl std::error::Error for CompilationError {}

impl CompilationError {
    pub fn pprint_with_source(&self, source: &str) {
        println!("Error: {}", self.reason);
        let mut offset = 0;
        let mut lines = source.lines().enumerate().peekable();
        while let Some(&(_, line)) = lines.peek() {
            if offset + line.len() > self.span.lo {
                break;
            }
            offset += line.len() + 1;
            lines.next();
        }
        if let Some(&(line_no, _)) = lines.peek() {
            let col = self.span.lo - offset + 1;
            println!(" --> {}:{}:", line_no + 1, col);
            println!("    |");
        }
        for (line_no, line) in lines {
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
