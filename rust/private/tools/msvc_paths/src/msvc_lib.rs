//! Runs $TARGET_AR from the execroot with execroot-relative paths, so archive member names do not
//! depend on the checkout. Build scripts call it from deep inside bazel-out, where `../` prefixes
//! would push paths past MAX_PATH, so the archiver always runs from the execroot itself.

use std::env;
use std::io;
use std::path::{self, Path};
use std::process::{Command, ExitCode};

use msvc_paths::{Rewriter, edit_args, execroot, exit_code};

fn main() -> ExitCode {
    run().unwrap_or_else(|err| {
        eprintln!("msvc_lib: {err}");
        ExitCode::FAILURE
    })
}

fn run() -> io::Result<ExitCode> {
    let root = execroot(&env::current_dir()?)?;
    let rewriter = Rewriter::new(&root.to_string_lossy(), "");
    let ar = env::var_os("TARGET_AR").ok_or_else(|| io::Error::other("TARGET_AR is not set"))?;
    let ar = path::absolute(Path::new(&ar))?;
    let mut args: Vec<String> = env::args().skip(1).collect();
    edit_args(&mut args, |arg| Ok(rewriter.rewrite(arg)))?;
    for arg in args.iter_mut() {
        if let Some(file) = arg.strip_prefix('@') {
            let file = path::absolute(file)?;
            *arg = format!("@{}", rewriter.rewrite(&file.to_string_lossy()));
        }
    }
    let status = Command::new(ar).args(&args).current_dir(&root).status()?;
    Ok(exit_code(status))
}
