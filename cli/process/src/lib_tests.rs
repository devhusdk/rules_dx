use super::*;

use std::collections::{HashMap, HashSet};

struct FakeFs {
    files: HashSet<PathBuf>,
    broken: HashMap<PathBuf, String>,
}

impl FakeFs {
    fn with_files(paths: &[&str]) -> FakeFs {
        FakeFs {
            files: paths.iter().map(PathBuf::from).collect(),
            broken: HashMap::new(),
        }
    }
}

impl Fs for FakeFs {
    fn is_file(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    fn broken_marker_hint(&self, dir: &Path) -> Option<String> {
        self.broken.get(dir).cloned()
    }
}

#[test]
fn discovers_module_at_start() {
    let fs = FakeFs::with_files(&["/repo/MODULE.bazel"]);
    let found = discover(Path::new("/repo/sub/dir"), None, &fs).expect("discover");
    assert_eq!(found, PathBuf::from("/repo"));
}

#[test]
fn discovers_module_at_start_itself() {
    let fs = FakeFs::with_files(&["/repo/MODULE.bazel"]);
    let found = discover(Path::new("/repo"), None, &fs).expect("discover");
    assert_eq!(found, PathBuf::from("/repo"));
}

#[test]
fn override_with_module_wins() {
    let fs = FakeFs::with_files(&["/other/MODULE.bazel"]);
    let found = discover(Path::new("/repo/sub"), Some(Path::new("/other")), &fs).expect("override");
    assert_eq!(found, PathBuf::from("/other"));
}

#[test]
fn override_without_marker_fails() {
    let fs = FakeFs::with_files(&[]);
    let err = discover(Path::new("/repo"), Some(Path::new("/other")), &fs).expect_err("must fail");
    assert_eq!(
        err,
        DiscoverError::InvalidOverride {
            path: PathBuf::from("/other"),
        }
    );
    assert!(err.to_string().contains("--workspace"));
    assert!(err.to_string().contains("/other"));
}

#[test]
fn override_with_legacy_reports_migration() {
    let fs = FakeFs::with_files(&["/other/WORKSPACE"]);
    let err = discover(Path::new("/repo"), Some(Path::new("/other")), &fs).expect_err("must fail");
    assert_eq!(
        err,
        DiscoverError::UnsupportedLegacy {
            dir: PathBuf::from("/other"),
        }
    );
    assert!(err.to_string().contains("Bzlmod"));
}

#[test]
fn legacy_workspace_bazel_reports_migration() {
    let fs = FakeFs::with_files(&["/repo/WORKSPACE.bazel"]);
    let err = discover(Path::new("/repo/sub"), None, &fs).expect_err("must fail");
    assert!(matches!(err, DiscoverError::UnsupportedLegacy { .. }));
    assert!(err.to_string().contains("WORKSPACE"));
}

#[test]
fn missing_module_lists_searched_and_suggests_override() {
    let fs = FakeFs::with_files(&[]);
    let err = discover(Path::new("/repo/sub"), None, &fs).expect_err("must fail");
    assert!(
        matches!(&err, DiscoverError::NotFound { searched } if searched.contains(&PathBuf::from("/repo/sub")))
    );
    assert!(
        matches!(&err, DiscoverError::NotFound { searched } if searched.contains(&PathBuf::from("/repo")))
    );
    assert!(err.to_string().contains("--workspace"));
}

#[test]
fn nearest_legacy_wins_over_distant_module() {
    let fs = FakeFs::with_files(&["/repo/MODULE.bazel", "/repo/sub/WORKSPACE"]);
    let found = discover(Path::new("/repo/sub"), None, &fs).expect("module wins");
    assert_eq!(found, PathBuf::from("/repo"));
}

#[test]
fn real_fs_roundtrip_discovers_workspace() {
    let scratch = dx_test_scratch::scratch("dx-discover-");
    let root = scratch.path().to_path_buf();
    let nested = root.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("dirs");
    std::fs::write(root.join("MODULE.bazel"), "module(name = \"t\")\n").expect("marker");
    let found = discover_real(&nested, None).expect("real discover");
    let root = root.canonicalize().expect("canonical root");
    assert_eq!(found, root);
    scratch.close().expect("cleanup");
}

#[test]
fn real_fs_missing_module_suggests_override() {
    let scratch = dx_test_scratch::scratch("dx-missing-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir_all(&root).expect("dirs");
    let err = discover_real(&root, None).expect_err("must fail");
    assert!(err.to_string().contains("--workspace"));
    scratch.close().expect("cleanup");
}

#[test]
fn launcher_has_no_fallback() {
    assert_eq!(launcher_argv0(), "bazel");
    assert_eq!(WORKFLOW_STARTUP_OPTS, &["--nohome_rc", "--nosystem_rc"]);
}

#[test]
fn scope_rendering_stays_compact() {
    assert_eq!(describe_scope(&Scope::Repository), "//...");
    assert_eq!(
        describe_scope(&Scope::Pattern("//src/auth/...".to_owned())),
        "//src/auth/..."
    );
    assert_eq!(
        describe_scope(&Scope::Labels(vec![
            "//a:a".to_owned(),
            "//b/...".to_owned()
        ])),
        "//a:a //b/..."
    );
    assert_eq!(
        describe_scope(&Scope::ResolvedOwners(vec!["//a:a".to_owned()])),
        "//a:a"
    );
    assert_eq!(describe_scope(&Scope::Count(1)), "1 target");
    assert_eq!(describe_scope(&Scope::Count(3)), "3 targets");
    assert_eq!(describe_scope(&Scope::Count(0)), "0 targets");
}

#[test]
fn summaries_never_render_argv() {
    let summary = operation_summary("lint", "analysis", &Scope::Count(3));
    assert_eq!(summary, "Running lint analysis for 3 targets");
    let summary = operation_summary("lint", "analysis", &Scope::Repository);
    assert_eq!(summary, "Running lint analysis for //...");
    let summary = operation_summary(
        "lint",
        "analysis",
        &Scope::Pattern("//src/auth/...".to_owned()),
    );
    assert_eq!(summary, "Running lint analysis for //src/auth/...");
}

#[test]
fn protected_check_accepts_repeated_required_value() {
    let protected = vec![ProtectedFlag {
        name: "keep_going".to_owned(),
        required: Some("--keep_going".to_owned()),
        allowed: Vec::new(),
    }];
    let kept = check_protected(&["--keep_going".to_owned()], &protected).expect("repeat");
    assert_eq!(kept, vec!["--keep_going".to_owned()]);
}

#[test]
fn protected_check_rejects_conflict_without_echoing_values() {
    let protected = vec![ProtectedFlag {
        name: "keep_going".to_owned(),
        required: Some("--keep_going".to_owned()),
        allowed: Vec::new(),
    }];
    let err =
        check_protected(&["--keep_going=false".to_owned()], &protected).expect_err("conflict");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "keep_going".to_owned(),
        }
    );
    assert!(!err.to_string().contains("false"));
    assert!(err.to_string().contains("--keep_going"));
}

