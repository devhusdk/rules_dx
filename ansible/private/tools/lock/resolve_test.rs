//! The ansible lock step writes the golden lock with an empty `PATH`; the archives come from the fetcher.

use std::cell::RefCell;

use ansible_lock::{Ansible, parse_requirements};
use dx_lock::{Args, Error, Fetch, Outcome, run};

const MANIFEST: &str = "---
# Versions are exact.
collections:
  - name: dxtest.tiny
    version: 1.0.0
  - name: \"dxtest.other\"
    version: '2.0.0'
";

struct Archives(RefCell<Vec<String>>);

impl Fetch for Archives {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = if url.ends_with("/dxtest-tiny-1.0.0.tar.gz") {
            "TINY"
        } else if url.ends_with("/dxtest-other-2.0.0.tar.gz") {
            "OTHER"
        } else {
            return Err(Error::new(format!("unexpected download of {url}")));
        };
        Ok(dx_digest::sha256_hex(
            &std::fs::read(dx_testing::env_file(name)).map_err(|e| Error::new(e.to_string()))?,
        ))
    }
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("ansible").expect("ansible is registered");
    run(
        &Ansible::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Ansible,
        archives,
    )
}

#[test]
fn pinned_collections_write_the_golden_lock() {
    let hub = dx_families::hub("ansible").expect("ansible is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), MANIFEST).expect("manifest");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().len(), 2);
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join(hub.lock.as_deref().expect("lock")),
        Some("ansible/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn a_fresh_lock_downloads_nothing() {
    let hub = dx_families::hub("ansible").expect("ansible is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), MANIFEST).expect("manifest");
    assert_eq!(
        lock(dir.path(), &Archives(RefCell::default())),
        Ok(Outcome::Written)
    );
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Fresh));
    assert!(archives.0.borrow().is_empty());
}

#[test]
fn an_entry_that_is_not_pinned_is_refused() {
    assert!(parse_requirements("collections:\n  - name: dxtest.tiny\n").is_err());
    assert!(parse_requirements("collections:\n  - name: dxtest.tiny\n    version: '>=1.0'\n").is_err());
    assert!(parse_requirements("collections:\n  - name: Tiny\n    version: 1.0.0\n").is_err());
    assert!(parse_requirements("collections:\n  - name: dxtest.tiny\n    source: x\n").is_err());
    assert!(parse_requirements("collections: []\n").is_err());
    assert!(
        parse_requirements(
            "collections:\n  - name: a.b\n    version: 1\n  - name: a.b\n    version: 2\n"
        )
        .is_err()
    );
}
