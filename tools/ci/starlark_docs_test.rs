//! Every non-test rule(), aspect() and public attr in rules_dx has a one-line doc.

use std::path::{Path, PathBuf};

use dx_testing::runfiles_root;
use starlark_syntax::syntax::ast::{
    ArgumentP, AstArgument, AstExpr, AstLiteral, AstStmt, ExprP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};

/// Callees whose calls must pass doc when they pass implementation.
const DOCUMENTED_CALLS: [&str; 2] = ["rule", "aspect"];

/// Path segments whose .bzl files are test support, not public rules.
const SKIPPED_SEGMENTS: [&str; 4] = ["tests", "testdata", "fixtures", "third_party"];

/// Packages whose :corpus_starlark is in the test data.
const PACKAGES: [&str; 72] = [
    "appimage",
    "astro/private",
    "bicep/rules",
    "cc/rules",
    "cc/toolchains",
    "cli/entrypoint",
    "cli/roots",
    "config",
    "deploy/offline",
    "deploy/release",
    "deploy/rules",
    "dockerfile/rules",
    "elixir/rules",
    "elixir/toolchain",
    "elixir/tools",
    "env",
    "erlang/rules",
    "erlang/toolchain",
    "examples/shared-rust",
    "generation",
    "go/rules",
    "java/rules",
    "javascript/private",
    "kotlin/rules",
    "libs/starlark",
    "libs/testing",
    "lua/private",
    "dart/rules",
    "lua/rules",
    "lua/toolchain",
    "mdx/private",
    "modules",
    "ocaml/rules",
    "ocaml/toolchain",
    "perl/cpan",
    "perl/rules",
    "powershell/rules",
    "python/rules",
    "qt/rules",
    "qt/sdk",
    "quality",
    "quality/artifacts",
    "quality/artifacts/acquire_test",
    "quality/artifacts/hub_test",
    "quality/rules",
    "quality/tools/javascript",
    "quality/tools/jvm",
    "quality/tools/ruby",
    "ruby/rules",
    "rust/rules",
    "rust/toolchains",
    "scala/rules",
    "sql/rules",
    "svelte/private",
    "terraform/rules",
    "tools/docs/go",
    "tools/docs/ir",
    "tools/docs/javascript",
    "tools/docs/rust",
    "tools/docs/site",
    "tools/docs/site/browser",
    "tools/docs/typescript",
    "tools/ide",
    "tools/testing",
    "tools/testing/hermetic",
    "tools/visibility",
    "julia/rules",
    "julia/tools",
    "typescript/private",
    "vue/private",
    "zig/rules",
    "zig/tools",
];

/// Set by a family workspace to the comma-separated packages it checks; the size floors then do not apply.
const PACKAGES_ENV: &str = "STARLARK_DOCS_PACKAGES";

/// The packages to check: those the caller names, else the root list.
fn packages() -> Vec<String> {
    match std::env::var(PACKAGES_ENV) {
        Ok(list) => list.split(',').map(str::to_owned).collect(),
        Err(_) => PACKAGES.iter().map(|pkg| (*pkg).to_owned()).collect(),
    }
}

/// Fewer non-test rules and aspects than this means the sweep lost its data.
const MIN_CALLS: usize = 100;

/// Fewer public attrs than this means the sweep lost its data.
const MIN_ATTRS: usize = 550;

/// What one file contributes: checked calls and their findings.
#[derive(Default)]
struct Scan {
    calls: usize,
    attrs: usize,
    findings: Vec<String>,
}

/// Whether `path` is test support that the check skips.
fn skipped(path: &str) -> bool {
    path.ends_with("_tests.bzl")
        || path
            .split('/')
            .any(|segment| SKIPPED_SEGMENTS.contains(&segment))
}

/// The rule() and aspect() calls and public attrs in `source` that lack a usable doc, as `path:line: ...`.
fn findings(path: &str, source: &str) -> Vec<String> {
    scan(path, source).findings
}

