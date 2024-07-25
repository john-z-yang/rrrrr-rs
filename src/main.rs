#![allow(dead_code)]
extern crate rustyline;

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
                    println!("{:?}", tokenize(&lines));
                    lines.clear();
                } else {
                    lines.push_str(&line);
                    lines.push_str("\n");
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
