//! No test carries a banned tag outside tools/ci/test_tags_allowlist.txt, and the allowlist holds no spare entry.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json as _;
use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstAssignTarget, AstExpr, AstLiteral, AstStmt, BinOp, ExprP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};
use yaml_serde as _;

/// Tags no test may carry.
const BANNED: [&str; 8] = [
    "exclusive",
    "external",
    "local",
    "manual",
    "no-cache",
    "no-remote",
    "no-sandbox",
    "requires-network",
];

/// Words in a rule or macro name that mark it as a test.
const TEST_WORDS: [&str; 5] = ["bench", "fuzz", "suite", "test", "tests"];

/// The allowlist, relative to the checkout.
const ALLOWLIST: &str = "tools/ci/test_tags_allowlist.txt";

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

/// Occurrences keyed by file path and tag.
type Counts = BTreeMap<(String, String), usize>;

/// Where an expression sits relative to a tags value.
#[derive(Clone, Copy, PartialEq)]
enum Position {
    Outside,
    Tags,
    CallTags,
}

/// What one file contributes before names resolve across files.
#[derive(Default)]
struct FileScan {
    literals: BTreeMap<String, usize>,
    globals: BTreeMap<String, Carrier>,
    loads: BTreeMap<String, String>,
    test_rules: BTreeSet<String>,
    defs: BTreeMap<String, BTreeSet<String>>,
    calls: Vec<TaggedCall>,
}

/// A module-level tags constant: its banned tags and the names it is built from.
#[derive(Default, Clone)]
struct Carrier {
    tags: BTreeSet<String>,
    names: BTreeSet<String>,
}

/// One call that passes `tags`: its callee, the banned literals and the names in the value.
struct TaggedCall {
    callee: String,
    literals: Vec<String>,
    names: Vec<String>,
}

fn banned(text: &str) -> bool {
    BANNED.contains(&text)
}

fn names_tags(name: &str) -> bool {
    name.to_ascii_lowercase().contains("tags")
}

/// Whether `name` reads as a test rule or macro.
fn test_name(name: &str) -> bool {
    name.split('_').any(|word| TEST_WORDS.contains(&word))
}

/// The last name of a callee: `native.test_suite` gives `test_suite`.
fn callee_name(callee: &AstExpr) -> String {
    match &callee.node {
        ExprP::Identifier(ident) => ident.node.ident.clone(),
        ExprP::Dot(_, name) => name.node.clone(),
        _ => String::new(),
    }
}

fn is_true(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "True")
}

fn is_string(expr: &AstExpr, wanted: &str) -> bool {
    matches!(&expr.node, ExprP::Literal(AstLiteral::String(text)) if text.node == wanted)
}

/// Whether an assignment target holds tags: a tags-named variable or a `["tags"]` slot.
fn tags_target(target: &AstAssignTarget) -> bool {
    match &target.node {
        AssignTargetP::Identifier(ident) => names_tags(&ident.node.ident),
        AssignTargetP::Index(pair) => is_string(&pair.1, "tags"),
        _ => false,
    }
}

/// Collects banned literals and identifiers of one tags value.
#[derive(Default)]
struct Value {
    literals: Vec<String>,
    names: Vec<String>,
}

struct Walker<'a> {
    scan: &'a mut FileScan,
    def: Option<String>,
}

