use dx_lock::{Args, Error, Outcome, run};
use dx_lock_testing::{FixtureFetch, NoFetch};
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

struct Archives(FixtureFetch);

impl Archives {
    fn new() -> Self {
        Self(FixtureFetch::new(&[
            (MINI, "MINI"),
            ("https://example.com/mini-1.tar.xz", "MINI_XZ"),
            ("https://example.com/mini-1.tar.zst", "MINI_ZST"),
            ("https://example.com/bomb-1.tar.xz", "BOMB_XZ"),
            (BASE, "BASE"),
        ]))
    }
}

impl Download for Archives {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.0.bytes(url)
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
            upgrade: Vec::new(),
        },
        &Zig::new(&spec, archives),
        &NoFetch,
    )
}

#[test]
fn pinned_archives_write_the_golden_lock() {
    let dir = set(MANIFEST);
    let archives = Archives::new();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.asked(), [BASE, MINI]);
    let hub = dx_families::hub("zig").expect("zig is registered");
    let actual = dir.path().join(hub.lock.as_deref().expect("lock"));
    dx_testing::snapshot_diff(
        &dx_testing::env_file("GOLDEN"),
        &actual,
        Some("zig/private/tools/lock/testdata/golden.lock.json"),
    )
    .expect("the lock matches the golden");
}

fn other_format_lock(extension: &str, sha256: &str, inputs: &str) {
    let dir = set(&MANIFEST.replace("mini-1.tar.gz", &format!("mini-1.{extension}")));
    assert_eq!(lock(dir.path(), &Archives::new()), Ok(Outcome::Written));
    let hub = dx_families::hub("zig").expect("zig is registered");
    let actual =
        std::fs::read_to_string(dir.path().join(hub.lock.as_deref().expect("lock"))).expect("lock");
    let golden = std::fs::read_to_string(dx_testing::env_file("GOLDEN"))
        .expect("golden")
        .replace("mini-1.tar.gz", &format!("mini-1.{extension}"))
        .replace(
            "39e7449a41c316a9ec6bbd0c6fd652862c4bbc66b0467bc53b74442650349efc",
            sha256,
        )
        .replace(
            "a8b9b2fb7140e173cfe77e77025f50bc5a70a4ef8c87c9a7815acdde5e88505b",
            inputs,
        );
    assert_eq!(actual, golden);
}

#[test]
fn a_tar_xz_archive_writes_the_same_lock() {
    other_format_lock(
        "tar.xz",
        "00cda170c7fdaddaa2668dcf0340e70f804ffe37535723fedf3b20b7d4d9f654",
        "54ad6fdce2fcb9e1d4372a880942535a50bec04f25216e50211a291f51cb42ba",
    );
}

#[test]
fn a_tar_zst_archive_writes_the_same_lock() {
    other_format_lock(
        "tar.zst",
        "581896a1f9d840367da9331c41a9a076d6552eb7621a3db037ad8dddbe101b67",
        "a6636650c80bf09aa7a7c4b5bbfa1c77135bb5714d8b8c32d1b886da43feebc2",
    );
}

#[test]
fn unchanged_packages_are_kept_without_a_download() {
    let dir = set(MANIFEST);
    assert_eq!(lock(dir.path(), &Archives::new()), Ok(Outcome::Written));
    let hub = dx_families::hub("zig").expect("zig is registered");
    std::fs::write(dir.path().join(&hub.manifest), format!("{MANIFEST}\n")).expect("manifest");
    let archives = Archives::new();
    assert_eq!(lock(dir.path(), &archives), Ok(Outcome::Written));
    assert_eq!(archives.0.asked(), Vec::<String>::new());
}

#[test]
fn an_unsupported_archive_fails_with_its_url() {
    let dir = set(&MANIFEST.replace("mini-1.tar.gz", "mini-1.tar.bz2"));
    let error = lock(dir.path(), &Archives::new()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("https://example.com/mini-1.tar.bz2 is not supported"),
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
    let error = lock(dir.path(), &Archives::new()).expect_err("refused");
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
    let error = lock(dir.path(), &Archives::new()).expect_err("refused");
    assert!(
        error
            .to_string()
            .contains("only https archive URLs are supported"),
        "{error}"
    );
}

struct UnsafeArchive;

impl Download for UnsafeArchive {
    fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        if url != BASE {
            return Archives::new().bytes(url);
        }
        let failed = |error: std::io::Error| Error::new(error.to_string());
        let mut writer = dx_archive_testing::RawZip::new();
        writer
            .add_file("../build.zig.zon", b".{}")
            .map_err(failed)?;
        writer.finish().map_err(failed)
    }
}

#[test]
fn an_archive_with_an_unsafe_member_is_refused() {
    let dir = set(MANIFEST);
    let hub = dx_families::hub("zig").expect("zig is registered");
    let spec = Zig::spec(hub);
    let error = run(
        &spec,
        &Args {
            dir: dir.path().to_owned(),
            check: false,
            upgrade: Vec::new(),
        },
        &Zig::new(&spec, &UnsafeArchive),
        &NoFetch,
    )
    .expect_err("dot dot");
    assert!(error.to_string().contains("'..'"), "{error}");
}

#[test]
fn an_xz_bomb_is_refused() {
    let dir = set(&MANIFEST.replace("mini-1.tar.gz", "bomb-1.tar.xz"));
    let error = lock(dir.path(), &Archives::new()).expect_err("bomb");
    assert!(
        error
            .to_string()
            .contains("xz stream exceeds 134217728 bytes"),
        "{error}"
    );
}
