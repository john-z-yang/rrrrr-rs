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

## Building and Testing

The standard cargo incantations are available.

There is a REPL available with `cargo run`, where it shows the ANF-lowered representation::

```scheme
lisp> (let loop ((n 10) (acc 1))
        (if (= n 0)
            acc
            (loop (- n 1) (* acc n))))
  ...
(let ((anf:18
       (λ (loop:8)
         (let ((anf:13
                (λ (temp:11)
                  (let ((anf:12 (set! loop:8 temp:11)))
                    loop:8))))
           (let ((anf:17
                  (λ (n:9 acc:10)
                    (let ((anf:14 (=:free n:9 0)))
                      (if anf:14
                          acc:10
                          (let ((anf:15 (-:free n:9 1)))
                            (let ((anf:16 (*:free acc:10 n:9)))
                              (loop:8 anf:15 anf:16))))))))
             (anf:13 anf:17))))))
  (let ((anf:19 (anf:18 #<void>)))
    (anf:19 10 1)))
lisp>
Farewell.
```

Run the tests with `cargo test`

Run the benchmarks with `cargo bench`

## AI use

For this project I intentionally try to keep AI / coding assistant usage minimal. However, Claude Code has been very useful when I get stuck, since I don’t have many people to talk to about esoteric programming language papers from the 90s.

Here are the parts AI helped with:

- Partial expansion algorithm for `lambda` / syntax binding bodies
- `MatchedSExprs` trick for tracking cardinality during `...` pattern/template expansion
- Code and documentation reviews
- Migration from a binary project to a library project
- Migration from unit tests to integration tests
