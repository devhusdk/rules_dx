use super::*;
use clap::CommandFactory;

fn parse(words: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("advisory_prep").chain(words.iter().copied()))
        .expect("parses")
}

#[test]
fn every_flag_defaults_to_the_documented_value() {
    let options = options_from(parse(&[]), None).expect("defaults");
    assert_eq!(options.workspace, PathBuf::from("."));
    assert_eq!(options.out, PathBuf::from(DEFAULT_OUT));
    assert_eq!(options.limits, DEFAULT_LIMITS);
    assert!(options.archives.is_empty());
    assert_eq!(
        options.retrieved_at.len(),
        10,
        "today is stamped when no date is given"
    );
    assert!(is_audit_date(&options.retrieved_at));
}

#[test]
fn every_flag_overrides_its_default() {
    let options = options_from(
        parse(&[
            "--workspace",
            "/repo",
            "--out",
            "/snapshots",
            "--date",
            "2026-09-22",
            "--max-time",
            "90",
            "--connect-timeout",
            "7",
            "--retries",
            "5",
            "--retry-delay",
            "2",
            "--archive",
            "cargo=/mirrors/cargo.zip",
            "--archive",
            "npm=/mirrors/npm.zip",
        ]),
        None,
    )
    .expect("overrides");
    assert_eq!(options.workspace, PathBuf::from("/repo"));
    assert_eq!(options.out, PathBuf::from("/snapshots"));
    assert_eq!(options.retrieved_at, "2026-09-22");
    assert_eq!(
        options.limits,
        FetchLimits {
            connect_timeout_seconds: 7,
            max_seconds: 90,
            retries: 5,
            retry_delay_seconds: 2,
        }
    );
    assert_eq!(
        options.archives,
        BTreeMap::from([
            ("cargo".to_owned(), PathBuf::from("/mirrors/cargo.zip")),
            ("npm".to_owned(), PathBuf::from("/mirrors/npm.zip")),
        ])
    );
}

#[test]
fn the_defaults_anchor_at_the_workspace_root() {
    let root = tempfile::tempdir().expect("temp dir");
    let options = options_from(parse(&[]), Some(root.path())).expect("defaults");
    assert_eq!(options.workspace, root.path());
    assert_eq!(options.out, root.path().join(DEFAULT_OUT));
}

#[test]
fn relative_flags_anchor_at_the_workspace_root_and_absolute_ones_stay() {
    let root = tempfile::tempdir().expect("temp dir");
    let options = options_from(
        parse(&[
            "--workspace",
            "sub",
            "--out",
            "snaps",
            "--archive",
            "cargo=m/cargo.zip",
        ]),
        Some(root.path()),
    )
    .expect("relative");
    assert_eq!(options.workspace, root.path().join("sub"));
    assert_eq!(options.out, root.path().join("snaps"));
    assert_eq!(
        options.archives,
        BTreeMap::from([("cargo".to_owned(), root.path().join("m/cargo.zip"))])
    );
    let elsewhere = tempfile::tempdir().expect("temp dir");
    let absolute = |name: &str| {
        elsewhere
            .path()
            .join(name)
            .to_str()
            .expect("utf-8")
            .to_owned()
    };
    let options = options_from(
        parse(&[
            "--workspace",
            &absolute("ws"),
            "--out",
            &absolute("out"),
            "--archive",
            &format!("npm={}", absolute("npm.zip")),
        ]),
        Some(root.path()),
    )
    .expect("absolute");
    assert_eq!(options.workspace, elsewhere.path().join("ws"));
    assert_eq!(options.out, elsewhere.path().join("out"));
    assert_eq!(
        options.archives,
        BTreeMap::from([("npm".to_owned(), elsewhere.path().join("npm.zip"))])
    );
    assert_eq!(
        anchor(Some(root.path()), PathBuf::from(".")),
        root.path().to_path_buf()
    );
    assert_eq!(anchor(None, PathBuf::from("snaps")), PathBuf::from("snaps"));
}

#[test]
fn only_an_absolute_workspace_directory_is_a_root() {
    let root = tempfile::tempdir().expect("temp dir");
    assert_eq!(
        run_root(Some(root.path().as_os_str())),
        Some(root.path().to_path_buf())
    );
    assert_eq!(run_root(None), None);
    assert_eq!(run_root(Some(OsStr::new(""))), None);
    assert_eq!(run_root(Some(OsStr::new("relative/ws"))), None);
}

#[test]
fn an_unusable_date_is_a_usage_error() {
    assert_eq!(
        options_from(parse(&["--date", "2026-9-22"]), None),
        Err(PrepError::BadDate {
            retrieved_at: "2026-9-22".to_owned()
        })
    );
    assert!(options_from(parse(&["--date", "2026-13-01"]), None).is_err());
}

