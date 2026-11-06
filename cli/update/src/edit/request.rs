use super::gha::WORKFLOW_DIR;
use super::sets::UpdateSet;
use super::version::{self, VersionError, WidenVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateRequest {
    pub set: UpdateSet,
    pub package: String,
    pub version: WidenVersion,
    pub selector: String,
    pub raw_version: String,
    manifest: Option<String>,
    /// The configured npm, uv or bazel set this request edits, if any.
    configured: Option<ConfiguredSet>,
    /// Which of a MODULE.bazel dep's two versions a bazel update edits.
    pub dep_version: DepVersion,
    resolved_tag: Option<String>,
}

/// The ecosystem of a dependency set an update edits.
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
    /// A ruby set; only the gem's line in the Gemfile is edited, then `bundle lock` runs.
    Ruby,
    /// A cpan set; only the module's `requires` line in the cpanfile is edited.
    Cpan,
    /// A pub set; only the package's constraint in pubspec.yaml is edited.
    Pub,
    /// A cabal set; only the package's bound in the `.cabal` file is edited.
    Cabal,
    /// A julia set; only the package's `[compat]` entry in Project.toml is edited.
    Julia,
    /// A composer set; only the constraint in composer.json is edited, then composer.lock is refreshed.
    Composer,
    /// A swiftpm set; only the package's `from:` or `exact:` version in Package.swift is edited.
    SwiftPm,
    /// An opam set; only the package's constraint in `depends:` of the `.opam` file is edited.
    Opam,
    /// A hex set; only the requirement in the `deps` of mix.exs is edited.
    Hex,
}

impl ConfiguredKind {
    /// The selector and version grammar of this ecosystem.
    pub fn grammar(self) -> UpdateSet {
        match self {
            ConfiguredKind::Cargo => UpdateSet::Cargo,
            ConfiguredKind::Maven => UpdateSet::Maven,
            ConfiguredKind::Nuget => UpdateSet::NuGet,
            ConfiguredKind::Go => UpdateSet::Go,
            ConfiguredKind::Ruby => UpdateSet::Ruby,
            ConfiguredKind::Cpan => UpdateSet::Cpan,
            ConfiguredKind::Pub => UpdateSet::Pub,
            ConfiguredKind::Composer => UpdateSet::Composer,
            ConfiguredKind::Cabal => UpdateSet::Cabal,
            ConfiguredKind::SwiftPm => UpdateSet::SwiftPm,
            ConfiguredKind::Opam => UpdateSet::Opam,
            ConfiguredKind::Hex => UpdateSet::Hex,
            ConfiguredKind::Julia => UpdateSet::Julia,
            ConfiguredKind::Npm => UpdateSet::Npm,
            ConfiguredKind::Uv | ConfiguredKind::Bazel => UpdateSet::Bazel,
        }
    }
}

/// A dependency set an update edits: its name, ecosystem, manifests and hub repository.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfiguredSet {
    pub name: String,
    pub kind: ConfiguredKind,
    /// Every manifest of the set; a configured update never reads the builtin tables.
    pub manifests: Vec<String>,
    /// The hub repository of a maven set; it names the `maven.install` to edit.
    pub repository: Option<String>,
}

/// The version of a MODULE.bazel dependency a bazel update edits.
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
    pub fn from_flags(floor: bool, reason: Option<&str>) -> Result<DepVersion, EditError> {
        match (floor, reason) {
            (false, None) => Ok(DepVersion::Tested),
            (true, None) => Err(EditError::FloorNeedsReason),
            (false, Some(_)) => Err(EditError::ReasonNeedsFloor),
            (true, Some(reason)) => {
                let reason = reason.trim();
                if reason.is_empty() || reason.chars().any(char::is_control) {
                    return Err(EditError::UnsafeReason {
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
pub enum EditPlanOutcome {
    Changed(String),
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("empty update selector or version; expected `dx update <set:package> <version>`")]
    Empty,
    #[error("unknown update selector {selector:?}; expected {} as `set:package` (e.g. bazel:rules_cc)", UpdateSet::builtin_names().join("|"))]
    UnknownSelector { selector: String },
    #[error("invalid package {package:?} for set {set}: {reason}")]
    InvalidPackage {
        set: &'static str,
        package: String,
        reason: &'static str,
    },
    #[error(
        "update needs one package, not a whole set: {set:?} selects the set; use `{set}:<package> <version>`"
    )]
    BareSet { set: String },
    #[error("update needs `set:package`, not a label or path: {target:?}")]
    NotAPackage { target: String },
    #[error("no declared requirement for {package:?} in {manifest} (nothing widened)")]
    NotFound { manifest: String, package: String },
    #[error(
        "ambiguous requirement for {package:?} in {manifest}: {count} matches (nothing widened; widen one requirement per invocation)"
    )]
    Ambiguous {
        manifest: String,
        package: String,
        count: usize,
    },
    #[error("unsupported manifest shape in {manifest}: {reason} (nothing widened)")]
    UnsupportedManifest { manifest: String, reason: String },
    #[error(
        "github-actions {package:?} tag {tag:?} needs SHA resolution via the upstream GitHub releases client; pass the resolved SHA as <version> (nothing widened)"
    )]
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
    #[error(
        "floor {floor} of {package:?} would be above its tested version {tested} in the single_version_override; update the tested version first (nothing changed)"
    )]
    FloorAboveTested {
        package: String,
        floor: String,
        tested: String,
    },
    #[error(
        "tested version {tested} of {package:?} would be below its floor {floor} in the bazel_dep; lower the floor first with --floor --reason (nothing changed)"
    )]
    TestedBelowFloor {
        package: String,
        tested: String,
        floor: String,
    },
}

