//! Lists modules whose newest release on MetaCPAN is newer than the version in the cpanfile.

use dx_error::DxError;

use super::{Candidate, Fetch, Findings, LookupError, numeric_cmp, unquote, wanted};
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://fastapi.metacpan.org";

/// Checks each versioned `requires` line of the cpanfiles against the API at `base`.
pub fn outdated(
    set: &str,
    cpanfiles: &[String],
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let mut declared: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in cpanfiles {
        for line in text.lines() {
            if let Some((name, current)) = declaration(line) {
                let versions = declared.entry(name).or_default();
                if !versions.contains(&current) {
                    versions.push(current);
                }
            }
        }
    }
    for (name, versions) in declared {
        if !wanted(only, &name) {
            continue;
        }
        let url = format!("{base}/v1/module/{name}?fields=version,date,maturity");
        let (latest, published, prerelease) = match fetch.get(&url).and_then(|body| newest(&body)) {
            Ok(latest) => latest,
            Err(reason) => {
                found.errors.push(LookupError {
                    set: set.to_owned(),
                    package: name,
                    reason: reason.to_string(),
                });
                continue;
            }
        };
        if prerelease && !prereleases {
            continue;
        }
        for current in versions {
            if version_cmp(&latest, &current) == Ordering::Greater {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: latest.clone(),
                    published: published.clone(),
                    prerelease,
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

fn declaration(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("requires")?;
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let mut args = rest.split(',').map(str::trim);
    let name = unquote(args.next()?)?;
    let requirement = unquote(args.next()?)?;
    let literal = requirement.trim_start_matches(['>', '<', '=', '!', ' ']);
    let digits = literal.strip_prefix('v').unwrap_or(literal);
    digits
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| (name.to_owned(), literal.to_owned()))
}

fn newest(body: &str) -> Result<(String, Option<String>, bool), DxError> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| DxError::failure(e.to_string()))?;
    let version = match &json["version"] {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        _ => return Err(DxError::failure("no version")),
    };
    Ok((
        version,
        json["date"].as_str().map(str::to_owned),
        json["maturity"].as_str() == Some("developer"),
    ))
}

/// Orders CPAN versions: `v1.2.3` and `1.2.3` are dotted, `1.23` is decimal with three
/// digits per segment, so `1.9` is newer than `1.10`.
fn version_cmp(a: &str, b: &str) -> Ordering {
    numeric_cmp(&dotted(a), &dotted(b))
}

fn dotted(version: &str) -> String {
    if let Some(rest) = version.strip_prefix('v') {
        return rest.to_owned();
    }
    let Some((whole, fraction)) = version.split_once('.') else {
        return version.to_owned();
    };
    if fraction.contains('.') {
        return version.to_owned();
    }
    let mut padded = fraction.to_owned();
    while padded.len() % 3 != 0 {
        padded.push('0');
    }
    let segments: Vec<String> = padded
        .as_bytes()
        .chunks(3)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    format!("{whole}.{}", segments.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CPANFILE: &str = "requires 'Clone::PP', '== 1.09';\nrequires 'Carton', '== v1.0.35';\nrequires 'Fresh', '0.5';\nrequires 'NoVersion';\nrequires 'Gone', '1.0';\n  requires 'Dev', '>= 1.0';\n";

    fn api(url: &str) -> Result<String, DxError> {
        let path = url.strip_prefix("https://fastapi.metacpan.org/v1/module/");
        let module = path.and_then(|p| p.split('?').next()).unwrap_or(url);
        Ok(match module {
            "Clone::PP" => {
                r#"{"version":"1.10","date":"2026-01-01T00:00:00","maturity":"released"}"#
            }
            "Carton" => {
                r#"{"version":"v1.0.36","date":"2026-02-01T00:00:00","maturity":"released"}"#
            }
            "Fresh" => r#"{"version":0.5,"maturity":"released"}"#,
            "Dev" => r#"{"version":"2.0","maturity":"developer"}"#,
            _ => return Err(DxError::failure("HTTP 404")),
        }
        .to_owned())
    }

    #[test]
    fn decimal_and_dotted_versions_order_like_perl() {
        assert_eq!(version_cmp("1.9", "1.10"), Ordering::Greater);
        assert_eq!(version_cmp("1.10", "1.09"), Ordering::Greater);
        assert_eq!(version_cmp("v1.0.36", "v1.0.35"), Ordering::Greater);
        assert_eq!(version_cmp("0.5", "0.500"), Ordering::Equal);
        assert_eq!(version_cmp("20260826", "20260825"), Ordering::Greater);
    }

    #[test]
    fn lists_newer_releases_and_reports_failed_lookups() {
        let found = outdated(
            "perl",
            &[CPANFILE.to_owned()],
            &[],
            false,
            DEFAULT_BASE,
            &api,
        );
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                ("Carton", "v1.0.35", "v1.0.36"),
                ("Clone::PP", "1.09", "1.10")
            ]
        );
        assert_eq!(
            found.candidates[0].published.as_deref(),
            Some("2026-02-01T00:00:00")
        );
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "Gone");
    }

    #[test]
    fn developer_releases_need_the_prerelease_flag() {
        let only = ["Dev".to_owned()];
        let text = [CPANFILE.to_owned()];
        assert!(
            outdated("perl", &text, &only, false, DEFAULT_BASE, &api)
                .candidates
                .is_empty()
        );
        let found = outdated("perl", &text, &only, true, DEFAULT_BASE, &api);
        assert!(found.candidates[0].prerelease);
    }
}
