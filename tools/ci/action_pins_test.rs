use std::collections::BTreeMap;

use dx_testing::read_runfiles;

const GITHUB_BUILD: &str = ".github/BUILD.bazel";

struct ActionPin {
    workflow: String,
    line: usize,
    action: String,
    sha: String,
    tag: String,
}

fn workflow_exports() -> Vec<String> {
    dx_testing::workflow_exports(&read_runfiles(GITHUB_BUILD), GITHUB_BUILD)
}

fn uses_lines(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("- uses:")
            .or_else(|| trimmed.strip_prefix("uses:"))
        else {
            continue;
        };
        found.push((index + 1, rest.trim().to_owned()));
    }
    found
}

fn action_pins() -> Vec<ActionPin> {
    let mut pins = Vec::new();
    for name in workflow_exports() {
        let text = read_runfiles(&format!(".github/{name}"));
        for (line, uses) in uses_lines(&text) {
            if uses.starts_with("./") || uses.starts_with("docker://") {
                continue;
            }
            let (action, pin) = uses
                .split_once('@')
                .unwrap_or_else(|| panic!(".github/{name}:{line}: {uses} pins no ref"));
            let (sha, comment) = pin.split_once(char::is_whitespace).unwrap_or((pin, ""));
            let tag = comment.trim().trim_start_matches('#').trim();
            pins.push(ActionPin {
                workflow: name.clone(),
                line,
                action: action.to_owned(),
                sha: sha.to_owned(),
                tag: tag.to_owned(),
            });
        }
    }
    assert!(!pins.is_empty(), "no workflow pins a third-party action");
    pins
}

fn is_commit_sha(text: &str) -> bool {
    text.len() == 40
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn pinned_sha(site: &str) -> &str {
    site.rsplit_once(" = ")
        .map(|(_, sha)| sha)
        .unwrap_or_default()
}

#[test]
fn every_action_pin_is_a_commit_sha_with_its_tag() {
    for pin in action_pins() {
        let at = format!(".github/{}:{}", pin.workflow, pin.line);
        assert!(
            is_commit_sha(&pin.sha),
            "{at} pins {} at {:?}; a 40-char lowercase commit SHA is the only accepted ref",
            pin.action,
            pin.sha
        );
        assert!(
            !pin.tag.is_empty(),
            "{at} pins {} at a commit SHA with no trailing tag comment",
            pin.action
        );
    }
}

#[test]
fn each_action_pins_one_commit_tree_wide() {
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for pin in action_pins() {
        owners
            .entry(pin.action.clone())
            .or_default()
            .push(format!("{}:{} = {}", pin.workflow, pin.line, pin.sha));
    }
    for (action, sites) in owners {
        let first = &sites[0];
        for site in sites.iter().skip(1) {
            assert_eq!(
                pinned_sha(site),
                pinned_sha(first),
                "{action} is pinned at {site} but at {first}; one action, one commit"
            );
        }
    }
}
