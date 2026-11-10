//! Every non-dev bazel_dep of the root module has a consumer in the shipped tree.

use std::path::Path;

use dx_module_bazel::{Call, ModuleFile, Value};

const SKIPPED_DIRS: &[&str] = &[
    "tests",
    "test",
    "testdata",
    "fixtures",
    "examples",
    "e2e",
    "testing",
    "node_modules",
];

const SKIPPED_ROOTS: &[&str] = &["tools", "docs", "bcr_test_module"];

const NEEDED_MARKER: &str = "# needed:";

fn shipped_starlark(root: &Path, dir: &Path, found: &mut Vec<String>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read_dir {dir:?}: {error}"));
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || name.starts_with("bazel-") {
            continue;
        }
        if path.is_dir() {
            let top = dir == root;
            if SKIPPED_DIRS.contains(&name.as_str())
                || (top && SKIPPED_ROOTS.contains(&name.as_str()))
                || path.join("MODULE.bazel").exists()
            {
                continue;
            }
            shipped_starlark(root, &path, found);
        } else if name.ends_with(".bzl") || name == "BUILD.bazel" {
            if let Ok(text) = std::fs::read_to_string(&path) {
                found.push(text);
            }
        }
    }
}

fn mentions(text: &str, dep: &str) -> bool {
    let needle = format!("@{dep}");
    text.match_indices(&needle).any(|(start, found)| {
        matches!(
            text[start + found.len()..].chars().next(),
            Some('/' | '"' | ':' | '+')
        )
    })
}

fn is_dev(call: &Call) -> bool {
    matches!(call.arg("dev_dependency"), Some(Value::Ident(value)) if value == "True")
}

fn non_dev_deps(module: &ModuleFile) -> Vec<String> {
    module
        .calls_named("bazel_dep")
        .filter(|call| !is_dev(call))
        .filter_map(|call| call.str("name").map(str::to_owned))
        .collect()
}

/// Workspace paths of the `build_file` labels the module passes to repository rules.
fn build_files(module: &ModuleFile) -> Vec<String> {
    module
        .calls()
        .iter()
        .filter_map(|call| call.str("build_file"))
        .filter_map(|label| label.strip_prefix("//"))
        .map(|label| label.replacen(':', "/", 1))
        .collect()
}

fn uses_extension(module: &ModuleFile, dep: &str) -> bool {
    let prefix = format!("@{dep}//");
    module
        .calls_named("use_extension")
        .filter(|call| !is_dev(call))
        .any(|call| {
            let bzl = match call.positional.first() {
                Some(Value::Str(literal)) => Some(literal.value.as_str()),
                _ => call.str("extension_bzl_file"),
            };
            bzl.is_some_and(|bzl| bzl.starts_with(&prefix))
        })
}

fn is_patched(module: &ModuleFile, dep: &str) -> bool {
    module
        .calls()
        .iter()
        .filter(|call| call.name == "single_version_override" || call.name == "archive_override")
        .any(|call| call.str("module_name") == Some(dep) && !call.str_list("patches").is_empty())
}

