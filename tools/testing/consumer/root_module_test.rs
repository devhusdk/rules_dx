//! The root MODULE.bazel and every `<lang>/tests/consumer` fixture follow docs/design/consumer-setup.md outside tools/testing/consumer/pending.txt, and the list holds no spare line.

use std::collections::{BTreeMap, BTreeSet};

use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstExpr, AstLiteral, AstStmt, ExprP, StmtP};
use starlark_syntax::syntax::module::AstModuleFields;
use starlark_syntax::syntax::{AstModule, Dialect};

/// The pending list named in failures.
const PENDING: &str = "tools/testing/consumer/pending.txt";

/// The file that lists the fixtures with a test.
const FIXTURES: &str = "tools/testing/consumer/fixtures.bzl";

/// Every kind a pending line may start with.
const KINDS: [&str; 5] = ["dev-registration", "dev-repo", "fixture", "init", "override"];

/// The directories whose files may name a repo that only a `dev_dependency` usage brings in.
const DEV_DIRS: [&str; 3] = ["examples", "tests", "tools"];

/// The label and name of the one extension a consumer `MODULE.bazel` uses.
const LICENSES: [&str; 2] = ["@rules_dx//core:extensions.bzl", "dx_licenses"];

/// Fewer languages than this means the snapshot lost the tree.
const MIN_LANGS: usize = 40;

/// What the root `MODULE.bazel` declares.
#[derive(Default)]
struct Root {
    overrides: BTreeSet<String>,
    dev_registrations: BTreeSet<String>,
    dev_repos: BTreeSet<String>,
    repos: BTreeSet<String>,
    dev_proxies: BTreeMap<String, bool>,
}

/// The snapshot files the guard reads.
#[derive(Default)]
struct Tree {
    /// The root `MODULE.bazel`.
    module: String,
    /// Each language directory and whether its manifest has `entry_points`.
    langs: BTreeMap<String, bool>,
    /// Every `BUILD.bazel` and `.bzl` file by path.
    sources: BTreeMap<String, String>,
    /// Every file under a `<lang>/tests/consumer` directory by path.
    consumer: BTreeMap<String, String>,
    /// The languages `CONSUMER_FIXTURES` lists.
    tested: BTreeSet<String>,
    /// The paths the root `.bazelignore` lists.
    ignored: BTreeSet<String>,
}

fn statements(path: &str, source: &str) -> Vec<AstStmt> {
    let ast = AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()));
    let (_, statement, _, _) = ast.into_parts();
    match statement.node {
        StmtP::Statements(stmts) => stmts,
        _ => vec![statement],
    }
}

fn string_literal(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(text.node.as_str()),
        _ => None,
    }
}

fn identifier(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Identifier(ident) => Some(ident.node.ident.as_str()),
        _ => None,
    }
}

/// The arguments of one call: positional ones in order, and named ones by name.
struct Args<'a> {
    positional: Vec<&'a AstExpr>,
    named: BTreeMap<&'a str, &'a AstExpr>,
}

impl<'a> Args<'a> {
    fn of(args: &'a [starlark_syntax::syntax::ast::AstArgument]) -> Self {
        let mut out = Self {
            positional: Vec::new(),
            named: BTreeMap::new(),
        };
        for arg in args {
            match &arg.node {
                ArgumentP::Positional(value) => out.positional.push(value),
                ArgumentP::Named(key, value) => {
                    out.named.insert(key.node.as_str(), value);
                }
                _ => {}
            }
        }
        out
    }

    fn text(&self, name: &str) -> Option<&'a str> {
        self.named.get(name).and_then(|value| string_literal(value))
    }

    fn dev(&self) -> bool {
        self.named
            .get("dev_dependency")
            .is_some_and(|value| identifier(value) == Some("True"))
    }

    /// The repo names a `use_repo`, `override_repo` or `inject_repo` call gives after its proxy.
    fn repos(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .positional
            .iter()
            .skip(1)
            .filter_map(|value| string_literal(value))
            .map(str::to_owned)
            .collect();
        out.extend(self.named.keys().map(|key| (*key).to_owned()));
        out
    }
}

