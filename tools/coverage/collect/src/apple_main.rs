// LCOV_EXCL_START - reason: process orchestration over env and the pinned llvm tools, issue: 9981, policy: docs/cli/commands/build-test-coverage.md
use collect_native::apple;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tool(rlocation: &str) -> Result<PathBuf, String> {
    let runfiles = runfiles::Runfiles::create().map_err(|e| format!("runfiles: {e}"))?;
    runfiles::rlocation!(runfiles, rlocation).ok_or_else(|| format!("cannot resolve {rlocation}"))
}

fn export(root: &Path, profile: &Path, objects: &[PathBuf]) -> Result<String, String> {
    let profdata = tool(env!("DX_LLVM_PROFDATA"))?;
    let cov = tool(env!("DX_LLVM_COV"))?;
    let scratch = PathBuf::from(env::var("TEST_TMPDIR").unwrap_or_else(|_| ".".into()));
    let merged = scratch.join("_dx_merged.profdata");
    let out = Command::new(&profdata)
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
    if profile.is_dir() {
        fs::remove_dir_all(profile).ok();
    } else {
        fs::remove_file(profile).ok();
    }
    let arch = apple::llvm_arch(env::consts::ARCH);
    let manifest = env::var("COVERAGE_MANIFEST").ok();
    let out = Command::new(&cov)
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
    fs::remove_file(&merged).ok();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no coverage data found") {
            eprintln!("dx apple coverage: llvm-cov found no coverage data");
            return Ok(String::new());
        }
        return Err(format!("llvm-cov export failed:\n{stderr}"));
    }
    let report = String::from_utf8(out.stdout).map_err(|_| "llvm-cov wrote non-UTF-8")?;
    Ok(apple::rewrite_paths(&report, root))
}

fn collect() -> Result<(), String> {
    let output = env::var("COVERAGE_OUTPUT_FILE").map_err(|_| "COVERAGE_OUTPUT_FILE is not set")?;
    let root = env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let profile = env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(|dir| Path::new(&dir).join(apple::PROFDATA_NAME))
        .map_err(|_| "TEST_UNDECLARED_OUTPUTS_DIR is not set")?;
    let objects =
        apple::covered_objects(&env::var("TEST_BINARIES_FOR_LLVM_COV").unwrap_or_default());
    let report = if !profile.exists() || objects.is_empty() {
        eprintln!(
            "dx apple coverage: no profile or no instrumented objects; writing an empty report"
        );
        String::new()
    } else {
        export(&root, &profile, &objects)?
    };
    fs::write(&output, report).map_err(|e| format!("cannot write {output}: {e}"))
}

fn main() {
    let test_exit = env::var("TEST_EXIT_CODE")
        .ok()
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    if env::var("COVERAGE").as_deref() != Ok("1") {
        std::process::exit(test_exit);
    }
    let result = collect();
    if let Err(message) = &result {
        eprintln!("dx apple coverage: {message}");
    }
    std::process::exit(apple::exit_code(test_exit, result.is_ok()));
}
// LCOV_EXCL_STOP - reason: end process orchestration, issue: 9981, policy: docs/cli/commands/build-test-coverage.md
