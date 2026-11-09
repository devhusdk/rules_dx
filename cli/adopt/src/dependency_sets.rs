use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const ECOSYSTEM_UV: &str = "uv";

pub const ECOSYSTEM_NPM: &str = "npm";

pub const ECOSYSTEM_BAZEL: &str = "bazel";

pub const ECOSYSTEM_RUBY: &str = "ruby";

pub const ECOSYSTEM_POWERSHELL: &str = "powershell";

pub const ECOSYSTEM_CARGO: &str = "cargo";

pub const ECOSYSTEM_MAVEN: &str = "maven";

pub const ECOSYSTEM_NUGET: &str = "nuget";

pub const ECOSYSTEM_GO: &str = "go";

pub const ECOSYSTEM_CPAN: &str = "cpan";

pub const ECOSYSTEM_PUB: &str = "pub";

pub const ECOSYSTEM_CONAN: &str = "conan";

pub const ECOSYSTEM_JULIA: &str = "julia";

pub const ECOSYSTEM_HEX: &str = "hex";

pub const ECOSYSTEM_COMPOSER: &str = "composer";

pub const ECOSYSTEM_DUB: &str = "dub";

pub const ECOSYSTEM_LUAROCKS: &str = "luarocks";

pub const ECOSYSTEM_OPAM: &str = "opam";

pub const ECOSYSTEM_SWIFTPM: &str = "swiftpm";

pub const ECOSYSTEM_CABAL: &str = "cabal";

pub const ECOSYSTEM_ANSIBLE: &str = "ansible";

pub const ECOSYSTEM_BICEP: &str = "bicep";

pub const SUPPORTED_ECOSYSTEMS: &str = "uv, npm, bazel, ruby, powershell, cargo, maven, nuget, go, cpan, pub, conan, julia, hex, composer, opam, swiftpm, cabal, dub, luarocks, ansible, bicep";

pub const MODULE_BAZEL: &str = "MODULE.bazel";

pub const CARGO_LOCK: &str = "Cargo.lock";

pub const GEMFILE_LOCK: &str = "Gemfile.lock";

pub const GO_MOD: &str = "go.mod";

pub const GO_SUM: &str = "go.sum";

pub const CPANFILE: &str = "cpanfile";

pub const CPANFILE_SNAPSHOT: &str = "cpanfile.snapshot";

pub const MIX_EXS: &str = "mix.exs";

pub const MIX_LOCK: &str = "mix.lock";

pub const REBAR_CONFIG: &str = "rebar.config";

pub const REBAR_LOCK: &str = "rebar.lock";

pub const PUBSPEC_YAML: &str = "pubspec.yaml";

pub const PUBSPEC_LOCK: &str = "pubspec.lock";

const CONANFILES: [&str; 2] = ["conanfile.txt", "conanfile.py"];

pub const CONAN_LOCK: &str = "conan.lock";

pub const PROJECT_TOML: &str = "Project.toml";

pub const JULIA_MANIFEST: &str = "Manifest.toml";

pub const JULIA_BAZEL_MANIFEST: &str = "Manifest.bazel.json";
pub const DUB_SELECTIONS: &str = "dub.selections.json";

pub const DUB_DIGESTS: &str = "dub.digests.json";

const DUB_MANIFESTS: [&str; 2] = ["dub.json", "dub.sdl"];

pub const LUAROCKS_LOCK: &str = "luarocks.lock";

pub const LUAROCKS_DIGESTS: &str = "luarocks.digests.json";

pub const COMPOSER_JSON: &str = "composer.json";

pub const COMPOSER_LOCK: &str = "composer.lock";

pub const PACKAGE_SWIFT: &str = "Package.swift";

pub const PACKAGE_RESOLVED: &str = "Package.resolved";

pub const CABAL_PROJECT: &str = "cabal.project";

pub const CABAL_FREEZE: &str = "cabal.project.freeze";

pub const PAKET_LOCK: &str = "paket.lock";

pub const PSGALLERY_LOCK: &str = "PSGallery.lock.json";

pub const PNPM_LOCK: &str = "pnpm-lock.yaml";

pub const YARN_LOCK: &str = "yarn.lock";

pub const NPM_LOCK: &str = "package-lock.json";

pub const PNPM_WORKSPACE: &str = "pnpm-workspace.yaml";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
    LuaRocks,
    Opam,
    SwiftPm,
    Cabal,
    Ansible,
    Bicep,
}

