//! Finds the `deps` of a Java or Kotlin rule whose jars the compiler never read.
//!
//! The aspect writes a JSON Lines map next to the compiler's jdeps file. The first line holds the
//! target. Each later line is a direct dependency with its compile jars (`{"dep": label, "jars":
//! [path]}`), a jar the target may use without listing it (`{"known": path}`) or one jar of its
//! transitive compile classpath with the label that owns it (`{"jar": path, "label": owner}`). A dependency is unused
//! when none of its jars is named as used in the jdeps file. A transitive jar is indirect when it
//! is neither a direct jar nor a known jar and the target itself does not own it.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::ParseError;
use super::rust_unused_deps::UnusedDependency;

pub const RULE_ID: &str = "unused_deps";

/// Suffix the aspect adds to the label of the wrapped target in an upstream wrapper.
const UPSTREAM_SUFFIX: &str = "_upstream";

#[derive(Debug, Deserialize)]
struct Header {
    package: String,
    target: String,
    label: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Entry {
    Dep { dep: String, jars: Vec<String> },
    Known { known: String },
    Classpath { jar: String, label: String },
}

#[derive(Debug)]
struct DepsMap {
    package: String,
    target: String,
    deps: Vec<Dep>,
    indirect: Vec<Indirect>,
}

#[derive(Debug)]
struct Indirect {
    jar: String,
    label: String,
}

#[derive(Debug)]
struct Dep {
    label: String,
    jars: Vec<String>,
}

/// Reads the JSON Lines map the aspect writes and works out its indirect jars.
fn read_map(tool: &'static str, bytes: &[u8]) -> Result<DepsMap, ParseError> {
    let json = |err: serde_json::Error| ParseError::Json {
        tool,
        detail: err.to_string(),
    };
    let mut lines = bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace));
    let Some(first) = lines.next() else {
        return Err(ParseError::Shape {
            tool,
            detail: "empty dependency map".to_owned(),
        });
    };
    let header: Header = serde_json::from_slice(first).map_err(json)?;
    let mut known = BTreeSet::new();
    let mut deps = Vec::new();
    let mut classpath = Vec::new();
    for line in lines {
        match serde_json::from_slice(line).map_err(json)? {
            Entry::Dep { dep, jars } => {
                known.extend(jars.iter().cloned());
                deps.push(Dep { label: dep, jars });
            }
            Entry::Known { known: jar } => {
                known.insert(jar);
            }
            Entry::Classpath { jar, label } => classpath.push(Indirect { jar, label }),
        }
    }
    let indirect = classpath
        .into_iter()
        .filter(|item| !known.contains(&item.jar) && item.label != header.label)
        .map(|item| Indirect {
            label: item
                .label
                .strip_suffix(UPSTREAM_SUFFIX)
                .unwrap_or(&item.label)
                .to_owned(),
            jar: item.jar,
        })
        .collect();
    Ok(DepsMap {
        package: header.package,
        target: header.target,
        deps,
        indirect,
    })
}

/// Names every dependency in `map` that none of the used jars of `jdeps` belongs to.
///
/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
pub fn parse_unused_dependencies(
    tool: &'static str,
    map: &[u8],
    jdeps: &[u8],
) -> Result<Vec<UnusedDependency>, ParseError> {
    let map = read_map(tool, map)?;
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
    #[must_use]
    pub fn message(&self) -> String {
        format!(
            "`{}` is used by `{}` but is not in its deps",
            self.label, self.target
        )
    }
}

