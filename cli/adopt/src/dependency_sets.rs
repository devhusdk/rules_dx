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

pub const ECOSYSTEM_OPAM: &str = "opam";

pub const ECOSYSTEM_SWIFTPM: &str = "swiftpm";

pub const ECOSYSTEM_CABAL: &str = "cabal";

pub const SUPPORTED_ECOSYSTEMS: &str = "uv, npm, bazel, ruby, powershell, cargo, maven, nuget, go, cpan, pub, conan, julia, hex, composer, opam, swiftpm, cabal";

pub const MODULE_BAZEL: &str = "MODULE.bazel";

pub const CARGO_LOCK: &str = "Cargo.lock";

pub const GEMFILE_LOCK: &str = "Gemfile.lock";

pub const GO_MOD: &str = "go.mod";

pub const GO_SUM: &str = "go.sum";

pub const CPANFILE: &str = "cpanfile";

pub const CPANFILE_SNAPSHOT: &str = "cpanfile.snapshot";

pub const MIX_EXS: &str = "mix.exs";

pub const MIX_LOCK: &str = "mix.lock";

pub const PUBSPEC_YAML: &str = "pubspec.yaml";

pub const PUBSPEC_LOCK: &str = "pubspec.lock";

const CONANFILES: [&str; 2] = ["conanfile.txt", "conanfile.py"];

pub const CONAN_LOCK: &str = "conan.lock";

pub const PROJECT_TOML: &str = "Project.toml";

pub const JULIA_MANIFEST: &str = "Manifest.toml";

