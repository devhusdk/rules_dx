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
        assert!(Ecosystem::parse(&row.name).is_some(), "{} unparsed", row.name);
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
