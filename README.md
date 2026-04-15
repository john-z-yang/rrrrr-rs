# RRRRR-RS &middot; [![Rust](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml)


> I hope the field of computer science never loses its sense of fun. Above all, I hope we don’t become missionaries. Don’t feel as if you’re Bible salesmen. The world has too many of those already.
>
> — Alan J. Perlis, *Structure and Interpretation of Computer Programs*, Dedication


A compiler front-end and middle-end for [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/). The back-end and VM are left as an exercise for my future self.

This repository started as an excuse to learn Rust by implementing Flatt’s *Bindings as Sets of Scopes* algorithm from his [paper](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)
and [Strange Loop 2016 talk](https://youtu.be/Or_yKiI3Ha4).

These days it has grown into a small collection of compiler passes I yoinked from papers, books, and articles I found interesting.

## The pipeline so far

| Pass(es)                                                                                |                                                                                            References                                                                                             |
| :-------------------------------------------------------------------------------------- | :-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------: |
| • Tokenization<br>• Parsing                                                             |                                                           [*Crafting Interpreters* — Robert Nystrom](https://craftinginterpreters.com/)                                                           |
| • Hygienic macro & quasiquotation expansion                                             | [*Bindings as Sets of Scopes* — Matthew Flatt](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)<br>[*Quasiquotation in Lisp* — Alan Bawden](docs/references/Quasiquotation%20in%20Lisp.pdf) |
| • α-conversion                                                                          |                                                     [*Alpha Conversion* — Kevin Sookocheff](https://sookocheff.com/post/fp/alpha-conversion/)                                                     |
| • Lowering & `letrec` transformation                                                    |                                                   [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/)                                                    |
| • A-normalization                                                                       |                                                  [*A-Normalization: Why and How* — Matt Might](https://matt.might.net/articles/a-normalization/)                                                  |
| • β-contraction<br>• Copy propagation<br>• Const propagation<br>• Dead code elimination |                           [*The Essence of Compiling with Continuations* — Flanagan et al.](docs/references/The%20Essence%20of%20Compiling%20with%20Continuations.pdf)                            |

## Building and Testing

The standard cargo incantations are available.

There is a REPL available via `cargo run`. It prints the ANF-lowered representation after the full pipeline runs:

```scheme
lisp> (let loop ((n 10) (acc 1))
        (if (= n 0)
            acc
            (loop (- n 1) (* acc n))))
  ...
(let ((anf:18
       (λ (loop:8)
         (let ((anf:17
                (λ (n:9 acc:10)
                  (let ((anf:14 (=:free n:9 0)))
                    (if anf:14
                        acc:10
                        (let ((anf:15 (-:free n:9 1)))
                          (let ((anf:16 (*:free acc:10 n:9)))
                            (loop:8 anf:15 anf:16))))))))
           (let ((anf:12 (set! loop:8 anf:17)))
             loop:8)))))
  (let ((anf:19 (anf:18 #<void>)))
    (anf:19 10 1)))
lisp>
Farewell.
```

Run the tests with `cargo test`

Run the benchmarks with `cargo bench`


## AI use

For this project I intentionally try to keep AI / coding assistant usage minimal. However, Claude Code has been very useful when I get stuck, since I don’t have anyone to talk to about esoteric programming language papers from the 90s.

Here are the parts AI helped with:

- Partial expansion algorithm for `lambda` / syntax binding bodies
- `MatchedSExprs` trick for tracking cardinality during `...` pattern/template expansion
- Code and documentation reviews
- Code refactoring
