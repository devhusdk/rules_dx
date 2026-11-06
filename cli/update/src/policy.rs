//! Drops outdated packages that the pull request policy skips.

use crate::discover::Candidate;
use crate::pr::GroupPlan;

/// How big an update is.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Level {
    Patch,
    Minor,
    Major,
}

impl Level {
    /// Reads `patch`, `minor` or `all`. `none` and anything else give `None`.
    pub fn limit(text: &str) -> Option<Level> {
        match text {
            "patch" => Some(Level::Patch),
            "minor" => Some(Level::Minor),
            "all" => Some(Level::Major),
            _ => None,
        }
    }
}

fn numbers(version: &str) -> Option<(u64, u64)> {
    let start = version.find(|c: char| c.is_ascii_digit())?;
    let end = version[start..]
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .map_or(version.len(), |i| start + i);
    let mut parts = version[start..end]
        .split('.')
        .map(|part| part.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    Some((major, minor))
}

/// Sizes the move from `current` to `latest`. Zero-major versions treat a minor bump as major.
pub fn level(current: &str, latest: &str) -> Level {
    let (Some((cm, cn)), Some((lm, ln))) = (numbers(current), numbers(latest)) else {
        return Level::Major;
    };
    if cm != lm || (cm == 0 && cn != ln) {
        Level::Major
    } else if cn != ln {
        Level::Minor
    } else {
        Level::Patch
    }
}

/// What to drop.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Policy {
    pub ignore: Vec<String>,
    pub allow_major: bool,
    pub cutoff: Option<String>,
}

/// A dropped package and why.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Skip {
    pub candidate: Candidate,
    pub reason: &'static str,
}

/// Splits candidates into the ones to update and the ones skipped.
pub fn filter(candidates: &[Candidate], policy: &Policy) -> (Vec<Candidate>, Vec<Skip>) {
    let mut kept = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        let name = format!("{}:{}", candidate.set, candidate.package);
        let reason = if policy.ignore.contains(&name) {
            Some("ignored")
        } else if !policy.allow_major
            && level(&candidate.current, &candidate.latest) == Level::Major
        {
            Some("major")
        } else if too_new(candidate, policy.cutoff.as_deref()) {
            Some("too new")
        } else {
            None
        };
        match reason {
            Some(reason) => skipped.push(Skip {
                candidate: candidate.clone(),
                reason,
            }),
            None => kept.push(candidate.clone()),
        }
    }
    (kept, skipped)
}

fn too_new(candidate: &Candidate, cutoff: Option<&str>) -> bool {
    let (Some(cutoff), Some(published)) = (cutoff, candidate.published.as_deref()) else {
        return false;
    };
    let Some(date) = published.get(..10) else {
        return false;
    };
    let iso = date.len() == 10
        && date.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    iso && date > cutoff
}

/// Says whether every package in the group is within `limit`. `None` means never.
pub fn automerge(limit: Option<Level>, plan: &GroupPlan) -> bool {
    let Some(limit) = limit else {
        return false;
    };
    plan.items.iter().all(|item| {
        !item.candidate.prerelease
            && level(&item.candidate.current, &item.candidate.latest) <= limit
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::Registry;
    use crate::pr::{Group, Item, plan};

    fn candidate(package: &str, current: &str, latest: &str, published: Option<&str>) -> Candidate {
        Candidate {
            set: "cargo".into(),
            package: package.into(),
            current: current.into(),
            latest: latest.into(),
            published: published.map(str::to_owned),
            prerelease: false,
            repo: None,
            tags: None,
        }
    }

    #[test]
    fn sizes_updates() {
        assert_eq!(level("^1.2.0", "2.0.0"), Level::Major);
        assert_eq!(level("0.3", "0.4.1"), Level::Major);
        assert_eq!(level("0.3.1", "0.3.9"), Level::Patch);
        assert_eq!(level("4.12", "4.13.2"), Level::Minor);
        assert_eq!(level("1.2.3", "1.2.4"), Level::Patch);
        assert_eq!(level(">=1,<2", "2.0.0"), Level::Major);
        assert_eq!(level(">=1,<2", "1.4.0"), Level::Minor);
        assert_eq!(level("v1.2.3", "v1.3.0"), Level::Minor);
        assert_eq!(level("latest", "1.0.0"), Level::Major);
        assert_eq!(level("1.0.0", ""), Level::Major);
    }

    #[test]
    fn drops_ignored_major_and_new() {
        let all = [
            candidate("a", "1.0.0", "1.0.1", Some("2026-01-01T00:00:00Z")),
            candidate("b", "1.0.0", "1.0.1", None),
            candidate("c", "1.0.0", "2.0.0", None),
            candidate("d", "1.0.0", "1.0.1", Some("2026-02-01")),
            candidate("e", "1.0.0", "1.0.1", Some("soon")),
        ];
        let policy = Policy {
            ignore: vec!["cargo:b".into()],
            allow_major: false,
            cutoff: Some("2026-01-31".into()),
        };
        let (kept, skipped) = filter(&all, &policy);
        let kept: Vec<_> = kept.iter().map(|c| c.package.as_str()).collect();
        assert_eq!(kept, ["a", "e"]);
        let why: Vec<_> = skipped.iter().map(|s| s.reason).collect();
        assert_eq!(why, ["ignored", "major", "too new"]);
    }

    #[test]
    fn the_cutoff_day_is_kept_and_majors_can_be_allowed() {
        let all = [
            candidate("a", "1.0.0", "2.0.0", Some("2026-01-31T23:00:00Z")),
            candidate("b", "1.0.0", "1.1.0", Some("2026-02-01T00:00:00Z")),
        ];
        let policy = Policy {
            allow_major: true,
            cutoff: Some("2026-01-31".into()),
            ..Policy::default()
        };
        let (kept, skipped) = filter(&all, &policy);
        assert_eq!(kept.len(), 1);
        assert_eq!(skipped[0].candidate.package, "b");
    }

    #[test]
    fn automerge_checks_every_package() {
        let items: Vec<Item> = [("a", "1.0.0", "1.0.1"), ("b", "1.0.0", "1.2.0")]
            .into_iter()
            .map(|(p, c, l)| Item {
                candidate: candidate(p, c, l, None),
                registry: Registry::Crates,
                untagged: false,
            })
            .collect();
        let plans = plan(&items, Group::All);
        assert!(!automerge(Some(Level::Patch), &plans[0]));
        assert!(automerge(Some(Level::Minor), &plans[0]));
        assert!(!automerge(None, &plans[0]));
        assert_eq!(Level::limit("all"), Some(Level::Major));
        assert_eq!(Level::limit("none"), None);
    }

    #[test]
    fn a_prerelease_is_never_automerged() {
        let mut pre = candidate("a", "1.0.0", "1.0.1", None);
        pre.prerelease = true;
        let items = vec![Item {
            candidate: pre,
            registry: Registry::Crates,
            untagged: false,
        }];
        let plans = plan(&items, Group::All);
        assert!(!automerge(Some(Level::Major), &plans[0]));
    }
}
