mod compile;

use compile::syntax::SExpr;

fn main() {
    let def = SExpr::new_symbol("define");
    let lam = SExpr::new_symbol("lambda");
    let disp = SExpr::new_symbol("display");
    let plus = SExpr::new_symbol("+");
    let x = SExpr::new_symbol("x");
    let y = SExpr::new_symbol("y");
    let one = SExpr::new_num(1);
    let fn_name = SExpr::new_symbol("fn");
    let true_ = SExpr::new_bool(true);

    let lambda_expr = sexpr!(
        lam.clone(),
        (x.clone(), y.clone()),
        (plus.clone(), x.clone(), y.clone()),
        (plus.clone(), x.clone(), y.clone(),),
        (
            disp.clone(),
            (plus.clone(), one.clone(), x.clone()),
            true_.clone()
        )
    );

    let fn_defn = sexpr!(def.clone(), fn_name.clone(), lambda_expr.clone());

    println!("created expression:\n  {}", fn_defn);

    match_sexpr! {
        fn_defn,
        ('define, SExpr::Symbol(var_name), expr) => {
            println!("pattern 1:");
            println!("  setting symbol `{}` to be {}", var_name, expr);
        };
        ('define, SExpr::Symbol(var_name), (SExpr::Symbol(_), (first, SExpr::Symbol(second)), ..body)) => {
            println!("pattern 2:");
            println!("  assigning symbol {} to a function", var_name);
            println!("  function has args `{}` and `{}` with body {}", first, second, body);
        };
        ('define, var_name, ('lambda, (SExpr::Symbol(first), second), _, _, last)) => {
            println!("pattern 3:");
            println!("  assigning symbol {} to a function", var_name);
            println!("  function has args `{}` and `{}` with last line in body {}", first, second, last);
        };
        ('define, _var_name, ('lambda, (SExpr::Num(_num), _second), .._body)) => {
            println!("pattern 4:");
            panic!("    this should not be matched because the parameters should be all symbols");
        };
    }
}
