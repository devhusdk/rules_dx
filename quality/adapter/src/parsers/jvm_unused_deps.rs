//! Finds the `deps` of a Java or Kotlin rule whose jars the compiler never read.
//!
//! The aspect writes the direct dependencies with their compile jars next to the compiler's jdeps
//! file. A dependency is unused when none of its jars is named as used in the jdeps file.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::rust_unused_deps::UnusedDependency;
use super::ParseError;

pub const RULE_ID: &str = "unused_deps";

#[derive(Debug, Deserialize)]
struct DepsMap {
    package: String,
    target: String,
    deps: Vec<Dep>,
    #[serde(default)]
    indirect: Vec<Indirect>,
}

#[derive(Debug, Deserialize)]
struct Indirect {
    jar: String,
    label: String,
}

#[derive(Debug, Deserialize)]
struct Dep {
    label: String,
    jars: Vec<String>,
}

/// Names every dependency in `map` that none of the used jars of `jdeps` belongs to.
pub fn parse_unused_dependencies(
    tool: &'static str,
    map: &[u8],
    jdeps: &[u8],
) -> Result<Vec<UnusedDependency>, ParseError> {
    let map: DepsMap = serde_json::from_slice(map).map_err(|err| ParseError::Json {
        tool,
        detail: err.to_string(),
    })?;
    let used = used_jars(jdeps).map_err(|detail| ParseError::Shape { tool, detail })?;
    Ok(map
        .deps
        .iter()
        .filter(|dep| !dep.jars.is_empty() && !dep.jars.iter().any(|jar| used.contains(jar)))
        .map(|dep| {
            let name = dep.label.rsplit(':').next().unwrap_or(&dep.label);
            UnusedDependency {
                package: map.package.clone(),
                target: map.target.clone(),
                dependency: name.to_owned(),
                crate_name: dep.label.clone(),
            }
        })
        .collect())
}

/// A dependency the compiler used that the rule does not list in `deps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingDependency {
    pub package: String,
    pub target: String,
    pub label: String,
}

impl MissingDependency {
    pub fn message(&self) -> String {
        format!(
            "`{}` is used by `{}` but is not in its deps",
            self.label, self.target
        )
    }
}

