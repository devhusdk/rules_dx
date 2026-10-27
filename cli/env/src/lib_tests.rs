use super::*;

#[cfg(windows)]
fn test_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(windows))]
fn test_symlink(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn installed_target(link: &Path) -> PathBuf {
    fs::canonicalize(fs::read_link(link).expect("installed link")).expect("installed target")
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
        let alias_list = hosts
            .iter()
            .skip(1)
            .map(|host| format!("\"{host}\""))
            .collect::<Vec<_>>()
            .join(",");
        entries.push(format!(
            "{{\"bin_name\":\"{bin_name}\",\"owner\":\"{owner}\",\"aliases\":[{alias_list}],\"host_names\":[{host_list}]}}"
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
    let plans = parse_staged(&text, false).expect("parse");
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
    assert!(matches!(
        parse_staged("{oops", false),
        Err(Error::Staged { .. })
    ));
    assert!(matches!(
        parse_staged(r#"{"schema_version":1}"#, false),
        Err(Error::Staged { .. })
    ));
}

#[test]
fn parse_staged_rejects_schema() {
    assert_eq!(
        parse_staged(r#"{"schema_version":2,"tools":[]}"#, false),
        Err(Error::UnsupportedStagedSchema { found: 2 })
    );
}

#[test]
fn parse_staged_rejects_empty_set() {
    assert!(matches!(
        parse_staged(r#"{"schema_version":1,"tools":[]}"#, false),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn parse_staged_rejects_bad_records() {
    let empty_bin =
        r#"{"schema_version":1,"tools":[{"bin_name":"","owner":"o","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_bin, false),
        Err(Error::InvalidTool { .. })
    ));
    let empty_owner =
        r#"{"schema_version":1,"tools":[{"bin_name":"a","owner":"","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_owner, false),
        Err(Error::InvalidTool { .. })
    ));
    let no_hosts = r#"{"schema_version":1,"tools":[{"bin_name":"a","owner":"o","host_names":[]}]}"#;
    assert!(matches!(
        parse_staged(no_hosts, false),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn host_name_table() {
    for valid in ["a", "doctor-2", "x.y", "my_tool"] {
        let text = serde_json::json!({
            "schema_version": 1,
            "tools": [{"bin_name": valid, "owner": "o", "host_names": [valid]}],
        })
        .to_string();
        assert!(
            parse_staged(&text, false).is_ok(),
            "valid host name rejected: {valid}"
        );
    }
    for invalid in [
        ".",
        "..",
        "a/b",
        "a\\b",
        "run.exe",
        "RUN.BAT",
        "x.cmd",
        "y.com",
        "con",
        "aux.txt",
        "COM1",
        "lpt9",
        "NUL",
        MARKER_FILE_NAME,
        ".Rules_Dx_Managed",
    ] {
        let text = serde_json::json!({
            "schema_version": 1,
            "tools": [{"bin_name": invalid, "owner": "o", "host_names": [invalid]}],
        })
        .to_string();
        assert!(
            matches!(parse_staged(&text, false), Err(Error::InvalidTool { .. })),
            "invalid host name accepted: {invalid:?}"
        );
    }
}

fn tools_json(tools: serde_json::Value) -> String {
    serde_json::json!({"schema_version": 1, "tools": tools}).to_string()
}

#[test]
fn windows_host_names_carry_the_exe_suffix() {
    let text = tools_json(serde_json::json!([
        {"bin_name": "dx", "owner": "o", "aliases": ["d"], "host_names": ["dx.exe", "d.exe"]},
    ]));
    let plans = parse_staged(&text, true).expect("windows names accepted");
    assert_eq!(plans[0].host_names, vec!["dx.exe", "d.exe"]);
    assert!(matches!(
        parse_staged(&text, false),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn host_names_must_match_the_platform_mapping() {
    let bare = tools_json(serde_json::json!([
        {"bin_name": "dx", "owner": "o", "host_names": ["dx"]},
    ]));
    assert!(parse_staged(&bare, true).is_err());
    for wrong in ["dx.bat", "other.exe", "dx.exe.exe", "DX.EXE"] {
        let text = tools_json(serde_json::json!([
            {"bin_name": "dx", "owner": "o", "host_names": [wrong]},
        ]));
        assert!(parse_staged(&text, true).is_err(), "accepted {wrong}");
    }
    let missing_alias = tools_json(serde_json::json!([
        {"bin_name": "dx", "owner": "o", "aliases": ["d"], "host_names": ["dx.exe"]},
    ]));
    assert!(parse_staged(&missing_alias, true).is_err());
}

#[test]
fn logical_names_with_suffixes_are_rejected_on_windows() {
    let text = tools_json(serde_json::json!([
        {"bin_name": "dx.exe", "owner": "o", "host_names": ["dx.exe.exe"]},
    ]));
    assert!(parse_staged(&text, true).is_err());
}

#[test]
fn colliding_host_names_across_tools_are_rejected() {
    let text = tools_json(serde_json::json!([
        {"bin_name": "dx", "owner": "a", "host_names": ["dx.exe"]},
        {"bin_name": "DX", "owner": "b", "host_names": ["DX.exe"]},
    ]));
    assert!(parse_staged(&text, true).is_err());
}

#[test]
fn generated_metadata_parses_on_its_platform() {
    for (var, windows) in [("DX_POSIX_METADATA", false), ("DX_WINDOWS_METADATA", true)] {
        let rel = std::env::var(var).expect("generated metadata path");
        let text = fs::read_to_string(&rel).expect("read generated metadata");
        let plans = parse_staged(&text, windows).expect("generated metadata accepted");
        assert!(!plans.is_empty());
        let suffix_count = plans
            .iter()
            .flat_map(|plan| &plan.host_names)
            .filter(|name| name.ends_with(".exe"))
            .count();
        assert_eq!(suffix_count > 0, windows);
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
    assert!(parse_staged(&text, false).is_ok());
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
        installed_target(&bin.join("a")),
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

#[cfg(windows)]
fn native_tool(target: &Path) -> &'static [&'static str] {
    fs::copy(std::env::current_exe().expect("test binary"), target).expect("native tool");
    &["--list"]
}

#[cfg(unix)]
fn native_tool(target: &Path) -> &'static [&'static str] {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(target, fs::Permissions::from_mode(0o755)).expect("chmod target");
    &[]
}

#[test]
fn workspace_path_with_spaces_installs() {
    let scratch = dx_test_scratch::scratch("dx-env-test-with space-");
    let root = scratch.path().to_path_buf();
    let host = dx_path::host::host_filename_for("a", cfg!(windows));
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &[host.as_str()])]);
    let target = root.join("targets").join("a");
    let args = native_tool(&target);
    let mut opts = options(&root, staged_bin, metadata);
    opts.os = std::env::consts::OS;
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let link = root.join("ws").join(".dx").join("bin").join(&host);
    assert_eq!(
        installed_target(&link),
        fs::canonicalize(&target).expect("canonical target")
    );
    let output = std::process::Command::new(&link)
        .args(args)
        .output()
        .expect("run installed tool");
    assert!(output.status.success());
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    let _ = fs::remove_dir_all(&root);
}

fn installed_fixture(prefix: &str) -> (dx_test_scratch::TempDir, PathBuf, RefreshOptions) {
    let scratch = dx_test_scratch::scratch(prefix);
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) =
        write_staged(&root, &[("a", "//o:a", &["a"]), ("b", "//o:b", &["b"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let bin = root.join("ws").join(".dx").join("bin");
    (scratch, bin, opts)
}

#[test]
fn missing_link_is_repaired() {
    let (_scratch, bin, opts) = installed_fixture("dx-env-test-missing-");
    fs::remove_file(bin.join("b")).expect("remove link");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert!(bin.join("b").exists());
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
}

#[test]
fn dangling_link_is_repaired() {
    let (scratch, bin, opts) = installed_fixture("dx-env-test-dangling-");
    fs::remove_file(bin.join("a")).expect("remove link");
    test_symlink(&scratch.path().join("gone"), &bin.join("a")).expect("dangling link");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert!(bin.join("a").exists());
}

#[test]
fn changed_target_is_repaired() {
    let (scratch, bin, opts) = installed_fixture("dx-env-test-retarget-");
    let other = scratch.path().join("other");
    fs::write(&other, "x").expect("other target");
    fs::remove_file(bin.join("a")).expect("remove link");
    test_symlink(&other, &bin.join("a")).expect("stale link");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert_eq!(
        installed_target(&bin.join("a")),
        fs::canonicalize(scratch.path().join("targets").join("a")).expect("target")
    );
}

#[test]
fn moved_output_base_is_repaired() {
    let (scratch, bin, opts) = installed_fixture("dx-env-test-outbase-");
    let root = scratch.path();
    let moved = root.join("targets2");
    fs::create_dir_all(&moved).expect("targets2");
    fs::write(moved.join("a"), "x").expect("new a");
    fs::remove_file(opts.staged_bin.join("a")).expect("remove staged");
    test_symlink(&moved.join("a"), &opts.staged_bin.join("a")).expect("restage");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert_eq!(
        installed_target(&bin.join("a")),
        fs::canonicalize(moved.join("a")).expect("canonical")
    );
}

#[test]
fn foreign_entry_refuses_without_mutation() {
    let (_scratch, bin, opts) = installed_fixture("dx-env-test-foreign-");
    fs::write(bin.join("stray"), "mine").expect("stray");
    fs::remove_file(bin.join("b")).expect("remove link");
    let error = refresh(&opts, &probe_symlink).unwrap_err();
    assert!(matches!(
        error,
        Error::Unmanaged { detail, .. } if detail.contains("stray")
    ));
    assert_eq!(fs::read(bin.join("stray")).expect("kept"), b"mine".to_vec());
    assert!(!bin.join("b").exists());
}

#[test]
fn failed_repair_keeps_installed_tree() {
    let (scratch, bin, opts) = installed_fixture("dx-env-test-failed-repair-");
    fs::remove_file(bin.join("b")).expect("remove link");
    fs::remove_file(opts.staged_bin.join("b")).expect("break staged");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Staged { .. })
    ));
    assert!(bin.join("a").exists());
    assert!(bin.join(MARKER_FILE_NAME).exists());
    drop(scratch);
}

#[test]
fn corrupt_marker_on_installed_tree_refuses() {
    let (_scratch, bin, opts) = installed_fixture("dx-env-test-corrupt-");
    fs::write(bin.join(MARKER_FILE_NAME), b"garbage").expect("corrupt");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Unmanaged { .. })
    ));
    assert!(bin.join("a").exists());
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
fn marker_named_host_leaves_its_target_intact() {
    let scratch = dx_test_scratch::scratch("dx-env-test-marker-host-");
    let root = scratch.path().to_path_buf();
    let target = root.join("tool");
    let script = b"#!/bin/sh\necho tool\n";
    fs::write(&target, script).expect("target");
    let error = stage_tree(
        &root.join("stage"),
        &[(MARKER_FILE_NAME.to_string(), target.clone())],
        &[7u8; 32],
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains(&format!("cannot stage host name '{MARKER_FILE_NAME}'"))
    ));
    assert_eq!(fs::read(&target).expect("target survived"), script);
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
    let calls = std::cell::Cell::new(0u32);
    let counting = |dir: &Path| {
        calls.set(calls.get() + 1);
        probe_symlink(dir)
    };
    let error = refresh(&opts, &counting).unwrap_err();
    assert!(matches!(error, Error::Busy { .. }));
    assert_eq!(calls.get(), 0);
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
    let win_root = root.join("win");
    fs::create_dir_all(&win_root).expect("win root");
    let (win_bin, win_metadata, _) = write_staged(&win_root, &[("a", "//o:a", &["a.exe"])]);
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let failing = |_: &Path| -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
    };
    let mut opts = options(&root, win_bin, win_metadata);
    opts.os = "windows";
    let error = refresh(&opts, &failing).unwrap_err();
    assert!(matches!(error, Error::SymlinkUnsupported { .. }));
    assert!(matches!(
        error,
        Error::SymlinkUnsupported { detail } if detail.contains("Developer Mode")
    ));
    let mut opts = options(&root, staged_bin, metadata);
    opts.os = "other";
    let error = refresh(&opts, &failing).unwrap_err();
    assert!(matches!(
        error,
        Error::SymlinkUnsupported { detail } if detail.contains("no changes were made")
    ));
    assert!(!root.join("ws").join(".dx").join("bin").exists());
    acquire_lock(&root.join("ws").join(".dx"), Duration::from_millis(1))
        .expect("lock released after probe failure");
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn blocked_installed_tree_reports_unmanaged() {
    let scratch = dx_test_scratch::scratch("dx-env-test-blocked-bin-");
    let root = scratch.path().to_path_buf();
    let blocker = root.join("blocker");
    fs::write(&blocker, "file blocks dir").expect("blocker file");
    let bin = blocker.join(BIN_DIR_NAME);
    let error = read_current_identity(&bin).unwrap_err();
    assert!(matches!(
        error,
        Error::Unmanaged { path, detail }
            if path == bin && detail.contains("cannot inspect installed tree")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn crash_restore_failure_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-crash-fail-");
    let root = scratch.path().to_path_buf();
    let prev = root.join(PREV_DIR_NAME);
    fs::create_dir_all(&prev).expect("prev dir");
    let blocker = root.join("blocker");
    fs::write(&blocker, "file blocks dir").expect("blocker file");
    let error = recover_crashed_swap(&prev, &blocker.join(BIN_DIR_NAME)).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot restore interrupted tree")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uncreatable_staging_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stage-fail-");
    let root = scratch.path().to_path_buf();
    let target = root.join("tool");
    fs::write(&target, b"tool").expect("tool");
    let staged = vec![("a".to_string(), target)];
    let blocker = root.join("blocker");
    fs::write(&blocker, "file blocks dir").expect("blocker file");
    let error = stage_tree(&blocker.join(STAGE_DIR_NAME), &staged, &[7u8; 32]).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot create staging")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn commit_swap_failures_report_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-swap-fail-");
    let root = scratch.path().to_path_buf();
    let bin = root.join(BIN_DIR_NAME);
    fs::create_dir_all(&bin).expect("bin dir");
    let occupied = root.join(PREV_DIR_NAME);
    fs::create_dir_all(&occupied).expect("prev dir");
    fs::write(occupied.join("keep"), "keep").expect("occupied prev");
    let stage = root.join(STAGE_DIR_NAME);
    fs::create_dir_all(&stage).expect("stage dir");
    let error = commit_swap(&bin, &occupied, &stage, &|a, b| fs::rename(a, b)).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot retire current tree")
    ));

    let blocker = root.join("blocker");
    fs::write(&blocker, "file blocks dir").expect("blocker file");
    let error = commit_swap(
        &blocker.join(BIN_DIR_NAME),
        &root.join(PREV_DIR_NAME),
        &stage,
        &|a, b| fs::rename(a, b),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot publish staged tree")
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn commit_swap_publish_failure_restores_previous_tree() {
    let scratch = dx_test_scratch::scratch("dx-env-test-swap-restore-");
    let root = scratch.path().to_path_buf();
    let bin = root.join(BIN_DIR_NAME);
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("old"), "old").expect("old");
    let prev = root.join(PREV_DIR_NAME);
    let stage = root.join(STAGE_DIR_NAME);
    let error = commit_swap(&bin, &prev, &stage, &|a, b| fs::rename(a, b)).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason }
            if reason.contains("cannot publish staged tree") && !reason.contains("cannot restore")
    ));
    assert!(bin.join("old").exists());
    assert!(!prev.exists());
}

