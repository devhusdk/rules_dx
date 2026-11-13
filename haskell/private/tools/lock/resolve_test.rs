//! Tests the cabal lock step against a plan and a Hackage index fixture.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cabal_lock::{
    Revisions, StackLock, StackPackage, check_stack_pins, fetch_packages, freeze_text, index_state,
    packages, plan_pins, remote_packages, remote_tarballs, revisions, stack_lock, stack_pins,
};
use dx_lock::{Archive, Error, Lock, Package, SCHEMA};

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
    assert_eq!(lock.shape_problems(), Vec::<String>::new());
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

const PROJECT: &str = "packages: https://example.com/a-1.tar.gz\n  ./b\n  https://example.com/c-2.tar.gz\nindex-state: 2026-10-01T00:00:00Z\nsource: https://example.com/d.tar.gz\n";

fn download(url: &str) -> Result<Vec<u8>, Error> {
    let name = url
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".tar.gz"))
        .expect("name");
    let mut header = tar::Header::new_gnu();
    header.set_size(url.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    builder
        .append_data(&mut header, format!("{name}/{name}.cabal"), url.as_bytes())
        .expect("append");
    Ok(builder
        .into_inner()
        .and_then(flate2::write::GzEncoder::finish)
        .expect("tar.gz"))
}

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR")).join(name)
}

#[test]
fn the_remote_packages_are_the_urls_of_the_packages_field() {
    assert_eq!(
        remote_packages(PROJECT),
        [
            "https://example.com/a-1.tar.gz",
            "https://example.com/c-2.tar.gz"
        ]
    );
    assert!(remote_packages("packages: .\n").is_empty());
}

#[test]
fn fetched_packages_are_local_directories_in_the_project_and_urls_in_the_plan() {
    let dir = scratch("fetched");
    let (project, fetched) = fetch_packages(PROJECT, &dir, None, &download).expect("fetch");
    let a = dir.join("0").join("a-1");
    let c = dir.join("1").join("c-2");
    assert_eq!(
        project,
        PROJECT
            .replace("https://example.com/a-1.tar.gz", &a.to_string_lossy())
            .replace("https://example.com/c-2.tar.gz", &c.to_string_lossy())
    );
    assert_eq!(
        std::fs::read(a.join("a-1.cabal")).expect("unpacked"),
        b"https://example.com/a-1.tar.gz"
    );
    let mut plan = serde_json::json!({"install-plan": [
        {"type": "configured", "pkg-name": "a", "pkg-version": "1", "component-name": "lib",
         "pkg-src": {"type": "local", "path": format!("{}/", a.display())}},
        {"type": "configured", "pkg-name": "b", "pkg-version": "1", "component-name": "lib",
         "pkg-src": {"type": "local", "path": "./b"}},
    ]});
    remote_tarballs(&mut plan, &fetched);
    let found = packages(&plan, &Revisions::new()).expect("packages");
    assert_eq!(found.len(), 1);
    let archive = found.first().and_then(|package| package.archives.first());
    assert_eq!(
        archive.map(|archive| (archive.urls.clone(), archive.sha256.clone())),
        Some((
            vec!["https://example.com/a-1.tar.gz".to_owned()],
            dx_digest::sha256_hex(&download("https://example.com/a-1.tar.gz").expect("a"))
        ))
    );
}

#[test]
fn a_tarball_that_differs_from_the_lock_fails() {
    let old = Lock {
        schema: SCHEMA,
        ecosystem: "cabal".to_owned(),
        inputs: BTreeMap::new(),
        packages: vec![Package {
            name: "a".to_owned(),
            version: "1".to_owned(),
            archives: vec![Archive {
                urls: vec!["https://example.com/a-1.tar.gz".to_owned()],
                sha256: "0".repeat(64),
                ..Archive::default()
            }],
            ..Package::default()
        }],
    };
    let error = fetch_packages(PROJECT, &scratch("differs"), Some(&old), &download)
        .expect_err("sha256 mismatch");
    assert!(error.to_string().contains("a-1.tar.gz"), "{error}");
}

#[test]
fn an_http_package_is_not_fetched() {
    let project = "packages: http://example.com/a-1.tar.gz\nindex-state: 2026-10-01T00:00:00Z\n";
    assert!(remote_packages(project).is_empty());
    let error = fetch_packages(project, &scratch("http"), None, &|url| {
        panic!("fetched {url}")
    })
    .expect_err("http URL");
    assert!(
        error.to_string().contains("http://example.com/a-1.tar.gz"),
        "{error}"
    );
}

