//! The test run and the conversion of its `.tix` file.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The hpc data one test left in its undeclared outputs.
#[derive(Debug, PartialEq, Eq)]
pub struct HpcData {
    /// The first `.tix` file by name.
    pub tix: PathBuf,
    /// Every entry of `mix/`, sorted.
    pub mix: Vec<PathBuf>,
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries
}

/// Finds the `.tix` file and the mix directories below `outputs`.
pub fn hpc_data(outputs: &Path) -> Option<HpcData> {
    let tix = sorted(outputs)
        .into_iter()
        .find(|path| path.extension().is_some_and(|extension| extension == "tix"))?;
    Some(HpcData {
        tix,
        mix: sorted(&outputs.join("mix")),
    })
}

/// The arguments `hpc_lcov` gets: the LCOV file below `coverage`, the `.tix` file and the mix directories.
pub fn lcov_args(coverage: &Path, data: &HpcData) -> Vec<OsString> {
    let mut args = vec![
        coverage.join("haskell.dat").into_os_string(),
        data.tix.clone().into_os_string(),
    ];
    args.extend(data.mix.iter().map(|dir| dir.clone().into_os_string()));
    args
}

fn status(program: &OsString, args: &[OsString]) -> Result<i32, String> {
    Command::new(program)
        .args(args)
        .status()
        .map(|status| status.code().unwrap_or(1))
        .map_err(|error| format!("cannot run {}: {error}", Path::new(program).display()))
}

/// Runs the test and returns its exit code, or the one of the conversion after a passing test under coverage.
pub fn run(args: &[OsString], env: &dyn Fn(&str) -> Option<OsString>) -> Result<i32, String> {
    let [test, lcov, rest @ ..] = args else {
        return Err("usage: hpc_coverage <test> <hpc_lcov> [<test argument>...]".to_owned());
    };
    let code = status(test, rest)?;
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    let Some(coverage) = set("COVERAGE_DIR").filter(|_| code == 0) else {
        return Ok(code);
    };
    let outputs = set("TEST_UNDECLARED_OUTPUTS_DIR")
        .ok_or_else(|| "TEST_UNDECLARED_OUTPUTS_DIR is not set".to_owned())?;
    let Some(data) = hpc_data(Path::new(&outputs)) else {
        eprintln!("dx: the Haskell test wrote no .tix file");
        return Ok(1);
    };
    status(lcov, &lcov_args(Path::new(&coverage), &data))
}
