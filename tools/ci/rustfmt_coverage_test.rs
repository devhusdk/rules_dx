//! Every Rust target declared in a listed BUILD file is in some rustfmt_test's targets.
//!
//! The check reads each package's BUILD.bazel (from its :corpus_starlark) and parses it with
//! starlark_syntax. It sees direct calls to the Rust rules (and comprehensions over them), not
//! targets that a macro creates inside a .bzl file. dx_rust_crate makes its own rustfmt_test,
//! so it is not a checked kind. The wasm and wasi test wrappers are not checked either.
//!
//! Known gap: a new package with Rust targets is not seen until it is added to PACKAGES (and its
//! :corpus_starlark to this test's data). Packages that set `# gazelle:dx_corpus false` have no
//! :corpus_starlark and are left out.
//!
//! The evaluator fails closed: a name or targets value it cannot evaluate, or a Rust rule call it
//! cannot see (inside a def, an if, a for statement or another call), is a finding.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use dx_testing::runfiles_root;
use starlark_syntax::codemap::Span;
use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstArgument, AstExpr, AstLiteral, BinOp, ExprP, StmtP,
};
use starlark_syntax::syntax::{AstModule, Dialect};

/// rules_dx Rust rule wrappers and the upstream rules_rust ones.
const RUST_MODULES: [&str; 3] = [
    "//rust/rules:defs.bzl",
    "@rules_dx//rust/rules:defs.bzl",
    "@rules_rust//rust:defs.bzl",
];

/// rules_rust's cargo rules.
const CARGO_MODULES: [&str; 1] = ["@rules_rust//cargo:defs.bzl"];

/// A rule whose targets must be covered: the symbol, the modules it loads from, and the suffix
/// that turns the call's name into the label of the Rust crate it builds.
struct Kind {
    symbol: &'static str,
    modules: &'static [&'static str],
    suffix: &'static str,
}

const KINDS: [Kind; 7] = [
    Kind {
        symbol: "rust_library",
        modules: &RUST_MODULES,
        suffix: "",
    },
    Kind {
        symbol: "rust_binary",
        modules: &RUST_MODULES,
        suffix: "",
    },
    Kind {
        symbol: "rust_test",
        modules: &RUST_MODULES,
        suffix: "",
    },
    Kind {
        symbol: "rust_proc_macro",
        modules: &RUST_MODULES,
        suffix: "",
    },
    Kind {
        symbol: "rust_shared_library",
        modules: &RUST_MODULES,
        suffix: "",
    },
    Kind {
        symbol: "rust_static_library",
        modules: &RUST_MODULES,
        suffix: "",
    },
    // cargo_build_script builds its script as the manual `<name>_` rust_binary.
    Kind {
        symbol: "cargo_build_script",
        modules: &CARGO_MODULES,
        suffix: "_",
    },
];

/// A rule whose `targets` attr covers Rust targets. Keyed by kind so rust_clippy_test can join.
struct Checker {
    symbol: &'static str,
    modules: &'static [&'static str],
}

const CHECKERS: [Checker; 1] = [Checker {
    symbol: "rustfmt_test",
    modules: &RUST_MODULES,
}];

/// Rust targets that are deliberately not format-checked, with the reason.
const EXEMPT: [(&str, &str); 4] = [
    (
        "//deploy/rules:pypi_probe_verify",
        "incompatible with every platform; its verify_test.rs is covered by the other verify tests",
    ),
    (
        "//rust/tests/fixtures/hello:incompatible_cdylib",
        "incompatible with every platform by design, so a rustfmt_test on it never runs",
    ),
    (
        "//quality/testdata:fixture_upstream_rust",
        "quality fixture input, checked by the quality tests themselves",
    ),
    (
        "//quality/testdata:fixture_upstream_rust_dirty",
        "deliberately unformatted quality fixture input",
    ),
];

