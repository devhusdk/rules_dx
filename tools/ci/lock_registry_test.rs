//! The tree breaks no rule of the Checks section of docs/design/locks.md outside tools/ci/lock_registry_allowlist.txt, and the allowlist holds no spare line.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstExpr, AstLiteral, AstStmt, ExprP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};
use yaml_serde as _;

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/lock_registry_allowlist.txt";

/// Attribute names no hub extension tag may have.
const BANNED_ATTRS: [&str; 7] = [
    "checksums",
    "digest",
    "digests",
    "integrity",
    "sha256",
    "url",
    "version",
];

/// Banned names that also count as the last word of a longer attribute name.
const BANNED_SUFFIXES: [&str; 5] = ["checksums", "digest", "digests", "integrity", "sha256"];

/// The keys every `setup.hubs` entry holds.
const ENTRY_KEYS: [&str; 9] = [
    "ecosystem",
    "extension",
    "hub",
    "imports",
    "lock",
    "manifest",
    "resolver",
    "state",
    "tag",
];

/// Fewer snapshot entries than this means the sweep lost the tree.
const MIN_SOURCES: usize = 1000;
const MIN_PATHS: usize = 1000;
const MIN_LANGUAGES: usize = 50;

/// Fewer non-test Rust files than this means the sweep lost the tree.
const MIN_RUST: usize = 300;

/// The declared inputs: BUILD and .bzl texts, other texts, non-test Rust texts, and paths under language directories.
#[derive(Default)]
struct Tree {
    sources: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
    rust: BTreeMap<String, String>,
    lock_scan: BTreeMap<String, String>,
    paths: Vec<String>,
}

/// What the `lang.json` files say about dependency sets.
#[derive(Default)]
struct Languages {
    /// Directories of languages with an ecosystem or a lockfile.
    with_set: BTreeSet<String>,
    /// Ecosystems named by any language.
    ecosystems: BTreeSet<String>,
    /// Ecosystems with a complete `setup.hubs` entry.
    registered: BTreeSet<String>,
    /// Manifest, lock and state names of the registry.
    registry_names: BTreeSet<String>,
    /// `<lang>/lang.json` paired with a manifest or lockfile name the registry lacks.
    stray_names: BTreeSet<(String, String)>,
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn languages(files: &BTreeMap<String, String>) -> Languages {
    let mut out = Languages::default();
    let mut declared = Vec::new();
    for (path, text) in files {
        let Some(dir) = path.strip_suffix("/lang.json") else {
            continue;
        };
        let lang: Value =
            serde_json::from_str(text).unwrap_or_else(|error| panic!("{path}: {error}"));
        let ecosystems = strings(lang.get("ecosystems"));
        let manifests = strings(lang.get("manifests"));
        let lockfiles = strings(lang.get("lockfiles"));
        if !ecosystems.is_empty() || !lockfiles.is_empty() {
            out.with_set.insert(dir.to_owned());
        }
        let hubs = lang.get("setup").and_then(|setup| setup.get("hubs"));
        for entry in hubs.and_then(Value::as_array).into_iter().flatten() {
            let Some(ecosystem) = entry.get("ecosystem").and_then(Value::as_str) else {
                continue;
            };
            if ENTRY_KEYS.iter().all(|key| entry.get(key).is_some()) {
                out.registered.insert(ecosystem.to_owned());
            }
            for key in ["manifest", "lock", "state"] {
                out.registry_names.extend(strings(entry.get(key)));
            }
        }
        if !ecosystems.is_empty() {
            declared.extend(manifests.into_iter().map(|name| (path.clone(), name)));
        }
        declared.extend(lockfiles.into_iter().map(|name| (path.clone(), name)));
        out.ecosystems.extend(ecosystems);
    }
    out.stray_names = declared
        .into_iter()
        .filter(|(_, name)| !out.registry_names.contains(name))
        .collect();
    out
}

/// Whether `path` is test or fixture Starlark.
fn test_path(path: &str) -> bool {
    path.starts_with("examples/")
        || ["/tests/", "/fixtures/", "/testdata/"]
            .iter()
            .any(|part| path.contains(part))
        || ["_test.bzl", "_tests.bzl"]
            .iter()
            .any(|end| path.ends_with(end))
}

/// The first directory of `path`.
fn top_dir(path: &str) -> &str {
    path.split_once('/').map_or("", |(top, _)| top)
}

fn banned_attr(name: &str) -> bool {
    BANNED_ATTRS.contains(&name)
        || BANNED_SUFFIXES.iter().any(|word| {
            name.strip_suffix(word)
                .is_some_and(|rest| rest.ends_with('_'))
        })
}

fn string_literal(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(&text.node),
        _ => None,
    }
}

