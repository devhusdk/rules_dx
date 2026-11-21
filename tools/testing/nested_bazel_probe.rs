use std::process::ExitCode;

use dx_nested as nested;

fn server_pid(case: &nested::Case, version: &str) -> Result<String, String> {
    std::fs::write(case.join(".bazelversion"), version)
        .map_err(|error| format!("write .bazelversion: {error}"))?;
    std::fs::write(case.join("MODULE.bazel"), "module(name = \"probe\")\n")
        .map_err(|error| format!("write MODULE.bazel: {error}"))?;
    let info = case.bazel(&["info", "server_pid"]);
    if !info.status.success() {
        return Err(info.combined());
    }
    if case.servers().len() != 1 {
        return Err("one server per test binary".to_owned());
    }
    Ok(info.stdout.trim().to_owned())
}

fn run() -> Result<String, String> {
    let version = std::env::var("DX_PROBE_BAZEL_VERSION")
        .map_err(|error| format!("DX_PROBE_BAZEL_VERSION must name the Bazel version: {error}"))?;
    let first = server_pid(&nested::case("probe"), &version)?;
    let second = server_pid(&nested::case("probe"), &version)?;
    if first != second {
        return Err(format!("cases must share one server: {first} != {second}"));
    }
    Ok(second)
}

fn main() -> ExitCode {
    match run() {
        Ok(pid) => {
            println!("server_pid={pid}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nested_bazel_probe: {error}");
            ExitCode::FAILURE
        }
    }
}
