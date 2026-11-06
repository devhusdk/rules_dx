use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::exception::{check_expiry, line_of, ExceptionProblem};
use crate::license_expr::Tier;
use dx_config::conventions::pattern_covers;

pub const LICENSE_POLICY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PolicyTables {
    pub blocked: BTreeSet<String>,
    pub allow: BTreeSet<String>,
    pub review: BTreeSet<String>,
    pub deny: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetAdjustment {
    pub set: String,
    pub review: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    #[serde(default)]
    pub internal: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PolicyProblem {
    #[error("license {identity:?} is listed in more than one policy list")]
    MultiListed { identity: String },
    #[error("license {identity:?} for set {set:?} conflicts with the global policy table")]
    SetConflict { set: String, identity: String },
    #[error("invalid licenses.toml: {message}")]
    InvalidLicensesToml { message: String },
    #[error("license exception invalid: {0}")]
    Exception(#[from] ExceptionProblem),
    #[error(
        "license exception for {license} on {package} matches no finding; remove it explicitly"
    )]
    ExceptionObsolete { license: String, package: String },
    #[error("license exception missing {field}")]
    ExceptionMissingField { field: &'static str },
    #[error("inventory text for package {package:?} is invalid: {message}")]
    InventoryText { package: String, message: String },
}

impl PolicyTables {
    pub fn validate(&self) -> Result<(), PolicyProblem> {
        let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
        for identity in self
            .blocked
            .iter()
            .chain(self.allow.iter())
            .chain(self.review.iter())
            .chain(self.deny.iter())
        {
            if seen.insert(identity.as_str(), ()).is_some() {
                return Err(PolicyProblem::MultiListed {
                    identity: identity.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn validate_set(&self, adjustment: &SetAdjustment) -> Result<(), PolicyProblem> {
        for identity in &adjustment.review {
            if self.blocked.contains(identity)
                || self.allow.contains(identity)
                || self.review.contains(identity)
                || self.deny.contains(identity)
            {
                return Err(PolicyProblem::SetConflict {
                    set: adjustment.set.clone(),
                    identity: identity.clone(),
                });
            }
        }
        Ok(())
    }
}

impl Distribution {
    pub fn tier_of(&self, label: &str) -> Tier {
        if self
            .internal
            .iter()
            .any(|entry| pattern_covers(entry, label))
        {
            Tier::Internal
        } else {
            Tier::Distributed
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenseException {
    pub package: String,
    pub set: String,
    pub license: String,
    pub versions: String,
    pub reason: String,
    pub expires: String,
    #[serde(skip)]
    pub line: usize,
}

impl LicenseException {
    pub fn entry_label(&self) -> String {
        use crate::exception::or_unknown;
        format!(
            "{} for {} in {}",
            or_unknown(&self.license),
            or_unknown(&self.package),
            or_unknown(&self.set)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseFinding {
    pub package: String,
    pub set: String,
    pub license: String,
    pub version: String,
}

pub fn validate_license_exception(
    exception: &LicenseException,
    today: &str,
) -> Result<(), PolicyProblem> {
    for (field, value) in [
        ("package", exception.package.as_str()),
        ("set", exception.set.as_str()),
        ("license", exception.license.as_str()),
        ("versions", exception.versions.as_str()),
        ("reason", exception.reason.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(PolicyProblem::ExceptionMissingField { field });
        }
    }
    check_expiry(&exception.expires, today)?;
    Ok(())
}

pub fn license_exception_covers(exception: &LicenseException, finding: &LicenseFinding) -> bool {
    if exception.set != finding.set
        || !crate::vuln::same_package(&finding.set, &exception.package, &finding.package)
        || exception.license != finding.license
    {
        return false;
    }
    crate::vuln::version_affected(&exception.set, &exception.versions, &finding.version)
}

pub fn is_obsolete(exception: &LicenseException, findings: &[LicenseFinding]) -> bool {
    !findings.iter().any(|finding| {
        crate::vuln::same_package(&finding.set, &exception.package, &finding.package)
            && finding.set == exception.set
            && finding.license == exception.license
    })
}

pub fn check_applies(
    exception: &LicenseException,
    findings: &[LicenseFinding],
) -> Result<(), PolicyProblem> {
    if is_obsolete(exception, findings) {
        return Err(PolicyProblem::ExceptionObsolete {
            license: exception.license.clone(),
            package: exception.package.clone(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenseInventory {
    pub package: String,
    pub set: String,
    pub license: String,
    pub versions: String,
    #[serde(default)]
    pub text_present: bool,
    /// Workspace-relative path of the vendored license words; implies
    /// `text_present`.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicensePolicy {
    pub tables: PolicyTables,
    pub sets: Vec<SetAdjustment>,
    pub distribution: Distribution,
    pub exceptions: Vec<LicenseException>,
    pub inventory: Vec<LicenseInventory>,
}

pub fn validate_license_inventory(entry: &LicenseInventory) -> Result<(), PolicyProblem> {
    if entry.package.trim().is_empty() {
        return Err(PolicyProblem::ExceptionMissingField {
            field: "inventory.package",
        });
    }
    if entry.set.trim().is_empty() {
        return Err(PolicyProblem::ExceptionMissingField {
            field: "inventory.set",
        });
    }
    if entry.license.trim().is_empty() {
        return Err(PolicyProblem::ExceptionMissingField {
            field: "inventory.license",
        });
    }
    if entry.versions.trim().is_empty() {
        return Err(PolicyProblem::ExceptionMissingField {
            field: "inventory.versions",
        });
    }
    if let Some(text) = &entry.text {
        crate::notice_manifest::notice_text_path_error(text).map_err(|message| {
            PolicyProblem::InventoryText {
                package: entry.package.clone(),
                message,
            }
        })?;
    }
    Ok(())
}

fn parse_file(text: &str) -> Result<LicensesFile, PolicyProblem> {
    let file: LicensesFile =
        toml::from_str(text).map_err(|error| PolicyProblem::InvalidLicensesToml {
            message: match error.span() {
                Some(span) => format!(
                    "invalid TOML at line {}: {}",
                    line_of(text, span.start),
                    error.message()
                ),
                None => error.to_string(),
            },
        })?;
    let version = file.schema_version.unwrap_or(LICENSE_POLICY_SCHEMA_VERSION);
    if version != LICENSE_POLICY_SCHEMA_VERSION {
        return Err(PolicyProblem::InvalidLicensesToml {
            message: format!(
                "unsupported licenses.toml schema_version {version} (want {LICENSE_POLICY_SCHEMA_VERSION})"
            ),
        });
    }
    Ok(file)
}

/// Parses one file. An `extends` line is accepted and not followed.
pub fn load_licenses_toml(text: &str) -> Result<LicensePolicy, PolicyProblem> {
    build_policy(text, parse_file(text)?, None)
}

/// The policy of the standard base, used when a workspace has no licenses.toml.
pub fn standard_policy() -> Result<LicensePolicy, PolicyProblem> {
    let text = crate::profile_base::base_text("licenses", crate::profile_base::Profile::Standard)
        .ok_or_else(|| PolicyProblem::InvalidLicensesToml {
        message: "the standard base is missing".to_owned(),
    })?;
    load_licenses_toml(text)
}

/// Parses `root_text` and merges it over the base named by its `extends`.
/// `read` returns the text of a workspace-relative path.
pub fn load_licenses(
    root_text: &str,
    read: impl Fn(&str) -> std::io::Result<String>,
) -> Result<LicensePolicy, PolicyProblem> {
    let root = parse_file(root_text)?;
    let Some(path) = root.extends.clone() else {
        return build_policy(root_text, root, None);
    };
    let invalid = |message: String| PolicyProblem::InvalidLicensesToml { message };
    let inside = !path.is_empty()
        && std::path::Path::new(&path)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        && !path.contains('\\');
    if !inside {
        return Err(invalid(format!(
            "extends {path}: must be a relative path inside the workspace"
        )));
    }
    let base_text = read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            invalid(format!("extends {path}: not found; run dx init --apply"))
        } else {
            invalid(format!("extends {path}: {error}"))
        }
    })?;
    let base =
        parse_file(&base_text).map_err(|error| invalid(format!("extends {path}: {error}")))?;
    if base.extends.is_some() || !base.exception.is_empty() || !base.inventory.is_empty() {
        return Err(invalid(format!(
            "extends {path}: a base may not have extends, [[exception]] or [[inventory]]"
        )));
    }
    build_policy(root_text, root, Some(base))
}

fn build_policy(
    text: &str,
    file: LicensesFile,
    base: Option<LicensesFile>,
) -> Result<LicensePolicy, PolicyProblem> {
    let base = base.unwrap_or_default();
    let list = |own: Option<Vec<String>>, inherited: Option<Vec<String>>| -> BTreeSet<String> {
        own.or(inherited).unwrap_or_default().into_iter().collect()
    };
    let mut sets = base.policy.sets;
    sets.extend(file.policy.sets);
    let policy = LicensePolicy {
        tables: PolicyTables {
            blocked: list(file.policy.blocked, base.policy.blocked),
            allow: list(file.policy.distributed.allow, base.policy.distributed.allow),
            review: list(
                file.policy.distributed.review,
                base.policy.distributed.review,
            ),
            deny: list(file.policy.distributed.deny, base.policy.distributed.deny),
        },
        sets: sets
            .into_iter()
            .map(|(set, adjustment)| SetAdjustment {
                set,
                review: adjustment.review.into_iter().collect(),
            })
            .collect(),
        distribution: file.distribution.or(base.distribution).unwrap_or_default(),
        exceptions: file
            .exception
            .into_iter()
            .map(|entry| {
                let line = line_of(text, entry.span().start);
                LicenseException {
                    line,
                    ..entry.into_inner()
                }
            })
            .collect(),
        inventory: file.inventory,
    };
    policy.tables.validate()?;
    for adjustment in &policy.sets {
        policy.tables.validate_set(adjustment)?;
    }
    for entry in &policy.inventory {
        validate_license_inventory(entry)?;
    }
    Ok(policy)
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LicensesFile {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    policy: PolicyFile,
    #[serde(default)]
    distribution: Option<Distribution>,
    #[serde(default)]
    #[schemars(with = "Vec<LicenseException>")]
    exception: Vec<toml::Spanned<LicenseException>>,
    #[serde(default)]
    inventory: Vec<LicenseInventory>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyFile {
    #[serde(default)]
    blocked: Option<Vec<String>>,
    #[serde(default)]
    distributed: PolicyLists,
    #[serde(default)]
    sets: BTreeMap<String, SetReview>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PolicyLists {
    #[serde(default)]
    allow: Option<Vec<String>>,
    #[serde(default)]
    review: Option<Vec<String>>,
    #[serde(default)]
    deny: Option<Vec<String>>,
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetReview {
    #[serde(default)]
    review: Vec<String>,
}

#[cfg(test)]
impl LicenseException {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            package,
            set,
            license,
            versions,
            reason,
            expires,
            line
        })
        .into_iter()
        .filter(|key| *key != "line")
        .collect()
    }
}

#[cfg(test)]
impl LicenseInventory {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            package,
            set,
            license,
            versions,
            text_present,
            text
        })
    }
}

#[cfg(test)]
impl Distribution {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self { internal })
    }
}

#[cfg(test)]
impl LicensesFile {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            schema_version,
            extends,
            policy,
            distribution,
            exception,
            inventory
        })
    }
}

#[cfg(test)]
impl PolicyFile {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            blocked,
            distributed,
            sets
        })
    }
}

#[cfg(test)]
impl PolicyLists {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            allow,
            review,
            deny
        })
    }
}

#[cfg(test)]
impl SetReview {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self { review })
    }
}

#[cfg(test)]
#[path = "license_policy_tests.rs"]
mod license_policy_tests;
