//! Groups outdated packages into pull requests and writes their branch names and bodies.

use crate::discover::{Candidate, Registry};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Which `dx-update/` branches one run owns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope {
    sets: Vec<String>,
}

impl Scope {
    /// Builds a scope from the sets this run updates whole.
    pub fn new(sets: Vec<String>) -> Scope {
        Scope { sets }
    }

    /// Tells whether this run may close the pull request on `branch`.
    pub fn covers(&self, branch: &str) -> bool {
        self.sets
            .iter()
            .any(|set| branch == format!("dx-update/{}", slug(set)))
    }
}

/// One outdated package and where it is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Item {
    pub candidate: Candidate,
    pub registry: Registry,
    /// The repository's tags were listed and none matched the versions.
    pub untagged: bool,
}

/// One pull request: its branch, title and packages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupPlan {
    pub branch: String,
    pub title: String,
    pub items: Vec<Item>,
}

/// Splits items into one plan per set, in branch order.
pub fn plan(items: &[Item]) -> Vec<GroupPlan> {
    let mut plans: BTreeMap<String, GroupPlan> = BTreeMap::new();
    for item in items {
        let set = &item.candidate.set;
        let branch = format!("dx-update/{}", slug(set));
        let title = format!("Update {set} dependencies");
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
        let pre = if c.prerelease { ", prerelease" } else { "" };
        let _ = write!(
            text,
            "- {}:{} {} -> {} ({}{age}{pre})",
            c.set,
            c.package,
            c.current,
            c.latest,
            item.registry.url(&c.package)
        );
        if let (Some(repo), true) = (&c.repo, item.untagged) {
            let _ = write!(text, ": [releases](https://github.com/{repo}/releases)");
        } else if let Some(repo) = &c.repo {
            let (old, new) = c
                .tags
                .clone()
                .unwrap_or_else(|| (c.current.clone(), c.latest.clone()));
            let _ = write!(
                text,
                ": [compare](https://github.com/{repo}/compare/{old}...{new}), [release](https://github.com/{repo}/releases/tag/{new})"
            );
        }
        text.push('\n');
    }
    if !numstat.is_empty() {
        text.push_str("\n| Lock file | Added | Removed |\n| --- | --- | --- |\n");
        for (file, added, removed) in numstat {
            let _ = writeln!(text, "| {file} | {added} | {removed} |");
        }
    }
    text
}

/// Picks the old and new tag names from `git ls-remote --tags --refs` output.
pub fn pick_tags(listing: &str, package: &str, old: &str, new: &str) -> Option<(String, String)> {
    let tags: std::collections::BTreeSet<&str> = listing
        .lines()
        .filter_map(|line| line.split_once("refs/tags/").map(|(_, tag)| tag.trim()))
        .collect();
    let name = package.rsplit('/').next().unwrap_or(package);
    let forms: [fn(&str, &str) -> String; 5] = [
        |_, v| v.to_owned(),
        |_, v| format!("v{v}"),
        |n, v| format!("{n}@{v}"),
        |n, v| format!("{n}-v{v}"),
        |n, v| format!("{n}-{v}"),
    ];
    forms.iter().find_map(|form| {
        let (old, new) = (form(name, old), form(name, new));
        (tags.contains(old.as_str()) && tags.contains(new.as_str())).then_some((old, new))
    })
}

