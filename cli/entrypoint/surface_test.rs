use std::collections::BTreeSet;

fn read(key: &str) -> String {
    let rel = std::env::var(key).unwrap_or_else(|_| panic!("{key} must name a file"));
    let path = dx_testing::resolve_runfiles(&rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn the_documented_surface_matches_the_consumer_surface_list() {
    let docs = read("DX_SURFACE_README");
    let documented: BTreeSet<String> = docs
        .lines()
        .filter_map(|line| line.strip_prefix("- `@rules_dx//"))
        .filter_map(|rest| rest.split('`').next())
        .filter(|label| label.ends_with(".bzl"))
        .map(str::to_string)
        .collect();
    let mut listed = lines(&read("DX_SURFACE_LIST"));
    assert!(
        listed.remove("quality:real_aspects.bzl"),
        "the list must cover //quality:real_aspects.bzl"
    );
    assert_eq!(
        documented, listed,
        "docs/cli/commands/README.md Supported Surface and cli/entrypoint/consumer_surface.txt must match"
    );
}

#[test]
fn every_listed_file_was_followed_without_reaching_a_dev_only_repository() {
    let listed = lines(&read("DX_SURFACE_LIST"));
    let closure = lines(&read("DX_SURFACE_CLOSURE"));
    for label in &listed {
        assert!(
            closure.contains(label),
            "{label} missing from the load closure"
        );
    }
}