/// The repos the `# needed: @repo ... loads @dep` lines directly above the `bazel_dep` of
/// `dep` name as its loaders.
fn needed_by(module: &ModuleFile, dep: &str) -> Vec<String> {
    let Some(call) = module
        .calls_named("bazel_dep")
        .find(|call| call.str("name") == Some(dep))
    else {
        return Vec::new();
    };
    let loads = format!("loads @{dep}");
    module.source()[..call.span.start]
        .lines()
        .rev()
        .map(str::trim)
        .take_while(|line| line.starts_with('#'))
        .filter_map(|line| line.strip_prefix(NEEDED_MARKER))
        .filter(|reason| reason.trim_end().ends_with(&loads))
        .filter_map(|reason| reason.trim_start().strip_prefix('@'))
        .filter_map(|reason| reason.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

fn imports_repo(module: &ModuleFile, repo: &str) -> bool {
    module.calls_named("use_repo").any(|call| {
        call.positional
            .iter()
            .any(|value| matches!(value, Value::Str(literal) if literal.value == repo))
    })
}

fn has_consumer(module: &ModuleFile, starlark: &[String], dep: &str) -> bool {
    starlark.iter().any(|text| mentions(text, dep))
        || uses_extension(module, dep)
        || is_patched(module, dep)
        || needed_by(module, dep)
            .iter()
            .any(|repo| imports_repo(module, repo))
}

/// The non-dev deps of `module` without a consumer in `module` or `starlark`.
fn unused(module: &ModuleFile, starlark: &[String]) -> Vec<String> {
    non_dev_deps(module)
        .iter()
        .filter(|dep| !has_consumer(module, starlark, dep))
        .map(|dep| {
            format!("bazel_dep {dep:?} has no consumer; make it dev_dependency or remove it")
        })
        .collect()
}

fn parse(source: &str) -> ModuleFile {
    ModuleFile::parse(source).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn every_non_dev_bazel_dep_has_a_consumer() {
    let root = dx_testing::rules_dx_source();
    let module = parse(&dx_testing::read_runfiles("MODULE.bazel"));
    let mut starlark = Vec::new();
    shipped_starlark(&root, &root, &mut starlark);
    assert!(
        starlark.len() > 100,
        "found only {} shipped Starlark files",
        starlark.len()
    );
    for path in build_files(&module) {
        let file = root.join(&path);
        starlark.push(
            std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{file:?}: {error}")),
        );
    }
    let problems = unused(&module, &starlark);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_dep_without_a_consumer_fails() {
    let module = parse(
        "bazel_dep(name = \"demo\", version = \"1\")\nbazel_dep(name = \"used\", version = \"1\")\n",
    );
    let starlark = vec!["load(\"@used//:defs.bzl\", \"x\")".to_owned()];
    let problems = unused(&module, &starlark);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("\"demo\""), "{problems:?}");
}

#[test]
fn a_dev_dep_needs_no_consumer() {
    let module = parse("bazel_dep(name = \"demo\", version = \"1\", dev_dependency = True)\n");
    assert!(unused(&module, &[]).is_empty());
}

#[test]
fn a_non_dev_extension_is_a_consumer() {
    let dep = "bazel_dep(name = \"demo\", version = \"1\")\n";
    let used = parse(&format!(
        "{dep}ext = use_extension(\"@demo//:extensions.bzl\", \"ext\")\n"
    ));
    assert!(unused(&used, &[]).is_empty());
    let dev = parse(&format!(
        "{dep}ext = use_extension(\"@demo//:extensions.bzl\", \"ext\", dev_dependency = True)\n"
    ));
    assert_eq!(unused(&dev, &[]).len(), 1);
    let other = parse(&format!(
        "{dep}ext = use_extension(\"@demo_extra//:extensions.bzl\", \"ext\")\n"
    ));
    assert_eq!(unused(&other, &[]).len(), 1);
}

#[test]
fn a_patched_override_is_a_consumer() {
    let dep = "bazel_dep(name = \"demo\", version = \"1\")\n";
    let patched = parse(&format!(
        "{dep}single_version_override(module_name = \"demo\", patches = [\"//patches/demo:a.patch\"], version = \"1\")\n"
    ));
    assert!(unused(&patched, &[]).is_empty());
    let pinned = parse(&format!(
        "{dep}single_version_override(module_name = \"demo\", version = \"1\")\n"
    ));
    assert_eq!(unused(&pinned, &[]).len(), 1);
}

#[test]
fn a_needed_line_counts_only_for_an_imported_repo() {
    let dep = "# needed: @loader (ext extension) loads @demo\nbazel_dep(name = \"demo\", version = \"1\")\n";
    let ext = "ext = use_extension(\"//:extensions.bzl\", \"ext\")\n";
    let imported = parse(&format!("{dep}{ext}use_repo(ext, \"loader\")\n"));
    assert!(unused(&imported, &[]).is_empty());
    let missing = parse(&format!("{dep}{ext}use_repo(ext, \"other\")\n"));
    assert_eq!(unused(&missing, &[]).len(), 1);
    let detached = parse(&format!(
        "# needed: @loader (ext extension) loads @demo\n\nbazel_dep(name = \"demo\", version = \"1\")\n{ext}use_repo(ext, \"loader\")\n"
    ));
    assert_eq!(unused(&detached, &[]).len(), 1);
    let wrong_dep = parse(&format!(
        "# needed: @loader (ext extension) loads @else\nbazel_dep(name = \"demo\", version = \"1\")\n{ext}use_repo(ext, \"loader\")\n"
    ));
    assert_eq!(unused(&wrong_dep, &[]).len(), 1);
}

#[test]
fn build_file_labels_resolve_to_workspace_paths() {
    let module = parse(
        "http_archive = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\nhttp_archive(name = \"demo\", build_file = \"//tools/demo:demo.BUILD\")\n",
    );
    assert_eq!(build_files(&module), ["tools/demo/demo.BUILD"]);
}

#[test]
fn a_prefix_of_another_repo_name_is_not_a_load() {
    assert!(!mentions(
        "load(\"@rules_python_extra//:x.bzl\")",
        "rules_python"
    ));
    assert!(mentions("load(\"@rules_python//:x.bzl\")", "rules_python"));
    assert!(mentions("\"@rules_python\"", "rules_python"));
}
