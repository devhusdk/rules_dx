//! No test goes over the size and time budget outside tools/ci/test_budget_allowlist.txt, and tools/testing/nested_smoke.txt lists every nested test.

mod test_calls;

use std::collections::{BTreeMap, BTreeSet};

use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstExpr, AstStmt, ExprP, StmtP};
use test_calls::{
    Callees, Counts, callee_name, differences, is_test, is_true, parse, parse_allowlist,
    read_input, string, test_callees,
};
use yaml_serde as _;

/// The kinds an allowlist line may name.
const KINDS: [&str; 8] = [
    "flaky",
    "no-size",
    "size-enormous",
    "size-large",
    "size-medium",
    "small-timeout",
    "timeout-eternal",
    "timeout-long",
];

/// Sizes over the budget.
const SIZES: [&str; 3] = ["enormous", "large", "medium"];

/// Timeouts over the budget.
const TIMEOUTS: [&str; 2] = ["eternal", "long"];

/// The start of the tag every nested test carries.
const NESTED_TAG: &str = "resources:nested_bazel";

/// The most nested tests the list may hold.
const NESTED_CAP: usize = 8;

/// The ending of a private wrapper target.
const UPSTREAM: &str = "_upstream";

/// The rule that groups tests and takes no size.
const SUITE: &str = "test_suite";

/// What a failure tells the author of a new hit to do.
const FIX: &str = "make the test small";

/// The allowlist path named in failures.
const ALLOWLIST: &str = "tools/ci/test_budget_allowlist.txt";

/// The nested list path named in failures.
const NESTED_LIST: &str = "tools/testing/nested_smoke.txt";

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

/// The value of a `size` or `timeout` argument.
#[derive(Default)]
enum Attr {
    #[default]
    Unset,
    Text(String),
    Other,
}

