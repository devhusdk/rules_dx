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
fn bazel_run_dir_keeps_absolute_values_and_falls_back_to_cwd() {
    let scratch = dx_test_scratch::scratch("dx-bazel-run-dir-");
    let cwd = scratch.path().join("cwd");
    let given = scratch.path().join("given");
    std::fs::create_dir_all(&cwd).expect("cwd");
    std::fs::create_dir_all(&given).expect("given");
    let canonical_cwd = cwd.canonicalize().expect("canonical cwd");
    assert_eq!(
        bazel_run_dir(Some(given.as_os_str()), &cwd),
        given.canonicalize().expect("canonical given")
    );
    assert_eq!(
        bazel_run_dir(Some(std::ffi::OsStr::new("relative/x")), &cwd),
        canonical_cwd
    );
    assert_eq!(bazel_run_dir(None, &cwd), canonical_cwd);
    let missing = scratch.path().join("missing");
    assert_eq!(bazel_run_dir(Some(missing.as_os_str()), &cwd), missing);
    scratch.close().expect("cleanup");
}

#[test]
fn current_directory_is_the_process_cwd_outside_bazel_run() {
    assert!(std::env::var_os("BUILD_WORKING_DIRECTORY").is_none());
    assert_eq!(
        current_directory().expect("current directory"),
        canonicalize_or_keep(&std::env::current_dir().expect("cwd"))
    );
}

#[test]
fn bazel_run_dir_resolves_workspace_and_here_outside_the_runfiles_cwd() {
    let scratch = dx_test_scratch::scratch("dx-bazel-run-here-");
    let ws = scratch.path().join("ws");
    let sub = ws.join("pkg").join("sub");
    let runfiles = scratch.path().join("runfiles");
    std::fs::create_dir_all(&sub).expect("sub");
    std::fs::create_dir_all(&runfiles).expect("runfiles");
    let workspace = bazel_run_dir(Some(ws.as_os_str()), &runfiles);
    let here = bazel_run_dir(Some(sub.as_os_str()), &runfiles);
    assert_eq!(workspace, ws.canonicalize().expect("canonical ws"));
    assert_eq!(
        here.strip_prefix(&workspace)
            .expect("here is under the workspace"),
        Path::new("pkg").join("sub")
    );
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
    for name in STARTUP_OPTIONS {
        assert!(is_startup_option(&format!("--{name}=x")), "--{name}=x");
        assert!(is_startup_option(&format!("--{name}")), "--{name}");
    }
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
fn workflow_argv_orders_startup_command_user_required_labels() {
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
        &[],
    )
    .expect("argv");
    assert_eq!(
        argv,
        vec![
            "bazel",
            "--nohome_rc",
            "--nosystem_rc",
            "build",
            "--jobs=4",
            "--keep_going",
            "--keep_going",
            "--config=dx",
            "//...",
        ]
    );
}

#[test]
fn workflow_argv_required_options_follow_user_config() {
    let argv = build_workflow_argv(
        "test",
        &[
            "--config=remote".to_owned(),
            "--remote_download_minimal".to_owned(),
            "--jobs=4".to_owned(),
        ],
        &["--remote_download_outputs=all".to_owned()],
        &[],
        &["//...".to_owned()],
        &[],
    )
    .expect("argv");
    assert_eq!(
        argv,
        vec![
            "bazel",
            "--nohome_rc",
            "--nosystem_rc",
            "test",
            "--config=remote",
            "--remote_download_minimal",
            "--jobs=4",
            "--remote_download_outputs=all",
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
        &[],
    )
    .expect_err("startup");
    assert_eq!(
        err,
        ForwardError::StartupOption {
            flag: "output_base".to_owned(),
        }
    );
    assert!(err.to_string().contains("native bazel"));
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
            &[],
        )
        .expect_err("binary args");
        assert_eq!(
            err,
            ForwardError::TestBinaryArgs {
                flag: "test_arg".to_owned(),
            },
            "{command}"
        );
        assert!(err.to_string().contains("native bazel"), "{command}: {err}");
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
            &[],
        )
        .expect("argv");
        assert_eq!(
            argv,
            vec![
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                command,
                "--test_arg=--exact",
                "--test_arg",
                "case with spaces héllo",
                "--jobs=4",
                "--keep_going",
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
        &[],
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
        &[],
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
        &[],
    )
    .expect_err("startup option");
    assert!(matches!(err, ForwardError::StartupOption { .. }));
}