#[test]
fn protected_check_rejects_bare_name_against_valued_requirement() {
    let protected = vec![ProtectedFlag {
        name: "config".to_owned(),
        required: Some("--config=dx".to_owned()),
        allowed: Vec::new(),
    }];
    let err = check_protected(&["--config=other".to_owned()], &protected).expect_err("conflict");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "config".to_owned(),
        }
    );
    assert!(!err.to_string().contains("other"));
    assert!(!err.to_string().contains("dx"));
    assert!(err.to_string().contains("--config"));
}

#[test]
fn protected_check_accepts_allowed_extras_beside_required() {
    let protected = vec![ProtectedFlag {
        name: "config".to_owned(),
        required: Some("--config=dx".to_owned()),
        allowed: vec!["--config=ci".to_owned()],
    }];
    let kept = check_protected(
        &["--config=ci".to_owned(), "--config=dx".to_owned()],
        &protected,
    )
    .expect("blessed extra passes");
    assert_eq!(
        kept,
        vec!["--config=ci".to_owned(), "--config=dx".to_owned()]
    );
    let err = check_protected(&["--config=other".to_owned()], &protected).expect_err("conflict");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "config".to_owned(),
        }
    );
}

#[test]
fn protected_check_rejects_unconditional_flag() {
    let protected = vec![ProtectedFlag {
        name: "build_event_json_file".to_owned(),
        required: None,
        allowed: Vec::new(),
    }];
    let err = check_protected(
        &["--build_event_json_file=/tmp/bep.json".to_owned()],
        &protected,
    )
    .expect_err("conflict");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "build_event_json_file".to_owned(),
        }
    );
    assert!(!err.to_string().contains("/tmp/bep.json"));
}

