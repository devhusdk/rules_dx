use super::*;

#[test]
fn malformed_records_and_foreign_parent_files_fail_closed() {
    let scratch = dx_test_scratch::scratch("setup-record-failures-");
    let setup = pair('a', 'b');
    let setups = scratch.path().join("setups");
    let record = setups.join(setup_hex(&setup));
    fs::create_dir_all(&record).expect("record");
    assert!(matches!(
        ensure_setup_record(&setups, &setup),
        Err(CommitError::RecordMismatch { .. })
    ));
    symlink_dir(
        Path::new(&expected_environment_target(&setup)),
        &record.join(ENVIRONMENT_LINK_NAME),
    )
    .expect("environment link");
    assert!(matches!(
        ensure_setup_record(&setups, &setup),
        Err(CommitError::RecordMismatch { .. })
    ));
    let file = scratch.path().join("foreign");
    fs::write(&file, "foreign").expect("file");
    assert!(matches!(
        ensure_setup_record(&file, &setup),
        Err(CommitError::Install { .. })
    ));
    assert!(matches!(
        install_and_swap(&file, None, &setup),
        Err(CommitError::Install { .. })
    ));
    assert!(matches!(
        clear_staged_pointer(&file.join("child")),
        Err(CommitError::Install { .. })
    ));
    assert!(matches!(
        map_lock_error(LockError::Busy { path: file.clone() }),
        CommitError::Busy { .. }
    ));
    let sides = || PreparedSides {
        prepared_environment: Some(setup.environment.clone()),
        prepared_generated: Some(setup.generated.clone()),
        empty_environment: setup.environment.clone(),
        empty_generated: setup.generated.clone(),
    };
    assert!(matches!(
        commit_prepared(&file, sides()),
        Err(CommitError::WorkspaceRoot { .. })
    ));
    fs::write(scratch.path().join(".dx"), "foreign").expect("dx collision");
    assert!(matches!(
        commit_pair(scratch.path(), &setup),
        Err(CommitError::Install { .. })
    ));
    assert!(matches!(
        commit_prepared(scratch.path(), sides()),
        Err(CommitError::Install { .. })
    ));
    assert!(matches!(
        read_current_pair(scratch.path()),
        Err(CommitError::CurrentInvalid { .. })
    ));
    assert_eq!(
        fs::read_to_string(file).expect("foreign preserved"),
        "foreign"
    );
}

#[test]
fn frozen_identities_match_codegen_and_env() {
    assert_eq!(
        CODEGEN_ASPECT,
        "//generation:codegen.bzl%dx_codegen_plan_aspect"
    );
    assert_eq!(ENV_ASPECT, "//env:plan.bzl%dx_env_plan_aspect");
    assert_eq!(CODEGEN_OUTPUT_GROUP, "dx_codegen_plans");
    assert_eq!(ENV_OUTPUT_GROUP, "dx_env_plans");
    assert_eq!(CODEGEN_REPOSITORY_TARGET, "//dx:codegen");
    assert_eq!(ENV_REPOSITORY_TARGET, "//dx:env");
}

#[test]
fn empty_scope_selects_both_repository_targets() {
    assert_eq!(resolve_scope(&[]), Ok(SetupScope::Repository));
    assert_eq!(
        scope_targets(&SetupScope::Repository),
        vec!["//dx:codegen", "//dx:env"]
    );
}

#[test]
fn root_plan_composes_baseline_combined_request() {
    let plan = dx_roots::repository_plan();
    assert_eq!(
        targets_for_root_plan(&plan),
        vec!["//dx:codegen", "//dx:env"]
    );
    let request = plan_request_for_root_plan(&plan);
    assert_eq!(request, plan_request(&SetupScope::Repository));
    assert_eq!(
        build_argv_for_plan(&plan),
        vec![
            "build".to_owned(),
            "//dx:codegen".to_owned(),
            "//dx:env".to_owned(),
            format!("--aspects={CODEGEN_ASPECT}"),
            format!("--aspects={ENV_ASPECT}"),
            format!("--output_groups={CODEGEN_OUTPUT_GROUP}"),
            format!("--output_groups={ENV_OUTPUT_GROUP}"),
        ]
    );
}

