//! Finds packages whose newest stable version is outside the declared requirement.

pub mod bcr;
pub mod cabal;
pub mod cargo;
pub mod composer;
pub mod containers;
pub mod cpan;
pub mod github;
pub mod go;
pub mod hex;
pub mod julia;
pub mod maven;
pub mod npm;
pub mod nuget;
pub mod opam;
pub mod pub_dev;
pub mod pypi;
pub mod ruby;
pub mod swiftpm;

/// Reads one registry URL and returns the response body.
pub trait Fetch {
    /// Returns the body, or a short reason the lookup failed.
    fn get(&self, url: &str) -> Result<String, String>;

    /// Returns the `Docker-Content-Digest` a registry reports for a manifest URL.
    fn digest(&self, _url: &str) -> Result<String, String> {
        Err("this fetcher cannot read image digests".to_owned())
    }
}

impl<F: Fn(&str) -> Result<String, String>> Fetch for F {
    fn get(&self, url: &str) -> Result<String, String> {
        self(url)
    }
}

/// One package that is outdated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub set: String,
    pub package: String,
    pub current: String,
    pub latest: String,
    pub published: Option<String>,
    pub prerelease: bool,
    /// The GitHub repository as `owner/name`, when the registry names one.
    pub repo: Option<String>,
    /// The exact old and new tag names in `repo`, when they are not the bare versions.
    pub tags: Option<(String, String)>,
}

/// One package that could not be looked up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupError {
    pub set: String,
    pub package: String,
    pub reason: String,
}

/// A declared version that dependency resolution replaced in the lock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Overridden {
    pub set: String,
    pub package: String,
    pub requested: String,
    pub resolved: String,
}

/// Everything one set produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Findings {
    pub candidates: Vec<Candidate>,
    pub errors: Vec<LookupError>,
    pub unknown: Vec<LookupError>,
    pub overridden: Vec<Overridden>,
}

/// Reads `owner/name` from a GitHub repository URL or `github:owner/name` shorthand.
pub fn github_repo(url: &str) -> Option<String> {
    let url = url.trim();
    let path = url
        .strip_prefix("github:")
        .or_else(|| url.split_once("github.com").map(|(_, rest)| rest))?;
    let mut parts = path.trim_start_matches([':', '/']).split('/');
    let owner = parts.next().filter(|part| !part.is_empty())?;
    let name = parts.next()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    let name = name
        .split(['#', '?'])
        .next()
        .filter(|part| !part.is_empty())?;
    Some(format!("{owner}/{name}"))
}

/// Keeps a package when no filter is given or the filter names it.
pub fn wanted(only: &[String], package: &str) -> bool {
    only.is_empty() || only.iter().any(|name| name == package)
}

/// The registry a set's packages come from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Registry {
    Bcr,
    Hackage,
    Crates,
    Npm,
    Pypi,
    Go,
    Maven,
    Nuget,
    Rubygems,
    Packagist,
    Metacpan,
    SwiftUrl,
    Opam,
    PubDev,
    Hex,
    JuliaGeneral,
    GitHub,
    BazelReleases,
}

impl Registry {
    /// The page that lists a package's releases.
    pub fn url(self, package: &str) -> String {
        match self {
            Registry::Bcr => format!("https://registry.bazel.build/modules/{package}"),
            Registry::Hackage => format!("https://hackage.haskell.org/package/{package}"),
            Registry::Crates => format!("https://crates.io/crates/{package}"),
            Registry::Npm => format!("https://www.npmjs.com/package/{package}"),
            Registry::Pypi => format!("https://pypi.org/project/{package}"),
            Registry::Go => format!("https://pkg.go.dev/{package}"),
            Registry::Maven => match package.split_once(':') {
                Some((group, artifact)) => {
                    format!("https://central.sonatype.com/artifact/{group}/{artifact}")
                }
                None => format!("https://central.sonatype.com/artifact/{package}"),
            },
            Registry::Nuget => format!("https://www.nuget.org/packages/{package}"),
            Registry::Rubygems => format!("https://rubygems.org/gems/{package}"),
            Registry::Packagist => format!("https://packagist.org/packages/{package}"),
            Registry::Metacpan => format!("https://metacpan.org/pod/{package}"),
            Registry::SwiftUrl => format!("https://{package}/tags"),
            Registry::Opam => format!("https://opam.ocaml.org/packages/{package}"),
            Registry::PubDev => format!("https://pub.dev/packages/{package}"),
            Registry::Hex => format!("https://hex.pm/packages/{package}"),
            Registry::JuliaGeneral => format!("https://juliahub.com/ui/Packages/General/{package}"),
            Registry::GitHub => format!("https://github.com/{package}/releases"),
            Registry::BazelReleases => "https://github.com/bazelbuild/bazel/releases".to_owned(),
        }
    }
}