impl EditError {
    /// Whether this is an argument error rather than a manifest or network failure.
    pub fn is_usage(&self) -> bool {
        matches!(
            self,
            EditError::FloorNeedsReason
                | EditError::ReasonNeedsFloor
                | EditError::FloorNotBazelDep { .. }
                | EditError::UnsafeReason { .. }
                | EditError::FloorAboveTested { .. }
                | EditError::TestedBelowFloor { .. }
        )
    }
}

/// One declared dependency a manifest edit targets: its set, package and manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepTarget {
    pub set: UpdateSet,
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

    /// Whether the dependency belongs to a configured ruby set.
    pub fn is_configured_ruby(&self) -> bool {
        matches!(&self.configured, Some(set) if set.kind == ConfiguredKind::Ruby)
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
        } else if self.is_configured_ruby() {
            format!("bundle lock --update {} --conservative", self.package)
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
        | ConfiguredKind::Cabal
        | ConfiguredKind::Go => set.name.clone(),
        ConfiguredKind::Ruby
        | ConfiguredKind::Cpan
        | ConfiguredKind::Pub
        | ConfiguredKind::Composer
        | ConfiguredKind::SwiftPm
        | ConfiguredKind::Opam
        | ConfiguredKind::Hex
        | ConfiguredKind::Julia => {
            format!("{}:{package}", set.name)
        }
    })
}

fn refresh_selector(set: UpdateSet, package: &str) -> String {
    match set {
        UpdateSet::Npm => format!("{}:{}", set.name(), package),
        UpdateSet::Bazel | UpdateSet::Containers | UpdateSet::GithubActions => String::new(),
        set => set.name().to_owned(),
    }
}

fn target_manifest<'a>(set: UpdateSet, package: &str, manifest: Option<&'a str>) -> &'a str {
    if let Some(manifest) = manifest {
        return manifest;
    }
    match set {
        UpdateSet::Bazel => {
            if package == ".bazelversion" {
                ".bazelversion"
            } else {
                "MODULE.bazel"
            }
        }
        UpdateSet::GithubActions => WORKFLOW_DIR,
        UpdateSet::Containers => "MODULE.bazel",
        UpdateSet::Cargo
        | UpdateSet::Go
        | UpdateSet::Maven
        | UpdateSet::Npm
        | UpdateSet::NuGet
        | UpdateSet::Cpan
        | UpdateSet::Opam
        | UpdateSet::Pub
        | UpdateSet::Cabal
        | UpdateSet::SwiftPm
        | UpdateSet::Hex
        | UpdateSet::Julia
        | UpdateSet::Ruby
        | UpdateSet::Composer => "",
    }
}

