use super::gha::WORKFLOW_DIR;
use super::sets::BumpSet;
use super::version::{self, VersionError, WidenVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BumpRequest {
    pub set: BumpSet,
    pub package: String,
    pub version: WidenVersion,
    pub selector: String,
    pub raw_version: String,
    manifest: Option<String>,
    /// The configured npm, uv or bazel set this request edits, if any.
    configured: Option<ConfiguredSet>,
    /// Which of a MODULE.bazel dep's two versions a bazel bump edits.
    pub dep_version: DepVersion,
}

/// The ecosystem of a configured dx.toml set a bump edits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfiguredKind {
    Npm,
    Uv,
    /// A bazel-modules set; its lock is refreshed with `bazel mod deps`.
    Bazel,
    /// A cargo set; every manifest that declares the crate is widened.
    Cargo,
    /// A maven set; only its own `maven.install` is edited.
    Maven,
    /// A nuget set; `paket.dependencies` is edited and `paket.lock` is not re-resolved.
    Nuget,
    /// A go set; only the `require` line of its go.mod is edited, then go.sum is refreshed.
    Go,
}

impl ConfiguredKind {
    /// The selector and version grammar of this ecosystem.
    pub fn grammar(self) -> BumpSet {
        match self {
            ConfiguredKind::Cargo => BumpSet::Cargo,
            ConfiguredKind::Maven => BumpSet::Maven,
            ConfiguredKind::Nuget => BumpSet::NuGet,
            ConfiguredKind::Go => BumpSet::Go,
            ConfiguredKind::Npm => BumpSet::Npm,
            ConfiguredKind::Uv | ConfiguredKind::Bazel => BumpSet::Bazel,
        }
    }
}

/// A configured dx.toml set a bump edits: its name, ecosystem, manifests and hub repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredSet {
    pub name: String,
    pub kind: ConfiguredKind,
    /// Every manifest of the set; a configured bump never reads the builtin tables.
    pub manifests: Vec<String>,
    /// The hub repository of a maven set; it names the `maven.install` to edit.
    pub repository: Option<String>,
}

/// The version of a MODULE.bazel dependency a bazel bump edits.
///
/// Each consumer-facing dep carries a floor in its `bazel_dep` (the oldest version
/// consumers may resolve) and a tested-latest in a root-only `single_version_override`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum DepVersion {
    /// The tested-latest version: the `single_version_override` version when one has a
    /// literal version, else the `bazel_dep` version.
    #[default]
    Tested,
    /// The floor in the `bazel_dep`, with the `# floor: <reason>` comment above it.
    Floor { reason: String },
}

