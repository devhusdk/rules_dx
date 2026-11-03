use std::io::{Read, Write};

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut out = std::io::stdout().lock();
    let shim = std::env::var_os("DX_SHIM").unwrap_or_default();
    out.write_all(b"shim=").unwrap();
    out.write_all(&bytes(&shim)).unwrap();
    out.write_all(b"\n").unwrap();
    for arg in &args {
        out.write_all(b"arg=[").unwrap();
        out.write_all(&bytes(arg)).unwrap();
        out.write_all(b"]\n").unwrap();
    }
    out.flush().unwrap();
    match args.first().and_then(|a| a.to_str()) {
        Some("stdin") => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input).unwrap();
            out.write_all(&input).unwrap();
            out.flush().unwrap();
        }
        Some("exit") => {
            let code = args
                .get(1)
                .and_then(|a| a.to_str())
                .and_then(|a| a.parse().ok());
            std::process::exit(code.unwrap_or(0));
        }
        #[cfg(unix)]
        Some("kill") => {
            use std::os::unix::process::CommandExt;
            let err = std::process::Command::new("/bin/sh")
                .args(["-c", "kill -TERM $$"])
                .exec();
            panic!("exec: {err}");
        }
        _ => {}
    }
}

#[cfg(unix)]
fn bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    value.to_string_lossy().into_owned().into_bytes()
}
