use crate::conventions::*;

fn value(text: &str) -> toml::Value {
    text.parse().unwrap()
}

#[test]
fn schema_version_is_required_and_checked() {
    assert_eq!(
        check_schema_version(None),
        Err(ConventionProblem::MissingSchemaVersion)
    );
    assert_eq!(check_schema_version(Some(&value("1"))), Ok(()));
    assert!(matches!(
        check_schema_version(Some(&value("\"1\""))),
        Err(ConventionProblem::BadSchemaVersion { .. })
    ));
    for (text, found) in [("2", 2), ("0", 0), ("-1", -1)] {
        let problem = check_schema_version(Some(&value(text))).unwrap_err();
        assert_eq!(
            problem,
            ConventionProblem::UnsupportedSchemaVersion { found }
        );
        assert_eq!(
            problem.to_string(),
            format!("schema_version {found} is not accepted; the only accepted value is 1")
        );
    }
}

#[test]
fn expiry_needs_a_valid_future_date() {
    assert_eq!(check_expiry("2030-01-01", "2029-12-31"), Ok(()));
    assert!(matches!(
        check_expiry("2029-12-31", "2029-12-31"),
        Err(ConventionProblem::Expired { .. })
    ));
    for bad in [
        "",
        "2030-1-1",
        "2030-13-01",
        "2030-02-30",
        "20300101",
        "0000-01-01",
    ] {
        assert!(
            matches!(
                check_expiry(bad, "2029-01-01"),
                Err(ConventionProblem::InvalidDate { .. })
            ),
            "{bad}"
        );
    }
}

#[test]
fn exception_needs_a_reason_and_a_future_date() {
    assert_eq!(
        check_exception(" ", "2030-01-01", "2029-01-01"),
        Err(ConventionProblem::EmptyField { field: "reason" })
    );
    assert!(check_exception("vendor fix pending", "2020-01-01", "2029-01-01").is_err());
    assert_eq!(
        check_exception("vendor fix pending", "2030-01-01", "2029-01-01"),
        Ok(())
    );
}

#[test]
fn label_patterns_cover_scopes() {
    let covers = pattern_covers;
    for root in [
        "//...",
        "//cli/...",
        "//cli/cli:dx_cli",
        "//:root",
        "@//cli:x",
        "@@//cli/...",
    ] {
        assert!(covers("//...", root), "{root}");
    }
    assert!(!covers("//...", "@other//a:b"));
    for root in ["//cli:x", "//cli/...", "//cli/cli/...", "//cli/cli:dx"] {
        assert!(covers("//cli/...", root), "{root}");
    }
    for root in ["//client:x", "//...", "//other/..."] {
        assert!(!covers("//cli/...", root), "{root}");
    }
    assert!(covers("//cli:all", "//cli:x"));
    assert!(!covers("//cli:all", "//cli/..."));
    assert!(covers("//cli:dx", "//cli:dx"));
    assert!(!covers("//cli:dx", "//cli:other"));
    assert!(covers("@repo//...", "@repo//a:b"));
    assert!(!covers("@repo//...", "//a:b"));
    assert!(!covers("//...", "cargo"));
}

#[test]
fn bad_patterns_are_rejected() {
    for bad in ["cli", "//cli/", "//a/.../b", "//a:", ""] {
        assert!(!is_valid_pattern(bad), "{bad}");
    }
    for good in ["//...", "//a:b", "//a", "@r//a/...", "//a:all"] {
        assert!(is_valid_pattern(good), "{good}");
    }
}

#[test]
fn recursive_dir_reads_every_recursive_spelling() {
    for pattern in [
        "//vendor/...",
        "//vendor/...:all",
        "//vendor/...:*",
        "//vendor/...:all-targets",
    ] {
        assert_eq!(recursive_dir(pattern), Some("vendor"), "{pattern}");
    }
    assert_eq!(recursive_dir("//a/b/..."), Some("a/b"));
    for pattern in [
        "//...",
        "//...:all",
        "//pkg:all",
        "//pkg",
        "@ext//vendor/...",
        "//vendor/...:x",
    ] {
        assert_eq!(recursive_dir(pattern), None, "{pattern}");
    }
}
