//! Every repository a hermetic consumer's BUILD files reference is declared in its module fragment.

use std::collections::BTreeSet;
use std::path::Path;

use dx_module_bazel::{ModuleFile, Value};
use dx_testing::{read_text, runfiles_root};

/// Repositories every consumer has: the rules_dx repo name from `defs.bzl`, the
/// `rules_dx` module itself, and the ones Bazel provides.
const DEFAULT_REPOS: &[&str] = &["my_dx", "rules_dx", "platforms", "bazel_tools"];

/// The repository a label string names after `@` or `@@`, if it names one.
fn label_repo(label: &str) -> Option<&str> {
    let rest = label.strip_prefix('@')?;
    let rest = rest.strip_prefix('@').unwrap_or(rest);
    let name = rest.split(['/', ':']).next().unwrap_or_default();
    (!name.is_empty()).then_some(name)
}

/// Repository names after `@` in the label-like strings of a BUILD file.
fn referenced(build: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in build.lines() {
        let code = line.split('#').next().unwrap_or_default();
        for piece in code.split(['"', '\'']).skip(1).step_by(2) {
            if let Some(name) = label_repo(piece) {
                found.insert(name.to_owned());
            }
        }
    }
    found
}

/// Repositories a fragment declares with `bazel_dep`, `use_repo` or a repository rule, or
/// loads an extension or repository rule from: the family modules a consumer uses are
/// provided by the harness, and the label of the extension it loads must resolve.
fn declared(fragment: &str) -> BTreeSet<String> {
    let module = ModuleFile::parse(fragment).expect("fragment parses");
    let mut names = BTreeSet::new();
    for call in module.calls_named("bazel_dep") {
        if let Some(name) = call.str("repo_name").or_else(|| call.str("name")) {
            names.insert(name.to_owned());
        }
    }
    for call in module
        .calls_named("use_extension")
        .chain(module.calls_named("use_repo_rule"))
    {
        if let Some(Value::Str(label)) = call.positional.first() {
            names.extend(label_repo(&label.value).map(str::to_owned));
        }
    }
    for call in module.calls_named("use_repo") {
        for value in &call.positional[1..] {
            if let Value::Str(repo) = value {
                names.insert(repo.value.clone());
            }
        }
        names.extend(call.named.iter().map(|(alias, _)| alias.clone()));
    }
    for rule in module.calls_named("use_repo_rule") {
        let Some(binding) = &rule.binding else {
            continue;
        };
        for call in module.calls_named(binding) {
            if let Some(name) = call.str("name") {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

fn build_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{dir:?}: {error}"));
    for entry in entries.map(|entry| entry.expect("directory entry").path()) {
        if entry.is_dir() {
            build_files(&entry, out);
        } else if entry.to_string_lossy().ends_with("BUILD.bazel") {
            out.push(entry);
        }
    }
}

/// The undeclared repositories of one consumer, as `file: @repo` lines.
fn undeclared(consumer: &Path) -> Vec<String> {
    let declared = declared(&read_text(&consumer.join("module_fragment.bazel")));
    let mut files = Vec::new();
    build_files(consumer, &mut files);
    let mut problems = Vec::new();
    for file in files {
        for repo in referenced(&read_text(&file)) {
            if !declared.contains(&repo) && !DEFAULT_REPOS.contains(&repo.as_str()) {
                let name = file.strip_prefix(consumer).unwrap_or(&file);
                problems.push(format!("{}: @{repo}", name.display()));
            }
        }
    }
    problems
}

#[test]
fn consumers_declare_the_repositories_they_reference() {
    let root = runfiles_root().join("tools/testing/hermetic/consumers");
    let mut problems = Vec::new();
    let mut count = 0;
    for entry in std::fs::read_dir(&root).expect("consumers directory") {
        let consumer = entry.expect("consumer entry").path();
        if consumer.join("module_fragment.bazel").exists() {
            count += 1;
            let name = consumer.file_name().unwrap().to_string_lossy().into_owned();
            problems.extend(
                undeclared(&consumer)
                    .into_iter()
                    .map(|line| format!("{name}/{line}")),
            );
        }
    }
    assert!(count > 10, "found only {count} consumers");
    assert!(
        problems.is_empty(),
        "a consumer references a repository its module_fragment.bazel does not declare:\n{}",
        problems.join("\n")
    );
}

#[test]
fn an_undeclared_repository_is_found() {
    let build = "deps = [\"@rules_cc//cc:defs\", \"@rules_dx//x:y\", \"@@maven//:z\", \"//local:t\"]  # \"@commented//a\"";
    let found: Vec<_> = referenced(build).into_iter().collect();
    assert_eq!(found, ["maven", "rules_cc", "rules_dx"]);
    let fragment = "bazel_dep(name = \"rules_cc\", version = \"1\")\nuse_repo(maven, \"maven\")\n";
    assert_eq!(
        declared(fragment).into_iter().collect::<Vec<_>>(),
        ["maven", "rules_cc"]
    );
    // A family module the harness provides is declared by the extension the fragment loads from it.
    let family = "cuda_tools = use_extension(\"@rules_dx_cuda//:extensions.bzl\", \"cuda_tools\")\n\
                  rule = use_repo_rule(\"@@rules_dx_android//toolchain:repos.bzl\", \"repo\")\n";
    assert_eq!(
        declared(family).into_iter().collect::<Vec<_>>(),
        ["rules_dx_android", "rules_dx_cuda"]
    );
}