impl Walker<'_> {
    fn count(&mut self, tag: &str) {
        *self.scan.literals.entry(tag.to_owned()).or_default() += 1;
    }

    fn stmt(&mut self, stmt: &AstStmt, top: bool) {
        match &stmt.node {
            StmtP::Statements(stmts) => {
                for child in stmts {
                    self.stmt(child, top);
                }
            }
            StmtP::Load(load) => {
                for arg in &load.args {
                    self.scan
                        .loads
                        .insert(arg.local.node.ident.clone(), arg.their.node.clone());
                }
            }
            StmtP::Def(def) => {
                let outer = self.def.replace(def.name.node.ident.clone());
                self.scan
                    .defs
                    .entry(def.name.node.ident.clone())
                    .or_default();
                self.stmt(&def.body, false);
                self.def = outer;
            }
            StmtP::Assign(assign) => {
                self.test_rule(&assign.lhs, &assign.rhs);
                self.assign(&assign.lhs, &assign.rhs, top);
            }
            StmtP::AssignModify(lhs, _, rhs) => self.assign(lhs, rhs, top),
            StmtP::If(condition, body) => {
                self.expr(condition, Position::Outside, None);
                self.stmt(body, false);
            }
            StmtP::IfElse(condition, branches) => {
                self.expr(condition, Position::Outside, None);
                self.stmt(&branches.0, false);
                self.stmt(&branches.1, false);
            }
            StmtP::For(each) => {
                self.expr(&each.over, Position::Outside, None);
                self.stmt(&each.body, false);
            }
            StmtP::Return(Some(expr)) | StmtP::Expression(expr) => {
                self.expr(expr, Position::Outside, None);
            }
            StmtP::Return(None) | StmtP::Break | StmtP::Continue | StmtP::Pass => {}
        }
    }

    /// Records `name = rule(..., test = True)`.
    fn test_rule(&mut self, lhs: &AstAssignTarget, rhs: &AstExpr) {
        let AssignTargetP::Identifier(ident) = &lhs.node else {
            return;
        };
        let ExprP::Call(callee, args) = &rhs.node else {
            return;
        };
        if callee_name(callee) != "rule" {
            return;
        }
        let test = args.args.iter().any(|arg| {
            matches!(&arg.node, ArgumentP::Named(key, value) if key.node == "test" && is_true(value))
        });
        if test {
            self.scan.test_rules.insert(ident.node.ident.clone());
        }
    }

    fn assign(&mut self, lhs: &AstAssignTarget, rhs: &AstExpr, top: bool) {
        if !tags_target(lhs) {
            self.expr(rhs, Position::Outside, None);
            return;
        }
        let mut value = Value::default();
        self.expr(rhs, Position::Tags, Some(&mut value));
        if let (true, AssignTargetP::Identifier(ident)) = (top, &lhs.node) {
            let carrier = self
                .scan
                .globals
                .entry(ident.node.ident.clone())
                .or_default();
            carrier.tags.extend(value.literals);
            carrier.names.extend(value.names);
        }
    }

    fn expr(&mut self, expr: &AstExpr, position: Position, mut value: Option<&mut Value>) {
        match &expr.node {
            ExprP::Literal(AstLiteral::String(text)) if banned(&text.node) => {
                if position == Position::Tags {
                    self.count(&text.node);
                }
                if position != Position::Outside
                    && let Some(value) = value
                {
                    value.literals.push(text.node.clone());
                }
            }
            ExprP::Identifier(ident) => {
                if position != Position::Outside
                    && let Some(value) = value
                {
                    value.names.push(ident.node.ident.clone());
                }
            }
            ExprP::Op(left, BinOp::In | BinOp::NotIn | BinOp::Equal | BinOp::NotEqual, right) => {
                self.expr(left, Position::Outside, None);
                self.expr(right, Position::Outside, None);
            }
            ExprP::Call(callee, args) => {
                let name = callee_name(callee);
                if let Some(def) = &self.def {
                    self.scan
                        .defs
                        .entry(def.clone())
                        .or_default()
                        .insert(name.clone());
                }
                self.expr(callee, Position::Outside, None);
                for arg in &args.args {
                    match &arg.node {
                        ArgumentP::Named(key, tags) if key.node == "tags" => {
                            self.tags_argument(&name, tags);
                        }
                        ArgumentP::Named(key, flag) if key.node == "local" && is_true(flag) => {
                            self.scan.calls.push(TaggedCall {
                                callee: name.clone(),
                                literals: vec!["local".to_owned()],
                                names: Vec::new(),
                            });
                        }
                        ArgumentP::Named(_, inner)
                        | ArgumentP::Positional(inner)
                        | ArgumentP::Args(inner)
                        | ArgumentP::KwArgs(inner) => {
                            let inner_position =
                                if position == Position::Outside && names_tags(&name) {
                                    Position::Tags
                                } else {
                                    position
                                };
                            self.expr(inner, inner_position, value.as_deref_mut());
                        }
                    }
                }
            }
            ExprP::Dict(entries) => {
                for (key, entry) in entries {
                    self.expr(key, Position::Outside, None);
                    if position == Position::Outside && is_string(key, "tags") {
                        self.expr(entry, Position::Tags, None);
                    } else {
                        self.expr(entry, position, value.as_deref_mut());
                    }
                }
            }
            other => other.visit_expr(|child| self.expr(child, position, value.as_deref_mut())),
        }
    }

    /// Walks the value of a `tags` argument and records the call for later classification.
    fn tags_argument(&mut self, callee: &str, tags: &AstExpr) {
        let mut value = Value::default();
        self.expr(tags, Position::CallTags, Some(&mut value));
        self.scan.calls.push(TaggedCall {
            callee: callee.to_owned(),
            literals: value.literals,
            names: value.names,
        });
    }
}

