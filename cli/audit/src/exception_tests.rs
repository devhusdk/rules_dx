use super::*;
use crate::profile_base::Profile;

fn sample() -> RiskException {
    RiskException {
        line: 0,
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
fn live_exceptions_keep_only_unexpired_entries_of_the_set() {
    let mut expired = sample();
    expired.expires = "2026-01-01".to_owned();
    let mut other_set = sample();
    other_set.set = "oci".to_owned();
    let policy = SecurityPolicy {
        exceptions: vec![sample(), expired, other_set],
        ..SecurityPolicy::default()
    };
    assert_eq!(
        live_exceptions_for_set(&policy, "cargo-lock", "2026-09-14"),
        vec![&sample()]
    );
    assert_eq!(
        live_exceptions_for_set(&policy, "oci", "2026-09-14").len(),
        1
    );
    assert!(live_exceptions_for_set(&policy, "npm", "2026-09-14").is_empty());
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
fn npm_unparseable_scopes_are_incomplete_and_branches_use_three_valued_or() {
    use crate::vuln::VersionScope::{Affected, Incomplete, Unaffected};
    let oversized = "1".repeat(4097);
    let many = vec!["9.9.9"; 65].join(" || ");
    for (scope, version) in [
        ("1.2.3", "banana"),
        ("not a range", "1.2.3"),
        ("1.x.3", "1.2.3"),
        ("9.9.9 || ", "1.0.0"),
        (oversized.as_str(), "1.0.0"),
        (many.as_str(), "1.0.0"),
        ("2.0.0 || banana", "1.0.0"),
        ("1.2.3", ""),
    ] {
        assert_eq!(npm_assess(scope, version), Incomplete, "{scope} {version}");
    }
    assert_eq!(npm_assess("1.0.0 || banana", "1.0.0"), Affected);
    assert_eq!(npm_assess("", "1.2.3"), Unaffected);
    assert_eq!(npm_assess("<1.0.0 || >=2.0.0", "1.2.3"), Unaffected);
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
fn npm_prerelease_comparators_only_allow_prereleases_on_the_same_tuple() {
    assert!(npm_in_scope(">=1.0.0-alpha <2.0.0", "1.0.0-alpha"));
    assert!(npm_in_scope(">=1.0.0-alpha <2.0.0", "1.0.0"));
    assert!(!npm_in_scope(">=1.0.0-alpha <2.0.0", "1.5.0-beta"));
    assert!(!npm_in_scope(">=1.0.0 <2.0.0", "1.5.0-beta"));
    assert!(!npm_in_scope(">1.2", "1.3.0-beta"));
}

#[test]
fn npm_partial_upper_bounds_are_inclusive_of_everything_below_them() {
    assert!(npm_in_scope(">1.2", "1.3.0"));
    assert!(!npm_in_scope(">1.2", "1.2.9"));
    assert!(npm_in_scope("<=1.2", "1.2.9"));
    assert!(!npm_in_scope("<=1.2", "1.3.0"));
}

#[test]
fn exception_schema_stays_versioned_without_allowlist() {
    let mut novel = sample();
    novel.advisory = "GHSA-novel-0000-0001".to_owned();
    novel.package = "brand-new-dep".to_owned();
    novel.versions = ">=9.0.0, <10.0.0".to_owned();
    validate_exception(&novel, "2026-09-14").expect("novel data validates");
}

fn load(text: &str) -> Result<SecurityPolicy, SecurityProblem> {
    load_security(Some(text), Profile::Standard)
}

fn security_toml(entries: &[RiskException]) -> String {
    let mut text = String::from("schema_version = 1\n");
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
    let policy = load(&text).expect("loads");
    assert_eq!(policy.exceptions.len(), 2);
    assert_eq!(
        policy.exceptions[0],
        RiskException {
            line: 2,
            ..sample()
        }
    );
    assert_eq!(policy.exceptions[1].advisory, "GHSA-dddd-eeee-ffff");
}

#[test]
fn security_toml_defaults_to_no_exceptions_and_needs_a_schema_version() {
    assert_eq!(
        load_security(None, Profile::Standard).expect("no file"),
        SecurityPolicy::default()
    );
    assert_eq!(
        load_security(Some("  \n"), Profile::Standard).expect("blank file"),
        SecurityPolicy::default()
    );
    assert_eq!(load("schema_version = 1\n"), Ok(SecurityPolicy::default()));
    for (text, want) in [
        ("fail_on = []\n", "schema_version is missing"),
        ("schema_version = 99\n", "schema_version 99 is newer"),
        ("schema_version = \"1\"\n", "must be a whole number"),
    ] {
        let error = load(text).expect_err(text).to_string();
        assert!(error.contains(want), "{text}: {error}");
    }
}

#[test]
fn security_toml_rejects_missing_and_unknown_keys() {
    let text = "schema_version = 1\n[[exception]]\npackage = \"p\"\n";
    assert!(matches!(
        load(text),
        Err(SecurityProblem::InvalidToml { .. })
    ));
    let text = format!("{}\nextra = 1\n", security_toml(&[sample()]));
    assert!(matches!(
        load(&text),
        Err(SecurityProblem::InvalidToml { .. })
    ));
}

#[test]
fn extends_is_an_unknown_key() {
    let error = load("schema_version = 1\nextends = \"base.toml\"\n")
        .expect_err("extends")
        .to_string();
    assert!(error.contains("unknown field `extends`"), "{error}");
    assert!(error.contains("line 2"), "{error}");
}

#[test]
fn security_toml_loads_uncovered_acceptances() {
    let text = "schema_version = 1\n[[uncovered]]\nset = \"powershell\"\nreason = \"r\"\nexpires = \"2027-01-01\"\n";
    let policy = load(text).expect("loads");
    assert_eq!(
        policy.uncovered,
        vec![UncoveredAcceptance {
            line: 2,
            set: "powershell".to_owned(),
            reason: "r".to_owned(),
            expires: "2027-01-01".to_owned(),
        }]
    );
    assert!(policy.exceptions.is_empty());
    let text = format!("{text}extra = 1\n");
    assert!(matches!(
        load(&text),
        Err(SecurityProblem::InvalidToml { .. })
    ));
}

#[test]
fn uncovered_acceptances_need_a_reason_and_a_live_expiry() {
    let mut acceptance = UncoveredAcceptance {
        line: 0,
        set: "powershell".to_owned(),
        reason: "No Gallery advisory source.".to_owned(),
        expires: "2027-01-01".to_owned(),
    };
    assert_eq!(validate_uncovered(&acceptance, "2026-10-24"), Ok(()));
    assert_eq!(
        validate_uncovered(&acceptance, "2027-01-01")
            .expect_err("expired")
            .to_string(),
        "uncovered acceptance for powershell expired 2027-01-01 (audit date 2027-01-01); renewal needs review"
    );
    acceptance.expires = "soon".to_owned();
    assert_eq!(
        validate_uncovered(&acceptance, "2026-10-24"),
        Err(ExceptionProblem::UncoveredInvalidDate {
            set: "powershell".to_owned(),
            value: "soon".to_owned(),
        })
    );
    acceptance.reason = " ".to_owned();
    assert_eq!(
        validate_uncovered(&acceptance, "2026-10-24"),
        Err(ExceptionProblem::MissingUncoveredField { field: "reason" })
    );
}

#[test]
fn line_of_counts_newlines_before_offset() {
    assert_eq!(line_of("a\nb\nc", 0), 1);
    assert_eq!(line_of("a\nb\nc", 2), 2);
    assert_eq!(line_of("a\nb\nc", 4), 3);
    assert_eq!(line_of("a", 99), 1);
}

#[test]
fn every_bad_entry_keeps_its_line_and_label() {
    let text = "\
schema_version = 1
[[exception]]
advisory = \"GHSA-1\"
package = \"alpha\"
set = \"cargo\"
versions = \"*\"
reason = \"ok\"
expires = \"2000-01-01\"

[[exception]]
advisory = \"GHSA-2\"
package = \"beta\"
set = \"cargo\"
versions = \"*\"
reason = \"ok\"
expires = \"soon\"

[[exception]]
advisory = \"GHSA-3\"
package = \"gamma\"
set = \"cargo\"
versions = \"*\"
reason = \"\"
expires = \"2999-01-01\"
";
    let policy = load(text).expect("loads");
    let found: Vec<(usize, String, bool)> = policy
        .exceptions
        .iter()
        .map(|entry| {
            (
                entry.line,
                entry.entry_label(),
                validate_exception(entry, "2026-01-01").is_err(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            (2, "GHSA-1 on alpha".to_owned(), true),
            (10, "GHSA-2 on beta".to_owned(), true),
            (18, "GHSA-3 on gamma".to_owned(), true),
        ]
    );
}

#[test]
fn syntax_error_reports_its_line() {
    let error = load("schema_version = 1\n[[exception\n").expect_err("bad toml");
    assert!(
        error.to_string().contains("invalid TOML at line 2"),
        "{error}"
    );
}

#[test]
fn empty_fields_show_as_question_mark() {
    let mut entry = sample();
    entry.advisory.clear();
    assert_eq!(entry.entry_label(), format!("? on {}", entry.package));
}

#[test]
fn the_profile_picks_the_base_and_a_root_list_replaces_it() {
    let standard = load_security(None, Profile::Standard).expect("standard");
    assert_eq!(
        standard.fail_on,
        vec![AdvisoryKind::Vulnerability, AdvisoryKind::Unsound]
    );
    assert_eq!(standard.fail_on, SecurityPolicy::default().fail_on);
    let strict = load_security(None, Profile::Strict).expect("strict");
    assert!(strict.fails_on(AdvisoryKind::Unmaintained));
    assert!(strict.fails_on(AdvisoryKind::Notice));
    assert!(strict.fails_on(AdvisoryKind::Yanked));
    let with_exception = security_toml(&[sample()]);
    let strict = load_security(Some(&with_exception), Profile::Strict).expect("strict");
    assert!(strict.fails_on(AdvisoryKind::Unmaintained));
    assert_eq!(strict.exceptions.len(), 1);
    let own = "schema_version = 1\nfail_on = [\"unsound\"]\n";
    let own = load_security(Some(own), Profile::Strict).expect("own list");
    assert_eq!(own.fail_on, vec![AdvisoryKind::Unsound]);
}

#[test]
fn unknown_kinds_are_rejected() {
    let withdrawn = load("schema_version = 1\nfail_on = [\"withdrawn\"]\n");
    assert!(matches!(
        withdrawn,
        Err(SecurityProblem::InvalidToml { .. })
    ));
}
