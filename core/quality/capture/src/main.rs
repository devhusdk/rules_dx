use capture::{Placeholders, USAGE, parse, run};

fn text(value: std::ffi::OsString) -> Result<String, String> {
    value
        .into_string()
        .map_err(|raw| format!("not UTF-8: {}", raw.to_string_lossy()))
}

fn execute() -> Result<i32, String> {
    let args = std::env::args_os()
        .skip(1)
        .map(text)
        .collect::<Result<Vec<_>, _>>()?;
    let request = parse(args).map_err(|error| format!("{error}\n{USAGE}"))?;
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let cwd = text(cwd.into_os_string())?;
    let developer_dir = std::env::var("DEVELOPER_DIR")
        .ok()
        .filter(|v| !v.is_empty());
    let sdkroot = std::env::var("SDKROOT").ok().filter(|v| !v.is_empty());
    run(
        &request,
        &Placeholders {
            execution_root: &cwd,
            developer_dir: developer_dir.as_deref(),
            sdkroot: sdkroot.as_deref(),
        },
    )
}

fn main() {
    match execute() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("capture: {error}");
            std::process::exit(2);
        }
    }
}
