use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::exception::{ExceptionProblem, check_expiry, line_of, toml_message};
use crate::license_expr::Tier;
use crate::profile_base::{Profile, base_text};
use dx_config::conventions::pattern_covers;

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
    let invalid = |message: String| PolicyProblem::InvalidLicensesToml { message };
    let table: toml::Table = text.parse().map_err(|error: toml::de::Error| {
        invalid(toml_message(text, &error, error.message().to_owned()))
    })?;
    dx_config::conventions::check_schema_version(table.get("schema_version"))
        .map_err(|problem| invalid(problem.to_string()))?;
    toml::from_str(text).map_err(|error: toml::de::Error| {
        let message = dx_config::conventions::explain_unknown_key(error.message());
        invalid(toml_message(text, &error, message))
    })
}

/// The policy for `profile`: its embedded base with `text`, the workspace's
/// licenses.toml, merged over it. Missing or blank text means the base alone.
pub fn load_licenses(text: Option<&str>, profile: Profile) -> Result<LicensePolicy, PolicyProblem> {
    let base_text =
        base_text("licenses", profile).ok_or_else(|| PolicyProblem::InvalidLicensesToml {
            message: format!("the {} base is missing", profile.as_str()),
        })?;
    let base = parse_file(base_text)?;
    match text.filter(|text| !text.trim().is_empty()) {
        Some(text) => build_policy(text, parse_file(text)?, base),
        None => build_policy("", LicensesFile::default(), base),
    }
}

fn build_policy(
    text: &str,
    file: LicensesFile,
    base: LicensesFile,
) -> Result<LicensePolicy, PolicyProblem> {
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
    if let Some(bad) = policy
        .distribution
        .internal
        .iter()
        .find(|entry| !dx_config::conventions::is_valid_pattern(entry))
    {
        let problem = dx_config::conventions::ConventionProblem::BadPattern { value: bad.clone() };
        return Err(PolicyProblem::InvalidLicensesToml {
            message: format!("[distribution] internal: {problem}"),
        });
    }
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
    // Checked by `parse_file` before the rest of the file.
    #[allow(dead_code)]
    schema_version: u32,
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
