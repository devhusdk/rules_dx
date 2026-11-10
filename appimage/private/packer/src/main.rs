use std::process::ExitCode;

fn main() -> ExitCode {
    let invoked = std::env::args().next().unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(appimage_packer::run(&invoked, &args))
}
