use super::AdoptError;

pub fn migrate_is_major_bump(from: &str, to: &str) -> bool {
    let from_v = match semver::Version::parse(from) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let to_v = match semver::Version::parse(to) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if from_v == to_v {
        return false;
    }
    to_v.major > from_v.major
}

pub fn migrate_is_upgrade(from: &str, to: &str) -> bool {
    let from_v = match semver::Version::parse(from) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let to_v = match semver::Version::parse(to) {
        Ok(v) => v,
        Err(_) => return false,
    };
    to_v > from_v
}

pub fn migrate_manifest_name(from_major: u64, to_major: u64) -> String {
    format!("migrate-v{from_major}-to-v{to_major}.json")
}

pub fn migrate_manifest_name_full(from: &str, to: &str) -> String {
    format!("migrate-v{from}-to-v{to}.json")
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigratePlan {
    pub from: String,
    pub to: String,
    pub manifest: String,
}

pub fn plan_migrate(from: &str, to: &str) -> Result<MigratePlan, AdoptError> {
    if from.is_empty() || to.is_empty() {
        return Err(AdoptError::MigrateVersions {
            detail: "from and to must both be set".to_owned(),
        });
    }
    let from_v = semver::Version::parse(from).map_err(|_| AdoptError::MigrateVersions {
        detail: format!("invalid from version: {from}"),
    })?;
    let to_v = semver::Version::parse(to).map_err(|_| AdoptError::MigrateVersions {
        detail: format!("invalid to version: {to}"),
    })?;
    if !migrate_is_upgrade(from, to) {
        return Err(AdoptError::MigrateNotUpgrade {
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }
    let manifest = if to_v.major > from_v.major {
        migrate_manifest_name(from_v.major, to_v.major)
    } else {
        migrate_manifest_name_full(from, to)
    };
    Ok(MigratePlan {
        from: from.to_owned(),
        to: to.to_owned(),
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_upgrade_gate_accepts_any_upgrade() {
        assert!(migrate_is_upgrade("1.2.3", "2.0.0"));
        assert!(migrate_is_upgrade("1.2.3", "1.3.0"));
        assert!(migrate_is_upgrade("1.2.3", "1.2.4"));
        assert!(migrate_is_upgrade("1.9.9", "2.0.0-alpha.1"));
        assert!(migrate_is_upgrade("0.0.0", "1.0.0"));
        assert!(migrate_is_upgrade("0.0.0", "0.1.0"));
        assert!(migrate_is_upgrade("1.0.0-alpha", "1.0.0"));
        assert!(!migrate_is_upgrade("2.0.0", "1.0.0"));
        assert!(!migrate_is_upgrade("1.2.3", "1.2.3"));
        assert!(!migrate_is_upgrade("abc", "2.0.0"));
        assert!(!migrate_is_upgrade("1.2.3", ""));
        assert!(migrate_is_major_bump("1.2.3", "2.0.0"));
        assert!(migrate_is_major_bump("1.9.9", "2.0.0-alpha.1"));
        assert!(migrate_is_major_bump("0.0.0", "1.0.0"));
        assert!(!migrate_is_major_bump("1.2.3", "1.3.0"));
        assert!(!migrate_is_major_bump("1.2.3", "1.2.4"));
        assert_eq!(migrate_manifest_name(1, 2), "migrate-v1-to-v2.json");
        assert_eq!(
            migrate_manifest_name_full("1.2.3", "1.3.0"),
            "migrate-v1.2.3-to-v1.3.0.json"
        );
        let plan = plan_migrate("1.2.3", "2.0.0").expect("major bump plans");
        assert_eq!(plan.manifest, "migrate-v1-to-v2.json");
        let minor = plan_migrate("1.2.3", "1.3.0").expect("minor plans");
        assert_eq!(minor.manifest, "migrate-v1.2.3-to-v1.3.0.json");
        let patch = plan_migrate("1.2.3", "1.2.4").expect("patch plans");
        assert_eq!(patch.manifest, "migrate-v1.2.3-to-v1.2.4.json");
        assert!(plan_migrate("2.0.0", "1.0.0").is_err());
        assert!(plan_migrate("", "2.0.0").is_err());
        assert!(plan_migrate("abc", "2.0.0").is_err());
        assert_eq!(
            plan_migrate("2.0.0", "1.0.0").unwrap_err().to_string(),
            "migrate is upgrade-only: 2.0.0 -> 1.0.0"
        );
    }

    #[test]
    fn migrate_manifest_selection_is_mechanical_per_major_hop() {
        assert_eq!(migrate_manifest_name(0, 1), "migrate-v0-to-v1.json");
        assert_eq!(migrate_manifest_name(1, 2), "migrate-v1-to-v2.json");
        assert_eq!(migrate_manifest_name(2, 3), "migrate-v2-to-v3.json");
        assert_eq!(migrate_manifest_name(1, 3), "migrate-v1-to-v3.json");
        assert_eq!(migrate_manifest_name(10, 11), "migrate-v10-to-v11.json");
        let plan = plan_migrate("1.2.3", "2.0.0").expect("plan");
        assert_eq!(plan.from, "1.2.3");
        assert_eq!(plan.to, "2.0.0");
        assert_eq!(plan.manifest, "migrate-v1-to-v2.json");
        let hop = plan_migrate("0.9.0", "1.0.0").expect("0 to 1 plans");
        assert_eq!(hop.manifest, "migrate-v0-to-v1.json");
        let skip = plan_migrate("1.0.0", "3.0.0").expect("multi-hop plans");
        assert_eq!(skip.manifest, "migrate-v1-to-v3.json");
    }

    #[test]
    fn migrate_syntax_pins_semver_prerelease_and_errors() {
        assert!(migrate_is_major_bump("1.0.0", "2.0.0-alpha.1"));
        assert!(migrate_is_major_bump("1.0.0+build.1", "2.0.0"));
        assert!(migrate_is_major_bump("1.0.0-alpha", "2.0.0"));
        assert!(!migrate_is_major_bump("1.0.0-alpha", "1.0.0"));
        assert!(!migrate_is_major_bump("1.0.0+build.1", "1.0.1"));
        assert!(!migrate_is_major_bump("2.0.0-alpha.1", "1.9.9"));
        assert!(!migrate_is_major_bump("1.0", "2.0.0"));
        assert!(!migrate_is_major_bump("1.0.0", "2.0"));
        assert_eq!(
            plan_migrate("", "2.0.0").unwrap_err().to_string(),
            "migrate needs distinct versions: from and to must both be set"
        );
        assert_eq!(
            plan_migrate("1.2.3", "").unwrap_err().to_string(),
            "migrate needs distinct versions: from and to must both be set"
        );
        assert_eq!(
            plan_migrate("abc", "2.0.0").unwrap_err().to_string(),
            "migrate needs distinct versions: invalid from version: abc"
        );
        assert_eq!(
            plan_migrate("1.2.3", "xyz").unwrap_err().to_string(),
            "migrate needs distinct versions: invalid to version: xyz"
        );
        assert_eq!(
            plan_migrate("2.0.0", "1.0.0").unwrap_err().to_string(),
            "migrate is upgrade-only: 2.0.0 -> 1.0.0"
        );
        assert_eq!(
            plan_migrate("1.2.3", "1.2.3").unwrap_err().to_string(),
            "migrate is upgrade-only: 1.2.3 -> 1.2.3"
        );
    }

    #[test]
    fn migrate_prerelease_and_build_metadata_table() {
        assert!(migrate_is_upgrade("1.9.9", "2.0.0-alpha.1"));
        assert!(migrate_is_upgrade("1.0.0-alpha", "1.0.0"));
        assert!(migrate_is_upgrade("1.0.0+build.1", "1.0.1"));
        assert!(migrate_is_upgrade("1.0.0+build.1", "2.0.0"));
        assert!(migrate_is_upgrade("1.0.0+build.1", "1.0.0+build.2"));
        assert!(migrate_is_upgrade("1.0.0", "1.0.0+build.1"));
        assert!(!migrate_is_upgrade("2.0.0-alpha.1", "1.9.9"));
        let skip = plan_migrate("1.0.0", "3.0.0").expect("multi-hop plans single");
        assert_eq!(skip.manifest, "migrate-v1-to-v3.json");
    }
}
