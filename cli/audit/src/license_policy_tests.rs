use super::*;
use crate::license_expr::{evaluate, IdClass, LicenseExpr, TierOutcome};

fn tables() -> PolicyTables {
    PolicyTables {
        blocked: ["AGPL-3.0-only"]
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
        allow: ["MIT"].iter().map(|item| (*item).to_owned()).collect(),
        review: ["MPL-2.0"].iter().map(|item| (*item).to_owned()).collect(),
        deny: ["GPL-3.0-only"]
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
    }
}

fn exception() -> LicenseException {
    LicenseException {
        package: "some-copyleft-lib".to_owned(),
        set: "cargo".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: ">=1.2.0, <2.0.0".to_owned(),
        reason: "Legal approved for internal fork; re-review on major bump.".to_owned(),
        expires: "2027-03-01".to_owned(),
    }
}

fn finding() -> LicenseFinding {
    LicenseFinding {
        package: "some-copyleft-lib".to_owned(),
        set: "cargo".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: "1.2.0".to_owned(),
    }
}

#[test]
fn single_listed_tables_validate() {
    tables().validate().expect("single listing passes");
}

#[test]
fn identity_in_two_lists_fails() {
    let mut bad = tables();
    bad.deny.insert("MIT".to_owned());
    assert_eq!(
        bad.validate(),
        Err(PolicyProblem::MultiListed {
            identity: "MIT".to_owned()
        })
    );
    let mut bad = tables();
    bad.review.insert("AGPL-3.0-only".to_owned());
    assert!(matches!(
        bad.validate(),
        Err(PolicyProblem::MultiListed { .. })
    ));
}

#[test]
fn set_adjustments_merge_additively_and_conflicts_fail() {
    let tables = tables();
    let additive = SetAdjustment {
        set: "npm-root".to_owned(),
        review: ["Unicode-3.0"]
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
    };
    tables.validate_set(&additive).expect("additive merges");
    let conflicting = SetAdjustment {
        set: "npm-root".to_owned(),
        review: ["MIT"].iter().map(|item| (*item).to_owned()).collect(),
    };
    assert_eq!(
        tables.validate_set(&conflicting),
        Err(PolicyProblem::SetConflict {
            set: "npm-root".to_owned(),
            identity: "MIT".to_owned(),
        })
    );
}

#[test]
fn unlisted_distributables_default_distributed() {
    let distribution = Distribution {
        internal: ["//tools/internal-admin:binary"]
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
    };
    assert_eq!(
        distribution.tier_of("//tools/internal-admin:binary"),
        Tier::Internal
    );
    assert_eq!(
        distribution.tier_of("//services/payments:image"),
        Tier::Distributed
    );
    assert_eq!(distribution.tier_of("//cli:dx"), Tier::Distributed);
}

#[test]
fn promotion_to_distributed_requalifies_under_strict_table() {
    let internal = Distribution {
        internal: ["//cli:dx"].iter().map(|item| (*item).to_owned()).collect(),
    };
    let promoted = Distribution::default();
    assert_eq!(internal.tier_of("//cli:dx"), Tier::Internal);
    assert_eq!(promoted.tier_of("//cli:dx"), Tier::Distributed);
    let lookup = |id: &str| match id {
        "MPL-2.0" => IdClass::Review,
        _ => IdClass::Unlisted,
    };
    let denied: fn(&str) -> bool = |_| false;
    let expr = LicenseExpr::Ident("MPL-2.0".to_owned());
    assert_eq!(
        evaluate(&expr, Tier::Internal, &lookup, &denied),
        TierOutcome::Allow
    );
    assert_eq!(
        evaluate(&expr, Tier::Distributed, &lookup, &denied),
        TierOutcome::Review
    );
    let unlisted = LicenseExpr::Ident("MIT".to_owned());
    assert_eq!(
        evaluate(&unlisted, Tier::Internal, &lookup, &denied),
        TierOutcome::Allow
    );
    assert_eq!(
        evaluate(&unlisted, Tier::Distributed, &lookup, &denied),
        TierOutcome::Deny
    );
}