/// Compares dotted numeric versions; a missing segment counts as 0.
pub(crate) fn numeric_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let segments = |text: &str| -> Vec<u64> {
        text.split('.')
            .map(|part| {
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    };
    let (a, b) = (segments(a), segments(b));
    (0..a.len().max(b.len()))
        .map(|i| {
            a.get(i)
                .copied()
                .unwrap_or(0)
                .cmp(&b.get(i).copied().unwrap_or(0))
        })
        .find(|order| order.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}

/// Compares versions where a suffix such as `-rc.1` or `.rc1` sorts below the bare release.
pub(crate) fn release_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |text: &'_ str| -> (String, String) {
        let end = text
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(text.len());
        let (core, suffix) = text.split_at(end);
        (
            core.trim_end_matches('.').to_owned(),
            suffix.trim_start_matches(['.', '-']).to_owned(),
        )
    };
    let (core_a, suffix_a) = split(a);
    let (core_b, suffix_b) = split(b);
    numeric_cmp(&core_a, &core_b).then_with(|| match (suffix_a.is_empty(), suffix_b.is_empty()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => numeric_cmp(&suffix_a, &suffix_b).then_with(|| suffix_a.cmp(&suffix_b)),
    })
}

#[cfg(test)]
mod tests {
    use super::{Registry, github_repo, numeric_cmp, release_cmp};
    use std::cmp::Ordering;

    #[test]
    fn github_repo_reads_the_usual_url_shapes() {
        for url in [
            "git+https://github.com/o/r.git",
            "https://github.com/o/r/tree/main",
            "git@github.com:o/r.git",
            "github:o/r",
        ] {
            assert_eq!(github_repo(url).as_deref(), Some("o/r"), "{url}");
        }
        assert_eq!(github_repo("https://gitlab.com/o/r"), None);
        assert_eq!(github_repo("https://github.com/o"), None);
    }

    #[test]
    fn registry_urls_for_every_ecosystem() {
        let url = |r: Registry, p: &str| r.url(p);
        assert_eq!(
            url(Registry::Go, "golang.org/x/text"),
            "https://pkg.go.dev/golang.org/x/text"
        );
        assert_eq!(
            url(Registry::Maven, "com.google.guava:guava"),
            "https://central.sonatype.com/artifact/com.google.guava/guava"
        );
        assert_eq!(
            url(Registry::Nuget, "Newtonsoft.Json"),
            "https://www.nuget.org/packages/Newtonsoft.Json"
        );
        assert_eq!(
            url(Registry::Rubygems, "rake"),
            "https://rubygems.org/gems/rake"
        );
        assert_eq!(
            url(Registry::GitHub, "actions/checkout"),
            "https://github.com/actions/checkout/releases"
        );
        assert_eq!(
            url(Registry::BazelReleases, ".bazelversion"),
            "https://github.com/bazelbuild/bazel/releases"
        );
    }

    #[test]
    fn numeric_cmp_pads_missing_segments_with_zero() {
        assert_eq!(numeric_cmp("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(numeric_cmp("1.10", "1.9"), Ordering::Greater);
        assert_eq!(numeric_cmp("1.2", "1.2.1"), Ordering::Less);
    }

    #[test]
    fn release_cmp_puts_suffixed_versions_below_the_release() {
        assert_eq!(release_cmp("2.1.0", "2.1.0-rc.1"), Ordering::Greater);
        assert_eq!(release_cmp("2.1.0-rc.2", "2.1.0-rc.1"), Ordering::Greater);
        assert_eq!(release_cmp("8.0.0", "8.0.0.rc1"), Ordering::Greater);
        assert_eq!(release_cmp("1.10", "1.9"), Ordering::Greater);
    }
}
