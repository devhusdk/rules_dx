#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RUNFILES_ENV: [&str; 3] = [
    "RUNFILES_DIR",
    "RUNFILES_MANIFEST_FILE",
    "RUNFILES_MANIFEST_ONLY",
];

#[must_use]
pub fn runtime_argv(
    declared_env: &BTreeMap<String, String>,
    preopens: &[(PathBuf, String)],
    module: &Path,
    args: &[String],
) -> Vec<String> {
    let mut command = vec!["run".to_owned()];
    for (name, value) in declared_env {
        command.push("--env".to_owned());
        command.push(format!("{name}={value}"));
    }
    for (host, guest) in preopens {
        command.push("--dir".to_owned());
        command.push(format!("{}::{guest}", host.display()));
    }
    command.push(module.display().to_string());
    command.extend(args.iter().cloned());
    command
}

fn stage_preopen(host_rel: &str) -> Result<dx_test_scratch::TempDir, String> {
    let host = dx_testing::resolve_runfiles(host_rel);
    let name = host
        .file_name()
        .ok_or_else(|| format!("preopened file {} has no file name", host.display()))?;
    let stage = dx_test_scratch::scratch("wasi-preopen")?;
    std::fs::copy(&host, stage.path().join(name))
        .map_err(|error| format!("cannot stage preopened file {}: {error}", host.display()))?;
    Ok(stage)
}

fn command(
    runtime: &Path,
    module: &Path,
    args: &[String],
    env: &BTreeMap<String, String>,
    preopens: &[(String, PathBuf)],
) -> Result<(Command, Vec<dx_test_scratch::TempDir>), String> {
    let managed = match std::env::var("DX_WASI_MANAGED_RUNTIME") {
        Ok(rel) => dx_testing::resolve_runfiles(&rel),
        Err(_) => runtime.to_path_buf(),
    };
    let stages: Vec<dx_test_scratch::TempDir> = preopens
        .iter()
        .map(|(_, host)| stage_preopen(&host.to_string_lossy()))
        .collect::<Result<_, _>>()?;
    let mounts: Vec<(PathBuf, String)> = stages
        .iter()
        .zip(preopens)
        .map(|(stage, (guest, _))| (stage.path().to_path_buf(), guest.clone()))
        .collect();
    let cache = dx_test_scratch::scratch("wasi-cache")?;
    let runtime = dx_testing::resolve_runfiles(&runtime.to_string_lossy());
    let module = dx_testing::resolve_runfiles(&module.to_string_lossy());
    let mut command = Command::new(runtime);
    command
        .args(runtime_argv(env, &mounts, &module, args))
        .env_clear()
        .env("DX_WASI_MANAGED_RUNTIME", &managed)
        .env("XDG_CACHE_HOME", cache.path());
    for name in RUNFILES_ENV {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let mut guards = stages;
    guards.push(cache);
    Ok((command, guards))
}

/// Runs one module under the runtime with only the declared args, env and preopens.
/// The runtime, module and preopened files are runfiles or absolute paths.
///
/// # Errors
///
/// Returns `Err` when a preopened file cannot be staged or the runtime cannot start.
pub fn run(
    runtime: &Path,
    module: &Path,
    args: &[String],
    env: &BTreeMap<String, String>,
    preopens: &[(String, PathBuf)],
) -> Result<Output, String> {
    let (mut command, _guards) = command(runtime, module, args, env, preopens)?;
    command
        .output()
        .map_err(|error| format!("the WASI runtime cannot execute: {error}"))
}

/// Runs one module like `run`, with inherited stdio, and returns its exit code.
///
/// # Errors
///
/// Returns `Err` when a preopened file cannot be staged or the runtime cannot start.
pub fn run_inherited(
    runtime: &Path,
    module: &Path,
    args: &[String],
    env: &BTreeMap<String, String>,
    preopens: &[(String, PathBuf)],
) -> Result<i32, String> {
    let (mut command, _guards) = command(runtime, module, args, env, preopens)?;
    let status = command
        .status()
        .map_err(|error| format!("the WASI runtime cannot execute: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_grants_only_declared_env_and_preopens() {
        let env = BTreeMap::from([
            ("B".to_owned(), "two".to_owned()),
            ("A".to_owned(), "one=1".to_owned()),
        ]);
        let preopens = vec![(PathBuf::from("/stage/0"), "/data".to_owned())];
        let argv = runtime_argv(
            &env,
            &preopens,
            Path::new("mod.wasm"),
            &["x".to_owned(), "--env".to_owned()],
        );
        assert_eq!(
            argv,
            [
                "run",
                "--env",
                "A=one=1",
                "--env",
                "B=two",
                "--dir",
                "/stage/0::/data",
                "mod.wasm",
                "x",
                "--env",
            ]
        );
    }

    #[test]
    fn argv_without_capabilities_names_only_the_module() {
        let argv = runtime_argv(&BTreeMap::new(), &[], Path::new("m.wasm"), &[]);
        assert_eq!(argv, ["run", "m.wasm"]);
    }
}
