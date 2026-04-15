use std::fmt::{self, Display};

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

pub struct CompilationErrorPrettyPrinter<'a, 'b> {
    error: &'a CompilationError,
    source: &'b str,
}

impl Display for CompilationErrorPrettyPrinter<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Error: {}", self.error.reason)?;
        let mut offset = 0;
        let mut lines = self.source.lines().enumerate().peekable();
        while let Some(&(_, line)) = lines.peek() {
            if offset + line.len() > self.error.span.lo {
                break;
            }
            offset += line.len() + 1;
            lines.next();
        }
        if let Some(&(line_no, _)) = lines.peek() {
            let col = self.error.span.lo - offset + 1;
            writeln!(f, " --> {}:{}:", line_no + 1, col)?;
            writeln!(f, "    |")?;
        }
        for (line_no, line) in lines {
            let mut highlight = String::with_capacity(line.len());
            for c in line.chars() {
                highlight.push(
                    if offset >= self.error.span.lo && offset < self.error.span.hi {
                        '^'
                    } else {
                        ' '
                    },
                );
                offset += c.len_utf8();
            }
            offset += 1;
            writeln!(f, "{:>3} | {}\n    | {}", line_no + 1, line, highlight)?;
            if offset >= self.error.span.hi {
                break;
            }
        }
        Ok(())
    }
}

impl CompilationError {
    pub fn pprint_with_source<'a, 'b>(
        &'a self,
        source: &'b str,
    ) -> CompilationErrorPrettyPrinter<'a, 'b> {
        CompilationErrorPrettyPrinter {
            error: self,
            source,
        }
    }
}