#[test]
fn root_plan_passes_aggregate_roots_through() {
    let plan = dx_roots::RepositoryRootPlan::monolithic_aggregate("//dx:setup_roots");
    assert_eq!(
        targets_for_root_plan(&plan),
        vec!["//dx:setup_roots".to_owned()]
    );
    assert_eq!(
        plan_request_for_root_plan(&plan).roots,
        vec!["//dx:setup_roots".to_owned()]
    );
}

#[test]
fn exact_labels_pass_through() {
    for label in ["//app:server", "@rules_dx//generation:result_proto_rs"] {
        let scope = resolve_scope(&[label.to_owned()]).expect("exact label");
        assert_eq!(scope, SetupScope::Exact(label.to_owned()));
        assert_eq!(scope_targets(&scope), vec![label.to_owned()]);
    }
}

#[test]
fn scope_rejects_non_exact_inputs() {
    assert_eq!(
        resolve_scope(&["//a:x".to_owned(), "//b:y".to_owned()]),
        Err(ScopeError::MultipleTargets { count: 2 })
    );
    for pattern in ["//...", "//app/...", "//app:*", "@repo//pkg:all?"] {
        assert_eq!(
            resolve_scope(&[pattern.to_owned()]),
            Err(ScopeError::TargetPattern {
                value: pattern.to_owned(),
            }),
            "pattern {pattern:?} must fail"
        );
    }
    for other in ["app/server", "gen", "rust", "--profile=fast", ":relative"] {
        assert_eq!(
            resolve_scope(&[other.to_owned()]),
            Err(ScopeError::NotTargetLabel {
                value: other.to_owned(),
            }),
            "non-label {other:?} must fail"
        );
    }
}

#[test]
fn scope_errors_display() {
    assert!(ScopeError::MultipleTargets { count: 2 }
        .to_string()
        .contains("at most one target"));
}

#[test]
fn repository_request_unions_both_sides() {
    let request = plan_request(&SetupScope::Repository);
    assert_eq!(request.roots, vec!["//dx:codegen", "//dx:env"]);
    assert_eq!(
        request.aspects,
        vec![
            "//generation:codegen.bzl%dx_codegen_plan_aspect",
            "//env:plan.bzl%dx_env_plan_aspect",
        ]
    );
    assert_eq!(
        request.output_groups,
        vec!["dx_codegen_plans", "dx_env_plans"]
    );
}

#[test]
fn exact_request_applies_both_sides_to_one_root() {
    let request = plan_request(&SetupScope::Exact("//app:server".to_owned()));
    assert_eq!(request.roots, vec!["//app:server"]);
    assert_eq!(request.aspects.len(), 2);
    assert_eq!(request.output_groups.len(), 2);
}

#[test]
fn request_sides_are_deterministic() {
    let first = plan_request(&SetupScope::Repository);
    let second = plan_request(&SetupScope::Repository);
    assert_eq!(first, second);
}

fn generation(tag: char) -> GenerationId {
    GenerationId::new(&tag.to_string().repeat(64)).expect("fixture digest")
}

fn pair_inputs(
    prepared_environment: Option<GenerationId>,
    prepared_generated: Option<GenerationId>,
    current: Option<SetupPair>,
) -> PairInputs {
    PairInputs {
        prepared_environment,
        prepared_generated,
        current,
        empty_environment: generation('e'),
        empty_generated: generation('0'),
    }
}

