use super::*;
use crate::exception::{version_in_scope, ExceptionProblem, RiskException};

#[test]
fn osv_range_boundaries_project_to_native_ecosystem_scopes() {
    use osv::schema::Event::{Fixed, Introduced, LastAffected, Limit};
    for (events, lower, upper, inclusive) in [
        (vec![Introduced("0".to_owned())], None, None, false),
        (
            vec![Introduced("1.0.0".to_owned())],
            Some("1.0.0"),
            None,
            false,
        ),
        (vec![Fixed("2.0.0".to_owned())], None, Some("2.0.0"), false),
        (
            vec![LastAffected("2.0.0".to_owned())],
            None,
            Some("2.0.0"),
            true,
        ),
        (vec![Limit("2.0.0".to_owned())], None, Some("2.0.0"), false),
        (
            vec![Introduced("1.0.0".to_owned()), Limit("2.0.0".to_owned())],
            Some("1.0.0"),
            Some("2.0.0"),
            false,
        ),
        (
            vec![
                Introduced("1.0.0".to_owned()),
                LastAffected("2.0.0".to_owned()),
            ],
            Some("1.0.0"),
            Some("2.0.0"),
            true,
        ),
    ] {
        let intervals = range_events_to_intervals(&events);
        assert_eq!(
            intervals,
            vec![(
                lower.map(str::to_owned),
                upper.map(str::to_owned),
                inclusive
            )]
        );
        for set in ["cargo", "npm", "go", "maven", "nuget"] {
            let scope =
                interval_to_scope(set, &intervals[0].0, &intervals[0].1, inclusive).expect("scope");
            assert!(version_affected(set, &scope, "1.5.0"), "{set} {scope}");
            if upper.is_some() {
                assert_eq!(
                    version_affected(set, &scope, "2.0.0"),
                    inclusive,
                    "{set} {scope}"
                );
                assert!(!version_affected(set, &scope, "2.0.1"), "{set} {scope}");
            }
            if lower.is_some() {
                assert!(!version_affected(set, &scope, "0.9.0"), "{set} {scope}");
            }
        }
    }
    assert_eq!(
        range_events_to_intervals(&[
            Introduced("1.0.0".to_owned()),
            Introduced("2.0.0".to_owned())
        ]),
        vec![
            (Some("1.0.0".to_owned()), None, false),
            (Some("2.0.0".to_owned()), None, false)
        ]
    );
    assert!(interval_to_scope("unknown", &None, &None, false).is_none());
}

#[test]
fn osv_projection_skips_incomplete_packages_and_uses_severity_precedence() {
    let base = serde_json::json!({
        "id": "GHSA-demo", "modified": "2026-01-01T00:00:00Z",
        "affected": [{"package": {"ecosystem": "npm", "name": "demo"}, "versions": ["", "1.0.0", "1.0.0"]}]
    });
    for source in ["affected", "top", "ecosystem"] {
        let mut value = base.clone();
        match source {
            "affected" => {
                value["affected"][0]["severity"] =
                    serde_json::json!([{"type":"CVSS_V3","score":"high"}])
            }
            "top" => value["severity"] = serde_json::json!([{"type":"CVSS_V3","score":"medium"}]),
            "ecosystem" => {
                value["affected"][0]["ecosystem_specific"] = serde_json::json!({"severity":"low"})
            }
            _ => unreachable!(),
        }
        let vuln: osv::schema::Vulnerability = serde_json::from_value(value).expect("OSV");
        let projected = project_osv_snapshot(&[vuln]);
        assert_eq!(projected.len(), 1);
        assert_eq!(projected[0].versions, "1.0.0");
        assert_eq!(
            projected[0].severity,
            match source {
                "affected" => "high",
                "top" => "medium",
                _ => "low",
            }
        );
    }
    for variant in ["empty-id", "missing-package", "empty-name", "no-affected"] {
        let mut value = base.clone();
        match variant {
            "empty-id" => value["id"] = serde_json::json!(" "),
            "missing-package" => {
                value["affected"][0]
                    .as_object_mut()
                    .expect("affected")
                    .remove("package");
            }
            "empty-name" => value["affected"][0]["package"]["name"] = serde_json::json!(" "),
            "no-affected" => {
                value.as_object_mut().expect("vuln").remove("affected");
            }
            _ => unreachable!(),
        }
        let vuln: osv::schema::Vulnerability = serde_json::from_value(value).expect("OSV");
        assert!(project_osv_snapshot(&[vuln]).is_empty(), "{variant}");
    }
}

#[test]
fn go_comparators_respect_partial_versions_and_zero_major_caret() {
    for (scope, version, expected) in [
        (">1.2.3", "1.2.4", true),
        (">1.2.3", "1.2.3", false),
        (">1.2", "1.3.0", true),
        (">1.2", "1.2.9", false),
        (">1", "2.0.0", true),
        (">1", "1.9.9", false),
        ("<1.2.3", "1.2.2", true),
        ("<1.2.3", "1.2.3", false),
        ("<1.2", "1.1.9", true),
        ("<1.2", "1.2.0", false),
        ("<1", "0.9.9", true),
        ("<1", "1.0.0", false),
        ("<=1.2.3", "1.2.3", true),
        ("<=1.2.3", "1.2.4", false),
        ("~1.2.3", "1.2.4", true),
        ("~1.2.3", "1.3.0", false),
        ("~1.2", "1.2.9", true),
        ("~1.2", "1.3.0", false),
        ("^1", "1.9.9", true),
        ("^1", "2.0.0", false),
        ("^1.2", "1.3.0", true),
        ("^0.2", "0.3.0", false),
        ("^0.2.3", "0.2.4", true),
        ("^0.2.3", "0.3.0", false),
        ("^0.0.3", "0.0.4", false),
        ("^0.0.3", "0.0.3", true),
    ] {
        assert_eq!(
            version_affected("go", scope, version),
            expected,
            "{scope} {version}"
        );
    }
    assert!(version_affected("unknown", " 1.0.0 ", "1.0.0"));
    assert!(!version_affected("unknown", "", ""));
}

#[test]
fn locked_package_new_defaults_to_registry_package() {
    assert_eq!(
        LockedPackage::new("npm", "react", "18.2.0"),
        LockedPackage {
            name: "react".to_owned(),
            version: "18.2.0".to_owned(),
            set: "npm".to_owned(),
            is_git: false,
            is_private: false,
            location: None,
            platform: None,
        }
    );
}

fn advisory(set: &str, id: &str, package: &str, versions: &str) -> Advisory {
    Advisory {
        id: id.to_owned(),
        package: package.to_owned(),
        versions: versions.to_owned(),
        severity: "medium".to_owned(),
        fixed: vec![],
        set: set.to_owned(),
    }
}

fn exception(set: &str, advisory: &str, package: &str, versions: &str) -> RiskException {
    RiskException {
        advisory: advisory.to_owned(),
        package: package.to_owned(),
        set: set.to_owned(),
        versions: versions.to_owned(),
        reason: "Accepted for this release.".to_owned(),
        expires: "2027-03-01".to_owned(),
    }
}

fn packages() -> Vec<LockedPackage> {
    vec![
        LockedPackage::new("cargo", "serde", "1.0.100"),
        LockedPackage::new("npm", "react", "18.2.0"),
    ]
}

fn advisories() -> Vec<Advisory> {
    vec![
        Advisory {
            severity: "high".to_owned(),
            fixed: vec!["1.0.150".to_owned()],
            ..advisory("cargo", "GHSA-aaaa-bbbb-cccc", "serde", ">=1.0.0, <1.0.150")
        },
        Advisory {
            severity: String::new(),
            ..advisory("npm", "GHSA-nofix-0000", "react", ">=18.0.0, <18.3.0")
        },
    ]
}