impl Attr {
    fn of(expr: &AstExpr) -> Self {
        string(expr).map_or(Self::Other, |text| Self::Text(text.to_owned()))
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// One call that passes `name`.
#[derive(Default)]
struct Call {
    def: Option<String>,
    callee: String,
    name: Option<String>,
    size: Attr,
    timeout: Attr,
    flaky: bool,
    spread: bool,
    trues: BTreeSet<String>,
    nested_tag: bool,
    tag_names: Vec<String>,
    guards: BTreeSet<String>,
}

/// A module-level constant: whether it holds the nested tag, and the names it is built from.
#[derive(Default, Clone)]
struct Constant {
    nested: bool,
    names: BTreeSet<String>,
}

/// What one file contributes before names resolve across files.
#[derive(Default)]
struct FileScan {
    callees: Callees,
    loads: BTreeMap<String, String>,
    constants: BTreeMap<String, Constant>,
    calls: Vec<Call>,
}

/// The string literals and identifiers under one expression.
#[derive(Default)]
struct Leaves {
    strings: Vec<String>,
    names: Vec<String>,
}

fn leaves(expr: &AstExpr, out: &mut Leaves) {
    if let Some(text) = string(expr) {
        out.strings.push(text.to_owned());
    } else if let ExprP::Identifier(ident) = &expr.node {
        out.names.push(ident.node.ident.clone());
    } else {
        expr.visit_expr(|child| leaves(child, out));
    }
}

fn nested_tag(strings: &[String]) -> bool {
    strings.iter().any(|text| text.starts_with(NESTED_TAG))
}

/// Whether `stmt` holds a `return`.
fn returns(stmt: &AstStmt) -> bool {
    match &stmt.node {
        StmtP::Return(_) => true,
        StmtP::Statements(stmts) => stmts.iter().any(returns),
        StmtP::If(_, body) => returns(body),
        StmtP::IfElse(_, branches) => returns(&branches.0) || returns(&branches.1),
        StmtP::For(each) => returns(&each.body),
        _ => false,
    }
}

struct Walker<'a> {
    scan: &'a mut FileScan,
    def: Option<String>,
    guards: Vec<String>,
}

impl Walker<'_> {
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
                let guards = std::mem::take(&mut self.guards);
                self.scan.callees.def(&def.name.node.ident);
                self.stmt(&def.body, false);
                self.def = outer;
                self.guards = guards;
            }
            StmtP::Assign(assign) => {
                self.scan.callees.rule(&assign.lhs, &assign.rhs);
                if let (true, AssignTargetP::Identifier(ident)) = (top, &assign.lhs.node) {
                    let mut found = Leaves::default();
                    leaves(&assign.rhs, &mut found);
                    let constant = self
                        .scan
                        .constants
                        .entry(ident.node.ident.clone())
                        .or_default();
                    constant.nested |= nested_tag(&found.strings);
                    constant.names.extend(found.names);
                }
                self.expr(&assign.rhs);
            }
            StmtP::AssignModify(_, _, rhs) => self.expr(rhs),
            StmtP::If(condition, body) => {
                self.branch(condition, &[body]);
            }
            StmtP::IfElse(condition, branches) => {
                self.branch(condition, &[&branches.0, &branches.1]);
            }
            StmtP::For(each) => {
                self.expr(&each.over);
                self.stmt(&each.body, false);
            }
            StmtP::Return(Some(expr)) | StmtP::Expression(expr) => self.expr(expr),
            StmtP::Return(None) | StmtP::Break | StmtP::Continue | StmtP::Pass => {}
        }
    }

    /// Walks the bodies of an `if` under its condition; a body that returns keeps the condition on what follows.
    fn branch(&mut self, condition: &AstExpr, bodies: &[&AstStmt]) {
        self.expr(condition);
        let mark = self.guards.len();
        let mut found = Leaves::default();
        leaves(condition, &mut found);
        self.guards.extend(found.names);
        for body in bodies {
            self.stmt(body, false);
        }
        if !bodies.iter().any(|body| returns(body)) {
            self.guards.truncate(mark);
        }
    }

    fn expr(&mut self, expr: &AstExpr) {
        let ExprP::Call(callee, args) = &expr.node else {
            expr.visit_expr(|child| self.expr(child));
            return;
        };
        let name = callee_name(callee);
        self.scan.callees.call(self.def.as_deref(), &name);
        let mut call = Call {
            def: self.def.clone(),
            callee: name,
            guards: self.guards.iter().cloned().collect(),
            ..Call::default()
        };
        let mut named = false;
        for arg in &args.args {
            match &arg.node {
                ArgumentP::Named(key, value) => {
                    match key.node.as_str() {
                        "name" => {
                            named = true;
                            call.name = string(value).map(str::to_owned);
                        }
                        "size" => call.size = Attr::of(value),
                        "timeout" => call.timeout = Attr::of(value),
                        "flaky" => call.flaky = is_true(value),
                        "tags" => {
                            let mut found = Leaves::default();
                            leaves(value, &mut found);
                            call.nested_tag = nested_tag(&found.strings);
                            call.tag_names = found.names;
                        }
                        _ => {}
                    }
                    if is_true(value) {
                        call.trues.insert(key.node.clone());
                    }
                }
                ArgumentP::KwArgs(_) => call.spread = true,
                ArgumentP::Positional(_) | ArgumentP::Args(_) => {}
            }
        }
        if named {
            self.scan.calls.push(call);
        }
        self.expr(callee);
        for arg in &args.args {
            self.expr(arg.node.expr());
        }
    }
}

/// Parses one BUILD or .bzl file.
fn scan_file(path: &str, source: &str) -> FileScan {
    let ast = parse(path, source);
    let mut scan = FileScan::default();
    Walker {
        scan: &mut scan,
        def: None,
        guards: Vec::new(),
    }
    .stmt(ast.statement(), true);
    scan
}

/// Whether `callee` takes no size: a binary macro, which may declare a test inside, or the test build of an app.
fn unsized_macro(callee: &str) -> bool {
    callee == "erlang_test_app"
        || callee.split('_').any(|word| word == "binary") && !callee.split('_').any(|word| word == "test")
}

/// Budget hits across `files`, keyed by path and kind.
fn budget(files: &BTreeMap<String, FileScan>) -> Counts {
    let tests = test_callees(files.values().map(|scan| &scan.callees));
    let mut counts = Counts::new();
    for (path, scan) in files {
        let mut add = |kind: String| {
            *counts.entry((path.clone(), kind)).or_default() += 1;
        };
        for call in &scan.calls {
            if call.callee == SUITE || unsized_macro(&call.callee) || !is_test(&tests, &call.callee) {
                continue;
            }
            match &call.size {
                Attr::Unset if !call.spread => add("no-size".to_owned()),
                Attr::Text(size) if SIZES.contains(&size.as_str()) => add(format!("size-{size}")),
                Attr::Text(size) if size == "small" && !matches!(call.timeout, Attr::Unset) => {
                    add("small-timeout".to_owned());
                }
                _ => {}
            }
            if let Some(timeout) = call.timeout.text()
                && TIMEOUTS.contains(&timeout)
            {
                add(format!("timeout-{timeout}"));
            }
            if call.flaky {
                add("flaky".to_owned());
            }
        }
    }
    counts
}

