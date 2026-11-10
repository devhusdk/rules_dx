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

#[test]
fn a_package_with_dependencies_fails() {
    let manifest = MANIFEST.replace("name: \"app\"", "name: \"mini\"");
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, format!("mini-{REVISION}/Package.swift"), manifest.as_bytes()).expect("entry");
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, &builder.into_inner().expect("tar")).expect("gzip");
    let bytes = encoder.finish().expect("gzip");
    struct Fixed(Vec<u8>);
    impl Download for Fixed {
        fn bytes(&self, _url: &str) -> Result<Vec<u8>, Error> {
            Ok(self.0.clone())
        }
    }
    let dir = set("https://github.com/example/mini");
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let spec = Swift::spec(hub);
    let download = Fixed(bytes);
    let resolver = Swift::new(dx_testing::env_file("SWIFT"), &spec, &download);
    let error = run(&spec, &Args { dir: dir.path().to_owned(), check: false }, &resolver, &NoFetch).expect_err("refused");
    assert!(error.to_string().contains("declares dependencies"), "{error}");
}

fn described(products: &str, targets: &str) -> serde_json::Value {
    serde_json::from_str(&format!("{{\"products\": {products}, \"targets\": {targets}}}")).expect("json")
}

#[test]
fn a_product_with_a_c_target_is_left_out() {
    let value = described(
        r#"[{"name": "Swifty", "targets": ["Swifty"], "type": {"library": ["automatic"]}},
            {"name": "Native", "targets": ["Native"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Swifty", "module_type": "SwiftTarget", "type": "library", "target_dependencies": []},
            {"name": "Native", "module_type": "ClangTarget", "type": "library", "target_dependencies": []}]"#,
    );
    let (products, targets) = swift_lock::library_targets(&value).expect("targets");
    assert_eq!(products.keys().collect::<Vec<_>>(), ["Swifty"]);
    assert_eq!(targets.keys().collect::<Vec<_>>(), ["Swifty"]);
}

#[test]
fn a_product_depending_on_a_macro_target_is_left_out() {
    let value = described(
        r#"[{"name": "Lib", "targets": ["Lib"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Lib", "module_type": "SwiftTarget", "type": "library", "target_dependencies": ["Macros"]},
            {"name": "Macros", "module_type": "SwiftTarget", "type": "macro", "target_dependencies": []}]"#,
    );
    let (products, targets) = swift_lock::library_targets(&value).expect("targets");
    assert!(products.is_empty() && targets.is_empty());
}

#[test]
fn a_product_with_two_targets_fails() {
    let value = described(
        r#"[{"name": "Lib", "targets": ["A", "B"], "type": {"library": ["automatic"]}}]"#,
        "[]",
    );
    assert!(swift_lock::library_targets(&value).is_err());
}