#[test]
fn matching_reports_findings_with_and_without_fix() {
    let (findings, unassessed) = match_packages(&packages(), &advisories());
    assert!(unassessed.is_empty());
    assert_eq!(findings.len(), 2);
    let serde_finding = findings
        .iter()
        .find(|finding| finding.package == "serde")
        .expect("serde");
    assert_eq!(serde_finding.advisory, "GHSA-aaaa-bbbb-cccc");
    assert_eq!(serde_finding.level, "error");
    assert_eq!(serde_finding.severity, "high");
    assert_eq!(serde_finding.fixed, vec!["1.0.150".to_owned()]);
    let react_finding = findings
        .iter()
        .find(|finding| finding.package == "react")
        .expect("react");
    assert_eq!(react_finding.severity, "unknown");
    assert_eq!(react_finding.level, "error");
    assert!(react_finding.fixed.is_empty());
}

#[test]
fn severity_normalizes_to_closed_levels_with_unknown_failing() {
    assert_eq!(normalize_level("critical"), "error");
    assert_eq!(normalize_level("HIGH"), "error");
    assert_eq!(normalize_level("medium"), "warning");
    assert_eq!(normalize_level("low"), "warning");
    assert_eq!(normalize_level(""), "error");
    assert_eq!(normalize_level("unknown"), "error");
    assert_eq!(normalize_level("bogus"), "error");
    assert_eq!(canonical_severity(""), "unknown");
    assert_eq!(canonical_severity("  "), "unknown");
    assert_eq!(canonical_severity(" high "), "high");
    assert_eq!(UNKNOWN_SEVERITY, "unknown");
    assert_eq!(VULN_RULE_PREFIX, "vuln");
}

#[test]
fn version_matching_uses_semver_for_cargo_npm_go_and_ranges_for_maven_nuget() {
    assert!(version_affected("cargo", ">=1.0.0, <2.0.0", "1.5.0"));
    assert!(!version_affected("cargo", ">=1.0.0, <2.0.0", "2.0.0"));
    assert!(version_affected("npm", "^18.0.0", "18.2.0"));
    assert!(version_affected("go", ">=1.0.0, <2.0.0", "1.5.0"));
    assert!(version_affected("maven", "1.2.0", "1.2.0"));
    assert!(version_affected("maven", "1.0", "1.0.0"));
    assert!(version_affected("maven", "[1.0,2.0)", "1.5.0"));
    assert!(!version_affected("maven", "[1.0,2.0)", "2.0.0"));
    assert!(version_affected("maven", "(,1.0]", "1.0.0"));
    assert!(!version_affected("maven", "(,1.0]", "1.0.1"));
    assert!(version_affected("maven", "[1.0]", "1.0.0"));
    assert!(!version_affected("maven", ">=1.0.0", "1.2.0"));
    assert!(version_affected("nuget", "1.2.3", "1.2.3"));
    assert!(version_affected("nuget", "1.0", "1.0.0"));
    assert!(!version_affected("nuget", "1.0", "1.5.0"));
    assert!(version_affected("nuget", "[1.0,2.0)", "1.5.0"));
    assert!(!version_affected("nuget", "[1.0,2.0)", "2.0.0"));
    assert!(version_affected("nuget", "(,1.0]", "1.0.0"));
    assert!(!version_affected("nuget", "(,1.0]", "1.0.1"));
    assert!(version_affected("nuget", "[1.0]", "1.0.0"));
    assert!(!version_affected("nuget", ">=1.0.0", "1.2.0"));
    assert!(!version_affected("nuget", "", "1.0.0"));
}

#[test]
fn go_scopes_normalize_v_prefix() {
    assert!(go_in_scope(">=1.0.0, <2.0.0", "v1.5.0"));
    assert!(go_in_scope(">=v1.0.0, <v2.0.0", "1.5.0"));
    assert!(go_in_scope(">=v1.0.0, <v2.0.0", "v1.5.0"));
    assert!(!go_in_scope(">=v1.0.0, <v2.0.0", "v2.0.0"));
    assert!(version_affected("go", ">=v1.0.0, <v2.0.0", "v1.5.0"));
    assert!(!version_affected("go", ">=v1.0.0, <v2.0.0", "v2.0.0"));
    assert!(go_in_scope("^v1.2.0", "v1.9.0"));
    assert!(!go_in_scope("^v1.2.0", "v2.0.0"));
    assert!(go_in_scope("=v1.2.0", "v1.2.0"));
    assert!(!go_in_scope("=v1.2.0", "v1.2.1"));
    assert!(go_in_scope(
        ">=v0.0.0-20250930140053-2eb4fccefb52, <v99.0.0",
        "v0.0.0-20250930140053-2eb4fccefb52"
    ));
    assert!(go_in_scope(">=v1.0.0, <v3.0.0", "v2.0.0+incompatible"));
    assert!(!version_in_scope(
        ">=1.0.0, <2.0.0",
        "1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(go_in_scope(
        ">=v1.0.0, <v2.0.0",
        "v1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(version_affected(
        "go",
        ">=v1.0.0, <v2.0.0",
        "v1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(go_in_scope(
        ">=v1.0.0, <v1.2.4",
        "v1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(!go_in_scope(
        ">=v1.2.4, <v2.0.0",
        "v1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(go_in_scope("*", "v1.2.4-0.20240101120000-abcdef123456"));
    assert!(go_in_scope(
        "^v1.2.0",
        "v1.9.0-0.20240101120000-abcdef123456"
    ));
    assert!(go_in_scope(
        "~v1.2.3",
        "v1.2.9-0.20240101120000-abcdef123456"
    ));
    assert!(!go_in_scope(
        "~v1.2.3",
        "v1.3.0-0.20240101120000-abcdef123456"
    ));
    assert!(!go_in_scope(
        "=v1.2.4",
        "v1.2.4-0.20240101120000-abcdef123456"
    ));
    assert!(go_in_scope("=v2.0.0", "v2.0.0+incompatible"));
    assert!(go_in_scope(
        ">=v1.0.0, <v3.0.0",
        "v2.0.0-0.20240101120000-abcdef123456+incompatible"
    ));
    assert_eq!(strip_go_v("very"), "very");
    assert_eq!(strip_go_v(">=v1.0.0, <v2.0.0"), ">=1.0.0, <2.0.0");
    assert!(!go_in_scope("not a range", "v1.2.0"));
    assert!(!go_in_scope(">=v1.0.0", "banana"));
    assert!(!go_in_scope("", "v1.0.0"));
    assert!(!go_in_scope(">=v1.0.0", ""));
}

#[test]
fn npm_ranges_cover_star_or_hyphen_and_prerelease() {
    assert!(version_affected("npm", "*", "18.2.0"));
    assert!(version_affected("npm", "1.2.7 || >=1.2.9 <2.0.0", "1.2.7"));
    assert!(version_affected("npm", "1.2.7 || >=1.2.9 <2.0.0", "1.2.9"));
    assert!(!version_affected("npm", "1.2.7 || >=1.2.9 <2.0.0", "1.2.8"));
    assert!(version_affected("npm", "1.2.3 - 2.3.4", "2.0.0"));
    assert!(!version_affected("npm", "1.2.3 - 2.3.4", "2.3.5"));
    assert!(version_affected("npm", ">=1.2.7 <1.3.0", "1.2.9"));
    assert!(!version_affected("npm", ">=1.2.7 <1.3.0", "1.3.0"));
    assert!(version_affected("npm", "1.2.3", "1.2.3"));
    assert!(!version_affected("npm", "1.2.3", "1.2.4"));
    assert!(!version_affected("npm", ">=1.0.0", "2.0.0-alpha"));
    assert!(version_affected(
        "npm",
        ">=1.0.0-alpha, <2.0.0",
        "1.0.0-alpha"
    ));
    assert!(version_affected("cargo", "1.2.0", "1.2.1"));
    assert!(!version_affected("cargo", "1.0.0 || 2.0.0", "1.0.0"));
    assert!(!version_affected("cargo", "1.2.3 - 2.3.4", "1.5.0"));
    assert!(!version_affected("npm", "", "1.2.3"));
    assert!(!version_affected("npm", "not a range", "1.2.3"));
    assert!(!version_affected("npm", ">=1.2.7 <1.3.0", "banana"));
}

#[test]
fn npm_range_advisories_report_findings() {
    let packages = vec![LockedPackage::new("npm", "react", "18.2.0")];
    let advisories = vec![
        Advisory {
            severity: "high".to_owned(),
            fixed: vec!["18.3.0".to_owned()],
            ..advisory("npm", "GHSA-npm-range", "react", ">=18.0.0 <18.3.0")
        },
        Advisory {
            severity: "high".to_owned(),
            ..advisory("npm", "GHSA-npm-miss", "react", ">=18.3.0 <19.0.0")
        },
    ];
    let (findings, unassessed) = match_packages(&packages, &advisories);
    assert!(unassessed.is_empty());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].advisory, "GHSA-npm-range");
    assert_eq!(findings[0].version, "18.2.0");
}

