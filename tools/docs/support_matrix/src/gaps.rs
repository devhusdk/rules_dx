//! Lists the missing tests the matrix shows and matches them to tracking issues.

use std::collections::BTreeMap;

use crate::{Facts, RowPlatforms};

const CAPABILITIES: [&str; 3] = ["lint", "format", "typecheck"];

/// Returns the quality tools of `capability` across `families`, sorted and unique.
pub fn tools(facts: &Facts, families: &[String], capability: &str) -> Vec<String> {
    let mut out: Vec<String> = families
        .iter()
        .filter_map(|family| facts.quality.get(family))
        .filter_map(|caps| caps.get(capability))
        .flatten()
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Returns every gap id, sorted. Ids look like `consumer:rust:windows_arm64`.
pub fn gaps(facts: &Facts, platforms: &BTreeMap<String, RowPlatforms>) -> Vec<String> {
    let mut out = Vec::new();
    for row in &facts.rows {
        if row.section != "formats" {
            let tested = platforms.get(&row.name).cloned().unwrap_or_default();
            for platform in &facts.platforms {
                if !tested.build.contains(&platform.id) {
                    out.push(format!("build:{}:{}", row.name, platform.id));
                }
                if !tested.consumer.contains(&platform.id) {
                    out.push(format!("consumer:{}:{}", row.name, platform.id));
                }
            }
        }
        if row.section == "languages" && row.starters.is_empty() {
            out.push(format!("starter:{}", row.name));
        }
        if !crate::render::shows_quality(row) {
            continue;
        }
        for capability in CAPABILITIES {
            if tools(facts, &row.families, capability).is_empty() {
                out.push(format!("{capability}:{}", row.name));
            }
        }
        for record in &facts.support {
            if !row.families.contains(&record.family) {
                continue;
            }
            if record.platforms.is_empty() {
                continue;
            }
            for platform in &facts.platforms {
                if !record.platforms.contains(&platform.id) {
                    out.push(format!(
                        "support:{}:{}:{}:{}",
                        row.name, record.tool, record.capability, platform.id
                    ));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Reports whether `text` matches `pattern`, where `*` matches any run of characters.
pub fn matches(pattern: &str, text: &str) -> bool {
    let Some((head, rest)) = pattern.split_once('*') else {
        return pattern == text;
    };
    let Some(tail) = text.strip_prefix(head) else {
        return false;
    };
    (0..=tail.len())
        .filter(|index| tail.is_char_boundary(*index))
        .any(|index| matches(rest, &tail[index..]))
}

/// Returns the issue tracking `gap`, if any pattern matches it.
pub fn issue_for(gap: &str, ledger: &BTreeMap<String, u64>) -> Option<u64> {
    ledger
        .iter()
        .find(|(pattern, _)| matches(pattern, gap))
        .map(|(_, issue)| *issue)
}

/// Fails when a gap has no issue or a ledger pattern matches no gap.
pub fn check_ledger(gaps: &[String], ledger: &BTreeMap<String, u64>) -> Result<(), String> {
    let untracked: Vec<&String> = gaps
        .iter()
        .filter(|gap| issue_for(gap, ledger).is_none())
        .collect();
    let stale: Vec<&String> = ledger
        .keys()
        .filter(|pattern| !gaps.iter().any(|gap| matches(pattern, gap)))
        .collect();
    if untracked.is_empty() && stale.is_empty() {
        return Ok(());
    }
    Err(format!(
        "tools/docs/support_matrix/languages.bzl GAP_ISSUES drift: \
         gaps without an issue {untracked:?}; patterns that match no gap {stale:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_globs() {
        assert!(matches(
            "consumer:*:windows_*",
            "consumer:rust:windows_arm64"
        ));
        assert!(matches("starter:julia", "starter:julia"));
        assert!(!matches(
            "consumer:*:windows_*",
            "consumer:rust:linux_x86_64"
        ));
        assert!(matches("*", "anything"));
    }

    #[test]
    fn ledger_is_bidirectional() {
        let ledger = BTreeMap::from([("starter:*".to_owned(), 1), ("build:*".to_owned(), 2)]);
        let gaps = vec!["starter:julia".to_owned()];
        let error = check_ledger(&gaps, &ledger).expect_err("build:* is stale");
        assert!(error.contains("build:*"), "{error}");
        let gaps = vec!["starter:julia".to_owned(), "lint:d".to_owned()];
        let ledger = BTreeMap::from([("starter:*".to_owned(), 1)]);
        let error = check_ledger(&gaps, &ledger).expect_err("lint:d is untracked");
        assert!(error.contains("lint:d"), "{error}");
        let ledger = BTreeMap::from([("*".to_owned(), 1)]);
        assert_eq!(check_ledger(&gaps, &ledger), Ok(()));
    }
}
