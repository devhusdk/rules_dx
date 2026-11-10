use std::collections::BTreeSet;

use super::*;

#[test]
fn shipped_table_has_every_ecosystem() {
    assert_eq!(rows().len(), 24);
    assert_eq!(rows().len(), Ecosystem::ALL.len());
}

#[test]
fn rows_are_complete_and_unique() {
    let mut seen = BTreeSet::new();
    for row in rows() {
        assert!(!row.name.is_empty());
        assert!(!row.manifest.is_empty(), "{} has no manifest", row.name);
        assert!(seen.insert(row.name.as_str()), "duplicate {}", row.name);
        assert!(
            Ecosystem::parse(&row.name).is_some(),
            "{} unparsed",
            row.name
        );
    }
}

#[test]
fn every_variant_has_a_row() {
    for ecosystem in Ecosystem::ALL {
        assert!(ecosystem.row().is_some(), "{} has no row", ecosystem.name());
        assert_eq!(Ecosystem::parse(ecosystem.name()), Some(ecosystem));
    }
}

#[test]
fn supported_list_matches_rows() {
    let names: Vec<&str> = rows().iter().map(|row| row.name.as_str()).collect();
    assert_eq!(SUPPORTED_ECOSYSTEMS, names.join(", "));
}

#[test]
fn missing_field_is_rejected() {
    let text = r#"{"ecosystems":[{"name":"uv","manifest":"pyproject.toml"}]}"#;
    assert!(parse_rows(text).is_err());
}

#[test]
fn false_flags_report_pinned() {
    assert_eq!(Ecosystem::Bazel.supported_check(), "pinned");
    assert_eq!(Ecosystem::Uv.supported_check(), "check");
    assert_eq!(Ecosystem::Dub.advisory_family(), None);
}

#[test]
fn lock_and_source_columns_match_the_audit_column() {
    for row in rows() {
        assert!(
            row.lockfiles
                .iter()
                .all(|name| !name.is_empty() && !name.contains('/'))
        );
        if row.advisory_family.is_none() {
            assert_eq!(
                row.osv_source, None,
                "{} has a source but no family",
                row.name
            );
        }
        if let Some(url) = &row.osv_source {
            assert!(url.starts_with("https://"), "{} source {url}", row.name);
        }
        if row.audit && row.advisory_family.is_some() {
            assert!(
                row.osv_source.is_some(),
                "{} audited without source",
                row.name
            );
        }
    }
}

#[test]
fn lookups_resolve_rows() {
    assert_eq!(Ecosystem::Hex.lockfiles(), ["mix.lock", "rebar.lock"]);
    assert_eq!(Ecosystem::Npm.lockfiles()[0], "pnpm-lock.yaml");
    assert_eq!(advisory_family_named("rubygems"), Some("rubygems"));
    assert_eq!(advisory_family_named("ruby"), None);
    assert_eq!(advisory_family_named("unknown-set"), None);
    assert_eq!(
        osv_source_of_family("cargo"),
        Some("https://osv-vulnerabilities.storage.googleapis.com/crates.io/all.zip")
    );
    assert_eq!(osv_source_of_family("nope"), None);
}