#[test]
fn only_the_packages_field_is_localized() {
    let url = "https://example.com/a-1.tar.gz";
    let project = format!(
        "packages: {url}\nindex-state: 2026-10-01T00:00:00Z\nsource-repository-package\n  type: git\n  location: {url}\n  subdir: {url}/sub\n"
    );
    let dir = scratch("fields");
    let (localized, _) = fetch_packages(&project, &dir, None, &download).expect("fetch");
    let a = dir.join("0").join("a-1");
    assert_eq!(
        localized,
        format!(
            "packages: {}\nindex-state: 2026-10-01T00:00:00Z\nsource-repository-package\n  type: git\n  location: {url}\n  subdir: {url}/sub\n",
            a.display()
        )
    );
}

const SNAPSHOT: &[u8] = b"packages:\n- hackage: safe-0.3.21@sha256:aaaa,1\n  pantry-tree:\n    sha256: bbbb\n    size: 2\n- hackage: split-0.2.4@sha256:cccc,3\n";

fn stack_text(snapshot_sha256: &str, cabal_sha256: &str) -> String {
    format!(
        "# comment\n\npackages:\n- completed:\n    hackage: split-0.2.5@sha256:{cabal_sha256},2625\n    pantry-tree:\n      sha256: {snapshot_sha256}\n      size: 443\n  original:\n    hackage: split-0.2.5\nsnapshots:\n- completed:\n    sha256: {snapshot_sha256}\n    size: 726103\n    url: https://example.invalid/lts.yaml\n  original: lts-24.15\n"
    )
}

#[test]
fn a_stack_lock_gives_its_snapshot_and_extra_dep_pins() {
    let digest = dx_digest::sha256_hex(SNAPSHOT);
    let cabal = dx_digest::sha256_hex(SPLITMIX);
    let lock = stack_lock(&stack_text(&digest, &cabal)).expect("stack lock");
    assert_eq!(
        lock.snapshots,
        vec![("https://example.invalid/lts.yaml".to_owned(), digest)]
    );
    assert_eq!(
        lock.packages,
        vec![StackPackage {
            name: "split".to_owned(),
            version: "0.2.5".to_owned(),
            cabal_sha256: Some(cabal),
        }]
    );
    let pins = stack_pins(&lock, &|url| {
        assert_eq!(url, "https://example.invalid/lts.yaml");
        Ok(SNAPSHOT.to_vec())
    })
    .expect("pins");
    assert_eq!(
        freeze_text(&pins, Some("2026-10-01T00:00:00Z")),
        "constraints: any.safe ==0.3.21,\n             any.split ==0.2.5\nindex-state: hackage.haskell.org 2026-10-01T00:00:00Z\n"
    );
}

#[test]
fn a_snapshot_of_another_sha256_fails() {
    let lock = stack_lock(&stack_text(&"0".repeat(64), &"1".repeat(64))).expect("stack lock");
    let error = stack_pins(&lock, &|_| Ok(SNAPSHOT.to_vec())).expect_err("mismatch");
    assert!(error.to_string().contains("stack.yaml.lock has"), "{error}");
}

#[test]
fn a_stack_lock_without_sha256_pins_fails() {
    let text = stack_text("feed", &"1".repeat(64));
    assert!(stack_lock(&text).is_err());
    let text = stack_text(&"0".repeat(64), "abc");
    assert!(stack_lock(&text).is_err());
    assert!(stack_lock("packages: []\nsnapshots: []\n").is_err());
}

#[test]
fn an_extra_dep_of_another_cabal_sha256_fails() {
    let plan = plan();
    let pins = plan_pins(&plan);
    assert_eq!(pins.get("splitmix").map(String::as_str), Some("0.1.1"));
    let pinned = |cabal: &str| StackLock {
        snapshots: Vec::new(),
        packages: vec![StackPackage {
            name: "splitmix".to_owned(),
            version: "0.1.1".to_owned(),
            cabal_sha256: Some(cabal.to_owned()),
        }],
    };
    check_stack_pins(&plan, &pinned(&dx_digest::sha256_hex(SPLITMIX))).expect("same sha256");
    let error = check_stack_pins(&plan, &pinned(&"2".repeat(64))).expect_err("mismatch");
    assert!(error.to_string().contains("splitmix-0.1.1"), "{error}");
}