#[test]
fn generation_ids_validate_digest_shape() {
    assert_eq!(generation('a').as_str(), &"a".repeat(64));
    assert!(GenerationId::new(&"0".repeat(64)).is_ok());
    for bad in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        format!("{}!", "a".repeat(63)),
    ] {
        assert!(GenerationId::new(&bad).is_err(), "digest {bad:?} must fail");
    }
    assert!(GenerationIdError("x".to_owned())
        .to_string()
        .contains("generation id"));
}

#[test]
fn generation_ids_from_digests_need_no_validation() {
    for byte in 0u8..=255 {
        let digest = [byte; 32];
        let id = GenerationId::from_digest(digest);
        assert_eq!(id.as_str(), dx_digest::to_hex(&digest));
        assert_eq!(id.as_str().len(), 64);
        assert_eq!(GenerationId::new(id.as_str()), Ok(id.clone()));
    }
}

#[test]
fn setup_with_both_sides_ignores_current() {
    let pair = resolve_pair(pair_inputs(
        Some(generation('1')),
        Some(generation('2')),
        Some(SetupPair {
            environment: generation('3'),
            generated: generation('4'),
        }),
    ))
    .expect("pair");
    assert_eq!(pair.environment, generation('1'));
    assert_eq!(pair.generated, generation('2'));
}

#[test]
fn independent_sides_carry_the_current_opposite_forward() {
    let current = || SetupPair {
        environment: generation('3'),
        generated: generation('4'),
    };
    let env_pair =
        resolve_pair(pair_inputs(Some(generation('1')), None, Some(current()))).expect("env pair");
    assert_eq!(env_pair.environment, generation('1'));
    assert_eq!(env_pair.generated, generation('4'));
    let codegen_pair = resolve_pair(pair_inputs(None, Some(generation('2')), Some(current())))
        .expect("codegen pair");
    assert_eq!(codegen_pair.environment, generation('3'));
    assert_eq!(codegen_pair.generated, generation('2'));
}

#[test]
fn first_selection_pairs_with_managed_empty_generations() {
    let env_pair = resolve_pair(pair_inputs(Some(generation('1')), None, None)).expect("env pair");
    assert_eq!(env_pair.environment, generation('1'));
    assert_eq!(env_pair.generated, generation('0'));
    let codegen_pair =
        resolve_pair(pair_inputs(None, Some(generation('2')), None)).expect("codegen pair");
    assert_eq!(codegen_pair.environment, generation('e'));
    assert_eq!(codegen_pair.generated, generation('2'));
    let setup_pair = resolve_pair(pair_inputs(
        Some(generation('1')),
        Some(generation('2')),
        None,
    ))
    .expect("setup pair");
    assert_eq!(setup_pair.environment, generation('1'));
    assert_eq!(setup_pair.generated, generation('2'));
}

#[test]
fn scope_without_either_capability_fails() {
    assert_eq!(
        resolve_pair(pair_inputs(None, None, None)),
        Err(ResolveError::NoCapability)
    );
    assert_eq!(
        resolve_pair(pair_inputs(
            None,
            None,
            Some(SetupPair {
                environment: generation('3'),
                generated: generation('4'),
            }),
        )),
        Err(ResolveError::NoCapability)
    );
    assert!(ResolveError::NoCapability
        .to_string()
        .contains("no capability"));
}

fn pair(env: char, gen: char) -> SetupPair {
    SetupPair {
        environment: generation(env),
        generated: generation(gen),
    }
}

fn workspace_of(root: &Path) -> PathBuf {
    root.join("ws")
}

fn commit_ok(workspace: &Path, pair: &SetupPair) -> CommitOutcome {
    commit_pair(workspace, pair).expect("commit succeeds")
}

#[test]
fn commit_lock_deadline_matches_env_owner() {
    assert_eq!(COMMIT_LOCK_TIMEOUT, dx_env::LOCK_TIMEOUT);
}

