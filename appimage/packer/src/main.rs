use std::process::ExitCode;

fn main() -> ExitCode {
    let invoked = std::env::args().next().unwrap_or_default();
    if std::path::Path::new(&invoked)
        .file_name()
        .is_some_and(|name| name == appimage_packer::FILE_COMMAND)
    {
        eprintln!(
            "dx_appimage: the file command is not provided; set ARCH and pass --no-appstream"
        );
        return ExitCode::from(2);
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    match appimage_packer::parse(&args).and_then(|request| appimage_packer::pack(&request)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("dx_appimage: {message}");
            ExitCode::FAILURE
        }
    }
}
