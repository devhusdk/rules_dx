//! Public API compatibility rule shared by the registry adapters.

/// What `api_compat` does with a breaking report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiCompatMode {
    Off,
    Warn,
    Fail,
}

impl ApiCompatMode {
    /// Parses the `api_compat` attribute value.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "warn" => Some(Self::Warn),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }
}

/// How large a version bump is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bump {
    Major,
    Minor,
    Patch,
}

/// Returns the numeric `major.minor.patch` of a version; build and pre-release parts are ignored.
pub fn parse_version(version: &str) -> Option<[u64; 3]> {
    let core = version.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let mut out = [0u64; 3];
    for slot in &mut out {
        *slot = parts.next()?.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

/// Returns the bump from `previous` to `next`; None when either is not a plain version.
/// Below 1.0.0 a minor change counts as major, as cargo treats it.
pub fn required_bump(previous: &str, next: &str) -> Option<Bump> {
    let (old, new) = (parse_version(previous)?, parse_version(next)?);
    let bump = if old[0] != new[0] || (old[0] == 0 && old[1] != new[1]) {
        Bump::Major
    } else if old[1] != new[1] {
        Bump::Minor
    } else {
        Bump::Patch
    };
    Some(bump)
}

/// Returns an error when a breaking report is not allowed by the version bump.
pub fn breaking_refusal(
    mode: ApiCompatMode,
    bump: Option<Bump>,
    breaking: usize,
) -> Option<String> {
    if mode != ApiCompatMode::Fail || breaking == 0 || bump == Some(Bump::Major) {
        return None;
    }
    Some(format!(
        "{breaking} breaking API change(s) need a major version bump"
    ))
}

/// Returns the highest non-yanked, non-prerelease version in a sparse index body.
pub fn latest_published(index_body: &str) -> Option<String> {
    let mut best: Option<([u64; 3], String)> = None;
    for line in index_body.lines() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let version = entry["vers"].as_str().unwrap_or("");
        if entry["yanked"].as_bool().unwrap_or(false) || version.contains('-') {
            continue;
        }
        let Some(parsed) = parse_version(version) else {
            continue;
        };
        if best.as_ref().is_none_or(|(top, _)| parsed > *top) {
            best = Some((parsed, version.to_owned()));
        }
    }
    best.map(|(_, version)| version)
}

/// Returns the highest `v<version>` tag below `next` from `git tag` output.
pub fn previous_tag(tags: &str, next: &str) -> Option<String> {
    let next = parse_version(next)?;
    tags.lines()
        .filter_map(|tag| {
            let parsed = parse_version(tag.trim().strip_prefix('v')?)?;
            (parsed < next).then(|| (parsed, tag.trim().to_owned()))
        })
        .max()
        .map(|(_, tag)| tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_follows_semver() {
        assert_eq!(required_bump("1.2.0", "1.2.1"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "1.3.0"), Some(Bump::Minor));
        assert_eq!(required_bump("1.2.0", "2.0.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.4.0"), Some(Bump::Major));
        assert_eq!(required_bump("0.3.1", "0.3.2"), Some(Bump::Patch));
        assert_eq!(required_bump("1.2.0", "x"), None);
    }

    #[test]
    fn breaking_fails_unless_major() {
        let fail = ApiCompatMode::Fail;
        assert!(breaking_refusal(fail, Some(Bump::Patch), 1).is_some());
        assert!(breaking_refusal(fail, Some(Bump::Minor), 2).is_some());
        assert!(breaking_refusal(fail, None, 1).is_some());
        assert_eq!(breaking_refusal(fail, Some(Bump::Major), 3), None);
        assert_eq!(breaking_refusal(fail, Some(Bump::Patch), 0), None);
        assert_eq!(
            breaking_refusal(ApiCompatMode::Warn, Some(Bump::Patch), 1),
            None
        );
        assert_eq!(
            breaking_refusal(ApiCompatMode::Off, Some(Bump::Patch), 1),
            None
        );
    }

    #[test]
    fn latest_published_skips_yanked_and_prerelease() {
        let body = "{\"vers\":\"1.0.0\",\"yanked\":false}\n{\"vers\":\"1.10.0\",\"yanked\":false}\n{\"vers\":\"1.11.0\",\"yanked\":true}\n{\"vers\":\"2.0.0-rc.1\",\"yanked\":false}\n";
        assert_eq!(latest_published(body).as_deref(), Some("1.10.0"));
        assert_eq!(latest_published(""), None);
    }

    #[test]
    fn previous_tag_picks_highest_below_next() {
        let tags = "v1.2.0\nv1.10.0\nv2.0.0\nnightly\n";
        assert_eq!(previous_tag(tags, "2.0.0").as_deref(), Some("v1.10.0"));
        assert_eq!(previous_tag(tags, "1.0.0"), None);
    }
}