fn identifier(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Identifier(ident) => Some(&ident.node.ident),
        _ => None,
    }
}

fn parse(path: &str, source: &str) -> AstModule {
    AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()))
}

/// Calls `visit` on every expression under `stmt`.
fn each_expr<'a>(stmt: &'a AstStmt, visit: &mut dyn FnMut(&'a AstExpr)) {
    fn walk<'a>(expr: &'a AstExpr, visit: &mut dyn FnMut(&'a AstExpr)) {
        visit(expr);
        expr.node.visit_expr(|child| walk(child, visit));
    }
    stmt.node.visit_expr(|expr| walk(expr, visit));
    stmt.node.visit_stmt(|child| each_expr(child, visit));
}

/// The top-level assignments of a module, by name.
fn globals(stmt: &AstStmt) -> BTreeMap<String, &AstExpr> {
    let mut out = BTreeMap::new();
    let mut pending = vec![stmt];
    while let Some(current) = pending.pop() {
        match &current.node {
            StmtP::Statements(stmts) => pending.extend(stmts.iter()),
            StmtP::Assign(assign) => {
                if let AssignTargetP::Identifier(ident) = &assign.lhs.node {
                    out.insert(ident.node.ident.clone(), &assign.rhs);
                }
            }
            _ => {}
        }
    }
    out
}

/// The banned attribute names of every `tag_class` in one .bzl file.
fn banned_tag_attrs(path: &str, source: &str) -> BTreeSet<String> {
    let module = parse(path, source);
    let globals = globals(module.statement());
    let mut found = BTreeSet::new();
    each_expr(module.statement(), &mut |expr| {
        let ExprP::Call(callee, args) = &expr.node else {
            return;
        };
        if identifier(callee) != Some("tag_class") {
            return;
        }
        for arg in &args.args {
            let ArgumentP::Named(key, value) = &arg.node else {
                continue;
            };
            if key.node != "attrs" {
                continue;
            }
            let value = identifier(value)
                .and_then(|name| globals.get(name).copied())
                .unwrap_or(value);
            let ExprP::Dict(entries) = &value.node else {
                continue;
            };
            found.extend(
                entries
                    .iter()
                    .filter_map(|(key, _)| string_literal(key))
                    .filter(|name| banned_attr(name))
                    .map(str::to_owned),
            );
        }
    });
    found
}

/// The language directory a root-module extension label is in, if any.
fn extension_dir(label: &str) -> Option<&str> {
    let rest = label
        .strip_prefix("@rules_dx//")
        .or_else(|| label.strip_prefix("//"))?;
    let package = rest.split(':').next().unwrap_or(rest);
    Some(package.split('/').next().unwrap_or(package))
}

/// `<extension>.<tag>.<attribute>` for every banned keyword on a tag of an extension under `dirs`.
fn banned_tag_uses(path: &str, source: &str, dirs: &BTreeSet<String>) -> BTreeSet<String> {
    let module = parse(path, source);
    let mut proxies = BTreeMap::new();
    for (name, value) in globals(module.statement()) {
        let ExprP::Call(callee, args) = &value.node else {
            continue;
        };
        if identifier(callee) != Some("use_extension") {
            continue;
        }
        let positional: Vec<&str> = args
            .args
            .iter()
            .filter_map(|arg| match &arg.node {
                ArgumentP::Positional(value) => string_literal(value),
                _ => None,
            })
            .collect();
        if let [label, extension, ..] = positional.as_slice()
            && extension_dir(label).is_some_and(|dir| dirs.contains(dir))
        {
            proxies.insert(name, (*extension).to_owned());
        }
    }
    let mut found = BTreeSet::new();
    each_expr(module.statement(), &mut |expr| {
        let ExprP::Call(callee, args) = &expr.node else {
            return;
        };
        let ExprP::Dot(proxy, tag) = &callee.node else {
            return;
        };
        let Some(extension) = identifier(proxy).and_then(|name| proxies.get(name)) else {
            return;
        };
        for arg in &args.args {
            if let ArgumentP::Named(key, _) = &arg.node
                && banned_attr(&key.node)
            {
                found.insert(format!("{extension}.{}.{}", tag.node, key.node));
            }
        }
    });
    found
}

