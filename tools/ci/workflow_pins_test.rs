use dx_testing::{read_runfiles, runfiles_root, sha256_file};

#[path = "workflow_rules/actions.rs"]
mod actions;
#[path = "workflow_rules/permissions.rs"]
mod permissions;

const CALLERS: [(&str, &str); 1] = [("examples/docs-ci/caller.yml", "reusable-docs.yml")];

const REUSABLE: [&str; 3] = [
    "reusable-consumer.yml",
    "reusable-docs.yml",
    "reusable-release.yml",
];

const SCRIPTS: [(&str, &[&str]); 2] = [
    (
        "tools/ci/input_validation/src/lib.rs",
        &["reusable-consumer.yml", "reusable-release.yml"],
    ),
    ("tools/ci/docs_scope/src/lib.rs", &["reusable-docs.yml"]),
];

const OWNER: &str = "devhusdk";

const REPO: &str = "rules_dx";

struct Uses<'a> {
    owner: &'a str,
    repo: &'a str,
    path: &'a str,
    commit: &'a str,
}

fn parse_uses(value: &str) -> Result<Uses<'_>, String> {
    let (target, commit) = value
        .split_once('@')
        .ok_or_else(|| format!("{value} has no @<commit>"))?;
    let mut parts = target.splitn(3, '/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts
        .next()
        .ok_or_else(|| format!("{value} has no repository"))?;
    let path = parts
        .next()
        .ok_or_else(|| format!("{value} has no workflow path"))?;
    Ok(Uses {
        owner,
        repo,
        path,
        commit,
    })
}

fn check_uses(value: &str, workflow: &str) -> Result<String, String> {
    let uses = parse_uses(value)?;
    if uses.owner != OWNER {
        return Err(format!("owner must be {OWNER}, found {}", uses.owner));
    }
    if uses.repo != REPO {
        return Err(format!("repository must be {REPO}, found {}", uses.repo));
    }
    let want = format!(".github/workflows/{workflow}");
    if uses.path != want {
        return Err(format!("workflow path must be {want}, found {}", uses.path));
    }
    let hex = uses
        .commit
        .chars()
        .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
    if uses.commit.len() != 40 || !hex {
        return Err(format!(
            "must pin a full lowercase commit SHA, found {}",
            uses.commit
        ));
    }
    Ok(uses.commit.to_owned())
}

fn pinned_commit(caller: &str, workflow: &str) -> String {
    let line = ci_workflow::jobs(caller)
        .into_iter()
        .find_map(|(_, job)| job["uses"].as_str().map(str::to_owned))
        .unwrap_or_else(|| panic!("{caller} has no job with uses:"));
    check_uses(&line, workflow).unwrap_or_else(|err| panic!("{caller}: {err}"))
}

#[test]
fn uses_check_rejects_a_missing_owner() {
    let sha = "78068c86f4cc5d5edf35f01315784f14196d01e4";
    let bad = format!("rules_dx/.github/workflows/reusable-docs.yml@{sha}");
    assert!(check_uses(&bad, "reusable-docs.yml").is_err());
    let good = format!("devhusdk/rules_dx/.github/workflows/reusable-docs.yml@{sha}");
    assert_eq!(check_uses(&good, "reusable-docs.yml").unwrap(), sha);
    let wrong_repo = format!("devhusdk/other/.github/workflows/reusable-docs.yml@{sha}");
    assert!(check_uses(&wrong_repo, "reusable-docs.yml").is_err());
    let wrong_path = format!("devhusdk/rules_dx/.github/workflows/other.yml@{sha}");
    assert!(check_uses(&wrong_path, "reusable-docs.yml").is_err());
    let short = "devhusdk/rules_dx/.github/workflows/reusable-docs.yml@main";
    assert!(check_uses(short, "reusable-docs.yml").is_err());
}

#[test]
fn every_starter_pins_its_reusable_workflow_at_a_full_commit() {
    for (caller, workflow) in CALLERS {
        let path = runfiles_root().join(".github/workflows").join(workflow);
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

#[test]
fn every_tools_ci_script_runs_from_its_bazel_target_and_is_never_inlined() {
    for (script, users) in SCRIPTS {
        let source = read_runfiles(script);
        let summary = source
            .lines()
            .next()
            .unwrap_or_default()
            .trim_start_matches("\"\"\"")
            .trim_start_matches("//! ");
        assert!(!summary.is_empty(), "{script} has no docstring summary");
        let name = script
            .trim_start_matches("tools/ci/")
            .trim_end_matches("/src/lib.rs");
        let target = format!("@rules_dx//tools/ci/{name}:{name}_bin");
        for workflow in REUSABLE {
            let strings = ci_workflow::strings(&format!(".github/workflows/{workflow}"));
            assert!(
                !strings.iter().any(|text| text.contains(summary)),
                "{workflow} carries an inline copy of {script}"
            );
            assert_eq!(
                strings.iter().any(|text| text.contains(&target)),
                users.contains(&workflow),
                "{workflow} must run {target} only where it uses {script}"
            );
        }
    }
}

const DIGEST_RECORD: &str = "tools/ci/starter_pin.sha256";

/// Returns the recorded (path, digest) pairs, panicking on a malformed record.
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
    recorded.sort();
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

/// Returns each `//` label in `text` that names no repository, with its 1-based line.
fn bare_main_repo_labels(text: &str) -> Vec<(usize, String)> {
    let repo_char = |c: char| c.is_ascii_alphanumeric() || "_.+-".contains(c);
    let label_char = |c: char| c.is_ascii_alphanumeric() || "_.+@=,/:-".contains(c);
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for (start, _) in line.match_indices("//") {
            let after = line[start + 2..].chars().next();
            if !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':') {
                continue;
            }
            let before = line[..start].trim_end_matches(repo_char);
            if before.ends_with('@')
                || line[..start].ends_with(':')
                || line[..start].trim_end().ends_with("cmd.exe")
            {
                continue;
            }
            let end = line[start..]
                .find(|c: char| !label_char(c))
                .map_or(line.len(), |offset| start + offset);
            found.push((index + 1, line[start..end].to_owned()));
        }
    }
    found
}

#[test]
fn bare_main_repo_label_check_flags_only_unqualified_labels() {
    for good in [
        "run: bazel run @rules_dx//tools/ci/input_validation:input_validation_bin",
        "run: bazel run @dx_tools//:dx",
        "run: bazel run @@//:dx",
        "url: https://example.com/a",
        "scope: \"//\"",
        "default: \"//...\"",
        "# a plain // comment",
        "cmd.exe //c tools\\dx.cmd completion",
    ] {
        assert_eq!(bare_main_repo_labels(good), vec![], "{good}");
    }
    for (bad, label) in [
        ("bazel run //tools/x", "//tools/x"),
        ("bazel run //:dx -- docs", "//:dx"),
        ("site: \"//docs:site\"", "//docs:site"),
    ] {
        assert_eq!(
            bare_main_repo_labels(bad),
            vec![(1, label.to_owned())],
            "{bad}"
        );
    }
}

#[test]
fn reusable_workflows_name_no_main_repo_label() {
    for workflow in REUSABLE {
        for text in ci_workflow::strings(&format!(".github/workflows/{workflow}")) {
            if let Some((_, label)) = bare_main_repo_labels(&text).into_iter().next() {
                panic!(
                    "{workflow} names {label}, which a caller's repository does not have; qualify it with @rules_dx or take it as an input"
                );
            }
        }
    }
}
