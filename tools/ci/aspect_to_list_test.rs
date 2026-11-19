//! Aspect implementations do not flatten depsets beyond `tools/ci/aspect_to_list_allowlist.txt`.
//!
//! An aspect runs at every node it reaches, so a `.to_list()` there costs the size of the
//! closure at each node and analysis turns quadratic on deep graphs. Keep depsets whole and
//! hand them to `Args.add_all`, or flatten once in the top-level rule. The scan follows the
//! implementation into helpers defined in the same file; helpers loaded from other files are
//! not followed.

use std::collections::{BTreeMap, BTreeSet};

use dx_guard::allowlist::{self, Counts, hit};
use dx_guard::snapshot::snapshot;
use dx_guard::starlark::{callee_name, exprs, is_starlark, parse};
use serde_json as _;
use starlark_syntax::syntax::ast::{ArgumentP, AstExpr, AstStmt, ExprP, StmtP};
use yaml_serde as _;

/// The kind every allowlist line names.
const KIND: &str = "aspect-to-list";

fn statements<'a>(stmt: &'a AstStmt, out: &mut Vec<&'a AstStmt>) {
    out.push(stmt);
    stmt.visit_stmt(|child| statements(child, out));
}

/// Whether `expr` is a call of `.to_list()`.
fn is_to_list(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Call(callee, _) if matches!(&callee.node, ExprP::Dot(_, name) if name.node == "to_list"))
}

/// The function names passed as `implementation` to `aspect(...)` calls in `module`.
fn aspect_impls(root: &AstStmt) -> BTreeSet<String> {
    let mut impls = BTreeSet::new();
    for expr in exprs(root) {
        let ExprP::Call(callee, args) = &expr.node else {
            continue;
        };
        if callee_name(callee) != "aspect" {
            continue;
        }
        for arg in &args.args {
            if let ArgumentP::Named(key, value) = &arg.node
                && key.node == "implementation"
                && let ExprP::Identifier(ident) = &value.node
            {
                impls.insert(ident.node.ident.clone());
            }
        }
    }
    impls
}

/// The `.to_list()` calls reachable from the aspect implementations of one Starlark file.
fn count_file(path: &str, source: &str, counts: &mut Counts) {
    let module = parse(path, source).unwrap_or_else(|error| panic!("{error}"));
    let mut all = Vec::new();
    statements(module.statement(), &mut all);
    // Per function: the names it calls and its own `.to_list()` count.
    let mut defs: BTreeMap<String, (BTreeSet<String>, usize)> = BTreeMap::new();
    for stmt in all {
        if let StmtP::Def(def) = &stmt.node {
            let body = exprs(&def.body);
            let callees = body
                .iter()
                .filter_map(|expr| {
                    if let ExprP::Call(callee, _) = &expr.node {
                        Some(callee_name(callee).to_owned())
                    } else {
                        None
                    }
                })
                .collect();
            let flattens = body.iter().filter(|expr| is_to_list(expr)).count();
            defs.insert(def.name.node.ident.clone(), (callees, flattens));
        }
    }
    let mut pending: Vec<String> = aspect_impls(module.statement()).into_iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some((callees, flattens)) = defs.get(&name) else {
            continue;
        };
        for _ in 0..*flattens {
            hit(counts, path, KIND);
        }
        pending.extend(callees.iter().filter(|c| defs.contains_key(*c)).cloned());
    }
}

fn scan(files: &BTreeMap<String, String>) -> Counts {
    let mut counts = Counts::new();
    for (path, source) in files {
        if is_starlark(path) {
            count_file(path, source, &mut counts);
        }
    }
    counts
}

#[test]
fn the_tree_matches_the_allowlist() {
    allowlist::verify(&scan(&snapshot()), &[KIND]);
}

#[test]
fn flattening_in_an_aspect_and_its_helpers_is_counted() {
    let source = r#"
def _helper(t):
    return t.files.to_list()

def _unused(t):
    return t.files.to_list()

def _impl(target, ctx):
    a = _helper(target)
    b = [f for f in target.files.to_list()]
    args = ctx.actions.args()
    args.add_all(target.files)
    return []

def _rule_impl(ctx):
    return ctx.files.srcs.to_list()

my_aspect = aspect(implementation = _impl)
my_rule = rule(implementation = _rule_impl)
"#;
    let files = BTreeMap::from([("pkg/a.bzl".to_owned(), source.to_owned())]);
    let counts = scan(&files);
    assert_eq!(
        counts.get(&("pkg/a.bzl".to_owned(), KIND.to_owned())),
        Some(&2)
    );
    assert_eq!(counts.len(), 1);
}