fn scan_root(source: &str) -> Root {
    let mut root = Root::default();
    for stmt in statements("MODULE.bazel", source) {
        match &stmt.node {
            StmtP::Assign(assign) => {
                let AssignTargetP::Identifier(proxy) = &assign.lhs.node else {
                    continue;
                };
                let ExprP::Call(callee, args) = &assign.rhs.node else {
                    continue;
                };
                if identifier(callee) == Some("use_extension") {
                    root.dev_proxies
                        .insert(proxy.node.ident.clone(), Args::of(&args.args).dev());
                }
            }
            StmtP::Expression(expr) => {
                let ExprP::Call(callee, args) = &expr.node else {
                    continue;
                };
                let Some(name) = identifier(callee) else {
                    continue;
                };
                root_call(name, &Args::of(&args.args), &mut root);
            }
            _ => {}
        }
    }
    root
}

fn root_call(name: &str, args: &Args<'_>, root: &mut Root) {
    match name {
        "override_repo" | "inject_repo" => {
            for repo in args.repos() {
                root.overrides.insert(format!("{name} {repo}"));
            }
        }
        _ if name.ends_with("_override") => {
            let module = args.text("module_name").unwrap_or("?");
            root.overrides.insert(format!("{name} {module}"));
        }
        "register_toolchains" | "register_execution_platforms" => {
            if args.dev() {
                for value in &args.positional {
                    root.dev_registrations
                        .insert(string_literal(value).unwrap_or("?").to_owned());
                }
            }
        }
        "use_repo" => {
            let proxy = args.positional.first().and_then(|value| identifier(value));
            let dev = proxy.is_some_and(|proxy| root.dev_proxies.get(proxy) == Some(&true));
            for repo in args.repos() {
                root.declare(repo, dev);
            }
        }
        "bazel_dep" => {
            if let Some(repo) = args.text("repo_name").or_else(|| {
                args.text("name")
                    .filter(|_| !args.named.contains_key("repo_name"))
            }) {
                root.declare(repo.to_owned(), args.dev());
            }
        }
        _ => {
            if let Some(repo) = args.text("name") {
                root.declare(repo.to_owned(), args.dev());
            }
        }
    }
}

impl Root {
    fn declare(&mut self, repo: String, dev: bool) {
        if dev {
            self.dev_repos.insert(repo);
        } else {
            self.repos.insert(repo);
        }
    }

    /// The repos that only a `dev_dependency` usage brings in.
    fn dev_only(&self) -> BTreeSet<&str> {
        self.dev_repos
            .difference(&self.repos)
            .map(String::as_str)
            .collect()
    }
}

fn repo_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'+' | b'~')
}

/// The repos of `known` that `text` names as `@repo//` or `"@repo"`.
fn named_repos<'a>(text: &str, known: &BTreeSet<&'a str>) -> BTreeSet<&'a str> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'@' {
            at += 1;
            continue;
        }
        let start = at + 1;
        let mut end = start;
        while end < bytes.len() && repo_byte(bytes[end]) {
            end += 1;
        }
        let closes = matches!(bytes.get(end), Some(b'/' | b'"' | b'\''));
        if closes && let Some(repo) = known.get(&text[start..end]) {
            out.insert(*repo);
        }
        at = end.max(start);
    }
    out
}

fn consumer_lang(path: &str) -> Option<&str> {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        [lang, "tests", "consumer", _, ..] => Some(lang),
        _ => None,
    }
}

fn dev_dir(path: &str) -> bool {
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    consumer_lang(path).is_none() && parts.iter().any(|part| DEV_DIRS.contains(part))
}

/// What a consumer `MODULE.bazel` holds besides `module`, the `rules_dx` `bazel_dep` and license tags.
fn consumer_module_problems(path: &str, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut licenses: Option<String> = None;
    for stmt in statements(path, source) {
        let allowed = match &stmt.node {
            StmtP::Assign(assign) => match (&assign.lhs.node, &assign.rhs.node) {
                (AssignTargetP::Identifier(proxy), ExprP::Call(callee, args)) => {
                    let args = Args::of(&args.args);
                    let texts: Vec<_> = args.positional.iter().map(|v| string_literal(v)).collect();
                    let licensed = identifier(callee) == Some("use_extension")
                        && args.named.is_empty()
                        && texts == [Some(LICENSES[0]), Some(LICENSES[1])];
                    if licensed {
                        licenses = Some(proxy.node.ident.clone());
                    }
                    licensed
                }
                _ => false,
            },
            StmtP::Expression(expr) => match &expr.node {
                ExprP::Call(callee, args) => {
                    let args = Args::of(&args.args);
                    match &callee.node {
                        ExprP::Identifier(name) if name.node.ident == "module" => true,
                        ExprP::Identifier(name) if name.node.ident == "bazel_dep" => {
                            args.text("name") == Some("rules_dx")
                                && args.named.keys().all(|key| matches!(*key, "name" | "version"))
                        }
                        ExprP::Dot(object, tag) => {
                            tag.node == "accept" && identifier(object) == licenses.as_deref()
                        }
                        _ => false,
                    }
                }
                _ => false,
            },
            _ => false,
        };
        if !allowed {
            out.push(format!(
                "{path}: holds more than module, the rules_dx bazel_dep and license tags: {}",
                source[stmt.span.begin().get() as usize..stmt.span.end().get() as usize]
                    .lines()
                    .next()
                    .unwrap_or_default()
            ));
        }
    }
    out
}

