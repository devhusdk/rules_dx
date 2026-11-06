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