impl Ecosystem {
    pub fn name(self) -> &'static str {
        match self {
            Ecosystem::Uv => ECOSYSTEM_UV,
            Ecosystem::Npm => ECOSYSTEM_NPM,
            Ecosystem::Bazel => ECOSYSTEM_BAZEL,
            Ecosystem::Ruby => ECOSYSTEM_RUBY,
            Ecosystem::PowerShell => ECOSYSTEM_POWERSHELL,
            Ecosystem::Cargo => ECOSYSTEM_CARGO,
            Ecosystem::Maven => ECOSYSTEM_MAVEN,
            Ecosystem::Nuget => ECOSYSTEM_NUGET,
            Ecosystem::Go => ECOSYSTEM_GO,
            Ecosystem::Cpan => ECOSYSTEM_CPAN,
            Ecosystem::Pub => ECOSYSTEM_PUB,
            Ecosystem::Conan => ECOSYSTEM_CONAN,
            Ecosystem::Julia => ECOSYSTEM_JULIA,
            Ecosystem::Hex => ECOSYSTEM_HEX,
            Ecosystem::Composer => ECOSYSTEM_COMPOSER,
            Ecosystem::Dub => ECOSYSTEM_DUB,
            Ecosystem::LuaRocks => ECOSYSTEM_LUAROCKS,
            Ecosystem::Opam => ECOSYSTEM_OPAM,
            Ecosystem::Cabal => ECOSYSTEM_CABAL,
            Ecosystem::Ansible => ECOSYSTEM_ANSIBLE,
            Ecosystem::Bicep => ECOSYSTEM_BICEP,
            Ecosystem::SwiftPm => ECOSYSTEM_SWIFTPM,
        }
    }

    pub fn parse(text: &str) -> Option<Ecosystem> {
        match text {
            ECOSYSTEM_UV => Some(Ecosystem::Uv),
            ECOSYSTEM_NPM => Some(Ecosystem::Npm),
            ECOSYSTEM_BAZEL => Some(Ecosystem::Bazel),
            ECOSYSTEM_RUBY => Some(Ecosystem::Ruby),
            ECOSYSTEM_POWERSHELL => Some(Ecosystem::PowerShell),
            ECOSYSTEM_CARGO => Some(Ecosystem::Cargo),
            ECOSYSTEM_MAVEN => Some(Ecosystem::Maven),
            ECOSYSTEM_NUGET => Some(Ecosystem::Nuget),
            ECOSYSTEM_GO => Some(Ecosystem::Go),
            ECOSYSTEM_CPAN => Some(Ecosystem::Cpan),
            ECOSYSTEM_PUB => Some(Ecosystem::Pub),
            ECOSYSTEM_CONAN => Some(Ecosystem::Conan),
            ECOSYSTEM_JULIA => Some(Ecosystem::Julia),
            ECOSYSTEM_HEX => Some(Ecosystem::Hex),
            ECOSYSTEM_COMPOSER => Some(Ecosystem::Composer),
            ECOSYSTEM_DUB => Some(Ecosystem::Dub),
            ECOSYSTEM_LUAROCKS => Some(Ecosystem::LuaRocks),
            ECOSYSTEM_OPAM => Some(Ecosystem::Opam),
            ECOSYSTEM_CABAL => Some(Ecosystem::Cabal),
            ECOSYSTEM_ANSIBLE => Some(Ecosystem::Ansible),
            ECOSYSTEM_BICEP => Some(Ecosystem::Bicep),
            ECOSYSTEM_SWIFTPM => Some(Ecosystem::SwiftPm),
            _ => None,
        }
    }

    pub fn supported_check(self) -> &'static str {
        match self {
            Ecosystem::Uv
            | Ecosystem::Npm
            | Ecosystem::Cargo
            | Ecosystem::Maven
            | Ecosystem::Nuget
            | Ecosystem::Go
            | Ecosystem::SwiftPm => "check",
            Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Cpan
            | Ecosystem::Pub
            | Ecosystem::Conan
            | Ecosystem::Julia
            | Ecosystem::Hex
            | Ecosystem::Composer
            | Ecosystem::Dub
            | Ecosystem::LuaRocks
            | Ecosystem::Opam
            | Ecosystem::Ansible
            | Ecosystem::Bicep
            | Ecosystem::Cabal => "pinned",
        }
    }

    pub fn supported_update(self) -> &'static str {
        match self {
            Ecosystem::Uv
            | Ecosystem::Npm
            | Ecosystem::Cargo
            | Ecosystem::Maven
            | Ecosystem::Nuget
            | Ecosystem::Go
            | Ecosystem::Ansible
            | Ecosystem::SwiftPm => "update",
            Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Cpan
            | Ecosystem::Pub
            | Ecosystem::Conan
            | Ecosystem::Julia
            | Ecosystem::Hex
            | Ecosystem::Composer
            | Ecosystem::Dub
            | Ecosystem::LuaRocks
            | Ecosystem::Opam
            | Ecosystem::Bicep
            | Ecosystem::Cabal => "pinned",
        }
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

    pub fn supported_audit(self) -> &'static str {
        match self {
            Ecosystem::Uv
            | Ecosystem::Npm
            | Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Cargo
            | Ecosystem::Maven
            | Ecosystem::Nuget
            | Ecosystem::Go
            | Ecosystem::Cpan
            | Ecosystem::Pub
            | Ecosystem::Conan
            | Ecosystem::Julia
            | Ecosystem::Hex
            | Ecosystem::Composer
            | Ecosystem::Dub
            | Ecosystem::LuaRocks
            | Ecosystem::Opam
            | Ecosystem::SwiftPm
            | Ecosystem::Ansible
            | Ecosystem::Bicep
            | Ecosystem::Cabal => "audit",
        }
    }

    /// Names the advisory snapshot family that covers this ecosystem's locks.
    pub fn advisory_family(self) -> Option<&'static str> {
        match self {
            Ecosystem::Uv => Some("pypi"),
            Ecosystem::Npm => Some("npm"),
            Ecosystem::Ruby => Some("rubygems"),
            Ecosystem::Cargo => Some("cargo"),
            Ecosystem::Maven => Some("maven"),
            Ecosystem::Nuget => Some("nuget"),
            Ecosystem::Go => Some("go"),
            Ecosystem::Cpan => Some("cpan"),
            Ecosystem::Pub => Some("pub"),
            Ecosystem::Julia => Some("julia"),
            Ecosystem::Hex => Some("hex"),
            Ecosystem::Composer => Some("packagist"),
            Ecosystem::Dub | Ecosystem::LuaRocks | Ecosystem::Ansible | Ecosystem::Bicep => None,
            Ecosystem::Opam => Some("opam"),
            Ecosystem::Cabal => Some("hackage"),
            Ecosystem::SwiftPm => Some("swifturl"),
            Ecosystem::Bazel | Ecosystem::PowerShell | Ecosystem::Conan => None,
        }
    }

    /// Names the file every manifest of this ecosystem must have.
    pub fn manifest_name(self) -> &'static str {
        match self {
            Ecosystem::Uv => "pyproject.toml",
            Ecosystem::Npm => "package.json",
            Ecosystem::Bazel => "MODULE.bazel",
            Ecosystem::Ruby => "Gemfile",
            Ecosystem::PowerShell => "PSGallery.requirements.psd1",
            Ecosystem::Cargo => "Cargo.toml",
            Ecosystem::Maven | Ecosystem::Ansible | Ecosystem::Bicep => MODULE_BAZEL,
            Ecosystem::Nuget => "paket.dependencies",
            Ecosystem::Go => GO_MOD,
            Ecosystem::Cpan => CPANFILE,
            Ecosystem::Pub => PUBSPEC_YAML,
            Ecosystem::Conan => "conanfile.txt",
            Ecosystem::Julia => PROJECT_TOML,
            Ecosystem::Hex => MIX_EXS,
            Ecosystem::Composer => COMPOSER_JSON,
            Ecosystem::Dub => "dub.json",
            Ecosystem::LuaRocks => LUAROCKS_LOCK,
            Ecosystem::Opam => "<name>.opam",
            Ecosystem::Cabal => CABAL_PROJECT,
            Ecosystem::SwiftPm => PACKAGE_SWIFT,
        }
    }

    /// Tells whether records of this ecosystem name the Bazel repository dx resolves through.
    pub fn uses_repository(self) -> bool {
        match self {
            Ecosystem::Cargo | Ecosystem::Maven => true,
            Ecosystem::Uv
            | Ecosystem::Npm
            | Ecosystem::Bazel
            | Ecosystem::Ruby
            | Ecosystem::PowerShell
            | Ecosystem::Nuget
            | Ecosystem::Go
            | Ecosystem::Cpan
            | Ecosystem::Pub
            | Ecosystem::Conan
            | Ecosystem::Julia
            | Ecosystem::Hex
            | Ecosystem::Composer
            | Ecosystem::Dub
            | Ecosystem::LuaRocks
            | Ecosystem::Opam
            | Ecosystem::SwiftPm
            | Ecosystem::Ansible
            | Ecosystem::Bicep
            | Ecosystem::Cabal => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencySet {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub manifests: Vec<String>,
    pub locks: Vec<String>,
    pub scopes: Vec<String>,
    pub repository: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registry {
    pub sets: Vec<DependencySet>,
}

impl Registry {
    pub fn find(&self, name: &str) -> Option<&DependencySet> {
        self.sets.iter().find(|set| set.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.sets.iter().map(|set| set.name.as_str()).collect()
    }

    pub fn name_list(&self) -> String {
        self.names().join(", ")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selection {
    Full,
    Packages(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedSet {
    pub name: String,
    pub ecosystem: Ecosystem,
    pub dir: String,
    pub manifests: Vec<String>,
    pub locks: Vec<String>,
    pub repository: Option<String>,
}

impl ResolvedSet {
    pub fn of(set: &DependencySet) -> ResolvedSet {
        ResolvedSet {
            name: set.name.clone(),
            ecosystem: set.ecosystem,
            dir: manifest_dir(&set.manifests),
            manifests: set.manifests.clone(),
            locks: set.locks.clone(),
            repository: set.repository.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DependencySetsError {
    #[error("could not read {rel}: {detail}")]
    Read { rel: String, detail: String },
    #[error("could not read {rel}: not valid UTF-8")]
    NotUtf8 { rel: String },
    #[error("invalid {rel}: {detail}")]
    InvalidToml { rel: String, detail: String },
    #[error("cannot read dependency sets from {MODULE_BAZEL}: {detail}")]
    Module { detail: String },
    #[error(
        "invalid {MODULE_BAZEL}: dependency set name {name:?} must start alphanumerically and use [A-Za-z0-9_.-] only"
    )]
    InvalidName { name: String },
    #[error(
        "invalid {MODULE_BAZEL}: dependency set {name:?} owns path {path:?} which escapes the workspace"
    )]
    EscapingPath { name: String, path: String },
    #[error(
        "invalid {MODULE_BAZEL}: dependency set {name:?} manifest {path:?} must be named {want} for ecosystem {ecosystem}"
    )]
    InvalidManifest {
        name: String,
        path: String,
        want: &'static str,
        ecosystem: &'static str,
    },
    #[error("invalid {MODULE_BAZEL}: duplicate dependency set name {name:?}")]
    DuplicateName { name: String },
    #[error("invalid {MODULE_BAZEL}: {path:?} is owned by dependency sets {names}")]
    ConflictingOwnership { path: String, names: String },
    #[error(
        "unknown dependency set or scope {selector:?}; expected {names}, set:package, or a workspace path"
    )]
    UnknownSelector { selector: String, names: String },
    #[error("invalid package {package:?} for set {set}: {reason}")]
    InvalidPackage {
        set: String,
        package: String,
        reason: &'static str,
    },
    #[error(
        "no owning dependency set for {scope:?} (non-dependency paths are out of dependency-set scope; sets: {names})"
    )]
    NoOwningSet { scope: String, names: String },
    #[error("dependency set {name:?} is missing {path} (declared in {rel})")]
    MissingFile {
        name: String,
        path: String,
        rel: String,
    },
    #[error("dependency set {name:?} path {path} leaves the workspace (declared in {rel})")]
    OutsideWorkspace {
        name: String,
        path: String,
        rel: String,
    },
    #[error(
        "dependency set {name:?} relies on uv settings dx does not apply: {detail}; keep resolver settings out of uv.toml and [tool.uv]"
    )]
    UnsupportedSettings { name: String, detail: String },
    #[error("dependency set {name:?} cannot run pnpm in {dir}: {detail}")]
    UnsupportedNpmLayout {
        name: String,
        dir: String,
        detail: String,
    },
    #[error(
        "dependency set {name:?} must declare {path} in locks; dx reads the {ecosystem} lock beside its manifest"
    )]
    MissingPinnedLock {
        name: String,
        path: String,
        ecosystem: &'static str,
    },
    #[error(
        "dependency set {name:?} has a stale {lock}: {detail}; regenerate Manifest.toml and Manifest.bazel.json together"
    )]
    StaleJuliaLock {
        name: String,
        lock: String,
        detail: String,
    },
    #[error(
        "dependency set {name:?} has a stale {lock}: {detail}; run the cpan_compiler next to the cpanfile"
    )]
    StaleCpanLock {
        name: String,
        lock: String,
        detail: String,
    },
    #[error(
        "dependency set {name:?} has a stale {lock}: {detail}; regenerate the digests for composer.lock"
    )]
    StaleComposerLock {
        name: String,
        lock: String,
        detail: String,
    },
    #[error(
        "dependency set {name:?} has a stale {lock}: {detail}; regenerate dub.digests.json for dub.selections.json"
    )]
    StaleDubLock {
        name: String,
        lock: String,
        detail: String,
    },
    #[error(
        "dependency set {name:?} has a stale {lock}: {detail}; run the haskell_deps_resolve target next to cabal.project"
    )]
    StaleCabalLock {
        name: String,
        lock: String,
        detail: String,
    },
    #[error("dependency set {name:?} cannot resolve maven hub {repository:?}: {detail}")]
    UnsupportedMavenHub {
        name: String,
        repository: String,
        detail: String,
    },
    #[error("dependency set {name:?} cannot resolve cargo hub {repository:?}: {detail}")]
    UnsupportedCargoHub {
        name: String,
        repository: String,
        detail: String,
    },
    #[error("dependency set {name:?} cannot run paket2bazel: {detail}")]
    UnsupportedNugetLayout { name: String, detail: String },
    #[error("dependency set {name:?} cannot run go mod download: {detail}")]
    UnsupportedGoLayout { name: String, detail: String },
    #[error(
        "invalid {MODULE_BAZEL}: dependency set {name:?} repository {repository:?} must start with a letter and use [A-Za-z0-9_.-] only"
    )]
    InvalidRepository { name: String, repository: String },
}

const UV_PROJECT_KEYS: &[&str] = &[
    "conflicts",
    "constraint-dependencies",
    "default-groups",
    "dev-dependencies",
    "managed",
    "override-dependencies",
    "package",
    "sources",
    "workspace",
];

/// Reads the dependency sets from the hubs in the root MODULE.bazel and its includes.
pub fn load(workspace: &Path) -> Result<Option<Registry>, DependencySetsError> {
    let root = match std::fs::read_to_string(workspace.join(MODULE_BAZEL)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(DependencySetsError::Read {
                rel: MODULE_BAZEL.to_owned(),
                detail: error.to_string(),
            });
        }
        Ok(text) => text,
    };
    discover(&root, &|rel: &str| {
        std::fs::read_to_string(workspace.join(rel)).map_err(|error| format!("{rel}: {error}"))
    })
}

