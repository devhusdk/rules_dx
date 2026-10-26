//! `dx add`: declare one new requirement in the manifest `dx bump` widens.

use super::request::{
    parse_configured_selector, parse_selector, BumpError, BumpRequest, DepTarget,
};
use super::sets::BumpSet;
use super::version::{self, WidenVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddRequest {
    pub target: DepTarget,
    pub version: WidenVersion,
    pub selector: String,
    pub raw_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AddError {
    #[error("empty add selector or version; expected `dx add <set:package> <version>`")]
    Empty,
    #[error("unknown add selector {selector:?}; expected {} as `set:package` (e.g. cargo:anyhow, maven:junit:junit)", AddRequest::pipe_list())]
    UnknownSelector { selector: String },
    #[error("add needs one package, not a whole set: {set:?} selects the set; use `{set}:<package> <version>`")]
    BareSet { set: String },
    #[error("add needs `set:package`, not a label or path: {target:?}")]
    NotAPackage { target: String },
    #[error("add does not support {selector:?}: {hint}")]
    UnsupportedSet {
        selector: String,
        hint: &'static str,
    },
    #[error("{selector} is already declared in {manifest}; use `dx bump {selector} <version>` to change its version (nothing added)")]
    Declared { selector: String, manifest: String },
    #[error("unsupported manifest shape in {manifest}: {reason} (nothing added)")]
    UnsupportedManifest { manifest: String, reason: String },
    #[error(transparent)]
    Bump(BumpError),
}

impl From<BumpError> for AddError {
    fn from(error: BumpError) -> AddError {
        match error {
            BumpError::Empty => AddError::Empty,
            BumpError::UnknownSelector { selector } => AddError::UnknownSelector { selector },
            BumpError::BareSet { set } => AddError::BareSet { set },
            BumpError::NotAPackage { target } => AddError::NotAPackage { target },
            BumpError::UnsupportedManifest { manifest, reason } => {
                AddError::UnsupportedManifest { manifest, reason }
            }
            other => AddError::Bump(other),
        }
    }
}

const GITHUB_ACTIONS_HINT: &str =
    "add the `uses:` step to the workflow, then pin it with `dx bump github-actions:<owner/repo> <tag-or-sha>`";
const BAZELVERSION_HINT: &str =
    ".bazelversion always pins one Bazel version; change it with `dx bump bazel:.bazelversion <version>`";

impl AddRequest {
    /// The sets `dx add` declares packages in, in help and docs order.
    pub const SETS: [BumpSet; 6] = [
        BumpSet::Bazel,
        BumpSet::Cargo,
        BumpSet::Go,
        BumpSet::Maven,
        BumpSet::Npm,
        BumpSet::NuGet,
    ];

    pub fn names() -> Vec<&'static str> {
        Self::SETS.iter().map(|set| set.name()).collect()
    }

    pub fn pipe_list() -> String {
        Self::names().join("|")
    }

    pub fn parse(selector: &str, version: &str) -> Result<AddRequest, AddError> {
        if selector.is_empty() || version.is_empty() {
            return Err(AddError::Empty);
        }
        if selector.split_once(':').map(|(head, _)| head) == Some(BumpSet::GithubActions.name())
            || selector == BumpSet::GithubActions.name()
        {
            return Err(AddError::UnsupportedSet {
                selector: selector.to_owned(),
                hint: GITHUB_ACTIONS_HINT,
            });
        }
        let (set, package) = parse_selector(selector)?;
        if set == BumpSet::Bazel && package == ".bazelversion" {
            return Err(AddError::UnsupportedSet {
                selector: selector.to_owned(),
                hint: BAZELVERSION_HINT,
            });
        }
        let parsed = version::parse(set, version).map_err(BumpError::from)?;
        Ok(AddRequest {
            target: DepTarget {
                set,
                package,
                manifest: None,
            },
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
        })
    }

    /// Parses a request against a configured dx.toml bazel set that owns `manifest`.
    pub fn parse_configured(
        set_name: &str,
        manifest: &str,
        selector: &str,
        version: &str,
    ) -> Result<AddRequest, AddError> {
        if selector.is_empty() || version.is_empty() {
            return Err(AddError::Empty);
        }
        let package = parse_configured_selector(set_name, selector)?;
        let parsed = version::parse(BumpSet::Bazel, version).map_err(BumpError::from)?;
        Ok(AddRequest {
            target: DepTarget {
                set: BumpSet::Bazel,
                package,
                manifest: Some(manifest.to_owned()),
            },
            version: parsed,
            selector: selector.to_owned(),
            raw_version: version.to_owned(),
        })
    }

    pub fn set(&self) -> BumpSet {
        self.target.set
    }

    pub fn package(&self) -> &str {
        &self.target.package
    }

    pub fn needs_update_refresh(&self) -> bool {
        self.target.needs_update_refresh()
    }

    pub fn refresh_selector(&self) -> String {
        self.target.refresh_selector()
    }

    pub fn target_manifest(&self) -> &str {
        self.target.target_manifest()
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
        format!(
            "Add {} at {} to {} ({through})",
            self.selector,
            self.version.display(),
            self.target_manifest(),
        )
    }

    /// The `dx bump` request for the same package and version.
    pub fn as_bump(&self) -> BumpRequest {
        BumpRequest::for_target(
            &self.target,
            self.version.clone(),
            &self.selector,
            &self.raw_version,
        )
    }

    /// The manifest with the new requirement declared, in the shape `dx bump` widens.
    pub fn plan_insert(&self, content: &str) -> Result<String, AddError> {
        match self.as_bump().plan_edit(content) {
            Ok(_) | Err(BumpError::Ambiguous { .. }) => {
                return Err(AddError::Declared {
                    selector: self.selector.clone(),
                    manifest: self.target_manifest().to_owned(),
                });
            }
            Err(BumpError::NotFound { .. }) => {}
            Err(other) => return Err(other.into()),
        }
        let version = match &self.version {
            WidenVersion::Semver(version) => version.to_string(),
            _ => {
                return Err(AddError::UnsupportedManifest {
                    manifest: self.target_manifest().to_owned(),
                    reason: "expected exact semver".to_owned(),
                })
            }
        };
        let manifest = self.target_manifest();
        let unsupported = |reason: String| AddError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason,
        };
        let package = self.package();
        match self.set() {
            BumpSet::Bazel => dx_module_bazel::ModuleFile::parse(content)
                .and_then(|module| module.insert_bazel_dep(package, &version))
                .map_err(|error| unsupported(error.to_string())),
            BumpSet::Maven => dx_module_bazel::ModuleFile::parse(content)
                .and_then(|module| module.insert_maven_artifact(&format!("{package}:{version}")))
                .map_err(|error| unsupported(error.to_string())),
            BumpSet::Go => dx_gomod::insert_require(content, package, &format!("v{version}"))
                .map_err(|error| match error {
                    dx_gomod::GoEditError::Invalid(reason) => unsupported(reason),
                    other => unsupported(format!("{other:?}")),
                }),
            BumpSet::Cargo => {
                plans::insert_cargo_dependency(content, package, &version).map_err(unsupported)
            }
            BumpSet::Npm => {
                plans::insert_npm_dependency(content, package, &version).map_err(unsupported)
            }
            BumpSet::NuGet => {
                plans::insert_paket_dependency(content, package, &version).map_err(unsupported)
            }
            BumpSet::GithubActions => Err(AddError::UnsupportedSet {
                selector: self.selector.clone(),
                hint: GITHUB_ACTIONS_HINT,
            }),
        }
    }
}

#[path = "add_plans.rs"]
mod plans;

#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