#[test]
fn valid_license_exception_passes_and_applies() {
    validate_license_exception(&exception(), "2026-09-14").expect("valid");
    check_applies(&exception(), &[finding()]).expect("applies");
}

#[test]
fn license_exception_rejects_empty_fields_bad_dates_and_expiry() {
    let mut bad = exception();
    bad.reason = String::new();
    assert_eq!(
        validate_license_exception(&bad, "2026-09-14"),
        Err(PolicyProblem::ExceptionMissingField { field: "reason" })
    );
    let mut bad = exception();
    bad.expires = "2027-02-29".to_owned();
    assert!(validate_license_exception(&bad, "2026-09-14").is_err());
    assert!(validate_license_exception(&exception(), "2027-03-01").is_err());
}

#[test]
fn license_exception_without_finding_is_obsolete() {
    assert!(is_obsolete(&exception(), &[]));
    assert!(check_applies(&exception(), &[]).is_err());
    assert!(!is_obsolete(&exception(), &[finding()]));
    let other_set = LicenseFinding {
        set: "npm".to_owned(),
        ..finding()
    };
    assert!(is_obsolete(&exception(), &[other_set]));
    let other_license = LicenseFinding {
        license: "MIT".to_owned(),
        ..finding()
    };
    assert!(is_obsolete(&exception(), &[other_license]));
}

#[test]
fn upgrade_within_range_retains_acceptance_at_identity_match() {
    let upgraded = LicenseFinding {
        version: "1.9.0".to_owned(),
        ..finding()
    };
    assert!(!is_obsolete(&exception(), &[upgraded.clone()]));
    check_applies(&exception(), &[upgraded.clone()]).expect("upgrade retains acceptance");
    assert!(license_exception_covers(&exception(), &upgraded));
}

#[test]
fn out_of_range_versions_do_not_inherit_acceptance() {
    let out_of_range = LicenseFinding {
        version: "2.0.0".to_owned(),
        ..finding()
    };
    assert!(!is_obsolete(&exception(), &[out_of_range.clone()]));
    check_applies(&exception(), &[out_of_range.clone()]).expect("identity still applies");
    assert!(!license_exception_covers(&exception(), &out_of_range));
    let bad_scope = LicenseException {
        versions: "not a range".to_owned(),
        ..exception()
    };
    assert!(!license_exception_covers(&bad_scope, &finding()));
    let bad_version = LicenseFinding {
        version: "banana".to_owned(),
        ..finding()
    };
    assert!(!license_exception_covers(&exception(), &bad_version));
}