/// The loads of a fixture `BUILD.bazel` that are not `@rules_dx//<lang>:defs.bzl`.
fn fixture_load_problems(path: &str, source: &str) -> Vec<String> {
    statements(path, source)
        .iter()
        .filter_map(|stmt| match &stmt.node {
            StmtP::Load(load) => Some(load.module.node.clone()),
            _ => None,
        })
        .filter(|module| {
            module
                .strip_prefix("@rules_dx//")
                .and_then(|rest| rest.strip_suffix(":defs.bzl"))
                .is_none_or(|lang| lang.is_empty() || lang.contains('/'))
        })
        .map(|module| format!("{path}: loads {module}, not @rules_dx//<lang>:defs.bzl"))
        .collect()
}

/// The pending lines the tree needs, besides `init` lines, and the problems no line allows.
fn lint(tree: &Tree) -> (BTreeSet<String>, Vec<String>) {
    let mut lines = BTreeSet::new();
    let mut problems = Vec::new();
    let root = scan_root(&tree.module);
    for found in &root.overrides {
        lines.insert(format!("override {found}"));
    }
    for label in &root.dev_registrations {
        lines.insert(format!("dev-registration {label}"));
    }
    let dev_only = root.dev_only();
    for (path, text) in tree.sources.iter().chain(&tree.consumer) {
        if dev_dir(path) {
            continue;
        }
        for repo in named_repos(text, &dev_only) {
            lines.insert(format!("dev-repo {repo} {path}"));
        }
    }
    let mut fixtures = BTreeSet::new();
    for (lang, entry_points) in &tree.langs {
        let dir = format!("{lang}/tests/consumer");
        let module = format!("{dir}/consumer.MODULE.bazel");
        match tree.consumer.get(&module) {
            Some(text) => {
                fixtures.insert(lang.clone());
                problems.extend(consumer_module_problems(&module, text));
                if !tree.consumer.contains_key(&format!("{dir}/BUILD.bazel")) {
                    problems.push(format!("{dir}/BUILD.bazel: missing"));
                }
                if !tree.ignored.contains(&dir) {
                    problems.push(format!("{dir}: missing from .bazelignore"));
                }
            }
            None if *entry_points => {
                lines.insert(format!("fixture {lang}"));
            }
            None => {}
        }
    }
    for (path, text) in &tree.consumer {
        let in_fixture = consumer_lang(path).is_some_and(|lang| fixtures.contains(lang));
        if in_fixture && path.ends_with("/BUILD.bazel") {
            problems.extend(fixture_load_problems(path, text));
        }
    }
    for lang in fixtures.symmetric_difference(&tree.tested) {
        problems.push(format!(
            "{lang}: CONSUMER_FIXTURES in {FIXTURES} lists exactly the languages with a {lang}/tests/consumer/consumer.MODULE.bazel"
        ));
    }
    (lines, problems)
}

/// The well-formed lines of the pending list, and what is wrong with the others.
fn parse_pending(text: &str) -> (BTreeSet<String>, Vec<String>) {
    let mut lines = BTreeSet::new();
    let mut problems = Vec::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let well_formed = match fields.as_slice() {
            ["fixture" | "init" | "dev-registration", name] => !name.is_empty(),
            ["override" | "dev-repo", first, second] => !first.is_empty() && !second.is_empty(),
            _ => false,
        };
        if !well_formed || !KINDS.contains(&fields[0]) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        if previous.is_some_and(|before| before >= line) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(line);
        lines.insert(line.to_owned());
    }
    (lines, problems)
}

