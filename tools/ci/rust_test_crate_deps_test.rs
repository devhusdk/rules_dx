//! No `rust_test` takes `all_crate_deps(normal = True)` without `normal_dev = True`.

use std::collections::BTreeMap;

use dx_guard::snapshot::read_input;
use dx_guard::starlark::{callee_name, exprs, is_true, parse, string};
use starlark_syntax::syntax::ast::{ArgumentP, AstArgument, AstExpr, ExprP};
use yaml_serde as _;

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

/// What a failure tells the author to do.
const FIX: &str = "pass normal_dev = True to all_crate_deps or list the crates the test uses";

fn named<'a>(args: &'a [AstArgument], key: &str) -> Option<&'a AstExpr> {
    args.iter().find_map(|arg| match &arg.node {
        ArgumentP::Named(name, value) if name.node == key => Some(value),
        ArgumentP::Named(..)
        | ArgumentP::Positional(_)
        | ArgumentP::Args(_)
        | ArgumentP::KwArgs(_) => None,
    })
}

fn flag(args: &[AstArgument], key: &str) -> bool {
    named(args, key).is_some_and(is_true)
}

/// Whether `expr` holds an `all_crate_deps` call with normal deps and without dev deps.
fn normal_only(expr: &AstExpr) -> bool {
    let mut found = false;
    let mut pending = vec![expr];
    while let Some(expr) = pending.pop() {
        if let ExprP::Call(callee, call) = &expr.node
            && callee_name(callee) == "all_crate_deps"
            && flag(&call.args, "normal")
            && !flag(&call.args, "normal_dev")
        {
            found = true;
        }
        expr.visit_expr(|child| pending.push(child));
    }
    found
}

fn violations(path: &str, source: &str) -> Vec<String> {
    let ast = parse(path, source);
    let mut out = Vec::new();
    for expr in exprs(ast.statement()) {
        let ExprP::Call(callee, call) = &expr.node else {
            continue;
        };
        if callee_name(callee) != "rust_test" {
            continue;
        }
        if named(&call.args, "deps").is_some_and(normal_only) {
            let name = named(&call.args, "name").and_then(string).unwrap_or("?");
            out.push(format!("{path}: rust_test {name:?}: {FIX}"));
        }
    }
    out
}

#[test]
fn rust_tests_take_dev_deps_with_normal_deps() {
    let sources: BTreeMap<String, String> =
        serde_json::from_str(&read_input("RUST_TEST_CRATE_DEPS_SOURCES"))
            .unwrap_or_else(|error| panic!("sources snapshot: {error}"));
    assert!(
        sources.len() >= MIN_FILES,
        "only {} BUILD and .bzl files in the sources snapshot",
        sources.len()
    );
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(path, source)| violations(path, source))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn only_normal_deps_without_dev_deps_on_a_rust_test_fail() {
    let build = r#"
rust_test(name = "a", deps = all_crate_deps(package_name = "p", normal = True) + [":x"])
rust_test(name = "b", deps = all_crate_deps(package_name = "p", normal = True, normal_dev = True))
rust_test(name = "c", deps = ["@crates//:serde_json"])
rust_test(name = "d", aliases = aliases(package_name = "p", normal = True))
rust_library(name = "e", deps = all_crate_deps(package_name = "p", normal = True))
native.rust_test(name = "f", deps = [":x"] + all_crate_deps(normal = True))
"#;
    assert_eq!(
        violations("pkg/BUILD.bazel", build),
        [
            format!("pkg/BUILD.bazel: rust_test \"a\": {FIX}"),
            format!("pkg/BUILD.bazel: rust_test \"f\": {FIX}"),
        ],
    );
}
