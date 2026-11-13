//! Tests the cabal lock step against a plan and a Hackage index fixture.

use std::collections::BTreeMap;

use cabal_lock::{Revisions, index_state, packages, revisions};
use dx_lock::{Lock, SCHEMA};

const SPLITMIX: &[u8] = b"name: splitmix\nversion: 0.1.1\n";
const RANDOM_0: &[u8] = b"name: random\nversion: 1.2.1\n";
const RANDOM_1: &[u8] = b"name: random\nversion: 1.2.1\nx-revision: 1\n";

fn index() -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, bytes) in [
        ("splitmix/0.1.1/splitmix.cabal", SPLITMIX),
        ("random/1.2.1/random.cabal", RANDOM_0),
        ("other/1.0/other.cabal", SPLITMIX),
        ("random/1.2.1/random.cabal", RANDOM_1),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, path, bytes)
            .expect("append");
    }
    builder.into_inner().expect("tar")
}

fn plan() -> serde_json::Value {
    let text = std::fs::read_to_string(std::env::var("PLAN").expect("PLAN")).expect("plan");
    let text = text
        .replace("SPLITMIX_CABAL", &dx_digest::sha256_hex(SPLITMIX))
        .replace("RANDOM_CABAL", &dx_digest::sha256_hex(RANDOM_1));
    serde_json::from_str(&text).expect("json")
}

#[test]
fn a_plan_and_an_index_give_the_golden_lock() {
    let wanted = [("random", "1.2.1"), ("splitmix", "0.1.1")]
        .into_iter()
        .map(|(name, version)| (name.to_owned(), version.to_owned()))
        .collect();
    let found = revisions(index().as_slice(), &wanted).expect("revisions");
    let lock = Lock {
        schema: SCHEMA,
        ecosystem: "cabal".to_owned(),
        inputs: BTreeMap::new(),
        packages: packages(&plan(), &found).expect("packages"),
    };
    let golden = std::fs::read_to_string(std::env::var("GOLDEN").expect("GOLDEN")).expect("golden");
    assert_eq!(lock.to_bytes(), golden);
}

#[test]
fn a_hackage_package_without_a_revision_fails() {
    let error = packages(&plan(), &Revisions::new()).expect_err("no revision");
    assert!(error.to_string().contains("random-1.2.1"), "{error}");
}

#[test]
fn the_index_state_comes_from_the_project() {
    let project = "packages: .\nindex-state: 2026-10-01T00:00:00Z\n";
    assert_eq!(index_state(project).expect("state"), "2026-10-01T00:00:00Z");
    assert!(index_state("packages: .\n").is_err());
}