/// Parses one BUILD or .bzl file.
fn scan_file(path: &str, source: &str) -> FileScan {
    let ast = AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()));
    let mut scan = FileScan::default();
    Walker {
        scan: &mut scan,
        def: None,
    }
    .stmt(ast.statement(), true);
    scan
}

/// Names that are tests: test-named callees, test rules, and macros that call one.
fn test_callees(files: &BTreeMap<String, FileScan>) -> BTreeSet<String> {
    let mut tests = BTreeSet::new();
    for scan in files.values() {
        tests.extend(scan.test_rules.iter().cloned());
    }
    loop {
        let mut grew = false;
        for scan in files.values() {
            for (def, callees) in &scan.defs {
                if tests.contains(def) {
                    continue;
                }
                if callees
                    .iter()
                    .any(|callee| test_name(callee) || tests.contains(callee))
                {
                    tests.insert(def.clone());
                    grew = true;
                }
            }
        }
        if !grew {
            return tests;
        }
    }
}

/// Banned tags each module-level tags constant carries, by constant name.
fn carriers(files: &BTreeMap<String, FileScan>) -> BTreeMap<String, BTreeSet<String>> {
    let mut merged: BTreeMap<String, Carrier> = BTreeMap::new();
    for scan in files.values() {
        for (name, carrier) in &scan.globals {
            let entry = merged.entry(name.clone()).or_default();
            entry.tags.extend(carrier.tags.iter().cloned());
            for inner in &carrier.names {
                let their = scan.loads.get(inner).unwrap_or(inner);
                entry.names.insert(their.clone());
            }
        }
    }
    let mut resolved: BTreeMap<String, BTreeSet<String>> = merged
        .iter()
        .map(|(name, carrier)| (name.clone(), carrier.tags.clone()))
        .collect();
    loop {
        let mut grew = false;
        for (name, carrier) in &merged {
            for inner in &carrier.names {
                let add = resolved.get(inner).cloned().unwrap_or_default();
                let entry = resolved.get_mut(name).expect("carrier");
                for tag in add {
                    grew |= entry.insert(tag);
                }
            }
        }
        if !grew {
            return resolved;
        }
    }
}

/// Banned tag occurrences across `files`, keyed by path and tag.
fn occurrences(files: &BTreeMap<String, FileScan>) -> Counts {
    let tests = test_callees(files);
    let carried = carriers(files);
    let mut counts = Counts::new();
    for (path, scan) in files {
        let mut add = |tag: &str, by: usize| {
            *counts.entry((path.clone(), tag.to_owned())).or_default() += by;
        };
        for (tag, count) in &scan.literals {
            add(tag, *count);
        }
        for call in &scan.calls {
            if !test_name(&call.callee) && !tests.contains(&call.callee) {
                continue;
            }
            for tag in &call.literals {
                add(tag, 1);
            }
            for name in &call.names {
                let known = scan.globals.contains_key(name) || scan.loads.contains_key(name);
                if !known {
                    continue;
                }
                let their = scan.loads.get(name).unwrap_or(name);
                for tag in carried.get(their).into_iter().flatten() {
                    add(tag, 1);
                }
            }
        }
    }
    counts
}

fn is_artifact(name: &str) -> bool {
    name.starts_with('.') || name.starts_with("bazel-") || name == "node_modules"
}

