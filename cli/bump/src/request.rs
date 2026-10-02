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
}

impl BumpRequest {
    pub fn parse(selector: &str, version: &str) -> Result<BumpRequest, BumpError> {
        if selector.is_empty() || version.is_empty() {
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
        let parsed = version::parse(set, version)?;
        Ok(BumpRequest {
            set,
            package: tail.to_owned(),
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
        })
    }

    pub fn needs_update_refresh(&self) -> bool {
        self.set.needs_update_refresh()
    }

    pub fn refresh_selector(&self) -> String {
        match self.set {
            BumpSet::Cargo => "cargo".to_owned(),
            BumpSet::Npm => format!("npm:{}", self.package),
            BumpSet::Go => "go".to_owned(),
            BumpSet::Maven => "maven".to_owned(),
            BumpSet::NuGet => "nuget".to_owned(),
            BumpSet::Bazel | BumpSet::GithubActions => String::new(),
        }
    }

    pub fn target_manifest(&self) -> &'static str {
        match self.set {
            BumpSet::Bazel => {
                if self.package == ".bazelversion" {
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

    pub fn summary(&self) -> String {
        let through = if self.needs_update_refresh() {
            format!(
                "then refresh via `dx update {}` automatically",
                self.refresh_selector()
            )
        } else {
            "then preset flag-diff review plus `bazel build //...`".to_owned()
        };
        let base = format!(
            "Widen {} to {} in {} ({through})",
            self.selector,
            self.version.display(),
            self.target_manifest(),
        );
        if self.version.is_semver() {
            format!("{base}; {}", version::generic_major_bump_hint())
        } else {
            base
        }
    }

    pub fn plan_edit(&self, content: &str) -> Result<String, BumpError> {
        match self.set {
            BumpSet::Bazel => {
                if self.package == ".bazelversion" {
                    plan_bazelversion(content, &self.version)
                } else {
                    plan_module_bazel(content, &self.package, &self.version)
                }
            }
            BumpSet::Cargo => plan_cargo_toml(content, &self.package, &self.version),
            BumpSet::Npm => plan_package_json(content, &self.package, &self.version),
            BumpSet::Go => plan_go_mod(content, &self.package, &self.version),
            BumpSet::Maven => plan_maven_module_bazel(content, &self.package, &self.version),
            BumpSet::NuGet => plan_paket_dependencies(content, &self.package, &self.version),
            BumpSet::GithubActions => plan_github_workflow(content, &self.package, &self.version),
        }
    }
}

#[path = "request_cargo.rs"]
mod cargo;
#[path = "request_plans.rs"]
mod plans;

use cargo::*;
use plans::*;

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