/// Whether a .bzl file is a lock parser: named for a lock, or defining a function that parses one.
fn lock_parser(path: &str, source: &str) -> bool {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".bzl");
    if stem == "lock" || stem.ends_with("_lock") {
        return true;
    }
    source
        .lines()
        .filter_map(|line| line.strip_prefix("def "))
        .map(|rest| rest.split('(').next().unwrap_or(rest))
        .any(|name| name.contains("lock") && (name.contains("parse") || name.contains("packages")))
}

/// The file names a text states: runs of letters, digits, `_`, `.` and `-`.
fn stated_names(text: &str) -> BTreeSet<&str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
        .map(|word| word.trim_end_matches('.'))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Whether a line holds a repository label such as `@composer//:monolog`.
fn hub_label(line: &str) -> bool {
    line.split('@').skip(1).any(|rest| {
        let name = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
            .unwrap_or(rest.len());
        name > 0 && rest[name..].starts_with("//")
    })
}

/// Whether the file is generated from the registry, so its names are the registry's own.
fn registry_generated(source: &str) -> bool {
    source.starts_with("\"\"\"Generated from every <lang>/lang.json ")
}

/// One allowlist line per broken rule.
fn violations(tree: &Tree) -> BTreeSet<String> {
    let langs = languages(&tree.files);
    let mut out = BTreeSet::new();
    for ecosystem in langs.ecosystems.difference(&langs.registered) {
        out.insert(format!("registry {ecosystem}"));
    }
    for path in &tree.paths {
        if path.ends_with(".digests.json") {
            out.insert(format!("digests-file {path}"));
        }
    }
    for (path, name) in &langs.stray_names {
        out.insert(format!("name {path} {name}"));
    }
    let names: BTreeSet<&str> = langs
        .registry_names
        .iter()
        .chain(langs.stray_names.iter().map(|(_, name)| name))
        .map(String::as_str)
        .filter(|name| !name.contains('*') && *name != "MODULE.bazel")
        .collect();
    for (path, source) in &tree.sources {
        if !path.ends_with(".bzl") || test_path(path) {
            continue;
        }
        if lock_parser(path, source) {
            out.insert(format!("parser {path}"));
        }
        if !registry_generated(source) {
            for name in stated_names(source).intersection(&names) {
                out.insert(format!("name {path} {name}"));
            }
        }
        if langs.with_set.contains(top_dir(path)) {
            for attr in banned_tag_attrs(path, source) {
                out.insert(format!("tag-attr {path} {attr}"));
            }
        }
    }
    for (path, source) in &tree.rust {
        for name in stated_names(source).intersection(&names) {
            out.insert(format!("name {path} {name}"));
        }
    }
    for (path, source) in &tree.lock_scan {
        if path.ends_with(".go") {
            for name in stated_names(source).intersection(&names) {
                out.insert(format!("name {path} {name}"));
            }
        }
    }
    for (path, source) in tree.files.iter().chain(&tree.lock_scan) {
        if path.ends_with(".go")
            || (path.rsplit('/').next() != Some("MODULE.bazel")
                && !path.ends_with(".fixture")
                && !path.ends_with("module_fragment.bazel"))
        {
            continue;
        }
        for used in banned_tag_uses(path, source, &langs.with_set) {
            out.insert(format!("tag-use {path} {used}"));
        }
    }
    for (path, text) in &tree.files {
        if !path.ends_with(".md") {
            continue;
        }
        let lines = text
            .lines()
            .filter(|line| line.contains("gazelle:resolve") && hub_label(line))
            .count();
        if lines > 0 {
            out.insert(format!("resolve-doc {path} {lines}"));
        }
    }
    out
}

