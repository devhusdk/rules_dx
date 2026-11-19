//! Fails when the public API differs from the snapshot checked in under `api/` or the page generated from it.

mod page;
mod snapshot;

#[test]
fn checked_in_snapshot_matches_the_code() {
    let lang_builds: Vec<(String, String)> = snapshot::lang_build_paths()
        .expect("DX_API_LANG_BUILDS is set")
        .into_iter()
        .map(|path| {
            let package = snapshot::package_of(&path).expect("a package BUILD file");
            (
                package,
                dx_testing::read_text(&dx_testing::resolve_runfiles(&path)),
            )
        })
        .collect();
    let starlark: Vec<Vec<u8>> = dx_testing::env_files("DX_API_STARLARK")
        .iter()
        .map(|path| {
            std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        })
        .collect();
    let files = snapshot::render(&lang_builds, &starlark).expect("snapshot renders");
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
    let surface = page::render(
        &files["cli.json"],
        &files["labels.txt"],
        &files["schemas.txt"],
    )
    .expect("page renders");
    let path = dx_testing::rlocation(&dx_testing::env_text("DX_SURFACE_PAGE"))
        .unwrap_or_else(|error| panic!("{}: {error}", page::PATH));
    if dx_testing::read_text(&path) != surface {
        stale.push(page::PATH.to_owned());
    }
    assert!(
        stale.is_empty(),
        "the public API changed: {}. If the change is intended, run `bazel run //tools/api:update` and commit the result",
        stale.join(", ")
    );
}
