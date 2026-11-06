//! Runs a command in a per-run AppContainer: no network unless `--net`, files only where granted.
mod args;

#[cfg(windows)]
mod sys;

#[cfg(windows)]
fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match args::parse(&argv).and_then(|config| sys::run(&config)) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("appcontainer_run: {err}");
            std::process::exit(125);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("appcontainer_run only runs on Windows");
    std::process::exit(125);
}
