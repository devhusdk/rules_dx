//! Native and WASI entry point over the shared task board.

use std::process::ExitCode;

use shared_tasks::{asset_report, Board, CORE_VERSION};

fn read(path: &str) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|error| {
        eprintln!("err:unreadable: {path}: {error}");
        ExitCode::from(2)
    })
}

fn run(args: &[String]) -> Result<(), ExitCode> {
    let mut assets = Vec::new();
    let mut rest = Vec::new();
    for arg in args {
        match arg.strip_prefix("--asset=") {
            Some(path) => assets.push(path),
            None => rest.push(arg.as_str()),
        }
    }
    let Some((board_path, commands)) = rest.split_first() else {
        eprintln!("usage: tasks [--asset=PATH]... BOARD [COMMAND]...");
        return Err(ExitCode::from(2));
    };
    let text = read(board_path)?;
    let text = String::from_utf8(text).map_err(|_| {
        eprintln!("err:not_utf8: {board_path}");
        ExitCode::from(2)
    })?;
    let mut board = Board::load(&text).map_err(|error| {
        eprintln!("err:{}: {error}", error.kind());
        ExitCode::from(2)
    })?;
    for command in commands {
        match board.apply(command) {
            Ok(event) => println!("ok:{event}"),
            Err(error) => {
                eprintln!("err:{}: {error}", error.kind());
                return Err(ExitCode::from(1));
            }
        }
    }
    for line in board.render() {
        println!("{line}");
    }
    println!("summary {}", board.summary());
    for path in assets {
        let bytes = read(path)?;
        let name = path.rsplit('/').next().unwrap_or(path);
        println!("{}", asset_report(name, &bytes));
    }
    let build = if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    };
    eprintln!("core:{CORE_VERSION} build={build}");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}
