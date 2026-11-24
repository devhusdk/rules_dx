use ansible_lock::{Ansible, parse_requirements};
use dx_lock::{Args, Error, Outcome, run};
use dx_lock_testing::FixtureFetch;

const MANIFEST: &str = "---
# Versions are exact.
collections:
  - name: dxtest.tiny
    version: 1.0.0
  - name: \"dxtest.other\"
    version: '2.0.0'
";

fn archives() -> FixtureFetch {
    FixtureFetch::new(&[
        ("/dxtest-tiny-1.0.0.tar.gz", "TINY"),
        ("/dxtest-other-2.0.0.tar.gz", "OTHER"),
    ])
}

fn lock(dir: &std::path::Path, archives: &FixtureFetch) -> Result<Outcome, Error> {
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
    let archives = archives();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.asked().len(), 2);
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
    assert_eq!(lock(dir.path(), &archives()), Ok(Outcome::Written));
    let archives = archives();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Fresh));
    assert!(archives.asked().is_empty());
}

#[test]
fn an_entry_that_is_not_pinned_is_refused() {
    assert!(parse_requirements("collections:\n  - name: dxtest.tiny\n").is_err());
    assert!(
        parse_requirements("collections:\n  - name: dxtest.tiny\n    version: '>=1.0'\n").is_err()
    );
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
