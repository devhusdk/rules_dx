//! `fake_tool EXIT STDERR [ARG]...` prints each ARG to stdout and STDERR to stderr, then exits with EXIT.

fn main() {
    let mut args = std::env::args().skip(1);
    let code = args.next().and_then(|code| code.parse().ok()).unwrap_or(2);
    let stderr = args.next().unwrap_or_default();
    for arg in args {
        println!("ARG:{arg}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    std::process::exit(code);
}
