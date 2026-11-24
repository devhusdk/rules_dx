use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arg0 = std::env::args_os().next().unwrap_or_default();
    let name = Path::new(&arg0)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    match writeln!(std::io::stdout(), "{name}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ignored: std::io::Result<()> =
                writeln!(std::io::stderr(), "deploy fixture: {error}");
            ExitCode::FAILURE
        }
    }
}
