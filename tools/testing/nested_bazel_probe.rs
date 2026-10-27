use dx_nested as nested;

fn server_pid(case: &nested::Case, version: &str) -> String {
    std::fs::write(case.join(".bazelversion"), version)
        .unwrap_or_else(|error| panic!("write .bazelversion: {error}"));
    std::fs::write(case.join("MODULE.bazel"), "module(name = \"probe\")\n")
        .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let info = case.bazel(&["info", "server_pid"]);
    assert!(info.status.success(), "{}", info.combined());
    assert_eq!(case.servers().len(), 1, "one server per test binary");
    info.stdout.trim().to_owned()
}

fn main() {
    let version = std::env::var("DX_PROBE_BAZEL_VERSION")
        .unwrap_or_else(|_| panic!("DX_PROBE_BAZEL_VERSION must name the Bazel version"));
    let first = server_pid(&nested::case("probe"), &version);
    let second = server_pid(&nested::case("probe"), &version);
    assert_eq!(first, second, "cases must share one server");
    println!("server_pid={second}");
}