pub const JULIA_BAZEL_MANIFEST: &str = "Manifest.bazel.json";
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
    Opam,
    SwiftPm,
    Cabal,
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
            Ecosystem::Opam => ECOSYSTEM_OPAM,
            Ecosystem::Cabal => ECOSYSTEM_CABAL,
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
            ECOSYSTEM_OPAM => Some(Ecosystem::Opam),
            ECOSYSTEM_CABAL => Some(Ecosystem::Cabal),
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
            | Ecosystem::Opam
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
            | Ecosystem::Opam
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
            | Ecosystem::Opam
            | Ecosystem::SwiftPm
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
            Ecosystem::Maven => MODULE_BAZEL,
            Ecosystem::Nuget => "paket.dependencies",
            Ecosystem::Go => GO_MOD,
            Ecosystem::Cpan => CPANFILE,
            Ecosystem::Pub => PUBSPEC_YAML,
            Ecosystem::Conan => "conanfile.txt",
            Ecosystem::Julia => PROJECT_TOML,
            Ecosystem::Hex => MIX_EXS,
            Ecosystem::Composer => COMPOSER_JSON,
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
            | Ecosystem::Opam
            | Ecosystem::SwiftPm
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
            let deps = path(haskell.deps)?;
            let dir = manifest_dir(std::slice::from_ref(&deps));
            sets.push(hub_set(
                haskell.name,
                Ecosystem::Cabal,
                vec![in_dir(&dir, CABAL_PROJECT)],
                vec![in_dir(&dir, CABAL_FREEZE), deps],
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
            Ecosystem::Opam => file.ends_with(".opam") && file != ".opam",
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
        let manifests: &[String] = if set.ecosystem == Ecosystem::Maven {
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
        | Ecosystem::Opam
        | Ecosystem::Cabal
        | Ecosystem::SwiftPm
        | Ecosystem::Nuget => {
            let charset = match ecosystem {
                Ecosystem::Nuget => NUGET_PACKAGE_CHARSET,
                Ecosystem::Bazel => BAZEL_PACKAGE_CHARSET,
                Ecosystem::Ruby => RUBY_PACKAGE_CHARSET,
                Ecosystem::PowerShell => POWERSHELL_PACKAGE_CHARSET,
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
        Ecosystem::Hex => check_pinned_lock(set, MIX_LOCK),
        Ecosystem::Conan => check_pinned_lock(set, CONAN_LOCK),
        Ecosystem::Julia => check_julia_layout(workspace, set),
        Ecosystem::Composer => check_composer_layout(workspace, set),
        Ecosystem::Opam => check_opam_layout(set),
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
    let package = if set.dir == "." { "" } else { set.dir.as_str() };
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
mod tests {
    use super::*;

    type Spec<'a> = (&'a str, Ecosystem, &'a [&'a str], &'a [&'a str]);

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn try_registry(specs: &[Spec<'_>]) -> Result<Registry, DependencySetsError> {
        let mut sets = Vec::new();
        for (name, ecosystem, manifests, locks) in specs {
            sets.push(hub_set(
                name,
                *ecosystem,
                strings(manifests),
                strings(locks),
            )?);
        }
        check_conflicts(&sets)?;
        sets.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Registry { sets })
    }

    fn registry(specs: &[Spec<'_>]) -> Registry {
        try_registry(specs).expect("valid registry")
    }

    fn two_uv() -> Registry {
        registry(&[
            (
                "frontend",
                Ecosystem::Uv,
                &["apps/frontend/pyproject.toml"],
                &["apps/frontend/uv.lock"],
            ),
            (
                "worker",
                Ecosystem::Uv,
                &["services/worker/pyproject.toml"],
                &["services/worker/uv.lock"],
            ),
        ])
    }

    fn discovered(
        root: &str,
        files: &[(&str, &str)],
    ) -> Result<Option<Registry>, DependencySetsError> {
        let files: BTreeMap<String, String> = files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect();
        discover(root, &|rel: &str| {
            files
                .get(rel)
                .cloned()
                .ok_or_else(|| format!("{rel}: missing"))
        })
    }

    fn found(root: &str, files: &[(&str, &str)]) -> Registry {
        discovered(root, files)
            .expect("valid hubs")
            .expect("hubs present")
    }

    const CRATE_EXT: &str =
        "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n";
    const MAVEN_EXT: &str =
        "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n";
    const NPM_EXT: &str =
        "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n";
    const UV_EXT: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\n";

    #[test]
    fn cargo_hubs_become_sets_with_the_lock_dir_manifest_first() {
        let module = format!(
            "{CRATE_EXT}crate.from_cargo(name = \"dx_crates\", cargo_lockfile = \"//b:Cargo.lock\", lockfile = \"//:cargo-bazel-lock.json\", manifests = [\"//a:Cargo.toml\", \"//b:Cargo.toml\"])\ncrate.from_cargo(name = \"nolock\", manifests = [\"//c:Cargo.toml\"])\n"
        );
        let registry = found(&module, &[]);
        assert_eq!(registry.names(), vec!["dx_crates"]);
        let set = registry.find("dx_crates").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Cargo);
        assert_eq!(set.manifests, ["b/Cargo.toml", "a/Cargo.toml"]);
        assert_eq!(set.locks, ["b/Cargo.lock", "cargo-bazel-lock.json"]);
        assert_eq!(set.scopes, [".", "a", "b"]);
        assert_eq!(set.repository.as_deref(), Some("dx_crates"));
        assert_eq!(ResolvedSet::of(set).dir, "b");
    }

    #[test]
    fn maven_installs_with_a_lock_file_become_sets() {
        let module = format!(
            "{MAVEN_EXT}maven.install(lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"nolock\")\n"
        );
        let registry = found(&module, &[]);
        assert_eq!(registry.names(), vec!["maven"]);
        let set = registry.find("maven").expect("maven");
        assert_eq!(set.ecosystem, Ecosystem::Maven);
        assert_eq!(set.manifests, [MODULE_BAZEL]);
        assert_eq!(set.locks, ["third_party/maven_install.json"]);
        assert_eq!(set.scopes, ["third_party"]);
        assert_eq!(set.repository.as_deref(), Some("maven"));
    }

    #[test]
    fn npm_translate_locks_become_sets() {
        let module = format!(
            "{NPM_EXT}npm.npm_translate_lock(name = \"web\", pnpm_lock = \"//apps/web:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"bare\")\n"
        );
        let registry = found(&module, &[]);
        assert_eq!(registry.names(), vec!["web"]);
        let set = registry.find("web").expect("web");
        assert_eq!(set.ecosystem, Ecosystem::Npm);
        assert_eq!(set.manifests, ["apps/web/package.json"]);
        assert_eq!(set.locks, ["apps/web/pnpm-lock.yaml"]);
        assert_eq!(set.repository, None);
    }

    #[test]
    fn uv_projects_are_named_by_hub_and_directory() {
        let module = format!(
            "{UV_EXT}uv.project(hub_name = \"pypi\", lock = \"//apps/api:uv.lock\", pyproject = \"//apps/api:pyproject.toml\")\nuv.project(hub_name = \"pypi\", lock = \"//:uv.lock\", pyproject = \"//:pyproject.toml\")\n"
        );
        let registry = found(&module, &[]);
        assert_eq!(registry.names(), vec!["pypi", "pypi-api"]);
        let api = registry.find("pypi-api").expect("api");
        assert_eq!(api.ecosystem, Ecosystem::Uv);
        assert_eq!(api.manifests, ["apps/api/pyproject.toml"]);
        assert_eq!(api.locks, ["apps/api/uv.lock"]);
        assert_eq!(registry.find("pypi").expect("root").scopes, ["."]);
    }

    #[test]
    fn go_deps_files_become_the_go_set() {
        let module = "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n";
        let registry = found(module, &[]);
        let set = registry.find("go").expect("go");
        assert_eq!(set.ecosystem, Ecosystem::Go);
        assert_eq!(set.manifests, ["third_party/go/go.mod"]);
        assert_eq!(set.locks, ["third_party/go/go.sum"]);
    }

    #[test]
    fn opam_packages_become_opam_sets() {
        let module = "ocaml = use_extension(\"//ocaml/toolchain:repos.bzl\", \"ocaml\")\nocaml.packages(name = \"app_opam\", lock = \"//ocaml/app:app.opam.locked\")\n";
        let registry = found(module, &[]);
        let set = registry.find("app_opam").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Opam);
        assert_eq!(set.manifests, ["ocaml/app/app.opam"]);
        assert_eq!(set.locks, ["ocaml/app/app.opam.locked"]);
        assert!(check_opam_layout(&ResolvedSet::of(set)).is_ok());
    }

    #[test]
    fn haskell_packages_become_cabal_sets() {
        let module = "haskell = use_extension(\"//haskell/toolchain:repos.bzl\", \"haskell\")\nhaskell.packages(name = \"hackage\", deps = \"//third_party:haskell_deps.json\")\n";
        let registry = found(module, &[]);
        let set = registry.find("hackage").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Cabal);
        assert_eq!(set.manifests, ["third_party/cabal.project"]);
        assert_eq!(
            set.locks,
            [
                "third_party/cabal.project.freeze",
                "third_party/haskell_deps.json"
            ]
        );
    }

    #[test]
    fn cabal_layout_compares_freeze_and_sidecar_versions() {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-cabal-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("hs")).expect("dir");
        std::fs::write(
            root.join("hs/cabal.project.freeze"),
            "constraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n",
        )
        .expect("freeze");
        let sidecar = |safe: &str| {
            format!(
                "{{\"packages\":[{{\"name\":\"base\",\"version\":\"4.20.2.0\"}},{{\"name\":\"safe\",\"version\":\"{safe}\"}}]}}"
            )
        };
        let set = ResolvedSet {
            name: "hackage".to_owned(),
            ecosystem: Ecosystem::Cabal,
            dir: "hs".to_owned(),
            manifests: vec!["hs/cabal.project".to_owned()],
            locks: vec![
                "hs/cabal.project.freeze".to_owned(),
                "hs/haskell_deps.json".to_owned(),
            ],
            repository: None,
        };
        std::fs::write(root.join("hs/haskell_deps.json"), sidecar("0.3.21")).expect("sidecar");
        assert!(check_cabal_layout(root, &set).is_ok());
        std::fs::write(root.join("hs/haskell_deps.json"), sidecar("0.3.20")).expect("sidecar");
        let message = check_cabal_layout(root, &set)
            .expect_err("stale")
            .to_string();
        assert!(message.contains("safe 0.3.20"), "{message}");
    }

    #[test]
    fn cpan_installs_become_cpan_sets() {
        let module = "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"perl_cpan\", lock = \"//perl/tools:cpanfile.snapshot.lock.json\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("perl_cpan").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Cpan);
        assert_eq!(set.manifests, ["perl/tools/cpanfile"]);
        assert_eq!(
            set.locks,
            [
                "perl/tools/cpanfile.snapshot",
                "perl/tools/cpanfile.snapshot.lock.json"
            ]
        );
    }

    const CPAN_SNAPSHOT: &str = "# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  Try-Tiny-0.31\n    pathname: E/ET/ETHER/Try-Tiny-0.31.tar.gz\n";

    fn cpan_workspace(lock: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-cpan-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("perl")).expect("dir");
        std::fs::write(root.join("perl/cpanfile.snapshot"), CPAN_SNAPSHOT).expect("snapshot");
        std::fs::write(root.join("perl/cpanfile.snapshot.lock.json"), lock).expect("lock");
        std::fs::write(
            root.join("perl/BUILD.bazel"),
            "load(\"@rules_perl//perl/cpan:cpan_compiler.bzl\", \"cpan_compiler\")\n\ncpan_compiler(\n    name = \"compiler\",\n    cpanfile = \"cpanfile\",\n    lockfile = \"cpanfile.snapshot.lock.json\",\n)\n",
        )
        .expect("build");
        let set = ResolvedSet {
            name: "perl_cpan".to_owned(),
            ecosystem: Ecosystem::Cpan,
            dir: "perl".to_owned(),
            manifests: vec!["perl/cpanfile".to_owned()],
            locks: vec![
                "perl/cpanfile.snapshot".to_owned(),
                "perl/cpanfile.snapshot.lock.json".to_owned(),
            ],
            repository: None,
        };
        (scratch, set)
    }

    #[test]
    fn cpan_compiler_label_reads_the_build_file_beside_the_lock() {
        let (scratch, set) = cpan_workspace("{}");
        assert_eq!(
            cpan_compiler_label(scratch.path(), &set).as_deref(),
            Some("//perl:compiler")
        );
        std::fs::write(scratch.path().join("perl/BUILD.bazel"), "").expect("build");
        assert_eq!(cpan_compiler_label(scratch.path(), &set), None);
    }

    #[test]
    fn cpan_layout_accepts_a_lock_that_matches_the_snapshot() {
        let (scratch, set) = cpan_workspace("{\"Try-Tiny\":{\"strip_prefix\":\"Try-Tiny-0.31\"}}");
        assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
    }

    #[test]
    fn cpan_layout_rejects_a_stale_lock_json() {
        let (scratch, set) = cpan_workspace("{\"Try-Tiny\":{\"strip_prefix\":\"Try-Tiny-0.30\"}}");
        let error = check_backend_settings(scratch.path(), &set).expect_err("stale");
        let message = error.to_string();
        assert!(message.contains("Try-Tiny-0.31"), "{message}");
        assert!(message.contains("run the cpan_compiler"), "{message}");
    }

    fn composer_workspace(digest_reference: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-composer-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("php")).expect("dir");
        std::fs::write(
            root.join("php/composer.lock"),
            "{\"packages\":[{\"name\":\"psr/log\",\"version\":\"3.0.0\",\"dist\":{\"reference\":\"aaa\"}}],\"packages-dev\":[]}",
        )
        .expect("lock");
        std::fs::write(
            root.join("php/composer.digests.json"),
            format!("{{\"packages\":{{\"psr/log\":{{\"reference\":\"{digest_reference}\"}}}}}}"),
        )
        .expect("digests");
        let set = ResolvedSet {
            name: "php".to_owned(),
            ecosystem: Ecosystem::Composer,
            dir: "php".to_owned(),
            manifests: vec!["php/composer.json".to_owned()],
            locks: vec![
                "php/composer.lock".to_owned(),
                "php/composer.digests.json".to_owned(),
            ],
            repository: None,
        };
        (scratch, set)
    }

    #[test]
    fn composer_locks_become_composer_sets() {
        let module = "composer = use_extension(\"@rules_dx//php/composer:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor\", lock = \"//php:composer.lock\", digests = \"//php:composer.digests.json\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("vendor").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Composer);
        assert_eq!(set.manifests, ["php/composer.json"]);
        assert_eq!(
            set.locks,
            ["php/composer.lock", "php/composer.digests.json"]
        );
    }

    #[test]
    fn composer_layout_accepts_matching_digests() {
        let (scratch, set) = composer_workspace("aaa");
        assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
    }

    #[test]
    fn composer_layout_rejects_a_stale_digest() {
        let (scratch, set) = composer_workspace("bbb");
        let error = check_backend_settings(scratch.path(), &set).expect_err("stale");
        let message = error.to_string();
        assert!(message.contains("psr/log"), "{message}");
        assert!(message.contains("aaa"), "{message}");
    }

    #[test]
    fn swift_from_package_tags_become_swiftpm_sets() {
        let module = "swift_deps = use_extension(\"@rules_swift_package_manager//:extensions.bzl\", \"swift_deps\")\nswift_deps.from_package(resolved = \"//app:Package.resolved\", swift = \"//app:Package.swift\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("swiftpm").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::SwiftPm);
        assert_eq!(set.manifests, ["app/Package.swift"]);
        assert_eq!(set.locks, ["app/Package.resolved"]);
    }

    #[test]
    fn julia_from_manifest_tags_become_julia_sets() {
        let module = "pkg = use_extension(\"@rules_julia//julia/pkg:extensions.bzl\", \"pkg\")\npkg.from_manifest(name = \"jl_deps\", lockfile = \"//third_party/julia:Manifest.bazel.json\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("jl_deps").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Julia);
        assert_eq!(set.manifests, ["third_party/julia/Project.toml"]);
        assert_eq!(
            set.locks,
            [
                "third_party/julia/Manifest.toml",
                "third_party/julia/Manifest.bazel.json"
            ]
        );
    }

    #[test]
    fn julia_layout_needs_both_locks_to_name_the_same_versions() {
        let dir = std::env::temp_dir().join(format!("dx-julia-layout-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let manifest = "manifest_format = \"2.0\"\n[[deps.Glob]]\nversion = \"1.3.1\"\n[[deps.Dates]]\nuuid = \"x\"\n";
        std::fs::write(dir.join("Manifest.toml"), manifest).expect("write");
        std::fs::write(
            dir.join("Manifest.bazel.json"),
            "{\"Glob\": {\"version\": \"1.3.1\"}}",
        )
        .expect("write");
        let set = ResolvedSet {
            name: "jl".to_owned(),
            ecosystem: Ecosystem::Julia,
            dir: ".".to_owned(),
            manifests: vec!["Project.toml".to_owned()],
            locks: vec!["Manifest.toml".to_owned(), "Manifest.bazel.json".to_owned()],
            repository: None,
        };
        assert!(check_julia_layout(&dir, &set).is_ok());
        std::fs::write(
            dir.join("Manifest.bazel.json"),
            "{\"Glob\": {\"version\": \"1.3.0\"}}",
        )
        .expect("write");
        let message = check_julia_layout(&dir, &set).unwrap_err().to_string();
        assert!(message.contains("Glob 1.3.0"), "{message}");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pub_locks_become_pub_sets() {
        let module = "pub = use_extension(\"@rules_dart//dart/pub:extensions.bzl\", \"pub\")\npub.from_lock(name = \"dart_deps\", lock = \"//third_party/dart:pubspec.lock\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("dart_deps").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Pub);
        assert_eq!(set.manifests, ["third_party/dart/pubspec.yaml"]);
        assert_eq!(set.locks, ["third_party/dart/pubspec.lock"]);
    }

    #[test]
    fn elixir_mix_tags_become_hex_sets() {
        let module = "elixir = use_extension(\"//elixir/toolchain:repos.bzl\", \"elixir\")\nelixir.mix(name = \"deps\", lock = \"//third_party/elixir:mix.lock\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("deps").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Hex);
        assert_eq!(set.manifests, ["third_party/elixir/mix.exs"]);
        assert_eq!(set.locks, ["third_party/elixir/mix.lock"]);
    }

    #[test]
    fn the_repos_own_dart_hub_becomes_a_pub_set() {
        let module = "pub = use_extension(\"//dart:extensions.bzl\", \"pub\", dev_dependency = True)\npub.from_lock(name = \"dx_dart_deps\", lock = \"//third_party/dart:pubspec.lock\")\n";
        let registry = discover(module, &|_: &str| Err("none".to_owned()))
            .expect("discovers")
            .expect("registry");
        let set = registry.find("dx_dart_deps").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Pub);
        assert_eq!(set.locks, ["third_party/dart/pubspec.lock"]);
    }

    #[test]
    fn conan_extensions_become_conan_sets() {
        let module = "conan = use_extension(\"//build/conan:conan_deps_module_extension.bzl\", \"conan_extension\")\n";
        let read = |path: &str| match path {
            "build/conanfile.py" => Ok(String::new()),
            _ => Err("none".to_owned()),
        };
        let registry = discover(module, &read)
            .expect("discovers")
            .expect("registry");
        let set = registry.find("conan").expect("set");
        assert_eq!(set.ecosystem, Ecosystem::Conan);
        assert_eq!(set.manifests, ["build/conanfile.py"]);
        assert_eq!(set.locks, ["build/conan.lock"]);
    }

    #[test]
    fn bundle_fetches_become_ruby_sets() {
        let module = "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(gemfile = \"//r:Gemfile\", gemfile_lock = \"//r:Gemfile.lock\")\n";
        let registry = found(module, &[]);
        let set = registry.find("bundle").expect("bundle");
        assert_eq!(set.ecosystem, Ecosystem::Ruby);
        assert_eq!(set.manifests, ["r/Gemfile"]);
        assert_eq!(set.locks, ["r/Gemfile.lock"]);
    }

    #[test]
    fn paket_extensions_become_the_nuget_set() {
        let module = "paket = use_extension(\"//third_party/dotnet:extension.bzl\", \"paket\")\n";
        let bzl = "load(\"//third_party/dotnet/deps:paket.main.bzl\", \"main\")\nload(\":framework.bzl\", \"framework\")\nload(\"//third_party/dotnet/deps:paket.docs.bzl\", \"docs\")\n";
        let registry = found(module, &[("third_party/dotnet/extension.bzl", bzl)]);
        let set = registry.find("nuget").expect("nuget");
        assert_eq!(set.ecosystem, Ecosystem::Nuget);
        assert_eq!(set.manifests, ["third_party/dotnet/paket.dependencies"]);
        assert_eq!(
            set.locks,
            [
                "third_party/dotnet/paket.lock",
                "third_party/dotnet/deps/paket.docs.bzl",
                "third_party/dotnet/deps/paket.main.bzl",
            ]
        );
        let generated =
            "main = use_extension(\"//x/deps:paket.main_extension.bzl\", \"main_extension\")\n";
        let registry = found(generated, &[]);
        let set = registry.find("nuget").expect("nuget");
        assert_eq!(set.manifests, ["x/paket.dependencies"]);
        assert_eq!(set.locks, ["x/paket.lock", "x/deps/paket.main.bzl"]);
    }

    #[test]
    fn included_module_files_are_followed() {
        let module = "include(\"//tools:deps.MODULE.bazel\")\n";
        let included = format!(
            "{NPM_EXT}npm.npm_translate_lock(name = \"tools\", pnpm_lock = \"//tools:pnpm-lock.yaml\")\n"
        );
        let registry = found(module, &[("tools/deps.MODULE.bazel", &included)]);
        assert_eq!(registry.names(), vec!["tools"]);
    }

    #[test]
    fn a_module_without_hubs_is_not_a_registry() {
        assert_eq!(discovered("module(name = \"x\")\n", &[]), Ok(None));
    }

    #[test]
    fn hub_problems_fail_closed() {
        let duplicate = format!(
            "{UV_EXT}uv.project(hub_name = \"pypi\", lock = \"//a/api:uv.lock\", pyproject = \"//a/api:pyproject.toml\")\nuv.project(hub_name = \"pypi\", lock = \"//b/api:uv.lock\", pyproject = \"//b/api:pyproject.toml\")\n"
        );
        assert_eq!(
            discovered(&duplicate, &[]),
            Err(DependencySetsError::DuplicateName {
                name: "pypi-api".to_owned()
            })
        );
        let shared = format!(
            "{NPM_EXT}npm.npm_translate_lock(name = \"a\", pnpm_lock = \"//:pnpm-lock.yaml\")\nnpm.npm_translate_lock(name = \"b\", pnpm_lock = \"//:pnpm-lock.yaml\")\n"
        );
        assert!(matches!(
            discovered(&shared, &[]),
            Err(DependencySetsError::ConflictingOwnership { .. })
        ));
        let computed = format!("{NPM_EXT}npm.npm_translate_lock(name = NAME)\n");
        assert!(matches!(
            discovered(&computed, &[]),
            Err(DependencySetsError::Module { .. })
        ));
        assert!(matches!(
            discovered(
                "paket = use_extension(\"//d:extension.bzl\", \"paket\")\n",
                &[]
            ),
            Err(DependencySetsError::Module { .. })
        ));
    }

    #[test]
    fn load_reads_the_workspace_module() {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-absent-");
        assert_eq!(load(&scratch.path().join("absent")).expect("absent"), None);
        std::fs::write(
            scratch.path().join(MODULE_BAZEL),
            "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//:go.mod\")\n",
        )
        .expect("module");
        let registry = load(scratch.path()).expect("loads").expect("registry");
        assert_eq!(registry.names(), vec!["go"]);
    }

    #[test]
    fn names_and_repositories_must_be_plain() {
        for bad in ["", "a:b", "_a", "a b", "@a"] {
            assert_eq!(
                hub_set(
                    bad,
                    Ecosystem::Uv,
                    strings(&["a/pyproject.toml"]),
                    strings(&["a/uv.lock"])
                ),
                Err(DependencySetsError::InvalidName {
                    name: bad.to_owned()
                }),
                "{bad}"
            );
        }
        assert_eq!(
            hub_set(
                "1crates",
                Ecosystem::Cargo,
                strings(&["a/Cargo.toml"]),
                strings(&["a/Cargo.lock"])
            ),
            Err(DependencySetsError::InvalidRepository {
                name: "1crates".to_owned(),
                repository: "1crates".to_owned(),
            })
        );
        assert!(
            hub_set(
                "Crate_index.v2-x",
                Ecosystem::Cargo,
                strings(&["a/Cargo.toml"]),
                strings(&["a/Cargo.lock"])
            )
            .is_ok()
        );
        for bad in [
            "../up/pyproject.toml",
            "a/../../pyproject.toml",
            "/abs/pyproject.toml",
        ] {
            assert!(
                matches!(
                    hub_set("a", Ecosystem::Uv, strings(&[bad]), strings(&["a/uv.lock"])),
                    Err(DependencySetsError::EscapingPath { .. })
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn two_same_ecosystem_sets_sort_by_name() {
        let registry = two_uv();
        assert_eq!(registry.names(), vec!["frontend", "worker"]);
        let frontend = registry.find("frontend").expect("frontend");
        assert_eq!(frontend.ecosystem, Ecosystem::Uv);
        assert_eq!(frontend.manifests, vec!["apps/frontend/pyproject.toml"]);
        assert_eq!(frontend.locks, vec!["apps/frontend/uv.lock"]);
        assert_eq!(frontend.scopes, vec!["apps/frontend"]);
        assert_eq!(ResolvedSet::of(frontend).dir, "apps/frontend");
    }

    #[test]
    fn bare_selection_resolves_every_configured_set_full() {
        let registry = two_uv();
        let resolved = resolve(&registry, &[]).expect("bare");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].set.name, "frontend");
        assert_eq!(resolved[0].request, Selection::Full);
        assert_eq!(resolved[1].set.name, "worker");
        assert_eq!(resolved[1].request, Selection::Full);
    }
    #[test]
    fn names_scopes_and_packages_resolve() {
        let registry = two_uv();
        let resolved = resolve(&registry, &strings(&["frontend"])).expect("name");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "frontend");
        let resolved = resolve(&registry, &strings(&["apps/frontend/app"])).expect("nested scope");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "frontend");
        let resolved =
            resolve(&registry, &strings(&["//services/worker:target"])).expect("label scope");
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].set.name, "worker");
        let resolved = resolve(&registry, &strings(&["//..."])).expect("repo scope");
        assert_eq!(resolved.len(), 2);
        let resolved =
            resolve(&registry, &strings(&["frontend:anyio", "frontend:httpx"])).expect("packages");
        assert_eq!(resolved.len(), 1);
        assert_eq!(
            resolved[0].request,
            Selection::Packages(vec!["anyio".to_owned(), "httpx".to_owned()])
        );
        let resolved =
            resolve(&registry, &strings(&["frontend", "frontend:httpx"])).expect("full wins");
        assert_eq!(resolved[0].request, Selection::Full);
    }

    #[test]
    fn unknown_selectors_and_scopes_fail_closed() {
        let registry = two_uv();
        assert!(matches!(
            resolve(&registry, &strings(&["cargo"])),
            Err(DependencySetsError::UnknownSelector { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["docs/cli/README.md"])),
            Err(DependencySetsError::NoOwningSet { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["frontend:"])),
            Err(DependencySetsError::InvalidPackage { .. })
        ));
        assert!(matches!(
            resolve(&registry, &strings(&["frontend:bad name"])),
            Err(DependencySetsError::InvalidPackage { .. })
        ));
    }

    #[test]
    fn root_scope_owns_only_root_package_labels() {
        let specs = |root: &'static [&'static str], lock: &'static [&'static str]| {
            registry(&[
                ("root", Ecosystem::Npm, root, lock),
                (
                    "leaf",
                    Ecosystem::Uv,
                    &["apps/leaf/pyproject.toml"],
                    &["apps/leaf/uv.lock"],
                ),
            ])
        };
        let registry = specs(&["package.json"], &["pnpm-lock.yaml"]);
        for target in ["//:node_modules", "//:", "."] {
            assert_eq!(owning_sets(&registry, target), vec!["root"], "{target}");
        }
        assert_eq!(owning_sets(&registry, "//foo:bar"), Vec::<&str>::new());
        assert_eq!(owning_sets(&registry, "//apps/leaf:t"), vec!["leaf"]);
        assert_eq!(owning_sets(&registry, "//foo/..."), Vec::<&str>::new());
        assert_eq!(owning_sets(&registry, "//...").len(), 2);
        assert_eq!(owning_sets(&registry, "//:MODULE.bazel").len(), 2);
        let bare = specs(&["apps/root/package.json"], &["apps/root/pnpm-lock.yaml"]);
        assert_eq!(owning_sets(&bare, "//:x"), Vec::<&str>::new());
    }

    #[test]
    fn declared_manifests_and_locks_select_their_own_set() {
        let registry = registry(&[
            (
                "cargo",
                Ecosystem::Cargo,
                &["rust/Cargo.toml"],
                &["rust/Cargo.lock", "cargo-bazel-lock.json"],
            ),
            (
                "maven",
                Ecosystem::Maven,
                &["MODULE.bazel"],
                &["third_party/jvm/maven_install.json"],
            ),
            (
                "npm",
                Ecosystem::Npm,
                &["web/package.json"],
                &["web/pnpm-lock.yaml"],
            ),
            ("root", Ecosystem::Uv, &["pyproject.toml"], &["uv.lock"]),
        ]);
        assert_eq!(
            owning_sets(&registry, "//:cargo-bazel-lock.json"),
            vec!["cargo"]
        );
        assert_eq!(
            owning_sets(&registry, "third_party/jvm/maven_install.json"),
            vec!["maven"]
        );
        assert_eq!(
            owning_sets(&registry, "//third_party/jvm:maven_install.json"),
            vec!["maven"]
        );
        assert_eq!(owning_sets(&registry, "//web:pnpm-lock.yaml"), vec!["npm"]);
        assert_eq!(owning_sets(&registry, "//:other"), vec!["cargo", "root"]);
    }

    #[test]
    fn overlapping_scopes_resolve_to_every_owner() {
        let registry = registry(&[
            (
                "mono",
                Ecosystem::Uv,
                &["apps/pyproject.toml"],
                &["apps/uv.lock"],
            ),
            (
                "leaf",
                Ecosystem::Uv,
                &["apps/leaf/pyproject.toml"],
                &["apps/leaf/uv.lock"],
            ),
        ]);
        let resolved = resolve(&registry, &strings(&["apps/leaf/app"])).expect("overlap");
        assert_eq!(resolved.len(), 2);
        let resolved = resolve(&registry, &strings(&["//apps/..."])).expect("recursive");
        assert_eq!(resolved.len(), 2);
    }

    fn settings_workspace(
        name: &str,
        pyproject: &str,
        config: Option<&str>,
    ) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-settings-{name}-"));
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("apps/frontend")).expect("mkdir");
        std::fs::write(workspace.join("apps/frontend/pyproject.toml"), pyproject)
            .expect("pyproject");
        if let Some(config) = config {
            std::fs::write(
                workspace.join(config),
                "exclude-newer = \"2020-01-01T00:00:00Z\"\n",
            )
            .expect("uv.toml");
        }
        scratch
    }

    #[test]
    fn uv_settings_allow_project_metadata_only() {
        let registry = two_uv();
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let metadata = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\npackage = false\ndev-dependencies = []\n\n[tool.uv.sources]\nlocal = { path = \"../local\" }\n\n[tool.ruff]\nline-length = 100\n";
        let scratch = settings_workspace("ok", metadata, None);
        let workspace = scratch.path().to_path_buf();
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        let resolver = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n\n[tool.uv]\nexclude-newer = \"2020-01-01T00:00:00Z\"\nindex-url = \"https://example.invalid/simple\"\n";
        let scratch = settings_workspace("tool", resolver, None);
        let workspace = scratch.path().to_path_buf();
        let error = check_backend_settings(&workspace, &frontend).expect_err("resolver settings");
        assert_eq!(
            error,
            DependencySetsError::UnsupportedSettings {
                name: "frontend".to_owned(),
                detail: "apps/frontend/pyproject.toml sets [tool.uv] exclude-newer, index-url"
                    .to_owned(),
            }
        );
        let message = error.to_string();
        assert!(message.contains("\"frontend\""), "{message}");
        assert!(message.contains("does not apply"), "{message}");
    }

    #[test]
    fn uv_config_files_in_the_set_or_its_parents_are_unsupported() {
        let registry = two_uv();
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
        for (config, shown) in [
            ("apps/frontend/uv.toml", "apps/frontend/uv.toml exists"),
            ("apps/uv.toml", "apps/uv.toml exists"),
            ("uv.toml", "uv.toml exists"),
        ] {
            let scratch = settings_workspace(&config.replace('/', "-"), plain, Some(config));
            let workspace = scratch.path().to_path_buf();
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedSettings {
                    name: "frontend".to_owned(),
                    detail: shown.to_owned(),
                }),
                "{config}"
            );
        }
        let scratch = settings_workspace("missing", plain, None);
        let workspace = scratch.path().to_path_buf();
        std::fs::remove_file(workspace.join("apps/frontend/pyproject.toml")).expect("rm");
        assert!(matches!(
            check_backend_settings(&workspace, &frontend),
            Err(DependencySetsError::Read { .. })
        ));
    }

    #[test]
    fn parent_uv_workspaces_are_unsupported() {
        let registry = two_uv();
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let plain = "[project]\nname = \"frontend\"\nversion = \"0.1.0\"\n";
        for (parent, shown) in [
            (
                "apps/pyproject.toml",
                "apps/pyproject.toml sets [tool.uv.workspace]",
            ),
            ("pyproject.toml", "pyproject.toml sets [tool.uv.workspace]"),
        ] {
            let scratch = settings_workspace(&parent.replace('/', "-"), plain, None);
            let workspace = scratch.path().to_path_buf();
            std::fs::write(
                workspace.join(parent),
                "[tool.uv.workspace]\nmembers = [\"frontend\"]\n",
            )
            .expect("parent pyproject");
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedSettings {
                    name: "frontend".to_owned(),
                    detail: shown.to_owned(),
                }),
                "{parent}"
            );
        }
        let scratch = settings_workspace("plain-parent", plain, None);
        let workspace = scratch.path().to_path_buf();
        std::fs::write(workspace.join("pyproject.toml"), plain).expect("parent pyproject");
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
    }

    #[test]
    fn missing_files_fail_closed_with_paths() {
        let registry = two_uv();
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-missing-");
        let workspace = scratch.path().join("absent");
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        assert!(matches!(
            check_files(&workspace, &frontend, true),
            Err(DependencySetsError::MissingFile { .. })
        ));
    }

    #[cfg(unix)]
    mod link_confinement {
        use super::*;

        fn setup() -> (dx_test_scratch::TempDir, std::path::PathBuf, ResolvedSet) {
            let outer = dx_test_scratch::scratch("dx-linkconf-");
            let workspace = outer.path().join("ws");
            std::fs::create_dir_all(&workspace).expect("ws");
            let registry = two_uv();
            let set = ResolvedSet::of(registry.find("frontend").expect("frontend"));
            (outer, workspace, set)
        }

        fn link_out(outer: &Path, workspace: &Path, rel: &str) -> std::path::PathBuf {
            let outside = outer.join("outside.txt");
            std::fs::write(&outside, "x").expect("outside");
            let at = workspace.join(rel);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::os::unix::fs::symlink(&outside, at).expect("link");
            outside
        }

        fn is_outside(result: Result<(), DependencySetsError>) -> bool {
            matches!(result, Err(DependencySetsError::OutsideWorkspace { .. }))
        }

        #[test]
        fn manifest_linking_outside_is_rejected() {
            let (outer, workspace, set) = setup();
            link_out(outer.path(), &workspace, &set.manifests[0]);
            assert!(is_outside(check_files(&workspace, &set, false)));
        }

        #[test]
        fn lock_linking_outside_is_rejected_with_and_without_need_lock() {
            let (outer, workspace, set) = setup();
            for manifest in &set.manifests {
                let at = workspace.join(manifest);
                std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
                std::fs::write(at, "").expect("manifest");
            }
            link_out(outer.path(), &workspace, &set.locks[0]);
            assert!(is_outside(check_files(&workspace, &set, false)));
            assert!(is_outside(check_files(&workspace, &set, true)));
        }

        #[test]
        fn dangling_lock_link_is_rejected_without_need_lock() {
            let (outer, workspace, set) = setup();
            for manifest in &set.manifests {
                let at = workspace.join(manifest);
                std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
                std::fs::write(at, "").expect("manifest");
            }
            let at = workspace.join(&set.locks[0]);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::os::unix::fs::symlink(outer.path().join("nope"), at).expect("link");
            assert!(is_outside(check_files(&workspace, &set, false)));
        }

        #[test]
        fn lock_linking_inside_is_accepted() {
            let (_outer, workspace, set) = setup();
            for manifest in &set.manifests {
                let at = workspace.join(manifest);
                std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
                std::fs::write(at, "").expect("manifest");
            }
            let real = workspace.join("real.lock");
            std::fs::write(&real, "x").expect("real");
            let at = workspace.join(&set.locks[0]);
            std::fs::create_dir_all(at.parent().expect("parent")).expect("dirs");
            std::os::unix::fs::symlink(&real, at).expect("link");
            assert_eq!(check_files(&workspace, &set, true), Ok(()));
        }
    }

    fn two_npm() -> Registry {
        registry(&[
            (
                "frontend",
                Ecosystem::Npm,
                &["apps/web/package.json"],
                &["apps/web/pnpm-lock.yaml"],
            ),
            (
                "worker",
                Ecosystem::Npm,
                &["services/worker/package.json"],
                &["services/worker/pnpm-lock.yaml"],
            ),
        ])
    }

    #[test]
    fn npm_sets_load_under_their_own_names() {
        let registry = two_npm();
        assert_eq!(registry.names(), vec!["frontend", "worker"]);
        let frontend = registry.find("frontend").expect("frontend");
        assert_eq!(frontend.ecosystem, Ecosystem::Npm);
        assert_eq!(frontend.manifests, vec!["apps/web/package.json"]);
        assert_eq!(frontend.locks, vec!["apps/web/pnpm-lock.yaml"]);
        assert_eq!(ResolvedSet::of(frontend).dir, "apps/web");
        assert_eq!(Ecosystem::parse("npm"), Some(Ecosystem::Npm));
        assert_eq!(Ecosystem::Npm.name(), "npm");
        assert_eq!(Ecosystem::Npm.supported_check(), "check");
        assert_eq!(Ecosystem::Npm.supported_update(), "update");
        assert_eq!(Ecosystem::Npm.supported_audit(), "audit");
        assert_eq!(
            SUPPORTED_ECOSYSTEMS,
            "uv, npm, bazel, ruby, powershell, cargo, maven, nuget, go, cpan, pub, conan, julia, hex, composer, opam, swiftpm, cabal"
        );
    }

    #[test]
    fn advisory_families_follow_the_ecosystem_never_the_set_name() {
        assert_eq!(Ecosystem::Uv.advisory_family(), Some("pypi"));
        assert_eq!(Ecosystem::Npm.advisory_family(), Some("npm"));
        assert_eq!(Ecosystem::Bazel.advisory_family(), None);
        assert_eq!(Ecosystem::Ruby.advisory_family(), Some("rubygems"));
        assert_eq!(Ecosystem::PowerShell.advisory_family(), None);
        let registry = two_npm();
        for set in &registry.sets {
            assert_eq!(set.ecosystem.advisory_family(), Some("npm"), "{}", set.name);
        }
    }

    #[test]
    fn manifests_must_carry_the_ecosystem_file_name() {
        for (ecosystem, manifest, want) in [
            ("uv", "a/package.json", "pyproject.toml"),
            ("uv", "a/pyproject.toml.bak", "pyproject.toml"),
            ("npm", "a/pyproject.toml", "package.json"),
            ("npm", "a/package.json5", "package.json"),
            ("npm", "package.yaml", "package.json"),
            ("bazel", "a/package.json", "MODULE.bazel"),
            ("ruby", "a/gems.rb", "Gemfile"),
            ("ruby", "a/Gemfile.lock", "Gemfile"),
            ("cargo", "a/cargo.toml", "Cargo.toml"),
            ("cargo", "a/Cargo.lock", "Cargo.toml"),
            (
                "powershell",
                "a/requirements.psd1",
                "PSGallery.requirements.psd1",
            ),
        ] {
            let kind = Ecosystem::parse(ecosystem).expect("known");
            let error = hub_set("a", kind, strings(&[manifest]), strings(&["a/lock"]))
                .expect_err("wrong manifest name");
            assert_eq!(
                error,
                DependencySetsError::InvalidManifest {
                    name: "a".to_owned(),
                    path: manifest.to_owned(),
                    want,
                    ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
                },
                "{ecosystem} {manifest}"
            );
            assert!(error.to_string().contains(want), "{error}");
        }
        assert!(
            hub_set(
                "root",
                Ecosystem::Npm,
                strings(&["package.json"]),
                strings(&["pnpm-lock.yaml"])
            )
            .is_ok()
        );
    }

    #[test]
    fn npm_packages_allow_one_scope_and_lowercase_names() {
        let registry = two_npm();
        for package in [
            "react",
            "@types/node",
            "lodash.merge",
            "a-b_c",
            "@scope/x.y",
        ] {
            let resolved = resolve(&registry, &[format!("frontend:{package}")])
                .unwrap_or_else(|error| panic!("{package}: {error}"));
            assert_eq!(
                resolved,
                vec![ResolvedTarget {
                    set: ResolvedSet::of(registry.find("frontend").expect("frontend")),
                    request: Selection::Packages(vec![package.to_owned()]),
                }]
            );
        }
        for package in [
            "React",
            "@types",
            "@/node",
            "@types/",
            "@a/b/c",
            ".hidden",
            "_private",
            "a b",
            "a;rm",
            "--registry=x",
            "pkg@1.0.0",
        ] {
            assert!(
                matches!(
                    resolve(&registry, &[format!("frontend:{package}")]),
                    Err(DependencySetsError::InvalidPackage { .. })
                ),
                "{package} must be rejected"
            );
        }
    }

    fn npm_workspace(name: &str) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch(&format!("dx-dependency-sets-npm-{name}-"));
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("apps/web")).expect("set dir");
        std::fs::write(workspace.join("apps/web/package.json"), "{}\n").expect("manifest");
        scratch
    }

    #[test]
    fn npm_sets_need_their_own_pnpm_lock_and_no_parent_pnpm_workspace() {
        let registry = two_npm();
        let frontend = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        let scratch = npm_workspace("plain");
        let workspace = scratch.path().to_path_buf();
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        std::fs::write(
            workspace.join("apps/web/pnpm-workspace.yaml"),
            "packages: []\n",
        )
        .expect("own workspace");
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        for parent in ["apps/pnpm-workspace.yaml", "pnpm-workspace.yaml"] {
            let scratch = npm_workspace(&parent.replace('/', "-"));
            let workspace = scratch.path().to_path_buf();
            std::fs::write(workspace.join(parent), "packages: [\"apps/*\"]\n")
                .expect("parent workspace");
            assert_eq!(
                check_backend_settings(&workspace, &frontend),
                Err(DependencySetsError::UnsupportedNpmLayout {
                    name: "frontend".to_owned(),
                    dir: "apps/web".to_owned(),
                    detail: format!("{parent} makes pnpm write the parent workspace lock"),
                }),
                "{parent}"
            );
        }
        let scratch = npm_workspace("root-only");
        let workspace = scratch.path().to_path_buf();
        std::fs::write(
            workspace.join("pnpm-workspace.yaml"),
            "packages:\n  - \".\" # root\nallowBuilds: {}\n",
        )
        .expect("root-only workspace");
        assert_eq!(check_backend_settings(&workspace, &frontend), Ok(()));
        assert!(npm_ignore_workspace(&workspace, &frontend));
        std::fs::write(
            workspace.join("apps/web/pnpm-workspace.yaml"),
            "packages: []\n",
        )
        .expect("own workspace");
        assert!(!npm_ignore_workspace(&workspace, &frontend));
        let scratch = npm_workspace("members");
        let workspace = scratch.path().to_path_buf();
        std::fs::write(
            workspace.join("pnpm-workspace.yaml"),
            "packages: [\"apps/*\"]\n",
        )
        .expect("members workspace");
        assert!(!npm_ignore_workspace(&workspace, &frontend));
        let scratch = npm_workspace("other-lock");
        let workspace = scratch.path().to_path_buf();
        let mut other = frontend.clone();
        other.locks = vec!["apps/web/bun.lockb".to_owned()];
        assert_eq!(
            check_backend_settings(&workspace, &other),
            Err(DependencySetsError::UnsupportedNpmLayout {
                name: "frontend".to_owned(),
                dir: "apps/web".to_owned(),
                detail: "locks must include apps/web/pnpm-lock.yaml, apps/web/yarn.lock, or apps/web/package-lock.json"
                    .to_owned(),
            })
        );
        for (lock, backend) in [
            ("apps/web/yarn.lock", NpmBackend::Yarn),
            ("apps/web/package-lock.json", NpmBackend::Npm),
        ] {
            other.locks = vec![lock.to_owned()];
            assert_eq!(check_backend_settings(&workspace, &other), Ok(()), "{lock}");
            assert_eq!(npm_backend(&other), Some(backend));
        }
    }

    fn pinned() -> Registry {
        registry(&[
            (
                "gems",
                Ecosystem::Ruby,
                &["apps/rb/Gemfile"],
                &["apps/rb/Gemfile.lock"],
            ),
            (
                "modules",
                Ecosystem::PowerShell,
                &["tools/ps/PSGallery.requirements.psd1"],
                &["tools/ps/PSGallery.lock.json"],
            ),
        ])
    }

    #[test]
    fn cpan_module_names_allow_double_colons() {
        for good in ["Moo", "Text::CSV", "Text::CSV_XS"] {
            assert!(validate_package(Ecosystem::Cpan, good).is_ok(), "{good}");
        }
        for bad in ["Text::", "::CSV", "Text:CSV", "Text::CSV-XS", ""] {
            assert!(validate_package(Ecosystem::Cpan, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ruby_and_powershell_sets_load_as_pinned_ecosystems() {
        let registry = pinned();
        assert_eq!(registry.names(), vec!["gems", "modules"]);
        let gems = registry.find("gems").expect("gems");
        assert_eq!(gems.ecosystem, Ecosystem::Ruby);
        assert_eq!(ResolvedSet::of(gems).dir, "apps/rb");
        let modules = registry.find("modules").expect("modules");
        assert_eq!(modules.ecosystem, Ecosystem::PowerShell);
        for (ecosystem, name, manifest) in [
            (Ecosystem::Ruby, "ruby", "Gemfile"),
            (
                Ecosystem::PowerShell,
                "powershell",
                "PSGallery.requirements.psd1",
            ),
        ] {
            assert_eq!(Ecosystem::parse(name), Some(ecosystem));
            assert_eq!(ecosystem.name(), name);
            assert_eq!(ecosystem.manifest_name(), manifest);
            assert_eq!(ecosystem.supported_check(), "pinned");
            assert_eq!(ecosystem.supported_update(), "pinned");
            assert_eq!(ecosystem.supported_audit(), "audit");
            assert_eq!(
                ecosystem.supported_operations(),
                "audit; check and update report pinned"
            );
        }
        assert_eq!(Ecosystem::Uv.supported_operations(), "check, update, audit");
        assert_eq!(
            Ecosystem::Npm.supported_operations(),
            "check, update, audit"
        );
        assert_eq!(Ecosystem::parse("rubygems"), None);
        assert_eq!(Ecosystem::parse("PowerShell"), None);
    }

    #[test]
    fn pinned_sets_must_declare_the_lock_beside_their_manifest() {
        let registry = pinned();
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-pinned-");
        let workspace = scratch.path().to_path_buf();
        for name in ["gems", "modules"] {
            let set = ResolvedSet::of(registry.find(name).expect("set"));
            assert_eq!(check_backend_settings(&workspace, &set), Ok(()), "{name}");
        }
        for (ecosystem, manifest, lock, want) in [
            (
                "ruby",
                "apps/rb/Gemfile",
                "apps/rb/gems.locked",
                "apps/rb/Gemfile.lock",
            ),
            ("ruby", "Gemfile", "other/Gemfile.lock", "Gemfile.lock"),
            (
                "powershell",
                "tools/ps/PSGallery.requirements.psd1",
                "tools/ps/lock.json",
                "tools/ps/PSGallery.lock.json",
            ),
        ] {
            let kind = Ecosystem::parse(ecosystem).expect("known");
            let set = ResolvedSet::of(
                &hub_set("a", kind, strings(&[manifest]), strings(&[lock])).expect("valid"),
            );
            let error = check_backend_settings(&workspace, &set).expect_err("missing lock");
            assert_eq!(
                error,
                DependencySetsError::MissingPinnedLock {
                    name: "a".to_owned(),
                    path: want.to_owned(),
                    ecosystem: Ecosystem::parse(ecosystem).expect("known").name(),
                },
                "{ecosystem} {lock}"
            );
            assert!(error.to_string().contains(want), "{error}");
        }
    }

    #[test]
    fn pinned_packages_use_the_plain_charset() {
        let registry = pinned();
        for (set, package) in [
            ("gems", "rspec-core"),
            ("gems", "nokogiri"),
            ("gems", "Ruby_Gem.2"),
            ("modules", "Pester"),
            ("modules", "Az.Accounts"),
        ] {
            let resolved = resolve(&registry, &[format!("{set}:{package}")])
                .unwrap_or_else(|error| panic!("{set}:{package}: {error}"));
            assert_eq!(
                resolved[0].request,
                Selection::Packages(vec![package.to_owned()])
            );
        }
        for (set, package, reason) in [
            ("gems", "a b", RUBY_PACKAGE_CHARSET),
            ("gems", "rails@7", RUBY_PACKAGE_CHARSET),
            ("gems", "a;rm", RUBY_PACKAGE_CHARSET),
            ("modules", "Az/Accounts", POWERSHELL_PACKAGE_CHARSET),
            ("modules", "Az Accounts", POWERSHELL_PACKAGE_CHARSET),
        ] {
            assert_eq!(
                resolve(&registry, &[format!("{set}:{package}")]),
                Err(DependencySetsError::InvalidPackage {
                    set: set.to_owned(),
                    package: package.to_owned(),
                    reason,
                }),
                "{set}:{package}"
            );
        }
    }

    fn cargo_sets() -> Registry {
        registry(&[
            (
                "crates",
                Ecosystem::Cargo,
                &["apps/rs/Cargo.toml"],
                &["apps/rs/Cargo.lock", "apps/rs/cargo-bazel-lock.json"],
            ),
            (
                "tool_crates",
                Ecosystem::Cargo,
                &["tools/rs/Cargo.toml"],
                &["tools/rs/Cargo.lock"],
            ),
        ])
    }

    const CARGO_EXT: &str =
        "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n";

    const CARGO_HUBS: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", lockfile = \"//apps/rs:cargo-bazel-lock.json\", manifests = [\"//apps/rs:Cargo.toml\"])\ncrate.from_cargo(name = \"tool_crates\", cargo_lockfile = \"//tools/rs:Cargo.lock\", manifests = [\"//tools/rs:Cargo.toml\"])\nuse_repo(crate, \"crates\", \"tool_crates\")\n";

    fn cargo_hub_detail(module: &str, set: &ResolvedSet) -> String {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-hub-");
        std::fs::write(scratch.path().join(MODULE_BAZEL), module).expect("module");
        match check_backend_settings(scratch.path(), set) {
            Err(DependencySetsError::UnsupportedCargoHub { detail, .. }) => detail,
            other => panic!("expected UnsupportedCargoHub, got {other:?}"),
        }
    }

    #[test]
    fn cargo_hubs_resolve_by_their_visible_name_and_refuse_everything_else() {
        let registry = cargo_sets();
        let crates = ResolvedSet::of(registry.find("crates").expect("crates"));
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-visible-");
        let hub = |import: &str| {
            format!(
                "{CARGO_EXT}crate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", lockfile = \"//apps/rs:cargo-bazel-lock.json\", manifests = [\"//apps/rs:Cargo.toml\"])\n{import}"
            )
        };
        for (import, visible) in [
            ("use_repo(crate, \"crates\")\n", "crates"),
            ("use_repo(crate, deps = \"crates\")\n", "deps"),
        ] {
            std::fs::write(scratch.path().join(MODULE_BAZEL), hub(import)).expect("module");
            assert_eq!(check_backend_settings(scratch.path(), &crates), Ok(()));
            assert_eq!(
                cargo_fetch_repository(scratch.path(), &crates).as_deref(),
                Some(visible)
            );
        }
        assert!(cargo_hub_detail(CARGO_EXT, &crates).contains("no crate.from_cargo"));
        assert!(cargo_hub_detail(&hub(""), &crates).contains("use_repo"));
        let two = hub("use_repo(crate, \"crates\")\n").replace(
            "use_repo",
            "crate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", manifests = [\"//apps/rs:Cargo.toml\"])\nuse_repo",
        );
        assert!(cargo_hub_detail(&two, &crates).contains("2 crate.from_cargo"));
        let mismatch = hub("use_repo(crate, \"crates\")\n")
            .replace("//apps/rs:Cargo.lock", "//other:Cargo.lock");
        assert!(cargo_hub_detail(&mismatch, &crates).contains("cargo_lockfile"));
        let unlisted =
            hub("use_repo(crate, \"crates\")\n").replace("cargo-bazel-lock.json", "other.json");
        assert!(cargo_hub_detail(&unlisted, &crates).contains("not listed in the set's locks"));
        let mut stray = crates.clone();
        stray.manifests.push("stray/Cargo.toml".to_owned());
        assert!(
            cargo_hub_detail(&hub("use_repo(crate, \"crates\")\n"), &stray)
                .contains("stray/Cargo.toml")
        );
    }

    #[test]
    fn cargo_sets_may_span_directories() {
        let set = hub_set(
            "a",
            Ecosystem::Cargo,
            strings(&["a/Cargo.toml", "b/Cargo.toml"]),
            strings(&["a/Cargo.lock"]),
        )
        .expect("valid");
        assert_eq!(set.manifests.len(), 2);
        assert_eq!(set.scopes, ["a", "b"]);
    }

    #[test]
    fn cargo_sets_load_with_their_repository() {
        let registry = cargo_sets();
        assert_eq!(registry.names(), vec!["crates", "tool_crates"]);
        let crates = registry.find("crates").expect("crates");
        assert_eq!(crates.ecosystem, Ecosystem::Cargo);
        assert_eq!(crates.repository.as_deref(), Some("crates"));
        let resolved = ResolvedSet::of(crates);
        assert_eq!(resolved.dir, "apps/rs");
        assert_eq!(resolved.repository.as_deref(), Some("crates"));
        assert_eq!(Ecosystem::parse("cargo"), Some(Ecosystem::Cargo));
        assert_eq!(Ecosystem::Cargo.name(), "cargo");
        assert_eq!(Ecosystem::Cargo.manifest_name(), "Cargo.toml");
        assert_eq!(Ecosystem::Cargo.supported_check(), "check");
        assert_eq!(Ecosystem::Cargo.supported_update(), "update");
        assert_eq!(
            Ecosystem::Cargo.supported_operations(),
            "check, update, audit"
        );
        assert_eq!(Ecosystem::Cargo.advisory_family(), Some("cargo"));
        assert!(Ecosystem::Cargo.uses_repository());
        for other in [
            Ecosystem::Uv,
            Ecosystem::Npm,
            Ecosystem::Bazel,
            Ecosystem::Ruby,
            Ecosystem::PowerShell,
        ] {
            assert!(!other.uses_repository(), "{}", other.name());
        }
        let uv = two_uv();
        assert_eq!(uv.find("frontend").expect("frontend").repository, None);
    }

    #[test]
    fn cargo_sets_must_declare_cargo_lock_beside_the_manifest() {
        let registry = cargo_sets();
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-cargo-");
        let workspace = scratch.path().to_path_buf();
        std::fs::write(workspace.join(MODULE_BAZEL), CARGO_HUBS).expect("module");
        for name in ["crates", "tool_crates"] {
            let set = ResolvedSet::of(registry.find(name).expect("set"));
            assert_eq!(check_backend_settings(&workspace, &set), Ok(()), "{name}");
        }
        let set = ResolvedSet::of(
            &hub_set(
                "a",
                Ecosystem::Cargo,
                strings(&["a/Cargo.toml"]),
                strings(&["a/cargo-bazel-lock.json"]),
            )
            .expect("valid"),
        );
        assert_eq!(
            check_backend_settings(&workspace, &set),
            Err(DependencySetsError::MissingPinnedLock {
                name: "a".to_owned(),
                path: "a/Cargo.lock".to_owned(),
                ecosystem: "cargo",
            })
        );
    }

    #[test]
    fn cargo_packages_use_the_crate_charset() {
        let registry = cargo_sets();
        for package in ["serde", "serde_json", "tokio-util", "Inflector"] {
            let resolved = resolve(&registry, &[format!("crates:{package}")])
                .unwrap_or_else(|error| panic!("{package}: {error}"));
            assert_eq!(
                resolved[0].request,
                Selection::Packages(vec![package.to_owned()])
            );
        }
        for package in ["serde@1.0.0", "a.b", "a b", "a;rm", "--locked", "_a", "a/b"] {
            assert_eq!(
                resolve(&registry, &[format!("crates:{package}")]),
                Err(DependencySetsError::InvalidPackage {
                    set: "crates".to_owned(),
                    package: package.to_owned(),
                    reason: CARGO_PACKAGE_CHARSET,
                }),
                "{package}"
            );
        }
    }
    const MAVEN_MODULE_TEXT: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"tool_maven\", lock_file = \"//tools:tool_install.json\")\nuse_repo(maven, \"maven\", \"tool_maven\")\n";

    fn maven_sets() -> Registry {
        registry(&[
            (
                "maven",
                Ecosystem::Maven,
                &["MODULE.bazel"],
                &["third_party/maven_install.json"],
            ),
            (
                "tool_maven",
                Ecosystem::Maven,
                &["MODULE.bazel"],
                &["tools/tool_install.json"],
            ),
            (
                "root",
                Ecosystem::Bazel,
                &["MODULE.bazel"],
                &["MODULE.bazel.lock"],
            ),
        ])
    }

    const SIGNED_LOCK: &str = "{\"__AUTOGENERATED_FILE_DO_NOT_MODIFY_THIS_FILE_MANUALLY\":\"x\",\"__INPUT_ARTIFACTS_HASH\":1,\"__RESOLVED_ARTIFACTS_HASH\":2}";

    fn maven_workspace(module: &str, lock: &str) -> (dx_test_scratch::TempDir, ResolvedSet) {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-maven-");
        let root = scratch.path();
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        std::fs::create_dir_all(root.join("third_party")).expect("dir");
        std::fs::write(root.join("third_party/maven_install.json"), lock).expect("lock");
        let registry = maven_sets();
        let set = ResolvedSet::of(registry.find("maven").expect("maven"));
        (scratch, set)
    }

    fn hub_detail(result: Result<(), DependencySetsError>) -> String {
        match result {
            Err(DependencySetsError::UnsupportedMavenHub { detail, .. }) => detail,
            other => panic!("expected UnsupportedMavenHub, got {other:?}"),
        }
    }

    #[test]
    fn maven_sets_share_the_root_module_but_own_their_locks() {
        let registry = maven_sets();
        assert_eq!(registry.names(), vec!["maven", "root", "tool_maven"]);
        let jvm = registry.find("maven").expect("maven");
        assert_eq!(jvm.ecosystem, Ecosystem::Maven);
        assert_eq!(jvm.repository.as_deref(), Some("maven"));
        assert_eq!(Ecosystem::parse("maven"), Some(Ecosystem::Maven));
        assert_eq!(Ecosystem::Maven.manifest_name(), "MODULE.bazel");
        assert_eq!(
            Ecosystem::Maven.supported_operations(),
            "check, update, audit"
        );
        assert_eq!(Ecosystem::Maven.advisory_family(), Some("maven"));
        assert!(Ecosystem::Maven.uses_repository());
        assert!(matches!(
            try_registry(&[
                (
                    "a",
                    Ecosystem::Maven,
                    &["MODULE.bazel"],
                    &["third_party/maven_install.json"],
                ),
                (
                    "b",
                    Ecosystem::Maven,
                    &["MODULE.bazel"],
                    &["third_party/maven_install.json"],
                ),
            ]),
            Err(DependencySetsError::ConflictingOwnership { .. })
        ));
        let two_uv = two_uv();
        assert!(two_uv.find("frontend").is_some());
    }

    #[test]
    fn maven_packages_are_group_and_artifact() {
        let registry = maven_sets();
        assert!(resolve(&registry, &["maven:com.google.guava:guava".to_owned()]).is_ok());
        for package in ["guava", "a:", ":b", "a:b:1.0", "a b:c", "a;x:b"] {
            assert_eq!(
                resolve(&registry, &[format!("maven:{package}")]),
                Err(DependencySetsError::InvalidPackage {
                    set: "maven".to_owned(),
                    package: package.to_owned(),
                    reason: MAVEN_PACKAGE_CHARSET,
                }),
                "{package}"
            );
        }
    }

    #[test]
    fn maven_hub_must_exist_in_the_root_module() {
        let (scratch, set) = maven_workspace(MAVEN_MODULE_TEXT, "");
        let root = scratch.path();
        assert_eq!(check_backend_settings(root, &set), Ok(()));
        let cases = [
            (
                "bazel_dep(name = \"x\", version = \"1\")\n",
                "no maven.install",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\n",
                "use_repo",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nuse_repo(maven, m2 = \"maven\")\n",
                "use_repo",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\")\nuse_repo(maven, \"maven\")\n",
                "literal lock_file",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//other:lock.json\")\nuse_repo(maven, \"maven\")\n",
                "not listed",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = L)\nuse_repo(maven, \"maven\")\n",
                "literal name and lock_file",
            ),
            (
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nmaven.install(name = \"maven\", lock_file = \"//third_party:maven_install.json\")\nuse_repo(maven, \"maven\")\n",
                "2 maven.install",
            ),
        ];
        for (module, want) in cases {
            std::fs::write(root.join("MODULE.bazel"), module).expect("module");
            let detail = hub_detail(check_backend_settings(root, &set));
            assert!(detail.contains(want), "{detail} should contain {want}");
        }
        std::fs::write(root.join("MODULE.bazel"), MAVEN_MODULE_TEXT).expect("module");
        std::fs::remove_file(root.join("third_party/maven_install.json")).expect("rm");
        assert!(hub_detail(check_backend_settings(root, &set)).contains("does not exist"));
    }

    #[test]
    fn maven_check_needs_a_signed_lock_and_fail_if_repin_required() {
        let (scratch, set) = maven_workspace(MAVEN_MODULE_TEXT, SIGNED_LOCK);
        let root = scratch.path();
        assert_eq!(check_lock_ready(root, &set), Ok(()));
        let lock = root.join("third_party/maven_install.json");
        for bad in [
            "",
            "{}",
            "not json",
            "[1]",
            "{\"__INPUT_ARTIFACTS_HASH\":1}",
            "{\"dependency_tree\":{}}",
        ] {
            std::fs::write(&lock, bad).expect("lock");
            assert!(
                hub_detail(check_lock_ready(root, &set)).contains("signatures"),
                "{bad}"
            );
        }
        std::fs::write(&lock, SIGNED_LOCK).expect("lock");
        for policy in ["False", "FLAG"] {
            let module = MAVEN_MODULE_TEXT.replacen(
                "maven_install.json\")",
                &format!("maven_install.json\", fail_if_repin_required = {policy})"),
                1,
            );
            std::fs::write(root.join("MODULE.bazel"), module).expect("module");
            assert!(hub_detail(check_lock_ready(root, &set)).contains("fail_if_repin_required"));
        }
        let module = MAVEN_MODULE_TEXT.replacen(
            "maven_install.json\")",
            "maven_install.json\", fail_if_repin_required = True)",
            1,
        );
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        assert_eq!(check_lock_ready(root, &set), Ok(()));
        let registry = two_uv();
        let uv = ResolvedSet::of(registry.find("frontend").expect("frontend"));
        assert_eq!(check_lock_ready(root, &uv), Ok(()));
    }

    const NUGET_LOCKS: [&str; 2] = [
        "third_party/dotnet/paket.lock",
        "third_party/dotnet/deps/paket.main.bzl",
    ];

    fn nuget_set(locks: &[&str]) -> DependencySet {
        hub_set(
            "dotnet",
            Ecosystem::Nuget,
            strings(&["third_party/dotnet/paket.dependencies"]),
            strings(locks),
        )
        .expect("valid")
    }

    const NUGET_MODULE: &str = "module(name = \"consumer\")\n";

    fn nuget_workspace(
        locks: &[&str],
        module: &str,
        dependencies: &str,
    ) -> (dx_test_scratch::TempDir, ResolvedSet) {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-nuget-");
        let root = scratch.path();
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        std::fs::create_dir_all(root.join("third_party/dotnet/deps")).expect("dir");
        std::fs::write(
            root.join("third_party/dotnet/paket.dependencies"),
            dependencies,
        )
        .expect("dependencies");
        (scratch, ResolvedSet::of(&nuget_set(locks)))
    }

    fn nuget_detail(result: Result<(), DependencySetsError>) -> String {
        match result {
            Err(DependencySetsError::UnsupportedNugetLayout { detail, .. }) => detail,
            other => panic!("expected UnsupportedNugetLayout, got {other:?}"),
        }
    }

    #[test]
    fn nuget_sets_load_with_paket_manifests() {
        let set = nuget_set(&NUGET_LOCKS);
        let registry = Registry {
            sets: vec![set.clone()],
        };
        let dotnet = &set;
        assert_eq!(dotnet.ecosystem, Ecosystem::Nuget);
        assert_eq!(Ecosystem::parse("nuget"), Some(Ecosystem::Nuget));
        assert_eq!(Ecosystem::Nuget.manifest_name(), "paket.dependencies");
        assert_eq!(
            Ecosystem::Nuget.supported_operations(),
            "check, update, audit"
        );
        assert_eq!(Ecosystem::Nuget.advisory_family(), Some("nuget"));
        assert!(!Ecosystem::Nuget.uses_repository());
        assert!(matches!(
            hub_set(
                "dotnet",
                Ecosystem::Nuget,
                strings(&["third_party/dotnet/packages.config"]),
                strings(&NUGET_LOCKS),
            ),
            Err(DependencySetsError::InvalidManifest { .. })
        ));
        assert!(resolve(&registry, &["dotnet:Newtonsoft.Json".to_owned()]).is_ok());
        for package in ["a b", "a;b", "a/b"] {
            assert_eq!(
                resolve(&registry, &[format!("dotnet:{package}")]),
                Err(DependencySetsError::InvalidPackage {
                    set: "dotnet".to_owned(),
                    package: package.to_owned(),
                    reason: NUGET_PACKAGE_CHARSET,
                }),
                "{package}"
            );
        }
    }

    #[test]
    fn nuget_layout_accepts_one_folder_of_groups() {
        let (scratch, set) = nuget_workspace(
            &NUGET_LOCKS,
            NUGET_MODULE,
            "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json\n",
        );
        assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
    }

    #[test]
    fn nuget_layout_refuses_what_staging_cannot_reproduce() {
        let dependencies = "source https://api.nuget.org/v3/index.json\n";
        let main = "third_party/dotnet/deps/paket.main.bzl";
        let lock = "third_party/dotnet/paket.lock";
        let notes: &[&str] = &[lock, "third_party/dotnet/deps/notes.txt"];
        let other: &[&str] = &[lock, main, "third_party/dotnet/other/paket.docs.bzl"];
        let extension: &[&str] = &[
            lock,
            main,
            "third_party/dotnet/deps/paket.main_extension.bzl",
        ];
        let cases: [(&[&str], &str, &str, &str); 4] = [
            (notes, NUGET_MODULE, dependencies, "neither paket.lock"),
            (other, NUGET_MODULE, dependencies, "share one folder"),
            (
                extension,
                NUGET_MODULE,
                dependencies,
                "paket2bazel extension",
            ),
            (
                &NUGET_LOCKS,
                NUGET_MODULE,
                "source ../local-feed\n",
                "not http(s)",
            ),
        ];
        let (scratch, set) = nuget_workspace(&[main], NUGET_MODULE, dependencies);
        assert!(matches!(
            check_backend_settings(scratch.path(), &set),
            Err(DependencySetsError::MissingPinnedLock { .. })
        ));
        for (locks, module, dependencies, want) in cases {
            let (scratch, set) = nuget_workspace(locks, module, dependencies);
            let detail = nuget_detail(check_backend_settings(scratch.path(), &set));
            assert!(detail.contains(want), "{want}: {detail}");
        }
    }

    const GO_SUM: [&str; 1] = ["third_party/go/go.sum"];

    fn go_set(manifest: &str, locks: &[&str]) -> Result<DependencySet, DependencySetsError> {
        hub_set("go", Ecosystem::Go, strings(&[manifest]), strings(locks))
    }

    const GO_MODULE: &str = "bazel_dep(name = \"rules_go\", version = \"0.50.1\")\n";

    const GO_MOD_TEXT: &str =
        "module example.com/m\n\ngo 1.22\n\nrequire github.com/google/go-cmp v0.7.0\n";

    fn go_workspace(
        locks: &[&str],
        module: &str,
        go_mod: &str,
    ) -> (dx_test_scratch::TempDir, ResolvedSet) {
        let scratch = dx_test_scratch::scratch("dx-dependency-sets-go-");
        let root = scratch.path();
        std::fs::write(root.join("MODULE.bazel"), module).expect("module");
        std::fs::create_dir_all(root.join("third_party/go")).expect("dir");
        std::fs::write(root.join("third_party/go/go.mod"), go_mod).expect("go.mod");
        let set = go_set("third_party/go/go.mod", locks).expect("valid");
        (scratch, ResolvedSet::of(&set))
    }

    fn go_detail(result: Result<(), DependencySetsError>) -> String {
        match result {
            Err(DependencySetsError::UnsupportedGoLayout { detail, .. }) => detail,
            other => panic!("expected UnsupportedGoLayout, got {other:?}"),
        }
    }

    #[test]
    fn go_sets_load_with_go_mod_manifests() {
        let registry = Registry {
            sets: vec![go_set("third_party/go/go.mod", &GO_SUM).expect("valid")],
        };
        let set = registry.find("go").expect("go");
        assert_eq!(set.ecosystem, Ecosystem::Go);
        assert_eq!(Ecosystem::parse("go"), Some(Ecosystem::Go));
        assert_eq!(Ecosystem::Go.manifest_name(), "go.mod");
        assert_eq!(Ecosystem::Go.supported_operations(), "check, update, audit");
        assert_eq!(Ecosystem::Go.advisory_family(), Some("go"));
        assert!(!Ecosystem::Go.uses_repository());
        assert!(matches!(
            go_set("third_party/go/go.work", &GO_SUM),
            Err(DependencySetsError::InvalidManifest { .. })
        ));
        assert!(resolve(&registry, &["go:github.com/google/go-cmp".to_owned()]).is_ok());
        assert!(matches!(
            resolve(&registry, &["go:a b".to_owned()]),
            Err(DependencySetsError::InvalidPackage { .. })
        ));
    }

    #[test]
    fn go_layout_accepts_one_go_mod_with_go_sum() {
        let (scratch, set) = go_workspace(&GO_SUM, GO_MODULE, GO_MOD_TEXT);
        assert_eq!(check_backend_settings(scratch.path(), &set), Ok(()));
    }

    #[test]
    fn go_layout_refuses_what_staging_cannot_reproduce() {
        let (scratch, set) = go_workspace(&["third_party/go/go.work.sum"], GO_MODULE, GO_MOD_TEXT);
        assert!(matches!(
            check_backend_settings(scratch.path(), &set),
            Err(DependencySetsError::MissingPinnedLock { .. })
        ));
        let extra: &[&str] = &["third_party/go/go.sum", "third_party/go/go.work.sum"];
        let replaced = format!("{GO_MOD_TEXT}\nreplace example.com/x => ../x\n");
        let renamed = "bazel_dep(name = \"rules_go\", version = \"0.50.1\", repo_name = \"io_bazel_rules_go\")\n";
        let with_env = format!(
            "{GO_MODULE}\ngo_deps.config(go_env = {{\"GOPRIVATE\": \"corp.example.com\"}})\n"
        );
        let cases: [(&[&str], &str, &str, &str); 6] = [
            (&GO_SUM, with_env.as_str(), GO_MOD_TEXT, "go_env"),
            (extra, GO_MODULE, GO_MOD_TEXT, "exactly"),
            (&GO_SUM, GO_MODULE, replaced.as_str(), "local path"),
            (&GO_SUM, GO_MODULE, "not a go.mod {", "cannot parse"),
            (&GO_SUM, "", GO_MOD_TEXT, "bazel_dep(name = \"rules_go\")"),
            (&GO_SUM, renamed, GO_MOD_TEXT, "repo_name"),
        ];
        for (locks, module, go_mod, want) in cases {
            let (scratch, set) = go_workspace(locks, module, go_mod);
            let detail = go_detail(check_backend_settings(scratch.path(), &set));
            assert!(detail.contains(want), "{want}: {detail}");
        }
    }
}
