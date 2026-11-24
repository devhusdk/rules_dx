use std::collections::{BTreeMap, BTreeSet};

use dx_guard::allowlist::{self, Counts, differences};
use dx_guard::snapshot::read_input;
use dx_guard::starlark::{callee_name, is_true, parse, string};
use dx_guard::test_calls::{Callees, is_test, test_callees};
use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstExpr, AstStmt, ExprP, StmtP};
use yaml_serde as _;

const NESTED_OVER: &str = "nested-cold-over-60";

const NESTED_SECONDS: usize = 60;

const KINDS: [&str; 9] = [
    "flaky",
    "nested-cold-over-60",
    "no-size",
    "size-enormous",
    "size-large",
    "size-medium",
    "small-timeout",
    "timeout-eternal",
    "timeout-long",
];

const SIZES: [&str; 3] = ["enormous", "large", "medium"];

const TIMEOUTS: [&str; 2] = ["eternal", "long"];

const NESTED_TAG: &str = "resources:nested_bazel";

const NESTED_CAP: usize = 8;

const PR_SKIPPED_TAGS: [&str; 2] = ["network-consumer", "audit"];

const AUDIT_TAGS: [&str; 2] = ["hermetic-consumer", "network-consumer"];

const AUDIT_WORKFLOW: &str = ".github/workflows/audit.yml";

const GATE_ACTION: &str = ".github/actions/gate/action.yml";

const UPSTREAM: &str = "_upstream";

const SUITE: &str = "test_suite";

const FIX: &str = "make the test small";

const ALLOWLIST: &str = "tools/ci/test_budget_allowlist.txt";

const NESTED_LIST: &str = "tools/testing/nested_smoke.txt";

const MIN_FILES: usize = 1000;

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
            Self::Unset | Self::Other => None,
        }
    }
}

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
    tag_strings: Vec<String>,
    tag_names: Vec<String>,
    guards: BTreeSet<String>,
}

#[derive(Default, Clone)]
struct Constant {
    strings: Vec<String>,
    names: BTreeSet<String>,
}

#[derive(Default)]
struct FileScan {
    callees: Callees,
    loads: BTreeMap<String, String>,
    constants: BTreeMap<String, Constant>,
    calls: Vec<Call>,
}

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

