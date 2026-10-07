//! One matrix case: resolve runfiles, run the runner, print, verify, compare.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use dx_path::runfiles::Resolver;
use quality_adapter::exec::{hermetic_env, spawn};
use quality_result::decode_validated;

use crate::manifest::{runfiles_root, Manifest, Rlocation};
use crate::verify;
use crate::Error;

const RUNFILES_DIR: &str = "RUNFILES_DIR";
const UPDATE_EXPECT: &str = "UPDATE_EXPECT";

impl Rlocation for Resolver {
    fn path(&self, rlocation: &str) -> Result<PathBuf, Error> {
        let path = self
            .lookup(rlocation)
            .map_err(|error| Error::MissingRunfile {
                key: rlocation.to_owned(),
                detail: error.to_string(),
            })?;
        if !path.is_file() {
            return Err(Error::MissingRunfile {
                key: rlocation.to_owned(),
                detail: format!("{} is not a file", path.display()),
            });
        }
        Ok(path)
    }
}

/// Runs the case the manifest names in this process' own runfiles.
pub fn run(manifest_rlocation: &str) -> Result<(), Error> {
    let files = Resolver::from_env().map_err(|error| Error::Runfiles(error.to_string()))?;
    let manifest = read_manifest(&files, manifest_rlocation)?;
    let update = update_requested(std::env::var(UPDATE_EXPECT).ok().as_deref());
    let staged = if update {
        Some(verify::update_dir())
    } else {
        None
    };
    execute(
        &manifest,
        &files,
        &runfiles_root(),
        &work_dir()?,
        staged.as_deref(),
    )
}

/// Runs one case from an already-read manifest.
///
/// `root` is the tree the child sees as `RUNFILES_DIR` and `work` is this case's
/// scratch parent. A `staged` directory writes the fresh result there instead of
/// comparing it against the pinned snapshot; the pinned snapshot is never
/// written either way.
pub fn execute<L: Rlocation>(
    manifest: &Manifest,
    files: &L,
    root: &Path,
    work: &Path,
    staged: Option<&Path>,
) -> Result<(), Error> {
    let scratch = work.join("scratch");
    std::fs::create_dir_all(&scratch).map_err(|error| Error::Scratch {
        path: scratch.display().to_string(),
        detail: error.to_string(),
    })?;
    let out = work.join("out.pb");
    let env = hermetic_env(
        &scratch,
        &[(OsString::from(RUNFILES_DIR), root.to_path_buf().into())],
        &ambient(),
    );
    let run = spawn(
        &manifest.runner_argv(&out, &scratch, root, files)?,
        &scratch,
        &env,
    )
    .map_err(|error| Error::Runner(error.to_string()))?;
    if run.code != Some(0) {
        return Err(Error::Runner(format!(
            "exited with {:?}\n{}",
            run.code,
            String::from_utf8_lossy(&run.stderr)
        )));
    }
    let bytes = std::fs::read(&out).map_err(|error| Error::Result(error.to_string()))?;
    let result = decode_validated(&bytes).map_err(|error| Error::Result(error.to_string()))?;
    verify::verify_result(manifest, &result)?;
    let printed = print_result(manifest, &out, &scratch, &env, files)?;
    verify::verify_print(&printed, &result)?;
    match staged {
        Some(dir) => stage(dir, manifest, printed.as_bytes()),
        None => compare(files, manifest, printed.as_bytes()),
    }
}

/// Whether one value asks for a fresh snapshot to be staged.
pub fn update_requested(value: Option<&str>) -> bool {
    value == Some("1")
}

fn read_manifest<L: Rlocation>(files: &L, manifest_rlocation: &str) -> Result<Manifest, Error> {
    let path = files.path(manifest_rlocation)?;
    let bytes = std::fs::read(&path).map_err(|error| Error::Manifest {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    Manifest::parse(&bytes)
}

fn print_result<L: Rlocation>(
    manifest: &Manifest,
    out: &Path,
    scratch: &Path,
    env: &[(OsString, OsString)],
    files: &L,
) -> Result<String, Error> {
    let argv = manifest.printer_argv(out, files)?;
    let printed = spawn(&argv, scratch, env).map_err(|error| Error::Printer(error.to_string()))?;
    if printed.code != Some(0) {
        return Err(Error::Printer(format!(
            "exited with {:?}\n{}",
            printed.code,
            String::from_utf8_lossy(&printed.stderr)
        )));
    }
    String::from_utf8(printed.stdout).map_err(|error| Error::Printer(error.to_string()))
}

fn stage(dir: &Path, manifest: &Manifest, actual: &[u8]) -> Result<(), Error> {
    let staged = verify::stage_update(dir, &manifest.update_name(), actual)?;
    println!(
        "snapshot UPDATE_EXPECT: staged fresh actual at {}",
        staged.display()
    );
    println!(
        "copy it to {}, then review before pinning.",
        manifest.snapshot_path()
    );
    println!("matrix PASS (updated): {}", manifest.name);
    Ok(())
}

fn compare<L: Rlocation>(files: &L, manifest: &Manifest, actual: &[u8]) -> Result<(), Error> {
    let path = files.path(&manifest.expected)?;
    let expected = std::fs::read(&path).map_err(|error| Error::Snapshot {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    if expected == actual {
        println!("matrix PASS: {}", manifest.name);
        return Ok(());
    }
    let snapshot = String::from_utf8(expected).map_err(|error| Error::Snapshot {
        path: manifest.snapshot_path(),
        detail: error.to_string(),
    })?;
    let printed = String::from_utf8(actual.to_vec()).map_err(|error| Error::Snapshot {
        path: manifest.snapshot_path(),
        detail: error.to_string(),
    })?;
    println!("{}", verify::snapshot_diff(&snapshot, &printed)?);
    println!("--- actual print_result:\n{printed}");
    println!(
        "re-run with {UPDATE_EXPECT}=1 (bazel test --test_env={UPDATE_EXPECT}) to stage the fresh snapshot, then review it before pinning."
    );
    Err(Error::Mismatch(manifest.snapshot_path()))
}

fn ambient() -> Vec<(OsString, OsString)> {
    std::env::vars_os().collect()
}

fn work_dir() -> Result<PathBuf, Error> {
    let base = std::env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    Ok(base.join("matrix_work"))
}

#[path = "run_tests.rs"]
#[cfg(test)]
mod tests;