#[test]
fn protected_check_preserves_unrelated_order() {
    let protected = vec![ProtectedFlag {
        name: "keep_going".to_owned(),
        required: Some("--keep_going".to_owned()),
        allowed: Vec::new(),
    }];
    let kept = check_protected(
        &[
            "--jobs=4".to_owned(),
            "plain".to_owned(),
            "--keep_going".to_owned(),
        ],
        &protected,
    )
    .expect("kept");
    assert_eq!(
        kept,
        vec![
            "--jobs=4".to_owned(),
            "plain".to_owned(),
            "--keep_going".to_owned()
        ]
    );
}

#[test]
fn startup_and_binary_args_are_detected() {
    assert!(is_startup_option("--output_base=/tmp/x"));
    assert!(is_startup_option("--bazelrc=/tmp/rc"));
    assert!(is_startup_option("--home_rc"));
    assert!(!is_startup_option("--jobs=4"));
    assert!(!is_startup_option("plain"));
    assert!(!is_startup_option("--"));
    assert!(is_test_binary_arg("--test_arg=fast"));
    assert!(!is_test_binary_arg("--jobs=4"));
    assert!(!is_test_binary_arg("plain"));
    assert!(flag_name("--").is_none());
    assert!(flag_name("plain").is_none());
}

#[test]
fn internal_errors_render_without_echoing_values() {
    let err = ForwardError::InvalidRequiredOption {
        flag: "keep_going".to_owned(),
    };
    assert!(err.to_string().contains("--keep_going"));
    let err = ForwardError::InvalidSetting {
        option: "clippy_output_diagnostics=true".to_owned(),
    };
    assert!(err.to_string().contains("malformed required setting"));
    let err = ForwardError::UnsupportedCommand {
        command: "lint".to_owned(),
    };
    assert!(err.to_string().contains("lint"));
}

#[test]
fn workflow_argv_orders_startup_command_required_user_labels() {
    let protected = vec![ProtectedFlag {
        name: "keep_going".to_owned(),
        required: Some("--keep_going".to_owned()),
        allowed: Vec::new(),
    }];
    let argv = build_workflow_argv(
        "build",
        &["--jobs=4".to_owned(), "--keep_going".to_owned()],
        &["--keep_going".to_owned(), "--config=dx".to_owned()],
        &protected,
        &["//...".to_owned()],
    )
    .expect("argv");
    assert_eq!(
        argv,
        vec![
            "bazel",
            "--nohome_rc",
            "--nosystem_rc",
            "build",
            "--keep_going",
            "--config=dx",
            "--jobs=4",
            "--keep_going",
            "//...",
        ]
    );
}

#[test]
fn workflow_argv_rejects_startup_options() {
    let err = build_workflow_argv(
        "build",
        &["--output_base=/tmp/x".to_owned()],
        &[],
        &[],
        &["//...".to_owned()],
    )
    .expect_err("startup");
    assert_eq!(
        err,
        ForwardError::StartupOption {
            flag: "output_base".to_owned(),
        }
    );
    assert!(err.to_string().contains("dx bazel"));
    assert!(!err.to_string().contains("/tmp/x"));
}

#[test]
fn workflow_argv_rejects_test_binary_args_outside_test_commands() {
    for command in ["build", "run", "lint"] {
        let err = build_workflow_argv(
            command,
            &["--test_arg=fast".to_owned()],
            &[],
            &[],
            &["//...".to_owned()],
        )
        .expect_err("binary args");
        assert_eq!(
            err,
            ForwardError::TestBinaryArgs {
                flag: "test_arg".to_owned(),
            },
            "{command}"
        );
        assert!(err.to_string().contains("dx bazel"), "{command}: {err}");
    }
}

