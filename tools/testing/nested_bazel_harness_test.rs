//! Offline self-tests of the nested Bazel harness: the pinned binary runs without bazelisk (#5689), and a seeded consumer resolves rules_dx extensions (#2978).

use dx_nested::{self as nested, Case};
use dx_testing::{read_runfiles, rlocation};

/// The facts key rules_py's `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@rules_py+//py:extensions.bzl%python_interpreters";

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
