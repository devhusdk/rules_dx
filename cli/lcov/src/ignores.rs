use std::collections::BTreeMap;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Ignores {
    pub singles: BTreeMap<u32, String>,
    pub ranges: Vec<(u32, u32, String)>,
}

pub fn is_ignored(ignores: &Ignores, line: u32) -> bool {
    if ignores.singles.contains_key(&line) {
        return true;
    }
    for range in &ignores.ranges {
        if range.0 <= line && line <= range.1 {
            return true;
        }
    }
    false
}

#[path = "ignores_find.rs"]
mod ignores_find;
#[path = "ignores_policy.rs"]
mod ignores_policy;
#[path = "ignores_scan.rs"]
mod ignores_scan;

pub use ignores_find::*;
pub use ignores_policy::*;
pub(crate) use ignores_scan::{comment_text, take_word};

#[cfg(test)]
#[path = "ignores_tests.rs"]
mod ignores_tests;