#[test]
fn qualified_startup_options_validate_as_single_tokens() {
    for token in [
        "--output_base=/tmp/x",
        "--output_user_root=/tmp/y",
        "--output_base=/tmp/a b/ünïcode",
    ] {
        assert_eq!(
            validate_startup_option(token).expect("qualified"),
            token.to_owned(),
            "{token} must pass through as one token"
        );
    }
    for token in ["--output_base", "--output_base=", "--output_user_root"] {
        let err = validate_startup_option(token).expect_err("missing value");
        assert_eq!(
            err,
            StartupOptionError::MissingValue {
                flag: token
                    .strip_prefix("--")
                    .expect("flag")
                    .split('=')
                    .next()
                    .expect("name")
                    .to_owned(),
            },
            "{token}"
        );
        assert!(err.to_string().contains("missing value"), "{token}: {err}");
    }
    for (token, flag) in [
        ("--jobs=4", "jobs"),
        ("--home_rc", "home_rc"),
        ("--nohome_rc", "nohome_rc"),
        ("--bazelrc=/tmp/rc", "bazelrc"),
        ("--host_jvm_args=-Xmx1g", "host_jvm_args"),
        ("--server_jvm_out=/tmp/jvm.out", "server_jvm_out"),
        ("--bogus=1", "bogus"),
        ("plain", "plain"),
        ("", ""),
        ("output_base=hunter2", "output_base"),
        (
            "https://user:hunter2@cache.example",
            "https://(credentials)@cache.example",
        ),
    ] {
        let err = validate_startup_option(token).expect_err("unsupported");
        assert_eq!(
            err,
            StartupOptionError::UnsupportedStartup {
                flag: flag.to_owned(),
            },
            "{token:?}"
        );
        assert!(err.to_string().contains("unsupported"), "{token:?}: {err}");
    }
}

#[test]
fn url_credentials_are_redacted_and_other_text_is_kept() {
    for (text, want) in [
        (
            "--remote_cache=grpcs://user:pw@cache.example:443/x",
            "--remote_cache=grpcs://(credentials)@cache.example:443/x",
        ),
        (
            "a https://t@h/p and http://u:p@g",
            "a https://(credentials)@h/p and http://(credentials)@g",
        ),
        ("https://cache.example/a@b", "https://cache.example/a@b"),
        ("no url here", "no url here"),
        ("trailing://", "trailing://"),
    ] {
        assert_eq!(redact_url_credentials(text), want, "{text}");
    }
}

#[test]
fn startup_argv_prefixes_the_launcher_and_managed_policy() {
    assert_eq!(
        startup_argv(&[]),
        vec![
            "bazel".to_owned(),
            "--nohome_rc".to_owned(),
            "--nosystem_rc".to_owned(),
        ]
    );
    assert_eq!(
        startup_argv(&[
            "--output_base=/tmp/a".to_owned(),
            "--output_user_root=/tmp/b".to_owned(),
        ]),
        vec![
            "bazel".to_owned(),
            "--nohome_rc".to_owned(),
            "--nosystem_rc".to_owned(),
            "--output_base=/tmp/a".to_owned(),
            "--output_user_root=/tmp/b".to_owned(),
        ]
    );
}