/// What to do about a new line of each rule.
fn fix(line: &str) -> &'static str {
    match line.split(' ').next().unwrap_or("") {
        "registry" => "add its `setup.hubs` entry with every key",
        "digests-file" => "delete the file; the lock holds the digests",
        "name" => "read the name from the registry",
        "parser" => "read the lock with `lock_hub`",
        "tag-attr" => "remove the attribute; the lock holds the value",
        "tag-use" => "remove the argument; the lock holds the value",
        "resolve-doc" => "delete the line; the lock resolves the import",
        _ => "not a rule of this test",
    }
}

/// The allowlist lines and what is wrong with the file itself.
fn parse_allowlist(text: &str) -> (BTreeSet<String>, Vec<String>) {
    let mut lines = BTreeSet::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        if line.trim() != line || line.is_empty() || fix(line) == fix("") {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        lines.insert(line.to_owned());
    }
    (lines, problems)
}

/// Lines that differ between the tree and the allowlist.
fn differences(found: &BTreeSet<String>, allowed: &BTreeSet<String>) -> Vec<String> {
    let new = found
        .difference(allowed)
        .map(|line| format!("new: {line}; {}", fix(line)));
    let stale = allowed
        .difference(found)
        .map(|line| format!("stale: {line}; delete the allowlist line"));
    new.chain(stale).collect()
}

/// Reads one runfile named by an environment variable.
fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

fn decode_texts(variable: &str) -> BTreeMap<String, String> {
    serde_json::from_str(&read_input(variable))
        .unwrap_or_else(|error| panic!("{variable}: {error}"))
}

fn decode_paths(variable: &str) -> Vec<String> {
    serde_json::from_str(&read_input(variable))
        .unwrap_or_else(|error| panic!("{variable}: {error}"))
}

fn tree(sources: &[(&str, &str)], files: &[(&str, &str)], paths: &[&str]) -> Tree {
    tree_with_rust(sources, files, &[], paths)
}

fn tree_with_rust(
    sources: &[(&str, &str)],
    files: &[(&str, &str)],
    rust: &[(&str, &str)],
    paths: &[&str],
) -> Tree {
    let map = |entries: &[(&str, &str)]| {
        entries
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect()
    };
    Tree {
        sources: map(sources),
        files: map(files),
        rust: map(rust),
        lock_scan: BTreeMap::new(),
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
    }
}

fn found(tree: &Tree) -> Vec<String> {
    violations(tree).into_iter().collect()
}

const PHP_LANG: &str = r#"{"name": "php", "ecosystems": ["composer"], "manifests": ["composer.json"], "lockfiles": ["composer.lock.json"], "setup": {"hubs": [{"ecosystem": "composer", "manifest": "composer.json", "lock": "composer.lock.json", "state": "composer.lock", "extension": "@rules_dx//php:extensions.bzl", "tag": "lock", "hub": "composer", "resolver": "@rules_dx//php:lock", "imports": "provides"}]}}"#;

const LOCK_TAG: &str = r#"
_lock = tag_class(attrs = {"name": attr.string(), "lock": attr.label()})
composer = module_extension(implementation = _impl, tag_classes = {"lock": _lock})
"#;

