#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use lsp_check::{Options, check};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

const USAGE: &str = "usage: lsp_check [--language-id ID] [--quiet-ms N] [--timeout-ms N] \
-- SERVER_COMMAND... -- FILE...";

fn parse(args: &[String]) -> Result<(Options, Vec<String>, Vec<PathBuf>), String> {
    let mut options = Options {
        quiet: Duration::from_millis(1500),
        timeout: Duration::from_secs(120),
        language_id: String::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |name: &str| {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--language-id" => options.language_id = value("--language-id")?,
            "--quiet-ms" => {
                options.quiet = Duration::from_millis(
                    value("--quiet-ms")?.parse().map_err(|_| "bad --quiet-ms")?,
                )
            }
            "--timeout-ms" => {
                options.timeout = Duration::from_millis(
                    value("--timeout-ms")?
                        .parse()
                        .map_err(|_| "bad --timeout-ms")?,
                )
            }
            "--" => break,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let rest: Vec<String> = rest.cloned().collect();
    let Some(split) = rest.iter().position(|arg| arg == "--") else {
        return Err("missing second --".to_string());
    };
    let server = rest[..split].to_vec();
    let files = rest[split + 1..].iter().map(PathBuf::from).collect();
    if server.is_empty() {
        return Err("missing server command".to_string());
    }
    Ok((options, server, files))
}

fn wait_briefly(child: &mut std::process::Child) -> Option<Option<std::process::ExitStatus>> {
    for _ in 0..40 {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(Some(status));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Some(None)
}

fn run(args: &[String]) -> Result<bool, String> {
    let (options, server, files) = parse(args)?;
    let absolute: Vec<PathBuf> = files
        .iter()
        .map(|file| std::path::absolute(file).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let mut child = Command::new(&server[0])
        .args(&server[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("start {}: {error}", server[0]))?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("server pipes unavailable".to_string());
    };
    let result = check(stdout, stdin, &absolute, &options);
    let exit = if result.is_err() {
        wait_briefly(&mut child)
    } else {
        None
    };
    let _ = child.kill();
    let _ = child.wait();
    let found = result.map_err(|error| match exit {
        Some(Some(status)) => format!("{error} (server exit: {status})"),
        Some(None) => format!("{error} (server still running)"),
        None => error.to_string(),
    })?;
    for (diagnostic, original) in found.iter().map(|d| {
        let original = absolute
            .iter()
            .position(|file| *file == d.file)
            .map(|i| files[i].clone())
            .unwrap_or_else(|| d.file.clone());
        (d, original)
    }) {
        let mut diagnostic = diagnostic.clone();
        diagnostic.file = original;
        println!("{}", diagnostic.to_json());
    }
    Ok(found.is_empty())
}

// LCOV_EXCL_START - reason: thin shim, issue: 6553, policy: docs/cli/commands/build-test-coverage.md
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("lsp_check: {error}\n{USAGE}");
            std::process::exit(2);
        }
    }
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 6553, policy: docs/cli/commands/build-test-coverage.md
