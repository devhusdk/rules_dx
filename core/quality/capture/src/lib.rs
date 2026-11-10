#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Runs one program with its streams redirected to files and its exit code recorded.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(test)]
mod tests;

pub const USAGE: &str = "usage: capture [--stdout FILE] [--stderr FILE] [--status FILE] [--ignore-exit] [--expand-bazel] -- PROGRAM [ARG]...\n       capture --fail MESSAGE";

const EXECUTION_ROOT: &str = "__BAZEL_EXECUTION_ROOT__";
const XCODE_PREFIX: &str = "__BAZEL_XCODE_";
const XCODE_DEVELOPER_DIR: &str = "__BAZEL_XCODE_DEVELOPER_DIR__";
const XCODE_SDKROOT: &str = "__BAZEL_XCODE_SDKROOT__";

#[derive(Debug, Default, PartialEq)]
pub struct Request {
    pub stdout: Option<PathBuf>,
    pub stderr: Option<PathBuf>,
    pub status: Option<PathBuf>,
    pub ignore_exit: bool,
    pub expand_bazel: bool,
    pub fail: Option<String>,
    pub command: Vec<String>,
}

/// The values the Bazel placeholders expand to.
pub struct Placeholders<'a> {
    pub execution_root: &'a str,
    pub developer_dir: Option<&'a str>,
    pub sdkroot: Option<&'a str>,
}

fn value(flag: &str, args: &mut impl Iterator<Item = String>) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

pub fn parse(args: Vec<String>) -> Result<Request, String> {
    let mut request = Request::default();
    let mut args = args.into_iter();
    let mut separated = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--stdout" => request.stdout = Some(PathBuf::from(value(&arg, &mut args)?)),
            "--stderr" => request.stderr = Some(PathBuf::from(value(&arg, &mut args)?)),
            "--status" => request.status = Some(PathBuf::from(value(&arg, &mut args)?)),
            "--fail" => request.fail = Some(value(&arg, &mut args)?),
            "--ignore-exit" => request.ignore_exit = true,
            "--expand-bazel" => request.expand_bazel = true,
            "--" => {
                separated = true;
                break;
            }
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    request.command = args.collect();
    if request.fail.is_some() {
        if separated {
            return Err("--fail takes no program".to_owned());
        }
        return Ok(request);
    }
    if request.command.is_empty() {
        return Err("no program after --".to_owned());
    }
    Ok(request)
}

/// Replaces the Bazel execution root and Xcode placeholders in `args`.
pub fn expand_bazel(args: &[String], values: &Placeholders) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        let mut arg = arg.replace(EXECUTION_ROOT, values.execution_root);
        if arg.contains(XCODE_PREFIX) {
            let (Some(developer_dir), Some(sdkroot)) = (values.developer_dir, values.sdkroot)
            else {
                return Err(format!(
                    "cannot expand an Xcode placeholder in '{arg}': DEVELOPER_DIR and SDKROOT must both be set"
                ));
            };
            arg = arg
                .replace(XCODE_DEVELOPER_DIR, developer_dir)
                .replace(XCODE_SDKROOT, sdkroot);
        }
        out.push(arg);
    }
    Ok(out)
}

fn create(path: &Path) -> Result<File, String> {
    File::create(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn clone(file: &File, path: &Path) -> Result<File, String> {
    file.try_clone()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn program(name: &str) -> String {
    if cfg!(windows) {
        name.replace('/', "\\")
    } else {
        name.to_owned()
    }
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

fn spawn(request: &Request, command: &[String]) -> Result<i32, String> {
    let Some((name, rest)) = command.split_first() else {
        return Err("no program after --".to_owned());
    };
    let mut child = Command::new(program(name));
    child.args(rest);
    let mut stderr_log = None;
    if let Some(path) = &request.stderr {
        let file = create(path)?;
        stderr_log = Some(clone(&file, path)?);
        if request.stdout.as_ref() == Some(path) {
            child.stdout(Stdio::from(clone(&file, path)?));
        }
        child.stderr(Stdio::from(file));
    }
    if let Some(path) = request
        .stdout
        .as_ref()
        .filter(|path| Some(*path) != request.stderr.as_ref())
    {
        child.stdout(Stdio::from(create(path)?));
    }
    match child.status() {
        Ok(status) => Ok(exit_code(status)),
        Err(error) => {
            use std::io::Write;
            let message = format!("capture: {name}: {error}\n");
            match stderr_log.as_mut() {
                Some(log) => log
                    .write_all(message.as_bytes())
                    .map_err(|error| error.to_string())?,
                None => eprint!("{message}"),
            }
            Ok(127)
        }
    }
}

/// Runs `request` and returns the exit code of this process.
pub fn run(request: &Request, values: &Placeholders) -> Result<i32, String> {
    if let Some(message) = &request.fail {
        eprintln!("{message}");
        return Ok(1);
    }
    let command = if request.expand_bazel {
        expand_bazel(&request.command, values)?
    } else {
        request.command.clone()
    };
    let code = spawn(request, &command)?;
    if let Some(path) = &request.status {
        std::fs::write(path, format!("{code}\n"))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        return Ok(0);
    }
    Ok(if request.ignore_exit { 0 } else { code })
}
