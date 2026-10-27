//! Checks the pinned Maven lock that every nested consumer copies into its workspace.

use dx_nested::{CONSUMER_MAVEN, CONSUMER_MAVEN_LOCK_ENV};
use dx_testing::{read, rlocation};
use serde_json::Value;

const PROTOBUF_ARTIFACTS: &[&str] = &[
    "com.google.code.findbugs:jsr305",
    "com.google.code.gson:gson",
    "com.google.errorprone:error_prone_annotations",
    "com.google.guava:guava",
    "com.google.j2objc:j2objc-annotations",
];

fn lock() -> Value {
    let key = std::env::var(CONSUMER_MAVEN_LOCK_ENV).expect("lock env var");
    let text = read(&rlocation(&key).expect("rlocation"));
    serde_json::from_str(&text).expect("lock is JSON")
}

#[test]
fn lock_is_v3() {
    assert_eq!(lock()["version"], "3");
}

#[test]
fn input_hash_covers_exactly_the_consumer_artifacts() {
    let lock = lock();
    let hash = lock["__INPUT_ARTIFACTS_HASH"]
        .as_object()
        .expect("hash map");
    let mut keys: Vec<&str> = hash.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut want: Vec<&str> = PROTOBUF_ARTIFACTS.to_vec();
    want.push("repositories");
    want.sort_unstable();
    assert_eq!(keys, want);
}

#[test]
fn guava_is_pinned_with_checksums() {
    let lock = lock();
    let artifacts = lock["artifacts"].as_object().expect("artifacts");
    assert_eq!(artifacts["com.google.guava:guava"]["version"], "32.0.1-jre");
    for (name, entry) in artifacts {
        assert!(
            entry["shasums"]["jar"].is_string(),
            "{name} has no jar shasum"
        );
    }
}

#[test]
fn block_pins_the_lock() {
    assert!(CONSUMER_MAVEN.contains("lock_file = \"//:maven_install.json\""));
    assert!(CONSUMER_MAVEN.contains("fail_if_repin_required = True"));
}
