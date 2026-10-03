pub const CURATOR_ADVISORY_SETS: &[&str] = &["cargo", "npm", "maven", "nuget", "go", "rubygems"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_sets_all_have_a_source_and_lock_coverage() {
        assert_eq!(
            CURATOR_ADVISORY_SETS,
            &["cargo", "npm", "maven", "nuget", "go", "rubygems"]
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