fn returns(stmt: &AstStmt) -> bool {
    match &stmt.node {
        StmtP::Return(_) => true,
        StmtP::Statements(stmts) => stmts.iter().any(returns),
        StmtP::If(_, body) => returns(body),
        StmtP::IfElse(_, branches) => returns(&branches.0) || returns(&branches.1),
        StmtP::For(each) => returns(&each.body),
        StmtP::Break
        | StmtP::Continue
        | StmtP::Pass
        | StmtP::Expression(_)
        | StmtP::Assign(_)
        | StmtP::AssignModify(..)
        | StmtP::Def(_)
        | StmtP::Load(_) => false,
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
                    constant.strings.extend(found.strings);
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
        let name = callee_name(callee).to_owned();
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
                            call.tag_strings = found.strings;
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

fn scan_file(path: &str, source: &str) -> FileScan {
    let ast = parse(path, source).unwrap_or_else(|error| panic!("{error}"));
    let mut scan = FileScan::default();
    Walker {
        scan: &mut scan,
        def: None,
        guards: Vec::new(),
    }
    .stmt(ast.statement(), true);
    scan
}

fn unsized_macro(callee: &str) -> bool {
    callee == "erlang_test_app"
        || callee.split('_').any(|word| word == "binary")
            && !callee.split('_').any(|word| word == "test")
}

fn budget(files: &BTreeMap<String, FileScan>) -> Counts {
    let tests = test_callees(&files.values().map(|scan| &scan.callees).collect::<Vec<_>>());
    let mut counts = Counts::new();
    for (path, scan) in files {
        let mut add = |kind: String| {
            *counts.entry((path.clone(), kind)).or_default() += 1;
        };
        for call in &scan.calls {
            if call.callee == SUITE || unsized_macro(&call.callee) || !is_test(&tests, &call.callee)
            {
                continue;
            }
            match &call.size {
                Attr::Unset if !call.spread => add("no-size".to_owned()),
                Attr::Text(size) if SIZES.contains(&size.as_str()) => add(format!("size-{size}")),
                Attr::Text(size) if size == "small" && !matches!(call.timeout, Attr::Unset) => {
                    add("small-timeout".to_owned());
                }
                Attr::Unset | Attr::Text(_) | Attr::Other => {}
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

fn pr_skipped(strings: &[String]) -> bool {
    strings
        .iter()
        .any(|text| text == "manual" || PR_SKIPPED_TAGS.contains(&text.as_str()))
}

fn audit_tag(strings: &[String]) -> bool {
    strings
        .iter()
        .any(|text| AUDIT_TAGS.contains(&text.as_str()))
}

fn pr_tag_filter() -> String {
    let skipped: Vec<String> = PR_SKIPPED_TAGS
        .iter()
        .map(|tag| format!("-{tag}"))
        .collect();
    format!("pr_tags={}", skipped.join(","))
}

fn nested_constants(
    files: &BTreeMap<String, FileScan>,
    hit: fn(&[String]) -> bool,
) -> BTreeSet<String> {
    let mut merged: BTreeMap<String, Constant> = BTreeMap::new();
    for scan in files.values() {
        for (name, constant) in &scan.constants {
            let entry = merged.entry(name.clone()).or_default();
            entry.strings.extend(constant.strings.iter().cloned());
            for inner in &constant.names {
                entry
                    .names
                    .insert(scan.loads.get(inner).unwrap_or(inner).clone());
            }
        }
    }
    let mut nested: BTreeSet<String> = merged
        .iter()
        .filter(|(_, constant)| hit(&constant.strings))
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

fn tagged(
    scan: &FileScan,
    call: &Call,
    constants: &BTreeSet<String>,
    hit: fn(&[String]) -> bool,
) -> bool {
    hit(&call.tag_strings)
        || call.tag_names.iter().any(|name| {
            let known = scan.constants.contains_key(name) || scan.loads.contains_key(name);
            known && constants.contains(scan.loads.get(name).unwrap_or(name))
        })
}

fn nested_macros(
    files: &BTreeMap<String, FileScan>,
    constants: &BTreeSet<String>,
    hit: fn(&[String]) -> bool,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut macros: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    loop {
        let mut grew = false;
        for scan in files.values() {
            for call in &scan.calls {
                let Some(def) = &call.def else {
                    continue;
                };
                if !tagged(scan, call, constants, hit) && !macros.contains_key(&call.callee) {
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

fn tests_tagged(
    files: &BTreeMap<String, FileScan>,
    hit: fn(&[String]) -> bool,
) -> BTreeSet<String> {
    let tests = test_callees(&files.values().map(|scan| &scan.callees).collect::<Vec<_>>());
    let constants = nested_constants(files, hit);
    let macros = nested_macros(files, &constants, hit);
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
            let direct = tagged(scan, call, &constants, hit) && is_test(&tests, &call.callee);
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

fn nested_tests(files: &BTreeMap<String, FileScan>) -> BTreeSet<String> {
    tests_tagged(files, nested_tag)
}

fn unselected(
    files: &BTreeMap<String, FileScan>,
    listed: &BTreeSet<String>,
    audit: &str,
) -> Vec<String> {
    let skipped = tests_tagged(files, pr_skipped);
    let audited = tests_tagged(files, audit_tag);
    listed
        .iter()
        .filter(|label| {
            skipped.contains(*label) && !audited.contains(*label) && !audit.contains(label.as_str())
        })
        .map(|label| {
            format!(
                "{label}: the pull request check job skips it and no audit job runs it; drop its \
                 manual, {} tag, tag it {}, or name it in {AUDIT_WORKFLOW}",
                PR_SKIPPED_TAGS.join(" or "),
                AUDIT_TAGS.join(" or ")
            )
        })
        .collect()
}

fn parse_nested(text: &str) -> (BTreeMap<String, usize>, Vec<String>) {
    let mut labels = BTreeMap::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [label, seconds] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let seconds = seconds.parse::<usize>().unwrap_or(0);
        if !label.starts_with("//") || seconds == 0 {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= *label) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(label);
        labels.insert((*label).to_owned(), seconds);
    }
    (labels, problems)
}

fn split_nested(allowed: Counts) -> (Counts, BTreeMap<String, usize>, Vec<String>) {
    let mut budget = Counts::new();
    let mut over = BTreeMap::new();
    let mut problems = Vec::new();
    for ((path, kind), count) in allowed {
        if kind != NESTED_OVER {
            budget.insert((path, kind), count);
        } else if path.starts_with("//") {
            over.insert(path, count);
        } else {
            problems.push(format!("{path}: `{NESTED_OVER}` needs a label"));
        }
    }
    (budget, over, problems)
}

fn nested_seconds(listed: &BTreeMap<String, usize>, over: &BTreeMap<String, usize>) -> Vec<String> {
    let mut out = Vec::new();
    for (label, seconds) in listed {
        if *seconds <= NESTED_SECONDS {
            continue;
        }
        match over.get(label) {
            None => out.push(format!(
                "{label}: {seconds} s cold, {NESTED_SECONDS} allowed; split the test"
            )),
            Some(limit) if seconds > limit => out.push(format!(
                "{label}: {seconds} s cold, {limit} allowed; split the test"
            )),
            Some(limit) if seconds < limit => out.push(format!(
                "{label}: {seconds} s cold; lower the allowlist seconds from {limit}"
            )),
            Some(_) => {}
        }
    }
    for label in over.keys() {
        match listed.get(label) {
            None => out.push(format!(
                "{label}: no nested list line; delete the allowlist line"
            )),
            Some(seconds) if *seconds <= NESTED_SECONDS => out.push(format!(
                "{label}: {seconds} s cold, at most {NESTED_SECONDS}; delete the allowlist line"
            )),
            Some(_) => {}
        }
    }
    out
}

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
    let (allowed, mut problems) = allowlist::parse(&read_input("TEST_BUDGET_ALLOWLIST"), &KINDS);
    let (allowed, over, split) = split_nested(allowed);
    problems.extend(split);
    problems.extend(differences(&budget(&files), &allowed, FIX));
    assert!(
        problems.is_empty(),
        "{ALLOWLIST} and the tree disagree:\n{}",
        problems.join("\n")
    );
    let (seconds, mut problems) = parse_nested(&read_input("NESTED_LIST"));
    let listed: BTreeSet<String> = seconds.keys().cloned().collect();
    problems.extend(nested_seconds(&seconds, &over));
    problems.extend(nested_differences(&nested_tests(&files), &listed));
    problems.extend(unselected(&files, &listed, &read_input("AUDIT_WORKFLOW")));
    let filter = pr_tag_filter();
    if !read_input("GATE_ACTION")
        .lines()
        .any(|line| line.trim() == filter)
    {
        problems.push(format!(
            "{GATE_ACTION} lacks `{filter}`, so the pull request check job runs the tests audit owns"
        ));
    }
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
            "a/BUILD.bazel size-large 3: 2 allowed; make the test small",
            "b/BUILD.bazel flaky 2: 1 left; lower the allowlist count",
            "c/BUILD.bazel no-size 1: none left; delete the allowlist line",
        ],
    );
    assert_eq!(differences(&found, &found, FIX), Vec::<String>::new());
    for kind in KINDS {
        let line = format!("a/BUILD.bazel {kind} 1\n");
        assert_eq!(allowlist::parse(&line, &KINDS).1, Vec::<String>::new());
    }
    for bad in [
        "a/BUILD.bazel size-small 1\n",
        "a/BUILD.bazel flaky 0\n",
        "a/BUILD.bazel flaky\n",
        "b/BUILD.bazel flaky 1\na/BUILD.bazel flaky 1\n",
    ] {
        assert!(!allowlist::parse(bad, &KINDS).1.is_empty(), "{bad:?}");
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
    assert_eq!(
        listed,
        BTreeMap::from([("//a:one".to_owned(), 12), ("//b:two".to_owned(), 653)]),
    );
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

#[test]
fn tests_the_pull_request_job_skips_need_an_audit_tag_or_line() {
    let defs = r#"
def smoke(name, **kwargs):
    rust_test(name = name, tags = ["hermetic-consumer", "audit"], **kwargs)
"#;
    let build = r#"
NETWORK = ["network-consumer"]
rust_test(name = "plain", tags = ["x"])
rust_test(name = "constant", tags = NETWORK)
rust_test(name = "named", tags = ["audit"])
rust_test(name = "orphan", tags = ["audit"])
rust_test(name = "hidden", tags = ["manual"])
smoke(name = "macro")
"#;
    let files = scan_sources(&[("tools/defs.bzl", defs), ("pkg/BUILD.bazel", build)]);
    let listed = labels(&[
        "//pkg:constant",
        "//pkg:hidden",
        "//pkg:macro",
        "//pkg:named",
        "//pkg:orphan",
        "//pkg:plain",
    ]);
    let problems = unselected(&files, &listed, "bazel test //pkg:named");
    let flagged: Vec<&str> = problems
        .iter()
        .filter_map(|problem| problem.split(':').take(2).last())
        .collect();
    assert_eq!(flagged, ["hidden", "orphan"], "{problems:?}");
    assert!(problems[1].contains(AUDIT_WORKFLOW), "{problems:?}");
}

#[test]
fn the_pull_request_tag_filter_names_every_skipped_tag() {
    assert_eq!(pr_tag_filter(), "pr_tags=-network-consumer,-audit");
}

fn seconds(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|(label, seconds)| ((*label).to_owned(), *seconds))
        .collect()
}

fn over(entries: &[(&str, usize)]) -> BTreeMap<String, usize> {
    entries
        .iter()
        .map(|(label, seconds)| ((*label).to_owned(), *seconds))
        .collect()
}

#[test]
fn nested_lines_at_or_under_the_limit_need_no_allowlist_line() {
    let listed = seconds(&[("//a:one", 33), ("//b:two", NESTED_SECONDS)]);
    assert_eq!(nested_seconds(&listed, &over(&[])), Vec::<String>::new());
}

#[test]
fn nested_lines_over_the_limit_need_an_allowlist_line() {
    let listed = seconds(&[("//a:one", 61), ("//b:two", 653)]);
    assert_eq!(
        nested_seconds(&listed, &over(&[("//b:two", 653)])),
        ["//a:one: 61 s cold, 60 allowed; split the test"],
    );
}

#[test]
fn nested_lines_over_the_allowlisted_seconds_fail() {
    let listed = seconds(&[("//a:one", 301)]);
    assert_eq!(
        nested_seconds(&listed, &over(&[("//a:one", 300)])),
        ["//a:one: 301 s cold, 300 allowed; split the test"],
    );
}

#[test]
fn nested_lines_under_the_allowlisted_seconds_lower_it() {
    let listed = seconds(&[("//a:one", 120)]);
    assert_eq!(
        nested_seconds(&listed, &over(&[("//a:one", 300)])),
        ["//a:one: 120 s cold; lower the allowlist seconds from 300"],
    );
}

#[test]
fn nested_allowlist_lines_without_a_nested_line_are_stale() {
    assert_eq!(
        nested_seconds(&seconds(&[]), &over(&[("//a:one", 300)])),
        ["//a:one: no nested list line; delete the allowlist line"],
    );
}

#[test]
fn nested_allowlist_lines_for_lines_at_or_under_the_limit_are_stale() {
    let listed = seconds(&[("//a:one", NESTED_SECONDS), ("//b:two", 33)]);
    assert_eq!(
        nested_seconds(&listed, &over(&[("//a:one", 300), ("//b:two", 300)])),
        [
            "//a:one: 60 s cold, at most 60; delete the allowlist line",
            "//b:two: 33 s cold, at most 60; delete the allowlist line",
        ],
    );
}

#[test]
fn nested_allowlist_lines_split_off_by_kind_and_need_a_label() {
    let allowed = counts(&[
        ("//a:one", NESTED_OVER, 300),
        ("a/BUILD.bazel", "flaky", 1),
        ("a/BUILD.bazel", NESTED_OVER, 300),
    ]);
    let (budget, over_lines, problems) = split_nested(allowed);
    assert_eq!(budget, counts(&[("a/BUILD.bazel", "flaky", 1)]));
    assert_eq!(over_lines, over(&[("//a:one", 300)]));
    assert_eq!(
        problems,
        ["a/BUILD.bazel: `nested-cold-over-60` needs a label"],
    );
}
