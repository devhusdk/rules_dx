//! Fails when the public API differs from the snapshot checked in under `api/`.

mod snapshot;

#[test]
fn checked_in_snapshot_matches_the_code() {
    let files = snapshot::render().expect("snapshot renders");
    let paths = dx_testing::env_text("DX_API_SNAPSHOT");
    let mut stale = Vec::new();
    for (name, text) in &files {
        let key = paths
            .split_whitespace()
            .find(|key| key.ends_with(&format!("api/{name}")))
            .unwrap_or_else(|| panic!("api/{name} is not in the test data"));
        let path = dx_testing::rlocation(key).unwrap_or_else(|error| panic!("{key}: {error}"));
        let checked_in = dx_testing::read_text(&path);
        if *name == "schemas.txt" {
            let problems = snapshot::unbumped_schemas(&checked_in, text);
            assert!(problems.is_empty(), "{}", problems.join("\n"));
        }
        if checked_in != *text {
            stale.push(format!("api/{name}"));
        }
    }
    assert!(
        stale.is_empty(),
        "the public API changed: {}. If the change is intended, run `bazel run //tools/api:update` and commit the result",
        stale.join(", ")
    );
}
