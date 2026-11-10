//! Offline self-tests of the nested Bazel harness: the pinned binary runs without bazelisk (#5689), a consumer resolves rules_dx extensions from its checked-in lock and cache list (#2978), and nested servers stop with their test binary.

use std::time::{Duration, Instant};

use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles, run};
#[cfg(target_os = "linux")]
use {dx_nested::Case, dx_testing::rlocation};

#[cfg(target_os = "linux")]
/// The facts key rules_py's `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@rules_py+//py:extensions.bzl%python_interpreters";

/// The checked-in lock of the consumer, as a runfiles path.
#[cfg(target_os = "linux")]
const CONSUMER_LOCK: &str = "tools/testing/fixtures/nested_harness/MODULE.bazel.lock";

#[cfg(target_os = "linux")]
#[test]
fn locked_consumer_resolves_python_interpreters_offline() {
    let case = Case::consumer("locked", "");
    case.lock(CONSUMER_LOCK);
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
fn nested_commands_see_no_host_path_and_a_home_under_the_test_tmpdir() {
    if std::env::var_os(nested::UPDATE_ENV).is_some() {
        return;
    }
    let case = nested::case("env");
    let tmp = std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR");
    assert!(case.root().starts_with(&tmp), "{}", case.root().display());
    assert!(case.user_root().starts_with(&tmp));
    let command = case.bazel_command(&["version"]);
    let env = |name: &str| {
        command
            .get_envs()
            .find(|(key, _)| *key == name)
            .and_then(|(_, value)| value)
            .unwrap_or_else(|| panic!("{name} is not set"))
            .to_string_lossy()
            .into_owned()
    };
    let wrapper = case.wrapper();
    let bin = wrapper.parent().expect("bin");
    assert_eq!(env("PATH"), format!("{}:/dx_no_path", bin.display()));
    assert!(env("HOME").starts_with(&tmp), "{}", env("HOME"));
}

#[cfg(target_os = "linux")]
#[test]
fn an_unlisted_download_fails_with_its_url() {
    if std::env::var_os(nested::UPDATE_ENV).is_some() {
        return;
    }
    let case = nested::case("unlisted");
    std::fs::write(
        case.join("MODULE.bazel"),
        "module(name = \"unlisted\")\nbazel_dep(name = \"dx_no_such_module\", version = \"1.0\")\n",
    )
    .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    std::fs::write(
        case.join("MODULE.bazel.lock"),
        "{\"lockFileVersion\": 28}\n",
    )
    .unwrap_or_else(|error| panic!("write MODULE.bazel.lock: {error}"));
    let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        case.bazel(&["mod", "graph"])
    })) {
        Ok(run) => panic!("no panic: {}", run.combined()),
        Err(panic) => panic.downcast_ref::<String>().cloned().unwrap_or_default(),
    };
    assert!(message.contains("https://bcr.bazel.build/"), "{message}");
    assert!(message.contains(".update target"), "{message}");
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
    if std::env::var_os(nested::UPDATE_ENV).is_some() {
        return;
    }
    let rel = std::env::var("DX_NESTED_PROBE")
        .unwrap_or_else(|_| panic!("DX_NESTED_PROBE must name the probe"));
    let version = read_runfiles(".bazelversion");
    let probe_tmp = dx_test_scratch::scratch("nested-probe");
    let probe = run(
        &resolve_runfiles(&rel),
        &[],
        &[
            ("DX_PROBE_BAZEL_VERSION", version.trim()),
            ("TEST_TMPDIR", &probe_tmp.path().to_string_lossy()),
            (nested::UPDATE_ENV, ""),
            (nested::CACHE_ENV, ""),
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
