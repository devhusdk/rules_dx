use super::*;
use crate::proto::exec_log_entry::{Spawn, Type};
use crate::proto::{EnvironmentVariable, ExecLogEntry, Platform};
use crate::tests::{entry, file, invocation, log, set, spawn};

fn with(spawn: Spawn) -> ExecLogEntry {
    entry(0, Type::Spawn(spawn))
}

fn pair(old: Spawn, new: Spawn, old_hash: &str, new_hash: &str) -> Vec<CacheMiss> {
    let old_log = log(&[
        invocation("old"),
        file(1, "src/a.rs", old_hash),
        file(2, "out/a", "o"),
        set(3, &[1], &[]),
        with(old),
    ]);
    let new_log = log(&[
        invocation("new"),
        file(1, "src/a.rs", new_hash),
        file(2, "out/a", "o2"),
        set(3, &[1], &[]),
        with(new),
    ]);
    explain(&old_log, &new_log).expect("explain")
}

fn kinds(miss: &CacheMiss) -> Vec<(&str, Option<&str>, Option<&str>)> {
    miss.changes
        .iter()
        .map(|change| {
            (
                change.kind.as_str(),
                change.delta.map(Delta::as_str),
                change.subject.as_deref(),
            )
        })
        .collect()
}

#[test]
fn modified_input_is_explained() {
    let misses = pair(spawn(3, 2, "Rustc"), spawn(3, 2, "Rustc"), "h1", "h2");
    assert_eq!(misses.len(), 1);
    assert_eq!(misses[0].mnemonic, "Rustc");
    assert_eq!(misses[0].primary_output, "out/a");
    assert_eq!(
        kinds(&misses[0]),
        vec![("input", Some("modified"), Some("src/a.rs"))]
    );
}

#[test]
fn cache_hits_are_not_misses() {
    let mut hit = spawn(3, 2, "Rustc");
    hit.cache_hit = true;
    assert!(pair(spawn(3, 2, "Rustc"), hit, "h1", "h2").is_empty());
}

#[test]
fn added_and_removed_inputs() {
    let old_log = log(&[
        file(1, "a", "x"),
        file(2, "out", "o"),
        set(3, &[1], &[]),
        with(spawn(3, 2, "M")),
    ]);
    let new_log = log(&[
        file(1, "b", "x"),
        file(2, "out", "o"),
        set(3, &[1], &[]),
        with(spawn(3, 2, "M")),
    ]);
    let misses = explain(&old_log, &new_log).expect("explain");
    assert_eq!(
        kinds(&misses[0]),
        vec![
            ("input", Some("removed"), Some("a")),
            ("input", Some("added"), Some("b")),
        ]
    );
}

#[test]
fn env_changes_name_variables_but_never_values() {
    let mut old = spawn(3, 2, "M");
    old.env_vars = vec![
        EnvironmentVariable {
            name: "TOKEN".to_owned(),
            value: "old-secret".to_owned(),
        },
        EnvironmentVariable {
            name: "GONE".to_owned(),
            value: "gone-secret".to_owned(),
        },
    ];
    let mut new = spawn(3, 2, "M");
    new.env_vars = vec![
        EnvironmentVariable {
            name: "TOKEN".to_owned(),
            value: "new-secret".to_owned(),
        },
        EnvironmentVariable {
            name: "NEW".to_owned(),
            value: "new-value".to_owned(),
        },
    ];
    let misses = pair(old, new, "h", "h");
    assert_eq!(
        kinds(&misses[0]),
        vec![
            ("env", Some("removed"), Some("GONE")),
            ("env", Some("added"), Some("NEW")),
            ("env", Some("modified"), Some("TOKEN")),
        ]
    );
    let debug = format!("{misses:?}");
    for value in ["old-secret", "gone-secret", "new-secret", "new-value"] {
        assert!(!debug.contains(value), "{value} leaked");
    }
}

#[test]
fn args_diff_as_multiset_then_order() {
    let mut old = spawn(3, 2, "M");
    old.args = vec!["a".into(), "-x".into(), "-x".into(), "b".into()];
    let mut new = spawn(3, 2, "M");
    new.args = vec!["a".into(), "-x".into(), "c".into(), "b".into()];
    assert_eq!(
        kinds(&pair(old, new, "h", "h")[0]),
        vec![
            ("arg", Some("removed"), Some("-x")),
            ("arg", Some("added"), Some("c")),
        ]
    );
    let mut old = spawn(3, 2, "M");
    old.args = vec!["a".into(), "b".into()];
    let mut new = spawn(3, 2, "M");
    new.args = vec!["b".into(), "a".into()];
    assert_eq!(
        kinds(&pair(old, new, "h", "h")[0]),
        vec![("arg", Some("order_changed"), None)]
    );
}

#[test]
fn platform_changes_by_property_name() {
    let property = |name: &str, value: &str| crate::proto::platform::Property {
        name: name.to_owned(),
        value: value.to_owned(),
    };
    let mut old = spawn(3, 2, "M");
    old.platform = Some(Platform {
        properties: vec![property("OSFamily", "linux")],
    });
    let mut new = spawn(3, 2, "M");
    new.platform = Some(Platform {
        properties: vec![property("OSFamily", "macos")],
    });
    assert_eq!(
        kinds(&pair(old, new, "h", "h")[0]),
        vec![("platform", Some("modified"), Some("OSFamily"))]
    );
}

#[test]
fn unmatched_and_uncacheable_spawns() {
    let old_log = log(&[file(2, "other", "o"), with(spawn(0, 2, "Other"))]);
    let mut new = spawn(0, 2, "M");
    new.target_label = "//new:t".to_owned();
    new.cacheable = false;
    let new_log = log(&[file(2, "out", "o"), with(new)]);
    let misses = explain(&old_log, &new_log).expect("explain");
    assert_eq!(
        kinds(&misses[0]),
        vec![
            ("not_cacheable", None, None),
            ("not_in_old_log", None, None)
        ]
    );
}

#[test]
fn fallback_matches_target_and_mnemonic() {
    let old_log = log(&[file(1, "a", "x"), set(3, &[1], &[]), with(spawn(3, 0, "M"))]);
    let new_log = log(&[file(1, "a", "y"), set(3, &[1], &[]), with(spawn(3, 0, "M"))]);
    let misses = explain(&old_log, &new_log).expect("explain");
    assert_eq!(misses[0].primary_output, NO_OUTPUT);
    assert_eq!(
        kinds(&misses[0]),
        vec![("input", Some("modified"), Some("a"))]
    );
}

#[test]
fn changes_are_capped() {
    let mut old = spawn(3, 2, "M");
    let mut new = spawn(3, 2, "M");
    old.args = (0..30).map(|n| format!("old{n}")).collect();
    new.args = (0..5).map(|n| format!("new{n}")).collect();
    let misses = pair(old, new, "h", "h");
    assert_eq!(misses[0].changes.len(), CHANGE_CAP);
    assert_eq!(misses[0].changes_omitted, 15);
}

#[test]
fn misses_keep_log_order_across_segments() {
    let old_log = log(&[invocation("o")]);
    let new_log = log(&[
        invocation("n1"),
        file(2, "x", "o"),
        with(spawn(0, 2, "First")),
        invocation("n2"),
        file(2, "y", "o"),
        with(spawn(0, 2, "Second")),
    ]);
    let misses = explain(&old_log, &new_log).expect("explain");
    let order: Vec<&str> = misses.iter().map(|miss| miss.mnemonic.as_str()).collect();
    assert_eq!(order, vec!["First", "Second"]);
    assert_eq!(misses[1].primary_output, "y");
}