/// Packages whose BUILD.bazel declares Rust targets; their :corpus_starlark is in the test data.
const PACKAGES: [&str; 132] = [
    ".devcontainer",
    "android/sdk/tests/fetch",
    "android/tests/fixtures/native_lib",
    "android/tests/fixtures/rust_apk",
    "appimage/packer",
    "appimage/tests",
    "apple/tests",
    "cc/context",
    "cc/tests",
    "cli/advisory_prep",
    "cli/cli",
    "cli/cli/tests/affected",
    "cli/cli/tests/agents",
    "cli/cli/tests/cargo_sets",
    "cli/cli/tests/latency",
    "cli/cli/tests/npm_sets",
    "cli/cli/tests/results_consumer",
    "cli/cli/tests/startup_consistency",
    "cli/cli/tests/uv_sets",
    "cli/cli/tests/worktree_cache",
    "cli/entrypoint",
    "cli/env",
    "cli/shim",
    "cli/tool_resolve",
    "csharp/tests/fixtures/roslyn",
    "cuda/tests",
    "cuda/tests/starter",
    "dart/tests/negative",
    "deploy/install",
    "deploy/offline",
    "deploy/offline/bootstrap",
    "deploy/release",
    "deploy/release/tests",
    "deploy/rules",
    "deploy/tests/cross",
    "deploy/tests/fixtures/consumer_archive",
    "deploy/tests/runfiles",
    "elixir/tests/negative",
    "elixir/tests/starter",
    "env/env_shard",
    "env/stubs",
    "erlang/tests/negative",
    "erlang/tests/starter",
    "examples/c-app/cli",
    "examples/c-app/diagnostics",
    "examples/c-app/store",
    "examples/cpp-app/app",
    "examples/cpp-app/core",
    "examples/cpp-app/diagnostics",
    "examples/cuda-app/checks",
    "examples/java-app/cli",
    "examples/java-app/diagnostics",
    "examples/kotlin-app/cli",
    "examples/kotlin-app/diagnostics",
    "examples/lua-app/diagnostics",
    "examples/node-app/cli",
    "examples/node-app/diagnostics",
    "examples/qt-rust/ledger",
    "examples/shared-rust",
    "examples/zig-app/diagnostics",
    "fortran/tests/negative",
    "gazelle/java",
    "gazelle/kotlin",
    "gazelle/rust",
    "gazelle/scala",
    "generation/codegen_shard",
    "generation/result",
    "haskell/tests/negative",
    "javascript/tests/fixtures/web_app",
    "julia/tests/starter",
    "lua/tests/negative",
    "modules",
    "ocaml/tests/negative",
    "perl/tests/negative",
    "php/tests/negative",
    "python/tests/consumer",
    "qt/rust",
    "qt/tests",
    "quality",
    "quality/adapter",
    "quality/markdown",
    "quality/result",
    "quality/runner",
    "quality/testdata",
    "quality/tests",
    "quality/tools/javascript/launcher",
    "rust/tests/fixtures/cc_optout",
    "rust/tests/fixtures/consumer_abi",
    "rust/tests/fixtures/consumer_hub",
    "rust/tests/fixtures/consumer_native",
    "rust/tests/fixtures/consumer_std",
    "rust/tests/fixtures/consumer_wasi",
    "rust/tests/fixtures/consumer_wasm",
    "rust/tests/fixtures/cxx_identity",
    "rust/tests/fixtures/hello",
    "rust/tests/fixtures/native_call",
    "rust/tests/fixtures/wasm_bindgen",
    "tools/bazelrc",
    "tools/ci",
    "tools/coverage",
    "tools/coverage/collect",
    "tools/coverage/dotnet",
    "tools/depcheck",
    "tools/docs/go",
    "tools/docs/go/extract",
    "tools/docs/ir/ir",
    "tools/docs/jvm",
    "tools/docs/jvm/convert",
    "tools/docs/powershell",
    "tools/docs/powershell/extract",
    "tools/docs/dotnet/extract",
    "tools/docs/ruby",
    "tools/docs/ruby/extract",
    "tools/docs/rust",
    "tools/docs/rust/extract",
    "tools/docs/rust/fixture",
    "tools/docs/site",
    "tools/docs/site/aggregate",
    "tools/docs/site/browser",
    "tools/docs/site/build",
    "tools/docs/site/check",
    "tools/docs/support_matrix",
    "tools/docs/typescript",
    "tools/docs/typescript/extract",
    "tools/ide",
    "tools/repin_all",
    "tools/testing",
    "tools/testing/hermetic",
    "tools/testing/load",
    "tools/testing/process_probe",
    "typescript/tests/fixtures/scoped",
    "wasi/testing",
];

