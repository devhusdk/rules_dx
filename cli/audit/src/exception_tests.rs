use super::*;

fn sample() -> RiskException {
    RiskException {
        advisory: "GHSA-aaaa-bbbb-cccc".to_owned(),
        package: "some-copyleft-lib".to_owned(),
        set: "cargo-lock".to_owned(),
        versions: ">=1.2.0, <2.0.0".to_owned(),
        reason: "Legal approved for internal fork.".to_owned(),
        expires: "2027-03-01".to_owned(),
    }
}

fn finding() -> FindingRef {
    FindingRef {
        advisory: "GHSA-aaaa-bbbb-cccc".to_owned(),
        package: "some-copyleft-lib".to_owned(),
    }
}

#[test]
fn valid_exception_passes_and_applies() {
    let exception = sample();
    validate_exception(&exception, "2026-09-14").expect("valid");
    check_applies(&exception, &[finding()]).expect("applies");
}

#[test]
fn empty_reason_fails_validation() {
    let mut exception = sample();
    exception.reason = "  ".to_owned();
    assert_eq!(
        validate_exception(&exception, "2026-09-14"),
        Err(ExceptionProblem::MissingField { field: "reason" })
    );
}

#[test]
fn malformed_dates_fail_validation() {
    let mut exception = sample();
    for bad in [
        "2027-3-1",
        "2027/03/01",
        "2027-13-01",
        "2027-02-30",
        "2027-04-31",
        "2027-00-10",
        "2027-01-00",
        "0000-01-01",
        "not-a-date",
    ] {
        exception.expires = bad.to_owned();
        assert!(
            matches!(
                validate_exception(&exception, "2026-09-14"),
                Err(ExceptionProblem::InvalidDate { .. })
            ),
            "{bad} must fail"
        );
    }
    assert!(validate_exception(&sample(), "today").is_err());
}

#[test]
fn expiry_boundary_fails_on_the_date_itself() {
    let exception = sample();
    assert_eq!(
        validate_exception(&exception, "2027-03-01"),
        Err(ExceptionProblem::Expired {
            expires: "2027-03-01".to_owned(),
            today: "2027-03-01".to_owned(),
        })
    );
    assert!(validate_exception(&exception, "2027-03-02").is_err());
    validate_exception(&exception, "2027-02-28").expect("day before passes");
}

#[test]
fn leap_year_february_validates() {
    let mut exception = sample();
    exception.expires = "2028-02-29".to_owned();
    validate_exception(&exception, "2026-09-14").expect("leap day valid");
    exception.expires = "2027-02-29".to_owned();
    assert!(validate_exception(&exception, "2026-09-14").is_err());
}

#[test]
fn exception_without_finding_is_obsolete() {
    let exception = sample();
    assert!(is_obsolete(&exception, &[]));
    assert_eq!(
        check_applies(&exception, &[]),
        Err(ExceptionProblem::Obsolete {
            advisory: "GHSA-aaaa-bbbb-cccc".to_owned(),
            package: "some-copyleft-lib".to_owned(),
        })
    );
    let other = FindingRef {
        advisory: "GHSA-xxxx-yyyy-zzzz".to_owned(),
        package: "some-copyleft-lib".to_owned(),
    };
    assert!(is_obsolete(&exception, &[other]));
    assert!(!is_obsolete(&exception, &[finding()]));
}

#[test]
fn version_scopes_match_cargo_flavor_ranges() {
    let scope = ">=1.2.0, <2.0.0";
    assert!(version_in_scope(scope, "1.2.0"));
    assert!(version_in_scope(scope, "1.9.0"));
    assert!(!version_in_scope(scope, "1.1.9"));
    assert!(!version_in_scope(scope, "2.0.0"));
    assert!(version_in_scope("^1.2.0", "1.9.0"));
    assert!(!version_in_scope("^1.2.0", "2.0.0"));
    assert!(version_in_scope("~1.2.0", "1.2.9"));
    assert!(!version_in_scope("~1.2.0", "1.3.0"));
    assert!(version_in_scope("1.2.0", "1.2.0"));
    assert!(version_in_scope("1.2.0", "1.2.1"));
    assert!(version_in_scope("=1.2.0", "1.2.0"));
    assert!(!version_in_scope("=1.2.0", "1.2.1"));
    assert!(version_in_scope("*", "9.9.9"));
}

#[test]
fn version_scopes_fail_closed_on_unparseable_input() {
    assert!(!version_in_scope("not a range", "1.2.0"));
    assert!(!version_in_scope(">=1.2.0, <2.0.0", "1.2"));
    assert!(!version_in_scope(">=1.2.0, <2.0.0", "banana"));
    assert!(!version_in_scope("", "1.2.0"));
}

#[test]
fn version_scopes_exclude_prereleases_from_bare_ranges() {
    assert!(!version_in_scope(">=1.0.0", "2.0.0-alpha"));
    assert!(version_in_scope(">=1.0.0-alpha, <2.0.0", "1.0.0-alpha"));
}

