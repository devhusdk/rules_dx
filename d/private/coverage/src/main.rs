//! Runs one D test built with `-cov` and writes its LCOV report into COVERAGE_DIR for Bazel.

#![deny(clippy::indexing_slicing, clippy::string_slice)]

mod lst;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

fn runfiles_root() -> Option<PathBuf> {
    env::var_os("RUNFILES_DIR")
        .or_else(|| env::var_os("TEST_SRCDIR"))
        .map(PathBuf::from)
}

fn manifest_lookup(rlocation: &str) -> Option<PathBuf> {
    let manifest = env::var_os("RUNFILES_MANIFEST_FILE")?;
    let text = fs::read_to_string(manifest).ok()?;
    text.lines().find_map(|line| {
        let (key, target) = line.split_once(' ')?;
        (key == rlocation).then(|| PathBuf::from(target))
    })
}

fn resolve(rlocation: &str) -> Result<PathBuf, String> {
    if let Some(path) = runfiles_root()
        .map(|root| root.join(rlocation))
        .filter(|path| path.exists())
    {
        return Ok(path);
    }
    manifest_lookup(rlocation).ok_or_else(|| format!("{rlocation} is not in the runfiles"))
}

fn lcov_of(dir: &Path) -> String {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map(|items| {
            items
                .filter_map(|item| item.ok().map(|item| item.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "lst"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| lst::lst_to_lcov(&text))
        .collect()
}

fn run() -> Result<i32, String> {
    let upstream = env::var("DX_D_COVERAGE_UPSTREAM")
        .map_err(|_| "DX_D_COVERAGE_UPSTREAM is not set".to_owned())?;
    let upstream = resolve(&upstream)?;
    let mut command = Command::new(&upstream);
    command.args(env::args().skip(1));
    let coverage_dir = env::var_os("COVERAGE_DIR").map(PathBuf::from);
    let lst_dir = env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("dx_d_lst");
    if coverage_dir.is_some() {
        fs::create_dir_all(&lst_dir)
            .map_err(|e| format!("cannot create {}: {e}", lst_dir.display()))?;
        command.arg(format!(
            "--DRT-covopt=dstpath:{} merge:0",
            lst_dir.display()
        ));
    }
    let status = command
        .status()
        .map_err(|e| format!("cannot run {}: {e}", upstream.display()))?;
    if let Some(dir) = coverage_dir {
        let dat = dir.join("d.dat");
        fs::write(&dat, lcov_of(&lst_dir))
            .map_err(|e| format!("cannot write {}: {e}", dat.display()))?;
    }
    Ok(status.code().unwrap_or(1))
}

fn main() {
    let code = run().unwrap_or_else(|message| {
        eprintln!("dx: {message}");
        1
    });
    process::exit(code);
}