#[test]
fn workflow_argv_forwards_test_binary_args_on_test_commands() {
    for command in ["test", "coverage"] {
        let argv = build_workflow_argv(
            command,
            &[
                "--test_arg=--exact".to_owned(),
                "--test_arg".to_owned(),
                "case with spaces héllo".to_owned(),
                "--jobs=4".to_owned(),
            ],
            &["--keep_going".to_owned()],
            &[],
            &["//...".to_owned()],
        )
        .expect("argv");
        assert_eq!(
            argv,
            vec![
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                command,
                "--keep_going",
                "--test_arg=--exact",
                "--test_arg",
                "case with spaces héllo",
                "--jobs=4",
                "//...",
            ],
            "{command}"
        );
    }
}

#[test]
fn workflow_argv_keeps_test_filter_unchanged_on_test_commands() {
    let argv = build_workflow_argv(
        "test",
        &["--test_filter=unit".to_owned()],
        &[],
        &[],
        &["//...".to_owned()],
    )
    .expect("argv");
    assert_eq!(argv.last().map(String::as_str), Some("//..."));
    assert_eq!(
        argv.iter()
            .filter(|arg| *arg == "--test_filter=unit")
            .count(),
        1
    );
}

#[test]
fn workflow_argv_still_protects_capture_options_on_test_commands() {
    let protected = vec![ProtectedFlag {
        name: "build_event_json_file".to_owned(),
        required: None,
        allowed: Vec::new(),
    }];
    let err = build_workflow_argv(
        "test",
        &[
            "--test_arg=fast".to_owned(),
            "--build_event_json_file=x".to_owned(),
        ],
        &[],
        &protected,
        &["//...".to_owned()],
    )
    .expect_err("protected capture option");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "build_event_json_file".to_owned(),
        }
    );
    let err = build_workflow_argv(
        "test",
        &["--output_base=/tmp/x".to_owned()],
        &[],
        &[],
        &["//...".to_owned()],
    )
    .expect_err("startup option");
    assert!(matches!(err, ForwardError::StartupOption { .. }));
}

#[test]
fn execution_gaps_forwarding_matrix_is_wont_fix() {
    for startup in [
        "--bazelrc=/tmp/rc",
        "--home_rc",
        "--nohome_rc",
        "--system_rc",
        "--nosystem_rc",
        "--output_base=/tmp/x",
        "--output_user_root=/tmp/y",
        "--host_jvm_args=-Xmx1g",
        "--server_jvm_out=/tmp/jvm.out",
    ] {
        assert!(
            is_startup_option(startup),
            "{startup} must count as startup"
        );
        let err = build_workflow_argv(
            "build",
            &[startup.to_owned()],
            &[],
            &[],
            &["//...".to_owned()],
        )
        .expect_err("startup must fail");
        assert!(
            matches!(err, ForwardError::StartupOption { .. }),
            "{startup} produced {err:?}"
        );
        assert!(err.to_string().contains("dx bazel"), "{startup}: {err}");
    }
    for binary in ["--test_arg=fast", "--test_arg"] {
        assert!(
            is_test_binary_arg(binary),
            "{binary} must count as test-binary"
        );
        let err = build_workflow_argv(
            "build",
            &[binary.to_owned()],
            &[],
            &[],
            &["//...".to_owned()],
        )
        .expect_err("test-binary must fail outside test commands");
        assert!(
            matches!(err, ForwardError::TestBinaryArgs { .. }),
            "{binary} produced {err:?}"
        );
    }
}

