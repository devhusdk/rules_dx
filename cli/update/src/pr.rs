//! Groups outdated packages into pull requests and writes their branch names and bodies.

use crate::discover::{Candidate, Registry};
use std::collections::BTreeMap;
use std::fmt::Write;

/// How outdated packages share pull requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Group {
    Package,
    #[default]
    Set,
    All,
}

impl Group {
    /// Reads `package`, `set` or `all`.
    pub fn parse(text: &str) -> Option<Group> {
        match text {
            "package" => Some(Group::Package),
            "set" => Some(Group::Set),
            "all" => Some(Group::All),
            _ => None,
        }
    }
}

/// Which `dx-update/` branches one run owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope {
    group: Group,
    everything: bool,
    sets: Vec<String>,
    packages: Vec<(String, String)>,
}

impl Scope {
    /// Builds a scope from whole sets, single packages, and whether no selector was given.
    pub fn new(
        group: Group,
        everything: bool,
        sets: Vec<String>,
        packages: Vec<(String, String)>,
    ) -> Scope {
        Scope {
            group,
            everything,
            sets,
            packages,
        }
    }

    /// Tells whether this run may close the pull request on `branch`.
    pub fn covers(&self, branch: &str) -> bool {
        match self.group {
            Group::All => self.everything && branch == "dx-update/all",
            Group::Set => self
                .sets
                .iter()
                .any(|set| branch == format!("dx-update/{}", slug(set))),
            Group::Package => {
                self.sets
                    .iter()
                    .any(|set| branch.starts_with(&format!("dx-update/{}--", slug(set))))
                    || self.packages.iter().any(|(set, package)| {
                        branch == format!("dx-update/{}--{}", slug(set), slug(package))
                    })
            }
        }
    }
}

/// One outdated package and where it is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub candidate: Candidate,
    pub registry: Registry,
}

/// One pull request: its branch, title and packages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupPlan {
    pub branch: String,
    pub title: String,
    pub items: Vec<Item>,
}

