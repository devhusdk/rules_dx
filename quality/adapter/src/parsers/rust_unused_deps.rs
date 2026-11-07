//! Places rustc's `unused_crate_dependencies` warnings on the BUILD file entry that causes them.
//!
//! The Rust aspect writes one `dx_crate_deps` line per crate next to the compiler diagnostics. It
//! maps each dependency's crate name to its target name, which is all the BUILD file needs.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::ParseError;

pub const RULE_ID: &str = "unused_crate_dependencies";

const DEPS_MESSAGE_TYPE: &str = "dx_crate_deps";

#[derive(Debug, Deserialize)]
struct CrateDeps {
    package: String,
    target: String,
    #[serde(rename = "crate")]
    crate_name: String,
    deps: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    message: String,
    code: Option<Code>,
}

#[derive(Debug, Deserialize)]
struct Code {
    code: String,
}

/// A dependency of one rule that its sources never use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnusedDependency {
    pub package: String,
    pub target: String,
    pub dependency: String,
    pub crate_name: String,
}

impl UnusedDependency {
    pub fn message(&self) -> String {
        format!(
            "`{}` is in deps of `{}` but no source uses it",
            self.crate_name, self.target
        )
    }
}

/// Reads the diagnostics and `dx_crate_deps` lines and names every unused dependency once.
pub fn parse_unused_dependencies(stream: &[u8]) -> Result<Vec<UnusedDependency>, ParseError> {
    let tool = "rustc";
    let text = std::str::from_utf8(stream).map_err(|err| ParseError::Shape {
        tool,
        detail: err.to_string(),
    })?;
    let mut crates = Vec::new();
    let mut unused: Vec<(String, String)> = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|err| ParseError::Shape {
                tool,
                detail: format!("unparsable line: {err}"),
            })?;
        match value
            .get("$message_type")
            .and_then(serde_json::Value::as_str)
        {
            Some(DEPS_MESSAGE_TYPE) => {
                crates.push(serde_json::from_value::<CrateDeps>(value).map_err(|err| {
                    ParseError::Shape {
                        tool,
                        detail: format!("malformed {DEPS_MESSAGE_TYPE}: {err}"),
                    }
                })?);
            }
            Some("diagnostic") => {
                let diagnostic: Diagnostic =
                    serde_json::from_value(value).map_err(|err| ParseError::Shape {
                        tool,
                        detail: format!("malformed diagnostic: {err}"),
                    })?;
                if diagnostic.code.is_some_and(|code| code.code == RULE_ID)
                    && let Some(names) = unused_names(&diagnostic.message)
                {
                    unused.push(names);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for (extern_crate, in_crate) in unused {
        for entry in crates.iter().filter(|entry| entry.crate_name == in_crate) {
            if let Some(dependency) = entry.deps.get(&extern_crate) {
                let item = UnusedDependency {
                    package: entry.package.clone(),
                    target: entry.target.clone(),
                    dependency: dependency.clone(),
                    crate_name: extern_crate.clone(),
                };
                if !found.contains(&item) {
                    found.push(item);
                }
            }
        }
    }
    Ok(found)
}

/// Splits "external crate `a` unused in `b`: ..." into the dependency and the crate using it.
fn unused_names(message: &str) -> Option<(String, String)> {
    let rest = message.strip_prefix("external crate `")?;
    let (extern_crate, rest) = rest.split_once('`')?;
    let rest = rest.strip_prefix(" unused in `")?;
    let (in_crate, _) = rest.split_once('`')?;
    Some((extern_crate.to_owned(), in_crate.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(extern_crate: &str) -> String {
        format!(
            concat!(
                r#"{{"$message_type":"dx_crate_deps","package":"p","target":"lib","crate":"lib","deps":{{"used":"used","serde":"serde"}}}}"#,
                "\n",
                r#"{{"$message_type":"diagnostic","message":"external crate `{}` unused in `lib`: remove the dependency or add `use {} as _;`","code":{{"code":"unused_crate_dependencies"}},"level":"warning","spans":[],"children":[]}}"#,
            ),
            extern_crate, extern_crate
        )
    }

    #[test]
    fn names_the_unused_dependency() {
        let found = parse_unused_dependencies(stream("serde").as_bytes()).expect("parsed");
        assert_eq!(
            found,
            vec![UnusedDependency {
                package: "p".into(),
                target: "lib".into(),
                dependency: "serde".into(),
                crate_name: "serde".into(),
            }]
        );
    }

    #[test]
    fn ignores_crates_without_a_dep_entry() {
        let found = parse_unused_dependencies(stream("other").as_bytes()).expect("parsed");
        assert!(found.is_empty());
    }
}
