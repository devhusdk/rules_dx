//! Lists Go modules whose newest release on the module proxy is newer than the go.mod requirement.

use super::{Candidate, Fetch, Findings, LookupError, github_repo, wanted};
use semver::Version;

pub const DEFAULT_BASE: &str = "https://proxy.golang.org";

/// Checks each direct requirement of the go.mod files against the proxy at `base`.
pub fn outdated(
    set: &str,
    go_mods: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    for text in go_mods {
        let Ok(file) = dx_gomod::parse_mod(text) else {
            continue;
        };
        for requirement in &file.require {
            let module = requirement.module.as_str();
            if requirement.indirect
                || file.replacement_for(module, &requirement.version).is_some()
                || !wanted(only, module)
            {
                continue;
            }
            let error = |reason: String| LookupError {
                set: set.to_owned(),
                package: module.to_owned(),
                reason,
            };
            let current = requirement.version.trim_start_matches('v');
            let Ok(current_version) = Version::parse(current) else {
                found
                    .errors
                    .push(error(format!("invalid version {}", requirement.version)));
                continue;
            };
            let escaped = escape(module);
            let latest = match fetch
                .get(&format!("{base}/{escaped}/@v/list"))
                .and_then(|body| newest(&body, current.contains("+incompatible"), prereleases))
            {
                Ok(latest) => latest,
                Err(reason) => {
                    found.errors.push(error(reason));
                    continue;
                }
            };
            if latest.0 <= current_version {
                continue;
            }
            let published = fetch
                .get(&format!("{base}/{escaped}/@v/v{}.info", latest.1))
                .ok()
                .and_then(|body| serde_json::from_str::<serde_json::Value>(&body).ok())
                .and_then(|info| info["Time"].as_str().map(str::to_owned));
            let (repo, tags) = match tags(module, current, &latest.1) {
                Some((repo, tags)) => (Some(repo), Some(tags)),
                None => (None, None),
            };
            found.candidates.push(Candidate {
                set: set.to_owned(),
                package: module.to_owned(),
                current: current.to_owned(),
                latest: latest.1,
                published,
                prerelease: !latest.0.pre.is_empty(),
                repo,
                tags,
            });
        }
    }
    found
}

/// The GitHub repository of a module and its old and new tags.
fn tags(module: &str, current: &str, latest: &str) -> Option<(String, (String, String))> {
    let rest = module.strip_prefix("github.com/")?;
    let repo = github_repo(module)?;
    let mut parts: Vec<&str> = rest.split('/').skip(2).collect();
    if parts.last().is_some_and(|last| is_major_suffix(last)) {
        parts.pop();
    }
    let prefix = if parts.is_empty() {
        String::new()
    } else {
        format!("{}/", parts.join("/"))
    };
    let strip = |version: &str| version.trim_end_matches("+incompatible").to_owned();
    let old = match pseudo_hash(current) {
        Some(hash) => hash.to_owned(),
        None => format!("{prefix}v{}", strip(current)),
    };
    Some((repo, (old, format!("{prefix}v{}", strip(latest)))))
}

fn is_major_suffix(part: &str) -> bool {
    part.strip_prefix('v').is_some_and(|n| {
        !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && n != "0" && n != "1"
    })
}

/// The commit hash of a pseudo-version such as `0.0.0-20240101120000-abcdef123456`.
fn pseudo_hash(version: &str) -> Option<&str> {
    let (rest, hash) = version.rsplit_once('-')?;
    let (_, stamp) = rest.rsplit_once('-')?;
    let ok = stamp.len() == 14
        && stamp.bytes().all(|b| b.is_ascii_digit())
        && hash.len() == 12
        && hash.bytes().all(|b| b.is_ascii_hexdigit());
    ok.then_some(hash)
}

