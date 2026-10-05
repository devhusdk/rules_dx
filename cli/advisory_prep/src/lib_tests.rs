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
