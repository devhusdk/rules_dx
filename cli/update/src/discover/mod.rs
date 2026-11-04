//! Finds packages whose newest stable version is outside the declared requirement.

pub mod bcr;
pub mod cargo;
pub mod npm;
pub mod pypi;

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
}

/// Keeps a package when no filter is given or the filter names it.
pub fn wanted(only: &[String], package: &str) -> bool {
    only.is_empty() || only.iter().any(|name| name == package)
}
