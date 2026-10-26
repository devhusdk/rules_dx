use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    pub set: String,
    pub is_git: bool,
    pub is_private: bool,
    pub location: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
}

impl LockedPackage {
    /// Builds a registry package: not git, not private, no location or platform.
    pub fn new(
        set: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            set: set.into(),
            is_git: false,
            is_private: false,
            location: None,
            platform: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    pub versions: String,
    pub severity: String,
    pub fixed: Vec<String>,
    pub set: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VulnFinding {
    pub advisory: String,
    pub package: String,
    pub version: String,
    pub set: String,
    pub severity: String,
    pub level: &'static str,
    pub fixed: Vec<String>,
}

pub const REASON_GIT: &str = "unsupported git revision";
pub const REASON_PRIVATE: &str = "unidentified private package";
pub const REASON_VERSION_SYNTAX: &str = "unsupported version syntax";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Unassessed {
    pub package: String,
    pub set: String,
    pub reason: &'static str,
}

pub const UNKNOWN_SEVERITY: &str = "unknown";

pub const VULN_RULE_PREFIX: &str = "vuln";

pub fn normalize_level(severity: &str) -> &'static str {
    match severity.trim().to_ascii_lowercase().as_str() {
        "critical" | "high" | "error" => "error",
        "medium" | "low" | "moderate" | "warning" => "warning",
        "" | "unknown" => "error",
        _ => "error",
    }
}

pub fn canonical_severity(severity: &str) -> String {
    let trimmed = severity.trim();
    if trimmed.is_empty() {
        UNKNOWN_SEVERITY.to_owned()
    } else {
        trimmed.to_owned()
    }
}