#[test]
fn the_tree_matches_the_allowlist() {
    let tree = Tree {
        sources: decode_texts("DX_SOURCES_JSON"),
        files: decode_texts("DX_FILES_JSON"),
        rust: decode_texts("DX_RUST_JSON"),
        lock_scan: decode_texts("DX_LOCK_SCAN_JSON"),
        paths: decode_paths("DX_PATHS_JSON"),
    };
    assert!(
        tree.rust.len() >= MIN_RUST,
        "only {} non-test .rs files in the rust snapshot",
        tree.rust.len()
    );
    assert!(
        tree.sources.len() >= MIN_SOURCES,
        "only {} BUILD and .bzl files in the sources snapshot",
        tree.sources.len()
    );
    assert!(
        tree.paths.len() >= MIN_PATHS,
        "only {} paths in the paths snapshot",
        tree.paths.len()
    );
    let languages = tree
        .files
        .keys()
        .filter(|path| path.ends_with("/lang.json"))
        .count();
    assert!(
        languages >= MIN_LANGUAGES,
        "only {languages} lang.json files in the files snapshot"
    );
    let (allowed, mut problems) = parse_allowlist(&read_input("LOCK_REGISTRY_ALLOWLIST"));
    problems.extend(differences(&violations(&tree), &allowed));
    assert!(
        problems.is_empty(),
        "{ALLOWLIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
}

#[test]
fn a_registered_ecosystem_with_a_lock_tag_breaks_no_rule() {
    let tree = tree(
        &[("php/private/composer/extensions.bzl", LOCK_TAG)],
        &[
            ("php/lang.json", PHP_LANG),
            (
                "MODULE.bazel",
                "composer = use_extension(\"//php:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"composer\", lock = \"//:composer.lock.json\")\n",
            ),
            (
                "php/README.md",
                "Run `dx generate --apply`.\n`# gazelle:resolve` is not needed.\n",
            ),
        ],
        &["php/tests/fixtures/composer/composer.lock.json"],
    );
    assert_eq!(found(&tree), Vec::<String>::new());
}

#[test]
fn an_ecosystem_without_a_complete_registry_entry_fails() {
    let missing = r#"{"ecosystems": ["dub"], "manifests": [], "lockfiles": []}"#;
    let partial = r#"{"ecosystems": ["zig"], "manifests": [], "lockfiles": [], "setup": {"hubs": [{"ecosystem": "zig", "hub": "zig"}]}}"#;
    let tree = tree(
        &[],
        &[("d/lang.json", missing), ("zig/lang.json", partial)],
        &[],
    );
    assert_eq!(found(&tree), ["registry dub", "registry zig"]);
}

#[test]
fn a_new_digests_file_fails() {
    let tree = tree(
        &[],
        &[("php/lang.json", PHP_LANG)],
        &["php/tests/fixtures/app/x.digests.json"],
    );
    assert_eq!(
        found(&tree),
        ["digests-file php/tests/fixtures/app/x.digests.json"]
    );
}

#[test]
fn a_new_sha256_tag_attribute_fails() {
    let inline = r#"
_package = tag_class(attrs = {"name": attr.string(), "sha256": attr.string()})
"#;
    let named = r#"
_ATTRS = {
    "lock": attr.label(),
    "gem_checksums": attr.string_dict(),
    "version": attr.string(),
}
_lock = tag_class(attrs = _ATTRS)
"#;
    let rule = r#"
_repo = repository_rule(implementation = _impl, attrs = {"sha256": attr.string()})
"#;
    let tree = tree(
        &[
            ("php/private/composer/extensions.bzl", inline),
            ("php/private/composer/more.bzl", named),
            ("php/private/composer/repos.bzl", rule),
            ("qt/private/sdk/extensions.bzl", inline),
        ],
        &[
            ("php/lang.json", PHP_LANG),
            (
                "qt/lang.json",
                r#"{"ecosystems": [], "manifests": [], "lockfiles": []}"#,
            ),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "tag-attr php/private/composer/extensions.bzl sha256",
            "tag-attr php/private/composer/more.bzl gem_checksums",
            "tag-attr php/private/composer/more.bzl version",
        ]
    );
}

#[test]
fn a_banned_argument_on_a_root_module_tag_fails() {
    let module = r#"
composer = use_extension("//php:extensions.bzl", "composer", dev_dependency = True)
composer.lock(name = "composer", lock = "//:composer.lock.json", digests = "//:composer.digests.json")
composer.package(name = "monolog", gem_checksums = {}, url = "https://example.invalid")
go_sdk = use_extension("@rules_go//go:extensions.bzl", "go_sdk")
go_sdk.download(version = "1.25.0")
qt = use_extension("//qt:extensions.bzl", "qt")
qt.configure(version = "6.9")
"#;
    let tree = tree(
        &[],
        &[
            ("php/lang.json", PHP_LANG),
            (
                "qt/lang.json",
                r#"{"ecosystems": [], "manifests": [], "lockfiles": []}"#,
            ),
            ("MODULE.bazel", module),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "tag-use MODULE.bazel composer.lock.digests",
            "tag-use MODULE.bazel composer.package.gem_checksums",
            "tag-use MODULE.bazel composer.package.url",
        ]
    );
}

#[test]
fn a_banned_argument_on_a_nested_module_tag_fails() {
    let module = r#"
composer = use_extension("@rules_dx//php:extensions.bzl", "composer")
composer.lock(name = "composer", lock = "//:composer.lock.json", sha256 = "00")
npm = use_extension("@aspect_rules_js//npm:extensions.bzl", "npm")
npm.npm_translate_lock(name = "npm", integrity = "sha512-x")
"#;
    let tree = tree(
        &[],
        &[
            ("php/lang.json", PHP_LANG),
            ("php/tests/fixtures/composer/MODULE.bazel", module),
            ("examples/php/MODULE.bazel", module),
            ("php/tests/fixtures/composer/extra.MODULE.bazel", module),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "tag-use examples/php/MODULE.bazel composer.lock.sha256",
            "tag-use php/tests/fixtures/composer/MODULE.bazel composer.lock.sha256",
        ]
    );
}

#[test]
fn a_starlark_lock_parser_fails() {
    let tree = tree(
        &[
            (
                "elixir/private/mix_lock.bzl",
                "def packages(text):\n    return []\n",
            ),
            ("php/private/composer/lock.bzl", "X = 1\n"),
            (
                "python/private/vendor.bzl",
                "def parse_cargo_lock(text):\n    return []\n",
            ),
            (
                "erlang/private/hex.bzl",
                "def rebar_lock_packages(text):\n    return []\n",
            ),
            ("php/private/composer/lock_tests.bzl", "X = 1\n"),
            ("php/tests/lock.bzl", "X = 1\n"),
            (
                "core/pins.bzl",
                "def lock_hub(build_file):\n    return None\n",
            ),
        ],
        &[],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "parser elixir/private/mix_lock.bzl",
            "parser erlang/private/hex.bzl",
            "parser php/private/composer/lock.bzl",
            "parser python/private/vendor.bzl",
        ]
    );
}

