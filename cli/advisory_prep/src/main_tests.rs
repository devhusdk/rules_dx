use super::*;
use clap::CommandFactory;

fn parse(words: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("advisory_prep").chain(words.iter().copied()))
        .expect("parses")
}

#[test]
fn every_flag_defaults_to_the_documented_value() {
    let options = options_from(parse(&[])).expect("defaults");
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
    let options = options_from(parse(&[
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
    ]))
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
fn an_unusable_date_is_a_usage_error() {
    assert_eq!(
        options_from(parse(&["--date", "2026-9-22"])),
        Err(PrepError::BadDate {
            retrieved_at: "2026-9-22".to_owned()
        })
    );
    assert!(options_from(parse(&["--date", "2026-13-01"])).is_err());
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
        ".dx/advisory",
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
