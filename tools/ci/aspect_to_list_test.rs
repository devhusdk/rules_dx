use std::collections::{BTreeMap, BTreeSet};

use dx_guard::allowlist::{self, Counts, hit};
use dx_guard::snapshot::snapshot;
use dx_guard::starlark::{callee_name, exprs, is_starlark, parse};
use serde_json as _;
use starlark_syntax::syntax::AstModule;
use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstExpr, AstStmt, ExprP, StmtP};
use yaml_serde as _;

const KIND: &str = "aspect-to-list";

struct Def<'a> {
    calls: Vec<&'a AstExpr>,
    nested: Vec<String>,
    flattens: Vec<u32>,
}

#[derive(Default)]
struct Scope<'a> {
    defs: BTreeMap<String, Def<'a>>,
    globals: BTreeMap<String, &'a AstExpr>,
    loads: BTreeMap<String, (String, String)>,
}

type Func = (String, String);

fn statements<'a>(stmt: &'a AstStmt, out: &mut Vec<&'a AstStmt>) {
    out.push(stmt);
    stmt.visit_stmt(|child| statements(child, out));
}

fn is_to_list(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Call(callee, _) if matches!(&callee.node, ExprP::Dot(_, name) if name.node == "to_list"))
}

fn named_args<'a>(root: &'a AstStmt, function: &str, key: &str) -> Vec<&'a AstExpr> {
    let mut found = Vec::new();
    for expr in exprs(root) {
        let ExprP::Call(callee, args) = &expr.node else {
            continue;
        };
        if callee_name(callee) != function {
            continue;
        }
        for arg in &args.args {
            if let ArgumentP::Named(name, value) = &arg.node
                && name.node == key
            {
                found.push(value);
            }
        }
    }
    found
}

fn load_path(from: &str, label: &str, files: &BTreeMap<String, String>) -> Option<String> {
    for prefix in ["@rules_dx//", "@//", "//"] {
        if let Some(rest) = label.strip_prefix(prefix) {
            let path = match rest.split_once(':') {
                Some(("", name)) => name.to_owned(),
                Some((package, name)) => format!("{package}/{name}"),
                None => return None,
            };
            return files.contains_key(&path).then_some(path);
        }
    }
    let name = label.strip_prefix(':')?;
    let mut dir = from.rsplit_once('/').map_or("", |(dir, _)| dir);
    loop {
        let path = if dir.is_empty() {
            name.to_owned()
        } else {
            format!("{dir}/{name}")
        };
        if files.contains_key(&path) {
            return Some(path);
        }
        if dir.is_empty() {
            return None;
        }
        dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
}

fn scope<'a>(path: &str, module: &'a AstModule, files: &BTreeMap<String, String>) -> Scope<'a> {
    let mut scope = Scope::default();
    let root = module.statement();
    let top: Vec<&AstStmt> = if let StmtP::Statements(stmts) = &root.node {
        stmts.iter().collect()
    } else {
        vec![root]
    };
    for stmt in top {
        if let StmtP::Load(load) = &stmt.node
            && let Some(target) = load_path(path, &load.module.node, files)
        {
            for arg in &load.args {
                scope.loads.insert(
                    arg.local.node.ident.clone(),
                    (target.clone(), arg.their.node.clone()),
                );
            }
        } else if let StmtP::Assign(assign) = &stmt.node
            && let AssignTargetP::Identifier(ident) = &assign.lhs.node
        {
            scope.globals.insert(ident.node.ident.clone(), &assign.rhs);
        }
    }
    let mut all = Vec::new();
    statements(root, &mut all);
    for stmt in all {
        if let StmtP::Def(def) = &stmt.node {
            let body = exprs(&def.body);
            let mut inner = Vec::new();
            statements(&def.body, &mut inner);
            scope.defs.insert(
                def.name.node.ident.clone(),
                Def {
                    calls: body
                        .iter()
                        .copied()
                        .filter(|expr| matches!(expr.node, ExprP::Call(..)))
                        .collect(),
                    nested: inner
                        .iter()
                        .filter_map(|stmt| {
                            if let StmtP::Def(nested) = &stmt.node {
                                Some(nested.name.node.ident.clone())
                            } else {
                                None
                            }
                        })
                        .collect(),
                    flattens: body
                        .iter()
                        .filter(|expr| is_to_list(expr))
                        .map(|expr| expr.span.begin().get())
                        .collect(),
                },
            );
        }
    }
    scope
}

struct Index<'a> {
    scopes: BTreeMap<String, Scope<'a>>,
}

impl Index<'_> {
    fn resolve(&self, file: &str, name: &str, seen: &mut BTreeSet<Func>, out: &mut Vec<Func>) {
        if !seen.insert((file.to_owned(), name.to_owned())) {
            return;
        }
        let Some(scope) = self.scopes.get(file) else {
            return;
        };
        if scope.defs.contains_key(name) {
            out.push((file.to_owned(), name.to_owned()));
        } else if let Some(value) = scope.globals.get(name) {
            self.values(file, value, seen, out);
        } else if let Some((target, their)) = scope.loads.get(name) {
            self.resolve(target, their, seen, out);
        }
    }

    fn values(&self, file: &str, expr: &AstExpr, seen: &mut BTreeSet<Func>, out: &mut Vec<Func>) {
        if let ExprP::Identifier(ident) = &expr.node {
            self.resolve(file, &ident.node.ident, seen, out);
        } else if let ExprP::Dict(entries) = &expr.node {
            for (_, value) in entries {
                self.values(file, value, seen, out);
            }
        } else if let ExprP::Op(left, _, right) = &expr.node {
            self.values(file, left, seen, out);
            self.values(file, right, seen, out);
        } else if let ExprP::Call(callee, _) = &expr.node {
            self.values(file, callee, seen, out);
        }
    }

    fn callee(&self, file: &str, call: &AstExpr, out: &mut Vec<Func>) {
        let ExprP::Call(callee, _) = &call.node else {
            return;
        };
        let name = callee_name(callee);
        if let ExprP::Identifier(_) = &callee.node {
            self.resolve(file, name, &mut BTreeSet::new(), out);
        } else if self
            .scopes
            .get(file)
            .is_some_and(|scope| scope.defs.contains_key(name))
        {
            out.push((file.to_owned(), name.to_owned()));
        }
    }
}