#[test]
fn a_manifest_or_lock_name_outside_the_registry_fails() {
    let ruby = r#"{"ecosystems": ["ruby"], "manifests": ["Gemfile", "MODULE.bazel", "*.gemspec"], "lockfiles": ["Gemfile.lock"]}"#;
    let helm = r#"{"ecosystems": [], "manifests": ["Chart.yaml"], "lockfiles": ["Chart.lock"]}"#;
    let tree = tree(
        &[
            (
                "php/private/php.bzl",
                "_LOCK = \"composer.lock\"\n# reads //:composer.json.\nMODULE = \"MODULE.bazel\"\n",
            ),
            (
                "ruby/private/gems.bzl",
                "X = \"Gemfile.lock\"\nY = \"Gemfile.locked\"\n",
            ),
            ("ruby/tests/gems.bzl", "X = \"Gemfile\"\n"),
            ("ruby/BUILD.bazel", "exports_files([\"Gemfile\"])\n"),
        ],
        &[
            ("php/lang.json", PHP_LANG),
            ("ruby/lang.json", ruby),
            ("helm/lang.json", helm),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "name helm/lang.json Chart.lock",
            "name php/private/php.bzl composer.json",
            "name php/private/php.bzl composer.lock",
            "name ruby/lang.json *.gemspec",
            "name ruby/lang.json Gemfile",
            "name ruby/lang.json Gemfile.lock",
            "name ruby/lang.json MODULE.bazel",
            "name ruby/private/gems.bzl Gemfile.lock",
            "registry ruby",
        ]
    );
}

#[test]
fn a_registry_name_in_a_file_generated_from_the_registry_passes() {
    let tree = tree(
        &[
            ("core/modules/langs.bzl", "\"\"\"Generated from every <lang>/lang.json by a test.\"\"\"\nX = \"composer.lock\"\n"),
            ("php/private/php.bzl", "\"\"\"Hand written.\"\"\"\nX = \"composer.lock\"\n"),
        ],
        &[("php/lang.json", PHP_LANG)],
        &[],
    );
    assert!(found(&tree).iter().all(|line| !line.contains("langs.bzl")), "{:?}", found(&tree));
    assert!(found(&tree).contains(&"name php/private/php.bzl composer.lock".to_owned()));
}