#[test]
fn setup_hex_is_deterministic_lowercase_64() {
    let first = setup_hex(&pair('1', '2'));
    let second = setup_hex(&pair('1', '2'));
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(first, first.to_lowercase());
    assert_ne!(first, setup_hex(&pair('1', '3')));
    assert_ne!(first, setup_hex(&pair('3', '2')));
    assert!(setup_fingerprint(&pair('1', '2')).starts_with("dx-setup/v0\n"));
    insta::assert_snapshot!(setup_fingerprint(&pair('1', '2')), @"dx-setup/v0
1111111111111111111111111111111111111111111111111111111111111111
2222222222222222222222222222222222222222222222222222222222222222
");
}

#[test]
fn fresh_install_noop_and_replacement() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-lifecycle-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(read_current_pair(&workspace).expect("read"), None);
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    let selected = read_current_pair(&workspace)
        .expect("read")
        .expect("selected");
    assert_eq!(selected, pair('1', '2'));
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::AlreadyCurrent
    );
    assert_eq!(
        commit_ok(&workspace, &pair('3', '4')),
        CommitOutcome::InstalledReplacement
    );
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('3', '4'))
    );
    let setups = workspace.join(".dx").join("setups");
    let record = setups.join(setup_hex(&pair('3', '4')));
    assert!(record.join("environment").is_symlink());
    assert!(record.join("generated").is_symlink());
    assert_eq!(
        fs::read_link(setups.join("current")).expect("pointer"),
        PathBuf::from(setup_hex(&pair('3', '4')))
    );
    assert!(!setups.join("current.next").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn workspace_path_with_spaces_commits() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-with space-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(
        commit_ok(&workspace, &pair('a', 'b')),
        CommitOutcome::InstalledFresh
    );
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('a', 'b'))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prepared_commits_carry_forward_under_one_lock() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-carry-commit-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let sides = |env: Option<char>, gen: Option<char>| PreparedSides {
        prepared_environment: env.map(generation),
        prepared_generated: gen.map(generation),
        empty_environment: generation('e'),
        empty_generated: generation('0'),
    };
    let (first, outcome) = commit_prepared(&workspace, sides(Some('1'), None)).expect("env");
    assert_eq!(outcome, CommitOutcome::InstalledFresh);
    assert_eq!(first, pair('1', '0'));
    let (second, outcome) = commit_prepared(&workspace, sides(None, Some('2'))).expect("gen");
    assert_eq!(outcome, CommitOutcome::InstalledReplacement);
    assert_eq!(second, pair('1', '2'));
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('1', '2'))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn prepared_without_capability_fails_before_mutation() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-no-capability-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let error = commit_prepared(
        &workspace,
        PreparedSides {
            prepared_environment: None,
            prepared_generated: None,
            empty_environment: generation('e'),
            empty_generated: generation('0'),
        },
    )
    .unwrap_err();
    assert_eq!(error, CommitError::NoCapability);
    assert_eq!(read_current_pair(&workspace).expect("read"), None);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn record_mismatch_preserves_current() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-mismatch-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    let spoofed = pair('3', '4');
    let setups = workspace.join(".dx").join("setups");
    let record = setups.join(setup_hex(&spoofed));
    fs::create_dir_all(&record).expect("spoof record");
    symlink_dir(
        Path::new("../../environments/wrong"),
        &record.join("environment"),
    )
    .expect("wrong link");
    symlink_dir(
        Path::new(&expected_generated_target(&spoofed)),
        &record.join("generated"),
    )
    .expect("right link");
    let error = commit_pair(&workspace, &spoofed).unwrap_err();
    assert!(matches!(error, CommitError::RecordMismatch { .. }));
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('1', '2'))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unmanaged_current_states_fail_closed() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-unmanaged-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    let setups = workspace.join(".dx").join("setups");
    let current = setups.join("current");
    let pointed = fs::read_link(&current).expect("pointer");

    dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
    fs::write(&current, "not a symlink").expect("file pointer");
    assert!(matches!(
        read_current_pair(&workspace),
        Err(CommitError::CurrentInvalid { .. })
    ));
    assert!(matches!(
        commit_pair(&workspace, &pair('3', '4')),
        Err(CommitError::CurrentInvalid { .. })
    ));
    fs::remove_file(&current).expect("remove file pointer");

    fs::create_dir(&current).expect("dir pointer");
    assert!(matches!(
        read_current_pair(&workspace),
        Err(CommitError::CurrentInvalid { .. })
    ));
    fs::remove_dir(&current).expect("remove dir pointer");

    symlink_dir(Path::new("missing-setup"), &current).expect("dangling pointer");
    assert!(matches!(
        read_current_pair(&workspace),
        Err(CommitError::CurrentInvalid { .. })
    ));
    dx_test_scratch::remove_directory_link(&current).expect("remove dangling");
    symlink_dir(Path::new(&pointed), &current).expect("restore pointer");
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('1', '2'))
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn digest_spoofed_pointer_fails_closed() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-spoof-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    let setups = workspace.join(".dx").join("setups");
    let current = setups.join("current");
    let other = setup_hex(&pair('3', '4'));
    fs::create_dir_all(setups.join(&other)).expect("other record");
    symlink_dir(
        Path::new(&expected_environment_target(&pair('1', '2'))),
        &setups.join(&other).join("environment"),
    )
    .expect("env link");
    symlink_dir(
        Path::new(&expected_generated_target(&pair('1', '2'))),
        &setups.join(&other).join("generated"),
    )
    .expect("gen link");
    dx_test_scratch::remove_directory_link(&current).expect("remove pointer");
    symlink_dir(Path::new(&other), &current).expect("spoofed pointer");
    assert!(matches!(
        read_current_pair(&workspace),
        Err(CommitError::CurrentInvalid { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn stale_staged_pointer_is_reclaimed() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-stale-next-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let setups = workspace.join(".dx").join("setups");
    fs::create_dir_all(&setups).expect("setups");
    symlink_dir(Path::new("stale-target"), &setups.join("current.next")).expect("stale");
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    assert!(!setups.join("current.next").exists());
    let _ = fs::remove_dir_all(&root);
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(windows))]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[test]
fn staged_pointer_clear_matches_the_entry_kind() {
    let scratch = dx_test_scratch::scratch("dx-setup-test-stage-kinds-");
    let stage = scratch.path().join("current.next");
    assert!(
        clear_staged_pointer(&stage).is_ok(),
        "a missing staged pointer needs no clearing"
    );

    symlink_dir(Path::new("missing-setup"), &stage).expect("dangling directory link");
    clear_staged_pointer(&stage).expect("clear dangling directory link");
    assert!(stage.symlink_metadata().is_err(), "directory link removed");

    let file = scratch.path().join("foreign.txt");
    fs::write(&file, "foreign").expect("foreign file");
    symlink_file(&file, &stage).expect("file link");
    clear_staged_pointer(&stage).expect("clear file link");
    assert!(stage.symlink_metadata().is_err(), "file link removed");
    assert!(file.is_file(), "the linked file survives");

    fs::write(&stage, "foreign").expect("foreign staged file");
    clear_staged_pointer(&stage).expect("clear foreign staged file");
    assert!(stage.symlink_metadata().is_err(), "file removed");

    fs::create_dir(&stage).expect("foreign staged dir");
    assert!(
        matches!(
            clear_staged_pointer(&stage),
            Err(CommitError::Install { .. })
        ),
        "a staged directory stays refused"
    );
    assert!(stage.is_dir(), "the refused directory survives");
    let _ = fs::remove_dir_all(scratch.path());
}

#[test]
fn workspace_missing_fails() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-ws-missing-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let missing = root.join("no-such-dir");
    assert!(matches!(
        read_current_pair(&missing),
        Err(CommitError::WorkspaceRoot { .. })
    ));
    assert!(matches!(
        commit_pair(&missing, &pair('1', '2')),
        Err(CommitError::WorkspaceRoot { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn busy_lock_fails_after_deadline() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-busy-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let dx_dir = workspace.join(".dx");
    fs::create_dir_all(&dx_dir).expect("dx dir");
    let _held = dx_env::acquire_lock(&dx_dir, Duration::from_secs(10)).expect("hold commit lock");
    let error = commit_pair_with_timeout(&workspace, &pair('1', '2'), Duration::from_millis(1))
        .unwrap_err();
    assert!(matches!(error, CommitError::Busy { .. }));
    drop(_held);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn lock_open_failure_aborts() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-lock-open-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let dx_dir = workspace.join(".dx");
    fs::create_dir_all(&dx_dir).expect("dx dir");
    fs::create_dir_all(dx_dir.join(dx_env::LOCK_FILE_NAME)).expect("lock is a directory");
    assert!(matches!(
        commit_pair_with_timeout(&workspace, &pair('1', '2'), Duration::from_secs(1)),
        Err(CommitError::LockFailed { .. })
    ));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn concurrent_commits_serialize_with_idempotent_reuse() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-concurrent-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    let wanted: Vec<SetupPair> = (0..4)
        .map(|i| pair((b'1' + i) as char, (b'a' + i) as char))
        .collect();
    let outcomes = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let workspace_ref = &workspace;
                let candidate = wanted[i % wanted.len()].clone();
                scope.spawn(move || commit_pair(workspace_ref, &candidate))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("worker panics fail the test"))
            .collect::<Vec<_>>()
    });
    for outcome in &outcomes {
        assert!(outcome.is_ok(), "racing commit must succeed: {outcome:?}");
    }
    let setups = workspace.join(".dx").join("setups");
    for candidate in &wanted {
        let record = setups.join(setup_hex(candidate));
        assert!(record.join("environment").is_symlink());
        assert!(record.join("generated").is_symlink());
    }
    let selected = read_current_pair(&workspace)
        .expect("read")
        .expect("selected");
    assert!(wanted.contains(&selected));
    assert!(!setups.join("current.next").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn staged_directory_preserves_current() {
    let scratch = {
        let __scratch = dx_test_scratch::scratch("dx-setup-test-staged-dir-");
        std::fs::create_dir_all(__scratch.path().join("ws")).expect("create workspace");
        __scratch
    };
    let root = scratch.path().to_path_buf();
    let workspace = workspace_of(&root);
    assert_eq!(
        commit_ok(&workspace, &pair('1', '2')),
        CommitOutcome::InstalledFresh
    );
    let setups = workspace.join(".dx").join("setups");
    fs::create_dir_all(setups.join("current.next")).expect("foreign staged dir");
    assert!(matches!(
        commit_pair(&workspace, &pair('3', '4')),
        Err(CommitError::Install { .. })
    ));
    assert_eq!(
        read_current_pair(&workspace).expect("read"),
        Some(pair('1', '2'))
    );
    fs::remove_dir(setups.join("current.next")).expect("remove foreign staged dir");
    assert_eq!(
        commit_ok(&workspace, &pair('3', '4')),
        CommitOutcome::InstalledReplacement
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn commit_errors_display() {
    let errors = [
        CommitError::WorkspaceRoot {
            path: PathBuf::from("/ws"),
        },
        CommitError::Busy {
            path: PathBuf::from("/ws/.dx/.commit.lock"),
        },
        CommitError::LockFailed {
            path: PathBuf::from("/ws/.dx/.commit.lock"),
            reason: "r".to_string(),
        },
        CommitError::CurrentInvalid {
            reason: "r".to_string(),
        },
        CommitError::RecordMismatch {
            reason: "r".to_string(),
        },
        CommitError::Install {
            reason: "r".to_string(),
        },
        CommitError::NoCapability,
    ];
    for error in &errors {
        assert!(!format!("{error}").is_empty());
    }
}