/// Names every transitive dependency in `map` whose jar the compiler used, once per label.
pub fn parse_missing_dependencies(
    tool: &'static str,
    map: &[u8],
    jdeps: &[u8],
) -> Result<Vec<MissingDependency>, ParseError> {
    let map: DepsMap = serde_json::from_slice(map).map_err(|err| ParseError::Json {
        tool,
        detail: err.to_string(),
    })?;
    let used = used_jars(jdeps).map_err(|detail| ParseError::Shape { tool, detail })?;
    let mut labels = BTreeSet::new();
    for item in map.indirect.iter().filter(|item| used.contains(&item.jar)) {
        labels.insert(item.label.clone());
    }
    Ok(labels
        .into_iter()
        .map(|label| MissingDependency {
            package: map.package.clone(),
            target: map.target.clone(),
            label,
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct ScalacLog {
    diagnostics: Vec<ScalacDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct ScalacDiagnostic {
    code: String,
    message: String,
}

/// Code the scalac driver gives the dependency analyzer's unused-dependency warnings.
const SCALAC_UNUSED_DEP_CODE: &str = "scalac.unused.dep";

/// Names every dependency in `map` that the scalac log reports as unused.
///
/// The warning reads `Target '<label>' is specified as a dependency to <target> but isn't used`.
pub fn parse_scalac_unused_dependencies(
    map: &[u8],
    log: &[u8],
) -> Result<Vec<UnusedDependency>, ParseError> {
    let tool = "scalac";
    let json = |err: serde_json::Error| ParseError::Json {
        tool,
        detail: err.to_string(),
    };
    let map: DepsMap = serde_json::from_slice(map).map_err(json)?;
    let log: ScalacLog = serde_json::from_slice(log).map_err(json)?;
    let bare = |label: &str| label.trim_start_matches('@').to_owned();
    let mut found = Vec::new();
    for diagnostic in log
        .diagnostics
        .iter()
        .filter(|d| d.code == SCALAC_UNUSED_DEP_CODE)
    {
        let Some(rest) = diagnostic.message.strip_prefix("Target '") else {
            continue;
        };
        let Some((label, _)) = rest.split_once('\'') else {
            continue;
        };
        let label = bare(label);
        let Some(dep) = map.deps.iter().find(|dep| bare(&dep.label) == label) else {
            continue;
        };
        let name = dep.label.rsplit(':').next().unwrap_or(&dep.label);
        found.push(UnusedDependency {
            package: map.package.clone(),
            target: map.target.clone(),
            dependency: name.to_owned(),
            crate_name: dep.label.clone(),
        });
    }
    Ok(found)
}

/// Reads the `Dependencies` message of a jdeps file and returns the paths not marked unused.
/// Kotlin marks an idle jar UNUSED (2) where JavaBuilder writes 3.
fn used_jars(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    const NOT_USED: [u64; 2] = [2, 3];
    let mut used = BTreeSet::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let (field, wire, value) = next_field(&mut rest)?;
        if field != 1 || wire != 2 {
            continue;
        }
        let mut path = None;
        let mut kind = 0;
        let mut inner = value;
        while !inner.is_empty() {
            let (field, wire, value) = next_field(&mut inner)?;
            match (field, wire) {
                (1, 2) => path = Some(String::from_utf8_lossy(value).into_owned()),
                (2, 0) => kind = varint(value)?.0,
                _ => {}
            }
        }
        if let Some(path) = path.filter(|_| !NOT_USED.contains(&kind)) {
            used.insert(path);
        }
    }
    Ok(used)
}

/// Splits one field off the front of `rest`. Varints come back as their encoded bytes.
fn next_field<'a>(rest: &mut &'a [u8]) -> Result<(u64, u64, &'a [u8]), String> {
    let (tag, used) = varint(rest)?;
    let body = &rest[used..];
    let (value, after) = match tag & 7 {
        0 => {
            let (_, len) = varint(body)?;
            body.split_at(len)
        }
        1 => split(body, 8)?,
        5 => split(body, 4)?,
        2 => {
            let (len, used) = varint(body)?;
            let len = usize::try_from(len).map_err(|_| "length out of range".to_owned())?;
            let (value, after) = split(&body[used..], len)?;
            (value, after)
        }
        wire => return Err(format!("unsupported wire type {wire}")),
    };
    *rest = after;
    Ok((tag >> 3, tag & 7, value))
}

fn split(bytes: &[u8], len: usize) -> Result<(&[u8], &[u8]), String> {
    if bytes.len() < len {
        return Err("truncated jdeps file".to_owned());
    }
    Ok(bytes.split_at(len))
}

/// Decodes a varint from the front of `bytes` and returns it with the byte count.
fn varint(bytes: &[u8]) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    for (index, byte) in bytes.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    Err("bad varint in jdeps file".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependency(path: &str, kind: u8) -> Vec<u8> {
        let mut inner = vec![0x0a, path.len() as u8];
        inner.extend(path.as_bytes());
        inner.extend([0x10, kind]);
        let mut out = vec![0x0a, inner.len() as u8];
        out.extend(inner);
        out
    }

    const MAP: &str = r#"{"package":"p","target":"lib","deps":[
        {"label":"//a:used","jars":["a.jar"]},
        {"label":"//b:idle","jars":["b.jar"]},
        {"label":"//c:empty","jars":[]}]}"#;

    #[test]
    fn names_the_dependency_whose_jar_is_not_used() {
        let mut jdeps = dependency("a.jar", 0);
        jdeps.extend(dependency("b.jar", 3));
        let found = parse_unused_dependencies("javac", MAP.as_bytes(), &jdeps).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dependency, "idle");
        assert_eq!(found[0].target, "lib");
    }

    #[test]
    fn kotlin_marks_the_idle_jar_with_kind_two() {
        let mut jdeps = dependency("a.jar", 0);
        jdeps.extend(dependency("b.jar", 2));
        let found = parse_unused_dependencies("kotlinc", MAP.as_bytes(), &jdeps).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dependency, "idle");
    }

    #[test]
    fn implicit_use_counts_and_other_fields_are_skipped() {
        let mut jdeps = vec![0x18, 0x01];
        jdeps.extend(dependency("a.jar", 0));
        jdeps.extend(dependency("b.jar", 1));
        let found = parse_unused_dependencies("javac", MAP.as_bytes(), &jdeps).unwrap();
        assert!(found.is_empty());
    }

    #[test]
    fn a_truncated_jdeps_file_is_an_error() {
        assert!(parse_unused_dependencies("javac", MAP.as_bytes(), &[0x0a, 0x09, 0x01]).is_err());
        assert!(parse_unused_dependencies("javac", b"{", &[]).is_err());
    }

    const INDIRECT_MAP: &str = r#"{"package":"p","target":"lib","deps":[
        {"label":"//a:direct","jars":["a.jar"]}],
        "indirect":[
        {"jar":"u.jar","label":"//u:used"},
        {"jar":"u2.jar","label":"//u:used"},
        {"jar":"i.jar","label":"//i:idle"}]}"#;

    #[test]
    fn used_indirect_jar_is_missing_once_per_label() {
        let mut jdeps = dependency("a.jar", 0);
        jdeps.extend(dependency("u.jar", 0));
        jdeps.extend(dependency("u2.jar", 1));
        jdeps.extend(dependency("i.jar", 2));
        let found = parse_missing_dependencies("kotlinc", INDIRECT_MAP.as_bytes(), &jdeps).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "//u:used");
        assert_eq!(found[0].target, "lib");
    }

    #[test]
    fn jar_of_a_direct_dep_or_export_is_not_missing() {
        let jdeps = dependency("a.jar", 0);
        let found = parse_missing_dependencies("javac", INDIRECT_MAP.as_bytes(), &jdeps).unwrap();
        assert!(found.is_empty());
        assert!(parse_missing_dependencies("javac", MAP.as_bytes(), &jdeps)
            .unwrap()
            .is_empty());
    }

    fn scalac_log(entries: &[(&str, &str)]) -> Vec<u8> {
        let items: Vec<String> = entries
            .iter()
            .map(|(code, message)| {
                format!(
                    r#"{{"file":"","line":0,"column":0,"severity":"warning","code":"{code}","message":"{message}"}}"#
                )
            })
            .collect();
        format!(
            r#"{{"tool":"scalac","exit":0,"diagnostics":[{}]}}"#,
            items.join(",")
        )
        .into_bytes()
    }

    #[test]
    fn scalac_warning_names_the_dependency_through_the_map() {
        let log = scalac_log(&[(
            "scalac.unused.dep",
            "Target '@@//b:idle' is specified as a dependency to //p:lib but isn't used",
        )]);
        let found = parse_scalac_unused_dependencies(MAP.as_bytes(), &log).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].dependency, "idle");
        assert_eq!(found[0].target, "lib");
    }

    #[test]
    fn scalac_label_outside_the_map_is_ignored() {
        let log = scalac_log(&[(
            "scalac.unused.dep",
            "Target '//z:other' is specified as a dependency to //p:lib but isn't used",
        )]);
        assert!(parse_scalac_unused_dependencies(MAP.as_bytes(), &log)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn scalac_other_warnings_are_ignored() {
        let log = scalac_log(&[("scalac", "Target '//b:idle' is something else")]);
        assert!(parse_scalac_unused_dependencies(MAP.as_bytes(), &log)
            .unwrap()
            .is_empty());
    }
}