#[test]
fn workflow_argv_inserts_startup_options_before_the_verb() {
    let argv = build_workflow_argv(
        "test",
        &["--jobs=4".to_owned()],
        &["--config=dx".to_owned()],
        &[],
        &["//...".to_owned()],
        &[
            "--output_base=/tmp/a b".to_owned(),
            "--output_user_root=/tmp/ünïcode".to_owned(),
        ],
    )
    .expect("argv");
    assert_eq!(
        argv,
        vec![
            "bazel",
            "--nohome_rc",
            "--nosystem_rc",
            "--output_base=/tmp/a b",
            "--output_user_root=/tmp/ünïcode",
            "test",
            "--jobs=4",
            "--config=dx",
            "//...",
        ]
    );
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
            &[],
        )
        .expect_err("startup must fail");
        assert!(
            matches!(err, ForwardError::StartupOption { .. }),
            "{startup} produced {err:?}"
        );
        assert!(err.to_string().contains("native bazel"), "{startup}: {err}");
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
            &[],
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
        &[],
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
        &[],
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
        status: ChildStatus::exited(3),
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
fn default_withdrawable_run_checks_withdraw_only_before_launch() {
    let runner = FakeRunner {
        status: ChildStatus::exited(3),
    };
    let argv = ["bazel".to_owned(), "run".to_owned()];
    let ran = runner
        .run_withdrawable(&argv, Path::new("."), &[], &|| false)
        .expect("fake");
    assert_eq!(ran, Some(ChildStatus::exited(3)));
    let withdrawn = runner
        .run_withdrawable(&argv, Path::new("."), &[], &|| true)
        .expect("fake");
    assert_eq!(withdrawn, None);
}

#[test]
fn child_status_from_exit_keeps_the_code_or_the_known_signal() {
    assert_eq!(
        ChildStatus::from(lifecycle::Exit::Code(3)),
        ChildStatus {
            code: Some(3),
            signal: None
        }
    );
    assert_eq!(
        ChildStatus::from(lifecycle::Exit::Signal(11)),
        ChildStatus {
            code: None,
            signal: Some(11)
        }
    );
    assert_eq!(
        ChildStatus::from(lifecycle::Exit::Signal(0)),
        ChildStatus {
            code: None,
            signal: None
        }
    );
}

/// A probe argv that runs the Bazel-built probe with the given flags.
pub(crate) fn probe_argv(flags: &[&str]) -> Vec<String> {
    let mut argv = vec![dx_testing::process_probe().to_string_lossy().into_owned()];
    argv.extend(flags.iter().map(|flag| (*flag).to_owned()));
    argv
}

#[test]
fn pinned_tool_defaults_to_unset() {
    let runner = FakeRunner {
        status: ChildStatus::exited(0),
    };
    assert_eq!(runner.pinned_tool("DX_UV_BIN"), PinnedVar::Unset);
    assert_eq!(runner.pinned_tool("DX_GITLEAKS_BIN"), PinnedVar::Unset);
}

#[test]
fn pinned_var_classifies_values() {
    use std::ffi::OsString;
    assert_eq!(PinnedVar::classify(None), PinnedVar::Unset);
    assert_eq!(PinnedVar::classify(Some(OsString::new())), PinnedVar::Unset);
    assert_eq!(
        PinnedVar::classify(Some("relative/x".into())),
        PinnedVar::NotAbsolute
    );
    let absolute = std::env::temp_dir().join("uv");
    assert_eq!(
        PinnedVar::classify(Some(absolute.clone().into())),
        PinnedVar::Path(absolute.clone())
    );
    assert_eq!(PinnedVar::Path(absolute.clone()).path(), Some(absolute));
    assert_eq!(PinnedVar::NotAbsolute.path(), None);
}

#[test]
fn a_recorded_stop_signal_is_reported_with_its_exit_code() {
    assert_eq!(interrupted(), None);
    record_interrupt(2);
    assert_eq!(interrupted(), Some(2));
    assert_eq!(interrupted_code(2), 130);
    assert_eq!(interrupted_code(15), 143);
}

