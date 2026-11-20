#![deny(clippy::map_err_ignore, clippy::unused_result_ok)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use dx_testing::serde_json;

fn var(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|error| format!("{name} must hold the WASI test wiring: {error}"))
}

fn run() -> Result<i32, String> {
    let module = dx_testing::resolve_runfiles(&var("DX_WASI_MODULE")?);
    let runtime = dx_testing::resolve_runfiles(&var("DX_WASI_RUNTIME")?);
    let env: BTreeMap<String, String> = serde_json::from_str(&var("DX_WASI_ENV")?)
        .map_err(|error| format!("DX_WASI_ENV must be a JSON object: {error}"))?;
    let preopens: BTreeMap<String, String> = serde_json::from_str(&var("DX_WASI_PREOPENS")?)
        .map_err(|error| format!("DX_WASI_PREOPENS must be a JSON object: {error}"))?;
    let preopens: Vec<(String, PathBuf)> = preopens
        .into_iter()
        .map(|(guest, host)| (guest, PathBuf::from(host)))
        .collect();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(filter) = std::env::var("TESTBRIDGE_TEST_ONLY")
        .ok()
        .filter(|filter| !filter.is_empty())
    {
        args.push(filter);
    }
    wasi_run::run_inherited(&runtime, &module, &args, &env, &preopens)
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
