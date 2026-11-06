//! Lists packages of an opam.locked file whose newest version in the pinned opam-repository
//! snapshot is newer than the locked one. It reads the snapshot's `packages/<name>/` directories.

use super::{Candidate, Findings, LookupError, wanted};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::Path;

/// Checks each pinned package of the lock files against the snapshot checkout at `snapshot`.
pub fn outdated(
    set: &str,
    locks: &[String],
    only: &[String],
    prereleases: bool,
    snapshot: &Path,
) -> Findings {
    let mut found = Findings::default();
    let mut pinned: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for text in locks {
        for (name, version) in locked(text) {
            let versions = pinned.entry(name).or_default();
            if !versions.contains(&version) {
                versions.push(version);
            }
        }
    }
    for (name, versions) in pinned {
        if !wanted(only, &name) {
            continue;
        }
        let newest = match newest(snapshot, &name, prereleases) {
            Ok(newest) => newest,
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
            if version_cmp(&newest, &current) == Ordering::Greater {
                found.candidates.push(Candidate {
                    set: set.to_owned(),
                    package: name.clone(),
                    current,
                    latest: newest.clone(),
                    published: None,
                    prerelease: is_prerelease(&newest),
                    repo: None,
                    tags: None,
                });
            }
        }
    }
    found
}

/// Reads the `depends` entries that pin a version with `= "x"`.
fn locked(text: &str) -> Vec<(String, String)> {
    let Some(start) = text.find("depends:") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut name: Option<String> = None;
    let mut depth = 0;
    let mut braces = 0;
    let mut after_equals = false;
    let mut chars = text[start + "depends:".len()..].chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' if braces == 0 => depth += 1,
            ']' if braces == 0 => {
                depth -= 1;
                if depth <= 0 {
                    break;
                }
            }
            '{' => braces += 1,
            '}' => braces -= 1,
            '#' => {
                for rest in chars.by_ref() {
                    if rest == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut literal = String::new();
                while let Some(rest) = chars.next() {
                    match rest {
                        '\\' => literal.extend(chars.next()),
                        '"' => break,
                        _ => literal.push(rest),
                    }
                }
                if braces == 0 {
                    name = Some(literal);
                } else if after_equals
                    && let Some(name) = name.as_ref().filter(|n| !is_toolchain(n))
                    && !out.iter().any(|(seen, _)| seen == name)
                {
                    out.push((name.clone(), literal));
                }
            }
            _ => {}
        }
        if !c.is_whitespace() {
            after_equals = c == '=';
        }
    }
    out
}

fn is_toolchain(name: &str) -> bool {
    name.starts_with("base-")
        || matches!(
            name,
            "ocaml"
                | "ocaml-base-compiler"
                | "ocaml-config"
                | "ocaml-options-vanilla"
                | "ocaml-system"
                | "ocaml-variants"
                | "dune"
        )
}

fn newest(snapshot: &Path, name: &str, prereleases: bool) -> Result<String, String> {
    let dir = snapshot.join("packages").join(name);
    let entries =
        std::fs::read_dir(&dir).map_err(|_| "not in the opam-repository snapshot".to_owned())?;
    let prefix = format!("{name}.");
    let mut versions: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|dir| dir.strip_prefix(&prefix).map(str::to_owned))
        .filter(|version| prereleases || !is_prerelease(version))
        .collect();
    versions.sort_by(|a, b| version_cmp(a, b));
    versions
        .pop()
        .ok_or_else(|| "no version in the snapshot".to_owned())
}

fn is_prerelease(version: &str) -> bool {
    version.contains('~') || version.contains("dev")
}

/// Orders opam versions like Debian: alternating non-digit and digit runs, `~` before all.
fn version_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        let (text_a, rest_a) = split_run(a, |c| !c.is_ascii_digit());
        let (text_b, rest_b) = split_run(b, |c| !c.is_ascii_digit());
        let order = text_cmp(text_a, text_b);
        if order != Ordering::Equal {
            return order;
        }
        let (digits_a, rest_a) = split_run(rest_a, |c| c.is_ascii_digit());
        let (digits_b, rest_b) = split_run(rest_b, |c| c.is_ascii_digit());
        let order = number_cmp(digits_a, digits_b);
        if order != Ordering::Equal {
            return order;
        }
        if rest_a.is_empty() && rest_b.is_empty() {
            return Ordering::Equal;
        }
        (a, b) = (rest_a, rest_b);
    }
}

fn split_run(text: &str, keep: impl Fn(char) -> bool) -> (&str, &str) {
    let end = text.find(|c| !keep(c)).unwrap_or(text.len());
    text.split_at(end)
}

fn number_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn text_cmp(a: &str, b: &str) -> Ordering {
    let weight = |c: Option<char>| match c {
        Some('~') => -1,
        None => 0,
        Some(c) if c.is_ascii_alphabetic() => c as i32,
        Some(c) => c as i32 + 256,
    };
    let (mut a, mut b) = (a.chars(), b.chars());
    loop {
        let (x, y) = (a.next(), b.next());
        if x.is_none() && y.is_none() {
            return Ordering::Equal;
        }
        let order = weight(x).cmp(&weight(y));
        if order != Ordering::Equal {
            return order;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"opam-version: "2.0"
depends: [
  "astring" {= "0.8.5"}
  "cmdliner" {= "1.2.0" & with-test}
  "ocaml" {= "5.2.0"}
  "base-unix" {= "base"}
  "gone" {= "1.0"}
  "fresh" {= "2.0"}
]
build: ["dune" "build"]
"#;

    fn snapshot() -> dx_test_scratch::TempDir {
        let dir = dx_test_scratch::scratch("opam_snapshot");
        for package in [
            "astring/astring.0.8.5",
            "astring/astring.0.8.10",
            "astring/astring.0.9.0~rc1",
            "cmdliner/cmdliner.1.2.0",
            "cmdliner/cmdliner.1.3.0",
            "fresh/fresh.2.0",
        ] {
            dx_test_scratch::write(
                dir.path(),
                &format!("packages/{package}/opam"),
                "opam-version: \"2.0\"\n",
            );
        }
        dir
    }

    #[test]
    fn reads_pinned_versions_and_skips_the_toolchain() {
        assert_eq!(
            locked(LOCK),
            [
                ("astring".to_owned(), "0.8.5".to_owned()),
                ("cmdliner".to_owned(), "1.2.0".to_owned()),
                ("gone".to_owned(), "1.0".to_owned()),
                ("fresh".to_owned(), "2.0".to_owned()),
            ]
        );
    }

    #[test]
    fn lists_newer_snapshot_versions_in_opam_order() {
        let dir = snapshot();
        let found = outdated("ocaml", &[LOCK.to_owned()], &[], false, dir.path());
        let latest: Vec<_> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(latest, [("astring", "0.8.10"), ("cmdliner", "1.3.0")]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "gone");
    }

    #[test]
    fn prereleases_need_the_flag_and_only_filters() {
        let dir = snapshot();
        let found = outdated(
            "ocaml",
            &[LOCK.to_owned()],
            &["astring".to_owned()],
            true,
            dir.path(),
        );
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].latest, "0.9.0~rc1");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn orders_versions_like_opam() {
        assert_eq!(version_cmp("0.8.10", "0.8.5"), Ordering::Greater);
        assert_eq!(version_cmp("1.0~rc1", "1.0"), Ordering::Less);
        assert_eq!(version_cmp("1.0+x", "1.0"), Ordering::Greater);
        assert_eq!(version_cmp("2.0", "2.0"), Ordering::Equal);
    }
}
