#[test]
fn archive_lists_extracts_and_hashes_in_process() {
    let archive = dx_testing::env_file("DX_ARCHIVE");
    let suffix = dx_testing::env_text("DX_ARCHIVE_SUFFIX");
    assert!(
        archive.to_string_lossy().ends_with(&format!(".{suffix}")),
        "{} must end with .{suffix}",
        archive.display()
    );
    let checksum = std::fs::read_to_string(dx_testing::env_file("DX_ARCHIVE_CHECKSUM"))
        .expect("read checksum");
    let bytes = std::fs::read(&archive).expect("read archive");
    assert_eq!(
        checksum.split_whitespace().next(),
        Some(dx_digest::sha256_hex(&bytes).as_str())
    );
    let bare = dx_testing::env_text("DX_ARCHIVE_APP");
    let app = format!("{bare}{}", std::env::consts::EXE_SUFFIX);
    let runtime = dx_testing::env_text("DX_ARCHIVE_RUNTIME");
    let mut want: Vec<String> = dx_testing::env_text("DX_ARCHIVE_MEMBERS")
        .split(',')
        .map(|member| {
            if member == bare {
                app.clone()
            } else {
                member.to_owned()
            }
        })
        .collect();
    want.sort();
    let listed = dx_deploy_tools::archive_listing(&archive).expect("listing");
    let names: Vec<String> = listed.iter().map(|entry| entry.0.clone()).collect();
    assert_eq!(names, want);
    for (name, mode, _) in &listed {
        let expected = if *name == app { 0o755 } else { 0o644 };
        assert_eq!(*mode & 0o7777, expected, "{name}");
    }
    let scratch_dir = dx_test_scratch::scratch("consumer-extract");
    let scratch = scratch_dir.path();
    let dir = scratch.join("dx install \u{e9} \u{fc}");
    let extracted = dx_deploy_tools::archive_extract(&archive, &dir).expect("extract");
    assert_eq!(extracted, listed);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(dir.join(&app))
            .expect("app meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o7777, 0o755);
    }
    if !runtime.is_empty() {
        let body = std::fs::read_to_string(dir.join(&runtime)).expect("runtime member");
        assert!(body.contains("dx-consumer-runtime-marker"), "{body}");
    }
    let identity = std::fs::read_to_string(dir.join("IDENTITY.txt")).expect("IDENTITY");
    let records: Vec<&str> = identity
        .lines()
        .filter_map(|line| line.strip_prefix("runtime: "))
        .collect();
    let want_records: Vec<String> = std::env::var("DX_ARCHIVE_RUNTIME_RECORDS")
        .unwrap_or_default()
        .split(',')
        .filter(|record| !record.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(records, want_records);
    let notice = std::fs::read_to_string(dir.join("NOTICE")).expect("NOTICE");
    assert!(notice.contains("consumer-release NOTICE"), "{notice}");
}