/// The names of the module-level constants that hold the nested tag.
fn nested_constants(files: &BTreeMap<String, FileScan>) -> BTreeSet<String> {
    let mut merged: BTreeMap<String, Constant> = BTreeMap::new();
    for scan in files.values() {
        for (name, constant) in &scan.constants {
            let entry = merged.entry(name.clone()).or_default();
            entry.nested |= constant.nested;
            for inner in &constant.names {
                entry
                    .names
                    .insert(scan.loads.get(inner).unwrap_or(inner).clone());
            }
        }
    }
    let mut nested: BTreeSet<String> = merged
        .iter()
        .filter(|(_, constant)| constant.nested)
        .map(|(name, _)| name.clone())
        .collect();
    loop {
        let before = nested.len();
        for (name, constant) in &merged {
            if constant.names.iter().any(|inner| nested.contains(inner)) {
                nested.insert(name.clone());
            }
        }
        if nested.len() == before {
            return nested;
        }
    }
}

/// Whether the tags of `call` hold the nested tag.
fn tagged(scan: &FileScan, call: &Call, constants: &BTreeSet<String>) -> bool {
    call.nested_tag
        || call.tag_names.iter().any(|name| {
            let known = scan.constants.contains_key(name) || scan.loads.contains_key(name);
            known && constants.contains(scan.loads.get(name).unwrap_or(name))
        })
}

