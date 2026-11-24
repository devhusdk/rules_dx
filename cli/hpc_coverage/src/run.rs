use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn hpc_dir(mix: &Path) -> PathBuf {
    let parent = mix.parent().unwrap_or(mix);
    let mut ancestors: Vec<&Path> = parent.ancestors().collect();
    ancestors.reverse();
    ancestors
        .into_iter()
        .find(|dir| {
            dir.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with("_.hpc"))
        })
        .unwrap_or(parent)
        .to_path_buf()
}

pub fn hpc_dirs(mix: &[OsString]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = mix.iter().map(|file| hpc_dir(Path::new(file))).collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

pub fn tix_path(outputs: &Path, test: &Path) -> PathBuf {
    let stem = test
        .file_stem()
        .map_or_else(|| "test".into(), std::ffi::OsStr::to_os_string);
    let mut name = stem;
    name.push(".tix");
    outputs.join(name)
}

pub fn lcov_args(coverage: &Path, tix: &Path, dirs: &[PathBuf]) -> Vec<OsString> {
    let mut args = vec![
        coverage.join("haskell.dat").into_os_string(),
        tix.as_os_str().to_os_string(),
    ];
    args.extend(dirs.iter().map(|dir| dir.clone().into_os_string()));
    args
}

fn status(program: &OsString, args: &[OsString], env: &[(&str, &OsString)]) -> Result<i32, String> {
    Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .status()
        .map(|status| status.code().unwrap_or(1))
        .map_err(|error| format!("cannot run {}: {error}", Path::new(program).display()))
}

const USAGE: &str = "usage: hpc_coverage <test> <hpc_lcov> [<mix file>...] -- [<test argument>...]";

pub fn run(args: &[OsString], env: &dyn Fn(&str) -> Option<OsString>) -> Result<i32, String> {
    let [test, lcov, rest @ ..] = args else {
        return Err(USAGE.to_owned());
    };
    let split = rest
        .iter()
        .position(|arg| arg == "--")
        .ok_or_else(|| USAGE.to_owned())?;
    let (mix, test_args) = rest.split_at(split);
    let test_args = test_args.get(1..).unwrap_or_default();
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    let Some(coverage) = set("COVERAGE_DIR") else {
        return status(test, test_args, &[]);
    };
    let outputs = set("TEST_UNDECLARED_OUTPUTS_DIR")
        .ok_or_else(|| "TEST_UNDECLARED_OUTPUTS_DIR is not set".to_owned())?;
    let tix = tix_path(Path::new(&outputs), Path::new(test)).into_os_string();
    let code = status(test, test_args, &[("HPCTIXFILE", &tix)])?;
    if code != 0 {
        return Ok(code);
    }
    if !Path::new(&tix).is_file() {
        dx_error::stdio::eprint_line(format_args!("dx: the Haskell test wrote no .tix file"));
        return Ok(1);
    }
    status(
        lcov,
        &lcov_args(Path::new(&coverage), Path::new(&tix), &hpc_dirs(mix)),
        &[],
    )
}
