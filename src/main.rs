#![allow(dead_code)]
mod compile;

use crate::compile::syntax::Symbol;
use compile::syntax::SExpr;

fn main() {
    let lambda_expr = sexpr!(
        'lambda,
        ('x,'y),
        ('foo, 'x, 'y)
    );

    println!("created expression:\n  {}", lambda_expr);

    match_sexpr! {
        lambda_expr,
        ('lambda, (..args), ..body) => {
            println!("matched p1, args {}, body {}", args, body);
        };
    };
}
