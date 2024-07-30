#![allow(dead_code)]

extern crate rustyline;

use std::collections::HashMap;

use compile::bindings::Bindings;
use compile::expand::{expand, introduce};
use compile::parse::parse;
use rustyline::error::ReadlineError;
use rustyline::Editor;

use compile::lex::tokenize;
mod compile;

fn main() {
    let mut rl = Editor::<()>::new();
    let _ = rl.load_history("history.txt");
    let mut lines = String::new();
    loop {
        let readline = rl.readline(if lines.is_empty() { "lisp> " } else { "  ... " });
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                if line.is_empty() {
                    let expanded =
                        tokenize(&lines)
                            .and_then(|tokens| parse(&tokens))
                            .map(|sexpr| {
                                expand(
                                    &introduce(&sexpr),
                                    &mut Bindings::new(),
                                    &mut HashMap::new(),
                                )
                            });
                    match expanded {
                        Ok(sexpr) => println!("{}", sexpr),
                        Err(err) => err.pprint_with_source(&lines),
                    };
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
