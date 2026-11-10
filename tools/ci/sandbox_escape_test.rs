//! No action leaves the sandbox, reaches the network or shares /tmp outside tools/ci/sandbox_escape_allowlist.txt, and the allowlist holds no spare entry.

mod guard_scan;

use std::collections::{BTreeMap, BTreeSet};

use guard_scan::{Counts, Token, callee_name, exprs, hit, is_starlark, parse, rust_tokens, string};
use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstExpr, AstStmt, ExprP, StmtP};
use yaml_serde as _;

/// The kinds an allowlist line may name.
const KINDS: [&str; 3] = ["exec-requirement", "rc-flag", "tmp-path"];

/// Execution requirements no action may carry.
const BANNED: [&str; 5] = [
    "local",
    "no-cache",
    "no-remote",
    "no-sandbox",
    "requires-network",
];

/// Words in a rule or macro name that mark it as a test.
const TEST_WORDS: [&str; 5] = ["bench", "fuzz", "suite", "test", "tests"];

/// The rc file that carries the `rc-flag` line.
const ROOT_RC: &str = ".bazelrc";

/// Flags the rc files must set, each with the spellings that set it.
const REQUIRED: [(&str, [&str; 2]); 2] = [
    (
        "--incompatible_strict_action_env",
        [
            "--incompatible_strict_action_env",
            "--incompatible_strict_action_env=true",
        ],
    ),
    (
        "--sandbox_default_allow_network=false",
        [
            "--sandbox_default_allow_network=false",
            "--nosandbox_default_allow_network",
        ],
    ),
];

/// What the files contribute before test names resolve across them.
#[derive(Default)]
struct Scan {
    counts: Counts,
    test_rules: BTreeSet<String>,
    defs: BTreeMap<String, BTreeSet<String>>,
    tagged: Vec<(String, String, usize)>,
}

/// Whether `name` is a banned requirement that reads as nothing else.
fn banned(name: &str) -> bool {
    name != "local" && BANNED.contains(&name)
}

/// Whether `name` reads as a test rule or macro.
fn test_name(name: &str) -> bool {
    name.split('_').any(|word| TEST_WORDS.contains(&word))
}

/// Whether `text` is `/tmp` or a path under it.
fn tmp(text: &str) -> bool {
    text == "/tmp" || text.starts_with("/tmp/")
}

/// Whether a line of shell or rc text names `/tmp` or a path under it.
fn tmp_line(line: &str) -> bool {
    let joins = |c: char| c.is_alphanumeric() || "_-.".contains(c);
    line.match_indices("/tmp").any(|(at, found)| {
        let before = line[..at].chars().next_back();
        let after = line[at + found.len()..].chars().next();
        !before.is_some_and(|c| joins(c) || "/$}".contains(c)) && !after.is_some_and(joins)
    })
}

fn is_true(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "True")
}

/// Whether `name` holds execution requirements.
fn names_requirements(name: &str) -> bool {
    name.contains("requirements")
}

fn statements<'a>(stmt: &'a AstStmt, out: &mut Vec<&'a AstStmt>) {
    out.push(stmt);
    stmt.visit_stmt(|child| statements(child, out));
}

/// Keys named `local` in the dict literals under `value`.
fn local_keys(value: &AstExpr) -> usize {
    let mut found = 0;
    if let ExprP::Dict(entries) = &value.node {
        found += entries
            .iter()
            .filter(|(key, _)| string(key) == Some("local"))
            .count();
    }
    value.visit_expr(|child| found += local_keys(child));
    found
}

impl Scan {
    fn requirement(&mut self, path: &str, by: usize) {
        for _ in 0..by {
            hit(&mut self.counts, path, "exec-requirement");
        }
    }

