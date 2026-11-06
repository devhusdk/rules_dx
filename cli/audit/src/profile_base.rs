#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Standard,
    Strict,
}

impl Profile {
    fn name(self) -> &'static str {
        match self {
            Profile::Standard => "standard",
            Profile::Strict => "strict",
        }
    }
}

/// Workspace-relative path of the base file `dx init --apply` writes.
pub fn base_rel(file: &str, profile: Profile) -> String {
    format!(".dx/config/{file}/{}.toml", profile.name())
}

/// The base text that ships inside dx_audit; `None` for a file without bases.
pub fn base_text(file: &str, profile: Profile) -> Option<&'static str> {
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

    const COMMITTED: [(&str, Profile, &str); 4] = [
        (
            "licenses",
            Profile::Standard,
            include_str!("../../../.dx/config/licenses/standard.toml"),
        ),
        (
            "licenses",
            Profile::Strict,
            include_str!("../../../.dx/config/licenses/strict.toml"),
        ),
        (
            "security",
            Profile::Standard,
            include_str!("../../../.dx/config/security/standard.toml"),
        ),
        (
            "security",
            Profile::Strict,
            include_str!("../../../.dx/config/security/strict.toml"),
        ),
    ];

    #[test]
    fn committed_bases_match_the_embedded_ones() {
        for (file, profile, committed) in COMMITTED {
            let (header, body) = committed.split_once('\n').expect("header line");
            assert!(header.starts_with("# dx: generated"), "{file} {profile:?}");
            assert_eq!(
                Some(body),
                base_text(file, profile),
                "{} differs; run dx init --apply",
                base_rel(file, profile)
            );
        }
    }
}
