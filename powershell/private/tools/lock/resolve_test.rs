//! The powershell lock step writes the golden lock with an empty `PATH`; the archives come from the fetcher.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use powershell_lock::{Powershell, parse_requirements};

const MANIFEST: &str = "@{
    # Versions are exact.
    Pester = '5.7.1'
    PSScriptAnalyzer = '1.25.0'
}
";

struct Archives(RefCell<Vec<String>>);

impl Fetch for Archives {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = if url.ends_with("/Pester/5.7.1") {
            "PESTER"
        } else if url.ends_with("/PSScriptAnalyzer/1.25.0") {
            "PSSA"
        } else {
            return Err(Error::new(format!("unexpected download of {url}")));
        };
        Ok(dx_digest::sha256_hex(
            &std::fs::read(dx_testing::env_file(name)).map_err(|e| Error::new(e.to_string()))?,
        ))
    }
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("powershell").expect("powershell is registered");
    run(
        &Powershell::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Powershell,
        archives,
    )
}

#[test]
fn pinned_modules_write_the_golden_lock() {
    let hub = dx_families::hub("powershell").expect("powershell is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), MANIFEST).expect("manifest");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().len(), 2);
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join(hub.lock.as_deref().expect("lock")),
        Some("powershell/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn a_fresh_lock_downloads_nothing() {
    let hub = dx_families::hub("powershell").expect("powershell is registered");
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
fn a_line_that_is_not_an_entry_is_refused() {
    assert!(parse_requirements("@{\n    Pester = 5.7.1\n}\n").is_err());
    assert!(parse_requirements("@{\n}\n").is_err());
    assert!(parse_requirements("@{\n Pester = '1'\n Pester = '2'\n}\n").is_err());
}