#[test]
fn stdout_broken_pipe_maps_to_141() {
    assert_eq!(broken_pipe_code(), 128 + 13);
    assert_eq!(EXIT_BROKEN_PIPE, 128 + 13);
    let broken = io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe");
    assert!(is_broken_pipe_io(&broken));
    assert_eq!(stdout_io_code(&broken), 128 + 13);
    let other = io::Error::other("boom");
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

#[test]
fn disk_space_is_read_from_df_lines_and_byte_counts() {
    let df =
        "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/x 1000 900 100 90% /\n";
    assert_eq!(
        space_from_df(df),
        Some(DiskSpace {
            free: 100 * 1024,
            total: 1000 * 1024
        })
    );
    assert_eq!(
        space_from_counts("25", "100", 1),
        Some(DiskSpace {
            free: 25,
            total: 100
        })
    );
    assert_eq!(space_from_counts("1", "0", 1), None);
    assert_eq!(space_from_counts("x", "5", 1), None);
    assert_eq!(space_from_df(""), None);
}

#[test]
fn free_disk_bytes_defaults_to_unknown() {
    struct Plain;
    impl Runner for Plain {
        fn run(&self, _: &[String], _: &Path, _: &[(&str, &str)]) -> io::Result<ChildStatus> {
            Ok(ChildStatus::exited(0))
        }
    }
    assert_eq!(Plain.free_disk_bytes(Path::new("/")), None);
}

#[test]
fn disk_space_reads_the_disk_holding_a_real_directory() {
    let space = disk_space(&std::env::temp_dir()).expect("reads the temp dir");
    assert!(space.total > 0 && space.free <= space.total, "{space:?}");
}

#[test]
fn disk_space_reads_the_nearest_existing_ancestor() {
    let missing = std::env::temp_dir().join("dx-missing").join("child");
    assert!(!missing.exists());
    let space = disk_space(&missing).expect("reads the temp dir");
    assert!(space.total > 0 && space.free <= space.total, "{space:?}");
}

#[test]
fn disk_space_without_an_existing_ancestor_is_none() {
    assert_eq!(disk_space(Path::new("")), None);
}

#[test]
fn bazel_head_puts_startup_before_the_verb_and_options_after_verb_options() {
    let head = BazelHead {
        startup: vec!["--output_base=/ob".to_owned()],
        options: vec!["--config=dx_dev".to_owned()],
        settings: Vec::new(),
    };
    let argv = head.argv("run");
    let mut expected = startup_argv(&head.startup);
    expected.push("run".to_owned());
    expected.extend(build_verb_options("run", &head.options));
    expected.push("--config=dx_dev".to_owned());
    assert_eq!(argv, expected);
    let verb = argv.iter().position(|arg| arg == "run").expect("verb");
    let config = argv
        .iter()
        .position(|arg| arg == "--config=dx_dev")
        .expect("config");
    assert!(argv.contains(&"--output_base=/ob".to_owned()));
    assert!(argv.iter().position(|arg| arg == "--output_base=/ob") < Some(verb));
    assert!(config > verb);
}

#[test]
fn bazel_head_settings_follow_options_for_build_like_verbs_only() {
    let head = BazelHead {
        startup: Vec::new(),
        options: vec!["--config=dx_dev".to_owned()],
        settings: vec!["--@rules_dx//config:x=true".to_owned()],
    };
    for verb in SHARED_SETTINGS_VERBS {
        let argv = head.argv(verb);
        assert_eq!(
            argv.last().map(String::as_str),
            Some("--@rules_dx//config:x=true"),
            "{verb}"
        );
    }
    for verb in ["fetch", "query", "info"] {
        assert!(
            !head.argv(verb).iter().any(|arg| arg.contains("config:x")),
            "{verb}"
        );
    }
    let bare = BazelHead {
        settings: Vec::new(),
        ..head.clone()
    };
    assert_eq!(bare.argv("build").len() + 1, head.argv("build").len());
}

#[test]
fn default_bazel_head_is_the_bare_startup_verb_and_verb_options() {
    let mut expected = startup_argv(&[]);
    expected.push("build".to_owned());
    expected.extend(build_verb_options("build", &[]));
    assert_eq!(BazelHead::default().argv("build"), expected);
}

#[test]
fn probe_is_none_when_the_program_is_missing() {
    assert_eq!(probe("dx-no-such-program-2174", ["x"]), None);
}

#[cfg(unix)]
#[test]
fn probe_returns_stdout_only_on_success() {
    assert_eq!(probe("sh", ["-c", "echo hi"]).as_deref(), Some("hi\n"));
    assert_eq!(probe("sh", ["-c", "echo partial; exit 1"]), None);
    assert_eq!(probe("false", std::iter::empty::<&str>()), None);
}

#[cfg(windows)]
#[test]
fn probe_returns_stdout_only_on_success() {
    assert!(probe("cmd", ["/C", "echo hi"]).is_some_and(|text| text.starts_with("hi")));
    assert_eq!(probe("cmd", ["/C", "exit 1"]), None);
}

#[cfg(unix)]
#[test]
fn remove_tree_deletes_read_only_trees() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = dx_test_scratch::scratch("dx-remove-tree-");
    let build = |name: &str| {
        let dir = scratch.path().join(name).join("ro");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("f");
        std::fs::write(&file, "x").expect("file");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).expect("file");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("dir");
        scratch.path().join(name)
    };
    let plain = build("plain");
    let tree = build("tree");
    let blocked = std::fs::remove_dir_all(&plain).is_err();
    if !blocked {
        // Running as root: permission bits are ignored, so only the result is checked.
        assert!(remove_tree(&tree));
        assert!(!tree.exists());
        return;
    }
    assert!(remove_tree(&tree));
    assert!(!tree.exists());
    make_writable(&plain);
}

