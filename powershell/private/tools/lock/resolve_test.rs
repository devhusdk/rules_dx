use dx_lock::{Args, Error, Outcome, run};
use dx_lock_testing::FixtureFetch;
use powershell_lock::{Powershell, parse_requirements};

const MANIFEST: &str = "@{
    # Versions are exact.
    Pester = '5.7.1'
    PSScriptAnalyzer = '1.25.0'
}
";

fn archives() -> FixtureFetch {
    FixtureFetch::new(&[
        ("/Pester/5.7.1", "PESTER"),
        ("/PSScriptAnalyzer/1.25.0", "PSSA"),
    ])
}

fn lock(dir: &std::path::Path, archives: &FixtureFetch) -> Result<Outcome, Error> {
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
    let archives = archives();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.asked().len(), 2);
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
    assert_eq!(lock(dir.path(), &archives()), Ok(Outcome::Written));
    let archives = archives();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Fresh));
    assert!(archives.asked().is_empty());
}

#[test]
fn a_line_that_is_not_an_entry_is_refused() {
    assert!(parse_requirements("@{\n    Pester = 5.7.1\n}\n").is_err());
    assert!(parse_requirements("@{\n}\n").is_err());
    assert!(parse_requirements("@{\n Pester = '1'\n Pester = '2'\n}\n").is_err());
}