#[test]
fn quality_workflows_reject_nokeep_going() {
    let protected = vec![
        ProtectedFlag {
            name: "keep_going".to_owned(),
            required: Some("--keep_going".to_owned()),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "nokeep_going".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ];
    let err = build_workflow_argv(
        "build",
        &["--nokeep_going".to_owned()],
        &["--keep_going".to_owned()],
        &protected,
        &["//...".to_owned()],
    )
    .expect_err("nokeep_going");
    assert_eq!(
        err,
        ForwardError::ConflictingOption {
            flag: "nokeep_going".to_owned(),
        }
    );
}

#[test]
fn workflow_argv_rejects_protected_conflicts() {
    let protected = vec![ProtectedFlag {
        name: "build_event_json_file".to_owned(),
        required: None,
        allowed: Vec::new(),
    }];
    let err = build_workflow_argv(
        "build",
        &["--build_event_json_file=/tmp/bep.json".to_owned()],
        &["--build_event_json_file=/tmp/required.json".to_owned()],
        &protected,
        &["//...".to_owned()],
    )
    .expect_err("conflict");
    assert!(matches!(err, ForwardError::ConflictingOption { .. }));
    assert!(!err.to_string().contains("/tmp/bep.json"));
    assert!(!err.to_string().contains("/tmp/required.json"));
}

#[test]
fn dx_exit_codes_are_frozen() {
    assert_eq!(EXIT_SUCCESS, 0);
    assert_eq!(operational_code(), 1);
    assert_eq!(pre_exec_code(), 2);
}

#[test]
fn ci_gate_matrix_is_single_sourced() {
    assert!(is_ci_value(Some("true")));
    for allowed in [
        None,
        Some(""),
        Some("1"),
        Some("yes"),
        Some("false"),
        Some("0"),
        Some("True"),
        Some("TRUE"),
    ] {
        assert!(!is_ci_value(allowed), "{allowed:?} must not count as CI");
    }
    assert_eq!(
        is_ci(),
        is_ci_value(std::env::var("CI").ok().as_deref()),
        "is_ci must delegate to is_ci_value"
    );
}

struct FakeRunner {
    status: ChildStatus,
}

impl Runner for FakeRunner {
    fn run(&self, argv: &[String], cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        assert!(!argv.is_empty());
        assert!(cwd.is_absolute() || cwd.as_os_str() == ".");
        Ok(self.status.clone())
    }
}

#[test]
fn fake_runner_substitutes_the_boundary() {
    let runner = FakeRunner {
        status: ChildStatus { code: Some(3) },
    };
    let status = runner
        .run(
            &["bazel".to_owned(), "build".to_owned()],
            Path::new("."),
            &[],
        )
        .expect("fake");
    assert_eq!(status.code, Some(3));
}

#[test]
fn system_runner_preserves_exit_codes() {
    let runner = SystemRunner;
    let cwd = std::env::temp_dir();
    let ok = runner
        .run(&probe_argv(&["--exit-code=0"]), &cwd, &[])
        .expect("probe");
    assert_eq!(ok.code, Some(0));
    let fail = runner
        .run(&probe_argv(&["--exit-code=1"]), &cwd, &[])
        .expect("probe");
    assert_eq!(fail.code, Some(1));
}

/// A probe argv that runs the Bazel-built probe with the given flags.
fn probe_argv(flags: &[&str]) -> Vec<String> {
    let mut argv = vec![dx_testing::process_probe().to_string_lossy().into_owned()];
    argv.extend(flags.iter().map(|flag| (*flag).to_owned()));
    argv
}

#[test]
fn system_runner_forwards_extra_environment() {
    let runner = SystemRunner;
    let cwd = std::env::temp_dir();
    let probed = runner
        .run(
            &probe_argv(&["--require-env=DX_RUNNER_PROBE=forwarded"]),
            &cwd,
            &[("DX_RUNNER_PROBE", "forwarded")],
        )
        .expect("probe");
    assert_eq!(probed.code, Some(0));
    let rejected = runner
        .run(
            &probe_argv(&["--require-env=DX_RUNNER_PROBE=forwarded"]),
            &cwd,
            &[("DX_RUNNER_PROBE", "other")],
        )
        .expect("probe");
    assert_eq!(rejected.code, Some(70), "a wrong value must not pass");
}

#[test]
fn system_runner_rejects_bad_invocations() {
    let runner = SystemRunner;
    assert!(runner.run(&[], Path::new("."), &[]).is_err());
    assert!(runner
        .run(&["/nonexistent-dx-tool".to_owned()], Path::new("."), &[])
        .is_err());
}

#[test]
fn hermetic_runner_clears_the_parent_environment() {
    let cwd = std::env::temp_dir();
    let inherited = std::env::var("TEST_SRCDIR").expect("Bazel sets TEST_SRCDIR for every test");
    let argv = probe_argv(&[
        "--forbid-env=TEST_SRCDIR",
        "--print-env=TEST_SRCDIR",
        "--print-env=DX_HERMETIC_PROBE",
    ]);
    let kept = spawn_output(&argv, &cwd, &[("DX_HERMETIC_PROBE", "kept")], false)
        .expect("probe inherits the parent environment");
    assert_eq!(
        kept.status.code(),
        Some(71),
        "an uncleared child still sees TEST_SRCDIR"
    );
    let cleared = spawn_output(&argv, &cwd, &[("DX_HERMETIC_PROBE", "kept")], true)
        .expect("probe runs with a cleared environment");
    assert_eq!(cleared.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&cleared.stdout),
        format!("<unset>\nkept\n"),
        "inherited={inherited}"
    );
}

