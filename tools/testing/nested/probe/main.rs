#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! The test stand-in for host tools: records Bazel calls, prints its environment, sleeps, emits or exits.

use std::io::Write;
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
        Some("sleep") => {
            std::thread::sleep(std::time::Duration::from_secs(
                args.get(1).and_then(|secs| secs.parse().ok()).unwrap_or(60),
            ));
            Ok(ExitCode::SUCCESS)
        }
        Some("emit") => {
            eprintln!("{}", args.get(1).map_or("", String::as_str));
            Ok(code(args.get(2)))
        }
        Some("heartbeat") => {
            let file = args.get(1).ok_or("heartbeat file")?;
            loop {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(file)
                    .and_then(|mut out| out.write_all(b"x\n"))
                    .map_err(|error| format!("heartbeat: {error}"))?;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
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
