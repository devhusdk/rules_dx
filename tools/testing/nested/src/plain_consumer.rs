//! Checks that a rule-set fixture is a plain Bazel consumer of one `rules_<lang>` module.
//!
//! A rule-set module keeps its fixture in `<lang>/tests/fixtures/plain/` and its budget in
//! `<lang>/<lang>_fixture_budget.json`. The budget is a JSON object with the keys
//! `modules`, `lock_bytes`, `lock_extensions` and `external_repos`, each a number, as read by
//! `footprint::check_budget`.

/// Returns `Ok` when `module_bazel` depends only on `rules_<lang>` and `files` has no `.dx/` path.
pub fn check_plain_consumer(lang: &str, module_bazel: &str, files: &[&str]) -> Result<(), String> {
    let wanted = format!("rules_{lang}");
    let mut problems = Vec::new();
    for line in module_bazel.lines() {
        let code = line.split('#').next().unwrap_or("").trim();
        if let Some(name) = bazel_dep_name(code) {
            if name != wanted {
                problems.push(format!("bazel_dep on {name} is not {wanted}: {code}"));
            }
        }
        if uses_dx_extension(code) {
            problems.push(format!("dx extension use: {code}"));
        }
    }
    for file in files {
        let path = file.replace('\\', "/");
        let path = path.trim_start_matches("./");
        if path == ".dx" || path.starts_with(".dx/") || path.contains("/.dx/") {
            problems.push(format!("{file} must not exist in a plain consumer"));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn bazel_dep_name(code: &str) -> Option<String> {
    let rest = code.strip_prefix("bazel_dep(")?;
    let at = rest.find("name")?;
    let after = rest[at + "name".len()..].trim_start().strip_prefix('=')?;
    let after = after.trim_start().strip_prefix('"')?;
    Some(after[..after.find('"')?].to_owned())
}

fn uses_dx_extension(code: &str) -> bool {
    let Some(at) = code.find("use_extension(") else {
        return code.contains("use_repo(dx") || code.starts_with("dx.") || code.starts_with("dx_");
    };
    let rest = &code[at + "use_extension(".len()..];
    let label = rest.split('"').nth(1).unwrap_or("");
    label.contains("rules_dx")
        || label.contains("//dx")
        || label.contains(":dx")
        || code.trim_start().starts_with("dx")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(lang: &str) -> String {
        format!(
            "module(name = \"plain\")\n\
             bazel_dep(name = \"rules_{lang}\", version = \"0.0.0\")\n\
             local_path_override(module_name = \"rules_{lang}\", path = \"../../..\")\n"
        )
    }

    #[test]
    fn accepts_a_clean_consumer_per_language() {
        for lang in ["php", "lua", "qt"] {
            assert_eq!(
                check_plain_consumer(lang, &module(lang), &["MODULE.bazel", "BUILD.bazel"]),
                Ok(())
            );
        }
    }

    #[test]
    fn rejects_a_rules_dx_dep() {
        let text = format!(
            "{}bazel_dep(name = \"rules_dx\", version = \"0\")\n",
            module("php")
        );
        let error = check_plain_consumer("php", &text, &[]).expect_err("rules_dx");
        assert!(error.contains("rules_dx"), "{error}");
    }

    #[test]
    fn rejects_a_second_dep() {
        let text = format!(
            "{}bazel_dep(name = \"bazel_skylib\", version = \"1\")\n",
            module("lua")
        );
        let error = check_plain_consumer("lua", &text, &[]).expect_err("second dep");
        assert!(error.contains("bazel_skylib"), "{error}");
    }

    #[test]
    fn rejects_the_dx_bazelrc() {
        let error = check_plain_consumer("qt", &module("qt"), &[".dx/bazelrc"]).expect_err("rc");
        assert!(error.contains(".dx/bazelrc"), "{error}");
    }

    #[test]
    fn rejects_a_dx_extension() {
        let text = format!(
            "{}dx = use_extension(\"@rules_dx//dx:ext.bzl\", \"dx\")\n",
            module("php")
        );
        let error = check_plain_consumer("php", &text, &[]).expect_err("extension");
        assert!(error.contains("dx extension"), "{error}");
    }

    #[test]
    fn a_language_extension_is_allowed() {
        let text = format!(
            "{}lua = use_extension(\"@rules_lua//lua:ext.bzl\", \"lua\")\n",
            module("lua")
        );
        assert_eq!(check_plain_consumer("lua", &text, &[]), Ok(()));
    }
}