fn assert_exceptions_narrow(set: &str, name: &str, version: &str, covering: &str, outside: &str) {
    let id = format!("GHSA-{set}-exception");
    let packages = vec![LockedPackage::new(set, name, version)];
    let advisories = vec![advisory(set, &id, name, covering)];
    let (findings, _) = match_packages(&packages, &advisories);
    assert_eq!(findings.len(), 1, "{set}");
    let (unexempted, problems) = apply_exceptions(
        &findings,
        &[exception(set, &id, name, covering)],
        "2026-09-18",
    );
    assert!(problems.is_empty(), "{set}");
    assert!(unexempted.is_empty(), "{set}");
    let (unexempted, problems) = apply_exceptions(
        &findings,
        &[exception(set, &id, name, outside)],
        "2026-09-18",
    );
    assert!(problems.is_empty(), "{set}");
    assert_eq!(unexempted.len(), 1, "{set}");
}

#[test]
fn exceptions_narrow_with_ecosystem_semantics() {
    for (set, name, version, covering, outside) in [
        (
            "npm",
            "react",
            "18.2.0",
            ">=18.0.0 <18.3.0",
            ">=18.3.0 <19.0.0",
        ),
        (
            "go",
            "example.com/mod",
            "v1.2.4-0.20240101120000-abcdef123456",
            ">=v1.0.0, <v2.0.0",
            ">=v1.2.4, <v2.0.0",
        ),
        ("maven", "junit:junit", "4.13.2", "[4.0,5.0)", "[5.0,6.0)"),
        (
            "nuget",
            "Newtonsoft.Json",
            "13.0.1",
            "[12.0,13.0.2)",
            "[13.0.2,14.0)",
        ),
    ] {
        assert_exceptions_narrow(set, name, version, covering, outside);
    }
}

#[test]
fn scoped_set_exceptions_exempt_a_base_ecosystem_finding() {
    let packages = vec![LockedPackage::new("npm", "braces", "3.0.3")];
    let advisories = vec![Advisory {
        severity: "high".to_owned(),
        ..advisory("npm", "GHSA-npm-scoped", "braces", "<=3.0.3")
    }];
    let (findings, _) = match_packages(&packages, &advisories);
    assert_eq!(findings.len(), 1);
    let scoped = RiskException {
        reason: "No fixed release exists.".to_owned(),
        ..exception("npm-adopt", "GHSA-npm-scoped", "braces", "<=3.0.3")
    };
    let (unexempted, problems) = apply_exceptions(&findings, &[scoped], "2026-09-18");
    assert!(problems.is_empty());
    assert!(unexempted.is_empty());
}

#[test]
fn maven_version_ordering_follows_upstream_subset() {
    assert_eq!(maven_compare("1.0", "1.0.0"), std::cmp::Ordering::Equal);
    assert!(maven_version_eq("1.0", "1.0.0"));
    assert!(maven_version_eq("1.ga", "1"));
    assert!(maven_version_eq("1-final", "1"));
    assert!(maven_version_eq("1.0.0-foo.0.0", "1-foo"));
    assert!(maven_version_eq("1-a1", "1-alpha-1"));
    assert!(maven_compare("1-alpha", "1-beta") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-beta", "1-milestone") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-milestone", "1-rc") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-rc", "1-snapshot") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-snapshot", "1") == std::cmp::Ordering::Less);
    assert!(maven_compare("1", "1-sp") == std::cmp::Ordering::Less);
    assert!(maven_compare("1", "1-foo") == std::cmp::Ordering::Less);
    assert!(maven_compare("5.aardvark", "5.zebra") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-K", "1.7") == std::cmp::Ordering::Less);
    assert!(maven_compare("1-foo2", "1-foo10") == std::cmp::Ordering::Less);
    assert_eq!(maven_compare("1.foo", "1-foo"), std::cmp::Ordering::Equal);
    assert!(maven_compare("1-1", "1.1") == std::cmp::Ordering::Less);
    assert!(maven_compare("1", "1.1") == std::cmp::Ordering::Less);
    assert!(maven_compare("2.0-rc1", "2.0") == std::cmp::Ordering::Less);
    assert!(maven_compare("1.10", "1.9") == std::cmp::Ordering::Greater);
}

#[test]
fn maven_ordering_matches_comparable_version() {
    assert_eq!(
        maven_compare("1.0.0.beta", "1-beta"),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        maven_compare("2.0.0.beta", "2-beta"),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        maven_compare("1.0.0.X1", "1.0.0-X1"),
        std::cmp::Ordering::Equal
    );
    assert!(maven_version_eq("1.0.0.Final", "1.0.0"));
    assert!(maven_version_eq("2.4.0.RELEASE", "2.4.0"));
    assert!(maven_version_eq("1.0.0-cr1", "1.0.0-rc1"));
    assert!(maven_version_eq("1.0.0-M1", "1.0.0-milestone-1"));
    assert!(maven_compare("1.0_rc1", "1.0") == std::cmp::Ordering::Greater);
    assert!(maven_compare("1.0_rc1", "1.0-cr1") == std::cmp::Ordering::Greater);
    assert!(maven_compare("0-rc", "0.milestone") == std::cmp::Ordering::Greater);
    assert!(maven_compare("1.0.0.beta", "1-alpha") == std::cmp::Ordering::Greater);
    assert!(maven_compare("1.0.0-beta", "1.0.0-alpha") == std::cmp::Ordering::Greater);
}