#[test]
fn cargo_edges_pin_star_and_fail_closed_or_hyphen() {
    assert!(version_in_scope("*", "1.2.3"));
    assert!(!version_in_scope("1.0.0 || 2.0.0", "1.0.0"));
    assert!(!version_in_scope("1.2.3 - 2.3.4", "1.5.0"));
    assert!(!version_in_scope(">=1.0.0 || <0.5.0", "0.4.0"));
}

#[test]
fn npm_ranges_cover_star_or_hyphen_and_prerelease_edges() {
    assert!(npm_in_scope("*", "9.9.9"));
    assert!(npm_in_scope("x", "1.2.3"));
    assert!(npm_in_scope("1.2.x", "1.2.9"));
    assert!(!npm_in_scope("1.2.x", "1.3.0"));
    assert!(npm_in_scope("1.2", "1.2.5"));
    assert!(!npm_in_scope("1.2", "1.3.0"));
    assert!(npm_in_scope("1.2.7 || >=1.2.9 <2.0.0", "1.2.7"));
    assert!(npm_in_scope("1.2.7 || >=1.2.9 <2.0.0", "1.2.9"));
    assert!(npm_in_scope("1.2.7 || >=1.2.9 <2.0.0", "1.5.0"));
    assert!(!npm_in_scope("1.2.7 || >=1.2.9 <2.0.0", "1.2.8"));
    assert!(!npm_in_scope("1.2.7 || >=1.2.9 <2.0.0", "2.0.0"));
    assert!(npm_in_scope("1.2.3 - 2.3.4", "1.2.3"));
    assert!(npm_in_scope("1.2.3 - 2.3.4", "2.0.0"));
    assert!(npm_in_scope("1.2.3 - 2.3.4", "2.3.4"));
    assert!(!npm_in_scope("1.2.3 - 2.3.4", "1.2.2"));
    assert!(!npm_in_scope("1.2.3 - 2.3.4", "2.3.5"));
    assert!(npm_in_scope("1.2 - 2.3", "2.3.9"));
    assert!(!npm_in_scope("1.2 - 2.3", "2.4.0"));
    assert!(npm_in_scope(">=1.2.7 <1.3.0", "1.2.9"));
    assert!(!npm_in_scope(">=1.2.7 <1.3.0", "1.3.0"));
    assert!(npm_in_scope(">=1.2.7, <1.3.0", "1.2.9"));
    assert!(npm_in_scope("^1.2.3", "1.9.0"));
    assert!(!npm_in_scope("^1.2.3", "2.0.0"));
    assert!(npm_in_scope("~1.2.3", "1.2.9"));
    assert!(!npm_in_scope("~1.2.3", "1.3.0"));
    assert!(npm_in_scope("1.2.3", "1.2.3"));
    assert!(!npm_in_scope("1.2.3", "1.2.4"));
    assert!(npm_in_scope("v1.2.3", "1.2.3"));
    assert!(!npm_in_scope(">=1.0.0", "2.0.0-alpha"));
    assert!(npm_in_scope(">=1.0.0-alpha, <2.0.0", "1.0.0-alpha"));
    assert!(!npm_in_scope("", "1.2.3"));
    assert!(!npm_in_scope("not a range", "1.2.3"));
    assert!(!npm_in_scope("1.x.3", "1.2.3"));
    assert!(!npm_in_scope("01.2.3", "1.2.3"));
    assert!(!npm_in_scope(">=", "1.2.3"));
    assert!(!npm_in_scope("1.2.3", ""));
    assert!(!npm_in_scope("1.2.3", "banana"));
}

#[test]
fn npm_scope_gates_overlong_and_absurd_inputs() {
    assert!(!npm_in_scope(&"1".repeat(5000), "1.0.0"));
    assert!(!npm_in_scope("1.0.0", &"1".repeat(300)));
    assert!(!npm_in_scope(&vec!["1.0.0"; 65].join("||"), "1.0.0"));
    assert!(!npm_in_scope(&"1".repeat(300), "1.0.0"));
    assert!(!npm_in_scope(
        &format!("{} - 2.0.0", "1".repeat(300)),
        "1.5.0"
    ));
}

#[test]
fn npm_locked_markers_branch_shapes_and_merged_operators() {
    assert!(npm_in_scope("1.2.3", "v1.2.3"));
    assert!(npm_in_scope("1.2.3", "=1.2.3"));
    assert!(!npm_in_scope("1.2.3", "v"));
    assert!(!npm_in_scope("9.9.9 || ", "1.0.0"));
    assert!(!npm_in_scope(",", "1.0.0"));
    assert!(npm_in_scope(">= 1.2.3", "1.5.0"));
    assert!(!npm_in_scope("1 *", "1.5.0"));
    assert!(npm_in_scope("1.2.3+b", "1.2.3+b"));
    assert!(!npm_in_scope("+", "1.0.0"));
}

#[test]
fn npm_malformed_partials_fail_closed() {
    assert!(!npm_in_scope("1.2.3.4", "1.2.3"));
    assert!(!npm_in_scope("1..2", "1.2.0"));
    assert!(!npm_in_scope("1.x-rc", "1.2.0"));
    assert!(!npm_in_scope("1.2.3-rc_1", "1.2.3"));
}

