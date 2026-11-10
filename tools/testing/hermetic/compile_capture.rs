//! Creates the output directories, runs one command and writes its exit code and output to a log; exits 0 once the log is written.

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(log) = args.next() else {
        eprintln!("usage: compile_capture <log> [output dir...] -- <program> [args...]");
        return ExitCode::from(2);
    };
    let program = loop {
        match args.next() {
            Some(arg) if arg == "--" => break args.next(),
            Some(dir) => {
                if let Err(error) = std::fs::create_dir_all(&dir) {
                    eprintln!("cannot create {}: {error}", dir.display());
                    return ExitCode::from(2);
                }
            }
            None => break None,
        }
    };
    let Some(program) = program else {
        eprintln!("usage: compile_capture <log> [output dir...] -- <program> [args...]");
        return ExitCode::from(2);
    };
    let output = match Command::new(&program).args(args).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!("cannot run {}: {error}", program.display());
            return ExitCode::from(2);
        }
    };
    let Some(code) = output.status.code() else {
        eprintln!("{} ended without an exit code", program.display());
        return ExitCode::from(2);
    };
    let mut text = format!("exit: {code}\n").into_bytes();
    text.extend_from_slice(&output.stdout);
    text.extend_from_slice(&output.stderr);
    if let Err(error) = std::fs::write(&log, text) {
        eprintln!("cannot write {}: {error}", log.display());
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
