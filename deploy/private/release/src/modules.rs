//! The version and dependency checks of the `rules_dx` module a release publishes.

use crate::candidate::{call_end, string_fields};

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

/// Fails when the `module()` version of the root `MODULE.bazel` text is not `version`.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn module_version_error(text: &str, version: &str) -> Result<(), String> {
    let have = calls(text, "module")
        .iter()
        .find_map(|call| field(call, "version").map(str::to_owned))
        .unwrap_or_default();
    if have != version {
        return Err(format!(
            "release: MODULE.bazel module() version is {have:?}, want {version:?}; land the version bump first"
        ));
    }
    Ok(())
}

/// Fails when the root `MODULE.bazel` text overrides one of its `bazel_dep` modules with `local_path_override`.
///
/// # Errors
///
/// Returns an error naming every such module.
pub fn local_override_error(text: &str) -> Result<(), String> {
    let deps = calls(text, "bazel_dep");
    let overrides = calls(text, "local_path_override");
    let overridden: Vec<&str> = overrides
        .iter()
        .filter_map(|call| field(call, "module_name"))
        .filter(|name| deps.iter().any(|dep| field(dep, "name") == Some(*name)))
        .collect();
    if overridden.is_empty() {
        return Ok(());
    }
    Err(format!(
        "release: MODULE.bazel overrides bazel_dep {} with local_path_override; depend on a released version first",
        overridden.join(", ")
    ))
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
        assert!(error.contains("MODULE.bazel module() version"), "{error}");
        assert!(error.ends_with("land the version bump first"), "{error}");
    }

    #[test]
    fn released_rule_set_deps_pass() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_php\", version = \"0.1.0\")\nlocal_path_override(module_name = \"bcr_only\", path = \"x\")\n",
            core("1.2.3")
        );
        local_override_error(&text).expect("no overridden dep");
    }

    #[test]
    fn a_rule_set_dep_with_local_path_override_fails_and_is_named() {
        let text = format!(
            "{}\nbazel_dep(name = \"rules_php\", version = \"0.1.0\")\nbazel_dep(name = \"rules_lua\", version = \"0.1.0\")\nlocal_path_override(\n    module_name = \"rules_lua\",\n    path = \"../rules_lua\",\n)\n",
            core("1.2.3")
        );
        let error = local_override_error(&text).expect_err("override");
        assert!(
            error.contains("bazel_dep rules_lua with local_path_override"),
            "{error}"
        );
        assert!(!error.contains("rules_php"), "{error}");
    }
}