/// The dependency sets of the hubs in `root`; `read` returns a workspace file by its path.
pub fn discover(
    root: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Option<Registry>, DependencySetsError> {
    let module_error = |detail: String| DependencySetsError::Module { detail };
    let read_include = |label: &str| read(&dx_module_bazel::include_rel("", label)?);
    let modules = dx_module_bazel::load_with_includes(root, &read_include).map_err(module_error)?;
    let path = |label: &str| dx_module_bazel::label_path(label).map_err(module_error);
    let hub = |error: dx_module_bazel::ModuleError| module_error(error.to_string());
    let mut sets = Vec::new();
    for module in &modules {
        for cargo in module.cargo_hubs().map_err(hub)? {
            let Some(cargo_lock) = cargo.cargo_lockfile else {
                continue;
            };
            let cargo_lock = path(cargo_lock)?;
            let lock_dir = manifest_dir(std::slice::from_ref(&cargo_lock));
            let mut manifests = cargo
                .manifests
                .iter()
                .map(|manifest| path(&manifest.value))
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(at) = manifests
                .iter()
                .position(|manifest| manifest_dir(std::slice::from_ref(manifest)) == lock_dir)
            {
                let first = manifests.remove(at);
                manifests.insert(0, first);
            }
            let mut locks = vec![cargo_lock];
            if let Some(lockfile) = cargo.lockfile {
                locks.push(path(lockfile)?);
            }
            sets.push(hub_set(cargo.name, Ecosystem::Cargo, manifests, locks)?);
        }
        for install in module.maven_installs().map_err(hub)? {
            let Some(lock) = install.lock_file else {
                continue;
            };
            sets.push(hub_set(
                install.name,
                Ecosystem::Maven,
                vec![MODULE_BAZEL.to_owned()],
                vec![path(lock)?],
            )?);
        }
        for npm in module.npm_translate_locks().map_err(hub)? {
            let Some(lock) = npm.pnpm_lock else {
                continue;
            };
            let lock = path(lock)?;
            let manifest = beside(&lock, "package.json");
            sets.push(hub_set(
                npm.name,
                Ecosystem::Npm,
                vec![manifest],
                vec![lock],
            )?);
        }
        for project in module.uv_projects().map_err(hub)? {
            let manifest = path(project.pyproject)?;
            let dir = manifest_dir(std::slice::from_ref(&manifest));
            let name = match dir.rsplit('/').next() {
                Some(".") | None => project.hub_name.to_owned(),
                Some(base) => format!("{}-{base}", project.hub_name),
            };
            sets.push(hub_set(
                &name,
                Ecosystem::Uv,
                vec![manifest],
                vec![path(project.lock)?],
            )?);
        }
        for go in module.go_deps_files().map_err(hub)? {
            let manifest = path(go.go_mod)?;
            let lock = beside(&manifest, GO_SUM);
            sets.push(hub_set("go", Ecosystem::Go, vec![manifest], vec![lock])?);
        }
        for bundle in module.bundle_fetches().map_err(hub)? {
            sets.push(hub_set(
                bundle.name,
                Ecosystem::Ruby,
                vec![path(bundle.gemfile)?],
                vec![path(bundle.gemfile_lock)?],
            )?);
        }
        for cpan in module.cpan_installs().map_err(hub)? {
            let lock = path(cpan.lock)?;
            let dir = manifest_dir(std::slice::from_ref(&lock));
            sets.push(hub_set(
                cpan.name,
                Ecosystem::Cpan,
                vec![in_dir(&dir, CPANFILE)],
                vec![in_dir(&dir, CPANFILE_SNAPSHOT), lock],
            )?);
        }
        for pub_lock in module.pub_locks().map_err(hub)? {
            let dir = manifest_dir(std::slice::from_ref(&path(pub_lock.lock)?));
            sets.push(hub_set(
                pub_lock.name,
                Ecosystem::Pub,
                vec![in_dir(&dir, PUBSPEC_YAML)],
                vec![in_dir(&dir, PUBSPEC_LOCK)],
            )?);
        }
        for mix in module.mix_locks().map_err(hub)? {
            let dir = manifest_dir(std::slice::from_ref(&path(mix.lock)?));
            sets.push(hub_set(
                mix.name,
                Ecosystem::Hex,
                vec![in_dir(&dir, MIX_EXS)],
                vec![in_dir(&dir, MIX_LOCK)],
            )?);
        }
        for rebar in module.rebar_locks().map_err(hub)? {
            let dir = manifest_dir(std::slice::from_ref(&path(rebar.lock)?));
            sets.push(hub_set(
                rebar.name,
                Ecosystem::Hex,
                vec![in_dir(&dir, REBAR_CONFIG)],
                vec![in_dir(&dir, REBAR_LOCK)],
            )?);
        }
        for composer in module.composer_locks().map_err(hub)? {
            let lock = path(composer.lock)?;
            let dir = manifest_dir(std::slice::from_ref(&lock));
            sets.push(hub_set(
                composer.name,
                Ecosystem::Composer,
                vec![in_dir(&dir, COMPOSER_JSON)],
                vec![lock, path(composer.digests)?],
            )?);
        }
        for dub in module.dub_locks().map_err(hub)? {
            let selections = path(dub.selections)?;
            let dir = manifest_dir(std::slice::from_ref(&selections));
            let manifest = DUB_MANIFESTS
                .iter()
                .map(|file| in_dir(&dir, file))
                .find(|candidate| read(candidate).is_ok())
                .unwrap_or_else(|| in_dir(&dir, DUB_MANIFESTS[0]));
            sets.push(hub_set(
                dub.name,
                Ecosystem::Dub,
                vec![manifest],
                vec![selections, path(dub.digests)?],
            )?);
        }
        for luarocks in module.luarocks_locks().map_err(hub)? {
            let lock = path(luarocks.lock)?;
            sets.push(hub_set(
                luarocks.name,
                Ecosystem::LuaRocks,
                vec![lock.clone()],
                vec![lock, path(luarocks.digests)?],
            )?);
        }
        for opam in module.opam_packages().map_err(hub)? {
            let lock = path(opam.lock)?;
            let manifest = lock.strip_suffix(".locked").unwrap_or(&lock).to_owned();
            sets.push(hub_set(
                opam.name,
                Ecosystem::Opam,
                vec![manifest],
                vec![lock],
            )?);
        }
        for haskell in module.haskell_packages().map_err(hub)? {
            let deps = path(haskell.lock)?;
            let dir = manifest_dir(std::slice::from_ref(&deps));
            sets.push(hub_set(
                haskell.name,
                Ecosystem::Cabal,
                vec![in_dir(&dir, CABAL_PROJECT)],
                vec![in_dir(&dir, CABAL_FREEZE), deps],
            )?);
        }
        for ansible in module.ansible_collections().map_err(hub)? {
            sets.push(hub_set(
                ansible.name,
                Ecosystem::Ansible,
                vec![MODULE_BAZEL.to_owned()],
                vec![path(ansible.lock)?],
            )?);
        }
        for bicep in module.bicep_registry_modules().map_err(hub)? {
            sets.push(hub_set(
                bicep.name,
                Ecosystem::Bicep,
                vec![MODULE_BAZEL.to_owned()],
                Vec::new(),
            )?);
        }
        for label in module.conan_extensions() {
            let bzl = dx_module_bazel::label_path(label)
                .map_err(|detail| DependencySetsError::Module { detail })?;
            let dir = manifest_dir(std::slice::from_ref(&bzl));
            let parent = manifest_dir(std::slice::from_ref(&dir));
            let found = [&dir, &parent].into_iter().find_map(|folder| {
                CONANFILES
                    .iter()
                    .map(|file| in_dir(folder, file))
                    .find(|candidate| read(candidate).is_ok())
            });
            let manifest = found.unwrap_or_else(|| in_dir(&dir, CONANFILES[0]));
            let lock = in_dir(&manifest_dir(std::slice::from_ref(&manifest)), CONAN_LOCK);
            sets.push(hub_set(
                ECOSYSTEM_CONAN,
                Ecosystem::Conan,
                vec![manifest],
                vec![lock],
            )?);
        }
        for julia in module.julia_manifests().map_err(hub)? {
            let dir = manifest_dir(std::slice::from_ref(&path(julia.lockfile)?));
            sets.push(hub_set(
                julia.name,
                Ecosystem::Julia,
                vec![in_dir(&dir, PROJECT_TOML)],
                vec![
                    in_dir(&dir, JULIA_MANIFEST),
                    in_dir(&dir, JULIA_BAZEL_MANIFEST),
                ],
            )?);
        }
        for (index, swift) in module
            .swift_packages()
            .map_err(hub)?
            .into_iter()
            .enumerate()
        {
            let name = match index {
                0 => "swiftpm".to_owned(),
                _ => format!("swiftpm-{}", index + 1),
            };
            sets.push(hub_set(
                &name,
                Ecosystem::SwiftPm,
                vec![path(swift.swift)?],
                vec![path(swift.resolved)?],
            )?);
        }
        let mut groups = BTreeSet::new();
        for label in module.paket_extensions() {
            groups.extend(paket_groups(label, read)?);
        }
        if let Some(first) = groups.first() {
            let folder = manifest_dir(std::slice::from_ref(first));
            let parent = manifest_dir(std::slice::from_ref(&folder));
            let mut locks = vec![in_dir(&parent, PAKET_LOCK)];
            locks.extend(groups);
            sets.push(hub_set(
                "nuget",
                Ecosystem::Nuget,
                vec![in_dir(&parent, "paket.dependencies")],
                locks,
            )?);
        }
    }
    if sets.is_empty() {
        return Ok(None);
    }
    check_conflicts(&sets)?;
    sets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Some(Registry { sets }))
}

