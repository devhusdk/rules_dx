//! Every public name in modules/*.bzl is loaded somewhere, so no pin goes dead unnoticed.

/// Files that load names from modules/*.bzl. Add a new loader here.
const LOADERS: &[&str] = &[
    "cuda/toolchain/check.bzl",
    "java/ide/BUILD.bazel",
    "libs/testing/tested_stack.bzl",
    "lua/toolchain/BUILD.bazel",
    "python/rules/runtime_tests.bzl",
    "rust/rules/edition.bzl",
];

/// Top-level `NAME = ...` constants of a .bzl source.
fn public_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let (name, _) = line.split_once(" = ")?;
            let public = !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                && !name.starts_with('_');
            public.then(|| name.to_owned())
        })
        .collect()
}

/// Whether `name` is used as an identifier on a non-definition, non-comment line of `source`.
fn used_in_file(source: &str, name: &str) -> bool {
    source.lines().any(|line| {
        let code = line.split('#').next().unwrap_or("");
        !code.starts_with(&format!("{name} = "))
            && code.match_indices(name).any(|(at, _)| {
                let word =
                    |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
                !word(code[..at].chars().next_back())
                    && !word(code[at + name.len()..].chars().next())
            })
    })
}

/// Names in `defs` (file, source) neither quoted in a load() line of `loaders` nor used in-file.
fn unloaded(defs: &[(String, String)], loaders: &[String]) -> Vec<String> {
    let loads: Vec<&str> = loaders
        .iter()
        .flat_map(|source| source.lines())
        .filter(|line| line.trim_start().starts_with("load("))
        .collect();
    let mut dead = Vec::new();
    for (file, source) in defs {
        for name in public_names(source) {
            let quoted = format!("\"{name}\"");
            if !loads.iter().any(|line| line.contains(&quoted)) && !used_in_file(source, &name) {
                dead.push(format!("{file}: {name}"));
            }
        }
    }
    dead
}

#[test]
fn every_public_module_constant_is_loaded() {
    let files = [
        "consumer_deps",
        "cuda",
        "dotnet",
        "java-scala-kotlin",
        "js",
        "powershell",
        "python",
        "ruby",
        "rust",
        "toolchains",
        "versions",
    ];
    let mut defs: Vec<(String, String)> = files
        .iter()
        .map(|name| {
            let path = format!("modules/{name}.bzl");
            let source = dx_testing::read_runfiles(&path);
            (path, source)
        })
        .collect();
    defs.sort();
    let mut loaders: Vec<String> = LOADERS
        .iter()
        .map(|path| dx_testing::read_runfiles(path))
        .collect();
    for name in files.iter().chain(["versions_tests"].iter()) {
        loaders.push(dx_testing::read_runfiles(&format!("modules/{name}.bzl")));
    }
    let dead = unloaded(&defs, &loaders);
    assert!(
        dead.is_empty(),
        "public constants loaded nowhere (delete them, load them, or add the loader to LOADERS):\n{}",
        dead.join("\n")
    );
}

#[test]
fn an_unquoted_or_commented_name_counts_as_unloaded() {
    let defs = vec![(
        "a.bzl".to_owned(),
        "KEPT = 1\nDEAD = 2\n_PRIVATE = 3\n".to_owned(),
    )];
    let loaders = vec!["# load(\":a.bzl\", \"DEAD\")\nload(\":a.bzl\", \"KEPT\")\n".to_owned()];
    assert_eq!(unloaded(&defs, &loaders), vec!["a.bzl: DEAD".to_owned()]);
}