#[test]
fn npm_hyphen_ranges_cover_wildcard_and_partial_ends() {
    assert!(npm_in_scope("* - 2.0.0", "1.5.0"));
    assert!(npm_in_scope("1.0.0 - *", "1.5.0"));
    assert!(!npm_in_scope("* - *", "1.5.0"));
    assert!(!npm_in_scope("banana - 2.0.0", "1.5.0"));
    assert!(!npm_in_scope("1.0.0 - banana", "1.5.0"));
    assert!(npm_in_scope("1.0.0 - 2.3.4-beta", "2.3.4-beta"));
    assert!(npm_in_scope("1.2.3-rc.1 - 2.0.0", "1.2.3-rc.1"));
    assert!(npm_in_scope("1.0.0 - 1", "1.0.5"));
}

#[test]
fn npm_comparator_operators_expand_upstream() {
    assert!(npm_in_scope("<=2.0.0", "1.5.0"));
    assert!(npm_in_scope(">1.2.3", "1.5.0"));
    assert!(npm_in_scope("=1.2.3", "1.2.3"));
    assert!(npm_in_scope("1.2.3-rc.1", "1.2.3-rc.1"));
    assert!(npm_in_scope(">=1.2", "1.2.5"));
    assert!(npm_in_scope("<=1.2", "1.2.5"));
    assert!(npm_in_scope("<=1", "1.5.0"));
    assert!(npm_in_scope("<1.2", "1.1.0"));
    assert!(npm_in_scope(">1.2", "1.3.0"));
    assert!(npm_in_scope(">1", "2.0.0"));
    assert!(!npm_in_scope(">=^1.0", "1.0.0"));
    assert!(!npm_in_scope("=v", "1.0.0"));
    assert!(!npm_in_scope(">*", "1.0.0"));
    assert!(npm_in_scope("9", "9.0.0"));
}

#[test]
fn npm_caret_and_tilde_expand_each_upstream_branch() {
    assert!(npm_in_scope("^1.2.3-rc.1", "1.2.3-rc.1"));
    assert!(npm_in_scope("^0.2.3", "0.2.5"));
    assert!(npm_in_scope("^0.0.3", "0.0.3"));
    assert!(npm_in_scope("^0.0", "0.0.5"));
    assert!(npm_in_scope("^0", "0.9.0"));
    assert!(npm_in_scope("~1.2.3-rc.1", "1.2.3-rc.1"));
    assert!(npm_in_scope("~1", "1.9.0"));
}

#[test]
fn exception_schema_stays_versioned_without_allowlist() {
    assert_eq!(EXCEPTION_SCHEMA_VERSION, 1);
    let mut novel = sample();
    novel.advisory = "GHSA-novel-0000-0001".to_owned();
    novel.package = "brand-new-dep".to_owned();
    novel.versions = ">=9.0.0, <10.0.0".to_owned();
    validate_exception(&novel, "2026-09-14").expect("novel data validates");
}

fn security_toml(entries: &[RiskException]) -> String {
    let mut text = String::new();
    for entry in entries {
        text.push_str("[[exception]]\n");
        for (key, value) in [
            ("advisory", &entry.advisory),
            ("package", &entry.package),
            ("set", &entry.set),
            ("versions", &entry.versions),
            ("reason", &entry.reason),
            ("expires", &entry.expires),
        ] {
            text.push_str(&format!("{key} = {value:?}\n"));
        }
    }
    text
}

#[test]
fn security_toml_loads_every_exception_in_file_order() {
    let mut second = sample();
    second.advisory = "GHSA-dddd-eeee-ffff".to_owned();
    second.package = "another-copyleft-lib".to_owned();
    let text = security_toml(&[sample(), second]);
    let policy = load_security_toml(&text).expect("loads");
    assert_eq!(policy.exceptions.len(), 2);
    assert_eq!(policy.exceptions[0], sample());
    assert_eq!(policy.exceptions[1].advisory, "GHSA-dddd-eeee-ffff");
}

#[test]
fn security_toml_defaults_to_no_exceptions_and_the_current_schema() {
    assert_eq!(
        load_security_toml("").expect("empty file"),
        SecurityPolicy::default()
    );
    assert_eq!(
        load_security_toml(&format!("schema_version = {EXCEPTION_SCHEMA_VERSION}\n"))
            .expect("pinned"),
        SecurityPolicy::default()
    );
    assert_eq!(
        load_security_toml("schema_version = 99\n"),
        Err(SecurityProblem::UnsupportedSchema { version: 99 })
    );
}

#[test]
fn security_toml_rejects_missing_and_unknown_keys() {
    let text = "[[exception]]\npackage = \"p\"\n";
    assert!(matches!(
        load_security_toml(text),
        Err(SecurityProblem::InvalidToml { .. })
    ));
    let text = format!("{}\nextra = 1\n", security_toml(&[sample()]));
    assert!(matches!(
        load_security_toml(&text),
        Err(SecurityProblem::InvalidToml { .. })
    ));
}