#[test]
fn maven_ranges_cover_intervals_unions_and_edges() {
    assert!(maven_in_scope("[1.0,2.0]", "1.0"));
    assert!(maven_in_scope("[1.0,2.0]", "2.0"));
    assert!(!maven_in_scope("(1.0,2.0)", "1.0"));
    assert!(!maven_in_scope("(1.0,2.0)", "2.0"));
    assert!(maven_in_scope("[1.0,2.0)", "1.0"));
    assert!(!maven_in_scope("[1.0,2.0)", "2.0"));
    assert!(maven_in_scope("[1.0,2.0)", "2.0-rc1"));
    assert!(maven_in_scope("[1.5,)", "1.5"));
    assert!(maven_in_scope("[1.5,)", "9.9"));
    assert!(!maven_in_scope("[1.5,)", "1.4"));
    assert!(maven_in_scope("(,1.0]", "1.0"));
    assert!(!maven_in_scope("(,1.0]", "1.0.1"));
    assert!(maven_in_scope("(,1.0)", "0.9"));
    assert!(!maven_in_scope("(,1.0)", "1.0"));
    assert!(maven_in_scope("[1.0]", "1.0.0"));
    assert!(!maven_in_scope("[1.0]", "1.0.1"));
    assert!(!maven_in_scope("(1.0)", "1.0"));
    assert!(maven_in_scope("(,1.0],[1.2,)", "1.0"));
    assert!(maven_in_scope("(,1.0],[1.2,)", "1.2"));
    assert!(!maven_in_scope("(,1.0],[1.2,)", "1.1"));
    assert!(maven_in_scope("(,1.1),(1.1,)", "1.0"));
    assert!(maven_in_scope("(,1.1),(1.1,)", "1.2"));
    assert!(!maven_in_scope("(,1.1),(1.1,)", "1.1"));
    assert!(maven_in_scope("1.2.0", "1.2.0"));
    assert!(maven_in_scope("1.0", "1.0.0"));
    assert!(!maven_in_scope("1.2.0", "1.2.1"));
    assert!(!maven_in_scope(">=1.0.0", "1.2.0"));
    assert!(!maven_in_scope("", "1.0"));
    assert!(!maven_in_scope("[1.0,2.0)", ""));
    assert!(!maven_in_scope("[1.0,2.0", "1.5"));
    assert!(!maven_in_scope("[1.0,2.0,3.0]", "1.5"));
    assert!(!maven_in_scope("(,)", "1.0"));
    assert!(!maven_in_scope("[]", "1.0"));
}

#[test]
fn maven_range_bounds_follow_comparable_version() {
    assert!(maven_in_scope("[1.0.0.Final,2.0.0)", "1.0.0"));
    assert!(maven_in_scope("[1.0.0.RELEASE,2.0)", "1.0.0"));
    assert!(maven_in_scope("[1.0.0.Final,)", "1.0"));
    assert!(!maven_in_scope("[1.0.0.Final,)", "0.9"));
    assert!(maven_in_scope("[1.0-cr1,2.0)", "1.0-rc1"));
    assert!(maven_in_scope("[1.0-ga,2.0)", "1.0"));
    assert!(maven_in_scope("(,2.0.0.RELEASE]", "2.0.0"));
    assert!(!maven_in_scope("(,2.0.0.RELEASE]", "2.0.1"));
    assert!(maven_in_scope("[1.0.0.M1,2.0)", "1.0.0-milestone-1"));
    assert!(maven_in_scope("(,1.1),(1.1,)", "1.0.0.0"));
    assert!(!maven_in_scope("(,1.1),(1.1,)", "1.1.0.Final"));
}

#[test]
fn maven_range_advisories_report_findings() {
    let packages = vec![LockedPackage::new(
        "maven",
        "com.google.guava:guava",
        "32.0.0",
    )];
    let advisories = vec![
        Advisory {
            severity: "high".to_owned(),
            fixed: vec!["33.0".to_owned()],
            ..advisory(
                "maven",
                "GHSA-maven-range",
                "com.google.guava:guava",
                "[30.0,33.0)",
            )
        },
        Advisory {
            severity: "high".to_owned(),
            ..advisory(
                "maven",
                "GHSA-maven-miss",
                "com.google.guava:guava",
                "[33.0,34.0)",
            )
        },
    ];
    let (findings, unassessed) = match_packages(&packages, &advisories);
    assert!(unassessed.is_empty());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].advisory, "GHSA-maven-range");
    assert_eq!(findings[0].version, "32.0.0");
}

#[test]
fn nuget_version_ordering_follows_upstream_subset() {
    assert_eq!(nuget_compare("1.0", "1.0.0"), std::cmp::Ordering::Equal);
    assert_eq!(nuget_compare("1.0", "1.0.0.0"), std::cmp::Ordering::Equal);
    assert!(nuget_version_eq("1.0", "1.0.0"));
    assert!(nuget_version_eq("1.00", "1.0.0.0"));
    assert!(nuget_version_eq("1.01.1", "1.1.1"));
    assert!(nuget_version_eq("1.0.0.0", "1.0.0"));
    assert!(nuget_version_eq("10.1.201", "10.1.201.0"));
    assert!(nuget_version_eq("1.0.7+r3456", "1.0.7"));
    assert_eq!(
        nuget_compare("1.0.0+build", "1.0.0"),
        std::cmp::Ordering::Equal
    );
    assert!(nuget_compare("1.10", "1.9") == std::cmp::Ordering::Greater);
    assert!(nuget_compare("2.0.0.1", "2.0.0") == std::cmp::Ordering::Greater);
    assert!(nuget_compare("1.0.0", "1.0.0.1") == std::cmp::Ordering::Less);
    assert!(nuget_compare("1.0.0", "1.0.0-alpha") == std::cmp::Ordering::Greater);
    assert!(nuget_compare("1.0.0-alpha", "1.0.0") == std::cmp::Ordering::Less);
    assert!(nuget_compare("1.0.0-alpha.9", "1.0.0-alpha.10") == std::cmp::Ordering::Less);
    assert!(nuget_compare("1.0.0-1", "1.0.0-alpha") == std::cmp::Ordering::Less);
    assert!(nuget_version_eq("1.0.0-alpha", "1.0.0-Alpha"));
    assert!(nuget_version_eq("1.0.0-ALPHA", "1.0.0-alpha"));
    assert!(nuget_compare("1.0.0-alpha", "1.0.0-alpha.1") == std::cmp::Ordering::Less);
    assert!(nuget_compare("1.0.0-beta", "1.0.0-alpha") == std::cmp::Ordering::Greater);
    assert!(nuget_compare("2.0.0-alpha", "1.9.9") == std::cmp::Ordering::Greater);
}

#[test]
fn nuget_ranges_cover_intervals_minimums_and_edges() {
    assert!(nuget_in_scope("[1.0,2.0]", "1.0"));
    assert!(nuget_in_scope("[1.0,2.0]", "2.0"));
    assert!(!nuget_in_scope("(1.0,2.0)", "1.0"));
    assert!(!nuget_in_scope("(1.0,2.0)", "2.0"));
    assert!(nuget_in_scope("[1.0,2.0)", "1.0"));
    assert!(!nuget_in_scope("[1.0,2.0)", "2.0"));
    assert!(nuget_in_scope("(1.0,2.0]", "2.0"));
    assert!(!nuget_in_scope("(1.0,2.0]", "1.0"));
    assert!(nuget_in_scope("[1.0,)", "1.0"));
    assert!(nuget_in_scope("[1.0,)", "9.9"));
    assert!(!nuget_in_scope("[1.0,)", "0.9"));
    assert!(nuget_in_scope("(1.0,)", "1.0.1"));
    assert!(!nuget_in_scope("(1.0,)", "1.0"));
    assert!(nuget_in_scope("(,1.0]", "1.0"));
    assert!(!nuget_in_scope("(,1.0]", "1.0.1"));
    assert!(nuget_in_scope("(,1.0)", "0.9"));
    assert!(!nuget_in_scope("(,1.0)", "1.0"));
    assert!(nuget_in_scope("[1.0]", "1.0.0"));
    assert!(!nuget_in_scope("[1.0]", "1.0.1"));
    assert!(!nuget_in_scope("(1.0)", "1.0"));
    assert!(!nuget_in_scope("[1.0)", "1.0"));
    assert!(!nuget_in_scope("(1.0]", "1.0"));
    assert!(nuget_in_scope("1.2.0", "1.2.0"));
    assert!(nuget_in_scope("1.0", "1.0.0"));
    assert!(!nuget_in_scope("1.0", "1.0.1"));
    assert!(!nuget_in_scope("1.0", "0.9"));
    assert!(!nuget_in_scope(">=1.0.0", "1.2.0"));
    assert!(nuget_in_scope("[1.0,2.0)", "2.0.0-beta"));
    assert!(!nuget_in_scope("[1.0,2.0)", "2.0.0"));
    assert!(!nuget_in_scope("", "1.0"));
    assert!(!nuget_in_scope("[1.0,2.0)", ""));
    assert!(!nuget_in_scope("[1.0,2.0", "1.5"));
    assert!(!nuget_in_scope("[1.0,2.0,3.0]", "1.5"));
    assert!(!nuget_in_scope("(,)", "1.0"));
    assert!(!nuget_in_scope("[]", "1.0"));
    assert!(!nuget_in_scope("1.*", "1.5.0"));
    assert!(!nuget_in_scope("[1.*,2.0)", "1.5.0"));
    assert!(!nuget_in_scope("(,1.0],[1.2,)", "1.0"));
    assert!(!nuget_in_scope("[1.0]", "not-a-version"));
    assert!(!nuget_in_scope("not-a-version", "1.0.0"));
}

