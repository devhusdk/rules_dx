use std::collections::BTreeMap;
use std::path::PathBuf;

use dx_testing::serde_json;

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must hold the WASI test wiring"))
}

fn main() {
    let module = dx_testing::resolve_runfiles(&var("DX_WASI_MODULE"));
    let runtime = dx_testing::resolve_runfiles(&var("DX_WASI_RUNTIME"));
    let env: BTreeMap<String, String> =
        serde_json::from_str(&var("DX_WASI_ENV")).expect("DX_WASI_ENV must be a JSON object");
    let preopens: BTreeMap<String, String> = serde_json::from_str(&var("DX_WASI_PREOPENS"))
        .expect("DX_WASI_PREOPENS must be a JSON object");
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
    let code = wasi_run::run_inherited(&runtime, &module, &args, &env, &preopens);
    std::process::exit(code);
}
