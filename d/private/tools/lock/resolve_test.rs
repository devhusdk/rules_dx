//! The dub lock step writes the golden lock from registry fixtures with an empty `PATH` and no network.

use std::cell::RefCell;

use dub_lock::{Download, Dub};
use dx_lock::{Args, Error, Fetch, Outcome, run};

const SILLY: &str = "https://code.dlang.org/packages/silly/1.1.1.zip";
const BASE: &str = "https://code.dlang.org/packages/base/0.2.3.zip";

const MANIFEST: &str = "{\"name\": \"app\", \"dependencies\": {\"silly\": \"~>1.1.0\"}}\n";

struct Registry(RefCell<Vec<String>>);

impl Download for Registry {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        match url {
            "https://code.dlang.org/api/packages/silly/versions" => {
                Ok(br#"["~master", "1.0.0", "1.1.0", "1.1.1", "1.2.0", "2.0.0-rc.1"]"#.to_vec())
            }
            "https://code.dlang.org/api/packages/base/versions" => {
                Ok(br#"["0.1.0", "0.2.0", "0.2.3", "0.3.0"]"#.to_vec())
            }
            SILLY => std::fs::read(dx_testing::env_file("SILLY")).map_err(|error| Error::new(error.to_string())),
            BASE => std::fs::read(dx_testing::env_file("BASE")).map_err(|error| Error::new(error.to_string())),
            other => Err(Error::new(format!("unexpected download of {other}"))),
        }
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn set(manifest: &str) -> tempfile::TempDir {
    let hub = dx_families::hub("dub").expect("dub is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), manifest).expect("manifest");
    dir
}

fn lock(dir: &std::path::Path, registry: &Registry) -> Result<Outcome, Error> {
    let hub = dx_families::hub("dub").expect("dub is registered");
    let spec = Dub::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Dub::new(&spec, registry),
        &NoFetch,
    )
}

#[test]
fn registry_fixtures_write_the_golden_lock() {
    let dir = set(MANIFEST);
    let registry = Registry(RefCell::default());
    assert_eq!(lock(dir.path(), &registry), Ok(Outcome::Written));
    let hub = dx_families::hub("dub").expect("dub is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &actual,
        Some("d/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
    let state = std::fs::read_to_string(dir.path().join("dub.selections.json")).expect("state");
    assert_eq!(
        state,
        "{\n\t\"fileVersion\": 1,\n\t\"versions\": {\n\t\t\"base\": \"0.2.3\",\n\t\t\"silly\": \"1.1.1\"\n\t}\n}\n"
    );
}

#[test]
fn unchanged_packages_are_kept_without_a_download() {
    let dir = set(MANIFEST);
    assert_eq!(lock(dir.path(), &Registry(RefCell::default())), Ok(Outcome::Written));
    std::fs::write(dir.path().join("dub.json"), format!("{MANIFEST}\n")).expect("manifest");
    let registry = Registry(RefCell::default());
    assert_eq!(lock(dir.path(), &registry), Ok(Outcome::Written));
    assert_eq!(registry.0.borrow().as_slice(), Vec::<String>::new());
}

#[test]
fn a_path_dependency_fails() {
    let dir = set("{\"dependencies\": {\"local\": {\"path\": \"../local\"}}}");
    let error = lock(dir.path(), &Registry(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("selected by path or repository"), "{error}");
}

#[test]
fn an_unsatisfiable_range_fails() {
    let dir = set("{\"dependencies\": {\"silly\": \">=3.0.0\"}}");
    let error = lock(dir.path(), &Registry(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("no version of silly satisfies >=3.0.0"), "{error}");
}
