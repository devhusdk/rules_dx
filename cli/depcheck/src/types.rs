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
    Perl,
    Zig,
    Julia,
    Dart,
    Php,
    Elixir,
    Erlang,
    Fortran,
    D,
    Haskell,
    Swift,
    Powershell,
    Lua,
    Ocaml,
}

impl std::str::FromStr for Ecosystem {
    type Err = DepcheckError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| DepcheckError::UnknownEcosystem(text.to_owned()))
    }
}

impl Ecosystem {
    const ALL: [Ecosystem; 26] = [
        Self::Rust,
        Self::Python,
        Self::Js,
        Self::Ts,
        Self::Go,
        Self::Java,
        Self::Kotlin,
        Self::Scala,
        Self::Csharp,
        Self::Fsharp,
        Self::Cc,
        Self::Ruby,
        Self::Perl,
        Self::Zig,
        Self::Julia,
        Self::Dart,
        Self::Php,
        Self::Elixir,
        Self::Erlang,
        Self::Fortran,
        Self::D,
        Self::Haskell,
        Self::Swift,
        Self::Powershell,
        Self::Lua,
        Self::Ocaml,
    ];

    fn language(self) -> Option<&'static dx_ecosystems::Language> {
        let key = format!("{self:?}").to_ascii_lowercase();
        dx_ecosystems::languages()
            .iter()
            .find(|language| language.depcheck == key)
    }

    /// Parses an ecosystem name, returning `None` when it is not one of them.
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ecosystem| ecosystem.name() == text)
    }

    /// Picks the language that reads the dependency ecosystem `id`, the one named `prefer` first.
    pub fn of_dependency_ecosystem(id: &str, prefer: Option<&str>) -> Option<Self> {
        let languages = dx_ecosystems::languages();
        let lists = |language: &&dx_ecosystems::Language| {
            language.ecosystems.iter().any(|listed| listed == id)
        };
        let language = prefer
            .and_then(|name| {
                languages
                    .iter()
                    .filter(lists)
                    .find(|language| language.depcheck == name)
            })
            .or_else(|| languages.iter().find(lists))?;
        Self::parse(&language.depcheck)
    }

    pub fn name(self) -> &'static str {
        self.language()
            .map_or("", |language| language.depcheck.as_str())
    }

    /// Whether this ecosystem states its requirements in the semver grammar.
    pub fn uses_semver_grammar(self) -> bool {
        self.language().is_some_and(|language| language.semver)
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
