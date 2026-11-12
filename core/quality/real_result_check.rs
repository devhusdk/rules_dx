//! Checks result files for strings: `FILE... [+WANTED|-UNWANTED]...`.

use dx_testing::{contains_bytes, resolve_runfiles};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args
        .iter()
        .position(|a| a.starts_with('+') || a.starts_with('-'))
        .unwrap_or(args.len());
    let files: Vec<(String, Vec<u8>)> = args[..split]
        .iter()
        .map(|rel| {
            let path = resolve_runfiles(rel);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            (rel.clone(), bytes)
        })
        .collect();
    for want in &args[split..] {
        let (sign, text) = want.split_at(1);
        let found = files.iter().any(|(_, bytes)| contains_bytes(bytes, text.as_bytes()));
        if sign == "+" && !found {
            println!("missing: {text}");
            for (name, bytes) in &files {
                println!("== {name}");
                println!("{}", String::from_utf8_lossy(&bytes[..bytes.len().min(4000)]));
            }
            std::process::exit(1);
        }
        if sign == "-" && found {
            println!("unexpected: {text}");
            for (name, bytes) in &files {
                println!("== {name}");
                let lines = String::from_utf8_lossy(bytes);
                for (n, line) in lines.lines().enumerate().filter(|(_, l)| l.contains(text)).take(20) {
                    println!("{}:{line}", n + 1);
                }
            }
            std::process::exit(1);
        }
    }
}
