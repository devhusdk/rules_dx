//! Composes the consumer MODULE.bazel from the `dx init` snippet.

use std::path::Path;

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Inserts `fragment` before the first `register_toolchains(` line of `setup`.
fn insert_before_toolchains(setup: &str, fragment: &str) -> Result<String, String> {
    let mut at = None;
    let mut offset = 0;
    for line in setup.split_inclusive('\n') {
        if line.starts_with("register_toolchains(") {
            at = Some(offset);
            break;
        }
        offset += line.len();
    }
    let Some(at) = at else {
        return Ok(format!("{}\n{}", setup.trim_end(), fragment.trim_end()));
    };
    Ok(format!(
        "{}{}\n\n{}",
        &setup[..at],
        fragment.trim_end(),
        &setup[at..]
    ))
}

pub fn rules_dx_override(rules_dx: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&rules_dx.to_string_lossy()),
    )
}

/// Builds the consumer MODULE.bazel from the `dx init` snippet.
///
/// The fragment is appended, or with `before_toolchains` placed ahead of the snippet's first
/// toolchain registration so its own registrations resolve first.
pub fn compose_module(
    snippet: &str,
    module: &str,
    repo_name: &str,
    fragment: &str,
    before_toolchains: bool,
    rules_dx: &Path,
) -> Result<String, String> {
    let needle = "bazel_dep(name = \"rules_dx\", ";
    if snippet.matches(needle).count() != 1 {
        return Err(format!(
            "dx init snippet must declare rules_dx exactly once: {snippet}"
        ));
    }
    let setup = if repo_name == "rules_dx" {
        snippet.to_owned()
    } else {
        snippet.replacen(
            needle,
            &format!("{needle}repo_name = {}, ", starlark_string(repo_name)),
            1,
        )
    };
    let body = if before_toolchains {
        insert_before_toolchains(&setup, fragment)?
            .trim_end()
            .to_owned()
    } else {
        format!("{}\n{}", setup.trim_end(), fragment.trim_end())
    };
    Ok(format!(
        "module(name = {})\n\n{}\n{}",
        starlark_string(module),
        body,
        rules_dx_override(rules_dx),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNIPPET: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\n";

    #[test]
    fn module_renames_rules_dx_and_overrides_it_locally() {
        let module = compose_module(
            SNIPPET,
            "hermetic_consumer",
            "my_dx",
            "bazel_dep(name = \"googletest\", version = \"1.18.0\")\n",
            false,
            Path::new("/src/rules dx"),
        )
        .unwrap();
        assert_eq!(
            module,
            "module(name = \"hermetic_consumer\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\nbazel_dep(name = \"googletest\", version = \"1.18.0\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src/rules dx\",\n)\n"
        );
    }

    #[test]
    fn module_keeps_the_default_name_unrenamed() {
        let module =
            compose_module(SNIPPET, "m", "rules_dx", "", false, Path::new("/src")).unwrap();
        assert!(module.contains("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")"));
        assert!(!module.contains("repo_name"));
    }

    #[test]
    fn module_escapes_quotes_in_the_override_path() {
        let module =
            compose_module(SNIPPET, "m", "my_dx", "", false, Path::new("/a\"b\\c")).unwrap();
        assert!(module.contains("path = \"/a\\\"b\\\\c\","), "{module}");
    }

    #[test]
    fn module_rejects_a_snippet_without_exactly_one_rules_dx() {
        let missing = compose_module(
            "bazel_dep(name = \"rules_cc\")\n",
            "m",
            "x",
            "",
            false,
            Path::new("/"),
        );
        assert!(missing.unwrap_err().contains("exactly once"));
        let twice = format!("{SNIPPET}{SNIPPET}");
        assert!(compose_module(&twice, "m", "x", "", false, Path::new("/")).is_err());
    }

    #[test]
    fn module_puts_the_fragment_before_the_first_toolchain_registration_on_request() {
        let module = compose_module(
            SNIPPET,
            "m",
            "my_dx",
            "register_toolchains(\"@cuda_host_llvm//:all\")\n\n",
            true,
            Path::new("/src"),
        )
        .unwrap();
        assert_eq!(
            module,
            "module(name = \"m\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@cuda_host_llvm//:all\")\n\nregister_toolchains(\"@llvm//toolchain:all\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src\",\n)\n"
        );
    }

    #[test]
    fn module_appends_the_fragment_when_the_snippet_registers_nothing() {
        let module = compose_module(
            "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n",
            "m",
            "rules_dx",
            "register_toolchains(\"@x//:all\")\n",
            true,
            Path::new("/src"),
        )
        .unwrap();
        assert!(module.contains("0.0.0\")\nregister_toolchains(\"@x//:all\")\nlocal_path"));
    }

    #[test]
    fn module_appends_the_fragment_unchanged_without_the_flag() {
        let fragment = "register_toolchains(\"@cuda_host_llvm//:all\")\n";
        let module =
            compose_module(SNIPPET, "m", "my_dx", fragment, false, Path::new("/src")).unwrap();
        assert!(
            module.find("@llvm//toolchain:all").unwrap() < module.find("@cuda_host_llvm").unwrap(),
            "{module}"
        );
    }
}
