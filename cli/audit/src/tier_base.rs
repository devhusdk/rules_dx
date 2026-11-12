pub use dx_config::Tier;

/// The base text that ships inside dx_audit; `None` for a file without bases.
pub(crate) fn base_text(file: &str, tier: Tier) -> Option<&'static str> {
    match (file, tier) {
        ("licenses", Tier::Standard) => Some(include_str!("../tiers/licenses/standard.toml")),
        ("licenses", Tier::Strict) => Some(include_str!("../tiers/licenses/strict.toml")),
        ("security", Tier::Standard) => Some(include_str!("../tiers/security/standard.toml")),
        ("security", Tier::Strict) => Some(include_str!("../tiers/security/strict.toml")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_parses_and_has_no_entries() {
        for tier in [Tier::Standard, Tier::Strict] {
            let base = base_text("security", tier).expect("security base");
            let security = crate::exception::load_security(Some(base), tier).expect("parses");
            assert!(security.exceptions.is_empty() && security.uncovered.is_empty());
            let base = base_text("licenses", tier).expect("licenses base");
            let licenses =
                crate::license_policy::load_licenses(Some(base), tier).expect("parses");
            assert!(licenses.exceptions.is_empty() && licenses.inventory.is_empty());
        }
    }
}