fn is_starlark(name: &str) -> bool {
    name == "BUILD" || name == "BUILD.bazel" || name.ends_with(".bzl")
}

fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, FileScan>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read_dir {dir:?}: {error}"));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("{dir:?}: {error}"));
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if path.is_dir() {
            if !is_artifact(&name) {
                collect(root, &path, files);
            }
        } else if is_starlark(&name) {
            let rel = path
                .strip_prefix(root)
                .expect("file under the checkout")
                .to_string_lossy()
                .replace('\\', "/");
            let source = dx_testing::read_text(&path);
            files.insert(rel.clone(), scan_file(&rel, &source));
        }
    }
}

/// One allowlist line: `<path> <tag> <count>`.
fn parse_allowlist(text: &str) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, tag, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count.parse::<usize>().unwrap_or(0);
        if count == 0 || !banned(tag) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        let key = ((*path).to_owned(), (*tag).to_owned());
        if previous.as_ref().is_some_and(|before| *before >= key) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(key.clone());
        counts.insert(key, count);
    }
    (counts, problems)
}

/// Lines that differ between the tree and the allowlist.
fn differences(found: &Counts, allowed: &Counts) -> Vec<String> {
    let mut out = Vec::new();
    for ((path, tag), count) in found {
        let limit = allowed
            .get(&(path.clone(), tag.clone()))
            .copied()
            .unwrap_or(0);
        if *count > limit {
            out.push(format!(
                "{path}: {count} `{tag}` on tests, {limit} allowed; remove the tag"
            ));
        }
    }
    for ((path, tag), limit) in allowed {
        let count = found
            .get(&(path.clone(), tag.clone()))
            .copied()
            .unwrap_or(0);
        if count == 0 {
            out.push(format!(
                "{path}: no `{tag}` left; delete the allowlist line"
            ));
        } else if count < *limit {
            out.push(format!(
                "{path}: {count} `{tag}` left; lower the allowlist count from {limit}"
            ));
        }
    }
    out
}

fn scan_sources(sources: &[(&str, &str)]) -> Counts {
    let files = sources
        .iter()
        .map(|(path, source)| ((*path).to_owned(), scan_file(path, source)))
        .collect();
    occurrences(&files)
}

fn counts(entries: &[(&str, &str, usize)]) -> Counts {
    entries
        .iter()
        .map(|(path, tag, count)| (((*path).to_owned(), (*tag).to_owned()), *count))
        .collect()
}