fn scan(files: &BTreeMap<String, String>) -> Counts {
    let modules: BTreeMap<&String, AstModule> = files
        .iter()
        .filter(|(path, _)| is_starlark(path))
        .map(|(path, source)| {
            (
                path,
                parse(path, source).unwrap_or_else(|error| panic!("{error}")),
            )
        })
        .collect();
    let index = Index {
        scopes: modules
            .iter()
            .map(|(path, module)| ((*path).clone(), scope(path, module, files)))
            .collect(),
    };
    let mut pending = Vec::new();
    for (path, module) in &modules {
        let root = module.statement();
        for value in named_args(root, "aspect", "implementation")
            .into_iter()
            .chain(named_args(root, "real_config", "context_hooks"))
        {
            index.values(path, value, &mut BTreeSet::new(), &mut pending);
        }
    }
    let mut reached = BTreeSet::new();
    let mut flattens: BTreeSet<(String, u32)> = BTreeSet::new();
    while let Some((file, name)) = pending.pop() {
        if !reached.insert((file.clone(), name.clone())) {
            continue;
        }
        let Some(def) = index
            .scopes
            .get(&file)
            .and_then(|scope| scope.defs.get(&name))
        else {
            continue;
        };
        flattens.extend(def.flattens.iter().map(|offset| (file.clone(), *offset)));
        for call in &def.calls {
            index.callee(&file, call, &mut pending);
        }
        pending.extend(
            def.nested
                .iter()
                .map(|nested| (file.clone(), nested.clone())),
        );
    }
    let mut counts = Counts::new();
    for (file, _) in flattens {
        hit(&mut counts, &file, KIND);
    }
    counts
}

#[test]
fn the_tree_matches_the_allowlist() {
    allowlist::verify(&scan(&snapshot()), &[KIND]);
}

fn count(counts: &Counts, path: &str) -> Option<usize> {
    counts.get(&(path.to_owned(), KIND.to_owned())).copied()
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
    assert_eq!(count(&counts, "pkg/a.bzl"), Some(2));
    assert_eq!(counts.len(), 1);
}

#[test]
fn a_loaded_helper_counts_once_at_its_own_file() {
    let helpers = r#"
load(":deeper.bzl", _inner = "inner")

def transitive_srcs(t):
    return [f for f in t.files.to_list()] + _inner(t)

def unused(t):
    return t.files.to_list()
"#;
    let deeper = r#"
def inner(t):
    return t.srcs.to_list()
"#;
    let first = r#"
load("//lib:helpers.bzl", "transitive_srcs")

def _impl(target, ctx):
    return transitive_srcs(target)

a = aspect(implementation = _impl)
"#;
    let second = r#"
load("@rules_dx//lib:helpers.bzl", srcs = "transitive_srcs")

def _impl(target, ctx):
    return srcs(target)

b = aspect(implementation = _impl)
"#;
    let files = BTreeMap::from([
        ("lib/helpers.bzl".to_owned(), helpers.to_owned()),
        ("lib/deeper.bzl".to_owned(), deeper.to_owned()),
        ("pkg/a.bzl".to_owned(), first.to_owned()),
        ("other/b.bzl".to_owned(), second.to_owned()),
    ]);
    let counts = scan(&files);
    assert_eq!(count(&counts, "lib/helpers.bzl"), Some(1));
    assert_eq!(count(&counts, "lib/deeper.bzl"), Some(1));
    assert_eq!(counts.len(), 2);
}

#[test]
fn context_hooks_passed_to_real_config_are_reached() {
    let hooks = r#"
def _qmllint_hook(target, ctx, resolved, capability, what, shared):
    return target[DefaultInfo].files.to_list()

def compile_check_hook(tool):
    def hook(target, ctx, resolved, capability, what, shared):
        return target.files.to_list()
    return hook

def _not_a_hook(target):
    return target.files.to_list()

HOOKS = {"qmllint": _qmllint_hook}
"#;
    let family = r#"
load("//lang:hooks.bzl", "HOOKS", "compile_check_hook")

def _eslint_hook(target, ctx, resolved, capability, what, shared):
    return target.inputs.to_list()

_LOCAL = {"eslint": _eslint_hook}

CONFIG = real_config(
    tools = [],
    shard_specs = [],
    context_hooks = HOOKS | _LOCAL | {"cc": compile_check_hook("cc")},
)
"#;
    let files = BTreeMap::from([
        ("lang/hooks.bzl".to_owned(), hooks.to_owned()),
        ("lang/family.bzl".to_owned(), family.to_owned()),
    ]);
    let counts = scan(&files);
    assert_eq!(count(&counts, "lang/hooks.bzl"), Some(2));
    assert_eq!(count(&counts, "lang/family.bzl"), Some(1));
    assert_eq!(counts.len(), 2);
}
