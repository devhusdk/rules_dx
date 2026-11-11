//! The lua lock step writes the golden lock from the registry fixture with an empty `PATH` and no network.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use lua_lock::{BASE, Download, Lua};

const APP: &str = "package = \"app\"\nversion = \"0.0-0\"\ndependencies = {\n  \"lua >= 5.1\", -- interpreter\n  \"mid ~> 2.0\",\n}\n";

struct Site(RefCell<Vec<String>>);

impl Download for Site {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        let name = match url.strip_prefix(&format!("{BASE}/")) {
            Some("manifest-5.5.json") => "MANIFEST",
            Some("mid-2.1-1.rockspec") => "MID_21",
            Some("mid-2.0-1.rockspec") => "MID_20",
            Some("leaf-1.2-0.rockspec") => "LEAF",
            _ => return Err(Error::new(format!("unexpected download of {url}"))),
        };
        std::fs::read(dx_testing::env_file(name)).map_err(|error| Error::new(error.to_string()))
    }
}

struct Digests;

impl Fetch for Digests {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Ok(format!("{:064x}", url.len()))
    }
}

fn set(manifest: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("app.rockspec"), manifest).expect("manifest");
    dir
}

fn lock(dir: &std::path::Path, site: &Site) -> Result<Outcome, Error> {
    let hub = dx_families::hub("luarocks").expect("luarocks is registered");
    let spec = Lua::spec(hub);
    run(
        &spec,
        &Args {
            dir: dir.to_owned(),
            check: false,
        },
        &Lua::new(&spec, BASE, site),
        &Digests,
    )
}

fn fetched(site: &Site) -> Vec<String> {
    site.0
        .borrow()
        .iter()
        .map(|url| url.trim_start_matches(&format!("{BASE}/")).to_owned())
        .collect()
}

#[test]
fn the_registry_fixture_writes_the_golden_lock() {
    assert_eq!(
        std::fs::read_to_string(dx_testing::env_file("APP")).expect("fixture"),
        APP
    );
    let dir = set(APP);
    let site = Site(RefCell::default());
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert_eq!(
        fetched(&site),
        [
            "manifest-5.5.json",
            "mid-2.1-1.rockspec",
            "leaf-1.2-0.rockspec"
        ]
    );
    let hub = dx_families::hub("luarocks").expect("luarocks is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &actual,
        Some("lua/private/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn unchanged_rocks_are_kept_without_a_download() {
    let dir = set(APP);
    assert_eq!(
        lock(dir.path(), &Site(RefCell::default())),
        Ok(Outcome::Written)
    );
    std::fs::write(dir.path().join("app.rockspec"), format!("{APP}\n")).expect("manifest");
    let site = Site(RefCell::default());
    assert_eq!(lock(dir.path(), &site), Ok(Outcome::Written));
    assert_eq!(fetched(&site), Vec::<String>::new());
}

#[test]
fn a_bound_no_release_meets_fails_with_the_rock() {
    let dir = set(&APP.replace("~> 2.0", ">= 4"));
    let error = lock(dir.path(), &Site(RefCell::default())).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("mid has no release that satisfies its dependencies"),
        "{error}"
    );
}

#[test]
fn a_rockspec_that_runs_code_fails() {
    let dir = set(&APP.replace("\"mid ~> 2.0\"", "require(\"mid\")"));
    let error = lock(dir.path(), &Site(RefCell::default())).expect_err("refused");
    assert!(error.to_string().contains("app.rockspec"), "{error}");
}
