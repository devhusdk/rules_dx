//! Re-pins a Maven lock of the nested consumers and writes it over the committed file.
//!
//! With `DX_HERMETIC_MANIFEST` it pins in the materialized consumer of that manifest; without it
//! it pins the lock every nested consumer shares. `DX_MAVEN_LOCK` is the workspace path of the lock.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, Stdio};
use std::time::{Duration, Instant};

const PIN_DEADLINE: Duration = Duration::from_secs(1800);

fn tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(200)..].join("\n")
}

fn consumer(committed: &str) -> Result<dx_nested::Case, String> {
    let Ok(rel) = std::env::var("DX_HERMETIC_MANIFEST") else {
        let case = dx_nested::Case::consumer("consumer_maven_pin", dx_nested::CONSUMER_MAVEN);
        case.write("BUILD.bazel", "");
        case.write("maven_install.json", committed);
        return Ok(case);
    };
    let text = dx_testing::read_text(&dx_testing::resolve_runfiles(&rel));
    let base = std::env::current_dir().map_err(|error| format!("no runfiles cwd: {error}"))?;
    let manifest = hermetic::parse_manifest(&text, &base)?;
    let source = hermetic::source_tree(&manifest)?;
    let case = dx_nested::case("cpp_rust_pin");
    hermetic::materialize(&manifest, case.root(), &source, "cc")?;
    Ok(case)
}

fn pin(case: &dx_nested::Case) -> Result<String, String> {
    let log_path = case.root().with_extension("pin.log");
    let log = File::create(&log_path).map_err(|error| format!("no pin log: {error}"))?;
    let mut command = case.bazel_command(&["run", "--show_progress_rate_limit=5", "@maven//:pin"]);
    command
        .env("REPIN", "1")
        .env("RJE_VERBOSE", "1")
        .stdout(Stdio::from(
            log.try_clone()
                .map_err(|error| format!("no pin log: {error}"))?,
        ))
        .stderr(Stdio::from(log));
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start the pin: {error}"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot wait for the pin: {error}"))?
        {
            break status;
        }
        if started.elapsed() > PIN_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "pin exceeded {}s; last output:\n{}",
                PIN_DEADLINE.as_secs(),
                tail(&log_path)
            ));
        }
        std::thread::sleep(Duration::from_secs(5));
    };
    if !status.success() {
        return Err(tail(&log_path));
    }
    Ok(dx_testing::read_text(
        &case.root().join("maven_install.json"),
    ))
}

fn run() -> Result<String, String> {
    let workspace = std::env::var("BUILD_WORKSPACE_DIRECTORY")
        .map_err(|_| "run this tool with `bazel run`".to_owned())?;
    let rel = std::env::var("DX_MAVEN_LOCK").map_err(|_| "DX_MAVEN_LOCK is not set".to_owned())?;
    let lock = PathBuf::from(workspace).join(&rel);
    let committed = std::fs::read_to_string(&lock)
        .map_err(|error| format!("cannot read {}: {error}", lock.display()))?;
    let pinned = pin(&consumer(&committed)?)?;
    if !hermetic::lock_differs(&committed, &pinned) {
        return Ok(format!("{rel} is up to date"));
    }
    std::fs::write(&lock, &pinned)
        .map_err(|error| format!("cannot write {}: {error}", lock.display()))?;
    Ok(format!("{rel} updated"))
}

fn main() -> ExitCode {
    let result = run();
    dx_nested::release();
    match result {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
