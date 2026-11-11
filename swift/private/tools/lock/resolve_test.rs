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

const TAG: &str = "https://api.github.com/repos/example/mini/git/ref/tags/1.0.0";

struct Archives(RefCell<Vec<String>>);

impl Download for Archives {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        if url == TAG {
            return Ok(format!("{{\"object\": {{\"type\": \"commit\", \"sha\": \"{REVISION}\"}}}}").into_bytes());
        }
        let name = if url.contains("/multi/") { "ARCHIVE_MULTI" } else { "ARCHIVE" };
        std::fs::read(dx_testing::env_file(name)).map_err(|error| Error::new(error.to_string()))
    }
}

struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

fn state(identity: &str, location: &str) -> String {
    format!(
        "{{\n  \"pins\" : [\n    {{\n      \"identity\" : \"{identity}\",\n      \"kind\" : \"remoteSourceControl\",\n      \"location\" : \"{location}\",\n      \"state\" : {{\n        \"revision\" : \"{REVISION}\",\n        \"version\" : \"1.0.0\"\n      }}\n    }}\n  ],\n  \"version\" : 3\n}}\n"
    )
}

fn set(location: &str) -> tempfile::TempDir {
    set_named("mini", location)
}

fn set_named(identity: &str, location: &str) -> tempfile::TempDir {
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), MANIFEST).expect("manifest");
    std::fs::write(dir.path().join(hub.state.as_deref().expect("state")), state(identity, location)).expect("state");
    dir
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let spec = Swift::spec(hub);
    let swift = dx_testing::env_file("SWIFT");
    let resolver = Swift::new(swift, &spec, archives);
    run(&spec, &Args { dir: dir.to_owned(), check: false, upgrade: Vec::new() }, &resolver, &NoFetch)
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
    dx_testing::snapshot_diff(&dx_testing::env_file("GOLDEN"), &actual, Some("swift/private/tools/lock/testdata/golden.lock.json"))
        .expect("the lock matches the golden");
}

fn bare(manifest: &str) -> tempfile::TempDir {
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), manifest).expect("manifest");
    dir
}

#[test]
fn a_manifest_with_no_state_file_writes_the_golden_state_and_lock() {
    let dir = bare(MANIFEST);
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(
        archives.0.borrow().as_slice(),
        [TAG.to_owned(), format!("https://github.com/example/mini/archive/{REVISION}.tar.gz")]
    );
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let state = std::fs::read_to_string(dir.path().join(hub.state.as_deref().expect("state"))).expect("state");
    assert_eq!(state, self::state("mini", "https://github.com/example/mini"));
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(&dx_testing::env_file("GOLDEN"), &actual, Some("swift/private/tools/lock/testdata/golden.lock.json"))
        .expect("the lock matches the golden");
}

#[test]
fn a_range_requirement_fails_with_its_manifest_line() {
    let dir = bare(&MANIFEST.replace("exact: \"1.0.0\"", "from: \"1.0.0\""));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("Package.swift:7: https://github.com/example/mini is not an exact"), "{error}");
}

#[test]
fn a_dependency_outside_github_fails_with_its_manifest_line() {
    let dir = bare(&MANIFEST.replace("github.com", "gitlab.com"));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("Package.swift:7: https://gitlab.com/example/mini is not on github.com"), "{error}");
}

#[test]
fn an_unchanged_pin_is_kept_without_a_download() {
    let dir = set("https://github.com/example/mini");
    assert_eq!(lock(dir.path(), &Archives(RefCell::default())), Ok(Outcome::Written));
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    std::fs::write(dir.path().join(&hub.manifest), format!("{MANIFEST}\n")).expect("manifest");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().as_slice(), Vec::<String>::new());
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
    let error = run(&spec, &Args { dir: dir.path().to_owned(), check: false, upgrade: Vec::new() }, &resolver, &NoFetch).expect_err("refused");
    assert!(error.to_string().contains("declares dependencies"), "{error}");
}

fn described(products: &str, targets: &str) -> serde_json::Value {
    serde_json::from_str(&format!("{{\"products\": {products}, \"targets\": {targets}}}")).expect("json")
}

#[test]
fn a_product_with_a_c_target_and_two_targets_is_kept() {
    let value = described(
        r#"[{"name": "Both", "targets": ["Swifty", "Native"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Swifty", "module_type": "SwiftTarget", "type": "library", "target_dependencies": ["Native"]},
            {"name": "Native", "module_type": "ClangTarget", "type": "library", "target_dependencies": []}]"#,
    );
    let (products, targets) = swift_lock::library_targets(&value, &Default::default()).expect("targets");
    assert_eq!(products["Both"], ["Swifty", "Native"]);
    assert_eq!(targets.keys().collect::<Vec<_>>(), ["Native", "Swifty"]);
}

#[test]
fn a_product_depending_on_a_macro_target_is_skipped() {
    let value = described(
        r#"[{"name": "Lib", "targets": ["Lib"], "type": {"library": ["automatic"]}},
            {"name": "Ok", "targets": ["Fine"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Lib", "module_type": "SwiftTarget", "type": "library", "target_dependencies": ["Macros"]},
            {"name": "Fine", "module_type": "SwiftTarget", "type": "library", "target_dependencies": []},
            {"name": "Macros", "module_type": "SwiftTarget", "type": "macro", "target_dependencies": []}]"#,
    );
    let (products, targets) = swift_lock::library_targets(&value, &Default::default()).expect("targets");
    assert_eq!(products.keys().collect::<Vec<_>>(), ["Ok"]);
    assert_eq!(targets.keys().collect::<Vec<_>>(), ["Fine"]);
}

#[test]
fn a_product_depending_on_a_plugin_target_is_skipped() {
    let value = described(
        r#"[{"name": "Lib", "targets": ["Lib"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Lib", "module_type": "SwiftTarget", "type": "library", "target_dependencies": ["Gen"]},
            {"name": "Gen", "module_type": "PluginTarget", "type": "plugin", "target_dependencies": []}]"#,
    );
    let (products, _) = swift_lock::library_targets(&value, &Default::default()).expect("targets");
    assert!(products.is_empty());
}

#[test]
fn a_product_with_an_unsupported_target_is_skipped() {
    let value = described(
        r#"[{"name": "Lib", "targets": ["Lib"], "type": {"library": ["automatic"]}}]"#,
        r#"[{"name": "Lib", "module_type": "SwiftTarget", "type": "library", "target_dependencies": []}]"#,
    );
    let unsupported = ["Lib".to_owned()].into_iter().collect();
    let (products, _) = swift_lock::library_targets(&value, &unsupported).expect("targets");
    assert!(products.is_empty());
}

#[test]
fn a_c_target_and_a_two_target_product_write_the_golden_lock() {
    let dir = set_named("multi", "https://github.com/example/multi");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    let hub = dx_families::hub("swiftpm").expect("swiftpm is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN_MULTI"),
        &actual,
        Some("swift/private/tools/lock/testdata/multi.lock.json"),
    )
    .expect("the lock matches the golden");
}
