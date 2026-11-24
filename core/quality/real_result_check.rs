use std::process::ExitCode;

use dx_testing::{contains_bytes, resolve_runfiles};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args
        .iter()
        .position(|a| a.starts_with('+') || a.starts_with('-'))
        .unwrap_or(args.len());
    let (names, wants) = args.split_at(split);
    let mut files: Vec<(String, Vec<u8>)> = Vec::with_capacity(names.len());
    for rel in names {
        let path = resolve_runfiles(rel);
        match std::fs::read(&path) {
            Ok(bytes) => files.push((rel.clone(), bytes)),
            Err(e) => {
                eprintln!("read {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }
    for want in wants {
        let mut chars = want.chars();
        let sign = chars.next();
        let text = chars.as_str();
        let found = files.iter().any(|(_, bytes)| contains_bytes(bytes, text.as_bytes()));
        if sign == Some('+') && !found {
            println!("missing: {text}");
            for (name, bytes) in &files {
                println!("== {name}");
                let head = bytes.get(..bytes.len().min(4000)).unwrap_or(bytes);
                println!("{}", String::from_utf8_lossy(head));
            }
            return ExitCode::FAILURE;
        }
        if sign == Some('-') && found {
            println!("unexpected: {text}");
            for (name, bytes) in &files {
                println!("== {name}");
                let lines = String::from_utf8_lossy(bytes);
                for (n, line) in lines.lines().enumerate().filter(|(_, l)| l.contains(text)).take(20) {
                    println!("{}:{line}", n + 1);
                }
            }
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