fn scan(path: &str, source: &str) -> Scan {
    let mut out = Scan::default();
    if skipped(path) {
        return out;
    }
    let ast = match AstModule::parse(path, source.to_owned(), &Dialect::Extended) {
        Ok(ast) => ast,
        Err(error) => {
            out.findings.push(format!(
                "{path}: does not parse: {}",
                error.without_diagnostic()
            ));
            return out;
        }
    };
    ast.statement()
        .node
        .visit_expr(|expr| walk(&ast, path, expr, &mut out));
    check_defs(&ast, path, ast.statement(), &mut out);
    out
}

/// Reports every def whose docstring spans more than one line.
fn check_defs(ast: &AstModule, path: &str, stmt: &AstStmt, out: &mut Scan) {
    if let StmtP::Def(def) = &stmt.node {
        let first = match &def.body.node {
            StmtP::Statements(stmts) => stmts.first(),
            _ => Some(&*def.body),
        };
        if let Some(StmtP::Expression(expr)) = first.map(|s| &s.node) {
            if let ExprP::Literal(AstLiteral::String(text)) = &expr.node {
                if text.node.trim().contains('\n') {
                    out.findings.push(format!(
                        "{path}:{}: def {} has a multi-line docstring; use one plain line",
                        line_of(ast, expr),
                        def.name.node.ident
                    ));
                }
            }
        }
    }
    stmt.node
        .visit_stmt(|child| check_defs(ast, path, child, out));
}

fn walk(ast: &AstModule, path: &str, expr: &AstExpr, out: &mut Scan) {
    if !check_call(ast, path, expr, out) {
        return;
    }
    check_attr_dict(ast, path, expr, out);
    expr.node.visit_expr(|child| walk(ast, path, child, out));
}

/// Checks a rule() or aspect() call; false when it is a test rule whose inline attrs are exempt.
fn check_call(ast: &AstModule, path: &str, expr: &AstExpr, out: &mut Scan) -> bool {
    let ExprP::Call(callee, args) = &expr.node else {
        return true;
    };
    let ExprP::Identifier(ident) = &callee.node else {
        return true;
    };
    let name = ident.node.ident.as_str();
    if !DOCUMENTED_CALLS.contains(&name) {
        return true;
    }
    let named = |key: &str| named_arg(&args.args, key);
    if named("implementation").is_none() {
        return true;
    }
    if named("test").is_some_and(
        |value| matches!(&value.node, ExprP::Identifier(id) if id.node.ident == "True"),
    ) {
        return false;
    }
    out.calls += 1;
    if let Some(problem) = doc_problem(named("doc")) {
        let line = line_of(ast, expr);
        out.findings.push(format!(
            "{path}:{line}: {name}() {problem}; add doc = \"<one plain line>\""
        ));
    }
    true
}

/// Checks each public `"key": attr.x(...)` entry of a dict literal for a doc.
fn check_attr_dict(ast: &AstModule, path: &str, expr: &AstExpr, out: &mut Scan) {
    let ExprP::Dict(entries) = &expr.node else {
        return;
    };
    let tool_descriptor = ["kind", "shard"]
        .iter()
        .all(|wanted| entries.iter().any(|(key, _)| is_string(key, wanted)));
    for (key, value) in entries {
        let ExprP::Literal(AstLiteral::String(key)) = &key.node else {
            continue;
        };
        if key.node.starts_with('_') || (tool_descriptor && key.node == "attr") {
            continue;
        }
        let ExprP::Call(callee, args) = &value.node else {
            continue;
        };
        let ExprP::Dot(object, _) = &callee.node else {
            continue;
        };
        if !matches!(&object.node, ExprP::Identifier(id) if id.node.ident == "attr") {
            continue;
        }
        out.attrs += 1;
        if let Some(problem) = doc_problem(named_arg(&args.args, "doc")) {
            let line = line_of(ast, value);
            out.findings.push(format!(
                "{path}:{line}: attr \"{}\" {problem}; add doc = \"<one plain line>\"",
                key.node
            ));
        }
    }
}

fn is_string(expr: &AstExpr, wanted: &str) -> bool {
    matches!(&expr.node, ExprP::Literal(AstLiteral::String(text)) if text.node == wanted)
}

