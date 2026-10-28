use std::io::Write as _;

use super::*;
use crate::archive::ArchiveError;
use dx_audit::advisory::{advisory_source, identity_matches_bytes, parse_identity};
use dx_audit::vuln::parse_snapshot;
use tempfile::TempDir;

const STORED: u16 = 0;
const DEFLATED: u16 = 8;
const EOCD_FIXED: usize = 22;
const CENTRAL_FIXED: usize = 46;

struct Fixture {
    name: String,
    body: Vec<u8>,
    method: u16,
}

impl Fixture {
    fn stored(name: &str, body: &str) -> Self {
        Fixture {
            name: name.to_owned(),
            body: body.as_bytes().to_vec(),
            method: STORED,
        }
    }

    fn deflated(name: &str, body: &str) -> Self {
        Fixture {
            name: name.to_owned(),
            body: body.as_bytes().to_vec(),
            method: DEFLATED,
        }
    }
}

fn deflated_bytes(body: &[u8]) -> Vec<u8> {
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(body).expect("deflates");
    encoder.finish().expect("finishes")
}

fn le16(value: u16) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn small(value: usize) -> u32 {
    u32::try_from(value).expect("small fixture")
}

/// Builds the smallest zip archive that holds every fixture entry.
fn archive(fixtures: &[Fixture]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    for fixture in fixtures {
        let data = if fixture.method == DEFLATED {
            deflated_bytes(&fixture.body)
        } else {
            fixture.body.clone()
        };
        let crc = crc32fast::hash(&fixture.body);
        let name = fixture.name.as_bytes();
        let offset = small(out.len());
        out.extend(b"PK\x03\x04");
        out.extend(le16(20));
        out.extend(le16(0));
        out.extend(le16(fixture.method));
        out.extend(le16(0));
        out.extend(le16(0));
        out.extend(le32(crc));
        out.extend(le32(small(data.len())));
        out.extend(le32(small(fixture.body.len())));
        out.extend(le16(u16::try_from(name.len()).expect("short name")));
        out.extend(le16(0));
        out.extend(name);
        out.extend(&data);
        directory.extend(b"PK\x01\x02");
        directory.extend(le16(20));
        directory.extend(le16(20));
        directory.extend(le16(0));
        directory.extend(le16(fixture.method));
        directory.extend(le16(0));
        directory.extend(le16(0));
        directory.extend(le32(crc));
        directory.extend(le32(small(data.len())));
        directory.extend(le32(small(fixture.body.len())));
        directory.extend(le16(u16::try_from(name.len()).expect("short name")));
        directory.extend(le16(0));
        directory.extend(le16(0));
        directory.extend(le16(0));
        directory.extend(le16(0));
        directory.extend(le32(0));
        directory.extend(le32(offset));
        directory.extend(name);
    }
    let count = u16::try_from(fixtures.len()).expect("few entries");
    let size = small(directory.len());
    let offset = small(out.len());
    out.extend(&directory);
    out.extend(b"PK\x05\x06");
    out.extend(le16(0));
    out.extend(le16(0));
    out.extend(le16(count));
    out.extend(le16(count));
    out.extend(le32(size));
    out.extend(le32(offset));
    out.extend(le16(0));
    out
}

/// Where the single-entry central directory of one fixture archive starts.
fn central_at(archive: &[u8], name_len: usize) -> usize {
    archive.len() - EOCD_FIXED - CENTRAL_FIXED - name_len
}

fn advisory(id: &str, name: &str) -> String {
    format!(
        r#"{{"id":"{id}","modified":"2026-01-01T00:00:00Z","affected":[{{"package":{{"ecosystem":"crates.io","name":"{name}"}},"versions":["1.0.0"]}}]}}"#
    )
}

fn workspace() -> TempDir {
    let dir = tempfile::tempdir().expect("temp workspace");
    let lock = dir.path().join("rust/tests/fixtures/hello/Cargo.lock");
    std::fs::create_dir_all(lock.parent().expect("parent")).expect("creates");
    std::fs::write(&lock, b"version = 4\n").expect("writes");
    dir
}

