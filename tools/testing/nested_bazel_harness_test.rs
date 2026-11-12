//! Offline self-tests of the nested Bazel harness: the pinned binary runs without bazelisk (#5689), a locked consumer resolves rules_dx extensions from its repository cache sets (#2978), and nested servers stop with their test binary, and a restarted server refetches no repository rule unless its declared environment changed, which the probe proves the guard catches.

use std::time::{Duration, Instant};

use dx_nested as nested;
use dx_testing::{read_runfiles, resolve_runfiles, run};
#[cfg(target_os = "linux")]
use {dx_nested::Case, dx_testing::rlocation};

#[cfg(target_os = "linux")]
/// The facts key rules_py's `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@rules_py+//py:extensions.bzl%python_interpreters";

#[cfg(target_os = "linux")]
/// Starts the harness consumer with its checked-in lock.
fn locked_consumer(name: &str) -> (Case, String) {
    let case = nested::harness_consumer(name);
    case.use_lock("harness");
    let lock = if nested::recording() {
        String::new()
    } else {
        dx_testing::read_text(&case.root().join("MODULE.bazel.lock"))
    };
    (case, lock)
}

#[cfg(target_os = "linux")]
#[test]
fn seeded_consumer_resolves_python_interpreters_offline() {
    let (case, lock) = locked_consumer("seed");
    let run = case.bazel(nested::HARNESS_QUERY);
    assert!(run.status.success(), "{}", run.combined());
    if nested::recording() {
        return;
    }

    assert_eq!(
        dx_testing::read_text(&case.root().join("MODULE.bazel.lock")),
        lock,
        "the offline run rewrote the consumer lock"
    );
    let lock: serde_json::Value = serde_json::from_str(&lock).expect("lock json");
    assert!(
        lock["facts"].get(PYTHON_FACTS).is_some(),
        "the consumer lock lost the python_interpreters facts: {lock}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_server_restart_refetches_no_repository() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("restart");
    let run = case.bazel(nested::HARNESS_QUERY);
    assert!(run.status.success(), "{}", run.combined());
    case.assert_no_refetch(nested::HARNESS_QUERY);
}

#[cfg(target_os = "linux")]
#[test]
fn a_changed_repo_env_refetches_the_probe() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("probe");
    case.write(
        "probe.bzl",
        r#"def _impl(rctx):
    value = rctx.os.environ.get("DX_REFETCH_PROBE")
    rctx.file("BUILD.bazel", "filegroup(name = \"all\", srcs = [], tags = [\"" + str(value) + "\"])\n")

probe = repository_rule(implementation = _impl, environ = ["DX_REFETCH_PROBE"])
"#,
    );
    case.write("BUILD.bazel", "");
    let module = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    case.write(
        "MODULE.bazel",
        &format!("{module}\nprobe = use_repo_rule(\"//:probe.bzl\", \"probe\")\nprobe(name = \"probe\")\n"),
    );
    let query = |value: &str| {
        vec![
            "query".to_string(),
            "--color=no".to_string(),
            "--curses=no".to_string(),
            format!("--repo_env=DX_REFETCH_PROBE={value}"),
            "@probe//:all".to_string(),
        ]
    };
    let first = query("a");
    let first: Vec<&str> = first.iter().map(String::as_str).collect();
    let run = case.bazel(&first);
    assert!(run.status.success(), "{}", run.combined());
    let second = query("b");
    let second: Vec<&str> = second.iter().map(String::as_str).collect();
    let changed = case.refetched_after_restart(&second, "probe-2.log");
    assert!(
        changed.iter().any(|context| context.contains("probe")),
        "a changed repo env did not refetch the probe: {changed:?}"
    );
    case.assert_no_refetch(&second);
}

#[cfg(target_os = "linux")]
#[test]
fn a_download_outside_the_cache_sets_fails_with_its_url() {
    if nested::recording() {
        return;
    }
    let (case, _) = locked_consumer("undeclared");
    let rules_py = "4e50948fbb8564a8b3a3b1f354cd9ec7ffdfe304f63c6a5feb1fdbcfbfc782bd";
    let entry = nested::cache::entry(&case.repository_cache(), rules_py);
    let kept = entry.with_extension("kept");
    std::fs::rename(&entry, &kept).expect("hide the rules_py archive");
    let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        case.bazel(nested::HARNESS_QUERY)
    })) {
        Ok(run) => panic!("no panic: {}", run.combined()),
        Err(panic) => panic.downcast_ref::<String>().cloned().unwrap_or_default(),
    };
    std::fs::rename(&kept, &entry).expect("restore the rules_py archive");
    assert!(
        message.contains("https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz"),
        "{message}"
    );
    assert!(message.contains("nested_repository_cache"), "{message}");
}

#[cfg(target_os = "linux")]
#[test]
fn nested_commands_see_no_host_path_and_a_home_under_the_test_tmpdir() {
    let tmpdir = std::path::PathBuf::from(std::env::var_os("TEST_TMPDIR").expect("TEST_TMPDIR"));
    let case = nested::case("environment");
    std::fs::write(case.join("MODULE.bazel"), "module(name = \"environment\")\n")
        .unwrap_or_else(|error| panic!("write MODULE.bazel: {error}"));
    let run = case.bazel(&["info", "output_base"]);
    assert!(run.status.success(), "{}", run.combined());
    let output_base = std::path::PathBuf::from(run.stdout.trim());
    assert!(output_base.starts_with(&tmpdir), "{}", output_base.display());
    assert!(case.root().starts_with(&tmpdir), "{}", case.root().display());
    let environ = std::fs::read(format!("/proc/{}/environ", case.servers()[0])).expect("environ");
    let environ = String::from_utf8_lossy(&environ);
    let vars: Vec<&str> = environ.split('\0').collect();
    let bin = case.wrapper();
    let path = format!("PATH={}:/dx_no_path", bin.parent().expect("bin").display());
    assert!(vars.contains(&path.as_str()), "{vars:?}");
    let home = vars
        .iter()
        .find_map(|var| var.strip_prefix("HOME="))
        .expect("HOME");
    assert!(std::path::Path::new(home).starts_with(&tmpdir), "{home}");
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
    let probe_tmp = dx_test_scratch::scratch("probe");
    let probe = run(
        &resolve_runfiles(&rel),
        &[],
        &[
            ("DX_PROBE_BAZEL_VERSION", version.trim()),
            ("TEST_TMPDIR", &probe_tmp.path().to_string_lossy()),
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
