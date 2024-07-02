#![allow(dead_code)]
mod compile;

fn main() {
    let lambda_expr = sexpr!(S(lambda), (S(x), S(y)), (S(foo), S(x), S(y)));

    println!("created expression:\n  {}", lambda_expr);

    match_sexpr! {(#"lambda", (args @ ..), body @ ..) = lambda_expr => {
        println!("matched p1, args {}, body {}", args, body);
    }};
}
