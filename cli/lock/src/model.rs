//! The lock model: its keys, its checks and its bytes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// The `schema` value this writer writes.
pub const SCHEMA: u32 = 1;

/// The values of an archive `platform`.
pub const PLATFORMS: [&str; 6] = [
    "linux_arm64",
    "linux_x86_64",
    "macos_arm64",
    "macos_x86_64",
    "windows_arm64",
    "windows_x86_64",
];

/// One `<ecosystem>.lock.json`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    /// The schema version.
    pub schema: u32,
    /// The ecosystem name.
    pub ecosystem: String,
    /// The sha256 of the manifest and state files, by file name.
    pub inputs: BTreeMap<String, String>,
    /// Every package of the closure, sorted by name.
    pub packages: Vec<Package>,
}

/// One package at one exact version.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    /// The name as the ecosystem writes it.
    pub name: String,
    /// The exact version.
    pub version: String,
    /// The archives that hold the package.
    pub archives: Vec<Archive>,
    /// The names of the packages this one needs.
    #[serde(default)]
    pub deps: Vec<String>,
    /// The import names of the package.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Whether only development needs the package.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dev: bool,
    /// Other ecosystem data.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

/// One archive of a package.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    /// Where the archive is, best first.
    pub urls: Vec<String>,
    /// The archive digest; a resolver leaves it empty when its registry states none.
    #[serde(default)]
    pub sha256: String,
    /// The directory to strip on extraction.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub strip_prefix: String,
    /// The platform the archive is for; empty means every platform.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub platform: String,
    /// The archive type when the URL does not end in its suffix.
    #[serde(default, rename = "type", skip_serializing_if = "String::is_empty")]
    pub kind: String,
}

/// What a resolver returns: the closure, with a digest only where its registry states one.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    /// Every package of the closure, in any order.
    pub packages: Vec<Package>,
}

impl Resolution {
    /// Reads a resolution a resolver printed as JSON.
    ///
    /// # Errors
    /// Fails when the bytes are not a resolution.
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

/// Whether `text` is 64 lowercase hex characters.
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

impl Lock {
    /// Reads a lock file.
    ///
    /// # Errors
    /// Fails when the text is not a lock that passes [`Lock::problems`].
    pub fn from_json(text: &str) -> Result<Self, Error> {
        let lock: Self = serde_json::from_str(text).map_err(|error| Error::new(error.to_string()))?;
        match lock.problems().as_slice() {
            [] => Ok(lock),
            problems => Err(Error::new(problems.join("\n"))),
        }
    }

    /// Puts every list in its fixed order.
    pub fn normalize(&mut self) {
        for package in &mut self.packages {
            sort_dedup(&mut package.deps);
            sort_dedup(&mut package.provides);
            package
                .archives
                .sort_by(|a, b| (&a.platform, &a.urls).cmp(&(&b.platform, &b.urls)));
        }
        self.packages
            .sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    }

    /// What breaks the schema, one line each; archive digests may still be empty.
    #[must_use]
    pub fn shape_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let names: BTreeSet<&str> = self.packages.iter().map(|p| p.name.as_str()).collect();
        let mut seen = BTreeSet::new();
        for package in &self.packages {
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
                    problems.push(format!("package {name} has the unknown platform {platform}"));
                }
                if !platforms.insert(platform) {
                    problems.push(format!(
                        "package {name} has two archives for {}",
                        if platform.is_empty() { "every platform" } else { platform }
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

    /// What keeps a hub from building this lock, one line each.
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
        for package in &self.packages {
            for archive in &package.archives {
                if archive.sha256.is_empty() {
                    problems.push(format!("package {} has an archive with no sha256", package.name));
                }
            }
        }
        problems
    }

    /// The bytes of the lock file: sorted keys, two spaces, one final newline.
    #[must_use]
    pub fn to_bytes(&self) -> String {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        let mut text = serde_json::to_string_pretty(&sorted(value)).unwrap_or_default();
        text.push('\n');
        text
    }
}

/// `value` with every object rebuilt in key order.
fn sorted(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<String, Value> =
                map.into_iter().map(|(key, value)| (key, sorted(value))).collect();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}
