use std::collections::{BTreeMap, BTreeSet};

use starlark_syntax::syntax::ast::{ArgumentP, AssignTargetP, AstAssignTarget, AstExpr, ExprP};

use crate::starlark::{callee_name, is_true};

const TEST_WORDS: [&str; 5] = ["bench", "fuzz", "suite", "test", "tests"];

fn test_name(name: &str) -> bool {
    name.split('_').any(|word| TEST_WORDS.contains(&word))
}

#[derive(Default)]
pub struct Callees {
    test_rules: BTreeSet<String>,
    defs: BTreeMap<String, BTreeSet<String>>,
}

impl Callees {
    pub fn def(&mut self, name: &str) {
        self.defs.entry(name.to_owned()).or_default();
    }

    pub fn call(&mut self, def: Option<&str>, callee: &str) {
        if let Some(def) = def {
            self.defs
                .entry(def.to_owned())
                .or_default()
                .insert(callee.to_owned());
        }
    }

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

pub fn test_callees(files: &[&Callees]) -> BTreeSet<String> {
    let mut tests = BTreeSet::new();
    for file in files {
        tests.extend(file.test_rules.iter().cloned());
    }
    loop {
        let mut grew = false;
        for file in files {
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

pub fn is_test(tests: &BTreeSet<String>, callee: &str) -> bool {
    test_name(callee) || tests.contains(callee)
}