/// The generated `paket.<group>.bzl` files a paket extension label loads.
fn paket_groups(
    label: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<Vec<String>, DependencySetsError> {
    let module_error = |detail: String| DependencySetsError::Module { detail };
    let bzl = dx_module_bazel::label_path(label).map_err(module_error)?;
    let package = manifest_dir(std::slice::from_ref(&bzl));
    let file = bzl.rsplit('/').next().unwrap_or(&bzl);
    if let Some(group) = file
        .strip_prefix("paket.")
        .and_then(|rest| rest.strip_suffix("_extension.bzl"))
    {
        return Ok(vec![in_dir(&package, &format!("paket.{group}.bzl"))]);
    }
    let text = read(&bzl).map_err(module_error)?;
    let loads =
        dx_module_bazel::load_labels(&text).map_err(|error| module_error(error.to_string()))?;
    let mut groups = Vec::new();
    for load in loads {
        let target = match load.strip_prefix(':') {
            Some(name) => in_dir(&package, name),
            None if load.starts_with("//") => {
                dx_module_bazel::label_path(&load).map_err(module_error)?
            }
            None => continue,
        };
        let name = target.rsplit('/').next().unwrap_or(&target);
        if name.starts_with("paket.") && name.ends_with(".bzl") && !name.ends_with("_extension.bzl")
        {
            groups.push(target);
        }
    }
    Ok(groups)
}

fn beside(file: &str, name: &str) -> String {
    in_dir(&manifest_dir(&[file.to_owned()]), name)
}

/// One set for a hub: scopes are the directories of its files.
fn hub_set(
    name: &str,
    ecosystem: Ecosystem,
    manifests: Vec<String>,
    locks: Vec<String>,
) -> Result<DependencySet, DependencySetsError> {
    let mut scopes = BTreeSet::new();
    for file in manifests.iter().chain(&locks) {
        if file != MODULE_BAZEL {
            scopes.insert(manifest_dir(std::slice::from_ref(file)));
        }
    }
    let set = DependencySet {
        name: name.to_owned(),
        ecosystem,
        manifests,
        locks,
        scopes: scopes.into_iter().collect(),
        repository: ecosystem.uses_repository().then(|| name.to_owned()),
    };
    validate_set(&set)?;
    Ok(set)
}

fn validate_set(set: &DependencySet) -> Result<(), DependencySetsError> {
    let name = &set.name;
    if !valid_name(name) {
        return Err(DependencySetsError::InvalidName { name: name.clone() });
    }
    for path in set.manifests.iter().chain(&set.locks) {
        if path.is_empty() || escapes_workspace(path) {
            return Err(DependencySetsError::EscapingPath {
                name: name.clone(),
                path: path.clone(),
            });
        }
    }
    for manifest in &set.manifests {
        let file = manifest.rsplit('/').next().unwrap_or(manifest);
        let allowed = match set.ecosystem {
            Ecosystem::Conan => CONANFILES.contains(&file),
            Ecosystem::Dub => DUB_MANIFESTS.contains(&file),
            Ecosystem::Opam => file.ends_with(".opam") && file != ".opam",
            Ecosystem::Hex => file == REBAR_CONFIG,
            _ => false,
        };
        if file != set.ecosystem.manifest_name() && !allowed {
            return Err(DependencySetsError::InvalidManifest {
                name: name.clone(),
                path: manifest.clone(),
                want: set.ecosystem.manifest_name(),
                ecosystem: set.ecosystem.name(),
            });
        }
    }
    if let Some(repository) = &set.repository
        && !valid_repository(repository)
    {
        return Err(DependencySetsError::InvalidRepository {
            name: name.clone(),
            repository: repository.clone(),
        });
    }
    Ok(())
}

/// Accepts a Bazel apparent repository name; it flows into argv and env, so no '@', ',' or '/'.
fn valid_repository(repository: &str) -> bool {
    let mut chars = repository.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-')
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    for char in chars {
        if !(char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-') {
            return false;
        }
    }
    true
}

fn escapes_workspace(path: &str) -> bool {
    if path.contains('\\') || path.starts_with('/') || has_drive_prefix(path) {
        return true;
    }
    if Path::new(path).components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return true;
    }
    let mut depth = 0i32;
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            _ => depth += 1,
        }
    }
    false
}

fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub fn manifest_dir(manifests: &[String]) -> String {
    let first = manifests.first().map(String::as_str).unwrap_or("");
    match first.rfind('/') {
        Some(index) => first[..index].to_owned(),
        None => ".".to_owned(),
    }
}

fn check_conflicts(sets: &[DependencySet]) -> Result<(), DependencySetsError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for set in sets {
        if !seen.insert(set.name.as_str()) {
            return Err(DependencySetsError::DuplicateName {
                name: set.name.clone(),
            });
        }
    }
    let mut writers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for set in sets {
        let manifests: &[String] = if matches!(
            set.ecosystem,
            Ecosystem::Maven | Ecosystem::Ansible | Ecosystem::Bicep
        ) {
            &[]
        } else {
            &set.manifests
        };
        for path in manifests.iter().chain(set.locks.iter()) {
            writers
                .entry(path.as_str())
                .or_default()
                .push(set.name.as_str());
        }
    }
    for (path, mut names) in writers {
        names.sort();
        names.dedup();
        if names.len() > 1 {
            return Err(DependencySetsError::ConflictingOwnership {
                path: path.to_owned(),
                names: names.join(", "),
            });
        }
    }
    Ok(())
}

const BAZEL_PACKAGE_CHARSET: &str = "bazel module names use [A-Za-z0-9_.-] only";

const CPAN_PACKAGE_CHARSET: &str = "cpan module names are words joined by '::'";
const UV_PACKAGE_CHARSET: &str = "uv package names use [A-Za-z0-9_.-] only";

const RUBY_PACKAGE_CHARSET: &str = "gem names use [A-Za-z0-9_.-] only";

const LUAROCKS_PACKAGE_CHARSET: &str = "LuaRocks rock names use [A-Za-z0-9_.-] only";

const POWERSHELL_PACKAGE_CHARSET: &str = "PowerShell module names use [A-Za-z0-9_.-] only";

const CARGO_PACKAGE_CHARSET: &str = "crate names start alphanumerically and use [A-Za-z0-9_-] only";

const MAVEN_PACKAGE_CHARSET: &str = "maven packages are group:artifact with [A-Za-z0-9_.-] parts";

const NUGET_PACKAGE_CHARSET: &str = "nuget package ids use [A-Za-z0-9._-] only";

const NPM_PACKAGE_CHARSET: &str =
    "npm package names are [@scope/]name with lowercase [a-z0-9._-] parts";

fn npm_part(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with('.')
        && !part.starts_with('_')
        && part.chars().all(|char| {
            char.is_ascii_lowercase()
                || char.is_ascii_digit()
                || char == '.'
                || char == '_'
                || char == '-'
        })
}

fn validate_package(ecosystem: Ecosystem, package: &str) -> Result<(), &'static str> {
    match ecosystem {
        Ecosystem::Npm => {
            let valid = match package.strip_prefix('@') {
                Some(scoped) => scoped
                    .split_once('/')
                    .is_some_and(|(scope, name)| npm_part(scope) && npm_part(name)),
                None => npm_part(package),
            };
            if valid {
                Ok(())
            } else {
                Err(NPM_PACKAGE_CHARSET)
            }
        }
        Ecosystem::Cargo => {
            if package
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphanumeric())
                && package
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
            {
                Ok(())
            } else {
                Err(CARGO_PACKAGE_CHARSET)
            }
        }
        Ecosystem::Maven => {
            let part = |part: &str| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
            };
            match package.split_once(':') {
                Some((group, artifact)) if part(group) && part(artifact) => Ok(()),
                _ => Err(MAVEN_PACKAGE_CHARSET),
            }
        }
        Ecosystem::Go => dx_module_bazel::identity::validate_go(package),
        Ecosystem::Cpan => {
            let word = |part: &str| {
                !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            };
            if package.split("::").all(word) {
                Ok(())
            } else {
                Err(CPAN_PACKAGE_CHARSET)
            }
        }
        Ecosystem::Uv
        | Ecosystem::Bazel
        | Ecosystem::Ruby
        | Ecosystem::PowerShell
        | Ecosystem::Pub
        | Ecosystem::Conan
        | Ecosystem::Julia
        | Ecosystem::Hex
        | Ecosystem::Composer
        | Ecosystem::Dub
        | Ecosystem::LuaRocks
        | Ecosystem::Opam
        | Ecosystem::Cabal
        | Ecosystem::Ansible
        | Ecosystem::Bicep
        | Ecosystem::SwiftPm
        | Ecosystem::Nuget => {
            let charset = match ecosystem {
                Ecosystem::Nuget => NUGET_PACKAGE_CHARSET,
                Ecosystem::Bazel => BAZEL_PACKAGE_CHARSET,
                Ecosystem::Ruby => RUBY_PACKAGE_CHARSET,
                Ecosystem::PowerShell => POWERSHELL_PACKAGE_CHARSET,
                Ecosystem::LuaRocks => LUAROCKS_PACKAGE_CHARSET,
                _ => UV_PACKAGE_CHARSET,
            };
            if package.is_empty() {
                return Err(charset);
            }
            for char in package.chars() {
                if !(char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-') {
                    return Err(charset);
                }
            }
            Ok(())
        }
    }
}

pub fn resolve(
    registry: &Registry,
    selectors: &[String],
) -> Result<Vec<ResolvedTarget>, DependencySetsError> {
    if selectors.is_empty() {
        return Ok(registry
            .sets
            .iter()
            .map(|set| ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Full,
            })
            .collect());
    }
    let mut full: BTreeSet<&str> = BTreeSet::new();
    let mut packages: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for raw in selectors {
        match parse_selector(registry, raw)? {
            ParsedSelector::Set(name) => {
                full.insert(name);
            }
            ParsedSelector::Package(name, package) => {
                packages.entry(name).or_default().push(package);
            }
            ParsedSelector::Scope(owners) => {
                for name in owners {
                    full.insert(name);
                }
            }
        }
    }
    let mut resolved = Vec::new();
    for set in &registry.sets {
        if full.contains(set.name.as_str()) {
            resolved.push(ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Full,
            });
        } else if let Some(mut names) = packages.remove(set.name.as_str()) {
            names.sort();
            names.dedup();
            resolved.push(ResolvedTarget {
                set: ResolvedSet::of(set),
                request: Selection::Packages(names),
            });
        }
    }
    Ok(resolved)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTarget {
    pub set: ResolvedSet,
    pub request: Selection,
}

