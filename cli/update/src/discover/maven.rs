//! Lists Maven artifacts whose newest release on Maven Central is newer than the declared version.

use super::{Candidate, Fetch, Findings, LookupError, Overridden, wanted};
use dx_module_bazel::ModuleFile;
use mvn_version::ComparableVersion;
use std::collections::BTreeMap;

pub const DEFAULT_BASE: &str = "https://repo1.maven.org/maven2";

const PRERELEASE: [&str; 6] = ["snapshot", "alpha", "beta", "rc", "cr", "preview"];

/// Checks each literal coordinate of the `maven.install` named `set` against the repository at `base`.
/// `resolved` maps `group:artifact` to the version the lock holds; that version is the current one,
/// and a coordinate that asked for another is reported as overridden.
pub fn outdated(
    set: &str,
    module: &ModuleFile,
    resolved: &BTreeMap<String, String>,
    only: &[String],
    prereleases: bool,
    base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let artifacts = match module.maven_artifacts_of(set) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            found.errors.push(LookupError {
                set: set.to_owned(),
                package: "maven.install".to_owned(),
                reason: error.to_string(),
            });
            return found;
        }
    };
    for artifact in artifacts {
        let parts: Vec<&str> = artifact.value.split(':').collect();
        if !(3..=5).contains(&parts.len()) {
            continue;
        }
        let package = format!("{}:{}", parts[0], parts[1]);
        if !wanted(only, &package) {
            continue;
        }
        let requested = parts[parts.len() - 1];
        let current = resolved.get(&package).map_or(requested, String::as_str);
        if current != requested {
            found.overridden.push(Overridden {
                set: set.to_owned(),
                package: package.clone(),
                requested: requested.to_owned(),
                resolved: current.to_owned(),
            });
        }
        let url = format!(
            "{base}/{}/{}/maven-metadata.xml",
            parts[0].replace('.', "/"),
            parts[1]
        );
        match fetch.get(&url).and_then(|body| newest(&body, prereleases)) {
            Ok(latest) => {
                if ComparableVersion::new(&latest) > ComparableVersion::new(current) {
                    let prerelease = !is_release(&latest);
                    found.candidates.push(Candidate {
                        set: set.to_owned(),
                        package,
                        current: current.to_owned(),
                        latest,
                        published: None,
                        prerelease,
                        repo: None,
                        tags: None,
                    });
                }
            }
            Err(reason) => found.errors.push(LookupError {
                set: set.to_owned(),
                package,
                reason,
            }),
        }
    }
    found
}

fn is_release(version: &str) -> bool {
    !version.to_ascii_lowercase().split(['-', '.']).any(|token| {
        PRERELEASE.iter().any(|word| token.starts_with(word))
            || token == "milestone"
            || token.starts_with("milestone")
            || (token.starts_with('m') && token[1..].starts_with(|c: char| c.is_ascii_digit()))
    })
}

