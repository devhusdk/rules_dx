//! Composes the consumer MODULE.bazel from the one `dx init` writes.

use std::path::Path;

fn starlark_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub fn rules_dx_override(rules_dx: &Path) -> String {
    format!(
        "local_path_override(\n    module_name = \"rules_dx\",\n    path = {},\n)\n",
        starlark_string(&rules_dx.to_string_lossy()),
    )
}

/// Pins the patched rules_py the way the root MODULE.bazel does, until the BCR serves alpha.11 (#6811).
pub fn rules_py_override() -> String {
    let patches = [
        "py_cc_toolchain_platform",
        "native_sdist_toolchain_flags",
        "pep517_msvc",
        "venv_bin_files",
        "host_interpreter_windows_arm64",
        "external_repo_pytest_paths",
    ]
    .map(|name| format!("        \"//patches/rules_py:{name}.patch\",\n"))
    .concat();
    format!(
        "archive_override(\n    module_name = \"rules_py\",\n    integrity = \"sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=\",\n    patch_strip = 1,\n    patches = [\n{patches}    ],\n    strip_prefix = \"rules_py-2.0.0-alpha.11\",\n    urls = [\"https://github.com/aspect-build/rules_py/releases/download/v2.0.0-alpha.11/rules_py-v2.0.0-alpha.11.tar.gz\"],\n)\n"
    )
}

/// Drops the `module()` line and any comment or blank lines before it.
fn without_module_line(snippet: &str) -> String {
    let mut lines = snippet.lines().peekable();
    while let Some(line) = lines.peek() {
        if line.starts_with("module(") {
            lines.next();
            let rest: Vec<&str> = lines.collect();
            return rest.join("\n").trim_start().to_owned() + "\n";
        }
        if !line.starts_with('#') && !line.trim().is_empty() {
            break;
        }
        lines.next();
    }
    snippet.to_owned()
}

/// Builds the consumer MODULE.bazel from the `MODULE.bazel` a fresh `dx init` wrote.
///
/// Its `module()` line is replaced by one named `module`.
///
/// The fragment is appended.
pub fn compose_module(
    snippet: &str,
    module: &str,
    repo_name: &str,
    fragment: &str,
    rules_dx: &Path,
) -> Result<String, String> {
    let snippet = without_module_line(snippet);
    let snippet = snippet.as_str();
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
    let body = format!("{}\n{}", setup.trim_end(), fragment.trim_end());
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
            Path::new("/src/rules dx"),
        )
        .unwrap();
        assert_eq!(
            module,
            "module(name = \"hermetic_consumer\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\nbazel_dep(name = \"googletest\", version = \"1.18.0\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src/rules dx\",\n)\n"
        );
    }

    #[test]
    fn module_replaces_the_init_module_line() {
        let written = format!("module(name = \"my_project\")\n\n{SNIPPET}");
        let module = compose_module(&written, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert!(
            module.starts_with("module(name = \"m\")\n\nbazel_dep("),
            "{module}"
        );
        assert_eq!(module.matches("module(name").count(), 1);
    }

    #[test]
    fn module_replaces_the_init_module_line_after_a_comment_header() {
        let written = format!(
            "# dx: yours; dx adds its missing lines\nmodule(name = \"my_project\")\n\n{SNIPPET}"
        );
        let module = compose_module(&written, "m", "rules_dx", "", Path::new("/s")).unwrap();
        assert!(
            module.starts_with("module(name = \"m\")\n\nbazel_dep("),
            "{module}"
        );
        assert_eq!(module.matches("module(name").count(), 1);
    }

    #[test]
    fn module_keeps_the_default_name_unrenamed() {
        let module = compose_module(SNIPPET, "m", "rules_dx", "", Path::new("/src")).unwrap();
        assert!(module.contains("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")"));
        assert!(!module.contains("repo_name"));
    }

    #[test]
    fn module_escapes_quotes_in_the_override_path() {
        let module = compose_module(SNIPPET, "m", "my_dx", "", Path::new("/a\"b\\c")).unwrap();
        assert!(module.contains("path = \"/a\\\"b\\\\c\","), "{module}");
    }

    #[test]
    fn module_rejects_a_snippet_without_exactly_one_rules_dx() {
        let missing = compose_module(
            "bazel_dep(name = \"rules_cc\")\n",
            "m",
            "x",
            "",
            Path::new("/"),
        );
        assert!(missing.unwrap_err().contains("exactly once"));
        let twice = format!("{SNIPPET}{SNIPPET}");
        assert!(compose_module(&twice, "m", "x", "", Path::new("/")).is_err());
    }

    #[test]
    fn module_appends_the_fragment_after_the_snippet() {
        let fragment = "register_toolchains(\"@x//:all\")\n";
        let module = compose_module(SNIPPET, "m", "my_dx", fragment, Path::new("/src")).unwrap();
        assert!(
            module.find("@llvm//toolchain:all").unwrap() < module.find("@x").unwrap(),
            "{module}"
        );
    }
}
