//! The zig lock step writes the golden lock from pinned archives with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use zig_lock::{Download, Zig};

const MINI: &str = "https://example.com/mini-1.tar.gz";
const BASE: &str = "https://example.com/base-1.zip";

const MANIFEST: &str = "// root manifest
.{
    .name = .app,
    .version = \"0.0.0\",
    .dependencies = .{
        .mini = .{
            .url = \"https://example.com/mini-1.tar.gz\",
            .hash = \"mini-1\",
        },
        .base = .{
            .url = \"https://example.com/base-1.zip\",
            .hash = \"base-1\",
        },
    },
}
";

struct Archives(RefCell<Vec<String>>);

impl Download for Archives {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = match url {
            MINI => "MINI",
            BASE => "BASE",
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
    let hub = dx_families::hub("zig").expect("zig is registered");
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join(&hub.manifest), manifest).expect("manifest");
    dir
}

fn lock(dir: &std::path::Path, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("zig").expect("zig is registered");
    let spec = Zig::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
        },
        &Zig::new(&spec, archives),
        &NoFetch,
    )
}

#[test]
fn pinned_archives_write_the_golden_lock() {
    let dir = set(MANIFEST);
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().as_slice(), [BASE, MINI]);
    let hub = dx_families::hub("zig").expect("zig is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &actual,
        Some("zig/private/lock/testdata/golden.lock.json"),
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
    let hub = dx_families::hub("zig").expect("zig is registered");
    std::fs::write(dir.path().join(&hub.manifest), format!("{MANIFEST}\n")).expect("manifest");
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.borrow().as_slice(), Vec::<String>::new());
}

#[test]
fn an_unsupported_archive_fails_with_its_url() {
    let dir = set(&MANIFEST.replace("mini-1.tar.gz", "mini-1.tar.xz"));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("https://example.com/mini-1.tar.xz is not supported"),
        "{error}"
    );
}

#[test]
fn a_dependency_missing_from_the_root_fails() {
    let manifest = MANIFEST.replace(
        "        .base = .{\n            .url = \"https://example.com/base-1.zip\",\n            .hash = \"base-1\",\n        },\n",
        "",
    );
    let dir = set(&manifest);
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("depends on base, which the root build.zig.zon does not list"),
        "{error}"
    );
}

#[test]
fn a_plain_http_url_fails() {
    let dir = set(&MANIFEST.replace("https://example.com/mini", "http://example.com/mini"));
    let error = lock(dir.path(), &Archives(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("only https archive URLs are supported"),
        "{error}"
    );
}
