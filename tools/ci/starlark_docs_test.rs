//! Every non-test rule() and aspect() in rules_dx has a one-line doc.

use std::path::Path;

use dx_testing::runfiles_root;
use starlark_syntax::syntax::ast::{ArgumentP, AstExpr, AstLiteral, ExprP};
use starlark_syntax::syntax::{AstModule, Dialect};

/// Callees whose calls must pass doc when they pass implementation.
const DOCUMENTED_CALLS: [&str; 2] = ["rule", "aspect"];

/// Path segments whose .bzl files are test support, not public rules.
const SKIPPED_SEGMENTS: [&str; 4] = ["tests", "testdata", "fixtures", "third_party"];

/// Packages whose :corpus_starlark is in the test data.
const PACKAGES: [&str; 94] = [
    "android/packaging",
    "android/platforms",
    "android/rules",
    "android/sdk",
    "android/toolchain",
    "appimage",
    "apple/bundle",
    "apple/ios",
    "apple/provisioning",
    "apple/signing",
    "astro/rules",
    "cc/rules",
    "cc/toolchains",
    "cli/entrypoint",
    "cli/roots",
    "config",
    "csharp/rules",
    "cuda/rules",
    "cuda/toolchain",
    "cue/rules",
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
    "fsharp/rules",
    "generation",
    "gherkin/rules",
    "gleam/rules",
    "gleam/toolchain",
    "gleam/tools",
    "go/rules",
    "html_template/rules",
    "java/rules",
    "javascript/rules",
    "jsonnet/rules",
    "kotlin/rules",
    "libs/starlark",
    "libs/testing",
    "lua/private",
    "lua/rules",
    "lua/toolchain",
    "mdx/rules",
    "modules",
    "msix",
    "ocaml/rules",
    "ocaml/toolchain",
    "perl/cpan",
    "perl/rules",
    "pkl/rules",
    "powershell/rules",
    "python/rules",
    "qt/rules",
    "qt/sdk",
    "quality",
    "quality/artifacts",
    "quality/artifacts/acquire_test",
    "quality/artifacts/hub_test",
    "quality/tools/cc",
    "quality/tools/dotnet/fsharplint",
    "quality/tools/javascript",
    "quality/tools/jvm",
    "quality/tools/ruby",
    "ruby/rules",
    "rust/rules",
    "rust/toolchains",
    "scala/rules",
    "sql/rules",
    "svelte/rules",
    "terraform/rules",
    "tools/docs/go",
    "tools/docs/ir",
    "tools/docs/rust",
    "tools/docs/site",
    "tools/docs/site/browser",
    "tools/docs/site/mdbook",
    "tools/docs/typescript",
    "tools/ide",
    "tools/testing",
    "tools/testing/hermetic",
    "tools/visibility",
    "typescript/rules",
    "vue/rules",
    "wasi/runtime",
    "xml/rules",
    "yaml/rules",
    "zig/rules",
    "zig/tools",
];

/// Fewer non-test rules and aspects than this means the sweep lost its data.
const MIN_CALLS: usize = 100;

/// What one file contributes: checked calls and their findings.
#[derive(Default)]
struct Scan {
    calls: usize,
    findings: Vec<String>,
}

/// Whether `path` is test support that the check skips.
fn skipped(path: &str) -> bool {
    path.ends_with("_tests.bzl")
        || path
            .split('/')
            .any(|segment| SKIPPED_SEGMENTS.contains(&segment))
}

/// The rule() and aspect() calls in `source` that lack a usable doc, as `path:line: ...`.
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
    out
}

fn walk(ast: &AstModule, path: &str, expr: &AstExpr, out: &mut Scan) {
    check_call(ast, path, expr, out);
    expr.node.visit_expr(|child| walk(ast, path, child, out));
}

fn check_call(ast: &AstModule, path: &str, expr: &AstExpr, out: &mut Scan) {
    let ExprP::Call(callee, args) = &expr.node else {
        return;
    };
    let ExprP::Identifier(ident) = &callee.node else {
        return;
    };
    let name = ident.node.ident.as_str();
    if !DOCUMENTED_CALLS.contains(&name) {
        return;
    }
    let named = |key: &str| {
        args.args.iter().find_map(|arg| match &arg.node {
            ArgumentP::Named(arg_key, value) if arg_key.node == key => Some(value),
            _ => None,
        })
    };
    if named("implementation").is_none() {
        return;
    }
    if named("test").is_some_and(
        |value| matches!(&value.node, ExprP::Identifier(id) if id.node.ident == "True"),
    ) {
        return;
    }
    out.calls += 1;
    let line = ast.file_span(expr.span).resolve_span().begin.line + 1;
    let problem = match named("doc").map(|value| &value.node) {
        None => "has no doc",
        Some(ExprP::Literal(AstLiteral::String(text))) if text.node.trim().is_empty() => {
            "has an empty doc"
        }
        Some(ExprP::Literal(AstLiteral::String(text))) if text.node.trim().contains('\n') => {
            "has a multi-line doc"
        }
        Some(_) => return,
    };
    out.findings.push(format!(
        "{path}:{line}: {name}() {problem}; add doc = \"<one plain line>\""
    ));
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
fn owner(path: &str) -> Option<&'static str> {
    let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
    PACKAGES
        .iter()
        .copied()
        .filter(|pkg| dir == *pkg || dir.starts_with(&format!("{pkg}/")))
        .max_by_key(|pkg| pkg.len())
}

#[test]
fn every_rule_and_aspect_has_doc() {
    let root = runfiles_root();
    let mut calls = 0;
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for path in bzl_files(&root) {
        if skipped(&path) {
            continue;
        }
        if let Some(pkg) = owner(&path) {
            seen.insert(pkg);
        }
        let source = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|error| panic!("read {path}: {error}"));
        let result = scan(&path, &source);
        calls += result.calls;
        problems.extend(result.findings);
    }
    let missing: Vec<_> = PACKAGES.iter().filter(|pkg| !seen.contains(*pkg)).collect();
    assert!(missing.is_empty(), "no .bzl files from {missing:?}");
    assert!(
        calls > MIN_CALLS,
        "only {calls} rules and aspects were checked"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
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
    assert!(findings(
        "pkg/defs.bzl",
        "x = rule(implementation = _impl, test = True)\n"
    )
    .is_empty());
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