#[test]
fn nuget_malformed_versions_and_bounds_fail_closed() {
    assert_eq!(nuget_compare("", "1.0"), std::cmp::Ordering::Equal);
    assert_eq!(
        nuget_compare(&"1".repeat(300), "1.0"),
        std::cmp::Ordering::Equal
    );
    assert_eq!(nuget_compare("banana", "1.0"), std::cmp::Ordering::Equal);
    assert!(!nuget_version_eq("", "1.0"));
    assert!(!nuget_version_eq("1.0", &"1".repeat(300)));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.*"));
    assert!(!nuget_in_scope("[1.0,2.0)", "+"));
    assert!(!nuget_in_scope("[1.0,2.0)", "-1.0"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.2.3.4.5"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.0-"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.0-a b"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.0-a..b"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.0-a_b"));
    assert!(!nuget_in_scope("[1.0,2.0)", "1.0-00"));
    assert!(!nuget_in_scope(&"1".repeat(5000), "1.0"));
    assert!(!nuget_in_scope("[ ]", "1.0"));
    assert!(!nuget_in_scope("[banana]", "1.0"));
    assert!(!nuget_in_scope(
        &format!("[{},2.0)", "1".repeat(300)),
        "1.0"
    ));
    assert!(!nuget_in_scope("[banana,2.0)", "1.0"));
    assert!(!nuget_in_scope("[1.0,banana)", "1.0"));
}

#[test]
fn nuget_prerelease_labels_compare_numeric_then_alpha_upstream() {
    assert_eq!(
        nuget_compare("1.0-1.beta", "1.0-1.beta"),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        nuget_compare("1.0-alpha", "1.0-2"),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn nuget_range_advisories_report_findings() {
    let packages = vec![LockedPackage::new("nuget", "Newtonsoft.Json", "13.0.1")];
    let advisories = vec![
        Advisory {
            severity: "high".to_owned(),
            fixed: vec!["13.0.2".to_owned()],
            ..advisory(
                "nuget",
                "GHSA-nuget-range",
                "Newtonsoft.Json",
                "[12.0,13.0.2)",
            )
        },
        Advisory {
            severity: "high".to_owned(),
            ..advisory(
                "nuget",
                "GHSA-nuget-miss",
                "Newtonsoft.Json",
                "[13.0.2,14.0)",
            )
        },
    ];
    let (findings, unassessed) = match_packages(&packages, &advisories);
    assert!(unassessed.is_empty());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].advisory, "GHSA-nuget-range");
    assert_eq!(findings[0].version, "13.0.1");
}

#[test]
fn unassessed_reasons_are_pinned() {
    assert_eq!(REASON_GIT, "unsupported git revision");
    assert_eq!(REASON_PRIVATE, "unidentified private package");
}

#[test]
fn git_and_private_packages_are_incomplete_never_clean() {
    let pkgs = vec![
        LockedPackage {
            is_git: true,
            ..LockedPackage::new("cargo", "git-dep", "abc123")
        },
        LockedPackage {
            is_private: true,
            ..LockedPackage::new("npm", "internal", "0.1.0")
        },
        LockedPackage {
            is_git: true,
            ..LockedPackage::new("nuget", "git-nuget-dep", "1.0.0")
        },
        LockedPackage {
            is_private: true,
            ..LockedPackage::new("maven", "private-maven", "2.0.0")
        },
    ];
    let (findings, unassessed) = match_packages(&pkgs, &advisories());
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 4);
    assert!(unassessed.iter().any(|u| u.reason == REASON_GIT));
    assert!(unassessed.iter().any(|u| u.reason == REASON_PRIVATE));
    assert!(unassessed.iter().filter(|u| u.reason == REASON_GIT).count() == 2);
}

#[test]
fn clean_package_with_no_matching_advisory_is_clean() {
    let pkgs = vec![LockedPackage::new("cargo", "clean", "9.9.9")];
    let (findings, unassessed) = match_packages(&pkgs, &advisories());
    assert!(findings.is_empty());
    assert!(unassessed.is_empty());
}

#[test]
fn exceptions_exempt_only_matching_versioned_findings() {
    let (findings, _) = match_packages(&packages(), &advisories());
    let exception = exception("cargo", "GHSA-aaaa-bbbb-cccc", "serde", ">=1.0.0, <1.0.150");
    let (unexempted, problems) = apply_exceptions(&findings, &[exception], "2026-09-18");
    assert!(problems.is_empty());
    assert_eq!(unexempted.len(), 1);
    assert_eq!(unexempted[0].package, "react");
}

#[test]
fn exception_problems_fail_and_obsolete_is_reported() {
    let (findings, _) = match_packages(&packages(), &advisories());
    let expired = RiskException {
        reason: "Old.".to_owned(),
        expires: "2026-01-01".to_owned(),
        ..exception("cargo", "GHSA-aaaa-bbbb-cccc", "serde", ">=1.0.0, <1.0.150")
    };
    let (_, problems) = apply_exceptions(&findings, &[expired], "2026-09-18");
    assert!(!problems.is_empty());
    let obsolete = RiskException {
        reason: "Stale.".to_owned(),
        ..exception("cargo", "GHSA-gone", "gone", "*")
    };
    let (_, problems) = apply_exceptions(&findings, &[obsolete], "2026-09-18");
    assert!(problems
        .iter()
        .any(|p| matches!(p, ExceptionProblem::Obsolete { .. })));
}

#[test]
fn snapshot_parses_osv_array_and_rejects_malformed() {
    let text = r#"[{"id":"GHSA-1","package":"serde","versions":">=1.0.0, <2.0.0","severity":"high","fixed":["1.5.0"],"set":"cargo"}]"#;
    let parsed = parse_snapshot(text).expect("parses");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "GHSA-1");
    assert!(parse_snapshot("not json").is_err());
}

#[test]
fn osv_typed_cargo_projects_interval_and_matches() {
    let text = r#"[{
        "id": "GHSA-cargo-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "1.0.0"}, {"fixed": "1.5.0"}]}],
            "database_specific": {"severity": "high"}
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "GHSA-cargo-osv-0001");
    assert_eq!(parsed[0].package, "serde");
    assert_eq!(parsed[0].set, "cargo");
    assert_eq!(parsed[0].versions, ">=1.0.0, <1.5.0");
    assert_eq!(parsed[0].severity, "high");
    assert_eq!(parsed[0].fixed, vec!["1.5.0".to_owned()]);
    assert!(version_affected("cargo", &parsed[0].versions, "1.2.0"));
    assert!(!version_affected("cargo", &parsed[0].versions, "1.5.0"));
}

