#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::sync::OnceLock;

use serde::Deserialize;

pub const SUPPORTED_ECOSYSTEMS: &str = "uv, npm, bazel, ruby, powershell, cargo, maven, nuget, go, cpan, pub, conan, julia, hex, composer, opam, swiftpm, cabal, dub, fpm, zig, luarocks, ansible, bicep";

const TABLE: &str = include_str!("../ecosystems.json");
const LANGUAGES: &str = include_str!("../languages.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageRow {
    name: String,
    depcheck: String,
    semver: bool,
    ecosystems: Vec<Ecosystem>,
    #[serde(skip)]
    id: Option<Language>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageTable {
    languages: Vec<LanguageRow>,
}

fn parse_language_rows(text: &str) -> Result<Vec<LanguageRow>, serde_json::Error> {
    let mut rows = serde_json::from_str::<LanguageTable>(text)?.languages;
    for row in &mut rows {
        let name = serde_json::Value::String(row.depcheck.clone());
        row.id = Some(serde_json::from_value(name)?);
    }
    Ok(rows)
}

fn language_rows() -> &'static [LanguageRow] {
    static LANGUAGES_TABLE: OnceLock<Vec<LanguageRow>> = OnceLock::new();
    LANGUAGES_TABLE.get_or_init(|| parse_language_rows(LANGUAGES).unwrap_or_default())
}

/// A language whose manifests and sources depcheck reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
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

impl Language {
    pub const ALL: [Language; 26] = [
        Language::Rust,
        Language::Python,
        Language::Js,
        Language::Ts,
        Language::Go,
        Language::Java,
        Language::Kotlin,
        Language::Scala,
        Language::Csharp,
        Language::Fsharp,
        Language::Cc,
        Language::Ruby,
        Language::Perl,
        Language::Zig,
        Language::Julia,
        Language::Dart,
        Language::Php,
        Language::Elixir,
        Language::Erlang,
        Language::Fortran,
        Language::D,
        Language::Haskell,
        Language::Swift,
        Language::Powershell,
        Language::Lua,
        Language::Ocaml,
    ];

    fn row(self) -> Option<&'static LanguageRow> {
        language_rows().iter().find(|row| row.id == Some(self))
    }

    /// Names the language as depcheck spells it.
    pub fn name(self) -> &'static str {
        self.row().map_or("", |row| row.depcheck.as_str())
    }

    /// Parses a depcheck language name, returning `None` when it is not one of them.
    pub fn parse(text: &str) -> Option<Language> {
        language_rows().iter().find(|row| row.depcheck == text)?.id
    }

    /// Names the language directory that holds this language's `lang.json`.
    pub fn directory(self) -> &'static str {
        self.row().map_or("", |row| row.name.as_str())
    }

    /// Picks the language that reads `ecosystem`, `prefer` first when it reads it too.
    pub fn of_ecosystem(ecosystem: Ecosystem, prefer: Option<Language>) -> Option<Language> {
        let reads = |row: &&LanguageRow| row.ecosystems.contains(&ecosystem);
        let rows = language_rows();
        prefer
            .and_then(|language| {
                rows.iter()
                    .filter(reads)
                    .find(|row| row.id == Some(language))
            })
            .or_else(|| rows.iter().find(reads))?
            .id
    }

    /// Lists the ecosystems whose packages this language depends on.
    pub fn ecosystems(self) -> &'static [Ecosystem] {
        self.row().map_or(&[], |row| row.ecosystems.as_slice())
    }

    /// Whether this language states its requirements in the semver grammar.
    pub fn uses_semver_grammar(self) -> bool {
        self.row().is_some_and(|row| row.semver)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    manifest: String,
    alt_manifests: Vec<String>,
    shared_manifest: bool,
    plan_backend: String,
    check_backend: String,
    graph_only: bool,
    check: bool,
    update: bool,
    audit: bool,
    advisory_family: Option<String>,
    uses_repository: bool,
    lockfiles: Vec<String>,
    osv_source: Option<String>,
    license_audit: bool,
    lock_required: bool,
    pinned_reason: Option<String>,
    #[serde(skip)]
    id: Option<Ecosystem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    ecosystems: Vec<Row>,
}

fn parse_rows(text: &str) -> Result<Vec<Row>, serde_json::Error> {
    let mut rows = serde_json::from_str::<Table>(text)?.ecosystems;
    for row in &mut rows {
        let name = serde_json::Value::String(row.name.clone());
        row.id = Some(serde_json::from_value(name)?);
    }
    Ok(rows)
}

fn rows() -> &'static [Row] {
    static ROWS: OnceLock<Vec<Row>> = OnceLock::new();
    ROWS.get_or_init(|| parse_rows(TABLE).unwrap_or_default())
}

/// Names the OSV archive URL of an advisory family.
pub fn osv_source_of_family(family: &str) -> Option<&'static str> {
    rows()
        .iter()
        .find(|row| row.advisory_family.as_deref() == Some(family))?
        .osv_source
        .as_deref()
}

