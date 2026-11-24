use std::collections::BTreeMap;
use std::path::Path;

use dx_testing::allowlist::{Lines, parse_lines};

use crate::snapshot::read_input;

pub type Counts = BTreeMap<(String, String), usize>;

pub const REMOVE: &str = "remove the new one";

pub fn hit(counts: &mut Counts, path: &str, kind: &str) {
    *counts
        .entry((path.to_owned(), kind.to_owned()))
        .or_default() += 1;
}

pub fn add(counts: &mut Counts, path: &str, kind: &str, count: usize) {
    if count > 0 {
        *counts
            .entry((path.to_owned(), kind.to_owned()))
            .or_default() += count;
    }
}

pub fn render(counts: &Counts) -> String {
    counts
        .iter()
        .map(|((path, kind), count)| format!("{path} {kind} {count}\n"))
        .collect()
}

pub fn parse(text: &str, kinds: &[&str]) -> (Counts, Vec<String>) {
    let mut counts = Counts::new();
    let mut problems = Vec::new();
    let mut previous = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let [path, kind, count] = fields.as_slice() else {
            problems.push(format!("malformed line: {line:?}"));
            continue;
        };
        let count = count
            .parse::<usize>()
            .ok()
            .filter(|parsed| parsed.to_string() == *count)
            .unwrap_or(0);
        if path.is_empty() || count == 0 || !kinds.contains(kind) {
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

pub fn differences(found: &Counts, allowed: &Counts, fix: &str) -> Vec<String> {
    let mut out = Vec::new();
    for ((path, kind), count) in found {
        let limit = allowed
            .get(&(path.clone(), kind.clone()))
            .copied()
            .unwrap_or(0);
        if *count > limit {
            out.push(format!("{path} {kind} {count}: {limit} allowed; {fix}"));
        }
    }
    for ((path, kind), limit) in allowed {
        let count = found
            .get(&(path.clone(), kind.clone()))
            .copied()
            .unwrap_or(0);
        if count == 0 {
            out.push(format!(
                "{path} {kind} {limit}: none left; delete the allowlist line"
            ));
        } else if count < *limit {
            out.push(format!(
                "{path} {kind} {limit}: {count} left; lower the allowlist count"
            ));
        }
    }
    out
}

fn budget_of(budget: &Counts, key: &(String, String)) -> usize {
    budget.get(key).copied().unwrap_or(0)
}

pub fn over_budget(found: &Counts, budget: &Counts, unit: &str) -> Vec<String> {
    found
        .iter()
        .filter(|&(key, count)| *count > budget_of(budget, key))
        .map(|(key, count)| {
            format!(
                "{}: {count} {unit}, budget {}",
                key.0,
                budget_of(budget, key)
            )
        })
        .collect()
}

pub fn under_budget(found: &Counts, budget: &Counts, unit: &str) -> Vec<String> {
    budget
        .iter()
        .filter_map(|(key, limit)| {
            let count = budget_of(found, key);
            (count < *limit).then(|| format!("{}: {count} {unit}, budget {limit}", key.0))
        })
        .collect()
}

pub fn lowered(found: &Counts, budget: &Counts) -> Counts {
    let mut counts = Counts::new();
    for (key, limit) in budget {
        add(
            &mut counts,
            &key.0,
            &key.1,
            budget_of(found, key).min(*limit),
        );
    }
    counts
}

pub fn check(found: &Counts, text: &str, kinds: &[&str], fix: &str) -> Vec<String> {
    let (allowed, mut problems) = parse(text, kinds);
    problems.extend(differences(found, &allowed, fix));
    problems
}

pub fn verify(found: &Counts, kinds: &[&str]) {
    let key = dx_testing::env_text("ALLOWLIST");
    let problems = check(found, &read_input("ALLOWLIST"), kinds, REMOVE);
    let outputs = std::env::var_os("TEST_UNDECLARED_OUTPUTS_DIR");
    if let Some(dir) = outputs.filter(|_| !problems.is_empty()) {
        let name = key.rsplit('/').next().unwrap_or(&key);
        let listed: Counts = found
            .iter()
            .filter(|((_, kind), _)| kinds.contains(&kind.as_str()))
            .map(|(key, count)| (key.clone(), *count))
            .collect();
        dx_testing::write_text(&Path::new(&dir).join(name), &render(&listed));
    }
    assert!(
        problems.is_empty(),
        "{key} and the tree disagree ({} problems):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

pub fn line_differences(
    found: &Lines,
    allowed: &Lines,
    fix: impl Fn(&str) -> String,
) -> Vec<String> {
    let new = found
        .difference(allowed)
        .map(|line| format!("new: {line}; {}", fix(line)));
    let stale = allowed
        .difference(found)
        .map(|line| format!("stale: {line}; delete the allowlist line"));
    new.chain(stale).collect()
}

pub fn check_lines(
    found: &Lines,
    text: &str,
    well_formed: impl Fn(&str) -> bool,
    fix: impl Fn(&str) -> String,
) -> Vec<String> {
    let (allowed, mut problems) = parse_lines(text, well_formed);
    problems.extend(line_differences(found, &allowed, fix));
    problems
}
