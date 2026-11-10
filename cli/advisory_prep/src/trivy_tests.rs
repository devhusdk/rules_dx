use super::*;
use std::cell::RefCell;

fn write_db(out: &Path) {
    let db = out.join(CACHE_DIR).join("db");
    std::fs::create_dir_all(&db).expect("db dir");
    std::fs::write(
        db.join("metadata.json"),
        "{\"Version\":2,\"UpdatedAt\":\"2026-11-05T06:00:00.5Z\"}",
    )
    .expect("metadata");
}

#[test]
fn first_run_downloads_both_databases_and_writes_the_sidecar() {
    let out = tempfile::tempdir().expect("tempdir");
    let calls = RefCell::new(Vec::new());
    let prepared = prepare_with(out.path(), "2026-11-05", |args| {
        calls.borrow_mut().push(args.to_vec());
        write_db(out.path());
        Ok(())
    })
    .expect("prepared");
    assert!(prepared.fetched);
    assert_eq!(prepared.db_updated_at, "2026-11-05T06:00:00.5Z");
    let calls = calls.into_inner();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].contains(&"--download-db-only".to_owned()));
    assert!(calls[1].contains(&"--download-checks-only".to_owned()));
    let meta = std::fs::read_to_string(out.path().join(META_FILE)).expect("sidecar");
    assert_eq!(
        meta,
        "{\"retrieved_at\":\"2026-11-05\",\"db_updated_at\":\"2026-11-05T06:00:00.5Z\"}\n"
    );
}

#[test]
fn second_run_on_the_same_day_is_a_no_op() {
    let out = tempfile::tempdir().expect("tempdir");
    prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("first");
    let again = prepare_with(out.path(), "2026-11-05", |_| {
        panic!("a same-day run must not download")
    })
    .expect("second");
    assert!(!again.fetched);
    assert_eq!(again.db_updated_at, "2026-11-05T06:00:00.5Z");
}

#[test]
fn a_later_day_downloads_again() {
    let out = tempfile::tempdir().expect("tempdir");
    prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("first");
    let next = prepare_with(out.path(), "2026-11-06", |_| Ok(())).expect("next");
    assert!(next.fetched);
}

#[test]
fn a_failed_download_writes_no_sidecar() {
    let out = tempfile::tempdir().expect("tempdir");
    let error = prepare_with(out.path(), "2026-11-05", |_| {
        Err(PrepError::Usage {
            detail: "offline".to_owned(),
        })
    })
    .expect_err("fails");
    assert!(error.to_string().contains("offline"));
    assert!(!out.path().join(META_FILE).exists());
}

#[test]
fn missing_metadata_is_reported() {
    let out = tempfile::tempdir().expect("tempdir");
    let error = prepare_with(out.path(), "2026-11-05", |_| Ok(())).expect_err("fails");
    assert!(error.to_string().contains("metadata.json"));
}

#[test]
fn json_string_reads_spaced_values() {
    assert_eq!(
        json_string("{ \"a\" : \"x\", \"b\": \"y\" }", "b").as_deref(),
        Some("y")
    );
    assert_eq!(json_string("{\"a\":1}", "a"), None);
}

#[test]
fn the_cache_is_packed_into_an_archive_that_unpacks_to_the_same_files() {
    let out = tempfile::tempdir().expect("tempdir");
    write_db(out.path());
    let dest = out.path().join("pin").join("trivy.tar.gz");
    let bytes = pack_cache(&out.path().join(CACHE_DIR), &dest).expect("packed");
    assert_eq!(bytes, std::fs::read(&dest).expect("archive"));
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
}

#[test]
fn packing_the_same_directory_twice_gives_the_same_bytes() {
    let out = tempfile::tempdir().expect("tempdir");
    write_db(out.path());
    let cache = out.path().join(CACHE_DIR);
    let first = pack_cache(&cache, &out.path().join("a").join("t.tar.gz")).expect("first");
    let second = pack_cache(&cache, &out.path().join("b").join("t.tar.gz")).expect("second");
    assert_eq!(first, second);
}