/// Fewer packages than this means the list lost its data.
const MIN_PACKAGES: usize = 120;

/// Fewer Rust targets than this means the sweep lost its data.
const MIN_TARGETS: usize = 400;

/// One Rust target a BUILD file declares.
#[derive(Debug)]
struct Target {
    label: String,
    line: usize,
    covered: bool,
}

/// What one BUILD file contributes.
#[derive(Default, Debug)]
struct Scan {
    targets: Vec<Target>,
    findings: Vec<String>,
}

/// A value the small evaluator understands.
#[derive(Clone, Debug)]
enum Value {
    Str(String),
    List(Vec<Value>),
}

/// What a load()ed symbol is to this check.
#[derive(Clone, Copy)]
enum Role {
    Rule(&'static str),
    Checker,
}

/// A call to a Rust rule, before coverage is known.
struct Declared {
    name: String,
    line: usize,
    crate_only: Option<String>,
}

/// Module-level state for one BUILD file.
struct Module<'a> {
    ast: &'a AstModule,
    path: &'a str,
    pkg: &'a str,
    roles: BTreeMap<String, Role>,
    consts: BTreeMap<String, Option<&'a AstExpr>>,
    seen: HashSet<Span>,
    declared: Vec<Declared>,
    covered: BTreeSet<String>,
    findings: Vec<String>,
}

type Bindings = BTreeMap<String, Value>;

impl<'a> Module<'a> {
    fn line(&self, expr: &AstExpr) -> usize {
        self.ast.file_span(expr.span).resolve_span().begin.line + 1
    }

    fn fail(&mut self, expr: &AstExpr, message: String) {
        let line = self.line(expr);
        self.findings
            .push(format!("{}:{line}: {message}", self.path));
    }

