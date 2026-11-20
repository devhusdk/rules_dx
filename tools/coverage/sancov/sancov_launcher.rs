//! Runs one sancov-instrumented test, symbolizes the PCs it wrote and writes its LCOV report into COVERAGE_DIR.
//!
//! Reads runfiles keys from DX_SANCOV_UPSTREAM, DX_SANCOV_SOURCES and DX_LLVM_SYMBOLIZER, plus
//! DX_DSYMUTIL for a Mach-O test. Without COVERAGE_DIR it only runs the test.

#![deny(
    clippy::indexing_slicing,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok
)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs the test and the pinned llvm tools, which exit on their own"
)]

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn resolve(runfiles: &runfiles::Runfiles, var: &str) -> Result<PathBuf, String> {
    let key = env::var(var).map_err(|e| format!("{var}: {e}"))?;
    runfiles::rlocation!(runfiles, &key).ok_or_else(|| format!("cannot resolve {var}={key}"))
}

fn checked(command: &mut Command, what: &str) -> Result<Vec<u8>, String> {
    let out = command
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {what}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{what} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out.stdout)
}

/// Symbolizes every PC in `pcs` against `test`, through a dSYM when `dsymutil` is given.
fn symbolize(
    symbolizer: &Path,
    dsymutil: Option<&Path>,
    test: &Path,
    pcs: &str,
    scratch: &Path,
) -> Result<String, String> {
    let mut command = Command::new(symbolizer);
    command.arg("--obj").arg(test).arg("--addresses");
    if let Some(dsymutil) = dsymutil {
        // A Mach-O test keeps its DWARF in the object files its debug map names; llvm-symbolizer reads only a dSYM.
        let dsym = scratch.join("test.dSYM");
        checked(
            Command::new(dsymutil).arg(test).arg("-o").arg(&dsym),
            "dsymutil",
        )?;
        let mut hint = OsString::from("--dsym-hint=");
        hint.push(&dsym);
        command.arg(hint);
    }
    let addresses: String = pcs
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(|address| format!("{address}\n"))
        .collect();
    // Feeding stdin from a file means no writer can stall on a full stdout pipe.
    let input = scratch.join("addresses.txt");
    fs::write(&input, addresses).map_err(|e| format!("cannot write {}: {e}", input.display()))?;
    let stdin =
        fs::File::open(&input).map_err(|e| format!("cannot read {}: {e}", input.display()))?;
    let out = checked(command.stdin(stdin), "llvm-symbolizer")?;
    String::from_utf8(out).map_err(|e| format!("llvm-symbolizer wrote non-UTF-8: {e}"))
}

fn run(args: &[OsString]) -> Result<u8, String> {
    let runfiles = runfiles::Runfiles::create().map_err(|e| format!("runfiles: {e}"))?;
    let test = resolve(&runfiles, "DX_SANCOV_UPSTREAM")?;
    let coverage_dir = env::var_os("COVERAGE_DIR").filter(|dir| !dir.is_empty());
    let scratch = env::var_os("TEST_TMPDIR")
        .map_or_else(env::temp_dir, PathBuf::from)
        .join("dx_sancov");
    fs::create_dir_all(&scratch)
        .map_err(|e| format!("cannot create {}: {e}", scratch.display()))?;
    let pcs_path = scratch.join("pcs.txt");
    let mut command = Command::new(&test);
    command.args(args);
    if coverage_dir.is_some() {
        command.env("DX_SANCOV_PCS", &pcs_path);
    }
    let status = command
        .status()
        .map_err(|e| format!("cannot run {}: {e}", test.display()))?;
    let code = status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1);
    let Some(coverage_dir) = coverage_dir else {
        return Ok(code);
    };
    let pcs = fs::read_to_string(&pcs_path)
        .map_err(|e| format!("{} wrote no sancov PCs: {e}", test.display()))?;
    if pcs.trim().is_empty() {
        return Err(format!("{} wrote no sancov PCs", test.display()));
    }
    let symbolizer = resolve(&runfiles, "DX_LLVM_SYMBOLIZER")?;
    let dsymutil = match env::var_os("DX_DSYMUTIL") {
        Some(_) => Some(resolve(&runfiles, "DX_DSYMUTIL")?),
        None => None,
    };
    let symbolized = symbolize(&symbolizer, dsymutil.as_deref(), &test, &pcs, &scratch)?;
    let lcov = dx_lcov::sancov_to_lcov(&pcs, &symbolized).map_err(|e| e.to_string())?;
    let sources_path = resolve(&runfiles, "DX_SANCOV_SOURCES")?;
    let list = fs::read_to_string(&sources_path)
        .map_err(|e| format!("cannot read {}: {e}", sources_path.display()))?;
    let sources: Vec<&str> = list.lines().filter(|line| !line.is_empty()).collect();
    let lcov = dx_lcov::keep_sources(&lcov, &sources);
    if lcov.is_empty() && !sources.is_empty() {
        // On macOS this means dsymutil could not open the object files the test's debug map names.
        dx_error::stdio::eprint_line(format_args!(
            "dx: no sancov line maps to a listed source of {}",
            test.display()
        ));
    }
    let report = Path::new(&coverage_dir).join("sancov.dat");
    dx_atomic_fs::write_atomic(&report, lcov.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", report.display()))?;
    Ok(code)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            dx_error::stdio::eprint_line(format_args!("dx: {error}"));
            ExitCode::FAILURE
        }
    }
}
