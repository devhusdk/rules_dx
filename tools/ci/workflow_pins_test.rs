use std::path::{Path, PathBuf};

const CALLERS: [(&str, &str); 2] = [
    ("examples/consumer-ci/caller.yml", "reusable-consumer.yml"),
    ("examples/docs-ci/caller.yml", "reusable-docs.yml"),
];

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn read(rel: &str) -> String {
    let path = workspace_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {rel}: {error}"))
}

fn pinned_commit(caller: &str, workflow: &str) -> String {
    let want = format!("rules_dx/.github/workflows/{workflow}@");
    let text = read(caller);
    let line = text
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("uses: "))
        .unwrap_or_else(|| panic!("{caller} has no uses: line"));
    assert!(
        line.starts_with(&want),
        "{caller} must call {want}<commit>, found {line}"
    );
    let commit = line[want.len()..].to_owned();
    let hex = commit
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    assert!(
        commit.len() == 40 && hex,
        "{caller} must pin a full lowercase commit SHA, found {commit}"
    );
    commit
}

#[test]
fn every_starter_pins_its_reusable_workflow_at_a_full_commit() {
    for (caller, workflow) in CALLERS {
        let path = workspace_root().join(".github/workflows").join(workflow);
        assert!(
            path.is_file(),
            "{caller} pins {workflow}, which this workspace does not ship"
        );
        pinned_commit(caller, workflow);
    }
}

#[test]
fn both_starters_pin_one_reviewed_commit() {
    let mut pins: Vec<(&str, String)> = CALLERS
        .iter()
        .map(|(caller, workflow)| (*caller, pinned_commit(caller, workflow)))
        .collect();
    let (_, head) = pins.remove(0);
    for (caller, commit) in &pins {
        assert_eq!(
            commit, &head,
            "{caller} pins a different commit than {}; bump them together",
            CALLERS[0].0
        );
    }
}