enum ParsedSelector<'a> {
    Set(&'a str),
    Package(&'a str, String),
    Scope(Vec<&'a str>),
}

fn parse_selector<'a>(
    registry: &'a Registry,
    text: &str,
) -> Result<ParsedSelector<'a>, DependencySetsError> {
    let names = registry.name_list();
    if text.contains(':') {
        if let Some((head, tail)) = text.split_once(':')
            && let Some(set) = registry.find(head)
        {
            if tail.is_empty() {
                return Err(DependencySetsError::InvalidPackage {
                    set: set.name.clone(),
                    package: tail.to_owned(),
                    reason: "package identity is empty",
                });
            }
            if let Err(reason) = validate_package(set.ecosystem, tail) {
                return Err(DependencySetsError::InvalidPackage {
                    set: set.name.clone(),
                    package: tail.to_owned(),
                    reason,
                });
            }
            return Ok(ParsedSelector::Package(&set.name, tail.to_owned()));
        }
        if text.starts_with("//") || text.starts_with('@') {
            return scope_selector(registry, text, &names);
        }
        return Err(DependencySetsError::UnknownSelector {
            selector: text.to_owned(),
            names,
        });
    }
    if let Some(set) = registry.find(text) {
        return Ok(ParsedSelector::Set(&set.name));
    }
    if is_scope_shape(text) {
        return scope_selector(registry, text, &names);
    }
    Err(DependencySetsError::UnknownSelector {
        selector: text.to_owned(),
        names,
    })
}

fn is_scope_shape(text: &str) -> bool {
    text == "..."
        || text == "//..."
        || text.starts_with("//")
        || text.starts_with("./")
        || text.starts_with('/')
        || text.contains('/')
        || text.ends_with("/...")
}

fn scope_selector<'a>(
    registry: &'a Registry,
    text: &str,
    names: &str,
) -> Result<ParsedSelector<'a>, DependencySetsError> {
    let owners = owning_sets(registry, text);
    if owners.is_empty() {
        return Err(DependencySetsError::NoOwningSet {
            scope: text.to_owned(),
            names: names.to_owned(),
        });
    }
    Ok(ParsedSelector::Scope(owners))
}

fn normalize_scope(text: &str) -> (String, bool) {
    if text == "..." || text == "//..." {
        return (String::new(), true);
    }
    let mut path = text.to_owned();
    let mut recursive = false;
    if let Some(prefix) = path.strip_suffix("/...") {
        path = prefix.to_owned();
        recursive = true;
    }
    let label = path.starts_with("//") || path.starts_with('@');
    if let Some(rest) = path.strip_prefix("//") {
        path = rest.to_owned();
    }
    while let Some(rest) = path.strip_prefix("./") {
        path = rest.to_owned();
    }
    while path.ends_with('/') && path.len() > 1 {
        path.pop();
    }
    if label && let Some(colon) = path.find(':') {
        path = path[..colon].to_owned();
    }
    (path, recursive)
}

fn scope_covers(scope: &str, package: &str) -> bool {
    match package.strip_prefix(scope) {
        Some("") => true,
        Some(rest) => rest.starts_with('/'),
        None => false,
    }
}

/// Returns the workspace-relative file a path or `//pkg:file` label names, before
/// `normalize_scope` drops the label's target name.
fn declared_file_path(target: &str) -> Option<String> {
    if target.ends_with("...") {
        return None;
    }
    if let Some(rest) = target.strip_prefix("//") {
        let (package, name) = rest.split_once(':')?;
        if name.is_empty() {
            return None;
        }
        return Some(if package.is_empty() {
            name.to_owned()
        } else {
            format!("{package}/{name}")
        });
    }
    let mut path = target;
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    (!path.is_empty() && !path.contains(':')).then(|| path.to_owned())
}

pub fn owning_sets<'a>(registry: &'a Registry, target: &str) -> Vec<&'a str> {
    if target.starts_with('@') {
        return Vec::new();
    }
    if !target.contains("MODULE.bazel")
        && let Some(path) = declared_file_path(target)
    {
        let mut owners: Vec<&str> = registry
            .sets
            .iter()
            .filter(|set| {
                set.manifests
                    .iter()
                    .chain(&set.locks)
                    .any(|file| *file == path)
            })
            .map(|set| set.name.as_str())
            .collect();
        if !owners.is_empty() {
            owners.sort();
            return owners;
        }
    }
    let (mut package, recursive) = normalize_scope(target);
    if package == "." {
        package.clear();
    }
    if package.is_empty() {
        if recursive || target.contains("MODULE.bazel") {
            return registry.sets.iter().map(|set| set.name.as_str()).collect();
        }
        // A root-package label (`//:x`, `//:`, `.`) belongs only to sets scoped `.`.
        return registry
            .sets
            .iter()
            .filter(|set| set.scopes.iter().any(|scope| scope == "."))
            .map(|set| set.name.as_str())
            .collect();
    }
    let mut owners: BTreeSet<&str> = BTreeSet::new();
    for set in &registry.sets {
        for scope in &set.scopes {
            if scope_covers(scope, &package) {
                owners.insert(set.name.as_str());
                break;
            }
        }
    }
    if recursive {
        for set in &registry.sets {
            for scope in &set.scopes {
                if scope_covers(&package, scope) {
                    owners.insert(set.name.as_str());
                    break;
                }
            }
        }
    }
    let mut ordered: Vec<&str> = registry
        .sets
        .iter()
        .map(|set| set.name.as_str())
        .filter(|name| owners.contains(name))
        .collect();
    ordered.sort();
    ordered
}

