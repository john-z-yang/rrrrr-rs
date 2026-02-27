use std::collections::HashMap;

use compile::bindings::Bindings;
use compile::expand::{expand, introduce};
use compile::parse::parse;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

use compile::lex::tokenize;
mod compile;

fn main() {
    let mut bindings = Bindings::new();
    let mut env = HashMap::new();

    let mut rl = DefaultEditor::new().expect("Unable to open interactive terminal");
    let _ = rl.load_history("history.txt");
    let mut lines = String::new();
    loop {
        let readline = rl.readline(if lines.is_empty() { "lisp> " } else { "  ... " });
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                if line.is_empty() {
                    let expanded = tokenize(&lines)
                        .and_then(|tokens| parse(&tokens))
                        .and_then(|sexpr| expand(&introduce(&sexpr), &mut bindings, &mut env));
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