fn newest(body: &str, prereleases: bool) -> Result<String, String> {
    let versions = body
        .split_once("<versions>")
        .and_then(|(_, rest)| rest.split_once("</versions>"))
        .map(|(inside, _)| inside)
        .ok_or_else(|| "no versions in maven-metadata.xml".to_owned())?;
    versions
        .split("<version>")
        .skip(1)
        .filter_map(|chunk| chunk.split_once("</version>").map(|(v, _)| v.trim()))
        .filter(|version| !version.is_empty() && (prereleases || is_release(version)))
        .max_by_key(|version| ComparableVersion::new(version))
        .map(str::to_owned)
        .ok_or_else(|| "no release version".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(artifacts = [\"junit:junit:4.12\", \"org.apache.commons:commons-lang3:3.12.0\", \"com.x:fresh:1.0\", \"com.x:gone:1.0\", \"bad:coordinate\", \"g:a:jar:sources:1.0\"])\n";

    fn metadata(versions: &[&str]) -> String {
        let list: String = versions
            .iter()
            .map(|v| format!("<version>{v}</version>"))
            .collect();
        format!(
            "<metadata><versioning><latest>zzz</latest><versions>{list}</versions></versioning></metadata>"
        )
    }

    fn repo(url: &str) -> Result<String, String> {
        let path = url
            .strip_prefix("https://repo1.maven.org/maven2/")
            .unwrap_or(url);
        Ok(match path {
            "junit/junit/maven-metadata.xml" => metadata(&[
                "4.12",
                "4.13.2",
                "4.14-SNAPSHOT",
                "5.0.0-M1",
                "5.0.0-rc1",
                "5.0-alpha-2",
            ]),
            "org/apache/commons/commons-lang3/maven-metadata.xml" => {
                metadata(&["3.12.0", "3.13.0", "3.14.0-beta1"])
            }
            "com/x/fresh/maven-metadata.xml" => metadata(&["1.0", "1.0-RC1"]),
            "g/a/maven-metadata.xml" => metadata(&["1.0", "2.0"]),
            _ => return Err("HTTP 404".to_owned()),
        })
    }

    fn run(only: &[String]) -> Findings {
        let module = ModuleFile::parse(MODULE).expect("parse");
        outdated(
            "maven",
            &module,
            &BTreeMap::new(),
            only,
            false,
            DEFAULT_BASE,
            &repo,
        )
    }

    #[test]
    fn prereleases_on_lists_the_newest_prerelease() {
        let module = ModuleFile::parse(MODULE).expect("parse");
        let only = ["junit:junit".to_owned()];
        let found = outdated(
            "maven",
            &module,
            &BTreeMap::new(),
            &only,
            true,
            DEFAULT_BASE,
            &repo,
        );
        assert_eq!(found.candidates[0].latest, "5.0.0-rc1");
        assert!(found.candidates[0].prerelease);
    }

    #[test]
    fn the_newest_release_is_listed() {
        let found = run(&[]);
        assert_eq!(
            found.candidates.iter().find(|c| c.package == "junit:junit"),
            Some(&Candidate {
                set: "maven".into(),
                package: "junit:junit".into(),
                current: "4.12".into(),
                latest: "4.13.2".into(),
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            })
        );
        let lang = found
            .candidates
            .iter()
            .find(|c| c.package.contains("lang3"));
        assert_eq!(lang.map(|c| c.latest.as_str()), Some("3.13.0"));
    }

    #[test]
    fn prereleases_and_current_artifacts_are_not_listed() {
        let found = run(&[]);
        assert!(!found.candidates.iter().any(|c| c.package == "com.x:fresh"));
    }

    #[test]
    fn short_coordinates_are_skipped_and_long_ones_use_the_last_part() {
        let found = run(&[]);
        assert!(!found.candidates.iter().any(|c| c.package.contains("bad")));
        let long = found.candidates.iter().find(|c| c.package == "g:a");
        assert_eq!(long.map(|c| c.current.as_str()), Some("1.0"));
        assert_eq!(long.map(|c| c.latest.as_str()), Some("2.0"));
    }

    #[test]
    fn failed_lookups_are_errors() {
        let found = run(&[]);
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "com.x:gone");
    }

    #[test]
    fn the_package_filter_limits_lookups() {
        let found = run(&["junit:junit".to_owned()]);
        assert_eq!(found.candidates.len(), 1);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn the_group_becomes_a_path() {
        let module = ModuleFile::parse(MODULE).expect("parse");
        let seen = std::cell::RefCell::new(Vec::new());
        outdated(
            "maven",
            &module,
            &BTreeMap::new(),
            &["org.apache.commons:commons-lang3".to_owned()],
            false,
            "https://m",
            &|url: &str| {
                seen.borrow_mut().push(url.to_owned());
                Err("HTTP 404".to_owned())
            },
        );
        assert_eq!(
            seen.into_inner(),
            ["https://m/org/apache/commons/commons-lang3/maven-metadata.xml"]
        );
    }

    #[test]
    fn release_shape_skips_qualifier_tokens() {
        for bad in [
            "1.0-SNAPSHOT",
            "1.0-alpha-2",
            "1.0.beta1",
            "1.0-rc1",
            "1.0-CR2",
            "1.0-M3",
            "1.0-milestone-1",
            "1.0-preview",
        ] {
            assert!(!is_release(bad), "{bad}");
        }
        for good in ["1.0", "2.0.0", "33.0.0-jre", "1.0.Final"] {
            assert!(is_release(good), "{good}");
        }
    }

    #[test]
    fn the_resolved_version_is_current_and_an_override_is_flagged() {
        let module = ModuleFile::parse(MODULE).expect("parse");
        let resolved = BTreeMap::from([
            ("junit:junit".to_owned(), "4.13.2".to_owned()),
            ("g:a".to_owned(), "1.0".to_owned()),
        ]);
        let found = outdated(
            "maven",
            &module,
            &resolved,
            &["junit:junit".to_owned(), "g:a".to_owned()],
            false,
            DEFAULT_BASE,
            &repo,
        );
        assert!(!found.candidates.iter().any(|c| c.package == "junit:junit"));
        assert_eq!(
            found.overridden,
            [Overridden {
                set: "maven".into(),
                package: "junit:junit".into(),
                requested: "4.12".into(),
                resolved: "4.13.2".into(),
            }]
        );
        let long = found.candidates.iter().find(|c| c.package == "g:a");
        assert_eq!(long.map(|c| c.current.as_str()), Some("1.0"));
    }
}
