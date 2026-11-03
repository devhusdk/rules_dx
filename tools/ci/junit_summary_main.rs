//! `junit_summary_bin <reports-dir>` prints the failing test cases from `junit.xml` as Markdown.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(dir) = std::env::args_os().nth(1) else {
        eprintln!("dx-ci: pass the reports directory.");
        return ExitCode::FAILURE;
    };
    let path = std::path::Path::new(&dir).join("junit.xml");
    if let Ok(xml) = std::fs::read_to_string(path) {
        print!("{}", junit_summary::render(&xml));
    }
    ExitCode::SUCCESS
}
