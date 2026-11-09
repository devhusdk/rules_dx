//! `coverage_cells_bin <cells.txt> <sha> <events.ndjson>...` exits 1 listing every failure.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cells, sha, inputs @ ..] = args.as_slice() else {
        eprintln!("usage: coverage_cells_bin <cells.txt> <sha> <events.ndjson>...");
        return ExitCode::from(2);
    };
    let read = |path: &String| std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"));
    let mut events = String::new();
    for path in inputs {
        match read(path) {
            Ok(text) => {
                events.push_str(&text);
                events.push('\n');
            }
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(1);
            }
        }
    }
    let expected = match read(cells) {
        Ok(text) => coverage_cells::expected_cells(&text),
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(1);
        }
    };
    let errors = coverage_cells::aggregate(&expected, sha, &events);
    for error in &errors {
        eprintln!("{error}");
    }
    if errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
