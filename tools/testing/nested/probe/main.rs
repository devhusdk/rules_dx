#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::io::{Read as _, Write};
use std::process::ExitCode;

fn code(arg: Option<&String>) -> ExitCode {
    ExitCode::from(arg.and_then(|code| code.parse::<u8>().ok()).unwrap_or(0))
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let me = std::env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    if me.file_stem().is_some_and(|stem| stem == "real-bazel") {
        let log = me.with_extension("log");
        let cwd = std::env::current_dir().map_err(|error| format!("cwd: {error}"))?;
        let line = format!(
            "{}|{}|{}\n",
            cwd.display(),
            std::env::var("TEST_TMPDIR").unwrap_or_else(|_| "unset".to_owned()),
            args.join(" ")
        );
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .map_err(|error| format!("record call: {error}"))?;
        return Ok(ExitCode::SUCCESS);
    }
    match args.first().map(String::as_str) {
        None => {
            for (key, value) in std::env::vars() {
                println!("{key}={value}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("hold") => {
            std::io::stdin()
                .read_to_end(&mut Vec::new())
                .map_err(|error| format!("hold: {error}"))?;
            Ok(ExitCode::SUCCESS)
        }
        Some("emit") => {
            eprintln!("{}", args.get(1).map_or("", String::as_str));
            Ok(code(args.get(2)))
        }
        Some("echo") => {
            let mut out = std::io::stdout().lock();
            for line in std::io::stdin().lines() {
                let line = line.map_err(|error| format!("echo: {error}"))?;
                writeln!(out, "{line}")
                    .and_then(|()| out.flush())
                    .map_err(|error| format!("echo: {error}"))?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Some("exit") => Ok(code(args.get(1))),
        Some(other) => {
            eprintln!("nested probe: unknown mode {other}");
            Ok(ExitCode::from(2))
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args).unwrap_or_else(|error| {
        eprintln!("nested probe: {error}");
        ExitCode::FAILURE
    })
}
