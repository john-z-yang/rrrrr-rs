# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

rrrrr-rs is an R5RS Scheme interpreter project written in Rust (edition 2024). The current implementation includes a lexer, parser, and hygienic macro expansion system with an interactive REPL, with later phases (such as semantic analysis) planned after expansion.

## Build Commands

```bash
cargo build              # Build the binary
cargo test               # Run all tests (currently 104 unit tests)
cargo test <test_name>   # Run a single test by name
cargo test compile::lex  # Run tests for a specific module
```

The only external dependency is `rustyline` (for the REPL).

## Using the REPL

The REPL (`cargo run`) reads lines until an **empty line** is entered, then evaluates the accumulated input. To pipe expressions non-interactively, use `printf` with `\n\n` after each expression:

```bash
printf '(lambda (x) x)\n\n' | cargo run
printf "'(1 2 3)\n\n(quote hello)\n\n" | cargo run   # multiple expressions
```

## Architecture

The codebase follows a compiler pipeline:

```
Input (REPL) → Lexer → Parser → Expander → (Planned) Semantic Analysis → Output
```

All compilation modules live under `src/compile/`:

- **lex.rs / token.rs** — Tokenizer. Converts source strings into `Token` enum values (ids, numbers, strings, booleans, characters, parens, etc.).
- **parse.rs** — Parser. Converts tokens into S-expressions (nested `Cons` cells). Handles quoted forms, quasiquote, vectors, dotted pairs.
- **sexpr.rs** — Core AST type. `SExpr` enum with variants for Id, Cons, Nil, Bool, Num, Char, Str, Vector. Every node carries a `Span`. Identifiers carry a `BTreeSet<ScopeId>` for hygienic macro tracking.
- **expand.rs** — Macro expander. Entry points: `introduce()` adds core scope to identifiers, `expand()` recursively expands forms (`quote`, `quote-syntax`, `lambda`, `define`, `set!`, `begin`, `letrec-syntax`, macro applications). It tracks context (`TopLevel`, `Expression`, `Body`) to reject invalid `define` usage in expression positions and to normalize lambda bodies (including leading internal-definition groups and spliced `begin` forms) before body expansion.
- **transformer.rs** — Implements R5RS `syntax-rules` pattern matching and template instantiation, including ellipsis (`...`) repetition.
- **bindings.rs** — Scope-based name resolution for hygienic macros. Maps symbols to binding candidates with scope sets. Core bindings are defined in `Bindings::CORE_BINDINGS`: `letrec-syntax`, `syntax-rules`, `quote`, `quote-syntax`, `if`, `lambda`, `define`, `set!`, `begin`, `list`, `cons`, `first`, `second`, `rest`.
- **span.rs** — Source position tracking (lo, hi) for error reporting.
- **compilation_error.rs** — `CompilationError` type (span + reason) with pretty-printed source location display. Defines `Result<T>` alias used by the lexer, parser, and expander.
- **util.rs** — Helper macros: `sexpr!` (construct S-expressions), `if_let_sexpr!` (single-pattern destructuring, like `if let`), `match_sexpr!` (multi-arm pattern match, like `match`), `template_sexpr!` (construct templates). Also list helpers such as `first()`, `rest()`, `len()`, `dotted_tail()`, and traversal helpers `try_for_each()`, `try_map()`.

**main.rs** — REPL loop using rustyline, accumulates multi-line expressions, runs the compile pipeline. Errors from any stage are pretty-printed with source context.

## Key Concepts

- **Hygienic macros**: Each expansion creates new scopes tracked via `ScopeId`. Binding resolution picks the candidate whose scope set is the largest subset of the identifier's scopes, preventing variable capture.
- **S-expression representation**: Lists are nested Cons cells (car/cdr). All values carry `Span` for error reporting through expansions.
- **Phase split**: Expansion currently focuses on binding/context correctness and macro hygiene. A semantic analysis phase is planned after expansion, so some malformed expression shapes may be tolerated during expansion and rejected later.
- **Tests**: Unit tests are inline (`#[cfg(test)]`) in each module, not in a separate test directory.

## Current Expander Notes

- `letrec-syntax` currently accepts a single body expression shape: `(letrec-syntax (spec ...) body)`.
- `begin` is a core binding and can be lexically shadowed; body normalization respects lexical binding resolution.
- Expansion is not the final validation pass: when behavior is intentionally deferred, malformed forms can survive expansion and should be diagnosed by semantic analysis once that phase is implemented.