#[cfg(unix)]
#[test]
fn remove_tree_never_chmods_a_symlink_target() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = dx_test_scratch::scratch("dx-remove-tree-link-");
    let outside = scratch.path().join("outside");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o555)).expect("outside");
    let tree = scratch.path().join("tree");
    std::fs::create_dir_all(&tree).expect("tree");
    std::os::unix::fs::symlink(&outside, tree.join("link")).expect("link");
    make_writable(&tree);
    assert!(remove_tree(&tree));
    assert!(outside.is_dir());
    let mode = std::fs::metadata(&outside)
        .expect("outside")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o555);
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o755)).expect("restore");
}

fn os_argv(argv: Vec<String>) -> Vec<std::ffi::OsString> {
    argv.into_iter().map(Into::into).collect()
}

#[test]
fn first_line_within_reads_the_first_stdout_line() {
    let argv = os_argv(probe_argv(&["--stdout-text=tool 1.2.3"]));
    assert_eq!(
        first_line_within(argv, &|| None),
        Some("tool 1.2.3".to_owned())
    );
}

#[test]
fn first_line_within_rejects_a_failing_tool() {
    let argv = os_argv(probe_argv(&["--exit-code=3"]));
    assert_eq!(first_line_within(argv, &|| None), None);
}

#[test]
fn first_line_within_runs_with_no_path() {
    let argv = os_argv(probe_argv(&["--forbid-env=PATH", "--stdout-text=x"]));
    assert_eq!(first_line_within(argv, &|| None), Some("x".to_owned()));
}

#[test]
fn version_line_of_a_missing_tool_is_none() {
    assert_eq!(
        version_line(Path::new("/nonexistent-dx-tool"), &|| None),
        None
    );
}

#[test]
fn first_line_within_ends_early_when_stopped() {
    let argv = os_argv(probe_argv(&["--sleep-ms=60000"]));
    let started = std::time::Instant::now();
    assert_eq!(first_line_within(argv, &|| Some(15)), None);
    assert!(started.elapsed() < VERSION_TIMEOUT);
}

#[test]
fn output_base_of_execroot_reads_the_grandparent() {
    assert_eq!(
        output_base_of_execroot(Path::new("/x/ob/execroot/_main")),
        Some(PathBuf::from("/x/ob"))
    );
}

#[cfg(windows)]
#[test]
fn output_base_of_execroot_reads_windows_paths() {
    assert_eq!(
        output_base_of_execroot(Path::new("C:/bz/abc/execroot/_main")),
        Some(PathBuf::from("C:/bz/abc"))
    );
}

#[test]
fn output_base_of_execroot_rejects_other_shapes() {
    for value in [
        "",
        "_main",
        "execroot/_main",
        "/x/ob/_main",
        "/",
        "/execroot",
    ] {
        assert_eq!(output_base_of_execroot(Path::new(value)), None, "{value}");
    }
}