#[test]
fn osv_explicit_versions_do_not_caret_match_fixed_release() {
    let text = r#"[{
        "id": "GHSA-qx2v-8332-m4fv",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "slab", "ecosystem": "crates.io"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0.4.10"}, {"fixed": "0.4.11"}]}],
            "versions": ["0.4.10"]
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert!(parsed.iter().any(|entry| entry.versions == "=0.4.10"));
    assert!(parsed
        .iter()
        .any(|entry| entry.versions == ">=0.4.10, <0.4.11"));
    let any_affected = |version: &str| {
        parsed
            .iter()
            .any(|entry| version_affected("cargo", &entry.versions, version))
    };
    assert!(any_affected("0.4.10"));
    assert!(!any_affected("0.4.11"));
    assert!(!any_affected("0.4.12"));
}

#[test]
fn osv_typed_sets_project_to_native_scopes() {
    let text = r#"[{
        "id": "GHSA-npm-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "react", "ecosystem": "npm"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "18.0.0"}, {"fixed": "18.3.0"}]}]
        }]
    },
    {
        "id": "GHSA-go-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "github.com/google/go-cmp", "ecosystem": "Go"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "v0.5.0"}, {"fixed": "v0.7.0"}]}]
        }]
    },
    {
        "id": "GHSA-maven-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "com.google.guava:guava", "ecosystem": "Maven"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "30.0"}, {"fixed": "33.0"}]}]
        }]
    },
    {
        "id": "GHSA-nuget-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "Newtonsoft.Json", "ecosystem": "NuGet"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "12.0"}, {"fixed": "13.0.2"}]}]
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert_eq!(parsed.len(), 4);
    let npm = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-npm-osv-0001")
        .expect("npm");
    assert_eq!(npm.set, "npm");
    assert_eq!(npm.versions, ">=18.0.0, <18.3.0");
    assert!(version_affected("npm", &npm.versions, "18.2.0"));
    assert!(!version_affected("npm", &npm.versions, "18.3.0"));
    let go = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-go-osv-0001")
        .expect("go");
    assert_eq!(go.set, "go");
    assert!(version_affected("go", &go.versions, "v0.6.0"));
    assert!(!version_affected("go", &go.versions, "v0.7.0"));
    let maven = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-maven-osv-0001")
        .expect("maven");
    assert_eq!(maven.set, "maven");
    assert_eq!(maven.versions, "[30.0,33.0)");
    assert!(version_affected("maven", &maven.versions, "32.0.0"));
    assert!(!version_affected("maven", &maven.versions, "33.0"));
    let nuget = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-nuget-osv-0001")
        .expect("nuget");
    assert_eq!(nuget.set, "nuget");
    assert_eq!(nuget.versions, "[12.0,13.0.2)");
    assert!(version_affected("nuget", &nuget.versions, "13.0.1"));
    assert!(!version_affected("nuget", &nuget.versions, "13.0.2"));
}

#[test]
fn osv_typed_rubygems_projects_concrete_versions_and_intervals() {
    let text = r#"[{
        "id": "GHSA-ruby-osv-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "diff-lcs", "ecosystem": "RubyGems", "purl": "pkg:gem/diff-lcs"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "0"}, {"fixed": "1.5.1"}]}],
            "versions": ["1.4.0", "1.5.0"],
            "database_specific": {"severity": "moderate"}
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert!(parsed.iter().all(|entry| entry.set == "ruby"));
    assert!(parsed.iter().any(|entry| entry.versions == "1.5.0"));
    assert!(parsed.iter().any(|entry| entry.versions == "<1.5.1"));
    assert_eq!(parsed[0].severity, "moderate");
    assert_eq!(parsed[0].fixed, vec!["1.5.1".to_owned()]);
    let affected = |version: &str| {
        parsed
            .iter()
            .any(|entry| version_affected("ruby", &entry.versions, version))
    };
    assert!(affected("1.5.0"));
    assert!(affected("1.4.0"));
    assert!(!affected("1.5.1"));
    assert!(
        ruby_in_scope("1.16.0-x86_64-linux", "1.16.0-x86_64-linux"),
        "a platform gem version matches itself"
    );
    assert!(!ruby_in_scope("1.5.0", "1.5.1"));
    assert!(!ruby_in_scope("", "1.5.0"));
    assert!(ruby_in_scope(">=1.0, <2.0", "1.5.0"));
    assert!(!ruby_in_scope(">=2.0", "1.5.0"));
    assert!(ruby_in_scope("*", "1.5.0"));
}

#[test]
fn osv_typed_withdrawn_unsupported_and_git_skip() {
    let text = r#"[{
        "id": "GHSA-withdrawn-0001",
        "modified": "2026-09-18T00:00:00Z",
        "withdrawn": "2026-09-19T00:00:00Z",
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "1.0.0"}, {"fixed": "2.0.0"}]}]
        }]
    },
    {
        "id": "GHSA-packagist-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "monolog/monolog", "ecosystem": "Packagist"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "1.0"}, {"fixed": "2.0"}]}]
        }]
    },
    {
        "id": "GHSA-git-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "ranges": [{"type": "GIT", "events": [{"introduced": "abc123"}, {"fixed": "def456"}]}]
        }]
    },
    {
        "id": "GHSA-explicit-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "versions": ["1.2.3", "1.2.4"]
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert_eq!(parsed.len(), 2);
    assert!(parsed.iter().all(|entry| entry.id == "GHSA-explicit-0001"));
    assert!(parsed.iter().any(|entry| entry.versions == "=1.2.3"));
    assert!(parsed.iter().any(|entry| entry.versions == "=1.2.4"));
    assert!(version_affected("cargo", "=1.2.3", "1.2.3"));
    assert!(!version_affected("cargo", "=1.2.3", "1.2.4"));
    assert!(!version_affected("cargo", "=1.2.3", "1.3.0"));
}

#[test]
fn osv_typed_severity_unknown_fails_closed_and_last_affected_inclusive() {
    let text = r#"[{
        "id": "GHSA-cvss-0001",
        "modified": "2026-09-18T00:00:00Z",
        "severity": [{"type": "CVSS_V3", "score": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"}],
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "1.0.0"}, {"last_affected": "1.4.0"}]}]
        }]
    },
    {
        "id": "GHSA-unbounded-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"name": "serde", "ecosystem": "crates.io"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "2.0.0"}]}]
        }]
    }]"#;
    let parsed = parse_snapshot(text).expect("osv parses");
    assert_eq!(parsed.len(), 2);
    let cvss = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-cvss-0001")
        .expect("cvss");
    assert_eq!(cvss.severity, "");
    assert_eq!(cvss.versions, ">=1.0.0, <=1.4.0");
    assert_eq!(canonical_severity(&cvss.severity), "unknown");
    assert_eq!(
        normalize_level(&canonical_severity(&cvss.severity)),
        "error"
    );
    assert!(version_affected("cargo", &cvss.versions, "1.4.0"));
    assert!(!version_affected("cargo", &cvss.versions, "1.4.1"));
    let unbounded = parsed
        .iter()
        .find(|entry| entry.id == "GHSA-unbounded-0001")
        .expect("unbounded");
    assert_eq!(unbounded.versions, "<2.0.0");
    assert!(version_affected("cargo", &unbounded.versions, "1.9.0"));
}

