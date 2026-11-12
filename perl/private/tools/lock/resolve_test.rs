//! The cpan lock step writes the golden lock with an empty `PATH`; the archives come from the fetcher.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use perl_lock::{Cpan, packages, parse_requires, parse_snapshot};

const CPANFILE: &str = "requires 'Text::CSV', '== 2.06';\nrequires \"Clone::PP\", '== 1.09';\n";

const SNAPSHOT: &str = "# carton snapshot format: version 1.0
DISTRIBUTIONS
  Clone-PP-1.09
    pathname: N/NE/NEILB/Clone-PP-1.09.tar.gz
    provides:
      Clone::PP 1.09
    requirements:
      ExtUtils::MakeMaker 0
  Text-CSV-2.06
    pathname: I/IS/ISHIGAKI/Text-CSV-2.06.tar.gz
    provides:
      Text::CSV 2.06
      Text::CSV_PP 2.06
    requirements:
      Clone::PP 0
      Text::CSV_PP 0
      perl 5.006
";

struct Archives(RefCell<Vec<String>>);

impl Fetch for Archives {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = if url.ends_with("/Text-CSV-2.06.tar.gz") {
            "TEXT_CSV"
        } else if url.ends_with("/Clone-PP-1.09.tar.gz") {
            "CLONE_PP"
        } else {
            return Err(Error::new(format!("unexpected download of {url}")));
        };
        Ok(dx_digest::sha256_hex(
            &std::fs::read(dx_testing::env_file(name)).map_err(|e| Error::new(e.to_string()))?,
        ))
    }
}

fn set() -> tempfile::TempDir {
    let hub = dx_families::hub("cpan").expect("cpan is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), CPANFILE).expect("manifest");
    std::fs::write(
        dir.path().join(hub.state.as_deref().expect("state")),
        SNAPSHOT,
    )
    .expect("snapshot");
    dir
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("cpan").expect("cpan is registered");
    run(
        &Cpan::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Cpan,
        archives,
    )
}

#[test]
fn the_snapshot_writes_the_golden_lock() {
    let hub = dx_families::hub("cpan").expect("cpan is registered");
    let dir = set();
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().len(), 2);
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join(hub.lock.as_deref().expect("lock")),
        Some("perl/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn a_fresh_lock_downloads_nothing() {
    let dir = set();
    assert_eq!(
        lock(dir.path(), &Archives(RefCell::default())),
        Ok(Outcome::Written)
    );
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Fresh));
    assert!(archives.0.borrow().is_empty());
}

#[test]
fn requirements_become_distribution_deps() {
    let packages = packages(CPANFILE, SNAPSHOT).expect("packages");
    let csv = packages.iter().find(|p| p.name == "Text-CSV").expect("Text-CSV");
    assert_eq!(csv.version, "2.06");
    assert_eq!(csv.deps, vec!["Clone-PP".to_owned()]);
    assert_eq!(csv.archives[0].strip_prefix, "Text-CSV-2.06");
}

#[test]
fn a_module_outside_the_snapshot_is_refused() {
    assert!(packages("requires 'Moo';\n", SNAPSHOT).is_err());
    assert_eq!(parse_requires("requires 'A::B', '1';\n# requires 'C'\n"), vec!["A::B"]);
    assert!(parse_snapshot("DISTRIBUTIONS\n").is_err());
    assert!(parse_snapshot("# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  A-1\n").is_err());
}
