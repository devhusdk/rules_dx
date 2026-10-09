use std::collections::BTreeMap;
use std::path::PathBuf;

use dx_testing::serde_json;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must hold the WASI test wiring"))
}

fn runfiles_path(rel: &str) -> PathBuf {
    dx_testing::resolve_runfiles(rel)
}

#[test]
fn wasi_module_runs_with_declared_capabilities() {
    let module = runfiles_path(&var("DX_WASI_MODULE"));
    let runtime = runfiles_path(&var("DX_WASI_RUNTIME"));
    let managed = runfiles_path(&var("DX_WASI_MANAGED_RUNTIME"));
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

    let mut argv = vec!["run".to_owned()];
    for (name, value) in &declared_env {
        argv.push("--env".to_owned());
        argv.push(format!("{name}={value}"));
    }
    for (guest, host_rel) in &preopens {
        let host = runfiles_path(host_rel);
        let name = host
            .file_name()
            .expect("a preopened file must have a file name");
        let stage = dx_testing::mkscratch("wasi-preopen").expect("scratch space must exist");
        std::fs::copy(&host, stage.join(name)).expect("a preopened file must stage");
        argv.push("--dir".to_owned());
        argv.push(format!("{}::{guest}", stage.display()));
    }
    argv.push(module.display().to_string());
    argv.extend(args);

    let output = std::process::Command::new(&runtime)
        .args(&argv)
        .env_clear()
        .env("DX_WASI_MANAGED_RUNTIME", &managed)
        .output()
        .expect("the WASI runtime must execute");
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
}
