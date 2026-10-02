use super::*;

#[cfg(windows)]
fn test_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(windows))]
fn test_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn plan(bin_name: &str, owner: &str) -> ToolPlan {
    ToolPlan {
        bin_name: bin_name.to_string(),
        owner: owner.to_string(),
        host_names: vec![bin_name.to_string()],
    }
}

fn write_staged(root: &Path, tools: &[(&str, &str, &[&str])]) -> (PathBuf, PathBuf, String) {
    let targets = root.join("targets");
    let staged_bin = root.join("staged");
    fs::create_dir_all(&targets).expect("create targets");
    fs::create_dir_all(&staged_bin).expect("create staged bin");
    let mut entries = Vec::new();
    for (bin_name, owner, hosts) in tools {
        let real = targets.join(bin_name);
        fs::write(&real, format!("#!/bin/sh\necho {bin_name}\n")).expect("write target");
        for host in *hosts {
            let _ = fs::remove_file(staged_bin.join(host));
            test_symlink(&real, &staged_bin.join(host)).expect("stage link");
        }
        let host_list = hosts
            .iter()
            .map(|host| format!("\"{host}\""))
            .collect::<Vec<_>>()
            .join(",");
        entries.push(format!(
            "{{\"bin_name\":\"{bin_name}\",\"owner\":\"{owner}\",\"host_names\":[{host_list}]}}"
        ));
    }
    let text = format!(
        "{{\"schema_version\":{STAGED_METADATA_SCHEMA_VERSION},\"tools\":[{}]}}",
        entries.join(",")
    );
    let metadata_path = root.join("staged.metadata.json");
    fs::write(&metadata_path, &text).expect("write metadata");
    (staged_bin, metadata_path, text)
}

fn options(root: &Path, staged_bin: PathBuf, metadata: PathBuf) -> RefreshOptions {
    fs::create_dir_all(root.join("ws")).expect("create workspace");
    RefreshOptions {
        workspace_root: root.join("ws"),
        staged_bin,
        staged_metadata: metadata,
        os: "linux",
        lock_timeout: Duration::from_secs(10),
    }
}

fn refresh_ok(options: &RefreshOptions) -> RefreshOutcome {
    refresh(options, &probe_symlink).expect("refresh succeeds")
}

#[test]
fn canonical_identity_is_order_independent() {
    let forward = vec![
        plan("alpha", "//env:tool_alpha"),
        plan("beta", "//env:tool_beta"),
    ];
    let reverse = vec![
        plan("beta", "//env:tool_beta"),
        plan("alpha", "//env:tool_alpha"),
    ];
    assert_eq!(
        canonical_identity_bytes(&forward),
        canonical_identity_bytes(&reverse)
    );
    let different = vec![plan("alpha", "//env:tool_alpha")];
    assert_ne!(
        canonical_identity_bytes(&forward),
        canonical_identity_bytes(&different)
    );
}

#[test]
fn identity_hex_is_lowercase_64() {
    let hex = identity_hex(&[plan("alpha", "//env:tool_alpha")]);
    assert_eq!(hex.len(), 64);
    assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(hex, hex.to_lowercase());
}

#[test]
fn marker_roundtrip() {
    let tools = vec![plan("alpha", "//env:tool_alpha")];
    let identity = identity_digest(&canonical_identity_bytes(&tools));
    assert_eq!(
        decode_marker(&encode_marker(&identity)).expect("decode"),
        identity
    );
}

#[test]
fn marker_rejects_garbage() {
    assert!(matches!(
        decode_marker(b"definitely not a marker"),
        Err(Error::MarkerInvalid { .. })
    ));
}

#[test]
fn marker_rejects_schema() {
    let bytes = EnvMarker {
        schema_version: 999,
        identity: vec![0u8; IDENTITY_LEN],
    }
    .encode_to_vec();
    assert_eq!(
        decode_marker(&bytes),
        Err(Error::UnsupportedMarkerSchema { found: 999 })
    );
}

#[test]
fn marker_rejects_short_identity() {
    let bytes = EnvMarker {
        schema_version: MARKER_SCHEMA_VERSION,
        identity: vec![7u8; 4],
    }
    .encode_to_vec();
    assert!(matches!(
        decode_marker(&bytes),
        Err(Error::MarkerInvalid { .. })
    ));
}