#[test]
fn osv_typed_malformed_fails_closed() {
    assert!(parse_snapshot(r#"[{"id":"GHSA-bad","affected":[]}]"#).is_err());
    assert!(parse_snapshot("not json").is_err());
    assert!(parse_snapshot("").is_err());
    assert_eq!(parse_snapshot("[]").expect("empty"), Vec::new());
}

fn nuget_package(name: &str) -> LockedPackage {
    LockedPackage::new("nuget", name, "12.0.1")
}

fn nuget_advisory(package: &str) -> Advisory {
    Advisory {
        severity: "high".to_owned(),
        ..advisory("nuget", "GHSA-nuget-case", package, "(,13.0.1)")
    }
}

#[test]
fn nuget_package_ids_match_case_insensitively() {
    let advisories = vec![nuget_advisory("Newtonsoft.Json")];
    let (lower, _) = match_packages(&[nuget_package("newtonsoft.json")], &advisories);
    let (exact, _) = match_packages(&[nuget_package("Newtonsoft.Json")], &advisories);
    assert_eq!(lower.len(), 1);
    assert_eq!(exact.len(), 1);
    assert_eq!(lower[0].package, "newtonsoft.json");
    assert_eq!(lower[0].advisory, exact[0].advisory);
}

#[test]
fn nuget_distinct_and_malformed_ids_do_not_match() {
    let advisories = vec![nuget_advisory("Newtonsoft.Json")];
    for name in ["Newtonsoft.Json.Bson", "", "  "] {
        let (findings, _) = match_packages(&[nuget_package(name)], &advisories);
        assert!(findings.is_empty(), "{name:?}");
    }
    let (findings, _) = match_packages(&[nuget_package("Newtonsoft.Json")], &[nuget_advisory("")]);
    assert!(findings.is_empty());
}

#[test]
fn case_sensitive_ecosystems_keep_exact_identity() {
    let mut package = nuget_package("Foo");
    package.set = "npm".to_owned();
    let mut advisory = nuget_advisory("foo");
    advisory.set = "npm".to_owned();
    advisory.versions = "<=12.0.1".to_owned();
    let (findings, _) = match_packages(&[package], &[advisory]);
    assert!(findings.is_empty());
}

#[test]
fn nuget_exceptions_match_case_insensitively_without_widening() {
    let (findings, _) = match_packages(
        &[nuget_package("newtonsoft.json")],
        &[nuget_advisory("Newtonsoft.Json")],
    );
    let exception = |package: &str, versions: &str| RiskException {
        reason: "No fixed release exists.".to_owned(),
        ..exception("nuget", "GHSA-nuget-case", package, versions)
    };
    let (left, problems) = apply_exceptions(
        &findings,
        &[exception("NEWTONSOFT.JSON", "(,13.0.0)")],
        "2026-09-18",
    );
    assert!(problems.is_empty());
    assert!(left.is_empty());
    let (left, _) = apply_exceptions(
        &findings,
        &[exception("NEWTONSOFT.JSON", "[13.0.0,)")],
        "2026-09-18",
    );
    assert_eq!(left.len(), 1);
}

fn ruby_osv(events: &str) -> Vec<Advisory> {
    let text = format!(
        r#"[{{
        "id": "GHSA-ruby-range-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{{
            "package": {{"name": "nokogiri", "ecosystem": "RubyGems"}},
            "ranges": [{{"type": "ECOSYSTEM", "events": {events}}}]
        }}]
    }}]"#
    );
    parse_snapshot(&text).expect("osv parses")
}

fn ruby_lock(spec: &str) -> Vec<LockedPackage> {
    let text = format!("GEM\n  remote: https://rubygems.org/\n  specs:\n    {spec}\n");
    crate::locks::parse_gemfile_lock(&text).expect("lock parses")
}

fn ruby_hits(events: &str, spec: &str) -> (usize, usize) {
    let (findings, unassessed) = match_packages(&ruby_lock(spec), &ruby_osv(events));
    (findings.len(), unassessed.len())
}

#[test]
fn ruby_four_component_version_matches_range_end_to_end() {
    let events = r#"[{"introduced": "2.0.0"}, {"fixed": "2.2.8.2"}]"#;
    assert_eq!(ruby_hits(events, "nokogiri (2.2.8.1)"), (1, 0));
    assert_eq!(ruby_hits(events, "nokogiri (2.2.8.2)"), (0, 0));
    assert_eq!(ruby_hits(events, "nokogiri (1.9.9.9)"), (0, 0));
    assert_eq!(ruby_hits(events, "nokogiri (2.0.0)"), (1, 0));
}

#[test]
fn ruby_platform_gem_matches_upstream_range_and_keeps_platform() {
    let events = r#"[{"introduced": "1.16.0"}, {"fixed": "1.16.9"}]"#;
    let (findings, unassessed) = match_packages(
        &ruby_lock("nokogiri (1.16.8-x86_64-linux)"),
        &ruby_osv(events),
    );
    assert!(unassessed.is_empty());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].version, "1.16.8-x86_64-linux");
    let fixed = ruby_lock("nokogiri (1.16.9-x86_64-linux)");
    assert!(match_packages(&fixed, &ruby_osv(events)).0.is_empty());
    let java = ruby_lock("nokogiri (1.16.8-java)");
    assert_eq!(match_packages(&java, &ruby_osv(events)).0.len(), 1);
}

#[test]
fn ruby_prerelease_and_bounds() {
    let fixed = r#"[{"introduced": "0"}, {"fixed": "3.0.0"}]"#;
    assert_eq!(ruby_hits(fixed, "nokogiri (3.0.0.pre.1)"), (1, 0));
    assert_eq!(ruby_hits(fixed, "nokogiri (3.0.0)"), (0, 0));
    assert_eq!(ruby_hits(fixed, "nokogiri (3.0.0.0)"), (0, 0));
    let last = r#"[{"introduced": "1.0.0"}, {"last_affected": "1.5.0"}]"#;
    assert_eq!(ruby_hits(last, "nokogiri (1.5.0)"), (1, 0));
    assert_eq!(ruby_hits(last, "nokogiri (1.5.0.1)"), (0, 0));
    assert_eq!(ruby_hits(last, "nokogiri (1.0.0.rc1)"), (0, 0));
    let disjoint = r#"[{"introduced": "1.0.0"}, {"fixed": "1.2.0"}, {"introduced": "2.0.0"}, {"fixed": "2.1.0"}]"#;
    assert_eq!(ruby_hits(disjoint, "nokogiri (1.1.0)"), (1, 0));
    assert_eq!(ruby_hits(disjoint, "nokogiri (1.5.0)"), (0, 0));
    assert_eq!(ruby_hits(disjoint, "nokogiri (2.0.5)"), (1, 0));
}

#[test]
fn ruby_unsupported_syntax_is_incomplete_not_clean() {
    let bad_scope = vec![Advisory {
        severity: "high".to_owned(),
        ..advisory("ruby", "GHSA-ruby-bad", "nokogiri", ">=1.0, <banana")
    }];
    let (findings, unassessed) = match_packages(&ruby_lock("nokogiri (1.5.0)"), &bad_scope);
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 1);
    assert_eq!(unassessed[0].reason, REASON_VERSION_SYNTAX);
    let events = r#"[{"introduced": "1.0.0"}, {"fixed": "2.0.0"}]"#;
    assert_eq!(ruby_hits(events, "nokogiri (1.x.@)"), (0, 1));
    let mut odd = ruby_lock("nokogiri (1.5.0)");
    odd[0].version = "not a version".to_owned();
    let (findings, unassessed) = match_packages(&odd, &ruby_osv(events));
    assert!(findings.is_empty());
    assert_eq!(unassessed.len(), 1);
}