#[test]
fn license_exceptions_narrow_per_set_like_vuln() {
    let cargo_exception = |versions: &str| LicenseException {
        package: "serde".to_owned(),
        set: "cargo".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: versions.to_owned(),
        reason: "Legal approved.".to_owned(),
        expires: "2027-03-01".to_owned(),
    };
    let cargo_finding = |version: &str| LicenseFinding {
        package: "serde".to_owned(),
        set: "cargo".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: version.to_owned(),
    };
    assert!(license_exception_covers(
        &cargo_exception(">=1.2.0, <2.0.0"),
        &cargo_finding("1.9.0")
    ));
    assert!(!license_exception_covers(
        &cargo_exception(">=1.2.0, <2.0.0"),
        &cargo_finding("2.0.0")
    ));
    assert!(license_exception_covers(
        &cargo_exception("*"),
        &cargo_finding("9.9.9")
    ));
    assert!(!license_exception_covers(
        &cargo_exception("1.0.0 || 2.0.0"),
        &cargo_finding("1.0.0")
    ));
    let npm_exception = |versions: &str| LicenseException {
        package: "react".to_owned(),
        set: "npm".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: versions.to_owned(),
        reason: "Legal approved.".to_owned(),
        expires: "2027-03-01".to_owned(),
    };
    let npm_finding = |version: &str| LicenseFinding {
        package: "react".to_owned(),
        set: "npm".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: version.to_owned(),
    };
    assert!(license_exception_covers(
        &npm_exception("1.2.7 || >=1.2.9 <2.0.0"),
        &npm_finding("1.2.9")
    ));
    assert!(!license_exception_covers(
        &npm_exception("1.2.7 || >=1.2.9 <2.0.0"),
        &npm_finding("1.2.8")
    ));
    assert!(license_exception_covers(
        &npm_exception("1.2.3 - 2.3.4"),
        &npm_finding("2.0.0")
    ));
    assert!(!license_exception_covers(
        &npm_exception("1.2.3 - 2.3.4"),
        &npm_finding("2.3.5")
    ));
    let go_exception = LicenseException {
        package: "example.com/mod".to_owned(),
        set: "go".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: ">=v1.0.0, <v2.0.0".to_owned(),
        reason: "Legal approved.".to_owned(),
        expires: "2027-03-01".to_owned(),
    };
    let go_covered = LicenseFinding {
        package: "example.com/mod".to_owned(),
        set: "go".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: "v1.5.0".to_owned(),
    };
    let go_missed = LicenseFinding {
        version: "v2.0.0".to_owned(),
        ..go_covered.clone()
    };
    assert!(license_exception_covers(&go_exception, &go_covered));
    assert!(!license_exception_covers(&go_exception, &go_missed));
    let maven_exception = |versions: &str| LicenseException {
        package: "junit:junit".to_owned(),
        set: "maven".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: versions.to_owned(),
        reason: "Legal approved.".to_owned(),
        expires: "2027-03-01".to_owned(),
    };
    let maven_finding = |version: &str| LicenseFinding {
        package: "junit:junit".to_owned(),
        set: "maven".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: version.to_owned(),
    };
    assert!(license_exception_covers(
        &maven_exception("[4.0,5.0)"),
        &maven_finding("4.13.2")
    ));
    assert!(!license_exception_covers(
        &maven_exception("[5.0,6.0)"),
        &maven_finding("4.13.2")
    ));
    assert!(license_exception_covers(
        &maven_exception("1.0"),
        &maven_finding("1.0.0")
    ));
    assert!(!license_exception_covers(
        &maven_exception(">=1.0.0"),
        &maven_finding("1.2.0")
    ));
    let nuget_exception = |versions: &str| LicenseException {
        package: "Newtonsoft.Json".to_owned(),
        set: "nuget".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        versions: versions.to_owned(),
        reason: "Legal approved.".to_owned(),
        expires: "2027-03-01".to_owned(),
    };
    let nuget_finding = |version: &str| LicenseFinding {
        package: "Newtonsoft.Json".to_owned(),
        set: "nuget".to_owned(),
        license: "GPL-3.0-only".to_owned(),
        version: version.to_owned(),
    };
    assert!(license_exception_covers(
        &nuget_exception("[12.0,13.0.2)"),
        &nuget_finding("13.0.1")
    ));
    assert!(!license_exception_covers(
        &nuget_exception("[13.0.2,14.0)"),
        &nuget_finding("13.0.1")
    ));
    assert!(license_exception_covers(
        &nuget_exception("1.0"),
        &nuget_finding("1.0.0")
    ));
    assert!(!license_exception_covers(
        &nuget_exception("1.*"),
        &nuget_finding("1.5.0")
    ));
    assert!(!license_exception_covers(
        &cargo_exception(">=1.0.0, <2.0.0"),
        &npm_finding("1.5.0")
    ));
}

fn doc_example() -> &'static str {
    r#"
[policy]
blocked = ["AGPL-3.0-only", "AGPL-3.0-or-later", "SSPL-1.0"]

