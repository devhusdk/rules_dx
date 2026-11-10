//! Lists NuGet packages whose newest stable version on nuget.org is newer than the paket.dependencies requirement.

use dx_error::DxError;

use super::{Candidate, Fetch, Findings, LookupError, numeric_cmp, release_cmp, wanted};
use crate::edit::request::plans::paket_tokens;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://api.nuget.org/v3-flatcontainer";

/// Checks each `nuget` line of the paket.dependencies files against the feed at `base`.
pub fn outdated(
    set: &str,
    files: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut declared: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in files {
        for line in text.lines() {
            if let Some((id, current)) = declaration(line) {
                let versions = declared.entry(id).or_default();
                if !versions.contains(&current) {
                    versions.push(current);
                }
            }
        }
    }
    for (id, versions) in declared {
        if !wanted(only, &id) {
            continue;
        }
        let url = format!("{base}/{}/index.json", id.to_ascii_lowercase());
        let latest = match fetch.get(&url).and_then(|body| newest(&body, prereleases)) {
            Ok(latest) => latest,
            Err(reason) => {
                found.errors.push(LookupError {
                    set: set.to_owned(),
                    package: id,
                    reason: reason.to_string(),
                });
                continue;
            }
        };
        for current in versions {
            if release_cmp(&latest, &current) == Ordering::Greater {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: id.clone(),
                    current,
                    latest: latest.clone(),
                    published: None,
                    prerelease: latest.contains('-'),
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

fn declaration(line: &str) -> Option<(String, String)> {
    let tokens = paket_tokens(line);
    let [(_, _, kind), (_, _, id), rest @ ..] = tokens.as_slice() else {
        return None;
    };
    if !kind.eq_ignore_ascii_case("nuget") {
        return None;
    }
    let mut best: Option<&str> = None;
    for (_, _, token) in rest {
        if token.contains(':') {
            break;
        }
        let literal = token.trim_start_matches(['>', '<', '=', '~', '@']);
        if literal.starts_with(|c: char| c.is_ascii_digit())
            && best.is_none_or(|b| numeric_cmp(literal, b) == Ordering::Greater)
        {
            best = Some(literal);
        }
    }
    best.map(|current| ((*id).to_owned(), current.to_owned()))
}

fn newest(body: &str, prereleases: bool) -> Result<String, DxError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| DxError::failure(e.to_string()))?;
    json["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str())
        .filter(|v| {
            (prereleases || !v.contains('-')) && v.starts_with(|c: char| c.is_ascii_digit())
        })
        .max_by(|a, b| release_cmp(a, b))
        .map(str::to_owned)
        .ok_or_else(|| DxError::failure("no stable version"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAKET: &str = "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json ~> 12.0 >= 11.0.1\nnuget Fresh 2.0.0\nnuget Gone 1.0.0\nnuget Unversioned\ngithub owner/repo\n# nuget Commented 1.0\n";

    fn feed(url: &str) -> Result<String, DxError> {
        let path = url
            .strip_prefix("https://api.nuget.org/v3-flatcontainer/")
            .unwrap_or(url);
        Ok(match path {
            "newtonsoft.json/index.json" => {
                r#"{"versions":["12.0.1","13.0.1","13.0.3","14.0.0-beta1"]}"#
            }
            "fresh/index.json" => r#"{"versions":["2.0.0","2.1.0-rc.1"]}"#,
            _ => return Err(DxError::failure("HTTP 404")),
        }
        .to_owned())
    }

    fn run(only: &[String]) -> Findings {
        outdated(
            "nuget",
            &[PAKET.to_owned()],
            only,
            false,
            DEFAULT_BASE,
            &feed,
        )
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let only = ["Fresh".to_owned()];
        let found = outdated(
            "nuget",
            &[PAKET.to_owned()],
            &only,
            true,
            DEFAULT_BASE,
            &feed,
        );
        assert_eq!(found.candidates[0].latest, "2.1.0-rc.1");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn the_newest_stable_version_is_listed_against_the_highest_literal() {
        let found = run(&[]);
        assert_eq!(
            found.candidates,
            [Candidate {
                set: "nuget".into(),
                package: "Newtonsoft.Json".into(),
                current: "12.0".into(),
                latest: "13.0.3".into(),
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            }]
        );
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "Gone");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["Fresh".to_owned()]);
        assert!(found.candidates.is_empty() && found.errors.is_empty());
    }

    #[test]
    fn the_id_is_lowercased_in_the_url() {
        let seen = std::cell::RefCell::new(Vec::new());
        outdated(
            "nuget",
            &[PAKET.to_owned()],
            &["Gone".to_owned()],
            false,
            "https://n",
            &|url: &str| {
                seen.borrow_mut().push(url.to_owned());
                Err(DxError::failure("HTTP 404"))
            },
        );
        assert_eq!(seen.into_inner(), ["https://n/gone/index.json"]);
    }
}
