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
        assert_eq!(records, [want.as_str()], "{}: {identity}", archive.display());
    }
}