[policy.distributed]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"]
review = ["MPL-2.0", "EPL-2.0"]
deny = ["GPL-3.0-only", "GPL-3.0-or-later"]

[policy.sets.npm-root]
review = ["Unicode-3.0"]

[distribution]
internal = ["//tools/internal-admin:binary"]

[[exception]]
package = "some-copyleft-lib"
set = "cargo"
license = "GPL-3.0-only"
versions = ">=1.2.0, <2.0.0"
reason = "Legal approved for internal fork."
expires = "2027-03-01"
"#
}

#[test]
fn loader_converts_doc_shape_to_domain_records() {
    let policy = load_licenses_toml(doc_example()).expect("doc example loads");
    assert_eq!(
        policy.tables.blocked,
        ["AGPL-3.0-only", "AGPL-3.0-or-later", "SSPL-1.0"]
            .iter()
            .map(|item| (*item).to_owned())
            .collect()
    );
    assert!(policy.tables.allow.contains("MIT"));
    assert!(policy.tables.review.contains("MPL-2.0"));
    assert!(policy.tables.deny.contains("GPL-3.0-only"));
    assert_eq!(
        policy.sets,
        vec![SetAdjustment {
            set: "npm-root".to_owned(),
            review: ["Unicode-3.0"]
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
        }]
    );
    assert_eq!(
        policy.distribution.tier_of("//tools/internal-admin:binary"),
        Tier::Internal
    );
    assert_eq!(policy.distribution.tier_of("//cli:dx"), Tier::Distributed);
    assert_eq!(
        policy.distribution.tier_of("//unlisted:thing"),
        Tier::Distributed
    );
    assert_eq!(
        policy.exceptions,
        vec![LicenseException {
            package: "some-copyleft-lib".to_owned(),
            set: "cargo".to_owned(),
            license: "GPL-3.0-only".to_owned(),
            versions: ">=1.2.0, <2.0.0".to_owned(),
            reason: "Legal approved for internal fork.".to_owned(),
            expires: "2027-03-01".to_owned(),
        }]
    );
}

#[test]
fn loader_defaults_missing_sections_to_empty_and_stays_closed() {
    let policy = load_licenses_toml("").expect("empty document loads");
    assert_eq!(
        policy,
        LicensePolicy {
            tables: PolicyTables::default(),
            sets: Vec::new(),
            distribution: Distribution::default(),
            exceptions: Vec::new(),
            inventory: Vec::new(),
        }
    );
    assert_eq!(policy.distribution.tier_of("//cli:dx"), Tier::Distributed);
}

#[test]
fn loader_rejects_unknown_fields_typos_and_malformed_toml() {
    for bad in [
        "[policy.distributed]\nalow = [\"MIT\"]\n",
        "[bogus]\nkey = 1\n",
        "[[exception]]\npackage = \"p\"\nset = \"s\"\nlicense = \"MIT\"\n\
         versions = \"*\"\nreason = \"r\"\nexpires = \"2027-03-01\"\nnote = \"x\"\n",
        "[[exception]\n",
        "[policy.distributed]\nallow = [\"MIT\"]\nallow = [\"ISC\"]\n",
        "[distribution]\ndistributed = [\"//cli:dx\"]\n",
    ] {
        assert!(
            matches!(
                load_licenses_toml(bad),
                Err(PolicyProblem::InvalidLicensesToml { .. })
            ),
            "{bad:?} must fail as invalid TOML"
        );
    }
}

#[test]
fn loader_validates_tables_and_set_adjustments() {
    let multi = "[policy.distributed]\nallow = [\"MIT\"]\ndeny = [\"MIT\"]\n";
    assert_eq!(
        load_licenses_toml(multi),
        Err(PolicyProblem::MultiListed {
            identity: "MIT".to_owned()
        })
    );
    let conflict = "[policy.distributed]\nallow = [\"MIT\"]\n\
                    [policy.sets.npm-root]\nreview = [\"MIT\"]\n";
    assert_eq!(
        load_licenses_toml(conflict),
        Err(PolicyProblem::SetConflict {
            set: "npm-root".to_owned(),
            identity: "MIT".to_owned(),
        })
    );
}

