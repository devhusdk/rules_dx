//! What the test guards share: the Starlark parse, the detection of test calls and the allowlist check.

use std::collections::{BTreeMap, BTreeSet};

use starlark_syntax::syntax::ast::{
    ArgumentP, AssignTargetP, AstAssignTarget, AstExpr, AstLiteral, ExprP,
};
use starlark_syntax::syntax::{AstModule, Dialect};

/// Occurrences keyed by file path and kind.
pub type Counts = BTreeMap<(String, String), usize>;

/// Words in a rule or macro name that mark it as a test.
const TEST_WORDS: [&str; 5] = ["bench", "fuzz", "suite", "test", "tests"];

/// Whether `name` reads as a test rule or macro.
fn test_name(name: &str) -> bool {
    name.split('_').any(|word| TEST_WORDS.contains(&word))
}

/// The last name of a callee: `native.test_suite` gives `test_suite`.
pub fn callee_name(callee: &AstExpr) -> String {
    match &callee.node {
        ExprP::Identifier(ident) => ident.node.ident.clone(),
        ExprP::Dot(_, name) => name.node.clone(),
        ExprP::Tuple(_)
        | ExprP::Call(..)
        | ExprP::Index(_)
        | ExprP::Index2(_)
        | ExprP::Slice(..)
        | ExprP::Lambda(_)
        | ExprP::Literal(_)
        | ExprP::Not(_)
        | ExprP::Minus(_)
        | ExprP::Plus(_)
        | ExprP::BitNot(_)
        | ExprP::Op(..)
        | ExprP::If(_)
        | ExprP::List(_)
        | ExprP::Dict(_)
        | ExprP::ListComprehension(..)
        | ExprP::DictComprehension(..)
        | ExprP::FString(_) => String::new(),
    }
}

pub fn is_true(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "True")
}

/// The text of a string literal.
pub fn string(expr: &AstExpr) -> Option<&str> {
    match &expr.node {
        ExprP::Literal(AstLiteral::String(text)) => Some(&text.node),
        ExprP::Tuple(_)
        | ExprP::Dot(..)
        | ExprP::Call(..)
        | ExprP::Index(_)
        | ExprP::Index2(_)
        | ExprP::Slice(..)
        | ExprP::Identifier(_)
        | ExprP::Lambda(_)
        | ExprP::Literal(_)
        | ExprP::Not(_)
        | ExprP::Minus(_)
        | ExprP::Plus(_)
        | ExprP::BitNot(_)
        | ExprP::Op(..)
        | ExprP::If(_)
        | ExprP::List(_)
        | ExprP::Dict(_)
        | ExprP::ListComprehension(..)
        | ExprP::DictComprehension(..)
        | ExprP::FString(_) => None,
    }
}

/// Parses one BUILD or .bzl file.
pub fn parse(path: &str, source: &str) -> AstModule {
    AstModule::parse(path, source.to_owned(), &Dialect::Extended)
        .unwrap_or_else(|error| panic!("{path}: does not parse: {}", error.without_diagnostic()))
}

/// The test rules one file declares and the callees of each of its functions.
#[derive(Default)]
pub struct Callees {
    test_rules: BTreeSet<String>,
    defs: BTreeMap<String, BTreeSet<String>>,
}

impl Callees {
    /// Records a function.
    pub fn def(&mut self, name: &str) {
        self.defs.entry(name.to_owned()).or_default();
    }

    /// Records a call made inside the function `def`.
    pub fn call(&mut self, def: Option<&str>, callee: &str) {
        if let Some(def) = def {
            self.defs
                .entry(def.to_owned())
                .or_default()
                .insert(callee.to_owned());
        }
    }

    /// Records `name = rule(..., test = True)`.
    pub fn rule(&mut self, lhs: &AstAssignTarget, rhs: &AstExpr) {
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
            self.test_rules.insert(ident.node.ident.clone());
        }
    }
}

/// Names that are tests without a test word: test rules, and macros that call a test.
pub fn test_callees<'a>(files: impl Iterator<Item = &'a Callees> + Clone) -> BTreeSet<String> {
    let mut tests = BTreeSet::new();
    for file in files.clone() {
        tests.extend(file.test_rules.iter().cloned());
    }
    loop {
        let mut grew = false;
        for file in files.clone() {
            for (def, callees) in &file.defs {
                if tests.contains(def) {
                    continue;
                }
                if callees.iter().any(|callee| is_test(&tests, callee)) {
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

/// Whether a call of `callee` is a test call.
pub fn is_test(tests: &BTreeSet<String>, callee: &str) -> bool {
    test_name(callee) || tests.contains(callee)
}

/// Reads one runfile named by an environment variable.
pub fn read_input(variable: &str) -> String {
    let key = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is set by the test"));
    let path = dx_testing::rlocation(&key).unwrap_or_else(|error| panic!("{variable}: {error}"));
    dx_testing::read_text(&path)
}

/// One allowlist line: `<path> <kind> <count>`.
pub fn parse_allowlist(text: &str, kinds: &[&str]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, kind, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count.parse::<usize>().unwrap_or(0);
        if count == 0 || !kinds.contains(kind) {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        }
        let key = ((*path).to_owned(), (*kind).to_owned());
        if previous.as_ref().is_some_and(|before| *before >= key) {
            problems.push(format!("not sorted or repeated: {line:?}"));
        }
        previous = Some(key.clone());
        counts.insert(key, count);
    }
    (counts, problems)
}

/// Lines that differ between the tree and the allowlist.
pub fn differences(found: &Counts, allowed: &Counts, fix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for ((path, tag), count) in found {
        let limit = allowed
            .get(&(path.clone(), tag.clone()))
            .copied()
            .unwrap_or(0);
        if *count > limit {
            out.push(format!(
                "{path}: {count} `{tag}` on tests, {limit} allowed; {fix}"
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