#[test]
fn hermetic_runner_forwards_extra_environment() {
    let runner = SystemRunner;
    let cwd = std::env::temp_dir();
    let kept = runner
        .run_hermetic(
            &probe_argv(&["--require-env=DX_HERMETIC_PROBE=kept"]),
            &cwd,
            &[("DX_HERMETIC_PROBE", "kept")],
        )
        .expect("probe");
    assert_eq!(kept.code, Some(0));
    let missing = runner
        .run_hermetic(
            &probe_argv(&["--require-env=DX_HERMETIC_PROBE=kept"]),
            &cwd,
            &[],
        )
        .expect("probe");
    assert_eq!(missing.code, Some(70), "an unset variable must not pass");
}

#[test]
fn hermetic_runner_sets_no_path() {
    let runner = SystemRunner;
    let cwd = std::env::temp_dir();
    let no_path = runner
        .run_hermetic(&probe_argv(&["--forbid-env=PATH"]), &cwd, &[])
        .expect("probe");
    assert_eq!(no_path.code, Some(0));
    let inherited = runner
        .run(&probe_argv(&["--forbid-env=PATH"]), &cwd, &[])
        .expect("probe");
    assert_eq!(
        inherited.code,
        Some(71),
        "only the hermetic runner clears PATH"
    );
}

#[test]
fn gitleaks_tool_defaults_to_absent() {
    let runner = FakeRunner {
        status: ChildStatus { code: Some(0) },
    };
    assert!(runner.gitleaks_tool().is_none());
}

#[test]
fn spawn_success_reports_status_without_triplication() {
    assert!(spawn_success(&probe_argv(&["--exit-code=0"])));
    assert!(!spawn_success(&probe_argv(&["--exit-code=1"])));
    assert!(!spawn_success(&[]));
    assert!(!spawn_success(&["/nonexistent-dx-tool".to_owned()]));
}

#[test]
fn exe_available_covers_help_file_and_path() {
    assert!(exe_available(
        &dx_testing::process_probe().to_string_lossy().into_owned()
    ));
    assert!(!exe_available("/nonexistent-dx-tool-xyz"));
    assert!(!exe_available(""));
}

#[test]
fn stdout_broken_pipe_maps_to_141() {
    assert_eq!(broken_pipe_code(), 128 + 13);
    assert_eq!(EXIT_BROKEN_PIPE, 128 + 13);
    let broken = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    assert!(is_broken_pipe_io(&broken));
    assert_eq!(stdout_io_code(&broken), 128 + 13);
    let other = io::Error::new(io::ErrorKind::Other, "boom");
    assert!(!is_broken_pipe_io(&other));
    assert_eq!(stdout_io_code(&other), operational_code());
}

#[test]
fn override_broken_marker_reports_unreadable() {
    let mut fs = FakeFs::with_files(&[]);
    fs.broken.insert(
        PathBuf::from("/other"),
        "No such file or directory".to_owned(),
    );
    let err = discover(Path::new("/repo"), Some(Path::new("/other")), &fs).expect_err("broken");
    assert!(matches!(err, DiscoverError::UnreadableOverride { .. }));
    assert!(err.to_string().contains("workspace_unreadable"));
    assert!(err.to_string().contains("/other"));
    assert!(err.to_string().contains("No such file"));
}

#[test]
fn ancestor_broken_marker_reports_unreadable() {
    let mut fs = FakeFs::with_files(&[]);
    fs.broken.insert(
        PathBuf::from("/repo"),
        "No such file or directory".to_owned(),
    );
    let err = discover(Path::new("/repo/sub"), None, &fs).expect_err("broken ancestor");
    assert!(matches!(err, DiscoverError::UnreadableMarker { .. }));
    assert!(err.to_string().contains("workspace_unreadable"));
}

