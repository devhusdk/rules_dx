use super::*;
use dx_atomic_fs::commit::LockError;

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
        let script = format!("#!/bin/sh\necho {bin_name}\n");
        #[cfg(unix)]
        dx_testing::executable_script(&real, &script);
        #[cfg(not(unix))]
        fs::write(&real, script).expect("write target");
        for host in *hosts {
            dx_atomic_fs::discard_file(staged_bin.join(host));
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
    let text = format!("{{\"tools\":[{}]}}", entries.join(","));
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
fn marker_rejects_short_identity() {
    let bytes = EnvMarker {
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
    let scratch = dx_test_scratch::scratch("dx-env-test-parse-ok-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn parse_staged_rejects_syntax_and_shape() {
    assert!(matches!(
        parse_staged("{oops", false),
        Err(Error::Staged { .. })
    ));
    assert!(matches!(
        parse_staged(r"{}", false),
        Err(Error::Staged { .. })
    ));
}

#[test]
fn parse_staged_rejects_empty_set() {
    assert!(matches!(
        parse_staged(r#"{"tools":[]}"#, false),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn parse_staged_rejects_bad_records() {
    let empty_bin = r#"{"tools":[{"bin_name":"","owner":"o","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_bin, false),
        Err(Error::InvalidTool { .. })
    ));
    let empty_owner = r#"{"tools":[{"bin_name":"a","owner":"","host_names":["a"]}]}"#;
    assert!(matches!(
        parse_staged(empty_owner, false),
        Err(Error::InvalidTool { .. })
    ));
    let no_hosts = r#"{"tools":[{"bin_name":"a","owner":"o","host_names":[]}]}"#;
    assert!(matches!(
        parse_staged(no_hosts, false),
        Err(Error::InvalidTool { .. })
    ));
}

#[test]
fn host_name_table() {
    for valid in ["a", "doctor-2", "x.y", "my_tool"] {
        let text = serde_json::json!({
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
            "tools": [{"bin_name": invalid, "owner": "o", "host_names": [invalid]}],
        })
        .to_string();
        assert!(
            matches!(parse_staged(&text, false), Err(Error::InvalidTool { .. })),
            "invalid host name accepted: {invalid:?}"
        );
    }
}

fn tools_json(tools: &serde_json::Value) -> String {
    serde_json::json!({"tools": tools}).to_string()
}

#[test]
fn windows_host_names_carry_the_exe_suffix() {
    let text = tools_json(&serde_json::json!([
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
    let bare = tools_json(&serde_json::json!([
        {"bin_name": "dx", "owner": "o", "host_names": ["dx"]},
    ]));
    assert!(parse_staged(&bare, true).is_err());
    for wrong in ["dx.bat", "other.exe", "dx.exe.exe", "DX.EXE"] {
        let text = tools_json(&serde_json::json!([
            {"bin_name": "dx", "owner": "o", "host_names": [wrong]},
        ]));
        assert!(parse_staged(&text, true).is_err(), "accepted {wrong}");
    }
    let missing_alias = tools_json(&serde_json::json!([
        {"bin_name": "dx", "owner": "o", "aliases": ["d"], "host_names": ["dx.exe"]},
    ]));
    assert!(parse_staged(&missing_alias, true).is_err());
}

#[test]
fn logical_names_with_suffixes_are_rejected_on_windows() {
    let text = tools_json(&serde_json::json!([
        {"bin_name": "dx.exe", "owner": "o", "host_names": ["dx.exe.exe"]},
    ]));
    assert!(parse_staged(&text, true).is_err());
}

#[test]
fn colliding_host_names_across_tools_are_rejected() {
    let text = tools_json(&serde_json::json!([
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
        assert_ne!(plans, [] as [ToolPlan; 0]);
        let suffix_count = plans
            .iter()
            .flat_map(|plan| &plan.host_names)
            .filter(|name| {
                std::path::Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            })
            .count();
        assert_eq!(suffix_count > 0, windows);
    }
}

#[test]
fn error_display_names_every_variant() {
    let dir = PathBuf::from("/ws/.dx/local/bin");
    let errors = [
        Error::WorkspaceRoot {
            path: PathBuf::from("/ws"),
        },
        Error::Staged {
            reason: "r".to_string(),
        },
        Error::InvalidTool {
            reason: "r".to_string(),
        },
        Error::Unmanaged {
            path: dir.clone(),
            detail: "d".to_string(),
        },
        Error::MarkerInvalid {
            reason: "r".to_string(),
        },
        Error::Busy {
            path: dir.clone(),
            holder: None,
        },
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
        assert_ne!(format!("{error}"), "");
    }
    assert!(matches!(
        &errors[3],
        Error::Unmanaged { detail, .. } if detail == "d"
    ));
    assert!(matches!(&errors[5], Error::Busy { path, .. } if path == &dir));
}

#[test]
fn lock_errors_convert_to_env_errors() {
    let dir = PathBuf::from("/ws/.dx/local");
    let busy = LockError::Busy {
        path: dir.join(dx_atomic_fs::commit::COMMIT_LOCK_FILE_NAME),
        holder: None,
    };
    let failed = LockError::LockFailed {
        path: dir.join(dx_atomic_fs::commit::COMMIT_LOCK_FILE_NAME),
        reason: "r".to_string(),
    };
    for error in [&busy, &failed] {
        let text = error.to_string();
        assert!(text.contains(".commit.lock"), "{text}");
    }
    assert!(matches!(
        Error::from(busy),
        Error::Busy { path, .. } if path == Path::new("/ws/.dx/local/.commit.lock")
    ));
    assert!(matches!(
        Error::from(failed),
        Error::LockFailed { path, reason }
            if path == Path::new("/ws/.dx/local/.commit.lock") && reason == "r"
    ));
}

#[test]
fn workspace_missing() {
    let scratch = dx_test_scratch::scratch("dx-env-test-ws-missing-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let mut opts = options(&root, staged_bin, metadata);
    opts.workspace_root = root.join("no-such-dir");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::WorkspaceRoot { .. })
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn staged_inputs_fail_before_workspace_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-staged-fail-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn staged_rejects_non_link_and_dangling() {
    let scratch = dx_test_scratch::scratch("dx-env-test-staged-links-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn fresh_install_noop_and_replacement() {
    let scratch = dx_test_scratch::scratch("dx-env-test-lifecycle-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("alpha", "//o:alpha", &["alpha", "a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let bin = dx_path::state::local_dir(&root.join("ws")).join("bin");
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
    assert!(
        !dx_path::state::local_dir(&root.join("ws"))
            .join("bin.prev")
            .exists()
    );
    dx_atomic_fs::discard_dir(&root);
}

#[cfg(windows)]
fn native_tool(target: &Path) -> &'static [&'static str] {
    fs::copy(std::env::current_exe().expect("test binary"), target).expect("native tool");
    &["--list"]
}

#[cfg(unix)]
fn native_tool(_target: &Path) -> &'static [&'static str] {
    &[]
}

#[test]
fn workspace_path_with_spaces_installs() {
    let scratch = dx_test_scratch::scratch("dx-env-test-with space-").unwrap();
    let root = scratch.path().to_path_buf();
    let host = dx_path::host::host_filename_for("a", cfg!(windows));
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &[host.as_str()])]);
    let target = root.join("targets").join("a");
    let args = native_tool(&target);
    let mut opts = options(&root, staged_bin, metadata);
    opts.os = std::env::consts::OS;
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let link = dx_path::state::local_dir(&root.join("ws"))
        .join("bin")
        .join(&host);
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
    dx_atomic_fs::discard_dir(&root);
}

fn installed_fixture(prefix: &str) -> (dx_test_scratch::TempDir, PathBuf, RefreshOptions) {
    let scratch = dx_test_scratch::scratch(prefix).unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) =
        write_staged(&root, &[("a", "//o:a", &["a"]), ("b", "//o:b", &["b"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let bin = dx_path::state::local_dir(&root.join("ws")).join("bin");
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
    let scratch = dx_test_scratch::scratch("dx-env-test-noop-clean-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let stage_dir = dx_path::state::local_dir(&root.join("ws")).join("bin.next");
    fs::create_dir_all(&stage_dir).expect("leftover staging");
    fs::write(stage_dir.join("junk"), "junk").expect("junk");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    assert!(!stage_dir.exists());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn unmanaged_states_refuse_without_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-unmanaged-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = dx_path::state::local_dir(&root.join("ws"));
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
    dx_atomic_fs::discard_dir(&root);
}

#[cfg(unix)]
#[test]
fn stale_prev_undeletable_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-prev-file-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Ok(RefreshOutcome::InstalledFresh)
    ));
    let dx = dx_path::state::local_dir(&root.join("ws"));
    // A file where the tree belongs cannot be cleared as a tree, even by root.
    fs::write(dx.join(PREV_DIR_NAME), b"").expect("stale prev");
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale")
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[cfg(unix)]
#[test]
fn stale_stage_undeletable_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-stage-file-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = dx_path::state::local_dir(&root.join("ws"));
    fs::create_dir_all(&dx).expect("dx dir");
    // A file where the tree belongs cannot be cleared as a tree, even by root.
    fs::write(dx.join(STAGE_DIR_NAME), b"").expect("stale stage");
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale staging")
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[cfg(windows)]
fn hold_open(path: &Path) -> fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    fs::write(path, b"held").expect("held file");
    fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
        .expect("hold the file open")
}

#[cfg(windows)]
#[test]
fn stale_prev_undeletable_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-prev-held-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Ok(RefreshOutcome::InstalledFresh)
    ));
    let dx = dx_path::state::local_dir(&root.join("ws"));
    let prev = dx.join(PREV_DIR_NAME);
    fs::create_dir_all(&prev).expect("stale prev");
    let held = hold_open(&prev.join("held.txt"));
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale")
    ));
    drop(held);
    let _ = fs::remove_dir_all(&root);
}

#[cfg(windows)]
#[test]
fn stale_stage_undeletable_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-stage-held-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = dx_path::state::local_dir(&root.join("ws"));
    let stage = dx.join(STAGE_DIR_NAME);
    fs::create_dir_all(&stage).expect("stale stage");
    let held = hold_open(&stage.join("held.txt"));
    let error = refresh(&opts, &|_: &Path| Ok(())).unwrap_err();
    assert!(matches!(
        error,
        Error::Install { reason } if reason.contains("cannot clear stale staging")
    ));
    drop(held);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn duplicate_host_name_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-dup-host-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn marker_named_host_leaves_its_target_intact() {
    let scratch = dx_test_scratch::scratch("dx-env-test-marker-host-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn crash_between_renames_restores_then_replaces() {
    let scratch = dx_test_scratch::scratch("dx-env-test-crash-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("old", "//o:old", &["old"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let dx = dx_path::state::local_dir(&root.join("ws"));
    fs::rename(dx.join("bin"), dx.join("bin.prev")).expect("simulate crash");
    let (staged_bin, metadata, _) = write_staged(&root, &[("new", "//o:new", &["new"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledReplacement);
    assert!(dx.join("bin").join("new").exists());
    assert!(!dx.join("bin").join("old").exists());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn stale_prev_is_cleared() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stale-prev-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    let prev = dx_path::state::local_dir(&root.join("ws")).join("bin.prev");
    fs::create_dir_all(&prev).expect("stale prev");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::AlreadyCurrent);
    assert!(!prev.exists());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn busy_lock_fails_after_deadline() {
    let scratch = dx_test_scratch::scratch("dx-env-test-busy-").unwrap();
    let root = scratch.path().to_path_buf();
    let dx = dx_path::state::local_dir(&root.join("ws"));
    fs::create_dir_all(&dx).expect("dx dir");
    let _held = dx_atomic_fs::commit::acquire_commit_lock(
        &dx_atomic_fs::commit::ManagedRoot::new(dx.clone()),
        Duration::from_secs(10),
    )
    .expect("setup lock");
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let mut opts = options(&root, staged_bin, metadata);
    opts.lock_timeout = Duration::from_millis(1);
    let error = refresh(&opts, &probe_symlink).unwrap_err();
    assert!(matches!(error, Error::Busy { .. }));
    let pid = format!("pid {}", std::process::id());
    let text = error.to_string();
    assert!(text.contains(&pid), "{text}");
    assert!(
        text.ends_with("wait for it to finish, then retry"),
        "{text}"
    );
    assert!(
        !dx_path::state::local_dir(&root.join("ws"))
            .join("symlink.probe")
            .exists()
    );
    drop(_held);
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn lock_open_failure_aborts() {
    let scratch = dx_test_scratch::scratch("dx-env-test-lock-open-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = dx_path::state::local_dir(&root.join("ws"));
    fs::create_dir_all(&dx).expect("dx dir");
    fs::create_dir_all(dx.join(dx_atomic_fs::commit::COMMIT_LOCK_FILE_NAME))
        .expect("lock is a directory");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::LockFailed { .. })
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn dx_creation_failure_aborts() {
    let scratch = dx_test_scratch::scratch("dx-env-test-dx-create-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    fs::create_dir_all(root.join("ws")).expect("ws dir");
    fs::write(root.join("ws").join(".dx"), "file blocks dir").expect("file dx");
    assert!(matches!(
        refresh(&opts, &probe_symlink),
        Err(Error::Install { .. })
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn probe_failure_reports_before_mutation() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-fail-").unwrap();
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
        &error,
        Error::SymlinkUnsupported { detail }
            if detail.contains("SeCreateSymbolicLinkPrivilege") && !detail.contains("SeBackupPrivilege")
    ));
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
    assert!(
        !dx_path::state::local_dir(&root.join("ws"))
            .join("bin")
            .exists()
    );
    dx_atomic_fs::commit::acquire_commit_lock(
        &dx_atomic_fs::commit::ManagedRoot::new(dx_path::state::local_dir(&root.join("ws"))),
        Duration::from_millis(1),
    )
    .expect("lock released after probe failure");
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn unreadable_installed_tree_reports_unmanaged() {
    let scratch = dx_test_scratch::scratch("dx-env-test-blocked-bin-").unwrap();
    let root = scratch.path().to_path_buf();
    let bin = root.join("bad\0name");
    let error = read_current_identity(&bin).unwrap_err();
    assert!(matches!(
        error,
        Error::Unmanaged { path, detail }
            if path == bin && detail.contains("cannot inspect installed tree")
    ));
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn uncreatable_staging_reports_install() {
    let scratch = dx_test_scratch::scratch("dx-env-test-stage-fail-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn default_probe_accepts_writable_dir() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-ok-").unwrap();
    let root = scratch.path().to_path_buf();
    probe_symlink(&root).expect("probe succeeds");
    assert!(!root.join("symlink.probe").exists());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn default_probe_replaces_stale_probe() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-stale-").unwrap();
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
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn refresh_succeeds_with_stale_probe() {
    let scratch = dx_test_scratch::scratch("dx-env-test-refresh-stale-probe-").unwrap();
    let root = scratch.path().to_path_buf();
    let (staged_bin, metadata, _) = write_staged(&root, &[("a", "//o:a", &["a"])]);
    let opts = options(&root, staged_bin, metadata);
    let dx = dx_path::state::local_dir(&root.join("ws"));
    fs::create_dir_all(&dx).expect("dx dir");
    fs::write(dx.join("symlink.probe"), "stale").expect("stale probe");
    assert_eq!(refresh_ok(&opts), RefreshOutcome::InstalledFresh);
    assert!(fs::symlink_metadata(dx.join("symlink.probe")).is_err());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn empty_logical_names_are_rejected() {
    let text = tools_json(&serde_json::json!([
        {"bin_name": "dx", "owner": "o", "aliases": [""], "host_names": ["dx", ""]},
    ]));
    let error = parse_staged(&text, false).unwrap_err();
    assert!(error.to_string().contains("non-empty"), "{error}");
}

#[test]
fn probe_fails_when_the_stale_probe_cannot_be_removed() {
    let scratch = dx_test_scratch::scratch("dx-env-test-probe-stuck-").unwrap();
    let root = scratch.path().to_path_buf();
    let stuck = root.join("symlink.probe");
    fs::create_dir_all(stuck.join("inner")).expect("non-empty probe dir");
    assert!(probe_symlink(&root).is_err());
    dx_atomic_fs::discard_dir(&root);
}

#[test]
fn verify_installed_reports_an_unlistable_tree() {
    let scratch = dx_test_scratch::scratch("dx-env-test-verify-missing-").unwrap();
    let root = scratch.path().to_path_buf();
    let error = verify_installed(&root.join("missing"), &[]).unwrap_err();
    assert!(matches!(error, Error::Unmanaged { .. }));
    assert!(
        error.to_string().contains("cannot list installed tree"),
        "{error}"
    );
    dx_atomic_fs::discard_dir(&root);
}