fn escape(module: &str) -> String {
    let mut out = String::new();
    for ch in module.chars() {
        if ch.is_ascii_uppercase() {
            out.push('!');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn newest(
    body: &str,
    incompatible_ok: bool,
    prereleases: bool,
) -> Result<(Version, String), String> {
    let mut best: Option<(Version, String)> = None;
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let text = line.trim_start_matches('v');
        let Ok(version) = Version::parse(text) else {
            continue;
        };
        if (!prereleases && !version.pre.is_empty())
            || (text.ends_with("+incompatible") && !incompatible_ok)
        {
            continue;
        }
        if best.as_ref().is_none_or(|(b, _)| version > *b) {
            best = Some((version, text.to_owned()));
        }
    }
    best.ok_or_else(|| "no stable version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GO_MOD: &str = "module example.com/app\n\ngo 1.22\n\nrequire (\n\tgithub.com/Foo/bar v1.0.0\n\tgithub.com/fresh/lib v1.2.0\n\tgithub.com/ind/lib v1.0.0 // indirect\n\tgithub.com/repl/lib v1.0.0\n\tgithub.com/gone/lib v1.0.0\n\tgithub.com/old/inc v2.0.0+incompatible\n)\n\nreplace github.com/repl/lib => ../repl\n";

    fn proxy(url: &str) -> Result<String, String> {
        let path = url.strip_prefix("https://proxy.golang.org/").unwrap_or(url);
        let body = match path {
            "github.com/!foo/bar/@v/list" => "v1.0.0\nv1.1.0\nv1.2.0-rc.1\nv2.0.0+incompatible\n",
            "github.com/!foo/bar/@v/v1.1.0.info" => {
                r#"{"Version":"v1.1.0","Time":"2026-02-03T00:00:00Z"}"#
            }
            "github.com/fresh/lib/@v/list" => "v1.2.0\nv1.3.0-beta\n",
            "github.com/old/inc/@v/list" => "v2.0.0+incompatible\nv2.1.0+incompatible\n",
            _ => return Err("HTTP 404".to_owned()),
        };
        Ok(body.to_owned())
    }

    fn run(only: &[String]) -> Findings {
        outdated(
            "go",
            &[GO_MOD.to_owned()],
            only,
            false,
            DEFAULT_BASE,
            &proxy,
        )
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let found = outdated(
            "go",
            &[GO_MOD.to_owned()],
            &["github.com/fresh/lib".to_owned()],
            true,
            DEFAULT_BASE,
            &proxy,
        );
        assert_eq!(found.candidates[0].latest, "1.3.0-beta");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn the_newest_stable_version_is_listed_with_its_publish_time() {
        let found = run(&[]);
        assert_eq!(
            found
                .candidates
                .iter()
                .find(|c| c.package == "github.com/Foo/bar"),
            Some(&Candidate {
                set: "go".into(),
                package: "github.com/Foo/bar".into(),
                current: "1.0.0".into(),
                latest: "1.1.0".into(),
                published: Some("2026-02-03T00:00:00Z".into()),
                prerelease: false,
                repo: Some("Foo/bar".into()),
                tags: Some(("v1.0.0".into(), "v1.1.0".into())),
            })
        );
    }

    #[test]
    fn tags_follow_the_module_path() {
        let t = |m, c, l| tags(m, c, l).map(|(r, t)| (r, t.0, t.1));
        let s = |r: &str, a: &str, b: &str| Some((r.to_owned(), a.to_owned(), b.to_owned()));
        assert_eq!(
            t("github.com/o/r/v2", "2.0.0", "2.1.0"),
            s("o/r", "v2.0.0", "v2.1.0")
        );
        assert_eq!(
            t("github.com/o/r/sub", "1.0.0", "1.1.0"),
            s("o/r", "sub/v1.0.0", "sub/v1.1.0")
        );
        assert_eq!(
            t("github.com/o/r/sub/v3", "3.0.0", "3.1.0"),
            s("o/r", "sub/v3.0.0", "sub/v3.1.0")
        );
        assert_eq!(
            t("github.com/o/r", "2.0.0+incompatible", "2.1.0+incompatible"),
            s("o/r", "v2.0.0", "v2.1.0")
        );
        assert_eq!(
            t(
                "github.com/o/r",
                "0.0.0-20240101120000-abcdef123456",
                "1.0.0"
            ),
            s("o/r", "abcdef123456", "v1.0.0")
        );
        assert_eq!(
            t("github.com/o/r", "1.0.0-20240101120000-xyz", "1.0.1"),
            s("o/r", "v1.0.0-20240101120000-xyz", "v1.0.1")
        );
        assert_eq!(t("golang.org/x/net", "1.0.0", "1.1.0"), None);
    }

    #[test]
    fn prereleases_and_current_modules_are_not_listed() {
        let found = run(&[]);
        assert!(!found.candidates.iter().any(|c| c.package.contains("fresh")));
    }

    #[test]
    fn incompatible_versions_count_only_when_already_in_use() {
        let found = run(&[]);
        let inc = found.candidates.iter().find(|c| c.package.contains("inc"));
        assert_eq!(inc.map(|c| c.latest.as_str()), Some("2.1.0+incompatible"));
        let bar = found.candidates.iter().find(|c| c.package.contains("bar"));
        assert_eq!(bar.map(|c| c.latest.as_str()), Some("1.1.0"));
    }

    #[test]
    fn indirect_and_replaced_modules_are_skipped() {
        let found = run(&[]);
        let all: Vec<_> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .chain(found.errors.iter().map(|e| e.package.as_str()))
            .collect();
        assert!(!all.contains(&"github.com/ind/lib"));
        assert!(!all.contains(&"github.com/repl/lib"));
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "github.com/gone/lib");
    }

    #[test]
    fn a_missing_info_file_leaves_the_publish_time_empty() {
        let found = outdated(
            "go",
            &[GO_MOD.to_owned()],
            &["github.com/old/inc".to_owned()],
            false,
            DEFAULT_BASE,
            &proxy,
        );
        assert_eq!(found.candidates[0].published, None);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["github.com/Foo/bar".to_owned()]);
        assert_eq!(found.candidates.len(), 1);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn uppercase_letters_are_escaped() {
        assert_eq!(escape("github.com/Azure/X"), "github.com/!azure/!x");
    }
}
