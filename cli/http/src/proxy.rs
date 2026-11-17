//! Proxy choice from `https_proxy`, `http_proxy`, `all_proxy` and `no_proxy`.

use ureq::Proxy;

use crate::Error;

/// The proxy for each URL scheme, with the `no_proxy` hosts built in.
#[derive(Clone, Debug, Default)]
pub(crate) struct Proxies {
    https: Option<Proxy>,
    http: Option<Proxy>,
}

impl Proxies {
    /// Reads the proxy variables through `env`; a variable that is set but is not a proxy URL is an error.
    pub(crate) fn from_env(env: &dyn Fn(&str) -> Option<String>) -> Result<Self, Error> {
        let bypass = first(env, &["no_proxy", "NO_PROXY"])
            .map(|(_, value)| bypass_entries(&value))
            .unwrap_or_default();
        let https = first(
            env,
            &["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"],
        );
        let http = first(env, &["http_proxy", "HTTP_PROXY", "all_proxy", "ALL_PROXY"]);
        Ok(Self {
            https: https
                .map(|(name, value)| proxy(name, &value, &bypass))
                .transpose()?,
            http: http
                .map(|(name, value)| proxy(name, &value, &bypass))
                .transpose()?,
        })
    }

    /// The proxy for `url`, picked by its scheme. ureq applies the `no_proxy` hosts on every redirect hop.
    pub(crate) fn for_url(&self, url: &str) -> Option<Proxy> {
        let scheme = url.split_once("://").map(|(scheme, _)| scheme)?;
        if scheme.eq_ignore_ascii_case("https") {
            self.https.clone()
        } else if scheme.eq_ignore_ascii_case("http") {
            self.http.clone()
        } else {
            None
        }
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

    #[test]
    fn each_scheme_reads_its_own_variable() {
        let proxies = Proxies::from_env(&env(&[
            ("HTTPS_PROXY", "http://secure.proxy:3128"),
            ("http_proxy", "plain.proxy:8080"),
        ]))
        .expect("proxies");
        let https = proxies
            .for_url("https://example.com/a")
            .expect("https proxy");
        assert_eq!((https.host(), https.port()), ("secure.proxy", 3128));
        let http = proxies.for_url("http://example.com/a").expect("http proxy");
        assert_eq!((http.host(), http.port()), ("plain.proxy", 8080));
        assert!(proxies.for_url("file:///tmp/a").is_none());
    }

    #[test]
    fn all_proxy_covers_both_schemes() {
        let proxies =
            Proxies::from_env(&env(&[("ALL_PROXY", "http://any.proxy:1")])).expect("proxies");
        assert!(proxies.for_url("https://example.com").is_some());
        assert!(proxies.for_url("http://example.com").is_some());
    }

    #[test]
    fn no_variables_means_no_proxy() {
        let proxies = Proxies::from_env(&env(&[])).expect("proxies");
        assert!(proxies.for_url("https://example.com").is_none());
        assert!(proxies.for_url("http://example.com").is_none());
    }

    #[test]
    fn no_proxy_covers_the_host_and_its_subdomains() {
        let proxies = Proxies::from_env(&env(&[
            ("HTTPS_PROXY", "http://proxy:3128"),
            (
                "NO_PROXY",
                "localhost, .internal.example ,*.corp,mirror.example",
            ),
        ]))
        .expect("proxies");
        let proxy = proxies.for_url("https://x").expect("proxy");
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
        let proxies = Proxies::from_env(&env(&[
            ("HTTPS_PROXY", "http://proxy:3128"),
            ("no_proxy", "*"),
        ]))
        .expect("proxies");
        let proxy = proxies.for_url("https://x").expect("proxy");
        assert!(proxy.is_no_proxy(&uri("https://anything.example/")));
    }

    #[test]
    fn credentials_survive_the_rebuild() {
        let proxies = Proxies::from_env(&env(&[("HTTPS_PROXY", "http://me:p%40ss@proxy:3128")]))
            .expect("proxies");
        let proxy = proxies.for_url("https://x").expect("proxy");
        assert_eq!(proxy.username(), Some("me"));
        assert_eq!(proxy.password(), Some("p@ss"));
    }

    #[test]
    fn a_bad_proxy_url_names_the_variable_but_not_the_value() {
        let error = Proxies::from_env(&env(&[("HTTPS_PROXY", "ftp://me:secret@proxy")]))
            .expect_err("bad proxy");
        assert_eq!(
            error,
            Error::Config("HTTPS_PROXY is not a proxy URL".into())
        );
    }
}
