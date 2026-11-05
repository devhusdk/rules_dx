//! Declares a requirement `dx update` found missing.

use super::request::{DepTarget, EditError, UpdateRequest};
use super::sets::UpdateSet;
use super::version::WidenVersion;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddRequest {
    pub target: DepTarget,
    pub version: WidenVersion,
    pub selector: String,
    pub raw_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AddError {
    #[error("add does not support {selector:?}: {hint}")]
    UnsupportedSet {
        selector: String,
        hint: &'static str,
    },
    #[error("{selector} is already declared in {manifest}; use `dx update {selector} <version>` to change its version (nothing added)")]
    Declared { selector: String, manifest: String },
    #[error("unsupported manifest shape in {manifest}: {reason} (nothing added)")]
    UnsupportedManifest { manifest: String, reason: String },
    #[error(transparent)]
    Update(EditError),
}

impl From<EditError> for AddError {
    fn from(error: EditError) -> AddError {
        match error {
            EditError::UnsupportedManifest { manifest, reason } => {
                AddError::UnsupportedManifest { manifest, reason }
            }
            other => AddError::Update(other),
        }
    }
}

const GITHUB_ACTIONS_HINT: &str =
    "add the `uses:` step to the workflow, then pin it with `dx update github-actions:<owner/repo> <tag-or-sha>`";

impl AddRequest {
    /// The insert request for the package an update request names.
    pub fn from_request(request: &UpdateRequest) -> AddRequest {
        AddRequest {
            target: request.target(),
            version: request.version.clone(),
            selector: request.selector.clone(),
            raw_version: request.raw_version.clone(),
        }
    }

    pub fn set(&self) -> UpdateSet {
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

    /// The `dx update` request for the same package and version.
    pub fn as_request(&self) -> UpdateRequest {
        UpdateRequest::for_target(
            &self.target,
            self.version.clone(),
            &self.selector,
            &self.raw_version,
        )
    }

    /// The manifest with the new requirement declared, in the shape `dx update` widens.
    pub fn plan_insert(&self, content: &str) -> Result<String, AddError> {
        match self.as_request().plan_edit(content) {
            Ok(_) | Err(EditError::Ambiguous { .. }) => {
                return Err(AddError::Declared {
                    selector: self.selector.clone(),
                    manifest: self.target_manifest().to_owned(),
                });
            }
            Err(EditError::NotFound { .. }) => {}
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
            UpdateSet::Bazel => dx_module_bazel::ModuleFile::parse(content)
                .and_then(|module| module.insert_bazel_dep(package, &version))
                .map_err(|error| unsupported(error.to_string())),
            UpdateSet::Maven => dx_module_bazel::ModuleFile::parse(content)
                .and_then(|module| {
                    let coordinate = format!("{package}:{version}");
                    match self
                        .target
                        .configured
                        .as_ref()
                        .and_then(|c| c.repository.as_deref())
                    {
                        Some(repository) => {
                            module.insert_maven_artifact_in(repository, &coordinate)
                        }
                        None => module.insert_maven_artifact(&coordinate),
                    }
                })
                .map_err(|error| unsupported(error.to_string())),
            UpdateSet::Go => dx_gomod::insert_require(content, package, &format!("v{version}"))
                .map_err(|error| match error {
                    dx_gomod::GoEditError::Invalid(reason) => unsupported(reason),
                    other => unsupported(format!("{other:?}")),
                }),
            UpdateSet::Cargo => {
                plans::insert_cargo_dependency(content, package, &version).map_err(unsupported)
            }
            UpdateSet::Npm => {
                plans::insert_npm_dependency(content, package, &version).map_err(unsupported)
            }
            UpdateSet::NuGet => {
                plans::insert_paket_dependency(content, package, &version).map_err(unsupported)
            }
            UpdateSet::GithubActions => Err(AddError::UnsupportedSet {
                selector: self.selector.clone(),
                hint: GITHUB_ACTIONS_HINT,
            }),
            UpdateSet::Containers => Err(AddError::UnsupportedSet {
                selector: self.selector.clone(),
                hint: "add an oci.pull to MODULE.bazel",
            }),
            UpdateSet::Ruby => Err(AddError::UnsupportedSet {
                selector: self.selector.clone(),
                hint: "edit the Gemfile and run `bundle lock`",
            }),
            UpdateSet::Cpan => Err(AddError::UnsupportedSet {
                selector: self.selector.clone(),
                hint: "edit the cpanfile and run `carton install`",
            }),
        }
    }
}

#[path = "add_plans.rs"]
mod plans;

#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