#[test]
fn ruby_requirement_operators() {
    assert!(ruby_in_scope("~> 2.2.8", "2.2.9"));
    assert!(!ruby_in_scope("~> 2.2.8", "2.3.0"));
    assert!(ruby_in_scope("~> 2.2", "2.9.1"));
    assert!(ruby_in_scope("!= 1.0, >= 0.5", "1.1"));
    assert!(!ruby_in_scope("!= 1.0", "1.0.0"));
    assert!(ruby_in_scope("> 1.0", "1.0.0.1"));
    assert!(!ruby_in_scope("> 1.0", "1.0"));
    assert!(ruby_in_scope("= 2.2.8.1", "2.2.8.1-x86_64-linux"));
    assert!(ruby_in_scope("2.2.8.1", "2.2.8.1"));
    assert!(!ruby_in_scope("2.2.8.1", "2.2.8.2"));
    assert!(ruby_in_scope("1.2.3-beta", "1.2.3.pre.beta"));
}

#[test]
fn ruby_scoped_exceptions_cover_platform_findings() {
    let events = r#"[{"introduced": "1.16.0"}, {"fixed": "1.16.9"}]"#;
    let (findings, _) = match_packages(
        &ruby_lock("nokogiri (1.16.8-x86_64-linux)"),
        &ruby_osv(events),
    );
    let exception = |versions: &str| RiskException {
        reason: "accepted".to_owned(),
        expires: "2099-01-01".to_owned(),
        ..exception("ruby", "GHSA-ruby-range-0001", "nokogiri", versions)
    };
    for covering in [">= 1.16.0, < 1.16.9", "1.16.8", "1.16.8-x86_64-linux"] {
        let (left, problems) = apply_exceptions(&findings, &[exception(covering)], "2026-10-22");
        assert!(left.is_empty(), "{covering}");
        assert!(problems.is_empty(), "{covering}");
    }
    let (left, _) = apply_exceptions(&findings, &[exception("< 1.16.0")], "2026-10-22");
    assert_eq!(left.len(), 1);
    let (left, _) = apply_exceptions(&findings, &[exception("<banana")], "2026-10-22");
    assert_eq!(left.len(), 1);
}

#[test]
fn a_nuget_advisory_never_matches_a_powershell_module_of_the_same_name_and_version() {
    let module = LockedPackage::new("powershell", "ExampleModule", "1.2.3");
    let advisory = Advisory {
        severity: "high".to_owned(),
        fixed: vec!["2.0.0".to_owned()],
        ..advisory(
            "nuget",
            "GHSA-collision-0001",
            "ExampleModule",
            "[1.0.0, 2.0.0)",
        )
    };
    let (findings, unassessed) = match_packages(&[module], &[advisory]);
    assert!(findings.is_empty());
    assert!(unassessed.is_empty());
}

fn pypi_osv(events: &str) -> Vec<Advisory> {
    let text = format!(
        r#"[{{
        "id": "PYSEC-2024-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{{
            "package": {{"ecosystem": "PyPI", "name": "Django"}},
            "ranges": [{{"type": "ECOSYSTEM", "events": {events}}}]
        }}]
    }}]"#
    );
    parse_snapshot(&text).expect("parses")
}

fn pypi_lock(name: &str, version: &str) -> Vec<LockedPackage> {
    vec![LockedPackage::new("uv", name, version)]
}

fn pypi_hits(events: &str, name: &str, version: &str) -> (usize, usize) {
    let (findings, unassessed) = match_packages(&pypi_lock(name, version), &pypi_osv(events));
    (findings.len(), unassessed.len())
}

#[test]
fn pypi_names_normalize_per_pep_503() {
    assert_eq!(normalize_pypi_name("Django"), "django");
    assert_eq!(normalize_pypi_name("zope.interface"), "zope-interface");
    assert_eq!(normalize_pypi_name("Foo__Bar..baz--qux"), "foo-bar-baz-qux");
    assert_eq!(normalize_pypi_name("  Pillow "), "pillow");
    let events = r#"[{"introduced":"0"},{"fixed":"3.2.5"}]"#;
    assert_eq!(pypi_hits(events, "django", "3.2.4"), (1, 0));
    assert_eq!(pypi_hits(events, "DJANGO", "3.2.4"), (1, 0));
    assert_eq!(pypi_hits(events, "dja-ngo", "3.2.4"), (0, 0));
    assert!(!same_package("uv", "", ""));
}

#[test]
fn pypi_ranges_use_pep_440_ordering() {
    let events = r#"[{"introduced":"1.0"},{"fixed":"2.0"}]"#;
    for (version, hit) in [
        ("0.9", 0),
        ("1.0", 1),
        ("1.0.0", 1),
        ("1.9.9", 1),
        ("2.0rc1", 1),
        ("2.0.dev1", 1),
        ("2.0", 0),
        ("2.0.post1", 0),
        ("1!0.5", 0),
        ("1.0+local", 1),
        ("1.0a1", 0),
    ] {
        assert_eq!(pypi_hits(events, "django", version), (hit, 0), "{version}");
    }
    let inclusive = r#"[{"introduced":"0"},{"last_affected":"2.0"}]"#;
    assert_eq!(pypi_hits(inclusive, "django", "2.0"), (1, 0));
    assert_eq!(pypi_hits(inclusive, "django", "2.0.1"), (0, 0));
}

#[test]
fn pypi_exact_versions_and_wildcard_scopes() {
    assert_eq!(pypi_assess("=1.0", "1.0.0"), PypiScope::Affected);
    assert_eq!(pypi_assess("=1.0", "1.1"), PypiScope::Unaffected);
    assert_eq!(pypi_assess("*", "9.9"), PypiScope::Affected);
    assert_eq!(pypi_assess("", "1.0"), PypiScope::Unaffected);
    assert!(pypi_in_scope(">=1.0, <2.0", "1.5"));
    assert!(!pypi_in_scope(">=1.0, <2.0", "2.0"));
}

#[test]
fn pypi_unparseable_versions_are_unassessed_never_clean() {
    let events = r#"[{"introduced":"0"},{"fixed":"3.2.5"}]"#;
    assert_eq!(pypi_hits(events, "django", "not-a-version"), (0, 1));
    assert_eq!(pypi_hits(events, "django", ""), (0, 1));
    assert_eq!(pypi_assess(">=banana", "1.0"), PypiScope::Incomplete);
    assert_eq!(pypi_assess("~=1.0", "1.0"), PypiScope::Incomplete);
}

#[test]
fn pypi_findings_carry_fixed_versions_and_skip_withdrawn() {
    let events = r#"[{"introduced":"0"},{"fixed":"3.2.5"}]"#;
    let (findings, _) = match_packages(&pypi_lock("django", "3.2.0"), &pypi_osv(events));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].advisory, "PYSEC-2024-0001");
    assert_eq!(findings[0].fixed, vec!["3.2.5".to_owned()]);
    let withdrawn = r#"[{
        "id": "PYSEC-1", "modified": "2026-09-18T00:00:00Z", "withdrawn": "2024-01-01T00:00:00Z",
        "affected": [{"package": {"ecosystem": "PyPI", "name": "x"},
        "versions": ["1.0"]}]
    }]"#;
    assert!(parse_snapshot(withdrawn).expect("parses").is_empty());
    let listed = r#"[{
        "id": "PYSEC-2", "modified": "2026-09-18T00:00:00Z",
        "affected": [{"package": {"ecosystem": "PyPI", "name": "x"},
        "versions": ["1.0", "1.1"]}]
    }]"#;
    let advisories = parse_snapshot(listed).expect("parses");
    assert_eq!(advisories.len(), 2);
    assert!(advisories.iter().all(|entry| entry.set == "uv"));
    let (hit, _) = match_packages(&pypi_lock("X", "1.1"), &advisories);
    assert_eq!(hit.len(), 1);
}