impl DepVersion {
    /// The target `--floor` and `--reason` select; both or neither must be given.
    pub fn from_flags(floor: bool, reason: Option<&str>) -> Result<DepVersion, BumpError> {
        match (floor, reason) {
            (false, None) => Ok(DepVersion::Tested),
            (true, None) => Err(BumpError::FloorNeedsReason),
            (false, Some(_)) => Err(BumpError::ReasonNeedsFloor),
            (true, Some(reason)) => {
                let reason = reason.trim();
                if reason.is_empty() || reason.chars().any(char::is_control) {
                    return Err(BumpError::UnsafeReason {
                        reason: reason.to_owned(),
                    });
                }
                Ok(DepVersion::Floor {
                    reason: reason.to_owned(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BumpPlanOutcome {
    Changed(String),
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BumpError {
    #[error("empty bump selector or version; expected `dx bump <set:package> <version>`")]
    Empty,
    #[error("unknown bump selector {selector:?}; expected {} as `set:package` (e.g. cargo:anyhow, maven:junit:junit)", BumpSet::pipe_list())]
    UnknownSelector { selector: String },
    #[error("invalid package {package:?} for set {set}: {reason}")]
    InvalidPackage {
        set: &'static str,
        package: String,
        reason: &'static str,
    },
    #[error("bump needs one package, not a whole set: {set:?} selects the set; use `{set}:<package> <version>`")]
    BareSet { set: String },
    #[error("bump needs `set:package`, not a label or path: {target:?}")]
    NotAPackage { target: String },
    #[error("no declared requirement for {package:?} in {manifest} (nothing widened)")]
    NotFound { manifest: String, package: String },
    #[error("ambiguous requirement for {package:?} in {manifest}: {count} matches (nothing widened; widen one requirement per invocation)")]
    Ambiguous {
        manifest: String,
        package: String,
        count: usize,
    },
    #[error("unsupported manifest shape in {manifest}: {reason} (nothing widened)")]
    UnsupportedManifest { manifest: String, reason: String },
    #[error("github-actions {package:?} tag {tag:?} needs SHA resolution via the upstream GitHub releases client; pass the resolved SHA as <version> (nothing widened)")]
    NeedsSha { package: String, tag: String },
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error("--floor needs --reason <text> naming why this is the oldest supported version")]
    FloorNeedsReason,
    #[error("--reason only applies with --floor")]
    ReasonNeedsFloor,
    #[error("--floor applies only to a MODULE.bazel bazel_dep, not {selector:?}")]
    FloorNotBazelDep { selector: String },
    #[error("floor reason {reason:?} must be one non-empty line")]
    UnsafeReason { reason: String },
    #[error("floor {floor} of {package:?} would be above its tested version {tested} in the single_version_override; bump the tested version first (nothing changed)")]
    FloorAboveTested {
        package: String,
        floor: String,
        tested: String,
    },
    #[error("tested version {tested} of {package:?} would be below its floor {floor} in the bazel_dep; lower the floor first with --floor --reason (nothing changed)")]
    TestedBelowFloor {
        package: String,
        tested: String,
        floor: String,
    },
}

impl BumpError {
    /// Whether this is an argument error rather than a manifest or network failure.
    pub fn is_usage(&self) -> bool {
        matches!(
            self,
            BumpError::FloorNeedsReason
                | BumpError::ReasonNeedsFloor
                | BumpError::FloorNotBazelDep { .. }
                | BumpError::UnsafeReason { .. }
                | BumpError::FloorAboveTested { .. }
                | BumpError::TestedBelowFloor { .. }
        )
    }
}

/// One declared dependency a manifest edit targets: its set, package and manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepTarget {
    pub set: BumpSet,
    pub package: String,
    pub manifest: Option<String>,
    /// The configured npm, uv or bazel set the dependency belongs to, if any.
    pub configured: Option<ConfiguredSet>,
}

impl DepTarget {
    pub fn needs_update_refresh(&self) -> bool {
        match &self.configured {
            Some(set) => set.kind != ConfiguredKind::Nuget,
            None => self.set.needs_update_refresh(),
        }
    }

    pub fn refresh_selector(&self) -> String {
        configured_refresh_selector(self.configured.as_ref(), &self.package)
            .unwrap_or_else(|| refresh_selector(self.set, &self.package))
    }

    /// Whether the dependency belongs to a configured bazel-modules set.
    pub fn is_configured_bazel(&self) -> bool {
        matches!(&self.configured, Some(set) if set.kind == ConfiguredKind::Bazel)
    }

    /// What the user must do after the edit when nothing re-resolves the lock.
    pub fn follow_up(&self) -> Option<String> {
        let set = self.configured.as_ref()?;
        (set.kind == ConfiguredKind::Nuget).then(|| {
            let dir = set
                .manifests
                .first()
                .and_then(|manifest| manifest.rsplit_once('/'))
                .map_or(".", |(dir, _)| dir);
            format!(
                "paket.lock is not re-resolved: run `paket update` in {dir}, then `dx update {}`",
                set.name
            )
        })
    }

    /// The command that refreshes this dependency's lock, as shown to the user.
    pub fn refresh_command(&self) -> String {
        if self.is_configured_bazel() {
            "bazel mod deps".to_owned()
        } else {
            format!("dx update {}", self.refresh_selector())
        }
    }

    pub fn target_manifest(&self) -> &str {
        target_manifest(self.set, &self.package, self.manifest.as_deref())
    }
}

fn configured_refresh_selector(
    configured: Option<&ConfiguredSet>,
    package: &str,
) -> Option<String> {
    configured.map(|set| match set.kind {
        ConfiguredKind::Npm => format!("{}:{package}", set.name),
        ConfiguredKind::Uv
        | ConfiguredKind::Bazel
        | ConfiguredKind::Cargo
        | ConfiguredKind::Maven
        | ConfiguredKind::Nuget
        | ConfiguredKind::Go => set.name.clone(),
    })
}

fn refresh_selector(set: BumpSet, package: &str) -> String {
    match set {
        BumpSet::Npm => format!("{}:{}", set.name(), package),
        BumpSet::Bazel | BumpSet::GithubActions => String::new(),
        set => set.name().to_owned(),
    }
}

fn target_manifest<'a>(set: BumpSet, package: &str, manifest: Option<&'a str>) -> &'a str {
    if let Some(manifest) = manifest {
        return manifest;
    }
    match set {
        BumpSet::Bazel => {
            if package == ".bazelversion" {
                ".bazelversion"
            } else {
                "MODULE.bazel"
            }
        }
        BumpSet::Cargo => "rust/tests/fixtures/hello/Cargo.toml",
        BumpSet::GithubActions => WORKFLOW_DIR,
        BumpSet::Go => "third_party/go/go.mod",
        BumpSet::Maven => "MODULE.bazel",
        BumpSet::Npm => "package.json",
        BumpSet::NuGet => "third_party/dotnet/paket.dependencies",
    }
}

impl BumpRequest {
    pub fn parse(selector: &str, version: &str) -> Result<BumpRequest, BumpError> {
        if selector.is_empty() || version.is_empty() {
            return Err(BumpError::Empty);
        }
        let (set, package) = parse_selector(selector)?;
        let parsed = version::parse(set, version)?;
        Ok(BumpRequest {
            set,
            package,
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
            manifest: None,
            configured: None,
            dep_version: DepVersion::Tested,
        })
    }

    pub fn parse_configured(
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<BumpRequest, BumpError> {
        Self::parse_configured_as(ConfiguredKind::Bazel, set_name, manifest, selector, version)
    }

    /// Parses a request against a configured set of `kind`.
    pub fn parse_configured_as(
        kind: ConfiguredKind,
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<BumpRequest, BumpError> {
        Self::parse_configured_set(
            kind,
            set_name,
            &[manifest.to_owned()],
            None,
            selector,
            version,
        )
    }

    /// Parses a request against a configured set of `kind` with all its manifests.
    pub fn parse_configured_set(
        kind: ConfiguredKind,
        set_name: &str,
        manifests: &[String],
        repository: Option<&str>,
        selector: &str,
        version: &str,
    ) -> Result<BumpRequest, BumpError> {
        if selector.is_empty() || version.is_empty() {
            return Err(BumpError::Empty);
        }
        let package = match kind {
            ConfiguredKind::Bazel => parse_configured_selector(set_name, selector)?,
            ConfiguredKind::Npm
            | ConfiguredKind::Uv
            | ConfiguredKind::Cargo
            | ConfiguredKind::Maven
            | ConfiguredKind::Nuget
            | ConfiguredKind::Go => parse_foreign_selector(set_name, kind, selector)?,
        };
        let set = kind.grammar();
        let parsed = version::parse(set, version)?;
        Ok(BumpRequest {
            set,
            package,
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
            manifest: manifests.first().cloned(),
            configured: Some(ConfiguredSet {
                name: set_name.to_owned(),
                kind,
                manifests: manifests.to_vec(),
                repository: repository.map(str::to_owned),
            }),
            dep_version: DepVersion::Tested,
        })
    }

    pub fn needs_update_refresh(&self) -> bool {
        self.target().needs_update_refresh()
    }

    pub fn refresh_selector(&self) -> String {
        self.target().refresh_selector()
    }

    pub fn target_manifest(&self) -> &str {
        target_manifest(self.set, &self.package, self.manifest.as_deref())
    }

    /// Every manifest of the configured set this request edits, if it has one.
    pub fn configured_manifests(&self) -> Option<&[String]> {
        self.configured.as_ref().map(|set| set.manifests.as_slice())
    }

    /// The name of the configured set this request edits, if it has one.
    pub fn configured_name(&self) -> Option<&str> {
        self.configured.as_ref().map(|set| set.name.as_str())
    }

    /// The set, package and manifest this request edits.
    pub fn target(&self) -> DepTarget {
        DepTarget {
            set: self.set,
            package: self.package.clone(),
            manifest: self.manifest.clone(),
            configured: self.configured.clone(),
        }
    }

    /// A request that widens `target` to `version`.
    pub fn for_target(
        target: &DepTarget,
        version: WidenVersion,
        selector: &str,
        raw_version: &str,
    ) -> BumpRequest {
        BumpRequest {
            set: target.set,
            package: target.package.clone(),
            version,
            selector: selector.to_owned(),
            raw_version: raw_version.to_owned(),
            manifest: target.manifest.clone(),
            configured: target.configured.clone(),
            dep_version: DepVersion::Tested,
        }
    }

    /// This request aimed at `dep_version`; a floor applies only to a MODULE.bazel bazel_dep.
    pub fn with_dep_version(mut self, dep_version: DepVersion) -> Result<BumpRequest, BumpError> {
        if dep_version != DepVersion::Tested
            && (self.set != BumpSet::Bazel
                || (self.configured.is_some() && !self.target().is_configured_bazel())
                || self.package == ".bazelversion")
        {
            return Err(BumpError::FloorNotBazelDep {
                selector: self.selector,
            });
        }
        self.dep_version = dep_version;
        Ok(self)
    }

    pub fn summary(&self) -> String {
        let through = if let Some(follow_up) = self.target().follow_up() {
            follow_up
        } else if self.needs_update_refresh() {
            format!(
                "then refresh via `{}` automatically",
                self.target().refresh_command()
            )
        } else {
            "then preset flag-diff review plus `bazel build //...`".to_owned()
        };
        let place = if let Some(set) = self
            .configured
            .as_ref()
            .filter(|set| set.kind == ConfiguredKind::Cargo)
        {
            format!("every manifest of set {} that declares the crate", set.name)
        } else if self.set == BumpSet::Cargo && self.manifest.is_none() {
            "every manifest of the crate.from_cargo hub in MODULE.bazel".to_owned()
        } else {
            self.target_manifest().to_owned()
        };
        let what = match (&self.dep_version, self.set, self.package.as_str()) {
            (DepVersion::Floor { .. }, _, _) => " floor",
            (DepVersion::Tested, BumpSet::Bazel, package)
                if package != ".bazelversion"
                    && (self.configured.is_none() || self.target().is_configured_bazel()) =>
            {
                " tested override"
            }
            _ => "",
        };
        let base = format!(
            "Widen {}{what} to {} in {place} ({through})",
            self.selector,
            self.version.display(),
        );
        if self.version.is_semver() {
            format!("{base}; {}", version::generic_major_bump_hint())
        } else {
            base
        }
    }

    pub fn plan_edit(&self, content: &str) -> Result<String, BumpError> {
        self.plan_edit_in(self.target_manifest(), content)
    }

    /// The widened `content` of `manifest`, a manifest of this request's set.
    pub fn plan_edit_in(&self, manifest: &str, content: &str) -> Result<String, BumpError> {
        if matches!(&self.configured, Some(set) if set.kind == ConfiguredKind::Uv) {
            return plan_pyproject_toml(content, &self.package, &self.version);
        }
        match self.set {
            BumpSet::Bazel => {
                if self.package == ".bazelversion" {
                    plan_bazelversion(content, &self.version)
                } else {
                    plan_module_bazel(content, &self.package, &self.version, &self.dep_version)
                }
            }
            BumpSet::Cargo => plan_cargo_toml(manifest, content, &self.package, &self.version),
            BumpSet::Npm => plan_package_json(content, &self.package, &self.version),
            BumpSet::Go => plan_go_mod(manifest, content, &self.package, &self.version),
            BumpSet::Maven => plan_maven_module_bazel(
                manifest,
                content,
                &self.package,
                &self.version,
                self.configured
                    .as_ref()
                    .and_then(|set| set.repository.as_deref()),
            ),
            BumpSet::NuGet => {
                plan_paket_dependencies(manifest, content, &self.package, &self.version)
            }
            BumpSet::GithubActions => plan_github_workflow(content, &self.package, &self.version),
        }
    }

    pub fn plan_edit_outcome(
        &self,
        manifest: &str,
        content: &str,
    ) -> Result<BumpPlanOutcome, BumpError> {
        let widened = self.plan_edit_in(manifest, content)?;
        if widened == content {
            Ok(BumpPlanOutcome::Unchanged)
        } else {
            Ok(BumpPlanOutcome::Changed(widened))
        }
    }
}

/// Parses a `set:package` selector into its set and package identity.
pub fn parse_selector(selector: &str) -> Result<(BumpSet, String), BumpError> {
    if selector.is_empty() {
        return Err(BumpError::Empty);
    }
    if is_target_shape(selector) {
        return Err(BumpError::NotAPackage {
            target: selector.to_owned(),
        });
    }
    if let Some(set) = BumpSet::parse(selector) {
        return Err(BumpError::BareSet {
            set: set.name().to_owned(),
        });
    }
    let (head, tail) = match selector.split_once(':') {
        Some((head, tail)) => (head, tail),
        None => {
            return Err(BumpError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    let set = match BumpSet::parse(head) {
        Some(set) => set,
        None => {
            return Err(BumpError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    if tail.is_empty() {
        return Err(BumpError::InvalidPackage {
            set: set.name(),
            package: tail.to_owned(),
            reason: "package identity is empty",
        });
    }
    if set != BumpSet::Maven && tail.contains(':') {
        return Err(BumpError::InvalidPackage {
            set: set.name(),
            package: tail.to_owned(),
            reason: "package identity never contains ':'",
        });
    }
    validate_package(set, tail)?;
    Ok((set, tail.to_owned()))
}

/// Parses the `set:module` selector of a configured dx.toml bazel set into its module name.
pub fn parse_configured_selector(set_name: &str, selector: &str) -> Result<String, BumpError> {
    if selector.is_empty() {
        return Err(BumpError::Empty);
    }
    let Some((_, tail)) = selector.split_once(':') else {
        return Err(BumpError::BareSet {
            set: set_name.to_owned(),
        });
    };
    let invalid = |reason: &'static str| BumpError::InvalidPackage {
        set: BumpSet::Bazel.name(),
        package: tail.to_owned(),
        reason,
    };
    if tail.is_empty() {
        return Err(invalid("package identity is empty"));
    }
    if tail.contains(':') {
        return Err(invalid("package identity never contains ':'"));
    }
    if tail == ".bazelversion" {
        return Err(invalid(
            "configured bazel sets widen MODULE.bazel bazel_dep pins only",
        ));
    }
    validate_package(BumpSet::Bazel, tail)?;
    Ok(tail.to_owned())
}

/// Parses a configured npm or uv `set:package` selector into its package identity.
pub(crate) fn parse_foreign_selector(
    set_name: &str,
    kind: ConfiguredKind,
    selector: &str,
) -> Result<String, BumpError> {
    let Some((_, tail)) = selector.split_once(':') else {
        return Err(BumpError::BareSet {
            set: set_name.to_owned(),
        });
    };
    let set = kind.grammar();
    if tail.is_empty() {
        return Err(BumpError::InvalidPackage {
            set: set.name(),
            package: tail.to_owned(),
            reason: "package identity is empty",
        });
    }
    if kind == ConfiguredKind::Uv {
        let valid = tail
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && tail.starts_with(|c: char| c.is_ascii_alphanumeric());
        if !valid {
            return Err(BumpError::InvalidPackage {
                set: "uv",
                package: tail.to_owned(),
                reason: "python package names use letters, digits, '-', '_' and '.'",
            });
        }
    } else {
        validate_package(set, tail)?;
    }
    Ok(tail.to_owned())
}

#[path = "request_cargo.rs"]
mod cargo;
#[path = "request_plans.rs"]
pub(crate) mod plans;
#[path = "request_pyproject.rs"]
mod pyproject;

pub use cargo::cargo_manifests;
use cargo::*;
use plans::*;
use pyproject::plan_pyproject_toml;

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
