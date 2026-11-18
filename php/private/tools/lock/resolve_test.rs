//! The composer lock step writes the golden lock and state from a fake Packagist with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use php_lock::{Download, Php};

const FACTORY_ZIP: &str = "https://example.com/zipball/factory.zip";
const MESSAGE_ZIP: &str = "https://example.com/zipball/message.zip";

const MANIFEST: &str = "{\n    \"name\": \"rules-dx/app\",\n    \"require\": {\n        \"php\": \">=8.1\",\n        \"psr/http-factory\": \"^1.1\"\n    }\n}\n";

fn factory() -> String {
    format!(
        "{{\"minified\":\"composer/2.0\",\"packages\":{{\"psr/http-factory\":[\
         {{\"name\":\"psr/http-factory\",\"version\":\"2.0.0-beta1\",\"version_normalized\":\"2.0.0.0-beta1\",\"require\":{{\"psr/http-message\":\"^2.0\"}},\"dist\":{{\"url\":\"{FACTORY_ZIP}\",\"type\":\"zip\"}}}},\
         {{\"version\":\"1.1.0\",\"version_normalized\":\"1.1.0.0\",\"require\":{{\"php\":\">=7.1\",\"psr/http-message\":\"^1.0 || ^2.0\"}},\"autoload\":{{\"psr-4\":{{\"Psr\\\\Http\\\\Message\\\\\":\"src/\"}}}}}},\
         {{\"version\":\"1.0.2\",\"version_normalized\":\"1.0.2.0\",\"autoload\":\"__unset\"}}]}}}}"
    )
}

fn message() -> String {
    format!(
        "{{\"minified\":\"composer/2.0\",\"packages\":{{\"psr/http-message\":[\
         {{\"name\":\"psr/http-message\",\"version\":\"2.0\",\"version_normalized\":\"2.0.0.0\",\"dist\":{{\"url\":\"{MESSAGE_ZIP}\",\"type\":\"zip\"}}}},\
         {{\"version\":\"1.1\",\"version_normalized\":\"1.1.0.0\"}}]}}}}"
    )
}

fn read(name: &str) -> Result<Vec<u8>, Error> {
    std::fs::read(dx_testing::env_file(name)).map_err(|error| Error::new(error.to_string()))
}

struct Remote(RefCell<Vec<String>>);

impl Download for Remote {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        match url {
            "https://repo.packagist.org/p2/psr/http-factory.json" => Ok(factory().into_bytes()),
            "https://repo.packagist.org/p2/psr/http-message.json" => Ok(message().into_bytes()),
            FACTORY_ZIP => read("FACTORY"),
            MESSAGE_ZIP => read("MESSAGE"),
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
    let hub = dx_families::hub("composer").expect("composer is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), manifest).expect("manifest");
    dir
}

fn lock(dir: &std::path::Path, remote: &Remote) -> Result<Outcome, Error> {
    let hub = dx_families::hub("composer").expect("composer is registered");
    let spec = Php::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Php::new(&spec, remote),
        &NoFetch,
    )
}

fn archives(remote: &Remote) -> Vec<String> {
    remote
        .0
        .borrow()
        .iter()
        .filter(|url| url.contains("/zipball/"))
        .cloned()
        .collect()
}

#[test]
fn a_manifest_writes_the_golden_lock_and_state() {
    let dir = set(MANIFEST);
    let remote = Remote(RefCell::default());
    assert_eq!(lock(dir.path(), &remote), Ok(Outcome::Written));
    assert_eq!(archives(&remote), [FACTORY_ZIP, MESSAGE_ZIP]);
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join("composer.lock.json"),
        Some("php/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN_STATE"),
        &dir.path().join("composer.lock"),
        Some("php/private/tools/lock/testdata/golden.composer.lock"),
    )
    .expect("the state matches the golden");
}

#[test]
fn an_unchanged_package_is_not_downloaded_again() {
    let dir = set(MANIFEST);
    lock(dir.path(), &Remote(RefCell::default())).expect("first lock");
    std::fs::write(
        dir.path().join("composer.json"),
        MANIFEST.replace("    ", "  "),
    )
    .expect("edit");
    let remote = Remote(RefCell::default());
    assert_eq!(lock(dir.path(), &remote), Ok(Outcome::Written));
    assert_eq!(archives(&remote), Vec::<String>::new());
}

#[test]
fn a_branch_constraint_names_the_requirement() {
    let dir = set(&MANIFEST.replace("^1.1", "dev-main"));
    let error = lock(dir.path(), &Remote(RefCell::default())).expect_err("unsupported");
    assert!(error.to_string().contains("psr/http-factory"), "{error}");
}

#[test]
fn a_dev_minimum_stability_is_refused() {
    let dir = set(&MANIFEST.replacen('{', "{\n    \"minimum-stability\": \"dev\",", 1));
    let error = lock(dir.path(), &Remote(RefCell::default())).expect_err("unsupported");
    assert!(error.to_string().contains("minimum-stability"), "{error}");
}

struct UnsafeArchive;

impl Download for UnsafeArchive {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        match url {
            FACTORY_ZIP | MESSAGE_ZIP => {
                let failed = |error: std::io::Error| Error::new(error.to_string());
                let mut writer = dx_archive_testing::RawZip::new();
                writer.add_file("../composer.json", b"{}").map_err(failed)?;
                writer.finish().map_err(failed)
            }
            other => Remote(RefCell::default()).bytes(other),
        }
    }
}

#[test]
fn an_archive_with_an_unsafe_member_is_refused() {
    let dir = set(MANIFEST);
    let hub = dx_families::hub("composer").expect("composer is registered");
    let spec = Php::spec(hub);
    let error = run(
        &spec,
        &Args {
            dir: dir.path().to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Php::new(&spec, &UnsafeArchive),
        &NoFetch,
    )
    .expect_err("dot dot");
    assert!(error.to_string().contains("'..'"), "{error}");
}

#[test]
fn a_short_version_equals_its_padded_form_and_prereleases_are_not_releases() {
    use php_lock::{Constraint, parse_version};
    let release = |text: &str| parse_version(text).unwrap_or_else(|| panic!("{text}"));
    assert_eq!(release("1.2"), release("1.2.0"));
    assert_eq!(release("v1.2.3.0"), release("1.2.3"));
    assert!(release("1.2.3.4.5") > release("1.2.3.4"));
    assert!(parse_version("1.2.3-RC1").is_none());
    assert!(parse_version("dev-main").is_none());
    let caret = Constraint::parse("^1.2").expect("constraint");
    assert!(caret.allows(&release("1.2.0")));
    assert!(caret.allows(&release("1.9.9.9.9")));
    assert!(!caret.allows(&release("2.0")));
    let pinned = Constraint::parse("1.2.3").expect("constraint");
    assert!(pinned.allows(&release("1.2.3.0")));
}