#[test]
fn an_unusable_archive_pairing_is_a_usage_error() {
    for value in ["cargo", "cargo=", "=/mirrors/cargo.zip", ""] {
        assert_eq!(
            archives_from(&[value.to_owned()]),
            Err(PrepError::Usage {
                detail: format!("--archive {value:?} wants FAMILY=PATH")
            }),
            "{value}"
        );
    }
}

#[test]
fn the_help_names_every_flag_and_the_exit_codes() {
    let help = Cli::command().render_long_help().to_string();
    for needle in [
        "--workspace",
        "--out",
        "--date",
        "--max-time",
        "--connect-timeout",
        "--retries",
        "--retry-delay",
        "--archive",
        "--ci",
        ".dx/local/advisory",
        "BUILD_WORKSPACE_DIRECTORY",
        "Exit codes",
    ] {
        assert!(help.contains(needle), "help omits {needle}:\n{help}");
    }
}

#[test]
fn an_unknown_flag_is_refused() {
    let error = Cli::try_parse_from(["advisory_prep", "--fetch"]).expect_err("refuses");
    assert_eq!(error.use_stderr(), true);
    assert!(error.to_string().contains("--fetch"), "{error}");
    let error = Cli::try_parse_from(["advisory_prep", "--retries", "many"]).expect_err("refuses");
    assert!(error.to_string().contains("many"), "{error}");
}

#[test]
fn the_status_file_receives_an_empty_status_for_a_dependency_free_workspace() {
    let dir = tempfile::tempdir().expect("temp dir");
    let status = dir.path().join("out.txt");
    std::fs::write(&status, "retrieved_at=2026-09-22\n").expect("seeds");
    let lines = run(parse(&[
        "--workspace",
        dir.path().to_str().expect("utf-8"),
        "--out",
        dir.path().join("snap").to_str().expect("utf-8"),
        "--date",
        "2026-09-22",
        "--status-file",
        status.to_str().expect("utf-8"),
    ]))
    .expect("empty preparation succeeds");
    assert_eq!(lines.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&status).expect("reads"),
        "retrieved_at=2026-09-22\nstatus=empty\necosystems=0\nfamilies=\n"
    );
    assert!(!dir.path().join("snap").exists());
}

#[test]
fn ci_with_pinned_snapshots_reports_empty_and_prepares_nothing() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".dx")).expect("dx dir");
    std::fs::write(
        root.path().join(".dx/pins"),
        "advisory cargo 2026-01-01 abc https://example.invalid/x\n",
    )
    .expect("pins");
    let status = root.path().join("out");
    let lines = run(parse(&[
        "--ci",
        "--workspace",
        root.path().to_str().expect("utf8"),
        "--status-file",
        status.to_str().expect("utf8"),
        "--date",
        "2026-01-02",
    ]))
    .expect("pinned run");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(
        std::fs::read_to_string(&status).expect("status"),
        "retrieved_at=2026-01-02\nstatus=empty\n"
    );
    assert!(!root.path().join(DEFAULT_OUT).exists());
}

#[test]
fn ci_without_locks_or_trivy_inputs_reports_empty() {
    let root = tempfile::tempdir().expect("temp dir");
    let status = root.path().join("out");
    run(parse(&[
        "--ci",
        "--workspace",
        root.path().to_str().expect("utf8"),
        "--out",
        root.path().join("snaps").to_str().expect("utf8"),
        "--status-file",
        status.to_str().expect("utf8"),
        "--date",
        "2026-01-02",
    ]))
    .expect("empty run");
    let text = std::fs::read_to_string(&status).expect("status");
    assert!(
        text.starts_with("retrieved_at=2026-01-02\nstatus=empty\n"),
        "{text}"
    );
    assert!(!text.contains("trivy=true"), "{text}");
}

#[test]
fn ci_with_trivy_inputs_sets_the_bundled_trivy_and_records_it() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".devcontainer")).expect("dir");
    std::fs::write(root.path().join(".devcontainer/Dockerfile"), "").expect("file");
    let status = root.path().join("out");
    let mut options = options_from(
        parse(&["--workspace", root.path().to_str().expect("utf8")]),
        None,
    )
    .expect("options");
    let trivy = root.path().join("bin/trivy");
    let lines = ci_setup(&mut options, Some(&status), Some(trivy.clone())).expect("setup");
    assert_eq!(lines, None);
    assert_eq!(options.trivy, Some(trivy));
    let text = std::fs::read_to_string(&status).expect("status");
    assert!(text.ends_with("trivy=true\n"), "{text}");
}

#[test]
fn ci_with_trivy_inputs_and_no_bundled_trivy_is_refused() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::write(root.path().join("main.tf"), "").expect("file");
    let mut options = options_from(
        parse(&["--workspace", root.path().to_str().expect("utf8")]),
        None,
    )
    .expect("options");
    let error = ci_setup(&mut options, None, None).expect_err("refused");
    assert!(error.to_string().contains("DX_TRIVY"), "{error}");
}