/// Splits items into one plan per group, in branch order.
pub fn plan(items: &[Item], group: Group) -> Vec<GroupPlan> {
    let mut plans: BTreeMap<String, GroupPlan> = BTreeMap::new();
    for item in items {
        let set = &item.candidate.set;
        let package = &item.candidate.package;
        let (branch, title) = match group {
            Group::Package => (
                format!("dx-update/{}--{}", slug(set), slug(package)),
                format!("Update {package} to {}", item.candidate.latest),
            ),
            Group::Set => (
                format!("dx-update/{}", slug(set)),
                format!("Update {set} dependencies"),
            ),
            Group::All => ("dx-update/all".to_owned(), "Update dependencies".to_owned()),
        };
        plans
            .entry(branch.clone())
            .or_insert_with(|| GroupPlan {
                branch,
                title,
                items: Vec::new(),
            })
            .items
            .push(item.clone());
    }
    plans.into_values().collect()
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Writes the pull request body: one line per package, then the lock file changes.
pub fn body(plan: &GroupPlan, numstat: &[(String, u64, u64)]) -> String {
    let mut text = String::new();
    for item in &plan.items {
        let c = &item.candidate;
        let age = if c.published.is_none() {
            ", age unknown"
        } else {
            ""
        };
        let _ = writeln!(
            text,
            "- {}:{} {} -> {} ({}{age})",
            c.set,
            c.package,
            c.current,
            c.latest,
            item.registry.url(&c.package)
        );
    }
    if !numstat.is_empty() {
        text.push_str("\n| Lock file | Added | Removed |\n| --- | --- | --- |\n");
        for (file, added, removed) in numstat {
            let _ = writeln!(text, "| {file} | {added} | {removed} |");
        }
    }
    text
}

/// Reads `git diff --numstat` lines, keeping lock files.
pub fn lock_numstat(text: &str) -> Vec<(String, u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?.parse().ok()?;
            let removed = parts.next()?.parse().ok()?;
            let file = parts.next()?;
            let name = file.rsplit('/').next().unwrap_or(file);
            let lock = name.ends_with(".lock")
                || name.contains("-lock.")
                || name.contains(".lock.")
                || name == "go.sum";
            lock.then(|| (file.to_owned(), added, removed))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn set_scope_covers_full_sets_only() {
        let scope = Scope::new(Group::Set, false, sets(&["cargo"]), Vec::new());
        assert!(scope.covers("dx-update/cargo"));
        assert!(!scope.covers("dx-update/npm"));
        assert!(!scope.covers("dx-update/cargo--serde"));
        assert!(!scope.covers("dx-update/all"));
    }

    #[test]
    fn package_scope_covers_the_set_prefix_and_selected_packages() {
        let scope = Scope::new(
            Group::Package,
            false,
            sets(&["cargo"]),
            vec![("npm".into(), "left pad".into())],
        );
        assert!(scope.covers("dx-update/cargo--serde"));
        assert!(scope.covers("dx-update/npm--left-pad"));
        assert!(!scope.covers("dx-update/npm--other"));
        assert!(!scope.covers("dx-update/cargo"));
    }

    #[test]
    fn all_scope_needs_no_selectors() {
        let all = Scope::new(Group::All, true, sets(&["cargo"]), Vec::new());
        assert!(all.covers("dx-update/all"));
        assert!(!all.covers("dx-update/cargo"));
        let picked = Scope::new(Group::All, false, sets(&["cargo"]), Vec::new());
        assert!(!picked.covers("dx-update/all"));
    }

    fn item(set: &str, package: &str, latest: &str, registry: Registry) -> Item {
        Item {
            candidate: Candidate {
                set: set.into(),
                package: package.into(),
                current: "1.0.0".into(),
                latest: latest.into(),
                published: None,
            },
            registry,
        }
    }

    fn items() -> Vec<Item> {
        vec![
            item("cargo", "serde", "2.0.0", Registry::Crates),
            item("cargo", "anyhow", "1.1.0", Registry::Crates),
            item("bazel", "rules_go", "0.5.0", Registry::Bcr),
        ]
    }

    #[test]
    fn set_groups_share_a_branch() {
        let got = plan(&items(), Group::Set);
        let names: Vec<_> = got.iter().map(|p| p.branch.as_str()).collect();
        assert_eq!(names, ["dx-update/bazel", "dx-update/cargo"]);
        assert_eq!(got[1].items.len(), 2);
        assert_eq!(got[1].title, "Update cargo dependencies");
    }

    #[test]
    fn package_groups_get_one_branch_each() {
        let got = plan(&items(), Group::Package);
        assert_eq!(got.len(), 3);
        assert_eq!(got[2].branch, "dx-update/cargo--serde");
        assert_eq!(got[2].title, "Update serde to 2.0.0");
    }

    #[test]
    fn all_is_one_group() {
        let got = plan(&items(), Group::All);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].branch, "dx-update/all");
        assert_eq!(got[0].items.len(), 3);
    }

    #[test]
    fn odd_characters_become_dashes() {
        let got = plan(
            &[item("npm", "@scope/pkg", "2.0.0", Registry::Npm)],
            Group::Package,
        );
        assert_eq!(got[0].branch, "dx-update/npm---scope-pkg");
        assert!(plan(&[], Group::Set).is_empty());
    }

    #[test]
    fn body_lists_packages_and_lock_changes() {
        let got = plan(&items()[..1], Group::Set);
        let text = body(&got[0], &[("Cargo.lock".into(), 4, 2)]);
        assert!(text.contains(
            "- cargo:serde 1.0.0 -> 2.0.0 (https://crates.io/crates/serde, age unknown)"
        ));
        assert!(text.contains("| Cargo.lock | 4 | 2 |"));
        assert!(!body(&got[0], &[]).contains("Lock file"));
    }

    #[test]
    fn numstat_keeps_lock_files_only() {
        let text =
            "4\t2\tCargo.lock\n10\t1\tsrc/main.rs\n-\t-\timg.lock\n3\t0\tweb/package-lock.json\n";
        assert_eq!(
            lock_numstat(text),
            vec![
                ("Cargo.lock".to_owned(), 4, 2),
                ("web/package-lock.json".to_owned(), 3, 0)
            ]
        );
    }

    #[test]
    fn group_names_parse() {
        assert_eq!(Group::parse("all"), Some(Group::All));
        assert_eq!(Group::parse("x"), None);
        assert_eq!(Group::default(), Group::Set);
    }
}
