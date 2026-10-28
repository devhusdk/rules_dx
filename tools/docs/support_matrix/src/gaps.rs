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

/// Reports whether `capability` never applies to a row: it has no tools and
/// every one of its families is declared not applicable.
pub fn not_applicable(facts: &Facts, families: &[String], capability: &str) -> bool {
    let Some(declared) = facts.not_applicable.get(capability) else {
        return false;
    };
    !families.is_empty()
        && tools(facts, families, capability).is_empty()
        && families.iter().all(|family| declared.contains_key(family))
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
        if !crate::render::shows_quality(row) {
            continue;
        }
        for capability in CAPABILITIES {
            if tools(facts, &row.families, capability).is_empty()
                && !not_applicable(facts, &row.families, capability)
            {
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

/// Counts the literal (non-`*`) characters of `pattern`.
fn specificity(pattern: &str) -> usize {
    pattern.chars().filter(|c| *c != '*').count()
}

/// Returns the issue tracking `gap`: the matching pattern with the most literal characters.
pub fn issue_for(gap: &str, ledger: &BTreeMap<String, u64>) -> Option<u64> {
    ledger
        .iter()
        .filter(|(pattern, _)| matches(pattern, gap))
        .max_by(|(a, _), (b, _)| specificity(a).cmp(&specificity(b)).then_with(|| b.cmp(a)))
        .map(|(_, issue)| *issue)
}

/// Returns the pairs of patterns that match `gap` with equal specificity.
fn ambiguous(gap: &str, ledger: &BTreeMap<String, u64>) -> Option<(String, String)> {
    let hits: Vec<&String> = ledger.keys().filter(|p| matches(p, gap)).collect();
    let top = hits.iter().map(|p| specificity(p)).max()?;
    let best: Vec<&&String> = hits.iter().filter(|p| specificity(p) == top).collect();
    match best.as_slice() {
        [a, b, ..] => Some(((**a).clone(), (**b).clone())),
        _ => None,
    }
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
    for gap in gaps {
        if let Some((a, b)) = ambiguous(gap, ledger) {
            return Err(format!(
                "tools/docs/support_matrix/languages.bzl GAP_ISSUES: patterns {a:?} and {b:?} \
                 both match {gap:?} with equal specificity"
            ));
        }
    }
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
        assert!(matches("consumer:julia", "consumer:julia"));
        assert!(!matches(
            "consumer:*:windows_*",
            "consumer:rust:linux_x86_64"
        ));
        assert!(matches("*", "anything"));
    }

    #[test]
    fn issue_for_prefers_most_specific() {
        let ledger = BTreeMap::from([("build:*".to_owned(), 1), ("build:cuda:*".to_owned(), 2)]);
        assert_eq!(issue_for("build:cuda:macos_arm64", &ledger), Some(2));
        assert_eq!(issue_for("build:d:macos_arm64", &ledger), Some(1));
    }

    #[test]
    fn check_ledger_rejects_ambiguous_overlap() {
        let ledger = BTreeMap::from([
            ("build:cuda:m*".to_owned(), 1),
            ("build:*:macos".to_owned(), 2),
        ]);
        let gaps = vec!["build:cuda:macos".to_owned()];
        let error = check_ledger(&gaps, &ledger).expect_err("tie");
        assert!(
            error.contains("build:cuda:m*") && error.contains("build:*:macos"),
            "{error}"
        );
    }

    #[test]
    fn narrow_pattern_goes_stale_when_its_gaps_close() {
        let ledger = BTreeMap::from([("build:*".to_owned(), 1), ("build:cuda:*".to_owned(), 2)]);
        let gaps = vec!["build:d:macos_arm64".to_owned()];
        let error = check_ledger(&gaps, &ledger).expect_err("stale");
        assert!(error.contains("build:cuda:*"), "{error}");
    }

    #[test]
    fn ledger_is_bidirectional() {
        let ledger = BTreeMap::from([("consumer:*".to_owned(), 1), ("build:*".to_owned(), 2)]);
        let gaps = vec!["consumer:julia".to_owned()];
        let error = check_ledger(&gaps, &ledger).expect_err("build:* is stale");
        assert!(error.contains("build:*"), "{error}");
        let gaps = vec!["consumer:julia".to_owned(), "lint:d".to_owned()];
        let ledger = BTreeMap::from([("consumer:*".to_owned(), 1)]);
        let error = check_ledger(&gaps, &ledger).expect_err("lint:d is untracked");
        assert!(error.contains("lint:d"), "{error}");
        let ledger = BTreeMap::from([("*".to_owned(), 1)]);
        assert_eq!(check_ledger(&gaps, &ledger), Ok(()));
    }

    #[test]
    fn per_language_lint_entries_leave_new_lint_gaps_untracked() {
        let ledger = BTreeMap::from([("lint:lua".to_owned(), 1)]);
        let gaps = vec!["lint:lua".to_owned(), "lint:newlang".to_owned()];
        let error = check_ledger(&gaps, &ledger).expect_err("lint:newlang is untracked");
        assert!(error.contains("lint:newlang"), "{error}");
        assert!(!error.contains("\"lint:lua\""), "{error}");
    }

    fn facts_with(not_applicable: &str, quality: &str) -> Facts {
        crate::parse_facts(&format!(
            r#"{{"schema_version":1,"platforms":[],"sections":[],"rows":[],
            "quality":{quality},"support":[],"deferred_families":[],
            "not_applicable":{not_applicable},"not_languages":[],
            "gap_issues":{{}},"gap_enforcement":false,"computed_constraints":{{}}}}"#
        ))
        .expect("facts")
    }

    #[test]
    fn not_applicable_needs_every_family_and_no_tools() {
        let na = r#"{"typecheck":{"a":"why"}}"#;
        let quality = r#"{"a":{"typecheck":[]},"b":{"typecheck":[]},"c":{"typecheck":["t"]}}"#;
        let facts = facts_with(na, quality);
        let fam = |names: &[&str]| names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
        assert!(not_applicable(&facts, &fam(&["a"]), "typecheck"));
        assert!(!not_applicable(&facts, &fam(&["a", "b"]), "typecheck"));
        assert!(!not_applicable(&facts, &fam(&["a"]), "lint"));
        let facts = facts_with(r#"{"typecheck":{"c":"why"}}"#, quality);
        assert!(!not_applicable(&facts, &fam(&["c"]), "typecheck"));
    }

    #[test]
    fn format_declaration_suppresses_format_gap_only() {
        let facts = facts_with(
            r#"{"format":{"a":"why"}}"#,
            r#"{"a":{"format":[],"lint":["t"]}}"#,
        );
        let fam = vec!["a".to_owned()];
        assert!(not_applicable(&facts, &fam, "format"));
        assert!(!not_applicable(&facts, &fam, "lint"));
    }
}
