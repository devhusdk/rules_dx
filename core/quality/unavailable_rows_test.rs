//! A tool whose alias resolves to `dx_unavailable_tool` on windows_arm64 must not be listed
//! for that platform in any `families/*_tools.bzl` row.

use dx_testing::runfiles_root;

/// Names of `<name>_tool` aliases that select a `<name>_unsupported` stub on windows_arm64.
fn unavailable_tools(build: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    for line in build.lines() {
        if line == "alias(" {
            name = None;
        }
        if let Some(rest) = line.split("name = \"").nth(1) {
            if let Some(tool) = rest.split("_tool\"").next().filter(|_| rest.contains("_tool\"")) {
                if tool.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    name = Some(tool.to_owned());
                }
            }
        }
        if line.contains("\":windows_arm64\": \":") && line.contains("_unsupported\"") {
            if let Some(tool) = &name {
                out.push(tool.clone());
            }
        }
    }
    out
}

/// Lines of `rows` whose `tool(` row has `id` and a windows_arm64 platform entry.
fn offending_rows(rows: &str, id: &str) -> Vec<String> {
    let mut current = String::new();
    let mut out = Vec::new();
    for line in rows.lines() {
        if line == "    tool(" {
            current.clear();
        }
        if let Some(rest) = line.strip_prefix("        id = \"") {
            current = rest.split('"').next().unwrap_or("").to_owned();
        }
        if current == id && line.contains("platforms = [") && line.contains("\"windows_arm64\"") {
            out.push(line.to_owned());
        }
    }
    out
}

#[test]
fn unavailable_tools_are_not_listed_for_windows_arm64() {
    let root = runfiles_root();
    let build = std::fs::read_to_string(root.join("rust/private/quality/tools/BUILD.bazel"))
        .expect("readable tools BUILD.bazel");
    let tools = unavailable_tools(&build);
    assert!(!tools.is_empty(), "no unavailable stubs found");
    let mut failures = Vec::new();
    let families = root.join("core/quality/families");
    for entry in std::fs::read_dir(&families).expect("readable families dir") {
        let path = entry.expect("dir entry").path();
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        if !file.ends_with("_tools.bzl") {
            continue;
        }
        let rows = std::fs::read_to_string(&path).expect("readable tools bzl");
        for tool in &tools {
            for line in offending_rows(&rows, tool) {
                failures.push(format!(
                    "{file}: {tool} resolves to dx_unavailable_tool on windows_arm64 but a row lists it: {line}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn stub_scanner_reads_alias_selects() {
    let build = "alias(\n    name = \"deadnix_tool\",\n    actual = select({\n        \":windows_arm64\": \":deadnix_unsupported\",\n    }),\n)\n";
    assert_eq!(unavailable_tools(build), vec!["deadnix".to_owned()]);
}

#[test]
fn row_scanner_matches_only_the_named_tool() {
    let rows = "    tool(\n        id = \"a\",\n        platforms = [\"windows_arm64\"],\n    ),\n    tool(\n        id = \"b\",\n        platforms = [\"linux\"],\n    ),\n";
    assert_eq!(offending_rows(rows, "a").len(), 1);
    assert!(offending_rows(rows, "b").is_empty());
}
