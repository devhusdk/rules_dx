use dx_testing::{nested, read_runfiles, rlocation};

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
