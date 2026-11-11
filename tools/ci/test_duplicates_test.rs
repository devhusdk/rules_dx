//! No BUILD file holds two test calls with the same callee and the same arguments apart from `name`, `tags` and `flaky`.

#[allow(dead_code)]
mod test_calls;

use std::collections::BTreeMap;

use starlark_syntax::syntax::AstModule;
use starlark_syntax::syntax::ast::{ArgumentP, AstArgument, AstExpr, AstStmt, ExprP, StmtP};
use test_calls::{Callees, callee_name, is_test, parse, read_input, string, test_callees};
use yaml_serde as _;

/// The rule that groups tests.
const SUITE: &str = "test_suite";

/// The arguments that do not make two calls different.
const IGNORED: [&str; 3] = ["name", "tags", "flaky"];

/// Fewer scanned files than this means the sweep lost the tree.
const MIN_FILES: usize = 1000;

/// One call that passes a string `name`.
struct Call {
    callee: String,
    name: String,
    line: usize,
    key: Option<Vec<String>>,
}

/// What one file contributes.
#[derive(Default)]
struct FileScan {
    callees: Callees,
    calls: Vec<Call>,
}

/// The arguments of a call that decide whether it repeats another, or none when a spread hides them.
fn key(args: &[AstArgument]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for arg in args {
        match &arg.node {
            ArgumentP::Named(name, value) => {
                if !IGNORED.contains(&name.node.as_str()) {
                    let text: String = value
                        .to_string()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect();
                    out.push(format!("{}={text}", name.node));
                }
            }
            ArgumentP::Positional(_) | ArgumentP::Args(_) | ArgumentP::KwArgs(_) => return None,
        }
    }
    out.sort();
    Some(out)
}

struct Walker<'a> {
    ast: &'a AstModule,
    scan: &'a mut FileScan,
    def: Option<String>,
}

impl Walker<'_> {
    fn stmt(&mut self, stmt: &AstStmt) {
        match &stmt.node {
            StmtP::Statements(stmts) => stmts.iter().for_each(|child| self.stmt(child)),
            StmtP::Def(def) => {
                let outer = self.def.replace(def.name.node.ident.clone());
                self.scan.callees.def(&def.name.node.ident);
                self.stmt(&def.body);
                self.def = outer;
            }
            StmtP::Assign(assign) => {
                self.scan.callees.rule(&assign.lhs, &assign.rhs);
                self.expr(&assign.rhs);
            }
            StmtP::AssignModify(_, _, rhs) => self.expr(rhs),
            StmtP::If(condition, body) => {
                self.expr(condition);
                self.stmt(body);
            }
            StmtP::IfElse(condition, branches) => {
                self.expr(condition);
                self.stmt(&branches.0);
                self.stmt(&branches.1);
            }
            StmtP::For(each) => {
                self.expr(&each.over);
                self.stmt(&each.body);
            }
            StmtP::Return(Some(expr)) | StmtP::Expression(expr) => self.expr(expr),
            _ => {}
        }
    }

    fn expr(&mut self, expr: &AstExpr) {
        let ExprP::Call(callee, args) = &expr.node else {
            expr.visit_expr(|child| self.expr(child));
            return;
        };
        let callee_text = callee_name(callee);
        self.scan.callees.call(self.def.as_deref(), &callee_text);
        let name = args.args.iter().find_map(|arg| match &arg.node {
            ArgumentP::Named(key, value) if key.node == "name" => string(value),
            _ => None,
        });
        if let (Some(name), None) = (name, &self.def) {
            self.scan.calls.push(Call {
                callee: callee_text,
                name: name.to_owned(),
                line: self.ast.file_span(expr.span).resolve_span().begin.line + 1,
                key: key(&args.args),
            });
        }
        self.expr(callee);
        for arg in &args.args {
            self.expr(arg.node.expr());
        }
    }
}