/// Returns the table's spelling of an advisory family name.
pub fn advisory_family_named(family: &str) -> Option<&'static str> {
    rows().iter().find_map(|row| {
        row.advisory_family
            .as_deref()
            .filter(|name| *name == family)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ecosystem {
    Uv,
    Npm,
    Bazel,
    Ruby,
    PowerShell,
    Cargo,
    Maven,
    Nuget,
    Go,
    Cpan,
    Pub,
    Conan,
    Julia,
    Hex,
    Composer,
    Dub,
    Fpm,
    Zig,
    LuaRocks,
    Opam,
    SwiftPm,
    Cabal,
    Ansible,
    Bicep,
}

impl Ecosystem {
    pub const ALL: [Ecosystem; 24] = [
        Ecosystem::Uv,
        Ecosystem::Npm,
        Ecosystem::Bazel,
        Ecosystem::Ruby,
        Ecosystem::PowerShell,
        Ecosystem::Cargo,
        Ecosystem::Maven,
        Ecosystem::Nuget,
        Ecosystem::Go,
        Ecosystem::Cpan,
        Ecosystem::Pub,
        Ecosystem::Conan,
        Ecosystem::Julia,
        Ecosystem::Hex,
        Ecosystem::Composer,
        Ecosystem::Dub,
        Ecosystem::Fpm,
        Ecosystem::Zig,
        Ecosystem::LuaRocks,
        Ecosystem::Opam,
        Ecosystem::SwiftPm,
        Ecosystem::Cabal,
        Ecosystem::Ansible,
        Ecosystem::Bicep,
    ];

    pub fn name(self) -> &'static str {
        self.row().map_or("", |row| row.name.as_str())
    }

    pub fn parse(text: &str) -> Option<Ecosystem> {
        rows().iter().find(|row| row.name == text)?.id
    }

    fn row(self) -> Option<&'static Row> {
        rows().iter().find(|row| row.id == Some(self))
    }

    fn operation(self, enabled: impl Fn(&Row) -> bool, label: &'static str) -> &'static str {
        match self.row() {
            Some(row) if enabled(row) => label,
            _ => "pinned",
        }
    }

    pub fn supported_check(self) -> &'static str {
        self.operation(|row| row.check, "check")
    }

    pub fn supported_update(self) -> &'static str {
        self.operation(|row| row.update, "update")
    }

    pub fn supported_audit(self) -> &'static str {
        self.operation(|row| row.audit, "audit")
    }

    /// Lists what dx does for this ecosystem, folding pinned check and update into one clause.
    pub fn supported_operations(self) -> String {
        let (check, update, audit) = (
            self.supported_check(),
            self.supported_update(),
            self.supported_audit(),
        );
        if check == "pinned" && update == "pinned" {
            format!("{audit}; check and update report pinned")
        } else {
            format!("{check}, {update}, {audit}")
        }
    }

    /// Names the advisory snapshot family that covers this ecosystem's locks.
    pub fn advisory_family(self) -> Option<&'static str> {
        self.row()?.advisory_family.as_deref()
    }

    /// Lists the lock file names of this ecosystem, preferred backend first.
    pub fn lockfiles(self) -> &'static [String] {
        self.row().map_or(&[], |row| row.lockfiles.as_slice())
    }

    /// Names the OSV archive URL of this ecosystem's advisories.
    pub fn osv_source(self) -> Option<&'static str> {
        self.row()?.osv_source.as_deref()
    }

    /// Names the file every manifest of this ecosystem must have.
    pub fn manifest_name(self) -> &'static str {
        self.row().map_or("", |row| row.manifest.as_str())
    }

    /// Names the other file names a manifest of this ecosystem may have.
    pub fn alt_manifests(self) -> &'static [String] {
        self.row().map_or(&[], |row| row.alt_manifests.as_slice())
    }

    /// Tells whether this ecosystem's manifest is a file other ecosystems write too.
    pub fn shared_manifest(self) -> bool {
        self.row().is_some_and(|row| row.shared_manifest)
    }

    /// Names the backend module that plans updates of this ecosystem, or `pinned`.
    pub fn plan_backend(self) -> &'static str {
        self.row().map_or("pinned", |row| row.plan_backend.as_str())
    }

    /// Names the backend module that checks this ecosystem, or `pinned`.
    pub fn check_backend(self) -> &'static str {
        self.row()
            .map_or("pinned", |row| row.check_backend.as_str())
    }

    /// Tells whether only the Bazel graph can say which packages of this ecosystem are used.
    pub fn graph_only(self) -> bool {
        self.row().is_some_and(|row| row.graph_only)
    }

    /// Tells whether the license audit reads this ecosystem's locks.
    pub fn license_audit(self) -> bool {
        self.row().is_some_and(|row| row.license_audit)
    }

    /// Tells whether every set of this ecosystem must list its first lock file.
    pub fn lock_required(self) -> bool {
        self.row().is_some_and(|row| row.lock_required)
    }

    /// Says how to change one package of a set that dx only pins.
    pub fn pinned_reason(self) -> Option<&'static str> {
        self.row()?.pinned_reason.as_deref()
    }

    /// Tells whether records of this ecosystem name the Bazel repository dx resolves through.
    pub fn uses_repository(self) -> bool {
        self.row().is_some_and(|row| row.uses_repository)
    }
}

#[cfg(test)]
mod tests;