#[test]
fn parse_staged_ok() {
    let scratch = dx_test_scratch::scratch("dx-env-test-parse-ok-");
    let root = scratch.path().to_path_buf();
    let (_, _, text) = write_staged(
        &root,
        &[
            ("alpha", "//env:tool_alpha", &["alpha", "a"]),
            ("beta", "//env:tool_beta", &["beta"]),
        ],
    );
    let plans = parse_staged(&text).expect("parse");
    assert_eq!(plans.len(), 2);
    assert_eq!(
        plans[0].host_names,
        vec!["alpha".to_string(), "a".to_string()]
    );
    assert_eq!(plans[1].owner, "//env:tool_beta".to_string());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn parse_staged_rejects_syntax_and_shape() {
    assert!(matches!(parse_staged("{oops"), Err(Error::Staged { .. })));
    assert!(matches!(
        parse_staged(r#"{"schema_version":1}"#),
        Err(Error::Staged { .. })
    ));
}

#[test]
fn parse_staged_rejects_schema() {
    assert_eq!(
        parse_staged(r#"{"schema_version":2,"tools":[]}"#),
        Err(Error::UnsupportedStagedSchema { found: 2 })
    );
}

#[test]
fn parse_staged_rejects_empty_set() {
    assert!(matches!(
        parse_staged(r#"{"schema_version":1,"tools":[]}"#),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn parse_staged_rejects_bad_records() {
    let empty_bin =
        r#"{"schema_version":1,"tools":[{"bin_name":"","owner":"o","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_bin),
        Err(Error::InvalidTool { .. })
    ));
    let empty_owner =
        r#"{"schema_version":1,"tools":[{"bin_name":"a","owner":"","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_owner),
        Err(Error::InvalidTool { .. })
    ));
    let no_hosts = r#"{"schema_version":1,"tools":[{"bin_name":"a","owner":"o","host_names":[]}]}"#;
    assert!(matches!(
        parse_staged(no_hosts),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn host_name_table() {
    for valid in ["a", "doctor-2", "x.y", "my_tool"] {
        let text = serde_json::json!({
            "schema_version": 1,
            "tools": [{"bin_name": "t", "owner": "o", "host_names": [valid]}],
        })
        .to_string();
        assert!(
            parse_staged(&text).is_ok(),
            "valid host name rejected: {valid}"
        );
    }
    for invalid in [
        "", ".", "..", "a/b", "a\\b", "run.exe", "RUN.BAT", "x.cmd", "y.com", "con", "aux.txt",
        "COM1", "lpt9", "NUL",
    ] {
        let text = serde_json::json!({
            "schema_version": 1,
            "tools": [{"bin_name": "t", "owner": "o", "host_names": [invalid]}],
        })
        .to_string();
        assert!(
            matches!(parse_staged(&text), Err(Error::InvalidTool { .. })),
            "invalid host name accepted: {invalid:?}"
        );
    }
}

#[test]
fn error_display_names_every_variant() {
    let dir = PathBuf::from("/ws/.dx/bin");
    let errors = [
        Error::WorkspaceRoot {
            path: PathBuf::from("/ws"),
        },
        Error::Staged {
            reason: "r".to_string(),
        },
        Error::UnsupportedStagedSchema { found: 2 },
        Error::InvalidTool {
            reason: "r".to_string(),
        },
        Error::Unmanaged {
            path: dir.clone(),
            detail: "d".to_string(),
        },
        Error::UnsupportedMarkerSchema { found: 3 },
        Error::MarkerInvalid {
            reason: "r".to_string(),
        },
        Error::Busy { path: dir.clone() },
        Error::LockFailed {
            path: dir.clone(),
            reason: "r".to_string(),
        },
        Error::SymlinkUnsupported {
            detail: "d".to_string(),
        },
        Error::Install {
            reason: "r".to_string(),
        },
    ];
    for error in &errors {
        assert!(!format!("{error}").is_empty());
    }
    assert!(matches!(
        &errors[4],
        Error::Unmanaged { detail, .. } if detail == "d"
    ));
    assert!(matches!(
        &errors[5],
        Error::UnsupportedMarkerSchema { found } if *found == 3
    ));
    assert!(matches!(&errors[7], Error::Busy { path } if path == &dir));
}

#[test]
fn acquire_lock_reports_only_the_two_lock_failures() {
    let dir = PathBuf::from("/ws/.dx");
    let busy = LockError::Busy {
        path: dir.join(LOCK_FILE_NAME),
    };
    let failed = LockError::LockFailed {
        path: dir.join(LOCK_FILE_NAME),
        reason: "r".to_string(),
    };
    for error in [&busy, &failed] {
        let text = error.to_string();
        assert!(text.contains(".commit.lock"), "{text}");
    }
    assert!(matches!(
        Error::from(busy),
        Error::Busy { path } if path == PathBuf::from("/ws/.dx/.commit.lock")
    ));
    assert!(matches!(
        Error::from(failed),
        Error::LockFailed { path, reason }
            if path == PathBuf::from("/ws/.dx/.commit.lock") && reason == "r"
    ));
}

#[test]
fn workspace_missing() {
    let scratch = dx_test_scratch::scratch("dx-env-test-ws-missing-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let mut opts = options(&root, staged_bin, metadata);
    opts.workspace_root = root.join("no-such-dir");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::WorkspaceRoot { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn staged_inputs_fail_before_workspace_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-staged-fail-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let missing_metadata = options(&root, staged_bin.clone(), root.join("nope.json"));
    assert!(matches!(
        refresh(&missing_metadata, &probe_symlink),
        Err(Error::Staged { .. })
    ));
    let missing_bin = options(&root, root.join("nope"), metadata);
    assert!(matches!(
        refresh(&missing_bin, &probe_symlink),
        Err(Error::Staged { .. })
    ));
    assert!(!root.join("ws").join(".dx").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn staged_rejects_non_link_and_dangling() {
    let scratch = dx_test_scratch::scratch("dx-env-test-staged-links-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, text) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    fs::remove_file(staged_bin.join("a")).expect("remove link");
    fs::write(staged_bin.join("a"), "regular file, not a link").expect("clobber link");
    let opts = options(&root, staged_bin.clone(), metadata.clone());
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Staged { .. })
    ));
    fs::remove_file(staged_bin.join("a")).expect("remove clobber");
    test_symlink(&root.join("dangling-target"), &staged_bin.join("a")).expect("dangling link");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Staged { .. })
    ));
    assert!(parse_staged(&text).is_ok());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn fresh_install_noop_and_replacement() {
    let scratch = dx_test_scratch::scratch("dx-env-test-lifecycle-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("alpha", "//o:alpha", &["alpha", "a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let bin = root.join("ws").join(".dx").join("bin");
    assert_eq!(
        fs::read_link(bin.join("a")).expect("alias link"),
        fs::canonicalize(root.join("targets").join("alpha")).expect("canonical target")
    );
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    let (staged_bin, metadata, _) = write_staged(
        &root,
        &[
            ("alpha", "//o:alpha", &["alpha"]),
            ("beta", "//o:beta", &["beta"]),
        ],
    );
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert!(!bin.join("a").exists());
    assert!(bin.join("beta").exists());
    assert!(!root.join("ws").join(".dx").join("bin.prev").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn workspace_path_with_spaces_installs() {
    let scratch = dx_test_scratch::scratch("dx-env-test-with space-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let link = root.join("ws").join(".dx").join("bin").join("a");
    let target = fs::canonicalize(root.join("targets").join("a")).expect("canonical target");
    assert_eq!(fs::read_link(&link).expect("tool link"), target);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).expect("chmod target");
    }
    let output = std::process::Command::new(&link)
        .output()
        .expect("run installed tool");
    assert!(output.status.success());
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn already_current_cleans_stale_staging() {
    let scratch = dx_test_scratch::scratch("dx-env-test-noop-clean-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let stage_dir = root.join("ws").join(".dx").join("bin.next");
    fs::create_dir_all(&stage_dir).expect("leftover staging");
    fs::write(stage_dir.join("junk"), "junk").expect("junk");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    assert!(!stage_dir.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unmanaged_states_refuse_without_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-unmanaged-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(&dx).expect("dx dir");

    fs::write(dx.join("bin"), "not a directory").expect("file bin");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Unmanaged { .. })
    ));
    assert_eq!(
        fs::read(dx.join("bin")).expect("untouched"),
        b"not a directory".to_vec()
    );
    fs::remove_file(dx.join("bin")).expect("remove file bin");

    test_symlink(&root.join("elsewhere"), &dx.join("bin")).expect("symlink bin");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Unmanaged { .. })
    ));
    fs::remove_file(dx.join("bin")).expect("remove symlink bin");

    fs::create_dir_all(dx.join("bin")).expect("bare bin dir");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Unmanaged { .. })
    ));

    fs::write(dx.join("bin").join(MARKER_FILE_NAME), b"garbage").expect("garbage marker");
    let error = refresh(&opts, &probe_symlink).unwrap_err();
    assert!(matches!(error, Error::Unmanaged { .. }));
    assert!(matches!(
        error,
        Error::Unmanaged { detail, .. } if detail.contains("not ours")
    ));

    fs::remove_file(dx.join("bin").join(MARKER_FILE_NAME)).expect("remove garbage");
    fs::create_dir_all(dx.join("bin").join(MARKER_FILE_NAME)).expect("marker dir");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Unmanaged { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn stale_prev_undeletable_reports_install() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-prev-perms-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Ok(RefreshOutcome::InstalledFresh)
    ));
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(dx.join(PREV_DIR_NAME)).expect("stale prev");
    fs::set_permissions(&dx, fs::Permissions::from_mode(0o555)).expect("read-only dx");
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale")
    ));
    fs::set_permissions(&dx, fs::Permissions::from_mode(0o755)).expect("writable dx");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn stale_stage_undeletable_reports_install() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-stage-perms-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(&dx).expect("dx dir");
    fs::write(dx.join(LOCK_FILE_NAME), b"").expect("lock file");
    fs::create_dir_all(dx.join(STAGE_DIR_NAME)).expect("stale stage");
    fs::set_permissions(&dx, fs::Permissions::from_mode(0o555)).expect("read-only dx");
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale staging")
    ));
    fs::set_permissions(&dx, fs::Permissions::from_mode(0o755)).expect("writable dx");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn duplicate_host_name_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-dup-host-");
    let root = scratch.path().to_path_buf();
    let target_a = root.join("tool-a");
    let target_b = root.join("tool-b");
    fs::write(&target_a, b"a").expect("target a");
    fs::write(&target_b, b"b").expect("target b");
    let error = stage_tree(
        &root.join("stage"),
        &[("dup".to_string(), target_a), ("dup".to_string(), target_b)],
        &[7u8; 32],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot stage host name 'dup'")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn newer_marker_refuses_with_upgrade_guidance() {
    let scratch = dx_test_scratch::scratch("dx-env-test-newer-marker-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let bin = root.join("ws").join(".dx").join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    let bytes = EnvMarker {
        schema_version: 999,
        identity: vec![0u8; IDENTITY_LEN],
    }
    .encode_to_vec();
    fs::write(bin.join(MARKER_FILE_NAME), bytes).expect("newer marker");
    assert_eq!(
        refresh(&opts, &probe_symlink),
        Err(Error::UnsupportedMarkerSchema { found: 999 })
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn crash_between_renames_restores_then_replaces() {
    let scratch = dx_test_scratch::scratch("dx-env-test-crash-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("old", "//o:old", &["old"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let dx = root.join("ws").join(".dx");
    fs::rename(dx.join("bin"), dx.join("bin.prev")).expect("simulate crash");
    let (staged_bin, metadata, _) = write_staged(&root, &[("new", "//o:new", &["new"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert!(dx.join("bin").join("new").exists());
    assert!(!dx.join("bin").join("old").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stale_prev_is_cleared() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-prev-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let prev = root.join("ws").join(".dx").join("bin.prev");
    fs::create_dir_all(&prev).expect("stale prev");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    assert!(!prev.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn busy_lock_fails_after_deadline() {
    let scratch = dx_test_scratch::scratch("dx-env-test-busy-");
    let root = scratch.path().to_path_buf();
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(&dx).expect("dx dir");
    let _held = acquire_lock(&dx, Duration::from_secs(10)).expect("setup lock");
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let mut opts = options(&root, staged_bin, metadata);
    opts.lock_timeout = Duration::from_millis(1);
    let error = refresh(&opts, &probe_symlink).unwrap_err();
    assert!(matches!(error, Error::Busy { .. }));
    drop(_held);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn lock_open_failure_aborts() {
    let scratch = dx_test_scratch::scratch("dx-env-test-lock-open-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(&dx).expect("dx dir");
    fs::create_dir_all(dx.join(LOCK_FILE_NAME)).expect("lock is a directory");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::LockFailed { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn dx_creation_failure_aborts() {
    let scratch = dx_test_scratch::scratch("dx-env-test-dx-create-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    fs::create_dir_all(root.join("ws")).expect("ws dir");
    fs::write(root.join("ws").join(".dx"), "file blocks dir").expect("file dx");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Install { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn probe_failure_reports_before_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-fail-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let failing = |_: &Path| -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
    };
    let mut opts = options(&root, staged_bin.clone(), metadata.clone());
    opts.os = "windows";
    let error = refresh(&opts, &failing).unwrap_err();
    assert!(matches!(error, Error::SymlinkUnsupported { .. }));
    assert!(matches!(
        error,
        Error::SymlinkUnsupported { detail } if detail.contains("Developer Mode")
    ));
    opts.os = "other";
    let error = refresh(&opts, &failing).unwrap_err();
    assert!(matches!(
        error,
        Error::SymlinkUnsupported { detail } if detail.contains("no changes were made")
    ));
    assert!(!root.join("ws").join(".dx").join("bin").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_probe_accepts_writable_dir() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-ok-");
    let root = scratch.path().to_path_buf();
    probe_symlink(&root).expect("probe succeeds");
    assert!(!root.join("symlink.probe").exists());
    let _ = fs::remove_dir_all(&root);
}
