//! Fails when the suite membership problems file named by the first argument lists a problem.

fn main() {
    let rel = std::env::args().nth(1).expect("problems file argument");
    let text = dx_testing::path_text(&dx_testing::resolve_runfiles(&rel));
    let problems = std::fs::read_to_string(&text).unwrap_or_else(|e| panic!("read {text}: {e}"));
    if problems.lines().any(|line| !line.trim().is_empty()) {
        eprint!("{problems}");
        std::process::exit(1);
    }
}
