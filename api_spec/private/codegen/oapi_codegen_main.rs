#![deny(clippy::indexing_slicing, clippy::string_slice)]
//! Runs oapi-codegen with the Go SDK as its only PATH entry and a scratch home beside the output.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The parsed command line.
#[derive(Debug, PartialEq)]
struct Args {
    go: PathBuf,
    tool: PathBuf,
    out: PathBuf,
    rest: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let [go, tool, out, rest @ ..] = args else {
        return Err("usage: oapi_codegen GO OAPI_CODEGEN OUT [ARG...]".to_owned());
    };
    Ok(Args {
        go: PathBuf::from(go),
        tool: PathBuf::from(tool),
        out: PathBuf::from(out),
        rest: rest.to_vec(),
    })
}

/// The scratch directory of one output file.
fn scratch_dir(cwd: &Path, out: &Path) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".scratch");
    cwd.join(name)
}

/// The absolute directory that holds `go`.
fn go_dir(cwd: &Path, go: &Path) -> Result<PathBuf, String> {
    cwd.join(go)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no parent directory", go.display()))
}

fn generate(args: &Args, cwd: &Path, scratch: &Path) -> Result<ExitCode, String> {
    let home = scratch.join("home");
    let cache = scratch.join("gocache");
    for dir in [&home, &cache] {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    let status = Command::new(cwd.join(&args.tool))
        .env("PATH", go_dir(cwd, &args.go)?)
        .env("HOME", &home)
        .env("GOCACHE", &cache)
        .arg("-o")
        .arg(&args.out)
        .args(&args.rest)
        .status()
        .map_err(|error| format!("{}: {error}", args.tool.display()))?;
    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let args = parse(args)?;
    let cwd = std::env::current_dir().map_err(|error| format!("current directory: {error}"))?;
    let scratch = scratch_dir(&cwd, &args.out);
    let result = generate(&args, &cwd, &scratch);
    let removed = match std::fs::remove_dir_all(&scratch) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{}: {error}", scratch.display()))
        }
        _ => Ok(()),
    };
    let code = result?;
    removed?;
    Ok(code)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("oapi_codegen: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn parse_takes_go_tool_out_and_the_generator_arguments() {
        let args = parse(&strings(&["sdk/bin/go", "tool", "pkg/out.go", "-package", "p"]))
            .expect("parse");
        assert_eq!(
            args,
            Args {
                go: PathBuf::from("sdk/bin/go"),
                tool: PathBuf::from("tool"),
                out: PathBuf::from("pkg/out.go"),
                rest: strings(&["-package", "p"]),
            }
        );
    }

    #[test]
    fn parse_rejects_a_short_command_line() {
        let error = parse(&strings(&["go", "tool"])).expect_err("usage");
        assert!(error.starts_with("usage:"), "{error}");
    }

    #[test]
    fn scratch_dir_sits_beside_the_output() {
        assert_eq!(
            scratch_dir(Path::new("/root"), Path::new("bin/pkg/out.go")),
            PathBuf::from("/root/bin/pkg/out.go.scratch")
        );
    }

    #[test]
    fn go_dir_is_absolute() {
        assert_eq!(
            go_dir(Path::new("/root"), Path::new("external/sdk/bin/go")).expect("dir"),
            PathBuf::from("/root/external/sdk/bin")
        );
    }
}