#[test]
fn expand_tilde_uses_home_and_leaves_user() {
    let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
        assert_eq!(expand_tilde(Path::new("~/ws")), PathBuf::from("~/ws"));
        return;
    };
    assert_eq!(
        expand_tilde(Path::new("~/ws")),
        PathBuf::from(home.clone()).join("ws")
    );
    assert_eq!(expand_tilde(Path::new("~")), PathBuf::from(home));
    assert_eq!(
        expand_tilde(Path::new("~other/ws")),
        PathBuf::from("~other/ws")
    );
    assert_eq!(expand_tilde(Path::new("/abs/ws")), PathBuf::from("/abs/ws"));
}

#[test]
fn override_display_joins_relative_and_keeps_absolute() {
    let base = Path::new("/base");
    assert_eq!(
        resolve_override_display(Path::new("sub/dir"), base),
        PathBuf::from("/base/sub/dir")
    );
    assert_eq!(
        resolve_override_display(Path::new("/abs/ws"), base),
        PathBuf::from("/abs/ws")
    );
}

#[test]
fn real_fs_canonicalizes_dotdot_and_trailing_slash() {
    let scratch = dx_test_scratch::scratch("dx-workspace-canonical-");
    let root = scratch.path().to_path_buf();
    let nested = root.join("a").join("b");
    std::fs::create_dir_all(&nested).expect("dirs");
    std::fs::write(root.join("MODULE.bazel"), "module(name = \"t\")\n").expect("marker");
    let canonical_root = std::fs::canonicalize(&root).expect("canonical");
    let dotdot = root.join("a").join("b").join("..").join("..");
    let found = discover_real(&dotdot, None).expect("dotdot");
    assert_eq!(found, canonical_root);
    let trailing = PathBuf::from(format!("{}/", root.display()));
    let found = discover_real(&trailing, None).expect("trailing");
    assert_eq!(found, canonical_root);
    let override_dotdot = root.join("./a/../");
    let found = discover_real(&nested, Some(override_dotdot.as_path())).expect("override dotdot");
    assert_eq!(found, canonicalize_or_keep(&root.join("a").join("..")));
    scratch.close().expect("cleanup");
}

#[cfg(unix)]
#[test]
fn real_fs_symlinked_root_canonicalizes() {
    let scratch = dx_test_scratch::scratch("dx-workspace-symlink-");
    let root = scratch.path().to_path_buf();
    let real = root.join("real");
    std::fs::create_dir_all(&real).expect("dirs");
    std::fs::write(real.join("MODULE.bazel"), "module(name = \"t\")\n").expect("marker");
    let link = root.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let canonical_real = std::fs::canonicalize(&real).expect("canonical");
    let found = discover_real(&link.join("sub"), None).expect("symlink start");
    assert_eq!(found, canonical_real);
    let found = discover_real(&root, Some(link.as_path())).expect("symlink override");
    assert_eq!(found, canonical_real);
    scratch.close().expect("cleanup");
}

