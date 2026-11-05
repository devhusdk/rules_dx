use std::path::PathBuf;

fn archives() -> Vec<PathBuf> {
    let paths = std::env::var("DX_QT_ARCHIVES").expect("DX_QT_ARCHIVES must be set");
    paths
        .split_whitespace()
        .map(dx_testing::resolve_runfiles)
        .collect()
}

#[test]
fn identity_records_the_os_supplied_runtime() {
    let archives = archives();
    assert_eq!(archives.len(), 3, "{archives:?}");
    let want = std::env::var("DX_QT_RUNTIME_RECORD").expect("DX_QT_RUNTIME_RECORD must be set");
    let scratch = dx_testing::mkscratch("qt-consumer-identity").expect("scratch");
    for (index, archive) in archives.iter().enumerate() {
        let dir = scratch.join(index.to_string());
        dx_deploy_tools::archive_extract(archive, &dir).expect("extract");
        let identity = std::fs::read_to_string(dir.join("IDENTITY.txt")).expect("IDENTITY");
        let records: Vec<&str> = identity
            .lines()
            .filter_map(|line| line.strip_prefix("runtime: "))
            .collect();
        if want.is_empty() {
            assert!(records.is_empty(), "{}: {identity}", archive.display());
        } else {
            assert_eq!(records, [want.as_str()], "{}: {identity}", archive.display());
        }
    }
}

#[test]
fn ledger_runs_from_the_extracted_archive() {
    if std::env::var_os("DX_QT_RUN_LEDGER").is_none() {
        return;
    }
    let archive = archives()
        .into_iter()
        .find(|path| path.to_string_lossy().contains("ledger_release"))
        .expect("ledger_release archive");
    let scratch = dx_testing::mkscratch("qt-consumer-run").expect("scratch");
    let dir = scratch.join("ledger");
    dx_deploy_tools::archive_extract(&archive, &dir).expect("extract");
    let core = std::env::var("DX_QT_CORE_MEMBER").expect("DX_QT_CORE_MEMBER must be set");
    assert!(dir.join(&core).exists(), "{core} is not staged");

    let out = std::process::Command::new(dir.join("ledger"))
        .args(["5", "7"])
        .env_clear()
        .current_dir(&scratch)
        .output()
        .expect("run ledger");
    assert!(out.status.success(), "ledger failed: {out:?}");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "added 5\nadded 7\n2 entries, total 12, adler32 004c000d\n"
    );
}
