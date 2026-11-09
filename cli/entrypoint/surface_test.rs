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

#[test]
fn every_rules_entry_point_is_pending_migration() {
    let legacy = lines(&read("DX_SURFACE_LEGACY_ENTRY_POINTS"));
    let pending = lines(&read("DX_SURFACE_PENDING"));
    let unlisted: Vec<_> = legacy.difference(&pending).collect();
    assert!(
        unlisted.is_empty(),
        "{unlisted:?} have <lang>/rules/defs.bzl; the entry point is <lang>/defs.bzl"
    );
}

#[test]
fn migrated_languages_are_removed_from_pending() {
    let legacy = lines(&read("DX_SURFACE_LEGACY_ENTRY_POINTS"));
    let entry = lines(&read("DX_SURFACE_ENTRY_POINTS"));
    let pending = lines(&read("DX_SURFACE_PENDING"));
    let migrated: Vec<_> = pending
        .iter()
        .filter(|name| entry.contains(*name) && !legacy.contains(*name))
        .collect();
    assert!(migrated.is_empty(), "remove from pending: {migrated:?}");
}

#[test]
fn entry_points_are_listed_in_the_consumer_surface() {
    let entry = lines(&read("DX_SURFACE_ENTRY_POINTS"));
    let pending = lines(&read("DX_SURFACE_PENDING"));
    let listed = lines(&read("DX_SURFACE_LIST"));
    for name in &entry {
        let label = format!("{name}:defs.bzl");
        assert!(
            listed.contains(&label),
            "{label} missing from consumer_surface.txt"
        );
    }
    for label in &listed {
        if let Some((package, _)) = label.split_once("/rules:") {
            assert!(
                pending.contains(package),
                "{label} is listed but {package} is not pending migration"
            );
        }
    }
}
