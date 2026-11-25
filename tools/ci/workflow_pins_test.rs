use dx_testing::{read_runfiles, runfiles_root, sha256_file};

const REUSABLE: [&str; 3] = [
    "reusable-consumer.yml",
    "reusable-docs.yml",
    "reusable-release.yml",
];

const DIGEST_RECORD: &str = "tools/ci/starter_pin.sha256";

fn recorded_digests() -> Vec<(String, String)> {
    let mut records: Vec<(String, String)> = Vec::new();
    for line in read_runfiles(DIGEST_RECORD).lines() {
        let (digest, path) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("{DIGEST_RECORD}: malformed line {line:?}"));
        let valid = digest.len() == 64
            && digest
                .chars()
                .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
        assert!(valid, "{DIGEST_RECORD}: bad digest in {line:?}");
        let known = REUSABLE
            .iter()
            .any(|name| path == format!(".github/workflows/{name}"));
        assert!(known, "{DIGEST_RECORD}: unexpected path {path:?}");
        assert!(
            records.iter().all(|(seen, _)| seen != path),
            "{DIGEST_RECORD}: duplicate path {path:?}"
        );
        records.push((path.to_string(), digest.to_string()));
    }
    records
}

#[test]
fn every_reusable_workflow_matches_the_digest_recorded_for_the_starter_pin() {
    let records = recorded_digests();
    let mut recorded: Vec<&str> = records.iter().map(|(path, _)| path.as_str()).collect();
    recorded.sort_unstable();
    let mut expected: Vec<String> = REUSABLE
        .iter()
        .map(|name| format!(".github/workflows/{name}"))
        .collect();
    expected.sort();
    assert_eq!(
        recorded, expected,
        "{DIGEST_RECORD} must list every reusable workflow"
    );
    for (path, digest) in &records {
        let actual = sha256_file(&runfiles_root().join(path)).expect("hash workflow");
        assert_eq!(
            &actual, digest,
            "{path} no longer matches the workflow the starters pin; update {DIGEST_RECORD} in this PR; after merge, bump both starters and CONSUMER_WORKFLOW_PIN to the merge commit"
        );
    }
}
