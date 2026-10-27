use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use dx_testing::serde_json;

const RUNFILES_ENV: [&str; 3] = [
    "RUNFILES_DIR",
    "RUNFILES_MANIFEST_FILE",
    "RUNFILES_MANIFEST_ONLY",
];

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must hold the WASI test wiring"))
}

pub fn runtime_argv(
    declared_env: &BTreeMap<String, String>,
    preopens: &[(PathBuf, String)],
    module: &Path,
    args: &[String],
) -> Vec<String> {
    let mut argv = vec!["run".to_owned()];
    for (name, value) in declared_env {
        argv.push("--env".to_owned());
        argv.push(format!("{name}={value}"));
    }
    for (host, guest) in preopens {
        argv.push("--dir".to_owned());
        argv.push(format!("{}::{guest}", host.display()));
    }
    argv.push(module.display().to_string());
    argv.extend(args.iter().cloned());
    argv
}

fn stage_preopen(host_rel: &str) -> PathBuf {
    let host = dx_testing::resolve_runfiles(host_rel);
    let name = host
        .file_name()
        .expect("a preopened file must have a file name");
    let stage = dx_testing::mkscratch("wasi-preopen").expect("scratch space must exist");
    std::fs::copy(&host, stage.join(name)).expect("a preopened file must stage");
    stage
}

pub fn assert_declared_run() {
    let module = dx_testing::resolve_runfiles(&var("DX_WASI_MODULE"));
    let runtime = dx_testing::resolve_runfiles(&var("DX_WASI_RUNTIME"));
    let managed = dx_testing::resolve_runfiles(&var("DX_WASI_MANAGED_RUNTIME"));
    let args: Vec<String> =
        serde_json::from_str(&var("DX_WASI_ARGS")).expect("DX_WASI_ARGS must be a JSON array");
    let declared_env: BTreeMap<String, String> =
        serde_json::from_str(&var("DX_WASI_ENV")).expect("DX_WASI_ENV must be a JSON object");
    let preopens: BTreeMap<String, String> = serde_json::from_str(&var("DX_WASI_PREOPENS"))
        .expect("DX_WASI_PREOPENS must be a JSON object");
    let expected_code: i32 = var("DX_WASI_EXPECTED_CODE")
        .parse()
        .expect("DX_WASI_EXPECTED_CODE must be an integer");
    let expected = var("DX_WASI_EXPECTED");
    let expected_stderr = var("DX_WASI_EXPECTED_STDERR");

    let staged: Vec<(PathBuf, String)> = preopens
        .iter()
        .map(|(guest, host_rel)| (stage_preopen(host_rel), guest.clone()))
        .collect();
    let cache = dx_testing::mkscratch("wasi-cache").expect("scratch space must exist");
    let mut command = std::process::Command::new(&runtime);
    command
        .args(runtime_argv(&declared_env, &staged, &module, &args))
        .env_clear()
        .env("DX_WASI_MANAGED_RUNTIME", &managed)
        .env("XDG_CACHE_HOME", &cache);
    for name in RUNFILES_ENV {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    let output = command.output().expect("the WASI runtime must execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected WASI exit code, stderr: {stderr}"
    );
    assert_eq!(
        stdout.trim_end_matches(['\r', '\n']),
        expected,
        "unexpected WASI output, stderr: {stderr}"
    );
    assert!(
        stderr.contains(&expected_stderr),
        "WASI stderr lacks {expected_stderr:?}, stderr: {stderr}"
    );
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
