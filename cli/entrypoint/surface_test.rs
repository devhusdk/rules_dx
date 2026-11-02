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
