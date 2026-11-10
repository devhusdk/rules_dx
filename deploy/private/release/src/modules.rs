//! The version check of the `rules_dx` module a release publishes.

use crate::candidate::{call_end, string_fields};

type Call = Vec<(String, String)>;

fn calls(text: &str, function: &str) -> Vec<Call> {
    let mut found = Vec::new();
    for (at, _) in text.match_indices(&format!("{function}(")) {
        if at != 0 && text.as_bytes()[at - 1] != b'\n' {
            continue;
        }
        let open = at + function.len();
        if let Some(close) = call_end(text, open) {
            let fields = string_fields(&text[open + 1..close], open + 1);
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
}
