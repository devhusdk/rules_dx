use std::sync::{Mutex, PoisonError};

use dx_lock::{Error, Fetch};

pub struct FixtureFetch {
    fixtures: Vec<(String, String)>,
    asked: Mutex<Vec<String>>,
}

impl FixtureFetch {
    #[must_use]
    pub fn new(fixtures: &[(&str, &str)]) -> Self {
        Self {
            fixtures: fixtures
                .iter()
                .map(|&(suffix, env)| (suffix.to_owned(), env.to_owned()))
                .collect(),
            asked: Mutex::default(),
        }
    }

    pub fn bytes(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.to_owned());
        let env = self
            .fixtures
            .iter()
            .find(|(suffix, _)| url.ends_with(suffix.as_str()))
            .map(|(_, env)| env)
            .ok_or_else(|| Error::new(format!("unexpected download of {url}")))?;
        std::fs::read(dx_testing::env_file(env)).map_err(|error| Error::new(error.to_string()))
    }

    #[must_use]
    pub fn asked(&self) -> Vec<String> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Fetch for FixtureFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Ok(dx_digest::sha256_hex(&self.bytes(url)?))
    }
}

pub struct NoFetch;

impl Fetch for NoFetch {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        Err(Error::new(format!("unexpected download of {url}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "DX_LOCK_TESTING_FIXTURE";

    #[test]
    fn a_matching_suffix_serves_its_fixture_and_digest() {
        let fetch = FixtureFetch::new(&[("/a-1.tar.gz", FIXTURE)]);
        let expected = std::fs::read(dx_testing::env_file(FIXTURE)).expect("fixture");
        assert_eq!(
            fetch.bytes("https://example.com/a-1.tar.gz"),
            Ok(expected.clone())
        );
        assert_eq!(
            fetch.sha256("https://mirror.example.com/a-1.tar.gz"),
            Ok(dx_digest::sha256_hex(&expected))
        );
        assert_eq!(
            fetch.asked(),
            [
                "https://example.com/a-1.tar.gz",
                "https://mirror.example.com/a-1.tar.gz"
            ]
        );
    }

    #[test]
    fn an_unknown_url_fails_and_is_recorded() {
        let fetch = FixtureFetch::new(&[("/a-1.tar.gz", FIXTURE)]);
        assert_eq!(
            fetch.sha256("https://example.com/a-1.zip"),
            Err(Error::new(
                "unexpected download of https://example.com/a-1.zip"
            ))
        );
        assert_eq!(fetch.asked(), ["https://example.com/a-1.zip"]);
    }

    #[test]
    fn no_fetch_fails_every_url() {
        assert_eq!(
            NoFetch.sha256("https://example.com/a"),
            Err(Error::new("unexpected download of https://example.com/a"))
        );
    }
}