/// Macros that declare a nested test, each with the names its conditions read; none means always.
fn nested_macros(
    files: &BTreeMap<String, FileScan>,
    constants: &BTreeSet<String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut macros: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    loop {
        let mut grew = false;
        for scan in files.values() {
            for call in &scan.calls {
                let Some(def) = &call.def else {
                    continue;
                };
                if !tagged(scan, call, constants) && !macros.contains_key(&call.callee) {
                    continue;
                }
                match macros.get_mut(def) {
                    None => {
                        macros.insert(def.clone(), call.guards.clone());
                        grew = true;
                    }
                    Some(guards) if call.guards.is_empty() && !guards.is_empty() => {
                        guards.clear();
                        grew = true;
                    }
                    Some(guards) if !guards.is_empty() => {
                        for guard in &call.guards {
                            grew |= guards.insert(guard.clone());
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        if !grew {
            return macros;
        }
    }
}

fn build_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "BUILD.bazel" || name == "BUILD"
}

/// The labels of the nested tests that BUILD files declare.
fn nested_tests(files: &BTreeMap<String, FileScan>) -> BTreeSet<String> {
    let tests = test_callees(files.values().map(|scan| &scan.callees));
    let constants = nested_constants(files);
    let macros = nested_macros(files, &constants);
    let mut labels = BTreeSet::new();
    for (path, scan) in files {
        if !build_file(path) {
            continue;
        }
        let package = path.rsplit_once('/').map_or("", |(package, _)| package);
        for call in &scan.calls {
            let Some(name) = &call.name else {
                continue;
            };
            if call.def.is_some() || name.ends_with(UPSTREAM) {
                continue;
            }
            let direct = tagged(scan, call, &constants) && is_test(&tests, &call.callee);
            let through = macros
                .get(&call.callee)
                .is_some_and(|guards| guards.is_empty() || !guards.is_disjoint(&call.trues));
            if direct || through {
                labels.insert(format!("//{package}:{name}"));
            }
        }
    }
    labels
}

/// One nested list line: `<label> <seconds>`.
fn parse_nested(text: &str) -> (BTreeSet<String>, Vec<String>) {
    let mut labels = BTreeSet::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [label, seconds] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        if !label.starts_with("//") || seconds.parse::<u32>().unwrap_or(0) == 0 {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= *label) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(label);
        labels.insert((*label).to_owned());
    }
    (labels, problems)
}

/// Lines that differ between the nested tests of the tree and the list.
fn nested_differences(found: &BTreeSet<String>, listed: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    if listed.len() > NESTED_CAP {
        out.push(format!(
            "{} nested tests listed, {NESTED_CAP} allowed; test the logic without a nested Bazel",
            listed.len()
        ));
    }
    for label in found.difference(listed) {
        out.push(format!(
            "{label}: nested test without a line; add `{label} <seconds>`"
        ));
    }
    for label in listed.difference(found) {
        out.push(format!("{label}: no nested test left; delete the line"));
    }
    out
}

/// Scans the snapshot that maps each BUILD and .bzl path to its text.
fn scan_snapshot(text: &str) -> BTreeMap<String, FileScan> {
    let sources: BTreeMap<String, String> =
        serde_json::from_str(text).unwrap_or_else(|error| panic!("sources snapshot: {error}"));
    sources
        .iter()
        .map(|(path, source)| (path.clone(), scan_file(path, source)))
        .collect()
}

fn scan_sources(sources: &[(&str, &str)]) -> BTreeMap<String, FileScan> {
    sources
        .iter()
        .map(|(path, source)| ((*path).to_owned(), scan_file(path, source)))
        .collect()
}

fn counts(entries: &[(&str, &str, usize)]) -> Counts {
    entries
        .iter()
        .map(|(path, kind, count)| (((*path).to_owned(), (*kind).to_owned()), *count))
        .collect()
}

fn labels(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|label| (*label).to_owned()).collect()
}

#[test]
fn the_tree_matches_the_allowlist_and_the_nested_list() {
    let files = scan_snapshot(&read_input("DX_SOURCES_JSON"));
    assert!(
        files.len() >= MIN_FILES,
        "only {} BUILD and .bzl files in the sources snapshot",
        files.len()
    );
    let (allowed, mut problems) = parse_allowlist(&read_input("TEST_BUDGET_ALLOWLIST"), &KINDS);
    problems.extend(differences(&budget(&files), &allowed, FIX));
    assert!(
        problems.is_empty(),
        "{ALLOWLIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
    let (listed, mut problems) = parse_nested(&read_input("NESTED_LIST"));
    problems.extend(nested_differences(&nested_tests(&files), &listed));
    assert!(
        problems.is_empty(),
        "{NESTED_LIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
}

#[test]
fn each_kind_counts_on_a_test_call() {
    let build = r#"
rust_test(name = "a")
rust_test(name = "b", size = "small")
rust_test(name = "c", size = "medium")
rust_test(name = "d", size = "large", timeout = "long")
rust_test(name = "e", size = "enormous", timeout = "eternal")
rust_test(name = "f", size = "small", timeout = "short")
rust_test(name = "g", size = "small", flaky = True)
rust_test(name = "h", size = "small", flaky = False)
rust_test(name = "i", size = "large", timeout = "moderate")
"#;
    assert_eq!(
        budget(&scan_sources(&[("pkg/BUILD.bazel", build)])),
        counts(&[
            ("pkg/BUILD.bazel", "flaky", 1),
            ("pkg/BUILD.bazel", "no-size", 1),
            ("pkg/BUILD.bazel", "size-enormous", 1),
            ("pkg/BUILD.bazel", "size-large", 2),
            ("pkg/BUILD.bazel", "size-medium", 1),
            ("pkg/BUILD.bazel", "small-timeout", 1),
            ("pkg/BUILD.bazel", "timeout-eternal", 1),
            ("pkg/BUILD.bazel", "timeout-long", 1),
        ]),
    );
}

#[test]
fn only_test_calls_that_pass_a_name_count() {
    let defs = r#"
_probe = rule(implementation = _impl, test = True)

def probe(name, size = None, **kwargs):
    _probe(name = name, size = size)
    _probe(name = name + "_spread", **kwargs)
    _probe(name = name + "_bare")

def plain(name, **kwargs):
    native.filegroup(name = name, **kwargs)
"#;
    let build = r#"
probe(name = "a")
plain(name = "b", timeout = "eternal", flaky = True)
cc_library(name = "c", size = "large")
native.test_suite(name = "d", tests = [":a"])
check_test("e", size = "large")
"#;
    assert_eq!(
        budget(&scan_sources(&[
            ("pkg/defs.bzl", defs),
            ("pkg/BUILD.bazel", build)
        ])),
        counts(&[
            ("pkg/BUILD.bazel", "no-size", 1),
            ("pkg/defs.bzl", "no-size", 1),
        ]),
    );
}

#[test]
fn the_allowlist_rejects_new_hits_and_spare_lines() {
    let found = counts(&[
        ("a/BUILD.bazel", "size-large", 3),
        ("b/BUILD.bazel", "flaky", 1),
    ]);
    let allowed = counts(&[
        ("a/BUILD.bazel", "size-large", 2),
        ("b/BUILD.bazel", "flaky", 2),
        ("c/BUILD.bazel", "no-size", 1),
    ]);
    assert_eq!(
        differences(&found, &allowed, FIX),
        [
            "a/BUILD.bazel: 3 `size-large` on tests, 2 allowed; make the test small",
            "b/BUILD.bazel: 1 `flaky` left; lower the allowlist count from 2",
            "c/BUILD.bazel: no `no-size` left; delete the allowlist line",
        ],
    );
    assert_eq!(differences(&found, &found, FIX), Vec::<String>::new());
    for kind in KINDS {
        let line = format!("a/BUILD.bazel {kind} 1\n");
        assert_eq!(parse_allowlist(&line, &KINDS).1, Vec::<String>::new());
    }
    for bad in [
        "a/BUILD.bazel size-small 1\n",
        "a/BUILD.bazel flaky 0\n",
        "a/BUILD.bazel flaky\n",
        "b/BUILD.bazel flaky 1\na/BUILD.bazel flaky 1\n",
    ] {
        assert!(!parse_allowlist(bad, &KINDS).1.is_empty(), "{bad:?}");
    }
}

#[test]
fn nested_tests_are_found_by_tag_constant_and_macro() {
    let tags = r#"
NESTED_TAGS = ["resources:cpu:2", "resources:nested_bazel:1"]
NETWORK_TAGS = NESTED_TAGS + ["x"]
"#;
    let defs = r#"
load("//tools:tags.bzl", "NESTED_TAGS")

def nested_test(name, **kwargs):
    rust_test(name = name, tags = NESTED_TAGS, **kwargs)

def qualification(name, smoke = False, **kwargs):
    if not smoke:
        rust_test(name = name, size = "small")
        return
    rust_test(name = name, tags = ["resources:nested_bazel:1"], **kwargs)
"#;
    let build = r#"
load("//tools:tags.bzl", TAGS = "NETWORK_TAGS")
rust_test(name = "literal", tags = ["resources:nested_bazel:1"])
rust_test(name = "constant", tags = TAGS + ["y"])
rust_test(name = "plain", tags = ["resources:cpu:2"])
rust_library(name = "library", tags = TAGS)
rust_test(name = "wrapped_upstream", tags = TAGS)
nested_test(name = "macro")
qualification(name = "golden")
qualification(name = "smoke", smoke = True)
"#;
    let files = scan_sources(&[
        ("tools/tags.bzl", tags),
        ("tools/defs.bzl", defs),
        ("pkg/BUILD.bazel", build),
    ]);
    assert_eq!(
        nested_tests(&files),
        labels(&[
            "//pkg:constant",
            "//pkg:literal",
            "//pkg:macro",
            "//pkg:smoke"
        ]),
    );
}

#[test]
fn the_nested_list_rejects_missing_spare_and_extra_lines() {
    let found = labels(&["//a:one", "//b:two"]);
    let listed = labels(&["//b:two", "//c:three"]);
    assert_eq!(
        nested_differences(&found, &listed),
        [
            "//a:one: nested test without a line; add `//a:one <seconds>`",
            "//c:three: no nested test left; delete the line",
        ],
    );
    assert_eq!(nested_differences(&found, &found), Vec::<String>::new());
    let grown: BTreeSet<String> = (0..=NESTED_CAP)
        .map(|index| format!("//a:{index}"))
        .collect();
    assert_eq!(
        nested_differences(&grown, &grown),
        ["9 nested tests listed, 8 allowed; test the logic without a nested Bazel"],
    );
}

#[test]
fn the_nested_list_format_is_strict() {
    let (listed, problems) = parse_nested("//a:one 12\n//b:two 653\n");
    assert_eq!(problems, Vec::<String>::new());
    assert_eq!(listed, labels(&["//a:one", "//b:two"]));
    for bad in [
        "//a:one\n",
        "//a:one 0\n",
        "//a:one fast\n",
        "a:one 12\n",
        "# comment\n",
        "//b:two 1\n//a:one 1\n",
        "//a:one 1\n//a:one 1\n",
    ] {
        assert!(!parse_nested(bad).1.is_empty(), "{bad:?}");
    }
}