/// What differs between the tree and the pending list.
fn differences(tree: &Tree, pending: &str) -> Vec<String> {
    let (allowed, mut out) = parse_pending(pending);
    let (found, problems) = lint(tree);
    out.extend(problems);
    for line in found.difference(&allowed) {
        out.push(format!("{line}: not allowed; fix it"));
    }
    for line in allowed.difference(&found) {
        match line.strip_prefix("init ") {
            Some(lang) if tree.tested.contains(lang) => {}
            Some(lang) => out.push(format!(
                "{line}: {lang} has no fixture test; delete the line from {PENDING}"
            )),
            None => out.push(format!(
                "{line}: no such violation; delete the line from {PENDING}"
            )),
        }
    }
    out
}

fn snapshot(variable: &str) -> BTreeMap<String, String> {
    let path = dx_testing::rlocation(dx_testing::env_text(variable).trim())
        .unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::serde_json::from_str(&dx_testing::read_text(&path))
        .unwrap_or_else(|error| panic!("{variable}: {error}"))
}

fn snapshot_tree() -> Tree {
    let mut files = snapshot("DX_FILES_JSON");
    let langs = files
        .iter()
        .filter_map(|(path, text)| {
            let lang = path.strip_suffix("/lang.json").filter(|lang| !lang.contains('/'))?;
            let manifest: dx_testing::serde_json::Value = dx_testing::serde_json::from_str(text)
                .unwrap_or_else(|error| panic!("{path}: {error}"));
            let entry_points = manifest["entry_points"]
                .as_array()
                .is_some_and(|names| !names.is_empty());
            Some((lang.to_owned(), entry_points))
        })
        .collect();
    let ignored = files
        .get(".bazelignore")
        .expect(".bazelignore is in the snapshot")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_end_matches('/').to_owned())
        .collect();
    Tree {
        module: files.remove("MODULE.bazel").expect("MODULE.bazel is in the snapshot"),
        langs,
        sources: snapshot("DX_SOURCES_JSON"),
        consumer: snapshot("DX_CONSUMER_JSON"),
        tested: dx_testing::env_text("DX_CONSUMER_FIXTURES")
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
        ignored,
    }
}

#[test]
fn root_module_and_fixtures_follow_the_consumer_setup_or_are_pending() {
    let tree = snapshot_tree();
    assert!(
        tree.langs.len() >= MIN_LANGS,
        "only {} language directories in the snapshot",
        tree.langs.len()
    );
    let pending = dx_testing::read_text(&dx_testing::env_file("DX_CONSUMER_PENDING"));
    let problems = differences(&tree, &pending);
    assert!(
        problems.is_empty(),
        "{} consumer setup problems against {PENDING}; rules: docs/design/consumer-setup.md\n{}",
        problems.len(),
        problems.join("\n")
    );
}

const TOY_ROOT: &str = r#"module(name = "rules_dx", version = "0.0.0")

bazel_dep(name = "rules_toy", version = "1.0")
bazel_dep(name = "rules_probe", version = "1.0", dev_dependency = True)
bazel_dep(name = "rules_renamed", version = "1.0", repo_name = "renamed", dev_dependency = True)
bazel_dep(name = "rules_hidden", version = "1.0", repo_name = None, dev_dependency = True)

toy = use_extension("//toy:extensions.bzl", "toy")
use_repo(toy, "toy_sdk", "toy_shared")

toy_dev = use_extension("//toy:extensions.bzl", "toy", dev_dependency = True)
use_repo(toy_dev, "toy_shared", "toy_test_hub", mapped = "toy_other")

_snapshot = use_repo_rule("//tools:repos.bzl", "snapshot")

_snapshot(
    name = "toy_snapshot",
    dev_dependency = True,
)

register_toolchains("//toy:all")
"#;

const TOY_CONSUMER_MODULE: &str = "# dx: yours; dx adds its missing lines\nmodule(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const TOY_CONSUMER_BUILD: &str = "load(\"@rules_dx//toy:defs.bzl\", \"toy_library\")\n\ntoy_library(name = \"consumer\")\n";

fn toy() -> Tree {
    let text = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect::<BTreeMap<String, String>>()
    };
    Tree {
        module: TOY_ROOT.to_owned(),
        langs: [("toy".to_owned(), true), ("notes".to_owned(), false)].into(),
        sources: text(&[
            ("toy/defs.bzl", "load(\"@toy_sdk//:defs.bzl\", \"sdk\")\n"),
            ("toy/tests/BUILD.bazel", "x(data = [\"@toy_test_hub//:all\", \"@rules_probe\"])\n"),
            ("tools/toy/BUILD.bazel", "x(data = [\"@toy_snapshot//:files.json\"])\n"),
            ("examples/toy/BUILD.bazel", "x(data = [\"@mapped//:all\"])\n"),
            ("toy/tests/consumer/BUILD.bazel", TOY_CONSUMER_BUILD),
        ]),
        consumer: text(&[
            ("toy/tests/consumer/BUILD.bazel", TOY_CONSUMER_BUILD),
            ("toy/tests/consumer/consumer.MODULE.bazel", TOY_CONSUMER_MODULE),
            ("toy/tests/consumer/toy.toy", ""),
        ]),
        tested: ["toy".to_owned()].into(),
        ignored: ["toy/tests/consumer".to_owned()].into(),
    }
}

