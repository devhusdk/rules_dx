//! Runs rust-lld as lld-link with execroot-relative paths, so the PDB does not depend on the checkout.

use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use msvc_paths::{Rewriter, edit_args, exit_code, link_copy_dir, rustc_temp_file, split_option};

fn main() -> ExitCode {
    run().unwrap_or_else(|err| {
        eprintln!("msvc_link: {err}");
        ExitCode::FAILURE
    })
}

fn run() -> io::Result<ExitCode> {
    let rewriter = Rewriter::from_cwd()?;
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "-flavor") {
        args.drain(..args.len().min(2));
    }

    let mut out = None;
    edit_args(&mut args, |arg| {
        let arg = rewriter.rewrite(arg);
        let (option, value) = split_option(&arg);
        if option.eq_ignore_ascii_case("/out:") || option.eq_ignore_ascii_case("-out:") {
            out = Some(value.to_string());
        }
        Ok(arg)
    })?;
    let copy_dir = link_copy_dir(&out.ok_or_else(|| io::Error::other("no /OUT: argument"))?);
    let copy = |path: &str, name: &str| -> io::Result<String> {
        fs::create_dir_all(&copy_dir)?;
        let target = format!("{copy_dir}/{name}");
        fs::copy(path, &target)?;
        Ok(target)
    };
    edit_args(&mut args, |arg| {
        let (option, value) = split_option(arg);
        match rustc_temp_file(value) {
            Some(name) => Ok(format!("{option}{}", copy(value, name)?)),
            None => Ok(arg.to_string()),
        }
    })?;
    for arg in args.iter_mut() {
        if let Some(path) = arg.strip_prefix('@') {
            let path = rewriter.rewrite(path);
            *arg = match rustc_temp_file(&path) {
                Some(name) => format!("@{}", copy(&path, name)?),
                None => format!("@{path}"),
            };
        }
    }

    let status = Command::new(rust_lld(&rewriter)?)
        .args(["-flavor", "link"])
        .args(&args)
        .status();
    let _ = fs::remove_dir_all(&copy_dir);
    Ok(exit_code(status?))
}

/// Finds rust-lld on the PATH that rustc sets for its linker.
fn rust_lld(rewriter: &Rewriter) -> io::Result<String> {
    let name = if cfg!(windows) {
        "rust-lld.exe"
    } else {
        "rust-lld"
    };
    let path = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|lld| lld.is_file())
        .map(|lld| rewriter.rewrite(&lld.to_string_lossy()))
        .filter(|lld| Path::new(lld).is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{name} is not on PATH")))
}
