use dx_testing::runfiles_root;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

const EXTENSION: &str = "@@rules_go+//go:extensions.bzl%go_sdk";

const PLATFORMS: [(&str, &str); 5] = [
    ("linux_x86_64", "os:linux,arch:amd64"),
    ("linux_arm64", "os:linux,arch:aarch64"),
    ("macos_arm64", "os:osx,arch:aarch64"),
    ("windows_x86_64", "os:windows,arch:amd64"),
    ("windows_arm64", "os:windows,arch:aarch64"),
];

fn entry(digest: &str, repos: &[String]) -> Value {
    let specs: serde_json::Map<String, Value> = repos
        .iter()
        .map(|repo| (repo.clone(), json!({ "repoRuleId": repo })))
        .collect();
    json!({
        "bzlTransitiveDigest": "bzl",
        "usagesDigest": digest,
        "generatedRepoSpecs": specs,
    })
}

fn fresh_repos(factor: &str) -> Vec<String> {
    vec![format!("fresh_{factor}"), "shared".to_owned()]
}

fn stale_repos(factor: &str) -> Vec<String> {
    vec![format!("stale_{factor}"), "shared".to_owned(), "dropped".to_owned()]
}

/// A lock as one platform job writes it: fresh for its own factor, stale for every other.
fn platform_lock(own: &str, fact: &str) -> Value {
    let mut entries = serde_json::Map::new();
    for (_, factor) in PLATFORMS {
        let value = if factor == own {
            entry(&format!("fresh {factor}"), &fresh_repos(factor))
        } else {
            entry("stale", &stale_repos(factor))
        };
        entries.insert(factor.to_owned(), value);
    }
    json!({
        "lockFileVersion": 26,
        "moduleExtensions": {
            EXTENSION: entries,
            "//general:ext.bzl%ext": { "general": entry("general", &["g".to_owned()]) },
        },
        "facts": { fact: { "value": own } },
    })
}

fn write_locks(dir: &Path, locks: &[(&str, Value)]) {
    for (name, lock) in locks {
        let platform = dir.join(name);
        std::fs::create_dir_all(&platform).expect("create the platform directory");
        std::fs::write(
            platform.join("MODULE.bazel.lock"),
            serde_json::to_string_pretty(lock).expect("serialize the lock"),
        )
        .expect("write the lock");
    }
}

fn fresh_locks() -> Vec<(String, Value)> {
    PLATFORMS
        .iter()
        .map(|(platform, factor)| (format!("lock-{platform}"), platform_lock(factor, platform)))
        .collect()
}

/// Runs `producers_bin --lock-merge` and returns the merged lock, or None when it fails.
fn merge(locks: &[(String, Value)]) -> Option<Value> {
    let dir = dx_test_scratch::scratch("lock-merge");
    let inputs = dir.path().join("locks");
    let borrowed: Vec<(&str, Value)> = locks
        .iter()
        .map(|(name, lock)| (name.as_str(), lock.clone()))
        .collect();
    write_locks(&inputs, &borrowed);
    let out = dir.path().join("MODULE.bazel.lock");
    let bin = runfiles_root().join(std::env::var("DX_PRODUCERS_BIN").expect("DX_PRODUCERS_BIN"));
    let output = Command::new(bin)
        .arg("--lock-merge")
        .arg(&inputs)
        .arg(&out)
        .output()
        .expect("run producers_bin --lock-merge");
    if !output.status.success() {
        return None;
    }
    Some(
        serde_json::from_str(&std::fs::read_to_string(&out).expect("read the merged lock"))
            .expect("the merged lock is JSON"),
    )
}

#[test]
fn every_factor_takes_its_own_platform_entry_whole() {
    let merged = merge(&fresh_locks()).expect("lock-merge merges five fresh locks");
    let entries = &merged["moduleExtensions"][EXTENSION];
    for (_, factor) in PLATFORMS {
        assert_eq!(
            entries[factor],
            entry(&format!("fresh {factor}"), &fresh_repos(factor)),
            "{factor}"
        );
    }
    assert_eq!(
        entries.as_object().expect("the extension is an object").len(),
        PLATFORMS.len()
    );
    assert_eq!(
        merged["moduleExtensions"]["//general:ext.bzl%ext"]["general"],
        entry("general", &["g".to_owned()])
    );
    for (platform, factor) in PLATFORMS {
        assert_eq!(merged["facts"][platform]["value"], factor, "{platform}");
    }
}

#[test]
fn an_unknown_extension_key_fails() {
    let mut locks = fresh_locks();
    locks[2].1["moduleExtensions"][EXTENSION]["os:osx"] = entry("os only", &[]);
    assert_eq!(merge(&locks), None);
}

#[test]
fn a_missing_platform_fails() {
    let mut locks = fresh_locks();
    locks.remove(4);
    assert_eq!(merge(&locks), None);
}

#[test]
fn an_unknown_platform_directory_fails() {
    let mut locks = fresh_locks();
    locks.push((
        "lock-freebsd_x86_64".to_owned(),
        platform_lock("os:freebsd,arch:amd64", "freebsd_x86_64"),
    ));
    assert_eq!(merge(&locks), None);
}