impl UpdateRequest {
    pub fn parse(selector: &str, version: &str) -> Result<UpdateRequest, EditError> {
        let head = selector.split_once(':').map_or(selector, |(head, _)| head);
        if !selector.is_empty() && !UpdateSet::builtin_names().contains(&head) {
            return Err(EditError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
        Self::parse_grammar(selector, version)
    }

    /// Parses a selector of any ecosystem; configured sets share these grammars.
    pub(crate) fn parse_grammar(selector: &str, version: &str) -> Result<UpdateRequest, EditError> {
        if selector.is_empty() || version.is_empty() {
            return Err(EditError::Empty);
        }
        let (set, package) = parse_selector(selector)?;
        let parsed = version::parse(set, version)?;
        Ok(UpdateRequest {
            set,
            package,
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
            manifest: None,
            configured: None,
            dep_version: DepVersion::Tested,
            resolved_tag: None,
        })
    }

    pub fn parse_configured(
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<UpdateRequest, EditError> {
        Self::parse_configured_as(ConfiguredKind::Bazel, set_name, manifest, selector, version)
    }

    /// Parses a request against a configured set of `kind`.
    pub fn parse_configured_as(
        kind: ConfiguredKind,
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<UpdateRequest, EditError> {
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
    ) -> Result<UpdateRequest, EditError> {
        if selector.is_empty() || version.is_empty() {
            return Err(EditError::Empty);
        }
        let package = match kind {
            ConfiguredKind::Bazel => parse_configured_selector(set_name, selector)?,
            ConfiguredKind::Npm
            | ConfiguredKind::Uv
            | ConfiguredKind::Cargo
            | ConfiguredKind::Maven
            | ConfiguredKind::Nuget
            | ConfiguredKind::Go
            | ConfiguredKind::Ruby
            | ConfiguredKind::Cpan
            | ConfiguredKind::Pub
            | ConfiguredKind::Cabal
            | ConfiguredKind::Composer
            | ConfiguredKind::SwiftPm
            | ConfiguredKind::Opam
            | ConfiguredKind::Hex
            | ConfiguredKind::Julia => parse_foreign_selector(set_name, kind, selector)?,
        };
        let set = kind.grammar();
        let parsed = version::parse(set, version)?;
        Ok(UpdateRequest {
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
            resolved_tag: None,
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

    /// The ecosystem of the configured set this request edits, if it has one.
    pub fn configured_kind(&self) -> Option<ConfiguredKind> {
        self.configured.as_ref().map(|set| set.kind)
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
    ) -> UpdateRequest {
        UpdateRequest {
            set: target.set,
            package: target.package.clone(),
            version,
            selector: selector.to_owned(),
            raw_version: raw_version.to_owned(),
            manifest: target.manifest.clone(),
            configured: target.configured.clone(),
            dep_version: DepVersion::Tested,
            resolved_tag: None,
        }
    }

    /// This request aimed at `dep_version`; a floor applies only to a MODULE.bazel bazel_dep.
    pub fn with_dep_version(mut self, dep_version: DepVersion) -> Result<UpdateRequest, EditError> {
        if dep_version != DepVersion::Tested
            && (self.set != UpdateSet::Bazel
                || (self.configured.is_some() && !self.target().is_configured_bazel())
                || self.package == ".bazelversion")
        {
            return Err(EditError::FloorNotBazelDep {
                selector: self.selector,
            });
        }
        self.dep_version = dep_version;
        Ok(self)
    }

    /// This tag request with the tag already resolved to `sha`; the tag still rewrites the version comment.
    pub fn with_resolved_sha(&self, sha: &str) -> Result<UpdateRequest, EditError> {
        let WidenVersion::GitTag(tag) = &self.version else {
            return Err(EditError::Empty);
        };
        let mut resolved = UpdateRequest::parse(&self.selector, sha)?;
        resolved.resolved_tag = Some(tag.clone());
        Ok(resolved)
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
        } else if self.set == UpdateSet::Cargo && self.manifest.is_none() {
            "every manifest of the crate.from_cargo hub in MODULE.bazel".to_owned()
        } else {
            self.target_manifest().to_owned()
        };
        let what = match (&self.dep_version, self.set, self.package.as_str()) {
            (DepVersion::Floor { .. }, _, _) => " floor",
            (DepVersion::Tested, UpdateSet::Bazel, package)
                if package != ".bazelversion"
                    && (self.configured.is_none() || self.target().is_configured_bazel()) =>
            {
                " tested override"
            }
            _ => "",
        };
        format!(
            "Widen {}{what} to {} in {place} ({through})",
            self.selector,
            self.version.display(),
        )
    }

    pub fn plan_edit(&self, content: &str) -> Result<String, EditError> {
        self.plan_edit_in(self.target_manifest(), content)
    }

    /// The widened `content` of `manifest`, a manifest of this request's set.
    pub fn plan_edit_in(&self, manifest: &str, content: &str) -> Result<String, EditError> {
        if matches!(&self.configured, Some(set) if set.kind == ConfiguredKind::Uv) {
            return plan_pyproject_toml(content, &self.package, &self.version);
        }
        match self.set {
            UpdateSet::Bazel => {
                if self.package == ".bazelversion" {
                    plan_bazelversion(content, &self.version)
                } else {
                    plan_module_bazel(content, &self.package, &self.version, &self.dep_version)
                }
            }
            UpdateSet::Containers => plan_oci_pull(content, &self.package, &self.version),
            UpdateSet::Cargo => plan_cargo_toml(manifest, content, &self.package, &self.version),
            UpdateSet::Npm => plan_package_json(content, &self.package, &self.version),
            UpdateSet::Go => plan_go_mod(manifest, content, &self.package, &self.version),
            UpdateSet::Maven => plan_maven_module_bazel(
                manifest,
                content,
                &self.package,
                &self.version,
                self.configured
                    .as_ref()
                    .and_then(|set| set.repository.as_deref()),
            ),
            UpdateSet::NuGet => {
                plan_paket_dependencies(manifest, content, &self.package, &self.version)
            }
            UpdateSet::Ruby => plan_gemfile(manifest, content, &self.package, &self.version),
            UpdateSet::Cpan => plan_cpanfile(manifest, content, &self.package, &self.version),
            UpdateSet::Pub => plan_pubspec(manifest, content, &self.package, &self.version),
            UpdateSet::Cabal => plan_cabal(manifest, content, &self.package, &self.version),
            UpdateSet::Hex => plan_mix_exs(manifest, content, &self.package, &self.version),
            UpdateSet::Julia => plan_julia_project(manifest, content, &self.package, &self.version),
            UpdateSet::Composer => {
                plan_composer_json(manifest, content, &self.package, &self.version)
            }
            UpdateSet::SwiftPm => {
                plan_package_swift(manifest, content, &self.package, &self.version)
            }
            UpdateSet::Opam => plan_opam(manifest, content, &self.package, &self.version),
            UpdateSet::GithubActions => plan_github_workflow(
                content,
                &self.package,
                &self.version,
                self.resolved_tag.as_deref(),
            ),
        }
    }

    pub fn plan_edit_outcome(
        &self,
        manifest: &str,
        content: &str,
    ) -> Result<EditPlanOutcome, EditError> {
        let widened = self.plan_edit_in(manifest, content)?;
        if widened == content {
            Ok(EditPlanOutcome::Unchanged)
        } else {
            Ok(EditPlanOutcome::Changed(widened))
        }
    }
}

/// Parses a `set:package` selector into its set and package identity.
pub fn parse_selector(selector: &str) -> Result<(UpdateSet, String), EditError> {
    if selector.is_empty() {
        return Err(EditError::Empty);
    }
    if is_target_shape(selector) {
        return Err(EditError::NotAPackage {
            target: selector.to_owned(),
        });
    }
    if let Some(set) = UpdateSet::parse(selector) {
        return Err(EditError::BareSet {
            set: set.name().to_owned(),
        });
    }
    let (head, tail) = match selector.split_once(':') {
        Some((head, tail)) => (head, tail),
        None => {
            return Err(EditError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    let set = match UpdateSet::parse(head) {
        Some(set) => set,
        None => {
            return Err(EditError::UnknownSelector {
                selector: selector.to_owned(),
            });
        }
    };
    if tail.is_empty() {
        return Err(EditError::InvalidPackage {
            set: set.name(),
            package: tail.to_owned(),
            reason: "package identity is empty",
        });
    }
    if set != UpdateSet::Maven && tail.contains(':') {
        return Err(EditError::InvalidPackage {
            set: set.name(),
            package: tail.to_owned(),
            reason: "package identity never contains ':'",
        });
    }
    validate_package(set, tail)?;
    Ok((set, tail.to_owned()))
}

/// Parses the `set:module` selector of a bazel dependency set into its module name.
pub fn parse_configured_selector(set_name: &str, selector: &str) -> Result<String, EditError> {
    if selector.is_empty() {
        return Err(EditError::Empty);
    }
    let Some((_, tail)) = selector.split_once(':') else {
        return Err(EditError::BareSet {
            set: set_name.to_owned(),
        });
    };
    let invalid = |reason: &'static str| EditError::InvalidPackage {
        set: UpdateSet::Bazel.name(),
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
    validate_package(UpdateSet::Bazel, tail)?;
    Ok(tail.to_owned())
}

/// Parses a configured npm or uv `set:package` selector into its package identity.
pub(crate) fn parse_foreign_selector(
    set_name: &str,
    kind: ConfiguredKind,
    selector: &str,
) -> Result<String, EditError> {
    let Some((_, tail)) = selector.split_once(':') else {
        return Err(EditError::BareSet {
            set: set_name.to_owned(),
        });
    };
    let set = kind.grammar();
    if tail.is_empty() {
        return Err(EditError::InvalidPackage {
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
            return Err(EditError::InvalidPackage {
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
#[path = "request_opam.rs"]
mod opam;
#[path = "request_plans.rs"]
pub(crate) mod plans;
#[path = "request_pyproject.rs"]
pub(crate) mod pyproject;

use cargo::*;
use opam::plan_opam;
use plans::*;
use pyproject::plan_pyproject_toml;

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
