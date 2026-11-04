//! Finds packages whose newest stable version is outside the declared requirement.

pub mod bcr;
pub mod cargo;
pub mod github;
pub mod go;
pub mod maven;
pub mod npm;
pub mod nuget;
pub mod pypi;
pub mod ruby;

/// Reads one registry URL and returns the response body.
pub trait Fetch {
    /// Returns the body, or a short reason the lookup failed.
    fn get(&self, url: &str) -> Result<String, String>;
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
}

/// One package that could not be looked up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupError {
    pub set: String,
    pub package: String,
    pub reason: String,
}

/// Everything one set produced.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Findings {
    pub candidates: Vec<Candidate>,
    pub errors: Vec<LookupError>,
    pub unknown: Vec<LookupError>,
}

/// Keeps a package when no filter is given or the filter names it.
pub fn wanted(only: &[String], package: &str) -> bool {
    only.is_empty() || only.iter().any(|name| name == package)
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

#[cfg(test)]
mod tests {
    use super::numeric_cmp;
    use std::cmp::Ordering;

    #[test]
    fn numeric_cmp_pads_missing_segments_with_zero() {
        assert_eq!(numeric_cmp("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(numeric_cmp("1.10", "1.9"), Ordering::Greater);
        assert_eq!(numeric_cmp("1.2", "1.2.1"), Ordering::Less);
    }
}
