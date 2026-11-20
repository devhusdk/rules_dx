//! The rules_apple xctestrun post action: passes the test's exit code through and writes LCOV under coverage.

use crate::apple;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The llvm tools the post action runs.
#[derive(Debug, PartialEq, Eq)]
struct Tools {
    llvm_cov: PathBuf,
    llvm_profdata: PathBuf,
}

/// Reads `--llvm-cov=PATH` and `--llvm-profdata=PATH`; both are required.
fn parse_tools(args: &[String]) -> Result<Tools, String> {
    let mut llvm_cov = None;
    let mut llvm_profdata = None;
    for arg in args {
        match arg.split_once('=') {
            Some(("--llvm-cov", path)) => llvm_cov = Some(PathBuf::from(path)),
            Some(("--llvm-profdata", path)) => llvm_profdata = Some(PathBuf::from(path)),
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(Tools {
        llvm_cov: llvm_cov.ok_or("--llvm-cov=PATH is required")?,
        llvm_profdata: llvm_profdata.ok_or("--llvm-profdata=PATH is required")?,
    })
}

// LCOV_EXCL_START - reason: process orchestration over env and the pinned llvm tools, issue: 9981, policy: docs/cli/commands/build-test-coverage.md
fn export(
    tools: &Tools,
    root: &Path,
    profile: &Path,
    objects: &[PathBuf],
) -> Result<String, String> {
    let profdata = &tools.llvm_profdata;
    let cov = &tools.llvm_cov;
    let scratch = PathBuf::from(env::var("TEST_TMPDIR").unwrap_or_else(|_| ".".into()));
    let merged = scratch.join("_dx_merged.profdata");
    let out = Command::new(profdata)
        .args(["merge", "--sparse"])
        .arg(profile)
        .arg("--output")
        .arg(&merged)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", profdata.display()))?;
    if !out.status.success() {
        return Err(format!(
            "llvm-profdata merge failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    crate::remove_scratch(profile);
    let arch = apple::llvm_arch(env::consts::ARCH);
    let manifest = env::var("COVERAGE_MANIFEST").ok();
    let out = Command::new(cov)
        .args(apple::apple_export_args(
            &merged,
            root,
            objects,
            arch,
            manifest.as_deref(),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", cov.display()))?;
    crate::remove_scratch(&merged);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no coverage data found") {
            eprintln!("dx apple coverage: llvm-cov found no coverage data");
            return Ok(String::new());
        }
        return Err(format!("llvm-cov export failed:\n{stderr}"));
    }
    let report = String::from_utf8(out.stdout)
        .map_err(|error| format!("llvm-cov wrote non-UTF-8: {error}"))?;
    Ok(apple::rewrite_paths(&report, root))
}

fn collect(tools: &Tools) -> Result<(), String> {
    let output = env::var("COVERAGE_OUTPUT_FILE")
        .map_err(|error| format!("COVERAGE_OUTPUT_FILE is not set: {error}"))?;
    let root = env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let profile = env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(|dir| Path::new(&dir).join(apple::PROFDATA_NAME))
        .map_err(|error| format!("TEST_UNDECLARED_OUTPUTS_DIR is not set: {error}"))?;
    let objects =
        apple::covered_objects(&env::var("TEST_BINARIES_FOR_LLVM_COV").unwrap_or_default());
    let report = if !profile.exists() || objects.is_empty() {
        eprintln!(
            "dx apple coverage: no profile or no instrumented objects; writing an empty report"
        );
        String::new()
    } else {
        export(tools, &root, &profile, &objects)?
    };
    fs::write(&output, report).map_err(|e| format!("cannot write {output}: {e}"))
}

/// Runs the rules_apple xctestrun post action and returns the test's exit code.
pub fn run_process(args: &[String]) -> i32 {
    let tools = match parse_tools(args) {
        Ok(tools) => tools,
        Err(message) => {
            eprintln!("dx apple_post_action: {message}");
            return 2;
        }
    };
    let test_exit = env::var("TEST_EXIT_CODE")
        .ok()
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    if env::var("COVERAGE").as_deref() != Ok("1") {
        return test_exit;
    }
    let result = collect(&tools);
    if let Err(message) = &result {
        eprintln!("dx apple coverage: {message}");
    }
    apple::exit_code(test_exit, result.is_ok())
}
// LCOV_EXCL_STOP - reason: end process orchestration, issue: 9981, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn both_flags_give_the_tools() {
        assert_eq!(
            parse_tools(&args(&["--llvm-profdata=/p", "--llvm-cov=/c"])),
            Ok(Tools {
                llvm_cov: PathBuf::from("/c"),
                llvm_profdata: PathBuf::from("/p"),
            })
        );
    }

    #[test]
    fn a_missing_flag_is_an_error() {
        assert_eq!(
            parse_tools(&args(&["--llvm-cov=/c"])),
            Err("--llvm-profdata=PATH is required".to_owned())
        );
        assert_eq!(
            parse_tools(&args(&["--llvm-profdata=/p"])),
            Err("--llvm-cov=PATH is required".to_owned())
        );
    }

    #[test]
    fn an_unknown_argument_is_an_error() {
        assert_eq!(
            parse_tools(&args(&["--llvm-cov=/c", "--llvm-profdata=/p", "--x"])),
            Err("unknown argument --x".to_owned())
        );
    }
}