/// Fails closed before any backend launches: manifests must exist and every
/// manifest and lock must stay inside the workspace after links resolve.
/// `need_lock` only controls whether a lock must exist; containment of a lock
/// is always checked. A link created after this check is not caught.
pub fn check_files(
    workspace: &Path,
    set: &ResolvedSet,
    need_lock: bool,
) -> Result<(), DependencySetsError> {
    let missing = |path: &String| DependencySetsError::MissingFile {
        name: set.name.clone(),
        path: path.clone(),
        rel: MODULE_BAZEL.to_owned(),
    };
    let outside = |path: &String| DependencySetsError::OutsideWorkspace {
        name: set.name.clone(),
        path: path.clone(),
        rel: MODULE_BAZEL.to_owned(),
    };
    for path in &set.manifests {
        resolve_existing(workspace, path, &missing, &outside)?;
    }
    for path in &set.locks {
        if need_lock {
            resolve_existing(workspace, path, &missing, &outside)?;
        } else {
            match dx_path::workspace::resolve_target_in_workspace(workspace, path) {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
                    return Err(outside(path));
                }
                Err(error) => {
                    return Err(DependencySetsError::Read {
                        rel: path.clone(),
                        detail: error.to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn resolve_existing(
    workspace: &Path,
    path: &String,
    missing: &dyn Fn(&String) -> DependencySetsError,
    outside: &dyn Fn(&String) -> DependencySetsError,
) -> Result<std::path::PathBuf, DependencySetsError> {
    match dx_path::workspace::resolve_in_workspace(workspace, path) {
        Ok(resolved) if resolved.is_file() => Ok(resolved),
        Ok(_) => Err(missing(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(missing(path)),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Err(outside(path)),
        Err(error) => Err(DependencySetsError::Read {
            rel: path.clone(),
            detail: error.to_string(),
        }),
    }
}

fn uv_workspace_root(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str::<toml::Table>(&text).ok())
        .and_then(|table| table.get("tool")?.get("uv")?.get("workspace").map(|_| ()))
        .is_some()
}

fn parent_dirs(dir: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut current = Some(dir);
    while let Some(here) = current {
        out.push(here);
        current = match here.rfind('/') {
            Some(index) => Some(&here[..index]),
            None if here != "." => Some("."),
            None => None,
        };
    }
    out
}

fn in_dir(dir: &str, file: &str) -> String {
    match dir {
        "." => file.to_owned(),
        _ => format!("{dir}/{file}"),
    }
}

/// Refuses a set whose backend would read settings or write files outside the set.
pub fn check_backend_settings(
    workspace: &Path,
    set: &ResolvedSet,
) -> Result<(), DependencySetsError> {
    match set.ecosystem {
        Ecosystem::Uv => check_uv_settings(workspace, set),
        Ecosystem::Npm => check_npm_layout(workspace, set),
        Ecosystem::Bazel => Ok(()),
        Ecosystem::Ruby => check_pinned_lock(set, GEMFILE_LOCK),
        Ecosystem::PowerShell => check_pinned_lock(set, PSGALLERY_LOCK),
        Ecosystem::Cpan => check_cpan_layout(workspace, set),
        Ecosystem::Pub => check_pinned_lock(set, PUBSPEC_LOCK),
        Ecosystem::Hex => match hex_backend(set) {
            Some(_) => Ok(()),
            None => check_pinned_lock(set, MIX_LOCK),
        },
        Ecosystem::Conan => check_pinned_lock(set, CONAN_LOCK),
        Ecosystem::Julia => check_julia_layout(workspace, set),
        Ecosystem::Composer => check_composer_layout(workspace, set),
        Ecosystem::Dub => check_dub_layout(workspace, set),
        Ecosystem::LuaRocks => check_pinned_lock(set, LUAROCKS_LOCK),
        Ecosystem::Opam => check_opam_layout(set),
        Ecosystem::Ansible => check_ansible_layout(set),
        Ecosystem::Bicep => Ok(()),
        Ecosystem::Cabal => check_cabal_layout(workspace, set),
        Ecosystem::SwiftPm => check_pinned_lock(set, PACKAGE_RESOLVED),
        Ecosystem::Cargo => {
            check_pinned_lock(set, CARGO_LOCK)?;
            check_cargo_hub(workspace, set).map(|_| ())
        }
        Ecosystem::Maven => check_maven_hub(workspace, set).map(|_| ()),
        Ecosystem::Nuget => check_nuget_layout(workspace, set),
        Ecosystem::Go => check_go_layout(workspace, set),
    }
}

/// A cabal set keeps `cabal.project.freeze` and `haskell_deps.json` on the same versions.
fn check_cabal_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    check_pinned_lock(set, CABAL_FREEZE)?;
    let Some(sidecar) = set.locks.iter().find(|lock| lock.ends_with(".json")) else {
        return Ok(());
    };
    let stale = |detail: String| DependencySetsError::StaleCabalLock {
        name: set.name.clone(),
        lock: sidecar.clone(),
        detail,
    };
    let freeze = in_dir(&set.dir, CABAL_FREEZE);
    let read = |file: &str| {
        std::fs::read_to_string(workspace.join(file))
            .map_err(|error| stale(format!("cannot read {file}: {error}")))
    };
    let freeze_text = read(&freeze)?;
    let json: serde_json::Value = serde_json::from_str(&read(sidecar)?)
        .map_err(|error| stale(format!("cannot parse it: {error}")))?;
    let pinned: std::collections::BTreeSet<String> = freeze_text
        .replace("constraints:", ",")
        .split([',', '\n'])
        .filter_map(|entry| {
            let (name, version) = entry.trim().strip_prefix("any.")?.split_once(" ==")?;
            Some(format!("{name} {}", version.trim()))
        })
        .collect();
    let packages = json
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for package in packages {
        let (Some(name), Some(version)) = (
            package.get("name").and_then(serde_json::Value::as_str),
            package.get("version").and_then(serde_json::Value::as_str),
        ) else {
            continue;
        };
        if !pinned.contains(&format!("{name} {version}")) {
            return Err(stale(format!(
                "it has {name} {version}, but {freeze} does not"
            )));
        }
    }
    Ok(())
}

/// The `.json` lock `cpan.install` reads, beside `cpanfile.snapshot`.
fn cpan_json_lock(set: &ResolvedSet) -> Option<&String> {
    set.locks.iter().find(|lock| lock.ends_with(".json"))
}

/// The `//pkg:name` of the `cpan_compiler` that writes the set's lock JSON.
///
/// Reads `BUILD.bazel` or `BUILD` in the lock's package; None when no call names the lock.
pub fn cpan_compiler_label(workspace: &Path, set: &ResolvedSet) -> Option<String> {
    let lock = cpan_json_lock(set)?;
    let (package, file) = match lock.rsplit_once('/') {
        Some((package, file)) => (package, file),
        None => ("", lock.as_str()),
    };
    let text = ["BUILD.bazel", "BUILD"]
        .iter()
        .find_map(|name| std::fs::read_to_string(workspace.join(package).join(name)).ok())?;
    let build = dx_module_bazel::ModuleFile::parse_build(&text).ok()?;
    build.calls_named("cpan_compiler").find_map(|call| {
        let named = call.str("lockfile")?;
        let named = named.rsplit([':', '/']).next().unwrap_or(named);
        (named == file).then(|| format!("//{package}:{}", call.str("name").unwrap_or_default()))
    })
}

/// The `//pkg:name` of the first `rule` call in the BUILD file of the set's directory.
pub fn julia_target_label(workspace: &Path, set: &ResolvedSet, rule: &str) -> Option<String> {
    target_label_in(workspace, &set.dir, rule)
}

/// The `//pkg:name` of the first `rule` call in the BUILD file of `dir`.
pub fn target_label_in(workspace: &Path, dir: &str, rule: &str) -> Option<String> {
    let package = if dir == "." { "" } else { dir };
    let text = ["BUILD.bazel", "BUILD"]
        .iter()
        .find_map(|name| std::fs::read_to_string(workspace.join(package).join(name)).ok())?;
    let build = dx_module_bazel::ModuleFile::parse_build(&text).ok()?;
    let name = build.calls_named(rule).find_map(|call| call.str("name"))?;
    Some(format!("//{package}:{name}"))
}

fn check_cpan_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    check_pinned_lock(set, CPANFILE_SNAPSHOT)?;
    let Some(lock) = cpan_json_lock(set) else {
        return Ok(());
    };
    let stale = |detail: String| DependencySetsError::StaleCpanLock {
        name: set.name.clone(),
        lock: lock.clone(),
        detail,
    };
    let snapshot = in_dir(&set.dir, CPANFILE_SNAPSHOT);
    let snapshot_text = std::fs::read_to_string(workspace.join(&snapshot))
        .map_err(|error| stale(format!("cannot read {snapshot}: {error}")))?;
    let lock_text = std::fs::read_to_string(workspace.join(lock))
        .map_err(|error| stale(format!("cannot read {lock}: {error}")))?;
    let json: serde_json::Value = serde_json::from_str(&lock_text)
        .map_err(|error| stale(format!("cannot parse it: {error}")))?;
    let in_lock: std::collections::BTreeSet<&str> = json
        .as_object()
        .map(|entries| {
            entries
                .values()
                .filter_map(|entry| entry.get("strip_prefix")?.as_str())
                .collect()
        })
        .unwrap_or_default();
    let mut in_snapshot = std::collections::BTreeSet::new();
    let mut distributions = false;
    for line in snapshot_text.lines() {
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            distributions = line.trim() == "DISTRIBUTIONS";
        } else if distributions && indent == 2 {
            in_snapshot.insert(line.trim());
        }
    }
    if let Some(missing) = in_snapshot.difference(&in_lock).next() {
        return Err(stale(format!(
            "{snapshot} has {missing}, which the lock lacks"
        )));
    }
    if let Some(extra) = in_lock.difference(&in_snapshot).next() {
        return Err(stale(format!("it has {extra}, which {snapshot} lacks")));
    }
    Ok(())
}

fn check_julia_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    check_pinned_lock(set, JULIA_MANIFEST)?;
    check_pinned_lock(set, JULIA_BAZEL_MANIFEST)?;
    let manifest = in_dir(&set.dir, JULIA_MANIFEST);
    let bazel = in_dir(&set.dir, JULIA_BAZEL_MANIFEST);
    let stale = |lock: &str, detail: String| DependencySetsError::StaleJuliaLock {
        name: set.name.clone(),
        lock: lock.to_owned(),
        detail,
    };
    let read = |file: &str| {
        std::fs::read_to_string(workspace.join(file))
            .map_err(|error| stale(file, format!("cannot read it: {error}")))
    };
    let table: toml::Table = toml::from_str(&read(&manifest)?)
        .map_err(|error| stale(&manifest, format!("cannot parse it: {error}")))?;
    let mut in_manifest = std::collections::BTreeMap::new();
    if let Some(deps) = table.get("deps").and_then(toml::Value::as_table) {
        for (name, entries) in deps {
            let versions = entries.as_array().into_iter().flatten();
            for version in versions.filter_map(|entry| entry.get("version")?.as_str()) {
                in_manifest.insert(name.as_str(), version);
            }
        }
    }
    let json: serde_json::Value = serde_json::from_str(&read(&bazel)?)
        .map_err(|error| stale(&bazel, format!("cannot parse it: {error}")))?;
    let in_bazel: std::collections::BTreeMap<&str, &str> = json
        .as_object()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(name, entry)| Some((name.as_str(), entry.get("version")?.as_str()?)))
                .collect()
        })
        .unwrap_or_default();
    for (name, version) in &in_manifest {
        match in_bazel.get(name) {
            Some(other) if other == version => {}
            Some(other) => {
                return Err(stale(
                    &bazel,
                    format!("it has {name} {other}, but {manifest} has {version}"),
                ));
            }
            None => {
                return Err(stale(
                    &bazel,
                    format!("{manifest} has {name}, which it lacks"),
                ));
            }
        }
    }
    if let Some(extra) = in_bazel
        .keys()
        .find(|name| !in_manifest.contains_key(*name))
    {
        return Err(stale(
            &bazel,
            format!("it has {extra}, which {manifest} lacks"),
        ));
    }
    Ok(())
}