fn options(workspace: &TempDir) -> PrepareOptions {
    PrepareOptions {
        workspace: workspace.path().to_path_buf(),
        out: workspace.path().join(DEFAULT_OUT),
        retrieved_at: "2026-09-22".to_owned(),
        limits: DEFAULT_LIMITS,
        archives: BTreeMap::new(),
    }
}

fn with_archive(workspace: &TempDir, options: PrepareOptions, body: &[u8]) -> PrepareOptions {
    let path = workspace.path().join("cargo.zip");
    std::fs::write(&path, body).expect("writes the archive");
    let mut options = options;
    options.archives.insert("cargo".to_owned(), path);
    options
}

#[test]
fn both_stored_and_deflated_json_entries_are_read() {
    let bytes = archive(&[
        Fixture::stored("all/2/GHSA-b.json", &advisory("GHSA-b", "bbb")),
        Fixture::deflated("all/1/GHSA-a.json", &advisory("GHSA-a", "aaa")),
        Fixture::stored("all/README.txt", "not an advisory"),
    ]);
    let entries = archive::json_entries(&bytes).expect("reads");
    assert_eq!(entries.len(), 2, "the README is not an advisory");
    assert_eq!(entries[0].0, "all/2/GHSA-b.json", "archive order");
    assert_eq!(entries[1].0, "all/1/GHSA-a.json", "archive order");
    assert_eq!(
        String::from_utf8(entries[1].1.clone()).expect("utf-8"),
        advisory("GHSA-a", "aaa")
    );
}

/// Rewrites a fixture archive to use a zip64 end record, optionally overriding its count or offset.
fn zip64(mut bytes: Vec<u8>, count: Option<u64>, record: Option<u64>) -> Vec<u8> {
    let eocd = bytes.len() - EOCD_FIXED;
    let real_count = u64::from(u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]));
    let size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().expect("size"));
    let offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().expect("offset"));
    bytes.truncate(eocd);
    let real_record = u64::try_from(bytes.len()).expect("small fixture");
    let count = count.unwrap_or(real_count);
    bytes.extend(b"PK\x06\x06");
    bytes.extend(44u64.to_le_bytes());
    bytes.extend(le16(45));
    bytes.extend(le16(45));
    bytes.extend(le32(0));
    bytes.extend(le32(0));
    bytes.extend(count.to_le_bytes());
    bytes.extend(count.to_le_bytes());
    bytes.extend(u64::from(size).to_le_bytes());
    bytes.extend(u64::from(offset).to_le_bytes());
    bytes.extend(b"PK\x06\x07");
    bytes.extend(le32(0));
    bytes.extend(record.unwrap_or(real_record).to_le_bytes());
    bytes.extend(le32(1));
    bytes.extend(b"PK\x05\x06");
    bytes.extend(le16(0));
    bytes.extend(le16(0));
    bytes.extend(le16(u16::MAX));
    bytes.extend(le16(u16::MAX));
    bytes.extend(le32(size));
    bytes.extend(le32(offset));
    bytes.extend(le16(0));
    bytes
}

fn two_advisories() -> Vec<u8> {
    archive(&[
        Fixture::stored("all/1/GHSA-a.json", &advisory("GHSA-a", "aaa")),
        Fixture::stored("all/2/GHSA-b.json", &advisory("GHSA-b", "bbb")),
    ])
}

#[test]
fn a_zip64_end_record_supplies_the_entry_count() {
    let bytes = zip64(two_advisories(), None, None);
    let entries = archive::json_entries(&bytes).expect("reads");
    assert_eq!(entries.len(), 2);
}

#[test]
fn a_zip64_entry_count_larger_than_the_directory_is_refused() {
    let plain = two_advisories();
    let eocd = plain.len() - EOCD_FIXED;
    let size = u64::from(u32::from_le_bytes(
        plain[eocd + 12..eocd + 16].try_into().expect("size"),
    ));
    let fits = size / CENTRAL_FIXED as u64;
    for count in [u64::MAX, fits + 1] {
        match archive::json_entries(&zip64(plain.clone(), Some(count), None)) {
            Err(ArchiveError::BadCentralDirectory { detail }) => {
                assert!(detail.contains("entries cannot fit"), "{count}: {detail}");
            }
            other => panic!("{count}: {other:?}"),
        }
    }
}

