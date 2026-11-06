//! Lists cabal packages whose newest preferred release on Hackage is newer than the freeze file pin.

use super::{numeric_cmp, wanted, Candidate, Fetch, Findings, LookupError};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_BASE: &str = "https://hackage.haskell.org";

/// One package named in a `build-depends` field of a `.cabal` file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Depend {
    pub name: String,
    /// Byte range of the version bound after the name; empty when the package has none.
    pub bound: std::ops::Range<usize>,
}

/// Checks each package of the `.cabal` files that the freeze files pin against Hackage at `base`.
pub fn outdated(
    set: &str,
    cabal_files: &[String],
    freezes: &[String],
    only: &[String],
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let declared: BTreeSet<String> = cabal_files
        .iter()
        .flat_map(|text| build_depends(text))
        .map(|depend| depend.name)
        .collect();
    let mut pinned: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in freezes {
        for (name, version) in frozen(text) {
            if declared.contains(&name) {
                let versions = pinned.entry(name).or_default();
                if !versions.contains(&version) {
                    versions.push(version);
                }
            }
        }
    }
    for (name, versions) in pinned {
        if !wanted(only, &name) {
            continue;
        }
        let url = format!("{base}/package/{name}/preferred");
        let latest = match fetch.get(&url).and_then(|body| newest(&body)) {
            Ok(latest) => latest,
            Err(reason) => {
                found.errors.push(LookupError {
                    set: set.to_owned(),
                    package: name,
                    reason,
                });
                continue;
            }
        };
        for current in versions {
            if numeric_cmp(&latest, &current) == Ordering::Greater {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: latest.clone(),
                    published: None,
                    prerelease: false,
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

/// Reads the `any.<name> ==<version>` entries of a cabal.project.freeze.
fn frozen(text: &str) -> Vec<(String, String)> {
    let mut constraints = String::new();
    let mut inside = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("constraints:") {
            inside = true;
            constraints.push_str(rest);
        } else if inside && line.starts_with([' ', '\t']) {
            constraints.push_str(line);
        } else {
            inside = false;
        }
    }
    constraints
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            let (name, version) = entry.strip_prefix("any.")?.split_once("==")?;
            Some((name.trim().to_owned(), version.trim().to_owned()))
        })
        .collect()
}

/// The highest version Hackage lists as normal, which leaves out deprecated ones.
fn newest(body: &str) -> Result<String, String> {
    let json: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    json["normal-version"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|version| version.as_str())
        .max_by(|a, b| numeric_cmp(a, b))
        .map(str::to_owned)
        .ok_or_else(|| "no preferred version".to_owned())
}

/// Reads every package of the `build-depends` fields of a `.cabal` file, with the byte range of its bound.
pub(crate) fn build_depends(text: &str) -> Vec<Depend> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut field_indent: Option<usize> = None;
    let mut entry: Option<(usize, usize)> = None;
    let mut finish = |entry: &mut Option<(usize, usize)>| {
        if let Some((start, end)) = entry.take() {
            if let Some(depend) = parse_entry(text, start, end) {
                out.push(depend);
            }
        }
    };
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let trimmed = body.trim_start();
        let indent = body.len() - trimmed.len();
        let mut content = body;
        if let Some(cut) = comment_start(body) {
            content = &body[..cut];
        }
        let continues = field_indent.is_some_and(|field| indent > field)
            || trimmed.is_empty()
            || trimmed.starts_with("--");
        let mut from = 0;
        if !continues {
            finish(&mut entry);
            field_indent = None;
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("build-depends:") {
                field_indent = Some(indent);
                from = indent + "build-depends:".len();
            } else {
                offset += line.len();
                continue;
            }
        }
        if field_indent.is_some() {
            let mut start = from;
            for (at, ch) in content.char_indices().filter(|(at, _)| *at >= from) {
                if ch == ',' {
                    extend(text, &mut entry, offset + start, offset + at);
                    finish(&mut entry);
                    start = at + 1;
                }
            }
            extend(text, &mut entry, offset + start, offset + content.len());
        }
        offset += line.len();
    }
    finish(&mut entry);
    out
}

