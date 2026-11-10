use std::path::Path;

use dx_ecosystems::Language;

#[derive(Debug, thiserror::Error)]
pub enum DepcheckError {
    #[error("unknown ecosystem: {0}")]
    UnknownEcosystem(String),
    #[error("unreadable manifest: {0}")]
    ManifestIo(#[source] std::io::Error),
    #[error("unreadable manifest: {0}")]
    ManifestToml(#[source] toml::de::Error),
    #[error("unreadable manifest: {0}")]
    ManifestJson(#[source] serde_json::Error),
    #[error("unreadable manifest: {0}")]
    ManifestRegex(#[source] regex::Error),
    #[error("unreadable manifest: {0}")]
    ManifestParse(String),
    #[error("unreadable lock: {0}")]
    LockIo(#[source] std::io::Error),
    #[error("unreadable lock: {0}")]
    LockToml(#[source] toml::de::Error),
    #[error("unreadable lock: {0}")]
    LockJson(#[source] serde_json::Error),
    #[error("unreadable lock: {0}")]
    LockRegex(#[source] regex::Error),
    #[error("unreadable lock: {0}")]
    LockParse(String),
    #[error("unreadable sources: {0}")]
    SourcesRegex(#[source] regex::Error),
    #[error("jvm dep entry without group/artifact")]
    JvmEntry,
    #[error("cc dep entry without name")]
    CcEntry,
    #[error("unreadable manifest: no maven coordinates")]
    NoMavenCoords,
}

/// Every distinct locked version per package name, in lock order.
pub type LockVersions = std::collections::BTreeMap<String, Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepInfo {
    pub spec: String,
    pub category: String,
    pub optional: bool,
    pub platform: bool,
    pub raw: String,
    pub peer: bool,
    pub sha256: String,
    /// Lock name when the dependency is renamed, empty otherwise.
    pub package: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub src: bool,
    pub test: bool,
    pub build: bool,
}

/// Parses the language named by `--ecosystem`.
pub fn language_named(text: &str) -> Result<Language, DepcheckError> {
    Language::parse(text).ok_or_else(|| DepcheckError::UnknownEcosystem(text.to_owned()))
}

pub struct WorkspaceLocks<'a> {
    pub cargo_manifest: &'a Path,
    pub cargo_lock: &'a Path,
    pub uv_manifest: &'a Path,
    pub uv_lock: &'a Path,
    pub pnpm_manifest: &'a Path,
    pub pnpm_lock: &'a Path,
    pub go_manifest: &'a Path,
    pub go_lock: &'a Path,
    pub maven_artifacts: &'a Path,
    pub maven_lock: &'a Path,
    pub paket_manifest: &'a Path,
    pub paket_lock: &'a Path,
    pub ruby_manifest: &'a Path,
    pub ruby_lock: &'a Path,
}

impl From<DepcheckError> for dx_error::DxError {
    fn from(error: DepcheckError) -> Self {
        match error {
            DepcheckError::UnknownEcosystem(_) => Self::usage(error.to_string()),
            DepcheckError::ManifestIo(ref source) | DepcheckError::LockIo(ref source) => {
                Self::from_io(error.to_string(), source)
            }
            _ => Self::failure(error.to_string()),
        }
    }
}
