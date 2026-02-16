# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

r5rs-rs is an R5RS Scheme macro expander written in Rust (edition 2024). It implements a lexer, parser, and hygienic macro expansion system with an interactive REPL.

## Build Commands

```bash
cargo build              # Build the binary
cargo test               # Run all tests (62 unit tests)
cargo test <test_name>   # Run a single test by name
cargo test compile::lex  # Run tests for a specific module
```

The only external dependency is `rustyline` (for the REPL).

## Architecture

The codebase follows a compiler pipeline:

```
Input (REPL) → Lexer → Parser → Expander → Output
```

All compilation modules live under `src/compile/`:

- **lex.rs / token.rs** — Tokenizer. Converts source strings into `Token` enum values (ids, numbers, strings, booleans, characters, parens, etc.).
- **parse.rs** — Parser. Converts tokens into S-expressions (nested `Cons` cells). Handles quoted forms, quasiquote, vectors, dotted pairs.
- **sexpr.rs** — Core AST type. `SExpr` enum with variants for Id, Cons, Nil, Bool, Num, Char, Str, Vector. Every node carries a `Span`. Identifiers carry a `BTreeSet<ScopeId>` for hygienic macro tracking.
- **expand.rs** — Macro expander. Entry points: `introduce()` adds core scope to identifiers, `expand()` recursively expands forms (`quote`, `lambda`, `letrec-syntax`, macro applications).
- **transformer.rs** — Implements R5RS `syntax-rules` pattern matching and template instantiation, including ellipsis (`...`) repetition.
- **bindings.rs** — Scope-based name resolution for hygienic macros. Maps symbols to binding candidates with scope sets. Core bindings: `letrec-syntax`, `if`, `lambda`, `list`, `cons`, `first`, `second`, `rest`.
- **span.rs** — Source position tracking (lo, hi) for error reporting.
- **compilation_error.rs** — `CompilationError` type (span + reason) with pretty-printed source location display. Defines `Result<T>` alias used by the lexer, parser, and expander.
- **util.rs** — Helper macros: `sexpr!` (construct S-expressions), `match_sexpr!` (pattern match), `template_sexpr!` (construct templates). Also `first()`, `try_for_each()`, `try_map()` utility functions.

**main.rs** — REPL loop using rustyline, accumulates multi-line expressions, runs the compile pipeline. Errors from any stage are pretty-printed with source context.

## Key Concepts

- **Hygienic macros**: Each expansion creates new scopes tracked via `ScopeId`. Binding resolution picks the candidate whose scope set is the largest subset of the identifier's scopes, preventing variable capture.
- **S-expression representation**: Lists are nested Cons cells (car/cdr). All values carry `Span` for error reporting through expansions.
- **Tests**: Unit tests are inline (`#[cfg(test)]`) in each module, not in a separate test directory.
