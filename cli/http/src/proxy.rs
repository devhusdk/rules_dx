//! Proxy choice from `https_proxy`, `all_proxy`, `http_proxy` and `no_proxy`.

use ureq::Proxy;

use crate::Error;

/// The variables that name the proxy, first set one wins. `http_proxy` comes last so a
/// host that sets only it still reaches `https://` URLs through it, as ureq's own
/// `Proxy::try_from_env` does.
const PROXY_VARIABLES: [&str; 6] = [
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "http_proxy",
    "HTTP_PROXY",
];

/// The proxy for `https://` URLs, with the `no_proxy` hosts built in.
///
/// ureq talks to an HTTP proxy only through `CONNECT`, which many proxies refuse for
/// port 80, so `http://` URLs (local mirrors and test servers) go direct.
#[derive(Clone, Debug)]
pub(crate) struct Proxies {
    /// A bad value only fails the requests that would use it.
    https: Result<Option<Proxy>, Error>,
}

impl Proxies {
    /// Reads the proxy variables through `env`.
    pub(crate) fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Self {
        let bypass = first(env, &["no_proxy", "NO_PROXY"])
            .map(|(_, value)| bypass_entries(&value))
            .unwrap_or_default();
        Self {
            https: first(env, &PROXY_VARIABLES)
                .map(|(name, value)| proxy(name, &value, &bypass))
                .transpose(),
        }
    }

    /// The proxy for `url`, picked by its scheme. ureq keeps it for every redirect hop
    /// and applies the `no_proxy` hosts on each one.
    ///
    /// # Errors
    /// Fails when `url` needs the proxy and its variable is not a proxy URL.
    pub(crate) fn for_url(&self, url: &str) -> Result<Option<Proxy>, Error> {
        let https = url
            .split_once("://")
            .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case("https"));
        if https { self.https.clone() } else { Ok(None) }
    }
}

fn first(
    env: &dyn Fn(&str) -> Option<String>,
    names: &[&'static str],
) -> Option<(&'static str, String)> {
    names
        .iter()
        .find_map(|name| env(name).map(|value| (*name, value)))
}

/// The `no_proxy` list in ureq's syntax. Like curl, `example.com` and `.example.com`
/// both cover the host and every subdomain, but ureq's own `example.com` only covers the host.
fn bypass_entries(value: &str) -> Vec<String> {
    let mut entries = Vec::new();
    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        if entry == "*" {
            entries.push("*".to_owned());
            continue;
        }
        let host = entry.trim_start_matches('*').trim_start_matches('.');
        if host.is_empty() {
            continue;
        }
        entries.push(host.to_owned());
        entries.push(format!(".{host}"));
    }
    entries
}

fn proxy(name: &str, value: &str, bypass: &[String]) -> Result<Proxy, Error> {
    // The value can hold a password, so the error names only the variable.
    let invalid = || Error::Config(format!("{name} is not a proxy URL"));
    let parsed = Proxy::new(value.trim()).map_err(|_| invalid())?;
    let mut builder = Proxy::builder(parsed.protocol())
        .host(parsed.host())
        .port(parsed.port());
    if let Some(user) = parsed.username() {
        builder = builder.username(user);
    }
    if let Some(password) = parsed.password() {
        builder = builder.password(password);
    }
    for entry in bypass {
        builder = builder.no_proxy(entry);
    }
    builder.build().map_err(|_| invalid())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn uri(url: &str) -> ureq::http::Uri {
        url.parse().expect("uri")
    }

    fn https_proxy(pairs: &'static [(&'static str, &'static str)]) -> Proxy {
        Proxies::from_env(&env(pairs))
            .for_url("https://example.com/a")
            .expect("valid proxy")
            .expect("a proxy")
    }

    #[test]
    fn https_proxy_wins_over_all_proxy_and_http_proxy() {
        let proxy = https_proxy(&[
            ("HTTP_PROXY", "http://plain.proxy:1"),
            ("ALL_PROXY", "http://any.proxy:2"),
            ("HTTPS_PROXY", "http://secure.proxy:3128"),
        ]);
        assert_eq!((proxy.host(), proxy.port()), ("secure.proxy", 3128));
    }

    #[test]
    fn all_proxy_wins_over_http_proxy() {
        let proxy = https_proxy(&[
            ("http_proxy", "http://plain.proxy:1"),
            ("all_proxy", "http://any.proxy:2"),
        ]);
        assert_eq!((proxy.host(), proxy.port()), ("any.proxy", 2));
    }

    #[test]
    fn http_proxy_alone_still_covers_https_urls() {
        let proxy = https_proxy(&[("http_proxy", "plain.proxy:8080")]);
        assert_eq!((proxy.host(), proxy.port()), ("plain.proxy", 8080));
    }

    #[test]
    fn plain_http_and_other_schemes_go_direct() {
        let proxies = Proxies::from_env(&env(&[("HTTPS_PROXY", "http://proxy:3128")]));
        assert_eq!(proxies.for_url("http://mirror.example/a"), Ok(None));
        assert_eq!(proxies.for_url("file:///tmp/a"), Ok(None));
    }

    #[test]
    fn no_variables_means_no_proxy() {
        let proxies = Proxies::from_env(&env(&[]));
        assert_eq!(proxies.for_url("https://example.com"), Ok(None));
    }

    #[test]
    fn no_proxy_covers_the_host_and_its_subdomains() {
        let proxy = https_proxy(&[
            ("HTTPS_PROXY", "http://proxy:3128"),
            (
                "NO_PROXY",
                "localhost, .internal.example ,*.corp,mirror.example",
            ),
        ]);
        for host in [
            "https://localhost/",
            "https://internal.example/",
            "https://a.internal.example/",
            "https://b.corp/",
            "https://mirror.example/",
            "https://cdn.mirror.example/",
        ] {
            assert!(proxy.is_no_proxy(&uri(host)), "{host} bypasses");
        }
        for host in ["https://example.com/", "https://notmirror.example/"] {
            assert!(
                !proxy.is_no_proxy(&uri(host)),
                "{host} goes through the proxy"
            );
        }
    }

    #[test]
    fn a_star_bypasses_everything() {
        let proxy = https_proxy(&[("HTTPS_PROXY", "http://proxy:3128"), ("no_proxy", "*")]);
        assert!(proxy.is_no_proxy(&uri("https://anything.example/")));
    }

    #[test]
    fn credentials_survive_the_rebuild() {
        let proxy = https_proxy(&[("HTTPS_PROXY", "http://me:p%40ss@proxy:3128")]);
        assert_eq!(proxy.username(), Some("me"));
        assert_eq!(proxy.password(), Some("p@ss"));
    }

    #[test]
    fn a_bad_proxy_url_names_the_variable_but_not_the_value() {
        let proxies = Proxies::from_env(&env(&[("HTTPS_PROXY", "ftp://me:secret@proxy")]));
        assert_eq!(
            proxies.for_url("https://example.com"),
            Err(Error::Config("HTTPS_PROXY is not a proxy URL".into()))
        );
    }

    #[test]
    fn a_bad_proxy_url_does_not_fail_urls_that_go_direct() {
        let proxies = Proxies::from_env(&env(&[("HTTPS_PROXY", "ftp://me:secret@proxy")]));
        assert_eq!(proxies.for_url("http://mirror.example/a"), Ok(None));
    }
}
