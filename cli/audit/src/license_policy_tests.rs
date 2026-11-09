use super::*;
use crate::license_expr::{IdClass, LicenseExpr, TierOutcome, evaluate};
use crate::profile_base::Profile;

/// Loads `body` as a standard-profile license.toml whose line 1 is `schema_version = 1`.
fn load(body: &str) -> Result<LicensePolicy, PolicyProblem> {
    load_licenses(
        Some(&format!("schema_version = 1\n{body}")),
        Profile::Standard,
    )
}

fn standard() -> LicensePolicy {
    load_licenses(None, Profile::Standard).expect("standard")
}

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
        line: 0,
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
fn internal_entries_match_as_label_patterns() {
    let tier = |entry: &str, root: &str| {
        Distribution {
            internal: [entry].iter().map(|item| (*item).to_owned()).collect(),
        }
        .tier_of(root)
    };
    for root in [
        "//...",
        "//cli/...",
        "//cli/cli:dx_cli",
        "//:root",
        "@//cli:x",
        "@@//cli/...",
    ] {
        assert_eq!(tier("//...", root), Tier::Internal, "{root}");
    }
    assert_eq!(tier("//...", "@other//a:b"), Tier::Distributed);
    for root in ["//cli:x", "//cli/...", "//cli/cli/...", "//cli/cli:dx"] {
        assert_eq!(tier("//cli/...", root), Tier::Internal, "{root}");
    }
    for root in ["//client:x", "//...", "//other/..."] {
        assert_eq!(tier("//cli/...", root), Tier::Distributed, "{root}");
    }
    for root in ["//cli:x", "//cli:all"] {
        assert_eq!(tier("//cli:all", root), Tier::Internal, "{root}");
    }
    for root in ["//cli/...", "//cli/sub:x"] {
        assert_eq!(tier("//cli:all", root), Tier::Distributed, "{root}");
    }
    assert_eq!(tier("//cli:dx", "//cli:dx"), Tier::Internal);
    assert_eq!(tier("//cli:dx", "//cli:other"), Tier::Distributed);
    assert_eq!(tier("@repo//...", "@repo//a:b"), Tier::Internal);
    assert_eq!(tier("@repo//...", "//a:b"), Tier::Distributed);
    assert_eq!(tier("//...", "cargo"), Tier::Distributed);
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
    assert!(!is_obsolete(&exception(), std::slice::from_ref(&upgraded)));
    check_applies(&exception(), std::slice::from_ref(&upgraded))
        .expect("upgrade retains acceptance");
    assert!(license_exception_covers(&exception(), &upgraded));
}

#[test]
fn out_of_range_versions_do_not_inherit_acceptance() {
    let out_of_range = LicenseFinding {
        version: "2.0.0".to_owned(),
        ..finding()
    };
    assert!(!is_obsolete(
        &exception(),
        std::slice::from_ref(&out_of_range)
    ));
    check_applies(&exception(), std::slice::from_ref(&out_of_range))
        .expect("identity still applies");
    assert!(!license_exception_covers(&exception(), &out_of_range));
    let bad_scope = LicenseException {
        line: 0,
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
        line: 0,
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
        line: 0,
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
        line: 0,
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
        line: 0,
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
        line: 0,
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
    let policy = load(doc_example()).expect("doc example loads");
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
            line: 17,
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
fn loader_defaults_missing_sections_to_the_base_and_stays_closed() {
    let policy = load("").expect("empty document loads");
    assert_eq!(policy, standard());
    assert_eq!(
        load_licenses(Some(" \n"), Profile::Standard),
        Ok(standard())
    );
    assert!(policy.tables.allow.contains("MIT"));
    assert!(policy.sets.is_empty() && policy.exceptions.is_empty() && policy.inventory.is_empty());
    assert_eq!(policy.distribution, Distribution::default());
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
            matches!(load(bad), Err(PolicyProblem::InvalidLicensesToml { .. })),
            "{bad:?} must fail as invalid TOML"
        );
    }
}

#[test]
fn loader_validates_tables_and_set_adjustments() {
    let multi = "[policy.distributed]\nallow = [\"MIT\"]\ndeny = [\"MIT\"]\n";
    assert_eq!(
        load(multi),
        Err(PolicyProblem::MultiListed {
            identity: "MIT".to_owned()
        })
    );
    let conflict = "[policy.distributed]\nallow = [\"MIT\"]\n\
                    [policy.sets.npm-root]\nreview = [\"MIT\"]\n";
    assert_eq!(
        load(conflict),
        Err(PolicyProblem::SetConflict {
            set: "npm-root".to_owned(),
            identity: "MIT".to_owned(),
        })
    );
}

#[test]
fn loaded_exceptions_validate_against_audit_date() {
    let policy = load(doc_example()).expect("doc example loads");
    validate_license_exception(&policy.exceptions[0], "2026-09-14").expect("valid");
    assert!(validate_license_exception(&policy.exceptions[0], "2027-03-01").is_err());
}

#[test]
fn schema_version_is_required_and_must_be_current() {
    let read = |text: &str| load_licenses(Some(text), Profile::Standard);
    read("schema_version = 1\n").expect("v1 loads");
    for (bad, want) in [
        ("[policy]\nblocked = []\n", "schema_version is missing"),
        ("schema_version = 0\n", "no longer read"),
        ("schema_version = 2\n", "newer than this dx reads"),
        ("schema_version = 999\n", "newer than this dx reads"),
    ] {
        let error = read(bad).expect_err(bad).to_string();
        assert!(error.contains(want), "{bad:?}: {error}");
    }
}

