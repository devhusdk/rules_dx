use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(rel) = std::env::args().nth(1) else {
        eprintln!("usage: suite_membership_check PROBLEMS_FILE");
        return ExitCode::FAILURE;
    };
    let text = dx_testing::path_text(&dx_testing::resolve_runfiles(&rel));
    let problems = match std::fs::read_to_string(&text) {
        Ok(problems) => problems,
        Err(e) => {
            eprintln!("read {text}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if problems.lines().any(|line| !line.trim().is_empty()) {
        eprint!("{problems}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
