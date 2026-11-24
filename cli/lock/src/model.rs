use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

pub const SCHEMA: u32 = 1;

pub const PLATFORMS: [&str; 6] = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "macos_x86_64",
    "windows_arm64",
    "windows_x86_64",
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub schema: u32,
    pub ecosystem: String,
    pub inputs: BTreeMap<String, String>,
    pub packages: Vec<Package>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub groups: BTreeMap<String, Vec<Package>>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub archives: Vec<Archive>,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub provides: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub dev: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub urls: Vec<String>,
    #[serde(default)]
    pub sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub strip_prefix: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub platform: String,
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub packages: Vec<Package>,
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<Package>>,
}

impl Resolution {
    pub fn from_json(bytes: &[u8]) -> Result<Self, Error> {
        serde_json::from_slice(bytes).map_err(|error| Error::new(format!("resolution: {error}")))
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}

#[must_use]
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn name_problem(name: &str) -> Option<&'static str> {
    let segment_ok = |segment: &str| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '@' | '_' | '+' | '.' | '-'))
    };
    (!name.split('/').all(segment_ok))
        .then_some("has a name outside letters, digits, `@`, `_`, `+`, `.`, `-` and `/`")
}

fn sort_dedup(items: &mut Vec<String>) {
    items.sort();
    items.dedup();
}

fn normalize(packages: &mut [Package]) {
    for package in packages.iter_mut() {
        sort_dedup(&mut package.deps);
        sort_dedup(&mut package.provides);
        package
            .archives
            .sort_by(|a, b| (&a.platform, &a.urls).cmp(&(&b.platform, &b.urls)));
    }
    packages.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
}

fn group_problem(group: &str) -> bool {
    group.is_empty()
        || !group
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn closure_problems(packages: &[Package]) -> Vec<String> {
    let mut problems = Vec::new();
    let names: BTreeSet<&str> = packages.iter().map(|p| p.name.as_str()).collect();
    let mut seen = BTreeSet::new();
    for package in packages {
        let name = &package.name;
        if let Some(problem) = name_problem(name) {
            problems.push(format!("package {name:?} {problem}"));
        }
        if !seen.insert(name.as_str()) {
            problems.push(format!("package {name} is listed twice"));
        }
        if package.version.is_empty() {
            problems.push(format!("package {name} has no version"));
        }
        if package.archives.is_empty() {
            problems.push(format!("package {name} has no archive"));
        }
        let mut platforms = BTreeSet::new();
        for archive in &package.archives {
            let platform = archive.platform.as_str();
            if !platform.is_empty() && !PLATFORMS.contains(&platform) {
                problems.push(format!(
                    "package {name} has the unknown platform {platform}"
                ));
            }
            if !platforms.insert(platform) {
                problems.push(format!(
                    "package {name} has two archives for {}",
                    if platform.is_empty() {
                        "every platform"
                    } else {
                        platform
                    }
                ));
            }
            if archive.urls.is_empty() {
                problems.push(format!("package {name} has an archive with no URL"));
            }
            for url in &archive.urls {
                if !url.starts_with("https://") || url.chars().any(char::is_whitespace) {
                    problems.push(format!("package {name} has the URL {url:?}; want https"));
                }
            }
            if !archive.sha256.is_empty() && !is_sha256(&archive.sha256) {
                problems.push(format!(
                    "package {name} has the sha256 {:?}; want 64 hex characters",
                    archive.sha256
                ));
            }
        }
        for dep in &package.deps {
            if !names.contains(dep.as_str()) {
                problems.push(format!("package {name} needs {dep}, which is not locked"));
            }
        }
        if package.provides.iter().any(String::is_empty) {
            problems.push(format!("package {name} provides an empty import name"));
        }
    }
    problems
}

impl Lock {
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let lock: Self =
            serde_json::from_str(text).map_err(|error| Error::new(error.to_string()))?;
        match lock.problems().as_slice() {
            [] => Ok(lock),
            problems => Err(Error::new(problems.join("\n"))),
        }
    }

    pub fn normalize(&mut self) {
        normalize(&mut self.packages);
        self.groups
            .values_mut()
            .for_each(|packages| normalize(packages));
    }

    #[must_use]
    pub fn shape_problems(&self) -> Vec<String> {
        let mut problems = closure_problems(&self.packages);
        for (group, packages) in &self.groups {
            if group_problem(group) {
                problems.push(format!(
                    "group {group:?} has a name outside letters, digits, `_`, `.` and `-`"
                ));
            }
            if !self.packages.is_empty() {
                problems.push(format!("group {group} sits next to top-level packages"));
            }
            problems.extend(
                closure_problems(packages)
                    .into_iter()
                    .map(|problem| format!("group {group}: {problem}")),
            );
        }
        problems
    }

    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema != SCHEMA {
            problems.push(format!("schema is {}; want {SCHEMA}", self.schema));
        }
        if self.inputs.is_empty() {
            problems.push("inputs is empty".to_owned());
        }
        for (file, digest) in &self.inputs {
            if !is_sha256(digest) {
                problems.push(format!("input {file} has no sha256"));
            }
        }
        problems.extend(self.shape_problems());
        for package in self.packages.iter().chain(self.groups.values().flatten()) {
            for archive in &package.archives {
                if archive.sha256.is_empty() {
                    problems.push(format!(
                        "package {} has an archive with no sha256",
                        package.name
                    ));
                }
            }
        }
        problems
    }

    #[must_use]
    pub fn to_bytes(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        let mut text = serde_json::to_string_pretty(&sorted(value)).unwrap_or_default();
        text.push('\n');
        text
    }
}

fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) => other,
    }
}
