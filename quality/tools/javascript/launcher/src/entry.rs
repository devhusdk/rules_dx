//! Prints the entry point a generated JavaScript launcher names.

use std::path::Path;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(script) = args.first() else {
        eprintln!("usage: js_entry_point <generated script>");
        return std::process::ExitCode::from(2);
    };
    match dx_js_launcher::entry_point(Path::new(script)) {
        Ok(entry) => {
            println!("{entry}");
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("js_entry_point: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
