//! Runs $TARGET_AR with execroot-relative paths, so archive member names do not depend on the checkout.

use std::env;
use std::io;
use std::process::{Command, ExitCode};

use msvc_paths::{edit_args, exit_code, Rewriter};

fn main() -> ExitCode {
    run().unwrap_or_else(|err| {
        eprintln!("msvc_lib: {err}");
        ExitCode::FAILURE
    })
}

fn run() -> io::Result<ExitCode> {
    let rewriter = Rewriter::from_cwd()?;
    let ar = env::var_os("TARGET_AR").ok_or_else(|| io::Error::other("TARGET_AR is not set"))?;
    let mut args: Vec<String> = env::args().skip(1).collect();
    edit_args(&mut args, |arg| Ok(rewriter.rewrite(arg)))?;
    Ok(exit_code(Command::new(ar).args(&args).status()?))
}
