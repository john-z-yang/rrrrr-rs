extern crate rustyline;

use std::collections::HashMap;

use compile::bindings::Bindings;
use compile::expand::{expand, introduce};
use compile::parse::parse;
use compile::sexpr::Symbol;
use compile::transformer::Transformer;
use rustyline::error::ReadlineError;
use rustyline::Editor;

use compile::lex::tokenize;
mod compile;

fn main() {
    let mut rl = Editor::<()>::new();
    let _ = rl.load_history("history.txt");
    let mut lines = String::new();
    loop {
        let readline = rl.readline(if lines.is_empty() { "lisp> " } else { " ... " });
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                if line.is_empty() {
                    match tokenize(&lines) {
                        Ok(tokens) => match parse(&tokens) {
                            Ok(sexpr) => {
                                let mut bindings = Bindings::new();
                                let mut env = HashMap::<Symbol, Transformer>::new();
                                print!(
                                    "{}",
                                    expand(
                                        &introduce(&sexpr.coerce_to_syntax()),
                                        &mut bindings,
                                        &mut env,
                                    )
                                );
                            }
                            Err(err) => println!("{:?}", err),
                        },
                        Err(err) => println!("{:?}", err),
                    }
                    lines.clear();
                } else {
                    lines.push_str(&line);
                    lines.push('\n');
                }
            }
            Err(ReadlineError::Eof) => {
                println!("Farewell.");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history("history.txt").unwrap();
}