#[test]
fn extends_is_an_unknown_key() {
    let error = load("extends = \"base.toml\"\n")
        .expect_err("extends")
        .to_string();
    assert!(error.contains("unknown field `extends`"), "{error}");
}

#[test]
fn license_additions_need_no_struct_edits() {
    let policy = load("[policy.distributed]\nallow = [\"MIT\", \"New-Permissive-1.0\"]\n")
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
    let policy = load(text).expect("inventory loads");
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
            matches!(load(bad), Err(PolicyProblem::ExceptionMissingField { .. })),
            "{bad:?} must fail on empty inventory field"
        );
    }
    let typo = "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"1.0.0\"\nlicence = \"x\"\n";
    assert!(
        matches!(load(typo), Err(PolicyProblem::InvalidLicensesToml { .. })),
        "typo'd inventory key must fail as invalid TOML"
    );
}

#[test]
fn inventory_text_parses_validates_and_implies_present() {
    let ok = "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"18.2.0\"\ntext = \"vendor/react/LICENSE\"\n";
    let policy = load(ok).expect("text loads");
    assert_eq!(
        policy.inventory[0].text.as_deref(),
        Some("vendor/react/LICENSE")
    );
    let pkg =
        crate::vuln::LockedPackage::new("npm".to_owned(), "react".to_owned(), "18.2.0".to_owned());
    assert!(crate::locks::inventory_text_present(
        &pkg,
        &policy.inventory
    ));
    for bad in ["../x", "/abs", "a\\\\b", "a//b", "a/"] {
        let text = format!(
            "[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"18.2.0\"\ntext = \"{bad}\"\n"
        );
        assert!(
            matches!(load(&text), Err(PolicyProblem::InventoryText { .. })),
            "{bad:?} must be rejected"
        );
    }
}

#[test]
fn bad_license_exceptions_keep_their_lines() {
    let text = "\
[[exception]]
package = \"alpha\"
set = \"cargo\"
license = \"MIT\"
versions = \"*\"
reason = \"\"
expires = \"2999-01-01\"

[[exception]]
package = \"beta\"
set = \"npm\"
license = \"ISC\"
versions = \"*\"
reason = \"ok\"
expires = \"2000-01-01\"
";
    let policy = load(text).expect("loads");
    let found: Vec<(usize, String, bool)> = policy
        .exceptions
        .iter()
        .map(|entry| {
            (
                entry.line,
                entry.entry_label(),
                validate_license_exception(entry, "2026-01-01").is_err(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            (2, "MIT for alpha in cargo".to_owned(), true),
            (10, "ISC for beta in npm".to_owned(), true),
        ]
    );
}

#[test]
fn a_root_list_replaces_that_list_and_inherits_the_rest() {
    let policy = load("[policy.distributed]\nreview = [\"Zlib\"]\n").expect("merged");
    let standard = standard();
    assert_eq!(policy.tables.review, ["Zlib".to_owned()].into());
    assert_eq!(policy.tables.allow, standard.tables.allow);
    assert_eq!(policy.tables.deny, standard.tables.deny);
    assert_eq!(policy.tables.blocked, standard.tables.blocked);
}

#[test]
fn the_strict_profile_denies_what_the_standard_profile_only_reviews() {
    let standard = standard();
    let strict = load_licenses(None, Profile::Strict).expect("strict");
    for id in ["LGPL-3.0-only", "MPL-2.0", "EPL-2.0", "CDDL-1.0"] {
        assert!(standard.tables.review.contains(id));
        assert!(strict.tables.deny.contains(id));
    }
    assert!(strict.tables.review.is_empty());
    let root = "schema_version = 1\n[distribution]\ninternal = [\"//a/...\"]\n";
    let strict_root = load_licenses(Some(root), Profile::Strict).expect("strict root");
    assert_eq!(strict_root.tables, strict.tables);
    assert_eq!(
        strict_root.distribution.internal,
        ["//a/...".to_owned()].into()
    );
}

#[test]
fn merged_tables_are_validated_and_exceptions_keep_root_lines() {
    let clash = "[policy.distributed]\nallow = [\"MPL-2.0\"]\n";
    assert!(matches!(
        load(clash),
        Err(PolicyProblem::MultiListed { .. })
    ));
    let root = "\n[[exception]]\npackage = \"a\"\nset = \"cargo\"\nlicense = \"MPL-2.0\"\nversions = \"*\"\nreason = \"r\"\nexpires = \"2999-01-01\"\n";
    let policy = load(root).expect("merged");
    assert_eq!(policy.exceptions[0].line, 3);
}

#[test]
fn set_overrides_and_the_distribution_come_from_the_root() {
    let root =
        "[policy.sets.cargo]\nreview = [\"BSL-1.0\"]\n[distribution]\ninternal = [\"//b/...\"]\n";
    let policy = load(root).expect("merged");
    assert_eq!(
        policy.sets,
        vec![SetAdjustment {
            set: "cargo".to_owned(),
            review: ["BSL-1.0".to_owned()].into(),
        }]
    );
    assert_eq!(policy.distribution.internal, ["//b/...".to_owned()].into());
}

#[test]
fn internal_entries_must_be_label_patterns() {
    let error = load("[distribution]\ninternal = [\"cli/...\"]\n")
        .expect_err("not a label")
        .to_string();
    assert!(
        error.contains("invalid label pattern \"cli/...\""),
        "{error}"
    );
}