fn scan_file(path: &str, source: &str) -> FileScan {
    let ast = parse(path, source);
    let mut scan = FileScan::default();
    Walker {
        ast: &ast,
        scan: &mut scan,
        def: None,
    }
    .stmt(ast.statement());
    scan
}

fn build_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name == "BUILD.bazel" || name == "BUILD"
}

fn scan_sources(sources: &[(&str, &str)]) -> BTreeMap<String, FileScan> {
    sources
        .iter()
        .map(|(path, source)| ((*path).to_owned(), scan_file(path, source)))
        .collect()
}

/// One line per test call that repeats an earlier call of the same BUILD file.
fn duplicates(files: &BTreeMap<String, FileScan>) -> Vec<String> {
    let tests = test_callees(files.values().map(|scan| &scan.callees));
    let mut out = Vec::new();
    for (path, scan) in files.iter().filter(|(path, _)| build_file(path)) {
        let mut seen: BTreeMap<(&str, &Vec<String>), &str> = BTreeMap::new();
        for call in &scan.calls {
            let Some(key) = &call.key else {
                continue;
            };
            if call.callee == SUITE || !is_test(&tests, &call.callee) {
                continue;
            }
            if let Some(first) = seen.insert((&call.callee, key), &call.name) {
                out.push(format!(
                    "{path}:{} {} repeats {first}; delete one of them",
                    call.line, call.name
                ));
            }
        }
    }
    out
}

#[test]
fn the_tree_has_no_duplicate_tests() {
    let sources: BTreeMap<String, String> =
        serde_json::from_str(&read_input("DX_SOURCES_JSON")).expect("sources snapshot");
    let files: BTreeMap<String, FileScan> = sources
        .iter()
        .map(|(path, source)| (path.clone(), scan_file(path, source)))
        .collect();
    assert!(
        files.len() >= MIN_FILES,
        "only {} BUILD and .bzl files in the sources snapshot",
        files.len()
    );
    let found = duplicates(&files);
    assert!(
        found.is_empty(),
        "duplicate test calls:\n{}",
        found.join("\n")
    );
}

#[test]
fn identical_calls_are_reported_whatever_the_tags() {
    let build = r#"
rust_test(name = "a", srcs = ["a.rs"], size = "small")
rust_test(name = "b", srcs = [ "a.rs" ], size = "small", tags = ["x"], flaky = True)
"#;
    assert_eq!(
        duplicates(&scan_sources(&[("pkg/BUILD.bazel", build)])),
        ["pkg/BUILD.bazel:3 b repeats a; delete one of them"],
    );
}

#[test]
fn different_arguments_or_callees_are_not_reported() {
    let build = r#"
cc_toolchain_test(name = "a", test = "//x:one")
cc_toolchain_test(name = "b", test = "//x:two")
cc_toolchain_test(name = "c", test = "//x:one", compiler = "gcc")
rust_test(name = "d", test = "//x:one")
rust_test(name = "e", test = "//x:one", size = "large")
rust_test(name = "f", test = "//x:one", size = "small")
rust_test(name = "g", test = "//x:one", size = "small", timeout = "long")
"#;
    assert_eq!(
        duplicates(&scan_sources(&[("pkg/BUILD.bazel", build)])),
        Vec::<String>::new(),
    );
}

#[test]
fn suites_non_tests_spreads_and_other_files_are_skipped() {
    let build = r#"
test_suite(name = "a", tests = [":x"])
test_suite(name = "b", tests = [":x"])
cc_library(name = "c", srcs = ["c.cc"])
cc_library(name = "d", srcs = ["c.cc"])
rust_test(name = "e", **kwargs)
rust_test(name = "f", **kwargs)
"#;
    let defs = r#"
rust_test(name = "g", srcs = ["g.rs"])
rust_test(name = "h", srcs = ["g.rs"])
"#;
    assert_eq!(
        duplicates(&scan_sources(&[
            ("pkg/BUILD.bazel", build),
            ("pkg/defs.bzl", defs)
        ])),
        Vec::<String>::new(),
    );
}
