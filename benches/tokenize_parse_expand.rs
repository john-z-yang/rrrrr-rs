use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use rrrrr_rs::Session;

const BENCH_SRC: &str = r#"
(begin
  ; Top-level macro expansion.
  (define-syntax when
    (syntax-rules ()
      ((_ test body ...)
       (if test (begin body ...) 'skipped))))

  ; Function-style define exercises define -> lambda lowering.
  (define (driver seed . rest)
    (begin
      ; A leading begin in a body forces body normalization.
      (define flag
        (letrec-syntax
          ((my-or
             (syntax-rules ()
               ((_ ) #f)
               ((_ x) x)
               ((_ x y ...)
                (if x x (my-or y ...))))))
          (my-or #f seed #f)))

      (define seed-info `(seed ,seed ,@rest))

      (define (helper x . more)
        (if x
            `(ok ,x ,@more #(1 #t "hi" #\space))
            '(empty . list)))

      (let-syntax
        ((select
           (syntax-rules (else)
             ((_ else then fallback) fallback)
             ((_ test then fallback) (if test then fallback)))))
        (begin
          ; Inner begin + define exercises let-syntax body normalization too.
          (define payload (cons seed-info (helper seed 1 "hi" #\newline)))
          (when flag
            (set! payload
              (cons (select flag
                            `(tag . ,seed)
                            (select else 'unused 'fallback))
                    payload)))
          (if flag
              (set! payload (cons 'hot payload)))
          payload))))

  ; Non-identifier application exercises generic function application expansion.
  ((lambda args
     (if #t
         (driver 'go "bench" 42 #\space)
         args))
   'ignored)
  (letrec-syntax
  ((ping (syntax-rules ()
          ;; Base case: empty list
          ((_ ()) 'leaf)
          ;; Recursive step: strip one token, branch into two 'pong's
          ((_ (x . rest))
              (cons (pong rest) (pong rest)))))

  (pong (syntax-rules ()
          ;; Base case: empty list
          ((_ ()) 'leaf)
          ;; Recursive step: strip one token, branch into two 'ping's
          ((_ (x . rest))
              (cons (ping rest) (ping rest))))))

  ;; The length of this list determines the expansion depth (N).
  ;; An N of 20 will generate an Abstract Syntax Tree (AST) with 2^20 leaves.
  (ping (* * * * * * * * * * * * * * * *))))"#;

fn bench_config() -> Criterion {
    Criterion::default().measurement_time(Duration::from_mins(1))
}

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("tokenize_parse_expand_broad", |b| {
        b.iter(|| {
            let mut session = Session::new();
            let tokens = session.tokenize(BENCH_SRC).unwrap();
            let parsed = session.parse(&tokens).unwrap();
            let introduced = session.introduce(parsed);
            session.expand(&introduced).unwrap()
        })
    });
}

criterion_group! {
    name = benches;
    config = bench_config();
    targets = criterion_benchmark
}
criterion_main!(benches);