#[test]
fn commit_swap_restore_failure_reports_both() {
    let scratch = dx_test_scratch::scratch("dx-env-test-swap-both-");
    let root = scratch.path().to_path_buf();
    let bin = root.join(BIN_DIR_NAME);
    fs::create_dir_all(&bin).expect("bin dir");
    fs::write(bin.join("old"), "old").expect("old");
    let prev = root.join(PREV_DIR_NAME);
    let stage = root.join(STAGE_DIR_NAME);
    fs::create_dir_all(&stage).expect("stage dir");
    let calls = std::cell::Cell::new(0u32);
    let rename = |a: &Path, b: &Path| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            fs::rename(a, b)
        } else {
            Err(io::Error::other("locked"))
        }
    };
    let error = commit_swap(&bin, &prev, &stage, &rename).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason }
            if reason.contains("cannot publish staged tree")
                && reason.contains("cannot restore previous tree")
    ));
    assert!(!bin.exists());
    assert!(prev.join("old").exists());
    recover_crashed_swap(&prev, &bin).expect("recover");
    assert!(bin.join("old").exists());
}

#[test]
fn default_probe_accepts_writable_dir() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-ok-");
    let root = scratch.path().to_path_buf();
    probe_symlink(&root).expect("probe succeeds");
    assert!(!root.join("symlink.probe").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn default_probe_replaces_stale_probe() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-stale-");
    let root = scratch.path().to_path_buf();
    let file_dir = root.join("file");
    let link_dir = root.join("link");
    fs::create_dir_all(&file_dir).expect("file dir");
    fs::create_dir_all(&link_dir).expect("link dir");
    fs::write(file_dir.join("symlink.probe"), "stale").expect("stale file");
    test_symlink(Path::new("missing"), &link_dir.join("symlink.probe")).expect("dangling link");
    probe_symlink(&file_dir).expect("probe replaces file");
    probe_symlink(&link_dir).expect("probe replaces dangling link");
    assert!(fs::symlink_metadata(file_dir.join("symlink.probe")).is_err());
    assert!(fs::symlink_metadata(link_dir.join("symlink.probe")).is_err());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn refresh_succeeds_with_stale_probe() {
    let scratch = dx_test_scratch::scratch("dx-env-test-refresh-stale-probe-");
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = root.join("ws").join(".dx");
    fs::create_dir_all(&dx).expect("dx dir");
    fs::write(dx.join("symlink.probe"), "stale").expect("stale probe");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    assert!(fs::symlink_metadata(dx.join("symlink.probe")).is_err());
    let _ = fs::remove_dir_all(&root);
}
