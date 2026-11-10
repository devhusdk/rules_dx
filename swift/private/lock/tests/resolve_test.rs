//! The swiftpm lock step writes the golden lock from a pinned archive with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use swift_lock::{Download, Swift};

const REVISION: &str = "1111111111111111111111111111111111111111";

const MANIFEST: &str = "// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: \"app\",
    dependencies: [
        .package(url: \"https://github.com/example/mini\", exact: \"1.0.0\"),
    ],
    targets: []
)
";

struct Archives(RefCell<Vec<String>>);

impl Download for Archives {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        std::fs::read(dx_testing::env_file("ARCHIVE")).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn state(location: &str) -> String {
    format!(
        "{{\n  \"pins\" : [\n    {{\n      \"identity\" : \"mini\",\n      \"kind\" : \"remoteSourceControl\",\n      \"location\" : \"{location}\",\n      \"state\" : {{\n        \"revision\" : \"{REVISION}\",\n        \"version\" : \"1.0.0\"\n      }}\n    }}\n  ],\n  \"version\" : 3\n}}\n"
    )
}

fn set(location: &str) -> tempfile::TempDir {
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), MANIFEST).expect("manifest");
    std::fs::write(dir.path().join(hub.state.as_deref().expect("state")), state(location)).expect("state");
    dir
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let spec = Swift::spec(hub);
    let swift = dx_testing::env_file("SWIFT");
    let resolver = Swift::new(swift, &spec, archives);
    run(&spec, &Args { dir: dir.to_owned(), check: false }, &resolver, &NoFetch)
}

#[test]
fn a_pinned_package_writes_the_golden_lock() {
    let dir = set("https://github.com/example/mini");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(
        archives.0.borrow().as_slice(),
        [format!("https://github.com/example/mini/archive/{REVISION}.tar.gz")]
    );
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(&dx_testing::env_file("GOLDEN"), &actual, Some("swift/private/lock/testdata/golden.lock.json"))
        .expect("the lock matches the golden");
}

#[test]
fn a_pin_outside_github_fails() {
    let dir = set("https://gitlab.com/example/mini");
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("only github.com is supported"), "{error}");
}