#[test]
fn the_tree_matches_the_allowlist() {
    let root = dx_testing::rules_dx_source();
    let mut files = BTreeMap::new();
    collect(&root, &root, &mut files);
    assert!(
        files.len() >= MIN_FILES,
        "only {} BUILD and .bzl files under {root:?}",
        files.len()
    );
    let found = occurrences(&files);
    let (allowed, mut problems) = parse_allowlist(&dx_testing::read_text(&root.join(ALLOWLIST)));
    problems.extend(differences(&found, &allowed));
    assert!(
        problems.is_empty(),
        "{ALLOWLIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
}

#[test]
fn tags_on_tests_count_and_tags_on_other_targets_do_not() {
    let build = r#"
cc_test(name = "a", tags = ["manual", "cpu:4"])
cc_library(name = "b", tags = ["manual"])
cc_binary(name = "c", tags = merge_tags(["x"], "local"))
native.test_suite(name = "d", tags = ["manual"] + ["no-cache"])
rust_test(name = "e", local = True)
rust_test(name = "f", args = ["manual"], env = {"MODE": "local"})
"#;
    assert_eq!(
        scan_sources(&[("pkg/BUILD.bazel", build)]),
        counts(&[
            ("pkg/BUILD.bazel", "local", 1),
            ("pkg/BUILD.bazel", "manual", 2),
            ("pkg/BUILD.bazel", "no-cache", 1),
        ]),
    );
}

#[test]
fn tags_built_outside_a_call_count_where_they_are_written() {
    let bzl = r#"
_GPU_TAGS = ["exclusive", "requires-gpu"]

def gpu_library(name, tags = [], **kwargs):
    tags = merge_tags(tags, "manual")
    kwargs["tags"] = sorted(tags + ["no-sandbox"])
    if "local" in tags or kwargs.get("route") == "external":
        return {"tags": ["no-remote"], "route": "local"}
    requirements["no-cache"] = "1"
    return None
"#;
    assert_eq!(
        scan_sources(&[("pkg/defs.bzl", bzl)]),
        counts(&[
            ("pkg/defs.bzl", "exclusive", 1),
            ("pkg/defs.bzl", "manual", 1),
            ("pkg/defs.bzl", "no-remote", 1),
            ("pkg/defs.bzl", "no-sandbox", 1),
        ]),
    );
}

#[test]
fn a_loaded_tags_constant_counts_at_each_test_that_uses_it() {
    let defs = r#"
BASE_TAGS = ["local", "resources:cpu:2"]
NETWORK_TAGS = BASE_TAGS + ["requires-network"]
"#;
    let build = r#"
load("//pkg:defs.bzl", "BASE_TAGS", NET = "NETWORK_TAGS")
rust_test(name = "a", tags = BASE_TAGS)
rust_test(name = "b", tags = NET + ["x"])
rust_library(name = "c", tags = NET)
"#;
    assert_eq!(
        scan_sources(&[("pkg/defs.bzl", defs), ("use/BUILD.bazel", build)]),
        counts(&[
            ("pkg/defs.bzl", "local", 1),
            ("pkg/defs.bzl", "requires-network", 1),
            ("use/BUILD.bazel", "local", 2),
            ("use/BUILD.bazel", "requires-network", 1),
        ]),
    );
}

#[test]
fn test_rules_and_the_macros_that_call_them_are_tests() {
    let defs = r#"
_probe = rule(implementation = _impl, test = True)

def probe(name, **kwargs):
    _probe(name = name, **kwargs)

def wrapped(name, **kwargs):
    probe(name = name, **kwargs)

def plain(name, **kwargs):
    native.filegroup(name = name, **kwargs)
"#;
    let build = r#"
wrapped(name = "a", tags = ["manual"])
plain(name = "b", tags = ["manual"])
"#;
    assert_eq!(
        scan_sources(&[("pkg/defs.bzl", defs), ("pkg/BUILD.bazel", build)]),
        counts(&[("pkg/BUILD.bazel", "manual", 1)]),
    );
}

#[test]
fn the_allowlist_rejects_new_tags_and_spare_entries() {
    let found = counts(&[
        ("a/BUILD.bazel", "manual", 3),
        ("b/BUILD.bazel", "local", 1),
    ]);
    let allowed = counts(&[
        ("a/BUILD.bazel", "manual", 2),
        ("b/BUILD.bazel", "local", 2),
        ("c/BUILD.bazel", "manual", 1),
    ]);
    assert_eq!(
        differences(&found, &allowed),
        [
            "a/BUILD.bazel: 3 `manual` on tests, 2 allowed; remove the tag",
            "b/BUILD.bazel: 1 `local` left; lower the allowlist count from 2",
            "c/BUILD.bazel: no `manual` left; delete the allowlist line",
        ],
    );
    assert_eq!(differences(&found, &found), Vec::<String>::new());
}

#[test]
fn the_allowlist_format_is_strict() {
    let good = "a/BUILD.bazel exclusive 1\na/BUILD.bazel manual 2\n";
    let (parsed, problems) = parse_allowlist(good);
    assert_eq!(problems, Vec::<String>::new());
    assert_eq!(
        parsed,
        counts(&[
            ("a/BUILD.bazel", "exclusive", 1),
            ("a/BUILD.bazel", "manual", 2)
        ]),
    );
    for bad in [
        "a/BUILD.bazel exclusive 1 resource=gpu\n",
        "a/BUILD.bazel manual 1 resource=gpu\n",
        "a/BUILD.bazel manual 0\n",
        "a/BUILD.bazel slow 1\n",
        "b/BUILD.bazel manual 1\na/BUILD.bazel manual 1\n",
        "a/BUILD.bazel manual\n",
    ] {
        assert!(!parse_allowlist(bad).1.is_empty(), "{bad:?}");
    }
}
