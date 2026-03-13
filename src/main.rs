use rrrrr_rs::Session;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

fn main() {
    let mut session = Session::new();

    let mut rl = DefaultEditor::new().expect("Unable to open interactive terminal");
    let _ = rl.load_history("history.txt");
    let mut lines = String::new();
    loop {
        let readline = rl.readline(if lines.is_empty() { "lisp> " } else { "  ... " });
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                if line.is_empty() {
                    let expanded = session
                        .tokenize(&lines)
                        .and_then(|tokens| session.parse(&tokens))
                        .and_then(|sexpr| session.expand(&session.introduce(sexpr)));
                    match expanded {
                        Ok(expanded) => println!("{}", expanded),
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
