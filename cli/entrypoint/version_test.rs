fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a file"));
    let path = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn status_fixtures_take_the_release_version_from_pins() {
    let repos = read("DX_ENTRYPOINT_REPOS");
    let pins = read("DX_PREBUILT_PINS");
    let version = pins
        .lines()
        .find_map(|line| line.strip_prefix("DX_VERSION = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("pins.bzl must define DX_VERSION");
    assert!(!version.is_empty());
    assert!(repos.contains("load(\"//cli/prebuilt:pins.bzl\", \"DX_VERSION\")"));
    for line in repos.lines() {
        let tracks_rules_dx = line.contains("bazel_dep(name = \"rules_dx\"")
            || line.contains("/.dx/pins")
            || line.starts_with("workspace ");
        assert!(
            !tracks_rules_dx || !line.contains("0.0.0"),
            "fixture line pins the literal 0.0.0: {line}"
        );
    }
}
