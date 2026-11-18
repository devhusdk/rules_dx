//! The fpm lock step writes the golden lock from pinned archives with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use fpm_lock::{Download, Fpm, read_archive};

const STDLIB: &str = "https://github.com/fortran-lang/stdlib/archive/v0.7.0.tar.gz";
const FSPARSE: &str = "https://github.com/jalvesz/FSPARSE/archive/v1.2.0.tar.gz";

const MANIFEST: &str = "name = \"app\"
version = \"0.1.0\"

[dependencies]
fsparse = { git = \"https://github.com/jalvesz/FSPARSE.git\", tag = \"v1.2.0\" }
local = { path = \"../local\" }
";

struct Archives(RefCell<Vec<String>>);

impl Download for Archives {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = match url {
            STDLIB | "https://github.com/fortran-lang/stdlib/archive/v0.6.0.tar.gz" => "STDLIB",
            FSPARSE => "FSPARSE",
            other => return Err(Error::new(format!("unexpected download of {other}"))),
        };
        std::fs::read(dx_testing::env_file(name)).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn set(manifest: &str) -> tempfile::TempDir {
    let hub = dx_families::hub("fpm").expect("fpm is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), manifest).expect("manifest");
    dir
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("fpm").expect("fpm is registered");
    let spec = Fpm::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Fpm::new(&spec, archives),
        &NoFetch,
    )
}

#[test]
fn pinned_archives_write_the_golden_lock() {
    let dir = set(MANIFEST);
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().as_slice(), [FSPARSE, STDLIB]);
    let hub = dx_families::hub("fpm").expect("fpm is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &actual,
        Some("fortran/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn unchanged_packages_are_kept_without_a_download() {
    let dir = set(MANIFEST);
    assert_eq!(
        lock(dir.path(), &Archives(RefCell::default())),
        Ok(Outcome::Written)
    );
    let hub = dx_families::hub("fpm").expect("fpm is registered");
    std::fs::write(dir.path().join(&hub.manifest), format!("{MANIFEST}\n")).expect("manifest");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().as_slice(), Vec::<String>::new());
}

#[test]
fn conflicting_references_fail() {
    let manifest = format!(
        "{MANIFEST}stdlib = {{ git = \"https://github.com/fortran-lang/stdlib\", tag = \"v0.6.0\" }}\n"
    );
    let dir = set(&manifest);
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("stdlib is wanted at v0.6.0 by fpm.toml and at v0.7.0 by fsparse"),
        "{error}"
    );
}

#[test]
fn a_branch_dependency_fails() {
    let dir = set(&MANIFEST.replace("tag = \"v1.2.0\"", "branch = \"main\""));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("fsparse must pin its git source with tag or rev"),
        "{error}"
    );
}

#[test]
fn a_registry_dependency_fails() {
    let dir = set(&MANIFEST.replace(
        "fsparse = { git = \"https://github.com/jalvesz/FSPARSE.git\", tag = \"v1.2.0\" }",
        "fsparse = \"1.2.0\"",
    ));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("fsparse is not supported; only git dependencies are"),
        "{error}"
    );
}

#[test]
fn a_non_github_source_fails() {
    let dir = set(&MANIFEST.replace("https://github.com/jalvesz", "https://gitlab.com/jalvesz"));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error.to_string().contains("only https://github.com/<owner>/<repo> git URLs are"),
        "{error}"
    );
}

fn tar_gz(name: &str, data: &[u8]) -> Vec<u8> {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    let mut header = tar::Header::new_ustar();
    header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    builder.append(&header, data).expect("append");
    builder
        .into_inner()
        .and_then(flate2::write::GzEncoder::finish)
        .expect("tar.gz")
}

#[test]
fn unsafe_or_oversized_archive_members_are_refused() {
    let error =
        read_archive(&tar_gz("../fpm.toml", b"name = \"x\"\n"), "fpm.toml").expect_err("dot dot");
    assert!(error.to_string().contains("'..'"), "{error}");
    let error = read_archive(&tar_gz("top/fpm.toml", &vec![b'#'; 9 << 20]), "fpm.toml")
        .expect_err("oversized");
    assert!(error.to_string().contains("size cap"), "{error}");
}