#[test]
fn loaded_exceptions_validate_against_audit_date() {
    let policy = load_licenses_toml(doc_example()).expect("doc example loads");
    validate_license_exception(&policy.exceptions[0], "2026-09-14").expect("valid");
    assert!(validate_license_exception(&policy.exceptions[0], "2027-03-01").is_err());
}

#[test]
fn schema_version_accepts_v1_and_rejects_other_versions() {
    assert_eq!(LICENSE_POLICY_SCHEMA_VERSION, 1);
    load_licenses_toml("").expect("empty document loads as v1");
    load_licenses_toml("schema_version = 1\n").expect("explicit v1 loads");
    for bad in [
        "schema_version = 0\n",
        "schema_version = 2\n",
        "schema_version = 999\n",
    ] {
        assert!(
            matches!(
                load_licenses_toml(bad),
                Err(PolicyProblem::InvalidLicensesToml { .. })
            ),
            "{bad:?} must fail as unsupported schema version"
        );
    }
}

#[test]
fn license_additions_need_no_struct_edits() {
    let policy =
        load_licenses_toml("[policy.distributed]\nallow = [\"MIT\", \"New-Permissive-1.0\"]\n")
            .expect("new allow identity loads");
    assert!(policy.tables.allow.contains("New-Permissive-1.0"));
    policy.tables.validate().expect("single listing passes");
}

#[test]
fn loader_converts_inventory_entries_per_ecosystem() {
    let text = r#"
[[inventory]]
package = "react"
set = "npm"
license = "MIT"
versions = "18.2.0"
text_present = true

[[inventory]]
package = "junit:junit"
set = "maven"
license = "EPL-1.0"
versions = "4.13.2"

[[inventory]]
package = "FSharp.Core"
set = "nuget"
license = "MIT"
versions = "10.1.201"
text_present = true

[[inventory]]
package = "github.com/google/go-cmp"
set = "go"
license = "BSD-3-Clause"
versions = "v0.6.0"
text_present = false

[[inventory]]
package = "serde"
set = "cargo"
license = "MIT OR Apache-2.0"
versions = "1.0.100"
text_present = true
"#;
    let policy = load_licenses_toml(text).expect("inventory loads");
    assert_eq!(policy.inventory.len(), 5);
    let npm = policy
        .inventory
        .iter()
        .find(|entry| entry.set == "npm")
        .expect("npm entry");
    assert_eq!(npm.package, "react");
    assert_eq!(npm.license, "MIT");
    assert!(npm.text_present);
    let maven = policy
        .inventory
        .iter()
        .find(|entry| entry.set == "maven")
        .expect("maven entry");
    assert_eq!(maven.package, "junit:junit");
    assert!(!maven.text_present);
    validate_license_inventory(npm).expect("valid entry passes");
}

#[test]
fn loader_rejects_empty_inventory_fields_and_unknown_keys() {
    for bad in [
        "[[inventory]]\npackage = \"\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"1.0.0\"\n",
        "[[inventory]]\npackage = \"react\"\nset = \"\"\nlicense = \"MIT\"\nversions = \"1.0.0\"\n",
        "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"\"\nversions = \"1.0.0\"\n",
        "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"\"\n",
    ] {
        assert!(
            matches!(
                load_licenses_toml(bad),
                Err(PolicyProblem::ExceptionMissingField { .. })
            ),
            "{bad:?} must fail on empty inventory field"
        );
    }
    let typo = "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"1.0.0\"\nlicence = \"x\"\n";
    assert!(
        matches!(
            load_licenses_toml(typo),
            Err(PolicyProblem::InvalidLicensesToml { .. })
        ),
        "typo'd inventory key must fail as invalid TOML"
    );
}