    fn eval(&self, expr: &AstExpr, env: &Bindings, depth: usize) -> Result<Value, String> {
        if depth > 32 {
            return Err("expression nests too deep".to_owned());
        }
        match &expr.node {
            ExprP::Literal(AstLiteral::String(text)) => Ok(Value::Str(text.node.clone())),
            ExprP::List(items) | ExprP::Tuple(items) => items
                .iter()
                .map(|item| self.eval(item, env, depth + 1))
                .collect::<Result<_, _>>()
                .map(Value::List),
            ExprP::Identifier(ident) => {
                let name = ident.node.ident.as_str();
                if let Some(value) = env.get(name) {
                    return Ok(value.clone());
                }
                match self.consts.get(name) {
                    Some(Some(value)) => self.eval(value, &Bindings::new(), depth + 1),
                    Some(None) => Err(format!("{name} is assigned more than once")),
                    None => Err(format!("{name} is not a module-level constant")),
                }
            }
            ExprP::Op(lhs, BinOp::Add, rhs) => {
                match (
                    self.eval(lhs, env, depth + 1)?,
                    self.eval(rhs, env, depth + 1)?,
                ) {
                    (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
                    (Value::List(mut a), Value::List(b)) => {
                        a.extend(b);
                        Ok(Value::List(a))
                    }
                    _ => Err("+ mixes a string and a list".to_owned()),
                }
            }
            ExprP::Op(lhs, BinOp::Percent, rhs) => {
                let Value::Str(format) = self.eval(lhs, env, depth + 1)? else {
                    return Err("% needs a string on the left".to_owned());
                };
                let args = match self.eval(rhs, env, depth + 1)? {
                    Value::Str(arg) => vec![arg],
                    Value::List(items) => items
                        .into_iter()
                        .map(|item| match item {
                            Value::Str(arg) => Ok(arg),
                            Value::List(_) => Err("% argument is a list".to_owned()),
                        })
                        .collect::<Result<_, _>>()?,
                };
                percent(&format, &args)
            }
            ExprP::ListComprehension(body, clause, rest) => {
                if !rest.is_empty() {
                    return Err("comprehension has more than one clause".to_owned());
                }
                let mut out = Vec::new();
                for bound in self.bind(&clause.var, &clause.over, env, depth)? {
                    out.push(self.eval(body, &bound, depth + 1)?);
                }
                Ok(Value::List(out))
            }
            _ => {
                Err("expression is not a string, list, constant, +, % or comprehension".to_owned())
            }
        }
    }

    /// One binding set per item of `over`, for a single-identifier or tuple loop variable.
    fn bind(
        &self,
        var: &starlark_syntax::syntax::ast::AstAssignTarget,
        over: &AstExpr,
        env: &Bindings,
        depth: usize,
    ) -> Result<Vec<Bindings>, String> {
        let Value::List(items) = self.eval(over, env, depth + 1)? else {
            return Err("comprehension iterates over a string".to_owned());
        };
        items
            .into_iter()
            .map(|item| {
                let mut bound = env.clone();
                assign(&var.node, item, &mut bound)?;
                Ok(bound)
            })
            .collect()
    }

    /// Records the Rust rule or checker `call`, evaluated under `env`.
    fn call(&mut self, call: &'a AstExpr, env: &Bindings) {
        let ExprP::Call(callee, args) = &call.node else {
            return;
        };
        let ExprP::Identifier(ident) = &callee.node else {
            return;
        };
        let Some(role) = self.roles.get(ident.node.ident.as_str()).copied() else {
            return;
        };
        self.seen.insert(call.span);
        match role {
            Role::Rule(suffix) => {
                self.rule(call, &args.args, suffix, ident.node.ident.as_str(), env)
            }
            Role::Checker => self.checker(call, &args.args, env),
        }
    }

    fn rule(
        &mut self,
        call: &AstExpr,
        args: &[AstArgument],
        suffix: &str,
        symbol: &str,
        env: &Bindings,
    ) {
        let name = match named_arg(args, "name").map(|value| self.eval(value, env, 0)) {
            Some(Ok(Value::Str(name))) => name,
            Some(Ok(Value::List(_))) => return self.fail(call, format!("{symbol} name is a list")),
            Some(Err(why)) => return self.fail(call, format!("{symbol} name: {why}")),
            None => return self.fail(call, format!("{symbol} has no name")),
        };
        let crate_only = match (named_arg(args, "crate"), named_arg(args, "srcs")) {
            (Some(krate), None) if symbol != "cargo_build_script" => {
                match self.eval(krate, env, 0) {
                    Ok(Value::Str(krate)) => Some(krate),
                    Ok(Value::List(_)) => {
                        return self.fail(call, format!("{symbol} crate is a list"))
                    }
                    Err(why) => return self.fail(call, format!("{symbol} crate: {why}")),
                }
            }
            _ => None,
        };
        let line = self.line(call);
        self.declared.push(Declared {
            name: name + suffix,
            line,
            crate_only,
        });
    }

    fn checker(&mut self, call: &AstExpr, args: &[AstArgument], env: &Bindings) {
        let Some(targets) = named_arg(args, "targets") else {
            return self.fail(call, "checker has no targets".to_owned());
        };
        match self.eval(targets, env, 0) {
            Ok(Value::List(items)) => {
                for item in items {
                    match item {
                        Value::Str(label) => {
                            if let Some(name) = local_name(self.pkg, &label) {
                                self.covered.insert(name);
                            }
                        }
                        Value::List(_) => {
                            return self.fail(call, "checker targets holds a list".to_owned());
                        }
                    }
                }
            }
            Ok(Value::Str(_)) => self.fail(call, "checker targets is a string".to_owned()),
            Err(why) => self.fail(call, format!("checker targets: {why}")),
        }
    }

    /// Handles one top-level expression statement: a call, or a comprehension of calls.
    fn statement(&mut self, expr: &'a AstExpr) {
        match &expr.node {
            ExprP::Call(..) => self.call(expr, &Bindings::new()),
            ExprP::ListComprehension(body, clause, rest) if rest.is_empty() => {
                if !matches!(&body.node, ExprP::Call(..)) {
                    return;
                }
                match self.bind(&clause.var, &clause.over, &Bindings::new(), 0) {
                    Ok(bindings) => {
                        // A comprehension over nothing still counts as seen.
                        if let ExprP::Call(callee, _) = &body.node {
                            if matches!(&callee.node, ExprP::Identifier(id) if self.roles.contains_key(id.node.ident.as_str()))
                            {
                                self.seen.insert(body.span);
                            }
                        }
                        for bound in bindings {
                            self.call(body, &bound);
                        }
                    }
                    Err(why) => {
                        if let ExprP::Call(callee, _) = &body.node {
                            if matches!(&callee.node, ExprP::Identifier(id) if self.roles.contains_key(id.node.ident.as_str()))
                            {
                                self.seen.insert(body.span);
                                self.fail(expr, format!("comprehension: {why}"));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Binds the loop variable `var` to `value`.
fn assign(
    var: &AssignTargetP<starlark_syntax::syntax::ast::AstNoPayload>,
    value: Value,
    env: &mut Bindings,
) -> Result<(), String> {
    match (var, value) {
        (AssignTargetP::Identifier(ident), value) => {
            env.insert(ident.node.ident.clone(), value);
            Ok(())
        }
        (AssignTargetP::Tuple(vars), Value::List(values)) if vars.len() == values.len() => {
            for (var, value) in vars.iter().zip(values) {
                assign(&var.node, value, env)?;
            }
            Ok(())
        }
        _ => Err("loop variable does not match its value".to_owned()),
    }
}

/// `format % args` for %s only.
fn percent(format: &str, args: &[String]) -> Result<Value, String> {
    let mut out = String::new();
    let mut args = args.iter();
    let mut chars = format.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('s') => out.push_str(args.next().ok_or("% has too few arguments")?),
            Some('%') => out.push('%'),
            _ => return Err("% supports only %s".to_owned()),
        }
    }
    if args.next().is_some() {
        return Err("% has too many arguments".to_owned());
    }
    Ok(Value::Str(out))
}

/// The target name `label` names in `pkg`, or None for a label in another package.
fn local_name(pkg: &str, label: &str) -> Option<String> {
    if let Some(name) = label.strip_prefix(':') {
        return Some(name.to_owned());
    }
    let rest = label
        .strip_prefix("@rules_dx//")
        .or_else(|| label.strip_prefix("//"));
    let Some(rest) = rest else {
        return Some(label.to_owned());
    };
    let (target_pkg, name) = match rest.split_once(':') {
        Some((target_pkg, name)) => (target_pkg, name),
        None => (rest, rest.rsplit('/').next().unwrap_or(rest)),
    };
    (target_pkg == pkg).then(|| name.to_owned())
}

/// The value of the named argument `key`, if passed.
fn named_arg<'a>(args: &'a [AstArgument], key: &str) -> Option<&'a AstExpr> {
    args.iter().find_map(|arg| match &arg.node {
        ArgumentP::Named(arg_key, value) if arg_key.node == key => Some(value),
        _ => None,
    })
}

/// Every top-level statement, with nested statement lists flattened.
fn top_level<'a>(
    stmt: &'a starlark_syntax::syntax::ast::AstStmt,
    out: &mut Vec<&'a starlark_syntax::syntax::ast::AstStmt>,
) {
    match &stmt.node {
        StmtP::Statements(stmts) => stmts.iter().for_each(|stmt| top_level(stmt, out)),
        _ => out.push(stmt),
    }
}

/// Reports every call to a tracked symbol that the walk did not see.
fn unseen(expr: &AstExpr, module: &Module, out: &mut Vec<(usize, String)>) {
    if let ExprP::Call(callee, _) = &expr.node {
        if let ExprP::Identifier(ident) = &callee.node {
            let name = ident.node.ident.as_str();
            if module.roles.contains_key(name) && !module.seen.contains(&expr.span) {
                out.push((module.line(expr), name.to_owned()));
            }
        }
    }
    expr.node.visit_expr(|child| unseen(child, module, out));
}

/// Scans the BUILD file of `pkg` at `path`.
fn scan(pkg: &str, path: &str, source: &str) -> Scan {
    let ast = match AstModule::parse(path, source.to_owned(), &Dialect::Extended) {
        Ok(ast) => ast,
        Err(error) => {
            return Scan {
                targets: Vec::new(),
                findings: vec![format!(
                    "{path}: does not parse: {}",
                    error.without_diagnostic()
                )],
            };
        }
    };
    let mut stmts = Vec::new();
    top_level(ast.statement(), &mut stmts);
    let mut module = Module {
        ast: &ast,
        path,
        pkg,
        roles: BTreeMap::new(),
        consts: BTreeMap::new(),
        seen: HashSet::new(),
        declared: Vec::new(),
        covered: BTreeSet::new(),
        findings: Vec::new(),
    };
    for stmt in &stmts {
        match &stmt.node {
            StmtP::Load(load) => {
                for arg in &load.args {
                    let their = arg.their.node.as_str();
                    let module_path = load.module.node.as_str();
                    let role = KINDS
                        .iter()
                        .find(|kind| kind.symbol == their && kind.modules.contains(&module_path))
                        .map(|kind| Role::Rule(kind.suffix))
                        .or_else(|| {
                            CHECKERS
                                .iter()
                                .any(|c| c.symbol == their && c.modules.contains(&module_path))
                                .then_some(Role::Checker)
                        });
                    if let Some(role) = role {
                        module.roles.insert(arg.local.node.ident.clone(), role);
                    }
                }
            }
            StmtP::Assign(assign) => {
                if let AssignTargetP::Identifier(ident) = &assign.lhs.node {
                    let name = ident.node.ident.clone();
                    let value = (!module.consts.contains_key(&name)).then_some(&assign.rhs);
                    module.consts.insert(name, value);
                }
            }
            StmtP::AssignModify(lhs, _, _) => {
                if let AssignTargetP::Identifier(ident) = &lhs.node {
                    module.consts.insert(ident.node.ident.clone(), None);
                }
            }
            _ => {}
        }
    }
    for stmt in &stmts {
        if let StmtP::Expression(expr) = &stmt.node {
            module.statement(expr);
        }
    }
    let mut missed = Vec::new();
    ast.statement()
        .node
        .visit_expr(|expr| unseen(expr, &module, &mut missed));
    for (line, name) in missed {
        module.findings.push(format!(
            "{path}:{line}: {name}() is called where this check cannot see it; call it at the top level"
        ));
    }
    let covered = module.covered;
    let targets = module
        .declared
        .into_iter()
        .map(|declared| {
            let by_crate = declared
                .crate_only
                .as_deref()
                .and_then(|krate| local_name(pkg, krate))
                .is_some_and(|krate| covered.contains(&krate));
            Target {
                covered: covered.contains(&declared.name) || by_crate,
                label: format!("//{pkg}:{}", declared.name),
                line: declared.line,
            }
        })
        .collect();
    Scan {
        targets,
        findings: module.findings,
    }
}

/// Findings for `scans`: uncovered targets that are not exempt and stale exempt entries.
fn findings(scans: &[(String, Scan)], exempt: &[(&str, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut labels = BTreeMap::new();
    for (path, scan) in scans {
        out.extend(scan.findings.iter().cloned());
        for target in &scan.targets {
            labels.insert(target.label.as_str(), target.covered);
            if !target.covered && !exempt.iter().any(|(label, _)| *label == target.label) {
                out.push(format!(
                    "{path}:{}: {} is in no rustfmt_test; add it to one in its package",
                    target.line, target.label
                ));
            }
        }
    }
    for (label, _) in exempt {
        match labels.get(label) {
            None => out.push(format!("EXEMPT {label} names no Rust target; drop it")),
            Some(true) => out.push(format!("EXEMPT {label} is now covered; drop it")),
            Some(false) => {}
        }
    }
    out
}

#[test]
fn every_rust_target_is_in_a_rustfmt_test() {
    let root = runfiles_root();
    let mut scans = Vec::new();
    let mut problems = Vec::new();
    for pkg in PACKAGES {
        let path = format!("{pkg}/BUILD.bazel");
        let source = std::fs::read_to_string(root.join(&path))
            .unwrap_or_else(|error| panic!("read {path}: {error}"));
        let result = scan(pkg, &path, &source);
        if result.targets.is_empty() && result.findings.is_empty() {
            problems.push(format!(
                "{path}: declares no Rust targets; drop it from PACKAGES"
            ));
        }
        scans.push((path, result));
    }
    let targets: usize = scans.iter().map(|(_, scan)| scan.targets.len()).sum();
    assert!(
        PACKAGES.len() >= MIN_PACKAGES,
        "only {} packages are listed",
        PACKAGES.len()
    );
    assert!(
        targets >= MIN_TARGETS,
        "only {targets} Rust targets were checked"
    );
    problems.extend(findings(&scans, &EXEMPT));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[cfg(test)]
fn check(source: &str, exempt: &[(&str, &str)]) -> Vec<String> {
    let path = "pkg/BUILD.bazel".to_owned();
    let result = scan("pkg", &path, source);
    findings(&[(path, result)], exempt)
}

#[cfg(test)]
const LOAD: &str = "load(\"//rust/rules:defs.bzl\", \"rust_binary\", \"rust_library\", \"rust_test\", \"rustfmt_test\")\n";

#[test]
fn a_covered_literal_target_passes() {
    let source = format!(
        "{LOAD}rust_library(name = \"lib\", srcs = [\"lib.rs\"])\nrustfmt_test(name = \"fmt\", targets = [\":lib\"])\n"
    );
    assert_eq!(check(&source, &[]), Vec::<String>::new());
}

#[test]
fn a_missing_target_is_reported() {
    let source = format!(
        "{LOAD}rust_library(name = \"lib\", srcs = [\"lib.rs\"])\nrust_binary(name = \"bin\", srcs = [\"main.rs\"])\nrustfmt_test(name = \"fmt\", targets = [\"//pkg:lib\"])\n"
    );
    assert_eq!(
        check(&source, &[]),
        ["pkg/BUILD.bazel:3: //pkg:bin is in no rustfmt_test; add it to one in its package"]
    );
}

#[test]
fn a_label_in_another_package_does_not_cover() {
    let source = format!(
        "{LOAD}rust_library(name = \"lib\", srcs = [\"lib.rs\"])\nrustfmt_test(name = \"fmt\", targets = [\"//other:lib\"])\n"
    );
    assert_eq!(check(&source, &[]).len(), 1);
}

#[test]
fn comprehensions_on_both_sides_are_evaluated() {
    let source = format!(
        "{LOAD}_CASES = [\"a\", \"b\"]\n_ALL = [\"lib\"] + _CASES\n[\n    rust_test(name = \"case_%s_test\" % case, srcs = [\"%s.rs\" % case])\n    for case in _CASES\n]\nrustfmt_test(name = \"fmt\", targets = [\":case_%s_test\" % c for c in _CASES])\n"
    );
    assert_eq!(check(&source, &[]), Vec::<String>::new());
    let partial = source.replace("for c in _CASES]", "for c in [\"a\"]]");
    assert_eq!(
        check(&partial, &[]),
        ["pkg/BUILD.bazel:5: //pkg:case_b_test is in no rustfmt_test; add it to one in its package"]
    );
}

#[test]
fn a_build_script_is_covered_under_its_underscore_name() {
    let source = format!(
        "load(\"@rules_rust//cargo:defs.bzl\", \"cargo_build_script\")\n{LOAD}cargo_build_script(name = \"bs\", srcs = [\"build.rs\"])\nrustfmt_test(name = \"fmt\", targets = [\":bs\"])\n"
    );
    assert_eq!(
        check(&source, &[]),
        ["pkg/BUILD.bazel:3: //pkg:bs_ is in no rustfmt_test; add it to one in its package"]
    );
    let fixed = source.replace("[\":bs\"]", "[\":bs_\"]");
    assert_eq!(check(&fixed, &[]), Vec::<String>::new());
}

#[test]
fn a_crate_only_test_follows_its_crate() {
    let source = format!(
        "{LOAD}rust_library(name = \"lib\", srcs = [\"lib.rs\"])\nrust_test(name = \"lib_test\", crate = \":lib\")\nrustfmt_test(name = \"fmt\", targets = [\":lib\"])\n"
    );
    assert_eq!(check(&source, &[]), Vec::<String>::new());
    let with_srcs = source.replace("crate = \":lib\"", "crate = \":lib\", srcs = [\"t.rs\"]");
    assert_eq!(check(&with_srcs, &[]).len(), 1);
}

#[test]
fn a_load_alias_is_resolved() {
    let source = "load(\"@rules_rust//rust:defs.bzl\", my_test = \"rust_test\", fmt = \"rustfmt_test\")\nmy_test(name = \"t\", srcs = [\"t.rs\"])\n";
    assert_eq!(
        check(source, &[]),
        ["pkg/BUILD.bazel:2: //pkg:t is in no rustfmt_test; add it to one in its package"]
    );
    let covered = format!("{source}fmt(name = \"f\", targets = [\"t\"])\n");
    assert_eq!(check(&covered, &[]), Vec::<String>::new());
}

#[test]
fn dx_rust_crate_and_other_modules_are_skipped() {
    let source = "load(\"//rust/rules:crate.bzl\", \"dx_rust_crate\")\nload(\"//other:defs.bzl\", \"rust_test\")\ndx_rust_crate(name = \"c\")\nrust_test(name = \"t\")\n";
    let result = scan("pkg", "pkg/BUILD.bazel", source);
    assert!(result.targets.is_empty());
    assert!(result.findings.is_empty());
}

#[test]
fn an_expression_it_cannot_evaluate_fails() {
    let source = format!(
        "{LOAD}rust_test(name = \"t\" + str(1), srcs = [\"t.rs\"])\nrustfmt_test(name = \"fmt\", targets = glob([\"*\"]))\n"
    );
    let found = check(&source, &[]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].starts_with("pkg/BUILD.bazel:2: rust_test name:"),
        "{found:?}"
    );
    assert!(
        found[1].starts_with("pkg/BUILD.bazel:3: checker targets:"),
        "{found:?}"
    );
}

#[test]
fn a_call_it_cannot_see_fails() {
    let source = format!("{LOAD}def f():\n    rust_test(name = \"t\", srcs = [\"t.rs\"])\n");
    assert_eq!(
        check(&source, &[]),
        ["pkg/BUILD.bazel:3: rust_test() is called where this check cannot see it; call it at the top level"]
    );
}

#[test]
fn a_stale_exempt_entry_fails() {
    let source = format!(
        "{LOAD}rust_library(name = \"lib\", srcs = [\"lib.rs\"])\nrust_library(name = \"other\", srcs = [\"o.rs\"])\nrustfmt_test(name = \"fmt\", targets = [\":lib\"])\n"
    );
    assert_eq!(
        check(&source, &[("//pkg:other", "why")]),
        Vec::<String>::new()
    );
    assert_eq!(
        check(
            &source,
            &[
                ("//pkg:other", "why"),
                ("//pkg:lib", "why"),
                ("//pkg:gone", "why")
            ]
        ),
        [
            "EXEMPT //pkg:lib is now covered; drop it",
            "EXEMPT //pkg:gone names no Rust target; drop it",
        ]
    );
}
