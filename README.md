# RRRRR-RS &middot; [![Rust](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/john-z-yang/rrrrr-rs/actions/workflows/rust.yml)


> I hope the field of computer science never loses its sense of fun. Above all, I hope we don’t become missionaries. Don’t feel as if you’re Bible salesmen. The world has too many of those already.
>
> — Alan J. Perlis, *Structure and Interpretation of Computer Programs*, Foreword


A compiler front-end and middle-end for [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/). The back-end and VM are left as an exercise for my future self.

This repository started as an excuse to learn Rust by implementing Flatt’s *Bindings as Sets of Scopes* algorithm from his [paper](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)
and [Strange Loop 2016 talk](https://youtu.be/Or_yKiI3Ha4).

These days it has grown into a collection of compiler passes I yoinked from papers, books, and articles I found interesting.

## The pipeline

| Pass(es)                                                                                                                    | References                                                                                                                                                                                                                                          |
| :-------------------------------------------------------------------------------------------------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| ↓ Tokenization<br>↓ Parsing                                                                                                 | • [*Crafting Interpreters* — Robert Nystrom](https://craftinginterpreters.com/)                                                                                                                                                                     |
| ↓ Hygienic macro & quasiquotation expansion                                                                                 | • [*Bindings as Sets of Scopes* — Matthew Flatt](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf)<br>• [*Quasiquotation in Lisp* — Alan Bawden](docs/references/Quasiquotation%20in%20Lisp.pdf)                                               |
| ↓ α-conversion                                                                                                              | • [*Alpha Conversion* — Kevin Sookocheff](https://sookocheff.com/post/fp/alpha-conversion/)                                                                                                                                                         |
| ↓ Lowering & `letrec` transformation                                                                                        | • [Revised(5) Scheme](https://conservatory.scheme.org/schemers/Documents/Standards/R5RS/HTML/)                                                                                                                                                      |
| ↓ A-normalization<br>↓ β-reduction<br>↓ η-reduction<br>↓ Copy propagation<br>↓ Const propagation<br>↓ Dead code elimination | • [*A-Normalization: Why and How* — Matt Might](https://matt.might.net/articles/a-normalization/)<br>• [*The Essence of Compiling with Continuations* — Flanagan et al.](docs/references/The%20Essence%20of%20Compiling%20with%20Continuations.pdf) |

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

## Details

### Hygienic macros & α-conversion

Macro expansion in the compiler front-end follows the "sets of scopes" model described in Flatt's [*Bindings as Sets of Scopes*](docs/references/Binding%20as%20Sets%20of%20Scopes.pdf) paper. It also has a pattern matcher for the `syntax-rules` pattern language defined in the R5RS specification. The expander operates on an S-expression, and spits out another S-expression where all macros are fully expanded, as well as some information for each binding.

The α-conversion pass later takes that output and uniquifies all the symbols.

```scheme
(letrec-syntax
    ((or (syntax-rules ()
           ((_) #f)
           ((_ e) e)
           ((_ e1 e2 ...)
            ((lambda (temp)
               (if temp
                   temp
                   (or e2 ...)))
             e1)))))
  ((lambda (temp) (or #f temp)) #t))

; Expansion
((lambda (temp:9)
   ((lambda (temp:10)
      (if temp:10 temp:10 temp:9))
    #f))
 #t)
```

### Quasiquotation

Quasiquotation is expanded with the algorithm taken from Bawden’s [*Quasiquotation in Lisp*](docs/references/Quasiquotation%20in%20Lisp.pdf). It can handle arbitrary levels of nested quasiquotation, and desugars all `quasiquote`, `unquote`, and `unquote-splicing` into combinations of `quote`, `list`, and `append` calls.

```scheme
(lambda (x ys) `(,x ,@ys 1))

; Expansion
(lambda (x:8 ys:9)
  (append (list x:8)
          (append (append ys:9)
                  (append (quote (1))
                          (quote ())))))
```

### A-normal form & reductions

The A-normalization algorithm is similarly yoinked from [*The Essence of Compiling with Continuations*](docs/references/The%20Essence%20of%20Compiling%20with%20Continuations.pdf) by Flanagan et al. It reduces the core scheme language, which by now is just 7 forms, into A-normal form, where all non-trivial expressions (function call, mutation, etc) are `let`-bound.

This turns a lot of optimizations into simple λ-calculus reductions, which the compiler applies over and over again.

```scheme
(let ((c 1))
  (let ((f (lambda (x) (+ x c))))
    (let ((g (lambda (y) (f y))))
      (g 42))))

; A-normalization
(let ((anf:17
       (λ (c:8)
         (let ((anf:15
                (λ (f:9)
                  (let ((anf:13
                         (λ (g:10) (g:10 42))))
                    (let ((anf:14
                           (λ (y:11) (f:9 y:11))))
                      (anf:13 anf:14))))))
           (let ((anf:16
                  (λ (x:12) (+:free x:12 c:8))))
             (anf:15 anf:16))))))
  (anf:17 1))

; First optimization pass:
; βη-reduction + DCE
(let ((c:8 1))
  (let ((anf:16
         (λ (x:12) (+:free x:12 c:8))))
    (let ((f:9 anf:16))
      (let ((anf:14
             (λ (y:11) (f:9 y:11))))
        (let ((g:10 anf:14))
          (g:10 42))))))

; Copy + Const propagation
(let ((c:8 1))
  (let ((anf:16
         (λ (x:12) (+:free x:12 1))))
    (let ((f:9 anf:16))
      (let ((anf:14
             (λ (y:11) (anf:16 y:11))))
        (let ((g:10 anf:16))
          (anf:16 42))))))

; DCE
(let ((anf:16
       (λ (x:12) (+:free x:12 1))))
  (anf:16 42))

; Second optimization pass:
; βη-reduction + DCE + Copy + Const propagation + DCE
(+:free 42 1)
```

## AI use

For this project I intentionally try to keep AI / coding assistant usage minimal. However, Claude Code has been very useful when I get stuck, since I don’t have anyone to talk to about esoteric programming language papers from the 90s.

Here are the parts AI helped with:

- Partial expansion algorithm for `lambda` / syntax binding bodies
- `MatchedSExprs` trick for tracking cardinality during `...` pattern/template expansion
- Code and documentation reviews
- Code refactoring