    fn starlark(&mut self, path: &str, source: &str) {
        let module = parse(path, source);
        let mut all = Vec::new();
        statements(module.statement(), &mut all);
        for stmt in all {
            match &stmt.node {
                StmtP::Def(def) => {
                    let callees = self.defs.entry(def.name.node.ident.clone()).or_default();
                    for expr in exprs(&def.body) {
                        if let ExprP::Call(callee, _) = &expr.node {
                            callees.insert(callee_name(callee).to_owned());
                        }
                    }
                }
                StmtP::Assign(assign) => match &assign.lhs.node {
                    AssignTargetP::Identifier(ident) => {
                        if test_rule(&assign.rhs) {
                            self.test_rules.insert(ident.node.ident.clone());
                        }
                        if names_requirements(&ident.node.ident) {
                            self.requirement(path, local_keys(&assign.rhs));
                        }
                    }
                    AssignTargetP::Index(pair) => {
                        let key = string(&pair.1).unwrap_or_default();
                        let holder = matches!(&pair.0.node, ExprP::Identifier(ident) if names_requirements(&ident.node.ident));
                        if banned(key) || (key == "local" && holder) {
                            self.requirement(path, 1);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        for expr in exprs(module.statement()) {
            match &expr.node {
                ExprP::Dict(entries) => {
                    let keys = entries
                        .iter()
                        .filter(|(key, _)| string(key).is_some_and(banned))
                        .count();
                    self.requirement(path, keys);
                }
                ExprP::Call(callee, args) => {
                    for arg in &args.args {
                        match &arg.node {
                            ArgumentP::Named(key, value)
                                if key.node == "execution_requirements" =>
                            {
                                self.requirement(path, local_keys(value));
                            }
                            ArgumentP::Named(key, value) if key.node == "tags" => {
                                let mut inner = Vec::new();
                                collect_strings(value, &mut inner);
                                let tags = inner.iter().filter(|tag| BANNED.contains(tag)).count();
                                if tags > 0 {
                                    self.tagged.push((
                                        path.to_owned(),
                                        callee_name(callee).to_owned(),
                                        tags,
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {
                    if string(expr).is_some_and(tmp) {
                        hit(&mut self.counts, path, "tmp-path");
                    }
                }
            }
        }
    }

    fn rust(&mut self, path: &str, source: &str) {
        let tokens = rust_tokens(source);
        for token in &tokens {
            if matches!(token, Token::Text(text) if tmp(text)) {
                hit(&mut self.counts, path, "tmp-path");
            }
        }
        if test_file(path) {
            return;
        }
        for _ in 0..temp_dir_calls(&product_tokens(tokens)) {
            hit(&mut self.counts, path, "tmp-path");
        }
    }

    fn lines(&mut self, path: &str, source: &str) {
        let rc = path.ends_with(".bazelrc");
        for line in source.lines() {
            let line = line.trim();
            if rc && line.starts_with('#') {
                continue;
            }
            if tmp_line(line) {
                hit(&mut self.counts, path, "tmp-path");
            }
            if !rc {
                continue;
            }
            for word in line.split_whitespace() {
                let Some(value) = word.strip_prefix("--modify_execution_info=") else {
                    continue;
                };
                let entries = value
                    .split(',')
                    .filter(|entry| {
                        entry
                            .rsplit_once("=+")
                            .is_some_and(|(_, tag)| BANNED.contains(&tag))
                    })
                    .count();
                self.requirement(path, entries);
            }
        }
    }

    fn file(&mut self, path: &str, source: &str) {
        if is_starlark(path) {
            self.starlark(path, source);
        } else if path.ends_with(".rs") {
            self.rust(path, source);
        } else {
            self.lines(path, source);
        }
    }

    /// Names that are tests: test-named callees, test rules, and macros that call one.
    fn tests(&self) -> BTreeSet<String> {
        let mut tests = self.test_rules.clone();
        loop {
            let before = tests.len();
            for (def, callees) in &self.defs {
                if callees
                    .iter()
                    .any(|callee| test_name(callee) || tests.contains(callee))
                {
                    tests.insert(def.clone());
                }
            }
            if tests.len() == before {
                return tests;
            }
        }
    }

    /// The counts, with banned tags on calls that are not test calls.
    fn finish(mut self) -> Counts {
        let tests = self.tests();
        for (path, callee, tags) in std::mem::take(&mut self.tagged) {
            if !test_name(&callee) && !tests.contains(&callee) {
                self.requirement(&path, tags);
            }
        }
        self.counts
    }
}

fn collect_strings<'a>(expr: &'a AstExpr, out: &mut Vec<&'a str>) {
    if let Some(text) = string(expr) {
        out.push(text);
    }
    expr.visit_expr(|child| collect_strings(child, out));
}

/// Whether `rhs` is `rule(..., test = True)`.
fn test_rule(rhs: &AstExpr) -> bool {
    let ExprP::Call(callee, args) = &rhs.node else {
        return false;
    };
    callee_name(callee) == "rule"
        && args.args.iter().any(|arg| {
            matches!(&arg.node, ArgumentP::Named(key, value) if key.node == "test" && is_true(value))
        })
}

/// Whether `path` names Rust test code.
fn test_file(path: &str) -> bool {
    path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
        || path.ends_with("/tests.rs")
        || path.contains("/tests/")
}

fn is_punct(token: Option<&Token>, wanted: char) -> bool {
    matches!(token, Some(Token::Punct(found)) if *found == wanted)
}

fn is_word(token: Option<&Token>, wanted: &str) -> bool {
    matches!(token, Some(Token::Word(found)) if found == wanted)
}

/// Whether `#[cfg(test)]` starts at `at`.
fn cfg_test(tokens: &[Token], at: usize) -> bool {
    is_punct(tokens.get(at), '#')
        && is_punct(tokens.get(at + 1), '[')
        && is_word(tokens.get(at + 2), "cfg")
        && is_punct(tokens.get(at + 3), '(')
        && is_word(tokens.get(at + 4), "test")
        && is_punct(tokens.get(at + 5), ')')
        && is_punct(tokens.get(at + 6), ']')
}

/// The tokens without the items under `#[cfg(test)]`.
fn product_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let mut skipped = vec![false; tokens.len()];
    let mut at = 0;
    while at < tokens.len() {
        if !cfg_test(&tokens, at) {
            at += 1;
            continue;
        }
        let start = at;
        let mut depth = 0usize;
        at += 7;
        while let Some(token) = tokens.get(at) {
            at += 1;
            match token {
                Token::Punct('{') => depth += 1,
                Token::Punct('}') => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                Token::Punct(';') if depth == 0 => break,
                _ => {}
            }
        }
        skipped[start..at].fill(true);
    }
    tokens
        .into_iter()
        .zip(skipped)
        .filter_map(|(token, skip)| (!skip).then_some(token))
        .collect()
}

/// Calls of `env::temp_dir`.
fn temp_dir_calls(tokens: &[Token]) -> usize {
    (0..tokens.len())
        .filter(|at| {
            is_word(tokens.get(*at), "env")
                && is_punct(tokens.get(at + 1), ':')
                && is_punct(tokens.get(at + 2), ':')
                && is_word(tokens.get(at + 3), "temp_dir")
                && is_punct(tokens.get(at + 4), '(')
        })
        .count()
}

/// Whether `path` is an rc file every build of this module reads.
fn module_rc(path: &str) -> bool {
    path == ROOT_RC || (path.starts_with(".dx/bazelrc/") && path.ends_with(".bazelrc"))
}

/// The required flags no rc file sets outside a config.
fn missing_flags(files: &BTreeMap<String, String>) -> Vec<&'static str> {
    let words: BTreeSet<&str> = files
        .iter()
        .filter(|(path, _)| module_rc(path))
        .flat_map(|(_, text)| text.lines())
        .map(str::trim)
        .filter(|line| {
            let command = line.split_whitespace().next().unwrap_or("#");
            !command.starts_with('#') && !command.contains(':')
        })
        .flat_map(str::split_whitespace)
        .collect();
    REQUIRED
        .iter()
        .filter(|(_, spellings)| !spellings.iter().any(|spelling| words.contains(spelling)))
        .map(|(flag, _)| *flag)
        .collect()
}

fn scan_files(files: &BTreeMap<String, String>) -> Counts {
    let mut scan = Scan::default();
    for (path, source) in files {
        scan.file(path, source);
    }
    for _ in missing_flags(files) {
        hit(&mut scan.counts, ROOT_RC, "rc-flag");
    }
    scan.finish()
}

fn scan_all(sources: &[(&str, &str)]) -> Vec<(String, String, usize)> {
    let mut scan = Scan::default();
    for (path, source) in sources {
        scan.file(path, source);
    }
    scan.finish()
        .into_iter()
        .map(|((path, kind), count)| (path, kind, count))
        .collect()
}

fn lines(entries: &[(&str, &str, usize)]) -> Vec<(String, String, usize)> {
    entries
        .iter()
        .map(|(path, kind, count)| ((*path).to_owned(), (*kind).to_owned(), *count))
        .collect()
}

fn rc_files(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn the_tree_matches_the_allowlist() {
    let files = guard_scan::snapshot();
    assert!(files.contains_key(ROOT_RC), "no {ROOT_RC} in the snapshot");
    guard_scan::verify(&scan_files(&files), &KINDS);
}

#[test]
fn execution_requirement_keys_count() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run(execution_requirements = {"requires-network": "1", "no-sandbox": "1"})
    ctx.actions.run(execution_requirements = {"local": "1", "supports-workers": "1"})
    requirements = {"no-cache": "1"}
    requirements["no-remote"] = "1"
    requirements["local"] = "1"
    requirements["cpu:4"] = "1"
    stage_requirements = dict(base, **{"local": "1"})
    routes = {"local": "here"}
    routes["local"] = "there"
"#;
    assert_eq!(
        scan_all(&[("pkg/defs.bzl", bzl)]),
        lines(&[("pkg/defs.bzl", "exec-requirement", 7)]),
    );
}

#[test]
fn banned_tags_count_on_targets_that_are_not_tests() {
    let bzl = r#"
probe_test = rule(implementation = _impl, test = True)
checked = rule(implementation = _impl, test = True)

def wrapped(name, **kwargs):
    checked(name = name, **kwargs)

def library(name, **kwargs):
    native.cc_library(name = name, tags = ["no-remote"], **kwargs)
"#;
    let build = r#"
cc_library(name = "a", tags = ["no-sandbox", "cpu:4"])
cc_binary(name = "b", tags = ["local"] + ["requires-network"])
cc_test(name = "c", tags = ["no-sandbox"])
checked(name = "d", tags = ["no-cache"])
wrapped(name = "e", tags = ["no-cache"])
cc_library(name = "f", tags = ["manual"], args = ["no-sandbox"])
"#;
    assert_eq!(
        scan_all(&[("pkg/BUILD.bazel", build), ("pkg/defs.bzl", bzl)]),
        lines(&[
            ("pkg/BUILD.bazel", "exec-requirement", 3),
            ("pkg/defs.bzl", "exec-requirement", 1),
        ]),
    );
}

#[test]
fn modify_execution_info_counts_each_banned_entry() {
    let rc = "
build:macos --modify_execution_info=CompileA.*=+no-sandbox,LinkA.*=+no-sandbox
common --modify_execution_info=PySdist.*=+block-network,Fetch=+requires-network
# build --modify_execution_info=X=+local
build --modify_execution_info=X=-no-sandbox
";
    assert_eq!(
        scan_all(&[(".bazelrc", rc)]),
        lines(&[(".bazelrc", "exec-requirement", 3)]),
    );
}

#[test]
fn tmp_paths_count_in_every_file_kind() {
    let bzl = r#"
_CACHE = "/tmp/go-cache"
_ROOT = "/tmp"
_OTHER = ["/tmpl", "x/tmp/y", "$TMPDIR/tmp"]
"#;
    let rust = r#"
const SCRATCH: &str = "/tmp/x";
fn scratch() -> std::path::PathBuf {
    std::env::temp_dir().join("x")
}
#[cfg(test)]
mod tests {
    #[test]
    fn scratch_dir() {
        let dir = std::env::temp_dir();
    }
}
fn other() -> std::path::PathBuf {
    env::temp_dir()
}
"#;
    let test = "fn dir() -> std::path::PathBuf { std::env::temp_dir().join(\"/tmp/y\") }\n";
    let sh = "
cd /tmp
out=\"/tmp/out\"
mkdir \"$TEST_TMPDIR/tmp\" \"${ROOT}/tmp/x\" a/tmp /tmpl
";
    assert_eq!(
        scan_all(&[
            ("pkg/defs.bzl", bzl),
            ("pkg/run.sh", sh),
            ("pkg/src/lib.rs", rust),
            ("pkg/src/lib_test.rs", test),
        ]),
        lines(&[
            ("pkg/defs.bzl", "tmp-path", 2),
            ("pkg/run.sh", "tmp-path", 2),
            ("pkg/src/lib.rs", "tmp-path", 3),
            ("pkg/src/lib_test.rs", "tmp-path", 1),
        ]),
    );
}

#[test]
fn a_required_rc_flag_set_nowhere_is_missing() {
    let unset = rc_files(&[
        (
            ".bazelrc",
            "build --jobs=4\nbuild:strict --incompatible_strict_action_env\n",
        ),
        (
            ".dx/bazelrc/correctness.bazelrc",
            "# common --sandbox_default_allow_network=false\n",
        ),
        (
            "pkg/consumer.bazelrc",
            "common --incompatible_strict_action_env\n",
        ),
    ]);
    assert_eq!(
        missing_flags(&unset),
        [
            "--incompatible_strict_action_env",
            "--sandbox_default_allow_network=false",
        ],
    );
    assert_eq!(
        scan_files(&unset).into_iter().collect::<Vec<_>>(),
        [((ROOT_RC.to_owned(), "rc-flag".to_owned()), 2)],
    );
    let set = rc_files(&[
        (".bazelrc", "common --incompatible_strict_action_env\n"),
        (
            ".dx/bazelrc/correctness.bazelrc",
            "build --nosandbox_default_allow_network\n",
        ),
    ]);
    assert_eq!(missing_flags(&set), Vec::<&str>::new());
}

#[test]
fn a_spare_or_loose_allowlist_line_fails() {
    let mut found = Counts::new();
    hit(&mut found, "pkg/defs.bzl", "tmp-path");
    hit(&mut found, "pkg/defs.bzl", "tmp-path");
    let check = |allowlist: &str| guard_scan::problems(&found, allowlist, &KINDS);
    assert_eq!(check("pkg/defs.bzl tmp-path 2\n"), Vec::<String>::new());
    assert_eq!(
        check(""),
        ["pkg/defs.bzl tmp-path 2: 0 allowed; remove the new one"],
    );
    assert_eq!(
        check("pkg/defs.bzl tmp-path 3\n"),
        ["pkg/defs.bzl tmp-path 3: 2 left; lower the allowlist count"],
    );
    assert_eq!(
        check(".bazelrc rc-flag 1\npkg/defs.bzl tmp-path 2\n"),
        [".bazelrc rc-flag 1: none left; delete the allowlist line"],
    );
}
