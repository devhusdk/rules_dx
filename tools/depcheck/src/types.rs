use std::path::Path;

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
    #[error("unreadable exceptions: {0}")]
    ExceptionsIo(#[source] std::io::Error),
    #[error("unreadable exceptions: {0}")]
    ExceptionsToml(#[source] toml::de::Error),
    #[error("unreadable sources: {0}")]
    SourcesRegex(#[source] regex::Error),
    #[error("jvm dep entry without group/artifact")]
    JvmEntry,
    #[error("cc dep entry without name")]
    CcEntry,
    #[error("exception entry without dependency")]
    ExceptionEntry,
    #[error("exceptions file missing: {0}")]
    ExceptionsMissing(String),
    #[error("unreadable manifest: no maven coordinates")]
    NoMavenCoords,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepInfo {
    pub spec: String,
    pub category: String,
    pub optional: bool,
    pub platform: bool,
    pub raw: String,
    pub peer: bool,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exception {
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub src: bool,
    pub test: bool,
    pub build: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ecosystem {
    Rust,
    Python,
    Js,
    Ts,
    Go,
    Java,
    Kotlin,
    Scala,
    Csharp,
    Fsharp,
    Cc,
    Ruby,
}

impl std::str::FromStr for Ecosystem {
    type Err = DepcheckError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| DepcheckError::UnknownEcosystem(text.to_owned()))
    }
}

impl Ecosystem {
    /// Parses an ecosystem name, returning `None` when it is not one of them.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "js" => Some(Self::Js),
            "ts" => Some(Self::Ts),
            "go" => Some(Self::Go),
            "java" => Some(Self::Java),
            "kotlin" => Some(Self::Kotlin),
            "scala" => Some(Self::Scala),
            "csharp" => Some(Self::Csharp),
            "fsharp" => Some(Self::Fsharp),
            "cc" => Some(Self::Cc),
            "ruby" => Some(Self::Ruby),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Js => "js",
            Self::Ts => "ts",
            Self::Go => "go",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::Scala => "scala",
            Self::Csharp => "csharp",
            Self::Fsharp => "fsharp",
            Self::Cc => "cc",
            Self::Ruby => "ruby",
        }
    }

    /// Whether this ecosystem states its requirements in the semver grammar.
    pub fn uses_semver_grammar(self) -> bool {
        matches!(self, Self::Rust | Self::Js | Self::Ts | Self::Go)
    }
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
