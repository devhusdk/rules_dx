pub use dx_config::Profile;

/// The base text that ships inside dx_audit; `None` for a file without bases.
pub(crate) fn base_text(file: &str, profile: Profile) -> Option<&'static str> {
    match (file, profile) {
        ("licenses", Profile::Standard) => Some(include_str!("../profiles/licenses/standard.toml")),
        ("licenses", Profile::Strict) => Some(include_str!("../profiles/licenses/strict.toml")),
        ("security", Profile::Standard) => Some(include_str!("../profiles/security/standard.toml")),
        ("security", Profile::Strict) => Some(include_str!("../profiles/security/strict.toml")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_parses_and_has_no_entries() {
        for profile in [Profile::Standard, Profile::Strict] {
            let base = base_text("security", profile).expect("security base");
            let security = crate::exception::load_security(Some(base), profile).expect("parses");
            assert!(security.exceptions.is_empty() && security.uncovered.is_empty());
            let base = base_text("licenses", profile).expect("licenses base");
            let licenses =
                crate::license_policy::load_licenses(Some(base), profile).expect("parses");
            assert!(licenses.exceptions.is_empty() && licenses.inventory.is_empty());
        }
    }
}