fn comment_start(line: &str) -> Option<usize> {
    let mut at = 0;
    while let Some(found) = line[at..].find("--") {
        let cut = at + found;
        if cut == 0 || line[..cut].ends_with(char::is_whitespace) {
            return Some(cut);
        }
        at = cut + 2;
    }
    None
}

/// Keeps the first non-blank piece of an entry; a bound has to sit on the line of the name.
fn extend(text: &str, entry: &mut Option<(usize, usize)>, start: usize, end: usize) {
    if entry.is_none() && start < end && !text[start..end].trim().is_empty() {
        *entry = Some((start, end));
    }
}

fn parse_entry(text: &str, start: usize, end: usize) -> Option<Depend> {
    let piece = &text[start..end];
    let lead = piece.len() - piece.trim_start().len();
    let piece = piece.trim();
    let name_end = piece
        .find(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '=' | '^' | '!'))
        .unwrap_or(piece.len());
    let name = &piece[..name_end];
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
    {
        return None;
    }
    let rest = &piece[name_end..];
    let gap = rest.len() - rest.trim_start().len();
    let bound_start = start + lead + name_end + gap;
    Some(Depend {
        name: name.to_owned(),
        bound: bound_start..bound_start + rest.trim().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CABAL: &str = "name: demo\nlibrary\n  build-depends: base, safe ==0.3.21, split ==0.2.5\n  -- note, ignored\n  default-language: Haskell2010\n";
    const FREEZE: &str = "constraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21,\n             any.split ==0.2.5,\n             any.text ==2.1.2\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n";

    fn preferred(url: &str) -> Result<String, String> {
        let name = url
            .strip_prefix("https://hackage.haskell.org/package/")
            .and_then(|rest| rest.strip_suffix("/preferred"))
            .unwrap_or(url);
        Ok(match name {
            "safe" => {
                r#"{"normal-version":["0.3.21","0.3.22","0.3.9"],"deprecated-version":["0.4.0"]}"#
            }
            "split" => r#"{"normal-version":["0.2.5"],"deprecated-version":[]}"#,
            "base" => r#"{"normal-version":["4.20.2.0","4.9.0.0"]}"#,
            _ => return Err(format!("unexpected lookup {name}")),
        }
        .to_owned())
    }

    #[test]
    fn lists_declared_packages_behind_the_newest_normal_version() {
        let found = outdated(
            "hs",
            &[CABAL.to_owned()],
            &[FREEZE.to_owned()],
            &[],
            DEFAULT_BASE,
            &preferred,
        );
        let listed: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(listed, [("safe", "0.3.21", "0.3.22")]);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn a_failed_lookup_is_an_error_not_a_candidate() {
        let fail = |_: &str| Err("offline".to_owned());
        let found = outdated(
            "hs",
            &[CABAL.to_owned()],
            &[FREEZE.to_owned()],
            &["safe".to_owned()],
            DEFAULT_BASE,
            &fail,
        );
        assert!(found.candidates.is_empty());
        assert_eq!(found.errors.len(), 1);
    }

    #[test]
    fn build_depends_reads_leading_comma_lists_and_bounds() {
        let text = "library\n  build-depends:\n      base >=4 && <5\n    , aeson ^>=2.2 -- json\n    , text\nexecutable x\n  build-depends: containers ==0.6.*\n";
        let depends = build_depends(text);
        let seen: Vec<_> = depends
            .iter()
            .map(|d| (d.name.as_str(), &text[d.bound.clone()]))
            .collect();
        assert_eq!(
            seen,
            [
                ("base", ">=4 && <5"),
                ("aeson", "^>=2.2"),
                ("text", ""),
                ("containers", "==0.6.*")
            ]
        );
    }
}