/// The value of the named argument `key`, if passed.
fn named_arg<'a>(args: &'a [AstArgument], key: &str) -> Option<&'a AstExpr> {
    args.iter().find_map(|arg| match &arg.node {
        ArgumentP::Named(arg_key, value) if arg_key.node == key => Some(value),
        _ => None,
    })
}

/// Why `doc` is not a usable one-line doc, or None when it is (computed docs count).
fn doc_problem(doc: Option<&AstExpr>) -> Option<&'static str> {
    match doc.map(|value| &value.node) {
        None => Some("has no doc"),
        Some(ExprP::Literal(AstLiteral::String(text))) if text.node.trim().is_empty() => {
            Some("has an empty doc")
        }
        Some(ExprP::Literal(AstLiteral::String(text))) if text.node.trim().contains('\n') => {
            Some("has a multi-line doc")
        }
        Some(_) => None,
    }
}

/// The 1-based line where `expr` starts.
fn line_of(ast: &AstModule, expr: &AstExpr) -> usize {
    ast.file_span(expr.span).resolve_span().begin.line + 1
}

/// Every .bzl file under `dir`, relative to the runfiles root.
fn bzl_files(dir: &Path) -> Vec<String> {
    let root = runfiles_root();
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .unwrap_or_else(|error| panic!("read_dir {}: {error}", current.display()));
        for entry in entries {
            let path = entry.expect("readable dir entry").path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "bzl") {
                continue;
            }
            let rel = path.strip_prefix(&root).expect("path under the workspace");
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    out.sort();
    out
}

/// The listed package that owns `path`, the longest matching prefix.
fn owner<'a>(packages: &'a [String], path: &str) -> Option<&'a str> {
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    packages
        .iter()
        .map(String::as_str)
        .filter(|pkg| dir == *pkg || dir.starts_with(&format!("{pkg}/")))
        .max_by_key(|pkg| pkg.len())
}

/// Listed packages with no .bzl file and no BUILD file in the runfiles.
fn missing_packages(
    packages: &[String],
    seen: &std::collections::BTreeSet<&str>,
    root: &Path,
) -> Vec<String> {
    packages
        .iter()
        .filter(|pkg| !seen.contains(pkg.as_str()))
        .filter(|pkg| {
            let dir = root.join(pkg);
            !dir.join("BUILD.bazel").is_file() && !dir.join("BUILD").is_file()
        })
        .cloned()
        .collect()
}

