use rrrrr_rs::Session;
use rrrrr_rs::compile::compilation_error::CompilationError;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

fn main() {
    let mut session = Session::with_prelude();

    let mut rl = DefaultEditor::new().expect("Unable to open interactive terminal");
    let _ = rl.load_history("history.txt");
    let mut lines = String::new();
    loop {
        let readline = rl.readline(if lines.is_empty() { "lisp> " } else { "  ... " });
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                if line.is_empty() {
                    let res = session
                        .tokenize(&lines)
                        .and_then(|tokens| session.parse(&tokens))
                        .and_then(|sexprs| {
                            sexprs
                                .into_iter()
                                .map(|sexpr| {
                                    session
                                        .expand(session.introduce(sexpr))
                                        .map(|expanded| session.alpha_convert(expanded))
                                        .map(|converted| session.lower(converted))
                                        .map(|lowered| session.a_normalize(lowered))
                                        .and_then(|normalized| session.beta_contract(normalized))
                                        .map(|contracted| session.dce(contracted))
                                })
                                .collect::<Result<Vec<_>, CompilationError>>()
                        });
                    match res {
                        Ok(res) => res.into_iter().for_each(|res| println!("{}", res)),
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