fn broken(edit: impl FnOnce(&mut Tree)) -> Vec<String> {
    let mut tree = toy();
    edit(&mut tree);
    differences(&tree, "init toy\n")
}

#[test]
fn a_conforming_tree_has_no_problem() {
    assert_eq!(broken(|_| {}), Vec::<String>::new());
    assert_eq!(differences(&toy(), ""), Vec::<String>::new());
}

#[test]
fn every_override_needs_a_line() {
    let found = broken(|tree| {
        tree.module.push_str(
            "single_version_override(module_name = \"rules_toy\", version = \"1.0\")\narchive_override(module_name = \"rules_probe\", urls = [])\noverride_repo(toy, \"toy_sdk\", toy_shared = \"other\")\ninject_repo(toy, \"injected\")\n",
        );
    });
    assert_eq!(
        found,
        [
            "override archive_override rules_probe: not allowed; fix it",
            "override inject_repo injected: not allowed; fix it",
            "override override_repo toy_sdk: not allowed; fix it",
            "override override_repo toy_shared: not allowed; fix it",
            "override single_version_override rules_toy: not allowed; fix it",
        ]
    );
}

#[test]
fn a_dev_registration_needs_a_line() {
    let found = broken(|tree| {
        tree.module.push_str(
            "register_toolchains(\"//toy/tests:all\", dev_dependency = True)\nregister_execution_platforms(\"//toy/tests:host\", dev_dependency = True)\nregister_execution_platforms(\"//toy:host\")\n",
        );
    });
    assert_eq!(
        found,
        [
            "dev-registration //toy/tests:all: not allowed; fix it",
            "dev-registration //toy/tests:host: not allowed; fix it",
        ]
    );
}

#[test]
fn a_dev_only_repo_is_named_only_under_tests_tools_and_examples() {
    let found = broken(|tree| {
        tree.sources.insert(
            "toy/private/defs.bzl".to_owned(),
            "A = [\"@toy_test_hub//:all\", \"@@toy_snapshot//:x\", \"@rules_probe\", \"@renamed//:x\", \"@mapped//:x\"]\nB = [\"@toy_shared//:x\", \"@toy_sdk//:x\", \"@rules_toy//:x\", \"@rules_hidden//:x\", \"@toy_test_hub_other//:x\"]\n".to_owned(),
        );
        tree.consumer.insert(
            "toy/tests/consumer/pkg/BUILD.bazel".to_owned(),
            "load(\"@rules_dx//toy:defs.bzl\", \"toy_test\")\n\ntoy_test(deps = [\"@toy_test_hub//:all\"])\n".to_owned(),
        );
    });
    assert_eq!(
        found,
        [
            "dev-repo mapped toy/private/defs.bzl: not allowed; fix it",
            "dev-repo renamed toy/private/defs.bzl: not allowed; fix it",
            "dev-repo rules_probe toy/private/defs.bzl: not allowed; fix it",
            "dev-repo toy_snapshot toy/private/defs.bzl: not allowed; fix it",
            "dev-repo toy_test_hub toy/private/defs.bzl: not allowed; fix it",
            "dev-repo toy_test_hub toy/tests/consumer/pkg/BUILD.bazel: not allowed; fix it",
        ]
    );
}

#[test]
fn a_language_with_entry_points_needs_a_fixture_or_a_line() {
    let missing = |tree: &mut Tree| {
        tree.consumer.clear();
        tree.tested.clear();
    };
    assert_eq!(
        broken(missing),
        [
            "fixture toy: not allowed; fix it",
            "init toy: toy has no fixture test; delete the line from tools/testing/consumer/pending.txt",
        ]
    );
    let mut tree = toy();
    missing(&mut tree);
    assert_eq!(differences(&tree, "fixture toy\n"), Vec::<String>::new());
    assert_eq!(
        differences(&toy(), "fixture toy\n"),
        ["fixture toy: no such violation; delete the line from tools/testing/consumer/pending.txt"]
    );
}