#[test]
fn a_registry_name_in_go_source_or_a_banned_attr_in_a_fragment_fails() {
    let mut tree = tree(&[], &[("php/lang.json", PHP_LANG)], &[]);
    tree.lock_scan.insert(
        "gazelle/php/lock.go".to_owned(),
        "var lock = \"composer.lock.json\"\n".to_owned(),
    );
    tree.lock_scan.insert(
        "tools/consumers/php/module_fragment.bazel".to_owned(),
        "composer = use_extension(\"//php:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"composer\", sha256 = \"x\")\n".to_owned(),
    );
    assert_eq!(
        found(&tree),
        [
            "name gazelle/php/lock.go composer.lock.json",
            "tag-use tools/consumers/php/module_fragment.bazel composer.lock.sha256",
        ]
    );
}

#[test]
fn a_registry_name_in_rust_source_fails() {
    let tree = tree_with_rust(
        &[],
        &[("php/lang.json", PHP_LANG)],
        &[
            (
                "cli/adopt/src/php.rs",
                "const LOCK: &str = \"composer.lock\";\n// Reads composer.json.\nconst OTHER: &str = \"composer.locked\";\n",
            ),
            (
                "gazelle/php/lock.rs",
                "fn lock(dir: &Path) -> PathBuf {\n    dir.join(\"composer.lock.json\")\n}\n",
            ),
            (
                "cli/plan/src/lib.rs",
                "const MODULE: &str = \"MODULE.bazel\";\n",
            ),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "name cli/adopt/src/php.rs composer.json",
            "name cli/adopt/src/php.rs composer.lock",
            "name gazelle/php/lock.rs composer.lock.json",
        ]
    );
}

#[test]
fn a_doc_line_pairing_gazelle_resolve_with_a_hub_label_fails() {
    let readme = "# gazelle:resolve php Monolog\\\\Logger @composer//:monolog_monolog\n\
        Use `# gazelle:resolve` for first-party code.\n\
        `# gazelle:resolve lua lpeg @rocks//lpeg`\n\
        Depend on `@composer//:monolog_monolog`.\n";
    let tree = tree(
        &[],
        &[
            ("php/lang.json", PHP_LANG),
            ("php/README.md", readme),
            (
                "docs/languages/php.md",
                "`gazelle:resolve_regexp x @hex//:jason`\n",
            ),
        ],
        &[],
    );
    assert_eq!(
        found(&tree),
        [
            "resolve-doc docs/languages/php.md 1",
            "resolve-doc php/README.md 2",
        ]
    );
}

#[test]
fn a_new_violation_and_a_stale_allowlist_line_fail() {
    let lines = |entries: &[&str]| -> BTreeSet<String> {
        entries.iter().map(|line| (*line).to_owned()).collect()
    };
    let allowed = lines(&["digests-file d/dub.digests.json", "registry dub"]);
    assert_eq!(differences(&allowed, &allowed), Vec::<String>::new());
    assert_eq!(
        differences(
            &lines(&[
                "digests-file d/dub.digests.json",
                "registry dub",
                "tag-attr d/extensions.bzl sha256"
            ]),
            &allowed
        ),
        ["new: tag-attr d/extensions.bzl sha256; remove the attribute; the lock holds the value"]
    );
    assert_eq!(
        differences(&lines(&["registry dub"]), &allowed),
        ["stale: digests-file d/dub.digests.json; delete the allowlist line"]
    );
    assert_eq!(
        differences(
            &lines(&["resolve-doc php/README.md 1"]),
            &lines(&["resolve-doc php/README.md 2"])
        ),
        [
            "new: resolve-doc php/README.md 1; delete the line; the lock resolves the import",
            "stale: resolve-doc php/README.md 2; delete the allowlist line",
        ]
    );
}

#[test]
fn the_allowlist_is_sorted_lines_of_known_rules() {
    let (lines, problems) = parse_allowlist("digests-file a/x.digests.json\nregistry dub\n");
    assert_eq!(lines.len(), 2);
    assert_eq!(problems, Vec::<String>::new());
    let (_, problems) = parse_allowlist(
        "registry zig\nregistry dub\nregistry dub\n# note\n\nother x\n registry a\n",
    );
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"registry dub\"",
            "not sorted or repeated: \"registry dub\"",
            "malformed line: \"# note\"",
            "malformed line: \"\"",
            "malformed line: \"other x\"",
            "malformed line: \" registry a\"",
        ]
    );
}
