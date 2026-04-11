# RRRRR-RS &middot; [![Rust](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml)


A compiler frontend for [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/).

Next step: convince future me to write the backend.

This repository started as an excuse to learn Rust by implementing Flatt’s *Bindings as Sets of Scopes* algorithm from his [paper](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf) and [Strange Loop 2016 talk](https://youtu.be/Or_yKiI3Ha4).

These days it has grown into a collection of compiler passes that I yoinked from papers, books, and articles I found interesting.


## The pipeline so far

| Pass                                          | Reference                                                                                                                                                     |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tokenization / parsing                        | [*Crafting Interpreters* — Robert Nystrom](https://craftinginterpreters.com/)                                                                                 |
| Hygienic macro expansion + binding resolution | [*Bindings as Sets of Scopes* — Matthew Flatt](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)                                                         |
| Quasiquotation                                | [*Quasiquotation in Lisp* — Alan Bawden](docs/references/Quasiquotation%20in%20Lisp.pdf)                                                                      |
| α-conversion                                  | [*Bindings as Sets of Scopes* — Matthew Flatt](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)                                                         |
| Lowering + `letrec` transformation            | [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/)                                                                  |
| A-normalization                               | [*The Essence of Compiling with Continuations* — Flanagan, Sabry, Duba, Felleisen](docs/references/The%20Essence%20of%20Compiling%20with%20Continuations.pdf) |

There is a REPL available with:

```bash
cargo run
```
