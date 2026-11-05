//! Offline self-tests of the nested Bazel harness: the pinned binary runs without bazelisk (#5689), a seeded consumer resolves rules_dx extensions (#2978), and nested servers stop with their test binary.

use std::time::{Duration, Instant};

use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles, run};
#[cfg(target_os = "linux")]
use {dx_nested::Case, dx_testing::rlocation};

#[cfg(target_os = "linux")]
/// The facts key rules_py's `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@rules_py+//py:extensions.bzl%python_interpreters";

#[cfg(target_os = "linux")]
#[test]
fn seeded_consumer_resolves_python_interpreters_offline() {
    let case = Case::consumer("seed", "");
    let run = case.bazel(&[
        "query",
        "--color=no",
        "--curses=no",
        "@@rules_py++python_interpreters+python_interpreters//:all",
    ]);
    assert!(run.status.success(), "{}", run.combined());

    let lock = dx_testing::read_json(&case.root().join("MODULE.bazel.lock"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        lock["facts"].get(PYTHON_FACTS).is_some(),
        "the consumer lock lost the python_interpreters facts: {lock}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn nested_bazel_runs_the_pinned_binary_without_bazelisk() {
    let key = std::env::var(nested::PINNED_ENV)
        .unwrap_or_else(|_| panic!("{} must name the pinned binary", nested::PINNED_ENV));
    let pinned = std::path::absolute(rlocation(&key).expect("pinned runfile"))
        .expect("absolute pinned path");
    let config = nested::Config::from_env().unwrap_or_else(|error| panic!("config: {error}"));
    assert_eq!(config.bazel, pinned);
    assert!(config.bazel.is_absolute(), "{}", config.bazel.display());
    let version = read_runfiles(".bazelversion");
    let case = nested::case("pinned");
    std::fs::write(case.join("MODULE.bazel"), "module(name = \"pinned\")\n")
        .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let run = case.bazel(&["version"]);
    assert!(run.status.success(), "{}", run.combined());
    let label = format!("Build label: {}", version.trim());
    assert!(
        run.stdout.lines().any(|line| line.trim() == label),
        "{}",
        run.combined()
    );
}

#[test]
fn nested_servers_stop_when_their_test_binary_exits() {
    let rel = std::env::var("DX_NESTED_PROBE")
        .unwrap_or_else(|_| panic!("DX_NESTED_PROBE must name the probe"));
    let version = read_runfiles(".bazelversion");
    let disk_cache_tmp = dx_test_scratch::scratch("leak-disk-cache");
    let disk_cache = disk_cache_tmp.path().to_path_buf();
    let repository_cache_tmp = dx_test_scratch::scratch("leak-repo-cache");
    let repository_cache = repository_cache_tmp.path().to_path_buf();
    let probe = run(
        &resolve_runfiles(&rel),
        &[],
        &[
            ("DX_PROBE_BAZEL_VERSION", version.trim()),
            ("DX_NESTED_BAZEL_DISK_CACHE", &disk_cache.to_string_lossy()),
            (
                "DX_NESTED_BAZEL_REPOSITORY_CACHE",
                &repository_cache.to_string_lossy(),
            ),
        ],
    )
    .unwrap_or_else(|error| panic!("run probe: {error}"));
    assert!(probe.status.success(), "{}", probe.combined());
    let pid: u32 = probe
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("server_pid="))
        .and_then(|pid| pid.trim().parse().ok())
        .unwrap_or_else(|| panic!("probe printed no server pid: {}", probe.combined()));
    let started = Instant::now();
    while nested::alive(pid) && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !nested::alive(pid),
        "nested Bazel server {pid} outlived its test binary"
    );
}
