//! Consumer-owned report adapter over the shared task board.

#[cfg(not(consumer_build))]
compile_error!("consumer rustc flags did not reach the adapter");

use std::process::ExitCode;

use shared_tasks::Board;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((path, commands)) = args.split_first() else {
        eprintln!("usage: consumer_report BOARD [COMMAND]...");
        return ExitCode::from(2);
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        eprintln!("err:unreadable: {path}");
        return ExitCode::from(2);
    };
    let mut board = match Board::load(&text) {
        Ok(board) => board,
        Err(error) => {
            eprintln!("err:{}", error.kind());
            return ExitCode::from(2);
        }
    };
    for command in commands {
        if let Err(error) = board.apply(command) {
            eprintln!("err:{}", error.kind());
            return ExitCode::from(1);
        }
    }
    #[cfg(feature = "csv")]
    {
        println!("id,done,title");
        for task in board.tasks() {
            println!("{},{},{}", task.id, task.done, task.title);
        }
    }
    println!("summary {}", board.summary());
    let runtime = std::env::var("CONSUMER_RUNTIME").unwrap_or_else(|_| "default".to_owned());
    println!("runtime={runtime}");
    ExitCode::SUCCESS
}
