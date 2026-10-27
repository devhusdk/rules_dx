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
    /// Which of a MODULE.bazel dep's two versions a bazel bump edits.
    pub dep_version: DepVersion,
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
}

impl DepTarget {
    pub fn needs_update_refresh(&self) -> bool {
        self.set.needs_update_refresh()
    }

    pub fn refresh_selector(&self) -> String {
        refresh_selector(self.set, &self.package)
    }

    pub fn target_manifest(&self) -> &str {
        target_manifest(self.set, &self.package, self.manifest.as_deref())
    }
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
            dep_version: DepVersion::Tested,
        })
    }

    pub fn parse_configured(
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<BumpRequest, BumpError> {
        if selector.is_empty() || version.is_empty() {
            return Err(BumpError::Empty);
        }
        let package = parse_configured_selector(set_name, selector)?;
        let parsed = version::parse(BumpSet::Bazel, version)?;
        Ok(BumpRequest {
            set: BumpSet::Bazel,
            package,
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
            manifest: Some(manifest.to_owned()),
            dep_version: DepVersion::Tested,
        })
    }

    pub fn needs_update_refresh(&self) -> bool {
        self.set.needs_update_refresh()
    }

    pub fn refresh_selector(&self) -> String {
        refresh_selector(self.set, &self.package)
    }

    pub fn target_manifest(&self) -> &str {
        target_manifest(self.set, &self.package, self.manifest.as_deref())
    }

    /// The set, package and manifest this request edits.
    pub fn target(&self) -> DepTarget {
        DepTarget {
            set: self.set,
            package: self.package.clone(),
            manifest: self.manifest.clone(),
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
            dep_version: DepVersion::Tested,
        }
    }

    /// This request aimed at `dep_version`; a floor applies only to a MODULE.bazel bazel_dep.
    pub fn with_dep_version(mut self, dep_version: DepVersion) -> Result<BumpRequest, BumpError> {
        if dep_version != DepVersion::Tested
            && (self.set != BumpSet::Bazel || self.package == ".bazelversion")
        {
            return Err(BumpError::FloorNotBazelDep {
                selector: self.selector,
            });
        }
        self.dep_version = dep_version;
        Ok(self)
    }

    pub fn summary(&self) -> String {
        let through = if self.needs_update_refresh() {
            format!(
                "then refresh via `dx update {}` automatically",
                self.refresh_selector()
            )
        } else {
            "then preset flag-diff review plus `bazel build //...`".to_owned()
        };
        let place = if self.set == BumpSet::Cargo && self.manifest.is_none() {
            "every manifest of the crate.from_cargo hub in MODULE.bazel"
        } else {
            self.target_manifest()
        };
        let what = match (&self.dep_version, self.set, self.package.as_str()) {
            (DepVersion::Floor { .. }, _, _) => " floor",
            (DepVersion::Tested, BumpSet::Bazel, package) if package != ".bazelversion" => {
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
            BumpSet::Go => plan_go_mod(content, &self.package, &self.version),
            BumpSet::Maven => plan_maven_module_bazel(content, &self.package, &self.version),
            BumpSet::NuGet => plan_paket_dependencies(content, &self.package, &self.version),
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

#[path = "request_cargo.rs"]
mod cargo;
#[path = "request_plans.rs"]
pub(crate) mod plans;

pub use cargo::cargo_manifests;
use cargo::*;
use plans::*;

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
