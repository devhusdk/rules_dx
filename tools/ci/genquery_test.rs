use dx_guard::allowlist::{self, Counts, hit};
use dx_guard::snapshot::snapshot;
use dx_guard::starlark::{callee_name, exprs, is_starlark, parse};
use starlark_syntax::syntax::ast::ExprP;

const KINDS: [&str; 1] = ["genquery"];

fn scan(path: &str, source: &str, counts: &mut Counts) {
    if !is_starlark(path) {
        return;
    }
    let module = parse(path, source).unwrap_or_else(|error| panic!("{error}"));
    for expr in exprs(module.statement()) {
        if matches!(expr.node, ExprP::Identifier(_) | ExprP::Dot(..))
            && callee_name(expr) == "genquery"
        {
            hit(counts, path, "genquery");
        }
    }
}

fn scan_one(path: &str, source: &str) -> Counts {
    let mut counts = Counts::new();
    scan(path, source, &mut counts);
    counts
}

#[test]
fn the_tree_matches_the_allowlist() {
    let mut counts = Counts::new();
    for (path, source) in &snapshot() {
        scan(path, source, &mut counts);
    }
    allowlist::verify(&counts, &KINDS);
}

#[test]
fn genquery_counts_as_a_rule_and_as_a_native_call() {
    let counts = scan_one(
        "pkg/BUILD.bazel",
        "genquery(name = \"a\", expression = \"//x\", scope = [\"//x\"])\n",
    );
    assert_eq!(
        counts.get(&("pkg/BUILD.bazel".to_owned(), "genquery".to_owned())),
        Some(&1)
    );
    let counts = scan_one(
        "pkg/defs.bzl",
        "def m(name):\n    native.genquery(name = name, expression = \"//x\", scope = [\"//x\"])\n    native.genquery(name = name + \"_2\", expression = \"//x\", scope = [\"//x\"])\n",
    );
    assert_eq!(
        counts.get(&("pkg/defs.bzl".to_owned(), "genquery".to_owned())),
        Some(&2)
    );
}

#[test]
fn an_aliased_native_genquery_counts() {
    let counts = scan_one(
        "pkg/defs.bzl",
        "q = native.genquery\n\ndef m(name):\n    q(name = name, expression = \"//x\", scope = [\"//x\"])\n",
    );
    assert_eq!(
        counts.get(&("pkg/defs.bzl".to_owned(), "genquery".to_owned())),
        Some(&1)
    );
    let counts = scan_one("pkg/defs.bzl", "rules = {\"query\": native.genquery}\n");
    assert_eq!(
        counts.get(&("pkg/defs.bzl".to_owned(), "genquery".to_owned())),
        Some(&1)
    );
}

#[test]
fn genquery_in_a_string_or_another_file_does_not_count() {
    assert!(scan_one("pkg/BUILD.bazel", "filegroup(name = \"genquery(\")\n").is_empty());
    assert!(scan_one("pkg/notes.txt", "genquery(\n").is_empty());
}