#[cfg(unix)]
#[test]
fn real_fs_broken_marker_carries_io_hint() {
    let scratch = dx_test_scratch::scratch("dx-workspace-broken-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir_all(&root).expect("dirs");
    let missing = root.join("missing-target");
    let marker = root.join("MODULE.bazel");
    std::os::unix::fs::symlink(&missing, &marker).expect("broken link");
    let err = discover_real(&root, Some(root.as_path())).expect_err("broken override");
    assert!(
        matches!(err, DiscoverError::UnreadableOverride { .. }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("workspace_unreadable"));
    let err = discover_real(&root.join("sub"), None).expect_err("broken ancestor");
    assert!(
        matches!(err, DiscoverError::UnreadableMarker { .. }),
        "got {err:?}"
    );
    scratch.close().expect("cleanup");
}

#[test]
fn real_fs_override_keeps_display_path() {
    let scratch = dx_test_scratch::scratch("dx-workspace-display-");
    let root = scratch.path().to_path_buf();
    std::fs::create_dir_all(&root).expect("dirs");
    let display = root.join("./missing/../missing");
    let err = discover_real(&root, Some(display.as_path())).expect_err("missing");
    match err {
        DiscoverError::InvalidOverride { path } => {
            assert_eq!(path, resolve_override_display(display.as_path(), &root));
        }
        other => panic!("expected InvalidOverride, got {other:?}"),
    }
    scratch.close().expect("cleanup");
}

const SELECTION_PROBE_FILTER: &str = "selection_argv_probe_marker";
const UNICODE_PROBE_ARG: &str = "dx_argv_probe with spaces héllo";

fn selection_probe_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

#[test]
fn selected_test_observes_its_selection_argv_probe_marker() {
    let args = selection_probe_args();
    if !args.iter().any(|arg| arg.contains(SELECTION_PROBE_FILTER)) {
        return;
    }
    assert!(
        args.iter().any(|arg| arg == UNICODE_PROBE_ARG),
        "verbatim argument must reach the test binary alongside the filter: {args:?}"
    );
}

#[test]
#[ignore]
fn ignored_selection_probe_runs_only_when_explicitly_selected() {
    let args = selection_probe_args();
    assert!(
        args.iter().any(|arg| arg == "--ignored"),
        "explicit ignored selection must reach the test binary: {args:?}"
    );
}

#[test]
fn startup_option_accepts_the_selectable_equals_tokens_whole() {
    for token in [
        "--output_base=/tmp/dx-base",
        "--output_user_root=/tmp/dx-root",
        "--output_base=/tmp/with spaces/héllo",
        "--output_base=relative/dir",
    ] {
        assert_eq!(
            validate_startup_option(token).expect("selectable"),
            token,
            "the token must survive validation untouched"
        );
    }
}

#[test]
fn startup_option_rejects_missing_and_malformed_values() {
    assert!(matches!(
        validate_startup_option("--output_base"),
        Err(StartupOptionError::MissingValue { .. })
    ));
    assert!(matches!(
        validate_startup_option("--output_user_root"),
        Err(StartupOptionError::MissingValue { .. })
    ));
    assert!(matches!(
        validate_startup_option("--output_base="),
        Err(StartupOptionError::EmptyValue { .. })
    ));
    for token in ["output_base=/tmp/x", "--", "", "--=x"] {
        assert!(
            matches!(
                validate_startup_option(token),
                Err(StartupOptionError::NotAnOption { .. })
            ),
            "{token:?} is no startup option"
        );
    }
    let text = validate_startup_option("--output_base=")
        .expect_err("empty")
        .to_string();
    assert!(text.contains("--output_base"), "{text}");
}

#[test]
fn startup_option_rejects_the_managed_policy_names() {
    for token in [
        "--nohome_rc",
        "--home_rc=false",
        "--nosystem_rc",
        "--bazelrc=/tmp/rc",
        "--host_jvm_args=-Xmx1g",
        "--server_jvm_out=/tmp/jvm",
    ] {
        let error = validate_startup_option(token).expect_err("managed stays managed");
        assert!(
            matches!(error, StartupOptionError::ConflictingOption { .. }),
            "{token}: got {error:?}"
        );
        assert!(error.to_string().contains("managed"), "{error}");
    }
}

#[test]
fn startup_options_splice_before_the_verb_and_keep_one_token_each() {
    let mut argv = vec![
        "bazel".to_owned(),
        "--nohome_rc".to_owned(),
        "--nosystem_rc".to_owned(),
        "build".to_owned(),
        "//...".to_owned(),
    ];
    splice_startup_options(
        &mut argv,
        &[
            "--output_base=/tmp/with spaces".to_owned(),
            "--output_user_root=/tmp/héllo".to_owned(),
        ],
    );
    assert_eq!(
        argv,
        vec![
            "bazel",
            "--output_base=/tmp/with spaces",
            "--output_user_root=/tmp/héllo",
            "--nohome_rc",
            "--nosystem_rc",
            "build",
            "//...",
        ]
    );
    let mut bare = vec!["bazel".to_owned(), "query".to_owned(), "--".to_owned()];
    splice_startup_options(&mut bare, &[]);
    assert_eq!(bare, vec!["bazel", "query", "--"]);
}