/// Names every transitive dependency in `map` whose jar the compiler used, once per label.
///
/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
pub fn parse_missing_dependencies(
    tool: &'static str,
    map: &[u8],
    jdeps: &[u8],
) -> Result<Vec<MissingDependency>, ParseError> {
    let map = read_map(tool, map)?;
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
///
/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
pub fn parse_scalac_unused_dependencies(
    map: &[u8],
    log: &[u8],
) -> Result<Vec<UnusedDependency>, ParseError> {
    let tool = "scalac";
    let json = |err: serde_json::Error| ParseError::Json {
        tool,
        detail: err.to_string(),
    };
    let map = read_map(tool, map)?;
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

/// Code the scalac driver gives the dependency analyzer's missing-dependency warnings.
const SCALAC_MISSING_DEP_CODE: &str = "scalac.missing.dep";

/// Names every transitive dependency in `map` that the scalac log reports as used but not listed.
///
/// The warning reads `Target '<label>' is used but isn't explicitly declared, please add it to the deps.`
///
/// # Errors
///
/// Returns [`ParseError`] when the tool output cannot be parsed.
pub fn parse_scalac_missing_dependencies(
    map: &[u8],
    log: &[u8],
) -> Result<Vec<MissingDependency>, ParseError> {
    let tool = "scalac";
    let json = |err: serde_json::Error| ParseError::Json {
        tool,
        detail: err.to_string(),
    };
    let map = read_map(tool, map)?;
    let log: ScalacLog = serde_json::from_slice(log).map_err(json)?;
    let bare = |label: &str| {
        let label = label.trim_start_matches('@');
        label.strip_suffix("_upstream").unwrap_or(label).to_owned()
    };
    let mut labels = BTreeSet::new();
    for diagnostic in log
        .diagnostics
        .iter()
        .filter(|d| d.code == SCALAC_MISSING_DEP_CODE)
    {
        let Some((label, _)) = diagnostic
            .message
            .strip_prefix("Target '")
            .and_then(|rest| rest.split_once('\''))
        else {
            continue;
        };
        let label = bare(label);
        if let Some(item) = map.indirect.iter().find(|item| bare(&item.label) == label) {
            labels.insert(item.label.clone());
        }
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

/// Reads the `Dependencies` message of a jdeps file and returns the paths not marked unused.
/// Kotlin marks an idle jar UNUSED (2) where `JavaBuilder` writes 3.
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
    let (_, body) = split(rest, used)?;
    let (value, after) = match tag & 7 {
        0 => {
            let (_, len) = varint(body)?;
            body.split_at(len)
        }
        1 => split(body, 8)?,
        5 => split(body, 4)?,
        2 => {
            let (len, used) = varint(body)?;
            let len =
                usize::try_from(len).map_err(|error| format!("length out of range: {error}"))?;
            let (_, tail) = split(body, used)?;
            split(tail, len)?
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
        let mut inner = vec![0x0a, u8::try_from(path.len()).expect("short path")];
        inner.extend(path.as_bytes());
        inner.extend([0x10, kind]);
        let mut out = vec![0x0a, u8::try_from(inner.len()).expect("short message")];
        out.extend(inner);
        out
    }

    const MAP: &str = r#"{"package":"p","target":"lib","label":"//p:lib"}
{"dep":"//a:used","jars":["a.jar"]}
{"dep":"//b:idle","jars":["b.jar"]}
{"dep":"//c:empty","jars":[]}"#;

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
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn a_truncated_jdeps_file_is_an_error() {
        assert!(parse_unused_dependencies("javac", MAP.as_bytes(), &[0x0a, 0x09, 0x01]).is_err());
        assert!(parse_unused_dependencies("javac", b"{", &[]).is_err());
        assert!(parse_unused_dependencies("javac", b"", &[]).is_err());
        let bad_line = format!("{MAP}\n{{\"other\":1}}");
        assert!(parse_unused_dependencies("javac", bad_line.as_bytes(), &[]).is_err());
    }

    const INDIRECT_MAP: &str = r#"{"package":"p","target":"lib","label":"//p:lib_upstream"}
{"dep":"//a:direct","jars":["a.jar"]}
{"jar":"a.jar","label":"//a:direct"}
{"jar":"u.jar","label":"//u:used"}
{"jar":"u2.jar","label":"//u:used_upstream"}
{"jar":"i.jar","label":"//i:idle"}
{"jar":"own.jar","label":"//p:lib_upstream"}
{"known":"std.jar"}
{"jar":"std.jar","label":"//t:stdlib"}
"#;

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
    fn jar_of_the_target_or_a_known_jar_is_not_missing() {
        let mut jdeps = dependency("own.jar", 0);
        jdeps.extend(dependency("std.jar", 0));
        let found = parse_missing_dependencies("javac", INDIRECT_MAP.as_bytes(), &jdeps).unwrap();
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn jar_of_a_direct_dep_or_export_is_not_missing() {
        let jdeps = dependency("a.jar", 0);
        let found = parse_missing_dependencies("javac", INDIRECT_MAP.as_bytes(), &jdeps).unwrap();
        assert!(found.is_empty(), "{found:?}");
        let actual = parse_missing_dependencies("javac", MAP.as_bytes(), &jdeps).unwrap();
        assert!(actual.is_empty(), "{actual:?}");
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
    fn scalac_missing_warning_names_the_indirect_label() {
        let map = r#"{"package":"p","target":"lib","label":"//p:lib"}
{"jar":"t.jar","label":"@@//b:transitive"}"#;
        let log = scalac_log(&[
            (
                "scalac.missing.dep",
                "Target '@@//b:transitive' is used but isn't explicitly declared, please add it to the deps.",
            ),
            (
                "scalac.missing.dep",
                "Target '//z:other' is used but isn't explicitly declared, please add it to the deps.",
            ),
        ]);
        let found = parse_scalac_missing_dependencies(map.as_bytes(), &log).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "@@//b:transitive");
        assert_eq!(found[0].target, "lib");
    }

    #[test]
    fn scalac_label_outside_the_map_is_ignored() {
        let log = scalac_log(&[(
            "scalac.unused.dep",
            "Target '//z:other' is specified as a dependency to //p:lib but isn't used",
        )]);
        let actual = parse_scalac_unused_dependencies(MAP.as_bytes(), &log).unwrap();
        assert!(actual.is_empty(), "{actual:?}");
    }

    #[test]
    fn scalac_other_warnings_are_ignored() {
        let log = scalac_log(&[("scalac", "Target '//b:idle' is something else")]);
        let actual = parse_scalac_unused_dependencies(MAP.as_bytes(), &log).unwrap();
        assert!(actual.is_empty(), "{actual:?}");
    }
}
