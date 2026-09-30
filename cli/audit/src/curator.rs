pub const LICENSES_TOML_LABEL: &str = "//:audit_curator";

pub const LICENSES_TOML_REL: &str = "licenses.toml";

pub const CURATOR_ADVISORY_SETS: &[&str] = &["cargo", "npm", "maven", "nuget", "go"];

pub fn advisory_inputs(set: &str) -> [String; 2] {
    [
        crate::advisory::snapshot_rel(set),
        crate::advisory::identity_rel(set),
    ]
}

pub fn curator_input_rels() -> Vec<String> {
    let mut out = vec![LICENSES_TOML_REL.to_owned()];
    for set in CURATOR_ADVISORY_SETS {
        out.extend(advisory_inputs(set));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curator_label_and_rel_are_pinned() {
        assert_eq!(LICENSES_TOML_LABEL, "//:audit_curator");
        assert_eq!(LICENSES_TOML_REL, "licenses.toml");
    }

    #[test]
    fn advisory_sets_all_have_a_source_and_lock_coverage() {
        assert_eq!(
            CURATOR_ADVISORY_SETS,
            &["cargo", "npm", "maven", "nuget", "go"]
        );
        for set in CURATOR_ADVISORY_SETS {
            assert!(
                crate::advisory::advisory_source(set).is_some(),
                "{set} needs an advisory source"
            );
            assert!(
                !crate::backend::vuln_locks(set).is_empty(),
                "{set} needs lock coverage"
            );
        }
    }

    #[test]
    fn advisory_inputs_name_snapshot_plus_identity() {
        assert_eq!(
            advisory_inputs("cargo"),
            [
                ".dx/advisory/cargo.json".to_owned(),
                ".dx/advisory/cargo.meta.json".to_owned(),
            ]
        );
        assert_eq!(
            advisory_inputs("go"),
            [
                ".dx/advisory/go.json".to_owned(),
                ".dx/advisory/go.meta.json".to_owned(),
            ]
        );
    }

    #[test]
    fn curator_rels_are_deterministic_and_complete() {
        let rels = curator_input_rels();
        assert_eq!(rels.first().map(String::as_str), Some("licenses.toml"));
        assert_eq!(rels.len(), 1 + CURATOR_ADVISORY_SETS.len() * 2);
        let mut want = vec!["licenses.toml".to_owned()];
        for set in CURATOR_ADVISORY_SETS {
            want.push(format!(".dx/advisory/{set}.json"));
            want.push(format!(".dx/advisory/{set}.meta.json"));
        }
        assert_eq!(rels, want);
    }

    #[test]
    fn inventory_shape_stays_per_package_for_notice_aggregation() {
        let entry = crate::license_policy::LicenseInventory {
            package: "react".to_owned(),
            set: "npm".to_owned(),
            license: "MIT".to_owned(),
            versions: "18.2.0".to_owned(),
            text_present: true,
        };
        crate::license_policy::validate_license_inventory(&entry).expect("valid");
        assert!(entry.text_present);
    }
}
