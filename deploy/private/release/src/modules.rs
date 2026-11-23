use std::fmt;

use crate::candidate::{call_end, string_fields};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleError {
    VersionMismatch {
        have: String,
        want: String,
    },
    RootOnlyOverride {
        overrides: Vec<(String, &'static str)>,
    },
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionMismatch { have, want } => write!(
                f,
                "release: MODULE.bazel module() version is {have:?}, want {want:?}; land the version bump first"
            ),
            Self::RootOnlyOverride { overrides } => write!(
                f,
                "release: MODULE.bazel overrides {}; depend on a released version first",
                overrides
                    .iter()
                    .map(|(module, function)| format!("bazel_dep {module} with {function}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl std::error::Error for ModuleError {}

type Call = Vec<(String, String)>;

fn calls(text: &str, function: &str) -> Vec<Call> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&format!("{function}(")) {
        if at != 0
            && text
                .as_bytes()
                .get(..at)
                .is_none_or(|before| before.last() != Some(&b'\n'))
        {
            continue;
        }
        let open = at + function.len();
        if let Some(close) = call_end(text, open)
            && let Some(body) = text.get(open + 1..close)
        {
            let fields = string_fields(body, open + 1);
            found.push(fields.into_iter().map(|f| (f.0, f.1)).collect());
        }
    }
    found
}

fn field<'a>(call: &'a Call, key: &str) -> Option<&'a str> {
    call.iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

pub fn module_version_error(text: &str, version: &str) -> Result<(), ModuleError> {
    let have = calls(text, "module")
        .iter()
        .find_map(|call| field(call, "version").map(str::to_owned))
        .unwrap_or_default();
    if have != version {
        return Err(ModuleError::VersionMismatch {
            have,
            want: version.to_owned(),
        });
    }
    Ok(())
}

const ROOT_ONLY_OVERRIDES: [&str; 2] = ["local_path_override", "git_override"];

pub fn root_only_override_error(text: &str) -> Result<(), ModuleError> {
    let deps = calls(text, "bazel_dep");
    let overrides: Vec<(String, &'static str)> = ROOT_ONLY_OVERRIDES
        .iter()
        .flat_map(|&function| {
            calls(text, function)
                .into_iter()
                .filter_map(|call| field(&call, "module_name").map(str::to_owned))
                .filter(|name| deps.iter().any(|dep| field(dep, "name") == Some(name)))
                .map(move |name| (name, function))
                .collect::<Vec<_>>()
        })
        .collect();
    if overrides.is_empty() {
        return Ok(());
    }
    Err(ModuleError::RootOnlyOverride { overrides })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core(version: &str) -> String {
        format!("module(\n    name = \"rules_dx\",\n    version = \"{version}\",\n)\n")
    }

    #[test]
    fn the_module_version_may_match_the_tag() {
        module_version_error(&core("1.2.3"), "1.2.3").expect("versions");
    }

    #[test]
    fn the_module_version_must_match_the_tag() {
        let error = module_version_error(&core("1.2.2"), "1.2.3").expect_err("core");
        assert!(matches!(error, ModuleError::VersionMismatch { .. }));
        let error = error.to_string();
        assert!(error.contains("MODULE.bazel module() version"), "{error}");
        assert!(error.ends_with("land the version bump first"), "{error}");
    }

    #[test]
    fn released_rule_set_deps_pass() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_php\", version = \"0.1.0\")\nlocal_path_override(module_name = \"bcr_only\", path = \"x\")\n",
            core("1.2.3")
        );
        root_only_override_error(&text).expect("no overridden dep");
    }

    #[test]
    fn a_rule_set_dep_with_local_path_override_fails_and_is_named() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_php\", version = \"0.1.0\")\nbazel_dep(name = \"rules_lua\", version = \"0.1.0\")\nlocal_path_override(\n    module_name = \"rules_lua\",\n    path = \"../rules_lua\",\n)\n",
            core("1.2.3")
        );
        let error = root_only_override_error(&text).expect_err("override");
        assert!(
            matches!(&error, ModuleError::RootOnlyOverride { overrides } if overrides == &[("rules_lua".to_owned(), "local_path_override")])
        );
        let error = error.to_string();
        assert!(
            error.contains("bazel_dep rules_lua with local_path_override"),
            "{error}"
        );
        assert!(!error.contains("rules_php"), "{error}");
    }

    #[test]
    fn a_rule_set_dep_with_git_override_fails_and_is_named() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_qt\", version = \"0.1.0\")\ngit_override(\n    module_name = \"rules_qt\",\n    remote = \"https://example.com/rules_qt.git\",\n    commit = \"0123456789abcdef0123456789abcdef01234567\",\n)\n",
            core("1.2.3")
        );
        let error = root_only_override_error(&text).expect_err("override");
        assert!(
            matches!(&error, ModuleError::RootOnlyOverride { overrides } if overrides == &[("rules_qt".to_owned(), "git_override")])
        );
        let error = error.to_string();
        assert!(error.contains("bazel_dep rules_qt with git_override"), "{error}");
    }

    #[test]
    fn local_and_git_overrides_are_named_together() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_lua\", version = \"0.1.0\")\nbazel_dep(name = \"rules_qt\", version = \"0.1.0\")\nlocal_path_override(module_name = \"rules_lua\", path = \"../rules_lua\")\ngit_override(module_name = \"rules_qt\", remote = \"https://example.com/rules_qt.git\", commit = \"0123456789abcdef0123456789abcdef01234567\")\n",
            core("1.2.3")
        );
        let error = root_only_override_error(&text)
            .expect_err("overrides")
            .to_string();
        assert!(
            error.contains(
                "overrides bazel_dep rules_lua with local_path_override, bazel_dep rules_qt with git_override;"
            ),
            "{error}"
        );
    }

    #[test]
    fn archive_override_on_a_bazel_dep_passes() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_cc\", version = \"0.1.0\")\narchive_override(module_name = \"rules_cc\", urls = [\"https://example.com/rules_cc.tar.gz\"], integrity = \"sha256-x\")\n",
            core("1.2.3")
        );
        root_only_override_error(&text).expect("archive_override is copied into release cells");
    }

    #[test]
    fn git_override_on_a_module_that_is_no_bazel_dep_passes() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_php\", version = \"0.1.0\")\ngit_override(module_name = \"bcr_only\", remote = \"https://example.com/x.git\", commit = \"0123456789abcdef0123456789abcdef01234567\")\n",
            core("1.2.3")
        );
        root_only_override_error(&text).expect("no overridden dep");
    }
}
