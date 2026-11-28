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
    write_checks(out);
}

fn write_checks(out: &Path) {
    let policy = out.join(CACHE_DIR).join("policy");
    std::fs::create_dir_all(&policy).expect("policy dir");
    std::fs::write(policy.join("metadata.json"), "{\"MajorVersion\":2}").expect("checks");
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
    assert_eq!(calls[0][2..], ["image", "--download-db-only"]);
    assert_eq!(calls[1][2], "config");
    let meta = std::fs::read_to_string(out.path().join(META_FILE)).expect("sidecar");
    assert_eq!(
        meta,
        "{\"retrieved_at\":\"2026-11-05\",\"db_updated_at\":\"2026-11-05T06:00:00.5Z\"}\n"
    );
}

#[test]
fn the_checks_bundle_is_fetched_by_a_config_scan_of_an_empty_directory() {
    let [db, checks] = download_args(Path::new("cache"), Path::new("empty"));
    assert_eq!(db, ["--cache-dir", "cache", "image", "--download-db-only"]);
    assert_eq!(
        checks,
        [
            "--cache-dir",
            "cache",
            "config",
            "--format",
            "json",
            "--exit-code",
            "0",
            "--disable-telemetry",
            "--skip-version-check",
            "empty",
        ]
    );
}

#[test]
fn the_scanned_directory_is_empty() {
    let out = tempfile::tempdir().expect("tempdir");
    prepare_with(out.path(), "2026-11-05", |args| {
        if let Some(dir) = args.last().filter(|_| args[2] == "config") {
            let entries = std::fs::read_dir(dir).expect("scan dir").count();
            assert_eq!(entries, 0);
        }
        write_db(out.path());
        Ok(())
    })
    .expect("prepared");
}

#[test]
fn a_fallback_to_embedded_checks_is_reported() {
    let out = tempfile::tempdir().expect("tempdir");
    write_db(out.path());
    let error = prepare_with(out.path(), "2026-11-05", |_| {
        let db = out.path().join(CACHE_DIR).join("db");
        std::fs::write(db.join("metadata.json"), "{\"UpdatedAt\":\"now\"}").expect("metadata");
        Ok(())
    })
    .expect_err("fails");
    assert!(error.to_string().contains("checks bundle"), "{error}");
    assert!(!out.path().join(META_FILE).exists());
}

#[test]
fn a_same_day_sidecar_without_the_checks_bundle_downloads_again() {
    let out = tempfile::tempdir().expect("tempdir");
    prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("first");
    std::fs::remove_dir_all(out.path().join(CACHE_DIR).join("policy")).expect("removes checks");
    let again = prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("second");
    assert!(again.fetched);
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
    let next = prepare_with(out.path(), "2026-11-06", |_| {
        write_checks(out.path());
        Ok(())
    })
    .expect("next");
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
fn metadata_without_a_string_updated_at_is_reported() {
    for metadata in ["{ \"Version\" : 2 }", "{\"UpdatedAt\":1}", "not json"] {
        let out = tempfile::tempdir().expect("tempdir");
        let error = prepare_with(out.path(), "2026-11-05", |_| {
            let db = out.path().join(CACHE_DIR).join("db");
            std::fs::create_dir_all(&db).expect("db dir");
            std::fs::write(db.join("metadata.json"), metadata).expect("metadata");
            Ok(())
        })
        .expect_err("fails");
        assert!(error.to_string().contains("metadata.json"), "{error}");
        assert!(!out.path().join(META_FILE).exists());
    }
}

#[test]
fn a_spaced_sidecar_and_spaced_metadata_are_read() {
    let out = tempfile::tempdir().expect("tempdir");
    write_db(out.path());
    std::fs::write(
        out.path().join(META_FILE),
        "{ \"db_updated_at\" : \"then\", \"retrieved_at\" : \"2026-11-05\" }",
    )
    .expect("sidecar");
    let kept = prepare_with(out.path(), "2026-11-05", |_| {
        panic!("a same-day run must not download")
    })
    .expect("kept");
    assert_eq!(kept.db_updated_at, "then");
    let next = prepare_with(out.path(), "2026-11-06", |_| {
        let db = out.path().join(CACHE_DIR).join("db");
        std::fs::create_dir_all(&db).expect("db dir");
        std::fs::write(db.join("metadata.json"), "{ \"UpdatedAt\" : \"now\" }").expect("metadata");
        write_checks(out.path());
        Ok(())
    })
    .expect("next");
    assert_eq!(next.db_updated_at, "now");
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

#[test]
fn a_same_day_sidecar_without_its_cache_downloads_again() {
    let out = tempfile::tempdir().expect("tempdir");
    prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("first");
    std::fs::remove_dir_all(out.path().join(CACHE_DIR)).expect("removes the cache");
    let again = prepare_with(out.path(), "2026-11-05", |_| {
        write_db(out.path());
        Ok(())
    })
    .expect("second");
    assert!(again.fetched);
    assert!(
        out.path()
            .join(CACHE_DIR)
            .join("db/metadata.json")
            .is_file()
    );
}

#[test]
fn packing_a_missing_cache_names_the_cache() {
    let out = tempfile::tempdir().expect("tempdir");
    let cache = out.path().join("absent-cache");
    let error =
        pack_cache(&cache, &out.path().join("pin").join("trivy.tar.gz")).expect_err("fails");
    assert!(
        error.to_string().contains(&cache.display().to_string()),
        "{error}"
    );
}