#[test]
fn every_rule_aspect_and_attr_has_doc() {
    let root = runfiles_root();
    let mut calls = 0;
    let mut attrs = 0;
    let mut problems = Vec::new();
    let packages = packages();
    let mut seen = std::collections::BTreeSet::new();
    for path in bzl_files(&root) {
        if skipped(&path) {
            continue;
        }
        if let Some(pkg) = owner(&packages, &path) {
            seen.insert(pkg);
        }
        let source = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|error| panic!("read {path}: {error}"));
        let result = scan(&path, &source);
        calls += result.calls;
        attrs += result.attrs;
        problems.extend(result.findings);
    }
    let missing = missing_packages(&packages, &seen, &root);
    assert!(missing.is_empty(), "no .bzl files from {missing:?}");
    if std::env::var(PACKAGES_ENV).is_err() {
        assert!(
            calls > MIN_CALLS,
            "only {calls} rules and aspects were checked"
        );
        assert!(attrs > MIN_ATTRS, "only {attrs} public attrs were checked");
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_package_with_only_a_build_file_is_not_missing() {
    let root = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR"))
        .join("missing_packages");
    std::fs::create_dir_all(root.join("tests")).expect("create tests");
    std::fs::write(root.join("tests/BUILD.bazel"), "").expect("write BUILD.bazel");
    let packages = ["tests".to_string(), "absent".to_string()];
    let seen = std::collections::BTreeSet::new();
    assert_eq!(missing_packages(&packages, &seen, &root), ["absent"]);
}

#[test]
fn a_rule_without_doc_is_reported() {
    let found = findings(
        "pkg/defs.bzl",
        "x = rule(\n    implementation = _impl,\n)\n",
    );
    assert_eq!(
        found,
        ["pkg/defs.bzl:1: rule() has no doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn an_aspect_without_doc_is_reported() {
    let found = findings("pkg/defs.bzl", "\n\nx = aspect(implementation = _impl)\n");
    assert_eq!(
        found,
        ["pkg/defs.bzl:3: aspect() has no doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_test_rule_is_exempt() {
    assert!(
        findings(
            "pkg/defs.bzl",
            "x = rule(implementation = _impl, test = True)\n"
        )
        .is_empty()
    );
}

#[test]
fn the_attr_key_is_exempt_only_in_a_tool_descriptor() {
    let descriptor =
        "T = {\"x\": {\"attr\": attr.label(), \"kind\": \"file\", \"shard\": \"py\"}}\n";
    assert!(findings("pkg/defs.bzl", descriptor).is_empty());
    let plain = "T = {\"attr\": attr.label()}\n";
    assert_eq!(findings("pkg/defs.bzl", plain).len(), 1);
}

#[test]
fn a_call_without_implementation_is_ignored() {
    assert!(findings("pkg/defs.bzl", "def f(rule):\n    rule(name = \"x\")\n").is_empty());
}

#[test]
fn a_multi_line_doc_is_reported() {
    let source = "x = rule(implementation = _impl, doc = \"\"\"One.\n\nTwo.\"\"\")\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:1: rule() has a multi-line doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_multi_line_def_docstring_is_reported() {
    let source = "def f():\n    \"\"\"One.\n\n    Two.\n    \"\"\"\n    return 1\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:2: def f has a multi-line docstring; use one plain line"]
    );
}

#[test]
fn a_one_line_def_docstring_is_accepted() {
    let source = "def f():\n    \"\"\"One.\"\"\"\n    return 1\n";
    assert!(findings("pkg/defs.bzl", source).is_empty());
}

#[test]
fn an_empty_doc_is_reported() {
    let source = "x = rule(implementation = _impl, doc = \" \")\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:1: rule() has an empty doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_computed_doc_counts() {
    let source =
        "def f(what):\n    return rule(implementation = _impl, doc = \"Builds \" + what)\n";
    assert!(findings("pkg/defs.bzl", source).is_empty());
}

#[test]
fn a_tests_path_is_skipped() {
    let source = "x = rule(implementation = _impl)\n";
    assert!(findings("pkg/tests/defs.bzl", source).is_empty());
    assert!(findings("pkg/defs_tests.bzl", source).is_empty());
    assert!(findings("third_party/x/defs.bzl", source).is_empty());
}

#[test]
fn an_attr_without_doc_is_reported() {
    let source = "x = rule(\n    implementation = _impl,\n    doc = \"X.\",\n    attrs = {\n        \"srcs\": attr.label_list(),\n    },\n)\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:5: attr \"srcs\" has no doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_private_attr_is_exempt() {
    let source =
        "x = rule(implementation = _impl, doc = \"X.\", attrs = {\"_tool\": attr.label()})\n";
    assert!(findings("pkg/defs.bzl", source).is_empty());
}

#[test]
fn a_test_rule_inline_attr_is_exempt() {
    let source = "x = rule(implementation = _i, test = True, attrs = {\"x\": attr.label()})\n";
    assert!(findings("pkg/defs.bzl", source).is_empty());
}

#[test]
fn an_attr_in_a_shared_dict_is_reported() {
    let source = "_COMMON = {\n    \"deps\": attr.label_list(),\n}\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:2: attr \"deps\" has no doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_multi_line_attr_doc_is_reported() {
    let source = "_A = {\"x\": attr.string(doc = \"\"\"One.\n\nTwo.\"\"\")}\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:1: attr \"x\" has a multi-line doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn an_attr_with_kwargs_still_needs_doc() {
    let source = "def f(**kw):\n    return {\"upstream\": attr.label(**kw)}\n";
    assert_eq!(
        findings("pkg/defs.bzl", source),
        ["pkg/defs.bzl:2: attr \"upstream\" has no doc; add doc = \"<one plain line>\""]
    );
}

#[test]
fn a_non_attr_dict_value_is_ignored() {
    let source = "_A = {\"x\": \"s\", \"y\": foo.label()}\n";
    assert!(findings("pkg/defs.bzl", source).is_empty());
}
