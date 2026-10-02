use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::exception::{check_expiry, ExceptionProblem};
use crate::license_expr::Tier;

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

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize)]
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
    #[error("license exception missing {field}")]
    ExceptionMissingField { field: &'static str },
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
        if self.internal.contains(label) {
            Tier::Internal
        } else {
            Tier::Distributed
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseException {
    pub package: String,
    pub set: String,
    pub license: String,
    pub versions: String,
    pub reason: String,
    pub expires: String,
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
    if exception.package != finding.package
        || exception.set != finding.set
        || exception.license != finding.license
    {
        return false;
    }
    crate::vuln::version_affected(&exception.set, &exception.versions, &finding.version)
}

pub fn is_obsolete(exception: &LicenseException, findings: &[LicenseFinding]) -> bool {
    !findings.iter().any(|finding| {
        finding.package == exception.package
            && finding.set == exception.set
            && finding.license == exception.license
    })
}

pub fn check_applies(
    exception: &LicenseException,
    findings: &[LicenseFinding],
) -> Result<(), PolicyProblem> {
    if is_obsolete(exception, findings) {
        return Err(PolicyProblem::Exception(ExceptionProblem::Obsolete {
            advisory: exception.license.clone(),
            package: exception.package.clone(),
        }));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseInventory {
    pub package: String,
    pub set: String,
    pub license: String,
    pub versions: String,
    #[serde(default)]
    pub text_present: bool,
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
    Ok(())
}

pub fn load_licenses_toml(text: &str) -> Result<LicensePolicy, PolicyProblem> {
    let file: LicensesFile =
        toml::from_str(text).map_err(|error| PolicyProblem::InvalidLicensesToml {
            message: error.to_string(),
        })?;
    let version = file.schema_version.unwrap_or(LICENSE_POLICY_SCHEMA_VERSION);
    if version != LICENSE_POLICY_SCHEMA_VERSION {
        return Err(PolicyProblem::InvalidLicensesToml {
            message: format!(
                "unsupported licenses.toml schema_version {version} (want {LICENSE_POLICY_SCHEMA_VERSION})"
            ),
        });
    }
    let policy = LicensePolicy {
        tables: PolicyTables {
            blocked: file.policy.blocked.into_iter().collect(),
            allow: file.policy.distributed.allow.into_iter().collect(),
            review: file.policy.distributed.review.into_iter().collect(),
            deny: file.policy.distributed.deny.into_iter().collect(),
        },
        sets: file
            .policy
            .sets
            .into_iter()
            .map(|(set, adjustment)| SetAdjustment {
                set,
                review: adjustment.review.into_iter().collect(),
            })
            .collect(),
        distribution: file.distribution,
        exceptions: file.exception,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LicensesFile {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    policy: PolicyFile,
    #[serde(default)]
    distribution: Distribution,
    #[serde(default)]
    exception: Vec<LicenseException>,
    #[serde(default)]
    inventory: Vec<LicenseInventory>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyFile {
    #[serde(default)]
    blocked: Vec<String>,
    #[serde(default)]
    distributed: PolicyLists,
    #[serde(default)]
    sets: BTreeMap<String, SetReview>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyLists {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    review: Vec<String>,
    #[serde(default)]
    deny: Vec<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SetReview {
    #[serde(default)]
    review: Vec<String>,
}

#[cfg(test)]
#[path = "license_policy_tests.rs"]
mod license_policy_tests;