#[test]
fn every_fixture_has_a_build_file_and_a_test() {
    let found = broken(|tree| {
        tree.consumer.remove("toy/tests/consumer/BUILD.bazel");
        tree.tested.clear();
    });
    assert_eq!(
        found,
        [
            "toy/tests/consumer/BUILD.bazel: missing",
            "toy: CONSUMER_FIXTURES in tools/testing/consumer/fixtures.bzl lists exactly the languages with a toy/tests/consumer/consumer.MODULE.bazel",
            "init toy: toy has no fixture test; delete the line from tools/testing/consumer/pending.txt",
        ]
    );
}

#[test]
fn every_fixture_is_in_the_bazelignore() {
    assert_eq!(
        broken(|tree| {
            tree.ignored.clear();
        }),
        ["toy/tests/consumer: missing from .bazelignore"]
    );
}

#[test]
fn a_consumer_module_holds_only_module_the_rules_dx_dep_and_license_tags() {
    let licensed = format!(
        "{TOY_CONSUMER_MODULE}\ndx_licenses = use_extension(\"@rules_dx//core:extensions.bzl\", \"dx_licenses\")\ndx_licenses.accept(ids = [\"toy-eula\"])\n"
    );
    assert_eq!(
        consumer_module_problems("m", &licensed),
        Vec::<String>::new()
    );
    let extra = format!(
        "{licensed}bazel_dep(name = \"rules_toy\", version = \"1.0\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", dev_dependency = True)\nsingle_version_override(module_name = \"rules_dx\")\ntoy = use_extension(\"@rules_dx//toy:extensions.bzl\", \"toy\")\ntoy.hub(lock = \"//:toy.lock\")\nuse_repo(toy, \"toy_hub\")\nregister_toolchains(\"//:all\")\ndx_licenses.other()\n"
    );
    let found: Vec<String> = consumer_module_problems("m", &extra)
        .iter()
        .map(|problem| problem.rsplit(": ").next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        found,
        [
            "bazel_dep(name = \"rules_toy\", version = \"1.0\")",
            "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", dev_dependency = True)",
            "single_version_override(module_name = \"rules_dx\")",
            "toy = use_extension(\"@rules_dx//toy:extensions.bzl\", \"toy\")",
            "toy.hub(lock = \"//:toy.lock\")",
            "use_repo(toy, \"toy_hub\")",
            "register_toolchains(\"//:all\")",
            "dx_licenses.other()",
        ]
    );
}

#[test]
fn a_fixture_build_file_loads_only_the_language_api() {
    let found = broken(|tree| {
        tree.consumer.insert(
            "toy/tests/consumer/BUILD.bazel".to_owned(),
            "load(\"@rules_dx//toy:defs.bzl\", \"toy_library\")\nload(\"@rules_dx//toy/private:defs.bzl\", \"a\")\nload(\"//toy:defs.bzl\", \"b\")\nload(\"@rules_toy//toy:defs.bzl\", \"c\")\n".to_owned(),
        );
    });
    assert_eq!(
        found,
        [
            "toy/tests/consumer/BUILD.bazel: loads @rules_dx//toy/private:defs.bzl, not @rules_dx//<lang>:defs.bzl",
            "toy/tests/consumer/BUILD.bazel: loads //toy:defs.bzl, not @rules_dx//<lang>:defs.bzl",
            "toy/tests/consumer/BUILD.bazel: loads @rules_toy//toy:defs.bzl, not @rules_dx//<lang>:defs.bzl",
        ]
    );
}

#[test]
fn pending_lines_are_well_formed_and_sorted() {
    let (lines, problems) = parse_pending(
        "fixture toy\ndev-repo a b\nfixture toy\nfixture toy\ninit\nother toy\noverride rules_toy\noverride single_version_override rules_toy\n",
    );
    assert_eq!(
        lines.into_iter().collect::<Vec<_>>(),
        [
            "dev-repo a b",
            "fixture toy",
            "override single_version_override rules_toy",
        ]
    );
    assert_eq!(
        problems,
        [
            "not sorted or repeated: \"dev-repo a b\"",
            "not sorted or repeated: \"fixture toy\"",
            "malformed line: \"init\"",
            "malformed line: \"other toy\"",
            "malformed line: \"override rules_toy\"",
        ]
    );
}
