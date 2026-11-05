//! Checks that every `starter_consumer_test` has a golden listing and every golden has a test.

use std::collections::BTreeSet;

fn names(var: &str) -> BTreeSet<String> {
    std::env::var(var)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

#[test]
fn starters_and_goldens_match() {
    let declared = names("DX_DECLARED");
    let goldens = names("DX_GOLDENS");
    assert!(!declared.is_empty(), "no starter_consumer_test is declared");
    let missing: Vec<&String> = declared.difference(&goldens).collect();
    let extra: Vec<&String> = goldens.difference(&declared).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "starters without a golden: {missing:?}; goldens without a starter: {extra:?}"
    );
}
