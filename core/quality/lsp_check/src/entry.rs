use crate::{Options, check, exception_headers};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const USAGE: &str = "usage: lsp_check [--language-id ID] [--quiet-ms N] [--timeout-ms N] \
-- SERVER_COMMAND... -- FILE...";

fn parse(args: &[String]) -> Result<(Options, Vec<String>, Vec<PathBuf>), String> {
    let mut options = Options {
        quiet: Duration::from_millis(1500),
        timeout: Duration::from_secs(45),
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
                );
            }
            "--timeout-ms" => {
                options.timeout = Duration::from_millis(
                    value("--timeout-ms")?
                        .parse()
                        .map_err(|_| "bad --timeout-ms")?,
                );
            }
            "--" => break,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let rest: Vec<String> = rest.cloned().collect();
    let Some(split) = rest.iter().position(|arg| arg == "--") else {
        return Err("missing second --".to_string());
    };
    let (server, tail) = rest.split_at(split);
    let server = server.to_vec();
    let files = tail.iter().skip(1).map(PathBuf::from).collect();
    if server.is_empty() {
        return Err("missing server command".to_string());
    }
    Ok((options, server, files))
}

fn wait_briefly(child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    for _ in 0..40 {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn run(args: &[String]) -> Result<bool, String> {
    let (options, server, files) = parse(args)?;
    let absolute: Vec<PathBuf> = files
        .iter()
        .map(|file| std::path::absolute(file).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    // Streams LSP over piped stdio for the whole session, so it is not a capture launch.
    let Some((program, program_args)) = server.split_first() else {
        return Err("missing server command".to_string());
    };
    let mut child = Command::new(program)
        .args(program_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start {program}: {error}"))?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        return Err("server pipes unavailable".to_string());
    };
    let server_stderr = Arc::new(Mutex::new(String::new()));
    let collected = Arc::clone(&server_stderr);
    let drain = std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut line = Vec::new();
        while reader.read_until(b'\n', &mut line).unwrap_or(0) > 0 {
            drop(std::io::stderr().write_all(&line));
            if let Ok(mut text) = collected.lock() {
                text.push_str(&String::from_utf8_lossy(&line));
            }
            line.clear();
        }
    });
    let result = check(stdout, stdin, &absolute, &options);
    let exit = result.is_err().then(|| wait_briefly(&mut child));
    drop(child.kill());
    drop(child.wait());
    drop(drain.join());
    let headers = server_stderr
        .lock()
        .map(|text| exception_headers(&text, 8))
        .unwrap_or_default();
    let found = result.map_err(|error| {
        let mut message = match exit.flatten() {
            Some(status) => format!("{error} (server exit: {status})"),
            None => format!("{error} (server still running)"),
        };
        for header in headers {
            message.push_str("\nserver exception: ");
            message.push_str(&header);
        }
        message
    })?;
    for (diagnostic, original) in found.iter().map(|d| {
        let original = absolute
            .iter()
            .position(|file| *file == d.file)
            .and_then(|i| files.get(i).cloned())
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
/// Runs `lsp_check` on the given arguments and returns the exit code.
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    match run(args) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) => {
            eprintln!("lsp_check: {error}\n{USAGE}");
            2
        }
    }
}
// LCOV_EXCL_STOP - reason: end thin shim, issue: 6553, policy: docs/cli/commands/build-test-coverage.md
