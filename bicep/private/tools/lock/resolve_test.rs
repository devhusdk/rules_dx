//! The bicep lock step writes the golden lock from the registry fixture with an empty `PATH` and no network.

use std::cell::RefCell;

use bicep_lock::{Bicep, Registry, references};
use dx_lock::{Args, Error, Fetch, Outcome, run};

const MAIN: &str = "// module old 'br:example.com/old:1'
module storage 'br:mcr.microsoft.com/bicep/avm/res/storage/storage-account:0.14.3' = {
  name: 'storage'
}
/* 'br:example.com/skipped:2' */
module vnet 'br/public:avm/res/network/virtual-network:0.1.0' = {
  name: 'vnet'
}
";

const SECOND: &str = "module again 'br:MCR.microsoft.com/bicep/avm/res/storage/storage-account:0.14.3' = {
  name: 'again'
}
";

struct Site(RefCell<Vec<String>>);

impl Registry for Site {
    fn manifest(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        std::fs::read(dx_testing::env_file("MANIFEST")).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn lock(dir: &std::path::Path, site: &Site) -> Result<Outcome, Error> {
    let hub = dx_families::hub("bicep").expect("bicep is registered");
    run(
        &Bicep::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Bicep::new(site),
        &NoFetch,
    )
}

#[test]
fn the_registry_fixture_writes_the_golden_lock() {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("main.bicep"), MAIN).expect("main.bicep");
    std::fs::write(dir.path().join("second.bicep"), SECOND).expect("second.bicep");
    let site = Site(RefCell::default());
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert_eq!(
        *site.0.borrow(),
        [
            "https://mcr.microsoft.com/v2/bicep/avm/res/network/virtual-network/manifests/0.1.0",
            "https://mcr.microsoft.com/v2/bicep/avm/res/storage/storage-account/manifests/0.14.3",
        ]
    );
    let written = std::fs::read_to_string(dir.path().join("bicep.lock.json")).expect("lock");
    let golden = std::fs::read_to_string(dx_testing::env_file("GOLDEN")).expect("golden");
    assert_eq!(written, golden);
    site.0.borrow_mut().clear();
    std::fs::write(dir.path().join("second.bicep"), "").expect("second.bicep");
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert!(site.0.borrow().is_empty());
}

#[test]
fn two_tags_of_one_module_fail() {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("a.bicep"), "module a 'br:example.com/m:1' = {}\n").expect("a");
    std::fs::write(dir.path().join("b.bicep"), "module b 'br:example.com/m:2' = {}\n").expect("b");
    let error = lock(dir.path(), &Site(RefCell::default())).expect_err("two tags");
    assert!(error.to_string().contains("example.com/m is referenced with the tags 1 and 2"), "{error}");
}

#[test]
fn malformed_references_fail() {
    for text in ["'br:example.com/m'", "'br:example.com:5000/m:1'", "'br:example.com/m:'", "'br:host:1'"] {
        assert!(references(text).is_err(), "{text}");
    }
    assert!(references("'ts:sub/rg/spec:1' 'br'").expect("no reference").is_empty());
}
