//! Lists GitHub Actions pins and the `.bazelversion` pin that lag the newest GitHub release.

use dx_error::DxError;

use super::{Candidate, Fetch, Findings, LookupError};
use crate::edit::gha::{
    parse_workflow_pins, resolve_sha_from_commit_response, split_pin, version_numbers as numbers,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://api.github.com";
pub const ACTIONS_SET: &str = "github-actions";
pub const BAZELVERSION_SET: &str = "bazel";
pub const BAZELVERSION_PACKAGE: &str = ".bazelversion";

struct Release {
    tag: String,
    published: Option<String>,
}

/// Checks each remote `uses:` pin of the workflows against the newest release of its repo.
pub fn actions(
    workflows: &[(String, String)],
    only: &[String],
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut pins: BTreeMap<String, Vec<Option<String>>> = BTreeMap::new();
    for (path, text) in workflows {
        let parsed = match parse_workflow_pins(text) {
            Ok(parsed) => parsed,
            Err(error) => {
                found
                    .errors
                    .push(problem(ACTIONS_SET, path, error.to_string()));
                continue;
            }
        };
        for pin in parsed.pins {
            let Some((action, reference)) = split_pin(&pin.value) else {
                continue;
            };
            if action.starts_with("./") || action.starts_with("docker://") {
                continue;
            }
            let mut parts = action.split('/');
            let (Some(owner), Some(repo)) = (parts.next(), parts.next()) else {
                continue;
            };
            let package = format!("{owner}/{repo}").to_ascii_lowercase();
            if !only.is_empty() && !only.iter().any(|name| name.eq_ignore_ascii_case(&package)) {
                continue;
            }
            pins.entry(package)
                .or_default()
                .push(current_version(reference, pin.comment.as_deref()));
        }
    }
    for (package, currents) in pins {
        let mut release = None;
        for current in currents {
            let Some(current) = current else {
                found.unknown.push(problem(
                    ACTIONS_SET,
                    &package,
                    "no version tag or `# vN` comment".to_owned(),
                ));
                continue;
            };
            if release.is_none() {
                release = Some(latest_release(&package, base, fetch));
            }
            judge(ACTIONS_SET, &package, current, release.as_ref(), &mut found);
        }
    }
    found.unknown.dedup();
    found.errors.dedup();
    found.candidates.dedup();
    found
}

/// Checks the Bazel version in `.bazelversion` against the newest bazelbuild/bazel release.
pub fn bazelversion(text: &str, base: &str, fetch: &dyn Fetch) -> Findings {
    let mut found = Findings::default();
    let current = text.trim();
    if numbers(current).is_none() {
        found.unknown.push(problem(
            BAZELVERSION_SET,
            BAZELVERSION_PACKAGE,
            format!("`{current}` is not a version number"),
        ));
        return found;
    }
    let release = latest_release("bazelbuild/bazel", base, fetch);
    judge(
        BAZELVERSION_SET,
        BAZELVERSION_PACKAGE,
        current.to_owned(),
        Some(&release),
        &mut found,
    );
    for candidate in &mut found.candidates {
        candidate.repo = Some("bazelbuild/bazel".to_owned());
    }
    found
}

fn problem(set: &str, package: &str, reason: String) -> LookupError {
    LookupError {
        set: set.to_owned(),
        package: package.to_owned(),
        reason,
    }
}

fn current_version(reference: &str, comment: Option<&str>) -> Option<String> {
    let sha = reference.len() == 40 && reference.chars().all(|c| c.is_ascii_hexdigit());
    let candidate = if sha {
        comment?.split_whitespace().next()?
    } else {
        reference
    };
    numbers(candidate).map(|_| candidate.to_owned())
}

fn judge(
    set: &str,
    package: &str,
    current: String,
    release: Option<&Result<Release, LookupError>>,
    found: &mut Findings,
) {
    let release = match release {
        Some(Ok(release)) => release,
        Some(Err(error)) => {
            let error = LookupError {
                set: set.to_owned(),
                package: package.to_owned(),
                reason: error.reason.clone(),
            };
            if error.reason == NO_RELEASE {
                found.unknown.push(error);
            } else {
                found.errors.push(error);
            }
            return;
        }
        None => return,
    };
    let (Some(have), Some(newest)) = (numbers(&current), numbers(&release.tag)) else {
        return;
    };
    let mut newest = newest;
    newest.resize(newest.len().max(have.len()), 0);
    if newest[..have.len()] > have[..] {
        found.candidates.push(Candidate {
            set: set.to_owned(),
            package: package.to_owned(),
            current,
            latest: release.tag.clone(),
            published: release.published.clone(),
            prerelease: false,
            repo: super::github_repo(&format!("github.com/{package}")),
            tags: None,
        });
    }
}

const NO_RELEASE: &str = "no GitHub release";

/// Resolves a tag or branch of `package` to its commit SHA.
pub fn commit_sha(
    package: &str,
    tag: &str,
    base: &str,
    fetch: &dyn Fetch,
) -> Result<String, DxError> {
    let mut encoded = String::new();
    for byte in tag.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    let body = fetch.get(&format!("{base}/repos/{package}/commits/{encoded}"))?;
    resolve_sha_from_commit_response(&body).map_err(|error| DxError::failure(error.to_string()))
}

fn latest_release(package: &str, base: &str, fetch: &dyn Fetch) -> Result<Release, LookupError> {
    let fail = |reason: String| problem("", package, reason);
    let body = fetch
        .get(&format!("{base}/repos/{package}/releases/latest"))
        .map_err(|reason| {
            fail(if reason.message() == "HTTP 404" {
                NO_RELEASE.to_owned()
            } else {
                reason.to_string()
            })
        })?;
    let json: Value = serde_json::from_str(&body).map_err(|e| fail(e.to_string()))?;
    let tag = json
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| fail("release has no tag_name".to_owned()))?;
    if tag.contains('-') {
        return Err(fail(NO_RELEASE.to_owned()));
    }
    Ok(Release {
        tag: tag.to_owned(),
        published: json
            .get("published_at")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn flow(uses: &str) -> (String, String) {
        (
            "w.yml".to_owned(),
            format!("jobs:\n  a:\n    steps:\n      - uses: {uses}\n"),
        )
    }

    fn release(tag: &str) -> Result<String, DxError> {
        Ok(format!(
            "{{\"tag_name\":\"{tag}\",\"published_at\":\"2026-01-01T00:00:00Z\"}}"
        ))
    }

    fn run(uses: &str, body: Result<String, DxError>) -> Findings {
        actions(&[flow(uses)], &[], "b", &move |_: &str| body.clone())
    }

    #[test]
    fn commit_sha_asks_for_the_encoded_tag() {
        let seen = std::cell::RefCell::new(String::new());
        let got = commit_sha("o/r", "release/v5", "b", &|url: &str| {
            *seen.borrow_mut() = url.to_owned();
            Ok(format!("{{\"sha\":\"{SHA}\"}}"))
        });
        assert_eq!(got.as_deref(), Ok(SHA));
        assert_eq!(*seen.borrow(), "b/repos/o/r/commits/release%2Fv5");
    }

    #[test]
    fn commit_sha_reports_fetch_and_upstream_errors() {
        let missing = commit_sha("o/r", "v9", "b", &|_: &str| {
            Err(DxError::failure("HTTP 404"))
        });
        assert_eq!(missing, Err(DxError::failure("HTTP 404")));
        let upstream = commit_sha("o/r", "v9", "b", &|_: &str| {
            Ok("{\"message\":\"No commit found\"}".to_owned())
        });
        assert!(
            upstream
                .expect_err("error")
                .message()
                .contains("No commit found")
        );
    }

    #[test]
    fn comment_version_behind_is_a_candidate() {
        let found = run(&format!("o/r@{SHA} # v4"), release("v5.0.0"));
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].current, "v4");
        assert_eq!(found.candidates[0].latest, "v5.0.0");
        assert_eq!(
            found.candidates[0].published.as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn same_major_is_current() {
        let found = run("o/r@v7", release("v7.0.1"));
        assert_eq!(found, Findings::default());
    }

    #[test]
    fn newer_pin_is_not_a_downgrade() {
        assert_eq!(run("o/r@v9", release("v7.0.1")), Findings::default());
    }

    #[test]
    fn unreadable_pins_are_unknown() {
        for uses in [format!("o/r@{SHA}"), "o/r@main".to_owned()] {
            let found = run(&uses, release("v5.0.0"));
            assert_eq!(found.unknown.len(), 1, "{uses}");
            assert!(found.candidates.is_empty());
        }
    }

    #[test]
    fn local_and_docker_pins_are_skipped() {
        let fetch = |_: &str| -> Result<String, DxError> { panic!("no fetch") };
        let flows = [flow("./local"), flow("docker://alpine:3")];
        assert_eq!(actions(&flows, &[], "b", &fetch), Findings::default());
    }

    #[test]
    fn missing_release_is_unknown_and_bad_json_is_an_error() {
        let found = run("o/r@v1", Err(DxError::failure("HTTP 404")));
        assert_eq!(found.unknown[0].reason, "no GitHub release");
        assert!(found.errors.is_empty());
        let found = run("o/r@v1", Ok("nope".into()));
        assert_eq!(found.errors.len(), 1);
        let found = run("o/r@v1", Err(DxError::failure("HTTP 403")));
        assert_eq!(found.errors.len(), 1);
    }

    #[test]
    fn prerelease_tags_are_not_offered() {
        let found = run("o/r@v1", release("v2.0.0-rc1"));
        assert!(found.candidates.is_empty());
    }

    #[test]
    fn one_fetch_per_repo() {
        let calls = Cell::new(0);
        let fetch = |url: &str| {
            calls.set(calls.get() + 1);
            assert_eq!(url, "b/repos/o/r/releases/latest");
            release("v5.0.0")
        };
        let flows = [flow("o/r@v1"), flow("O/R/sub@v2")];
        let found = actions(&flows, &[], "b", &fetch);
        assert_eq!(calls.get(), 1);
        assert_eq!(found.candidates.len(), 2);
    }

    #[test]
    fn only_filters_repos() {
        let fetch = |_: &str| release("v5.0.0");
        let flows = [flow("o/r@v1"), flow("x/y@v1")];
        let found = actions(&flows, &["x/y".to_owned()], "b", &fetch);
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].package, "x/y");
    }

    #[test]
    fn bazelversion_cases() {
        let fetch = |url: &str| {
            assert_eq!(url, "b/repos/bazelbuild/bazel/releases/latest");
            release("8.4.0")
        };
        let found = bazelversion("8.3.1\n", "b", &fetch);
        assert_eq!(found.candidates[0].latest, "8.4.0");
        assert_eq!(found.candidates[0].package, ".bazelversion");
        assert_eq!(
            found.candidates[0].repo.as_deref(),
            Some("bazelbuild/bazel")
        );
        assert_eq!(bazelversion("9.0.0", "b", &fetch), Findings::default());
        assert_eq!(bazelversion("latest", "b", &fetch).unknown.len(), 1);
    }
}