#[test]
fn central_entries_that_do_not_end_at_the_directory_end_are_refused() {
    let mut bytes = two_advisories();
    let eocd = bytes.len() - EOCD_FIXED;
    bytes[eocd + 10..eocd + 12].copy_from_slice(&1u16.to_le_bytes());
    match archive::json_entries(&bytes) {
        Err(ArchiveError::BadCentralDirectory { detail }) => {
            assert!(detail.contains("entries end at"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_zip64_record_offset_past_the_end_of_memory_is_refused() {
    let bytes = zip64(two_advisories(), None, Some(u64::MAX - 1));
    match archive::json_entries(&bytes) {
        Err(ArchiveError::BadCentralDirectory { detail }) => {
            assert!(detail.contains("no zip64 end record"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_archive_that_is_not_a_zip_is_refused() {
    let truncated = archive(&[Fixture::stored("all/1.json", "{}")]);
    for bytes in [
        Vec::new(),
        b"not a zip at all".to_vec(),
        truncated[..EOCD_FIXED / 2].to_vec(),
    ] {
        assert_eq!(
            archive::json_entries(&bytes),
            Err(ArchiveError::NotAnArchive),
            "{:?}",
            String::from_utf8_lossy(&bytes)
        );
    }
}

#[test]
fn a_tampered_payload_is_caught_by_its_crc() {
    let bytes = archive(&[Fixture::stored("all/1.json", &advisory("GHSA-a", "aaa"))]);
    let data_at = 30 + "all/1.json".len();
    let mut tampered = bytes;
    tampered[data_at] = b'!';
    match archive::json_entries(&tampered) {
        Err(ArchiveError::BadEntry { name, detail }) => {
            assert_eq!(name, "all/1.json");
            assert!(detail.contains("crc32"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_tampered_deflate_stream_is_refused() {
    let bytes = archive(&[Fixture::deflated("all/1.json", &advisory("GHSA-a", "aaa"))]);
    let data_at = 30 + "all/1.json".len();
    let mut tampered = bytes;
    tampered[data_at + 3] ^= 0xff;
    match archive::json_entries(&tampered) {
        Err(ArchiveError::BadEntry { name, detail }) => {
            assert_eq!(name, "all/1.json");
            assert!(
                detail.contains("crc32") || detail.contains("deflate"),
                "{detail}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unsupported_compression_method_is_named() {
    let bytes = archive(&[Fixture::stored("all/1.json", "{}")]);
    let directory = central_at(&bytes, "all/1.json".len());
    let mut tampered = bytes;
    tampered[8] = 0x0b;
    tampered[directory + 10] = 0x0b;
    match archive::json_entries(&tampered) {
        Err(ArchiveError::UnsupportedMethod { name, method }) => {
            assert_eq!(name, "all/1.json");
            assert_eq!(method, 11);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_entry_pointing_outside_the_archive_is_refused() {
    let bytes = archive(&[Fixture::stored("all/1.json", "{}")]);
    let directory = central_at(&bytes, "all/1.json".len());
    let mut tampered = bytes;
    tampered[directory + 42..directory + 46].copy_from_slice(&9_000u32.to_le_bytes());
    match archive::json_entries(&tampered) {
        Err(ArchiveError::BadEntry { name, detail }) => {
            assert_eq!(name, "all/1.json");
            assert!(detail.contains("local file header"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_central_directory_that_does_not_end_at_the_record_is_refused() {
    let bytes = archive(&[Fixture::stored("all/1.json", "{}")]);
    let mut tampered = bytes;
    let eocd = tampered.len() - EOCD_FIXED;
    tampered[eocd + 16..eocd + 20].copy_from_slice(&8u32.to_le_bytes());
    match archive::json_entries(&tampered) {
        Err(ArchiveError::BadCentralDirectory { detail }) => {
            assert!(detail.contains("directory covers"), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_preparation_writes_one_snapshot_and_sidecar_per_needed_ecosystem() {
    let workspace = workspace();
    let options = with_archive(
        &workspace,
        options(&workspace),
        &archive(&[
            Fixture::deflated("all/1/GHSA-a.json", &advisory("GHSA-a", "aaa")),
            Fixture::stored("all/2/GHSA-b.json", &advisory("GHSA-b", "bbb")),
        ]),
    );
    let prepared = prepare(&options).expect("prepares");
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].family, "cargo");
    assert_eq!(prepared[0].advisories, 2);
    assert_eq!(prepared[0].path, ".dx/advisory/cargo.json");
    let out = workspace.path().join(DEFAULT_OUT);
    let payload_bytes = std::fs::read(out.join("cargo.json")).expect("reads the snapshot");
    assert_eq!(prepared[0].identity, dx_digest::sha256_hex(&payload_bytes));
    let sidecar = std::fs::read_to_string(out.join("cargo.meta.json")).expect("reads the sidecar");
    let identity = parse_identity(&sidecar).expect("the audit reads the sidecar");
    assert_eq!(identity.sha256, prepared[0].identity);
    assert_eq!(identity.retrieved_at, "2026-09-22");
    assert_eq!(identity.path, ".dx/advisory/cargo.json");
    assert!(identity_matches_bytes(&identity, &payload_bytes));
    assert_eq!(
        parse_snapshot(&String::from_utf8(payload_bytes).expect("utf-8"))
            .expect("the audit reads the snapshot")
            .len(),
        2
    );
    assert_eq!(
        std::fs::read_dir(&out).expect("lists").count(),
        2,
        "one snapshot and one sidecar, no archive"
    );
}

#[test]
fn a_failed_preparation_writes_nothing_for_that_ecosystem() {
    let workspace = workspace();
    let options = with_archive(&workspace, options(&workspace), b"not a zip");
    let error = prepare(&options).expect_err("refuses");
    assert!(
        error.to_string().starts_with("advisory_prepare_failed:"),
        "{error}"
    );
    assert!(
        error.to_string().contains("cargo"),
        "{error} names the ecosystem"
    );
    assert!(
        !workspace
            .path()
            .join(DEFAULT_OUT)
            .join("cargo.json")
            .exists(),
        "a refused archive leaves no snapshot behind"
    );
}

#[test]
fn a_missing_local_archive_is_named() {
    let workspace = workspace();
    let mut options = options(&workspace);
    options
        .archives
        .insert("cargo".to_owned(), workspace.path().join("absent.zip"));
    match prepare(&options) {
        Err(PrepError::ArchiveRead {
            family,
            path,
            detail,
        }) => {
            assert_eq!(family, "cargo");
            assert!(path.ends_with("absent.zip"), "{path}");
            assert!(!detail.is_empty(), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_plan_lists_one_archive_per_ecosystem_the_locks_need() {
    let workspace = workspace();
    assert_eq!(
        plan(&options(&workspace)).expect("plans"),
        vec![(
            "cargo",
            advisory_source("cargo").expect("source").to_owned()
        )]
    );
    assert_eq!(
        needed_families(workspace.path()).expect("reads"),
        vec!["cargo"]
    );
}

#[test]
fn a_download_that_cannot_run_fails_with_the_program_it_needed() {
    match fetch(
        "file:///dx/absent/all.zip",
        Path::new("/tmp/dx/all.zip"),
        DEFAULT_LIMITS,
    ) {
        Err(PrepError::Fetch { program, detail }) => {
            assert_eq!(program, "curl");
            assert!(!detail.is_empty(), "{detail}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_workspace_without_audited_locks_prepares_an_explicit_empty_status() {
    let dir = tempfile::tempdir().expect("temp workspace");
    let mut options = options(&workspace());
    options.workspace = dir.path().to_path_buf();
    options.out = dir.path().join(DEFAULT_OUT);
    let prepared = prepare(&options).expect("empty preparation succeeds");
    assert!(prepared.is_empty());
    assert!(!options.out.exists(), "no snapshot is written");
    let summary = Summary::of(&prepared);
    assert_eq!(summary.status, "empty");
    assert_eq!(summary.render(), "status=empty\necosystems=0\nfamilies=\n");
}

#[test]
fn one_required_ecosystem_is_never_classified_as_empty() {
    let workspace = workspace();
    let options = with_archive(
        &workspace,
        options(&workspace),
        &archive(&[Fixture::stored(
            "all/1/GHSA-a.json",
            &advisory("GHSA-a", "aaa"),
        )]),
    );
    let summary = Summary::of(&prepare(&options).expect("prepares"));
    assert_eq!(summary.status, "prepared");
    assert_eq!(
        summary.render(),
        "status=prepared\necosystems=1\nfamilies=cargo\n"
    );
}

#[test]
fn a_required_ecosystem_with_a_corrupt_archive_is_an_error_not_empty() {
    let workspace = workspace();
    let options = with_archive(&workspace, options(&workspace), b"not a zip");
    assert!(prepare(&options).is_err());
}

#[test]
fn configured_sets_add_their_advisory_family_once() {
    let workspace = workspace();
    for (rel, text) in [
        (
            "dx.toml",
            "schema_version = 1\n\n[[dependency_set]]\nname = \"api\"\necosystem = \"uv\"\nmanifests = [\"api/pyproject.toml\"]\nlocks = [\"api/uv.lock\"]\nscopes = [\"api\"]\n\n[[dependency_set]]\nname = \"web\"\necosystem = \"npm\"\nmanifests = [\"web/package.json\"]\nlocks = [\"web/pnpm-lock.yaml\"]\nscopes = [\"web\"]\n\n[[dependency_set]]\nname = \"docs\"\necosystem = \"npm\"\nmanifests = [\"docs/package.json\"]\nlocks = [\"docs/pnpm-lock.yaml\"]\nscopes = [\"docs\"]\n",
        ),
        ("api/pyproject.toml", "[project]\nname = \"api\"\n"),
        ("api/uv.lock", "version = 1\n"),
        ("web/package.json", "{}\n"),
        ("web/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
        ("docs/package.json", "{}\n"),
        ("docs/pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
    ] {
        let path = workspace.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("creates");
        std::fs::write(&path, text).expect("writes");
    }
    let families: Vec<&str> = plan(&options(&workspace))
        .expect("plans")
        .into_iter()
        .map(|(family, _)| family)
        .collect();
    assert_eq!(families, ["cargo", "npm", "pypi"]);
}

fn planned_families(files: &[(&str, &str)]) -> Vec<&'static str> {
    let workspace = workspace();
    for (rel, text) in files {
        let path = workspace.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("creates");
        std::fs::write(&path, text).expect("writes");
    }
    plan(&options(&workspace))
        .expect("plans")
        .into_iter()
        .map(|(family, _)| family)
        .collect()
}

const RUBY_SET: &str = "[[dependency_set]]\nname = \"gems\"\necosystem = \"ruby\"\nmanifests = [\"rb/Gemfile\"]\nlocks = [\"rb/Gemfile.lock\"]\nscopes = [\"rb\"]\n";

const POWERSHELL_SET: &str = "[[dependency_set]]\nname = \"modules\"\necosystem = \"powershell\"\nmanifests = [\"ps/PSGallery.requirements.psd1\"]\nlocks = [\"ps/PSGallery.lock.json\"]\nscopes = [\"ps\"]\n";

#[test]
fn configured_ruby_sets_plan_rubygems_and_powershell_sets_plan_nothing() {
    let both = format!("schema_version = 1\n\n{RUBY_SET}\n{POWERSHELL_SET}");
    assert_eq!(
        planned_families(&[
            ("dx.toml", &both),
            ("rb/Gemfile", "source \"https://rubygems.org\"\n"),
            ("rb/Gemfile.lock", "GEM\n  specs:\n"),
            ("ps/PSGallery.requirements.psd1", "@{}\n"),
            ("ps/PSGallery.lock.json", "{}\n"),
        ]),
        ["cargo", "rubygems"]
    );
    let powershell_only = format!("schema_version = 1\n\n{POWERSHELL_SET}");
    assert_eq!(
        planned_families(&[
            ("dx.toml", &powershell_only),
            ("ps/PSGallery.requirements.psd1", "@{}\n"),
            ("ps/PSGallery.lock.json", "{}\n"),
        ]),
        ["cargo"]
    );
    let ruby_without_lock = format!("schema_version = 1\n\n{RUBY_SET}");
    assert_eq!(
        planned_families(&[
            ("dx.toml", &ruby_without_lock),
            ("rb/Gemfile", "source \"https://rubygems.org\"\n"),
        ]),
        ["cargo"]
    );
}

#[test]
fn configured_cargo_sets_plan_the_cargo_family_once() {
    let text = "schema_version = 1\n\n[[dependency_set]]\nname = \"crates\"\necosystem = \"cargo\"\nmanifests = [\"rs/Cargo.toml\"]\nlocks = [\"rs/Cargo.lock\"]\nscopes = [\"rs\"]\nrepository = \"crates\"\n";
    assert_eq!(
        planned_families(&[
            ("dx.toml", text),
            ("rs/Cargo.toml", "[package]\nname = \"a\"\n"),
            ("rs/Cargo.lock", "version = 4\n"),
        ]),
        ["cargo"]
    );
}

#[test]
fn configured_maven_sets_plan_the_maven_family_once() {
    let set = |name: &str, lock: &str, repository: &str| {
        format!("[[dependency_set]]\nname = \"{name}\"\necosystem = \"maven\"\nmanifests = [\"MODULE.bazel\"]\nlocks = [\"{lock}\"]\nscopes = [\"{lock}\"]\nrepository = \"{repository}\"\n")
    };
    let text = format!(
        "schema_version = 1\n\n{}\n{}",
        set("jvm", "jvm/maven_install.json", "maven"),
        set("tools", "tools/tool_install.json", "tool_maven")
    );
    assert_eq!(
        planned_families(&[
            ("dx.toml", &text),
            ("MODULE.bazel", "module(name = \"a\")\n"),
            ("jvm/maven_install.json", "{}\n"),
            ("tools/tool_install.json", "{}\n"),
        ]),
        ["cargo", "maven"]
    );
}

#[cfg(unix)]
fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[cfg(unix)]
fn two_archives() -> (Vec<u8>, Vec<u8>) {
    (
        archive(&[Fixture::stored(
            "all/1/GHSA-a.json",
            &advisory("GHSA-a", "aaa"),
        )]),
        archive(&[
            Fixture::stored("all/1/GHSA-a.json", &advisory("GHSA-a", "aaa")),
            Fixture::stored("all/2/GHSA-b.json", &advisory("GHSA-b", "bbb")),
        ]),
    )
}

#[cfg(unix)]
fn prepare_from_url(workspace: &TempDir, body: &[u8]) -> Prepared {
    let source = workspace.path().join("source.zip");
    std::fs::write(&source, body).expect("writes the source");
    let options = options(workspace);
    prepare_family(&options, "cargo", &file_url(&source)).expect("prepares")
}

#[cfg(unix)]
#[test]
fn concurrent_downloads_each_read_their_own_archive() {
    let (one, two) = two_archives();
    for _ in 0..20 {
        let (first, second) = (workspace(), workspace());
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| prepare_from_url(&first, &one));
            let b = scope.spawn(|| prepare_from_url(&second, &two));
            (a.join().expect("first"), b.join().expect("second"))
        });
        assert_eq!(a.advisories, 1);
        assert_eq!(b.advisories, 2);
        for (prepared, dir) in [(a, &first), (b, &second)] {
            let bytes = std::fs::read(dir.path().join(DEFAULT_OUT).join("cargo.json"))
                .expect("reads the snapshot");
            assert_eq!(prepared.identity, dx_digest::sha256_hex(&bytes));
        }
    }
}

#[cfg(unix)]
struct Planted(std::path::PathBuf);

#[cfg(unix)]
impl Drop for Planted {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn a_planted_file_at_the_old_temp_path_has_no_effect() {
    let old = std::env::temp_dir().join("dx-advisory-cargo.zip");
    let _ = std::fs::remove_file(&old);
    let _guard = Planted(old.clone());
    let (one, _) = two_archives();

    std::fs::write(&old, b"junk").expect("plants a file");
    let prepared = prepare_from_url(&workspace(), &one);
    assert_eq!(prepared.advisories, 1);
    assert_eq!(std::fs::read(&old).expect("still there"), b"junk");

    std::fs::remove_file(&old).expect("clears the plant");
    let elsewhere = tempfile::tempdir().expect("temp");
    let target = elsewhere.path().join("victim");
    std::fs::write(&target, b"victim").expect("writes the target");
    std::os::unix::fs::symlink(&target, &old).expect("plants a symlink");
    let prepared = prepare_from_url(&workspace(), &one);
    assert_eq!(prepared.advisories, 1);
    assert_eq!(std::fs::read(&target).expect("untouched"), b"victim");
}