/// Reads `git diff --numstat` lines, keeping the lock files the sets own.
pub fn lock_numstat(text: &str, locks: &[String]) -> Vec<(String, u64, u64)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?.parse().ok()?;
            let removed = parts.next()?.parse().ok()?;
            let file = parts.next()?;
            locks
                .iter()
                .any(|lock| lock == file)
                .then(|| (file.to_owned(), added, removed))
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
    fn scope_covers_full_sets_only() {
        let scope = Scope::new(sets(&["cargo"]));
        assert!(scope.covers("dx-update/cargo"));
        assert!(!scope.covers("dx-update/npm"));
        assert!(!scope.covers("dx-update/cargo--serde"));
        assert!(!scope.covers("dx-update/all"));
    }

    fn item(set: &str, package: &str, latest: &str, registry: Registry) -> Item {
        Item {
            candidate: Candidate {
                set: set.into(),
                package: package.into(),
                current: "1.0.0".into(),
                latest: latest.into(),
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            },
            registry,
            untagged: false,
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
        let got = plan(&items());
        let names: Vec<_> = got.iter().map(|p| p.branch.as_str()).collect();
        assert_eq!(names, ["dx-update/bazel", "dx-update/cargo"]);
        assert_eq!(got[1].items.len(), 2);
        assert_eq!(got[1].title, "Update cargo dependencies");
    }

    #[test]
    fn odd_characters_become_dashes() {
        let got = plan(&[item("c++/x", "pkg", "2.0.0", Registry::Npm)]);
        assert_eq!(got[0].branch, "dx-update/c---x");
        assert!(plan(&[]).is_empty());
    }

    #[test]
    fn body_lists_packages_and_lock_changes() {
        let got = plan(&items()[..1]);
        let text = body(&got[0], &[("Cargo.lock".into(), 4, 2)]);
        assert!(text.contains(
            "- cargo:serde 1.0.0 -> 2.0.0 (https://crates.io/crates/serde, age unknown)"
        ));
        assert!(text.contains("| Cargo.lock | 4 | 2 |"));
        assert!(!body(&got[0], &[]).contains("Lock file"));
    }

    #[test]
    fn body_links_compare_and_release_when_the_repo_is_known() {
        let mut one = plan(&[item("npm", "left-pad", "2.0.0", Registry::Npm)]);
        one[0].items[0].candidate.repo = Some("o/left-pad".into());
        assert!(body(&one[0], &[]).contains(
            ": [compare](https://github.com/o/left-pad/compare/1.0.0...2.0.0), [release](https://github.com/o/left-pad/releases/tag/2.0.0)\n"
        ));
        let plain = plan(&[item("npm", "left-pad", "2.0.0", Registry::Npm)]);
        assert!(!body(&plain[0], &[]).contains("compare"));
    }

    #[test]
    fn body_uses_the_tags_when_set() {
        let mut one = plan(&[item("go", "github.com/o/r/sub", "2.0.0", Registry::Npm)]);
        one[0].items[0].candidate.repo = Some("o/r".into());
        one[0].items[0].candidate.tags = Some(("sub/v1.0.0".into(), "sub/v2.0.0".into()));
        assert!(body(&one[0], &[]).contains(
            "[compare](https://github.com/o/r/compare/sub/v1.0.0...sub/v2.0.0), [release](https://github.com/o/r/releases/tag/sub/v2.0.0)\n"
        ));
    }

    #[test]
    fn body_marks_prereleases() {
        let mut one = items()[..1].to_vec();
        one[0].candidate.prerelease = true;
        let got = plan(&one);
        assert!(body(&got[0], &[]).contains(", age unknown, prerelease)"));
    }

    #[test]
    fn numstat_keeps_the_locks_sets_own() {
        let text = "4\t2\tCargo.lock\n10\t1\tsrc/main.rs\n-\t-\timg.lock\n\
            3\t0\tweb/package-lock.json\n2\t1\tios/Package.resolved\n";
        let owned = sets(&["Cargo.lock", "ios/Package.resolved"]);
        assert_eq!(
            lock_numstat(text, &owned),
            vec![
                ("Cargo.lock".to_owned(), 4, 2),
                ("ios/Package.resolved".to_owned(), 2, 1)
            ]
        );
        assert!(lock_numstat(text, &[]).is_empty());
    }

    const LISTING: &str = "a\trefs/tags/v1.0.0\nb\trefs/tags/v1.1.0\nc\trefs/tags/pkg@1.0.0\nd\trefs/tags/pkg@1.1.0\n";

    #[test]
    fn picks_the_first_matching_tag_form() {
        let pick = |pkg, old, new| pick_tags(LISTING, pkg, old, new);
        assert_eq!(
            pick("other", "1.0.0", "1.1.0"),
            Some(("v1.0.0".into(), "v1.1.0".into()))
        );
        assert_eq!(
            pick("pkg", "1.0.0", "1.1.0").map(|t| t.0),
            Some("v1.0.0".into())
        );
        let scoped = "a\trefs/tags/pkg@1.0.0\nb\trefs/tags/pkg@2.0.0\n";
        assert_eq!(
            pick_tags(scoped, "@s/pkg", "1.0.0", "2.0.0"),
            Some(("pkg@1.0.0".into(), "pkg@2.0.0".into()))
        );
        assert_eq!(pick_tags(LISTING, "pkg", "1.0.0", "3.0.0"), None);
    }

    #[test]
    fn an_untagged_repository_links_releases_only() {
        let mut item = item("npm", "left-pad", "2.0.0", Registry::Crates);
        item.candidate.repo = Some("o/r".into());
        item.untagged = true;
        let plan = GroupPlan {
            branch: "b".into(),
            title: "t".into(),
            items: vec![item],
        };
        let text = body(&plan, &[]);
        assert!(text.contains("[releases](https://github.com/o/r/releases)"));
        assert!(!text.contains("compare"));
    }
}