/// A dub set keeps `dub.digests.json` on the versions of `dub.selections.json`.
fn check_dub_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    check_pinned_lock(set, DUB_SELECTIONS)?;
    let digests = in_dir(&set.dir, DUB_DIGESTS);
    if !set.locks.contains(&digests) {
        return Err(DependencySetsError::MissingPinnedLock {
            name: set.name.clone(),
            path: digests,
            ecosystem: set.ecosystem.name(),
        });
    }
    let stale = |detail: String| DependencySetsError::StaleDubLock {
        name: set.name.clone(),
        lock: digests.clone(),
        detail,
    };
    let read = |file: &str| -> Result<serde_json::Value, DependencySetsError> {
        let text = std::fs::read_to_string(workspace.join(file))
            .map_err(|error| stale(format!("cannot read {file}: {error}")))?;
        serde_json::from_str(&text).map_err(|error| stale(format!("cannot parse {file}: {error}")))
    };
    let selections = in_dir(&set.dir, DUB_SELECTIONS);
    let selected = read(&selections)?;
    let digested = read(&digests)?;
    let Some(versions) = selected
        .get("versions")
        .and_then(serde_json::Value::as_object)
    else {
        return Ok(());
    };
    for (name, version) in versions {
        let Some(version) = version.as_str() else {
            continue;
        };
        let have = digested
            .get("packages")
            .and_then(|packages| packages.get(name))
            .and_then(|entry| entry.get("version"))
            .and_then(serde_json::Value::as_str);
        match have {
            None => return Err(stale(format!("it has no digest for {name}"))),
            Some(have) if have != version => {
                return Err(stale(format!(
                    "its version of {name} is {have}, but {selections} has {version}"
                )));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn check_composer_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    check_pinned_lock(set, COMPOSER_LOCK)?;
    let Some(digests) = set.locks.iter().find(|lock| lock.ends_with(".json")) else {
        return Ok(());
    };
    let stale = |detail: String| DependencySetsError::StaleComposerLock {
        name: set.name.clone(),
        lock: digests.clone(),
        detail,
    };
    let lock = in_dir(&set.dir, COMPOSER_LOCK);
    let read = |file: &str| -> Result<serde_json::Value, DependencySetsError> {
        let text = std::fs::read_to_string(workspace.join(file))
            .map_err(|error| stale(format!("cannot read {file}: {error}")))?;
        serde_json::from_str(&text).map_err(|error| stale(format!("cannot parse {file}: {error}")))
    };
    let lock_json = read(&lock)?;
    let digests_json = read(digests)?;
    for key in ["packages", "packages-dev"] {
        let Some(entries) = lock_json.get(key).and_then(serde_json::Value::as_array) else {
            continue;
        };
        for entry in entries {
            let Some(name) = entry.get("name").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let wanted = entry
                .get("dist")
                .and_then(|dist| dist.get("reference"))
                .and_then(serde_json::Value::as_str);
            let have = digests_json
                .get("packages")
                .and_then(|packages| packages.get(name))
                .and_then(|digest| digest.get("reference"))
                .and_then(serde_json::Value::as_str);
            match (wanted, have) {
                (_, None) => return Err(stale(format!("it has no digest for {name}"))),
                (Some(wanted), Some(have)) if wanted != have => {
                    return Err(stale(format!(
                        "its reference for {name} is {have}, but {lock} has {wanted}"
                    )));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// The name `@<name>` resolves the set's crate_universe hub by in the root module.
///
/// This is the `use_repo` name, which differs from the hub's own `name` (the value
/// `CARGO_BAZEL_REPIN_ONLY` must carry) when the module renames it. None when the set
/// is not a valid cargo set; callers run `check_backend_settings` first.
pub fn cargo_fetch_repository(workspace: &Path, set: &ResolvedSet) -> Option<String> {
    check_cargo_hub(workspace, set).ok()
}

fn check_cargo_hub(workspace: &Path, set: &ResolvedSet) -> Result<String, DependencySetsError> {
    let repository = set.repository.clone().unwrap_or_default();
    let unsupported = |detail: String| DependencySetsError::UnsupportedCargoHub {
        name: set.name.clone(),
        repository: repository.clone(),
        detail,
    };
    let root = std::fs::read_to_string(workspace.join(MODULE_BAZEL))
        .map_err(|error| unsupported(format!("cannot read {MODULE_BAZEL}: {error}")))?;
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(workspace.join(rel))
            .map_err(|error| format!("cannot read included {label}: {error}"))
    };
    let modules = dx_module_bazel::load_with_includes(&root, &read_include).map_err(unsupported)?;
    let mut hubs = Vec::new();
    for (index, module) in modules.iter().enumerate() {
        let found = module
            .cargo_hubs()
            .map_err(|error| unsupported(error.to_string()))?;
        hubs.extend(
            found
                .into_iter()
                .filter(|hub| hub.name == repository)
                .map(|hub| (index, hub)),
        );
    }
    let (index, hub) = match hubs.as_slice() {
        [] => {
            return Err(unsupported(format!(
                "no crate.from_cargo(name = {repository:?}) tag in {MODULE_BAZEL}"
            )));
        }
        [one] => one,
        _ => {
            return Err(unsupported(format!(
                "{} crate.from_cargo tags are named {repository:?}",
                hubs.len()
            )));
        }
    };
    let visible = std::iter::once(&modules[*index])
        .chain(
            modules
                .iter()
                .enumerate()
                .filter(|(i, _)| i != index)
                .map(|(_, m)| m),
        )
        .find_map(|module| module.cargo_apparent(hub))
        .ok_or_else(|| {
            unsupported(format!(
                "{MODULE_BAZEL} must list {repository:?} in use_repo of the crate extension"
            ))
        })?;
    let label = hub.cargo_lockfile.ok_or_else(|| {
        unsupported("the crate.from_cargo tag needs a literal cargo_lockfile".to_owned())
    })?;
    let cargo_lock = dx_module_bazel::label_path(label).map_err(unsupported)?;
    if cargo_lock != in_dir(&set.dir, CARGO_LOCK) {
        return Err(unsupported(format!(
            "cargo_lockfile {label} is not the set's {} (want {cargo_lock})",
            in_dir(&set.dir, CARGO_LOCK)
        )));
    }
    if let Some(label) = hub.lockfile {
        let lock = dx_module_bazel::label_path(label).map_err(unsupported)?;
        if !set.locks.contains(&lock) {
            return Err(unsupported(format!(
                "lockfile {label} is not listed in the set's locks (want {lock})"
            )));
        }
    }
    let hub_manifests = hub
        .manifests
        .iter()
        .map(|manifest| dx_module_bazel::label_path(&manifest.value).map_err(unsupported))
        .collect::<Result<BTreeSet<_>, _>>()?;
    for manifest in &set.manifests {
        if !hub_manifests.contains(manifest) {
            return Err(unsupported(format!(
                "{manifest} is not one of the manifests of crate.from_cargo({repository:?})"
            )));
        }
    }
    Ok(visible.to_owned())
}

const MAVEN_SIGNATURES: [&str; 2] = ["__INPUT_ARTIFACTS_HASH", "__RESOLVED_ARTIFACTS_HASH"];

/// What check mode needs from the maven hub beyond what both modes need.
#[derive(Clone, Debug, Eq, PartialEq)]
struct MavenHub {
    lock: String,
    policy: dx_module_bazel::RepinPolicy,
}

fn check_maven_hub(workspace: &Path, set: &ResolvedSet) -> Result<MavenHub, DependencySetsError> {
    let repository = set.repository.clone().unwrap_or_default();
    let unsupported = |detail: String| DependencySetsError::UnsupportedMavenHub {
        name: set.name.clone(),
        repository: repository.clone(),
        detail,
    };
    let root = std::fs::read_to_string(workspace.join(MODULE_BAZEL))
        .map_err(|error| unsupported(format!("cannot read {MODULE_BAZEL}: {error}")))?;
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(workspace.join(rel))
            .map_err(|error| format!("cannot read included {label}: {error}"))
    };
    let modules = dx_module_bazel::load_with_includes(&root, &read_include).map_err(unsupported)?;
    let mut found = Vec::new();
    let mut used = false;
    for module in &modules {
        let installs = module
            .maven_installs()
            .map_err(|error| unsupported(error.to_string()))?;
        for install in installs {
            if install.name == repository {
                found.push((
                    install.lock_file.map(str::to_owned),
                    install.fail_if_repin_required,
                ));
            }
        }
        used |= module.calls_named("use_repo").any(|call| {
            call.positional.iter().skip(1).any(|value| {
                matches!(value, dx_module_bazel::Value::Str(literal) if literal.value == repository)
            })
        });
    }
    let (lock_file, policy) = match found.as_slice() {
        [] => {
            return Err(unsupported(format!(
                "no maven.install(name = {repository:?}) tag in {MODULE_BAZEL}"
            )));
        }
        [one] => one.clone(),
        _ => {
            return Err(unsupported(format!(
                "{} maven.install tags are named {repository:?}",
                found.len()
            )));
        }
    };
    if !used {
        return Err(unsupported(format!(
            "{MODULE_BAZEL} must list {repository:?} in use_repo without renaming it"
        )));
    }
    let label = lock_file
        .ok_or_else(|| unsupported("the maven.install tag needs a literal lock_file".to_owned()))?;
    let lock = dx_module_bazel::label_path(&label).map_err(unsupported)?;
    if !set.locks.contains(&lock) {
        return Err(unsupported(format!(
            "lock_file {label} is not listed in the set's locks (want {lock})"
        )));
    }
    if !workspace.join(&lock).is_file() {
        return Err(unsupported(format!(
            "{lock} does not exist; create an empty file before the first update"
        )));
    }
    Ok(MavenHub { lock, policy })
}

/// Refuses a check when the pinned state cannot prove a stale lock fails closed.
///
/// rules_jvm_external only fails a stale lock when `fail_if_repin_required` is true and the
/// lock carries its signatures; an empty or old-format lock merely prints a note.
pub fn check_lock_ready(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    match set.ecosystem {
        Ecosystem::Maven => {
            let hub = check_maven_hub(workspace, set)?;
            let unsupported = |detail: String| DependencySetsError::UnsupportedMavenHub {
                name: set.name.clone(),
                repository: set.repository.clone().unwrap_or_default(),
                detail,
            };
            match hub.policy {
                dx_module_bazel::RepinPolicy::Unset | dx_module_bazel::RepinPolicy::True => {}
                _ => {
                    return Err(unsupported(
                        "fail_if_repin_required must stay true so a stale lock fails the check"
                            .to_owned(),
                    ));
                }
            }
            let text = std::fs::read_to_string(workspace.join(&hub.lock))
                .map_err(|error| unsupported(format!("cannot read {}: {error}", hub.lock)))?;
            let signed = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|json| {
                    let object = json.as_object()?.clone();
                    Some(MAVEN_SIGNATURES.iter().all(|key| object.contains_key(*key)))
                })
                .unwrap_or(false);
            if !signed {
                return Err(unsupported(format!(
                    "{} has no {} signatures; run dx update --apply first",
                    hub.lock,
                    MAVEN_SIGNATURES.join(" and ")
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Refuses a nuget set whose `paket2bazel` run could not stay inside the declared locks.
fn check_nuget_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let unsupported = |detail: String| DependencySetsError::UnsupportedNugetLayout {
        name: set.name.clone(),
        detail,
    };
    check_pinned_lock(set, PAKET_LOCK)?;
    let mut folders = BTreeSet::new();
    let mut groups = 0;
    for lock in &set.locks {
        let (folder, file) = lock.rsplit_once('/').unwrap_or((".", lock));
        if file == PAKET_LOCK {
            continue;
        }
        let group = file
            .strip_prefix("paket.")
            .and_then(|rest| rest.strip_suffix(".bzl"))
            .ok_or_else(|| {
                unsupported(format!(
                    "lock {lock} is neither paket.lock nor a generated paket.<group>.bzl file"
                ))
            })?;
        if group.is_empty() {
            return Err(unsupported(format!("lock {lock} names no paket group")));
        }
        if group.ends_with("_extension") {
            return Err(unsupported(format!(
                "lock {lock} is a paket2bazel extension; dx does not manage it, so drop it from locks"
            )));
        }
        folders.insert(folder.to_owned());
        groups += 1;
    }
    if groups == 0 {
        return Err(unsupported(
            "locks list no generated paket.<group>.bzl files".to_owned(),
        ));
    }
    if folders.len() != 1 {
        return Err(unsupported(
            "generated paket .bzl locks must share one folder".to_owned(),
        ));
    }
    for manifest in &set.manifests {
        let text = std::fs::read_to_string(workspace.join(manifest))
            .map_err(|error| unsupported(format!("cannot read {manifest}: {error}")))?;
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("source ") {
                let rest = rest.trim();
                if !(rest.starts_with("http://") || rest.starts_with("https://")) {
                    return Err(unsupported(format!(
                        "{manifest} source {rest:?} is not http(s); staging copies only paket.dependencies and paket.lock"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Refuses a go set whose staged `go mod download all` could not stay inside the declared locks.
fn check_go_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let unsupported = |detail: String| DependencySetsError::UnsupportedGoLayout {
        name: set.name.clone(),
        detail,
    };
    check_pinned_lock(set, GO_SUM)?;
    let sum = in_dir(&set.dir, GO_SUM);
    if set.locks.len() != 1 || set.locks[0] != sum {
        return Err(unsupported(format!(
            "locks must list exactly {sum} (the run writes go.sum only)"
        )));
    }
    let [manifest] = set.manifests.as_slice() else {
        return Err(unsupported(
            "a go set needs exactly one go.mod manifest".to_owned(),
        ));
    };
    let text = std::fs::read_to_string(workspace.join(manifest))
        .map_err(|error| unsupported(format!("cannot read {manifest}: {error}")))?;
    let parsed = dx_gomod::parse_mod(&text)
        .map_err(|error| unsupported(format!("cannot parse {manifest}: {error}")))?;
    for replacement in &parsed.replace {
        if let dx_gomod::GoReplacementTarget::FilePath(path) = &replacement.target {
            return Err(unsupported(format!(
                "{manifest} replaces {} with local path {path}; staging copies only go.mod and go.sum",
                replacement.module
            )));
        }
    }
    let root = std::fs::read_to_string(workspace.join(MODULE_BAZEL))
        .map_err(|error| unsupported(format!("cannot read {MODULE_BAZEL}: {error}")))?;
    let module = dx_module_bazel::ModuleFile::parse(&root)
        .map_err(|error| unsupported(format!("cannot parse {MODULE_BAZEL}: {error}")))?;
    let dep = module.bazel_dep("rules_go").map_err(|_| {
        unsupported(format!(
            "{MODULE_BAZEL} needs bazel_dep(name = \"rules_go\")"
        ))
    })?;
    if dep.arg("repo_name").is_some() {
        return Err(unsupported(
            "the rules_go bazel_dep must not set repo_name".to_owned(),
        ));
    }
    if root.lines().any(|line| {
        let code = line.split('#').next().unwrap_or("");
        code.contains("go_env")
    }) {
        return Err(unsupported(
            "go sets use the default public proxy and checksum database and do not read go_deps.config(go_env=...); private modules are not supported"
                .to_owned(),
        ));
    }
    Ok(())
}

fn check_pinned_lock(set: &ResolvedSet, file: &str) -> Result<(), DependencySetsError> {
    let lock = in_dir(&set.dir, file);
    if set.locks.contains(&lock) {
        return Ok(());
    }
    Err(DependencySetsError::MissingPinnedLock {
        name: set.name.clone(),
        path: lock,
        ecosystem: set.ecosystem.name(),
    })
}

/// An ansible set has exactly the one collections lock its tag names.
fn check_ansible_layout(set: &ResolvedSet) -> Result<(), DependencySetsError> {
    if set.locks.len() == 1 && set.locks[0].ends_with(".json") {
        return Ok(());
    }
    Err(DependencySetsError::MissingPinnedLock {
        name: set.name.clone(),
        path: "lock.json".to_owned(),
        ecosystem: set.ecosystem.name(),
    })
}

/// An opam set keeps `<name>.opam.locked` beside its `<name>.opam`.
fn check_opam_layout(set: &ResolvedSet) -> Result<(), DependencySetsError> {
    for manifest in &set.manifests {
        let lock = format!("{manifest}.locked");
        if !set.locks.contains(&lock) {
            return Err(DependencySetsError::MissingPinnedLock {
                name: set.name.clone(),
                path: lock,
                ecosystem: set.ecosystem.name(),
            });
        }
    }
    Ok(())
}

/// The package manager a configured npm set runs, chosen by the lock its `locks` list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NpmBackend {
    Pnpm,
    Yarn,
    Npm,
}

/// Picks the backend from the locks beside the manifest; pnpm wins over yarn over npm.
pub fn npm_backend(set: &ResolvedSet) -> Option<NpmBackend> {
    [
        (PNPM_LOCK, NpmBackend::Pnpm),
        (YARN_LOCK, NpmBackend::Yarn),
        (NPM_LOCK, NpmBackend::Npm),
    ]
    .into_iter()
    .find(|(file, _)| set.locks.contains(&in_dir(&set.dir, file)))
    .map(|(_, backend)| backend)
}

/// The package manager a configured hex set runs, chosen by the lock its `locks` list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HexBackend {
    Mix,
    Rebar3,
}

/// Picks the backend from the locks beside the manifest: mix.lock for mix, rebar.lock for rebar3.
pub fn hex_backend(set: &ResolvedSet) -> Option<HexBackend> {
    [
        (MIX_LOCK, HexBackend::Mix),
        (REBAR_LOCK, HexBackend::Rebar3),
    ]
    .into_iter()
    .find(|(file, _)| set.locks.contains(&in_dir(&set.dir, file)))
    .map(|(_, backend)| backend)
}

fn check_npm_layout(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let lock = in_dir(&set.dir, PNPM_LOCK);
    let unsupported = |detail: String| DependencySetsError::UnsupportedNpmLayout {
        name: set.name.clone(),
        dir: set.dir.clone(),
        detail,
    };
    let backend = npm_backend(set).ok_or_else(|| {
        unsupported(format!(
            "locks must include {lock}, {}, or {}",
            in_dir(&set.dir, YARN_LOCK),
            in_dir(&set.dir, NPM_LOCK)
        ))
    })?;
    if backend != NpmBackend::Pnpm {
        return Ok(());
    }
    match nearest_parent_pnpm_workspace(workspace, set) {
        None => Ok(()),
        Some((_, true)) => Ok(()),
        Some((config, false)) => Err(unsupported(format!(
            "{config} makes pnpm write the parent workspace lock"
        ))),
    }
}

/// The nearest `pnpm-workspace.yaml` above a pnpm set, as `(path, is_root_only)`.
///
/// A set with its own workspace file is its own root and has no parent to find.
/// `is_root_only` is true when the parent lists only `.` as a package, so the
/// set is not a member and pnpm needs `--ignore-workspace` to stay out of it.
fn nearest_parent_pnpm_workspace(workspace: &Path, set: &ResolvedSet) -> Option<(String, bool)> {
    if workspace.join(in_dir(&set.dir, PNPM_WORKSPACE)).is_file() {
        return None;
    }
    for dir in parent_dirs(&set.dir).into_iter().skip(1) {
        let config = in_dir(dir, PNPM_WORKSPACE);
        if let Ok(text) = std::fs::read_to_string(workspace.join(&config)) {
            let root_only = pnpm_workspace_packages(&text) == [".".to_owned()];
            return Some((config, root_only));
        }
    }
    None
}

/// Whether pnpm runs for this set need `--ignore-workspace`: the set has no
/// workspace file of its own and the nearest parent workspace lists only `.`.
pub fn npm_ignore_workspace(workspace: &Path, set: &ResolvedSet) -> bool {
    set.ecosystem == Ecosystem::Npm
        && npm_backend(set) == Some(NpmBackend::Pnpm)
        && nearest_parent_pnpm_workspace(workspace, set).is_some_and(|(_, root_only)| root_only)
}

/// The `packages` globs of a `pnpm-workspace.yaml`, read without a YAML parser.
fn pnpm_workspace_packages(text: &str) -> Vec<String> {
    let clean = |item: &str| {
        item.split(" #")
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(|c| c == '"' || c == '\'')
            .to_owned()
    };
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("packages:") else {
            continue;
        };
        let rest = rest.split(" #").next().unwrap_or("").trim();
        if let Some(inline) = rest.strip_prefix('[') {
            let inline = inline.trim_end_matches(']');
            out.extend(inline.split(',').map(clean).filter(|item| !item.is_empty()));
            break;
        }
        for item in lines.by_ref() {
            let trimmed = item.trim();
            if let Some(entry) = trimmed.strip_prefix("- ") {
                out.push(clean(entry));
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                break;
            }
        }
        break;
    }
    out
}

fn check_uv_settings(workspace: &Path, set: &ResolvedSet) -> Result<(), DependencySetsError> {
    let mut dir = Some(set.dir.as_str());
    while let Some(current) = dir {
        let prefix = match current {
            "." => String::new(),
            _ => format!("{current}/"),
        };
        let config = format!("{prefix}uv.toml");
        if workspace.join(&config).is_file() {
            return Err(DependencySetsError::UnsupportedSettings {
                name: set.name.clone(),
                detail: format!("{config} exists"),
            });
        }
        if current != set.dir {
            let parent = format!("{prefix}pyproject.toml");
            if uv_workspace_root(&workspace.join(&parent)) {
                return Err(DependencySetsError::UnsupportedSettings {
                    name: set.name.clone(),
                    detail: format!("{parent} sets [tool.uv.workspace]"),
                });
            }
        }
        dir = match current.rfind('/') {
            Some(index) => Some(&current[..index]),
            None if current != "." => Some("."),
            None => None,
        };
    }
    for manifest in &set.manifests {
        let resolved =
            dx_path::workspace::resolve_in_workspace(workspace, manifest).map_err(|error| {
                if error.kind() == std::io::ErrorKind::InvalidInput {
                    DependencySetsError::OutsideWorkspace {
                        name: set.name.clone(),
                        path: manifest.clone(),
                        rel: MODULE_BAZEL.to_owned(),
                    }
                } else {
                    DependencySetsError::Read {
                        rel: manifest.clone(),
                        detail: error.to_string(),
                    }
                }
            })?;
        let bytes = std::fs::read(resolved).map_err(|error| DependencySetsError::Read {
            rel: manifest.clone(),
            detail: error.to_string(),
        })?;
        let text = String::from_utf8(bytes).map_err(|_| DependencySetsError::NotUtf8 {
            rel: manifest.clone(),
        })?;
        let table: toml::Table =
            toml::from_str(&text).map_err(|error| DependencySetsError::InvalidToml {
                rel: manifest.clone(),
                detail: error.to_string(),
            })?;
        let unsupported: Vec<&str> = table
            .get("tool")
            .and_then(|tool| tool.get("uv"))
            .and_then(toml::Value::as_table)
            .map(|uv| {
                uv.keys()
                    .map(String::as_str)
                    .filter(|key| !UV_PROJECT_KEYS.contains(key))
                    .collect()
            })
            .unwrap_or_default();
        if !unsupported.is_empty() {
            return Err(DependencySetsError::UnsupportedSettings {
                name: set.name.clone(),
                detail: format!("{manifest} sets [tool.uv] {}", unsupported.join(", ")),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "dependency_sets_tests.rs"]
mod tests;
