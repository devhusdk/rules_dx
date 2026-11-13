//! The opam lock step writes the golden lock with an empty `PATH`; opam files and archives come from fakes.

use std::cell::RefCell;

use dx_lock::{Args, Error, Fetch, Outcome, run};
use opam_lock::{Download, Opam, UNPINNED, parse_depend_names, parse_locked, parse_url};

const REPOSITORY: &str = "https://opam.test/packages";

const MANIFEST: &str = "opam-version: \"2.0\"\ndepends: [\"dune\" \"either-sum\"]\n";

const LOCKED: &str = r#"opam-version: "2.0"
name: "app"
depends: [
  "base-unix" {= "base"}
  "dune" {= "3.20.2"}
  "either-sum" {= "1.0.0"}
  "fmt-lite" {= "0.9.0"}
  "odoc" {with-doc & = "2.4.0"}
  "ocaml" {= "5.5.2" & post}
]
"#;

const EITHER_SUM: &str = r#"opam-version: "2.0"
depends: [
  "ocaml" {>= "4.08"}
  "dune" {>= "3.0"}
  "fmt-lite"
  "alcotest" {with-test}
]
url {
  src: "https://example.org/either-sum-1.0.0.tbz"
  checksum: [
    "sha256=1111111111111111111111111111111111111111111111111111111111111111"
    "sha512=00ff"
  ]
}
"#;

const FMT_LITE: &str = r#"opam-version: "2.0"
depends: ["ocaml" "dune"]
# a comment with "quotes"
url {
  src:
    "https://example.org/fmt-lite-0.9.0.tbz"
  checksum: "sha512=00ff"
}
"#;

const FMT_LITE_ARCHIVE: &[u8] = b"fmt-lite archive";

struct Files(RefCell<Vec<String>>);

impl Download for Files {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.borrow_mut().push(url.to_owned());
        match url.strip_prefix(REPOSITORY) {
            Some("/either-sum/either-sum.1.0.0/opam") => Ok(EITHER_SUM.as_bytes().to_vec()),
            Some("/fmt-lite/fmt-lite.0.9.0/opam") => Ok(FMT_LITE.as_bytes().to_vec()),
            _ => Err(Error::new(format!("unexpected download of {url}"))),
        }
    }
}

struct Archives(RefCell<Vec<String>>);

impl Fetch for Archives {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.0.borrow_mut().push(url.to_owned());
        if url == "https://example.org/fmt-lite-0.9.0.tbz" {
            Ok(dx_digest::sha256_hex(FMT_LITE_ARCHIVE))
        } else {
            Err(Error::new(format!("unexpected download of {url}")))
        }
    }
}

fn set() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("app.opam"), MANIFEST).expect("manifest");
    std::fs::write(dir.path().join("app.opam.locked"), LOCKED).expect("state");
    dir
}

fn lock(dir: &std::path::Path, files: &Files, archives: &Archives) -> Result<Outcome, Error> {
    let hub = dx_families::hub("opam").expect("opam is registered");
    run(
        &Opam::spec(hub),
        &Args {
            dir: dir.to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Opam::new(REPOSITORY, files),
        archives,
    )
}

#[test]
fn the_locked_file_writes_the_golden_lock() {
    let hub = dx_families::hub("opam").expect("opam is registered");
    let dir = set();
    let files = Files(RefCell::default());
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &files, &archives), Ok(Outcome::Written));
    assert_eq!(files.0.borrow().len(), 2);
    assert_eq!(archives.0.borrow().len(), 1);
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &dir.path().join(hub.lock.as_deref().expect("lock")),
        Some("ocaml/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

#[test]
fn a_fresh_lock_downloads_nothing() {
    let dir = set();
    assert_eq!(
        lock(dir.path(), &Files(RefCell::default()), &Archives(RefCell::default())),
        Ok(Outcome::Written)
    );
    let files = Files(RefCell::default());
    let archives = Archives(RefCell::default());
    assert_eq!(lock(dir.path(), &files, &archives), Ok(Outcome::Fresh));
    assert!(files.0.borrow().is_empty());
    assert!(archives.0.borrow().is_empty());
}

#[test]
fn a_dependency_the_lock_leaves_out_is_listed() {
    let pins = parse_locked("depends: [\"either-sum\" {= \"1.0.0\"}]\n").expect("pins");
    let packages = Opam::new(REPOSITORY, &Files(RefCell::default()))
        .packages(&pins)
        .expect("packages");
    assert!(packages[0].deps.is_empty());
    assert_eq!(packages[0].extra[UNPINNED], serde_json::json!(["fmt-lite"]));
}

#[test]
fn opam_fields_are_read() {
    let pins = parse_locked(LOCKED).expect("pins");
    assert_eq!(pins.get("either-sum").map(String::as_str), Some("1.0.0"));
    assert_eq!(pins.get("ocaml").map(String::as_str), Some("5.5.2"));
    assert!(!pins.contains_key("odoc"));
    assert_eq!(
        parse_depend_names(EITHER_SUM).expect("names"),
        vec!["ocaml", "dune", "fmt-lite"]
    );
    let url = parse_url(FMT_LITE).expect("url");
    assert_eq!(url.src, "https://example.org/fmt-lite-0.9.0.tbz");
    assert_eq!(url.sha256, "");
    assert!(parse_url("url { src: \"http://x/a.tbz\" }\n").is_err());
    assert!(parse_locked("depends: [\"a\" {>= \"1\"} \"b\" {= \"1\" | = \"2\"}]").expect("pins").is_empty());
}
