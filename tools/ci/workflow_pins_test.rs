use std::path::{Path, PathBuf};

const CALLERS: [(&str, &str); 2] = [
    ("examples/consumer-ci/caller.yml", "reusable-consumer.yml"),
    ("examples/docs-ci/caller.yml", "reusable-docs.yml"),
];

const REUSABLE: [&str; 2] = ["reusable-consumer.yml", "reusable-docs.yml"];

const PIN_STEP: &str = "- name: Verify rules_dx pin";

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

/// Returns each job block of a workflow file with its name, after the jobs: key.
fn jobs(workflow: &str) -> Vec<(String, String)> {
    let text = read(&format!(".github/workflows/{workflow}"));
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == "jobs:")
        .unwrap_or_else(|| panic!("{workflow} has no jobs: key"));
    let mut starts: Vec<(usize, String)> = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        let Some(name) = line
            .strip_prefix("  ")
            .and_then(|rest| rest.strip_suffix(':'))
            .filter(|name| !name.is_empty() && name.chars().all(is_job_key_char))
        else {
            continue;
        };
        starts.push((index, name.to_owned()));
    }
    assert!(
        !starts.is_empty(),
        "{workflow} names no job after its jobs: key"
    );
    let mut out = Vec::with_capacity(starts.len());
    for (slot, (index, name)) in starts.iter().enumerate() {
        let end = starts.get(slot + 1).map_or(lines.len(), |(next, _)| *next);
        out.push((name.clone(), lines[*index..end].join("\n")));
    }
    out
}

/// Returns whether a character may appear in a top-level job key.
fn is_job_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')
}

#[test]
fn every_checkout_job_verifies_the_rules_dx_pin() {
    for workflow in REUSABLE {
        let jobs = jobs(workflow);
        let checkout_jobs: Vec<&(String, String)> = jobs
            .iter()
            .filter(|(_, body)| body.contains("uses: actions/checkout@"))
            .collect();
        assert!(
            !checkout_jobs.is_empty(),
            "{workflow} has no job that checks out the tree"
        );
        for (name, body) in checkout_jobs {
            let pins = body.matches(PIN_STEP).count();
            assert_eq!(
                pins, 1,
                "{workflow} job {name} must carry exactly one {PIN_STEP:?} step, found {pins}"
            );
        }
    }
}
