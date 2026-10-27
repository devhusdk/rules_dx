//! Minimal OCI Distribution client that pushes one staged OCI image layout.

use crate::http::{basic_auth, query_escape};
use std::path::{Path, PathBuf};

/// Fixed message for a registry or token realm that refuses the credentials.
pub(crate) const OCI_BAD_CREDENTIALS: &str =
    "registry refused the credentials; check OCI_REGISTRY_USER and OCI_REGISTRY_TOKEN";

const MANIFEST_TYPES: [&str; 2] = [
    "application/vnd.oci.image.manifest.v1+json",
    "application/vnd.docker.distribution.manifest.v2+json",
];

/// The scheme and authority of one registry API, such as `https://ghcr.io`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoint {
    origin: String,
}

impl Endpoint {
    fn https(host: &str) -> Self {
        Self {
            origin: format!("https://{}", host.to_ascii_lowercase()),
        }
    }

    /// Uses a loopback `http://` origin; tests only.
    #[cfg(test)]
    pub(crate) fn for_test(origin: &str) -> Self {
        Self {
            origin: origin.to_owned(),
        }
    }

    fn scheme(&self) -> &str {
        self.origin.split("://").next().unwrap_or("https")
    }
}

/// One repository on one registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RegistryRef {
    pub(crate) endpoint: Endpoint,
    pub(crate) name: String,
}

impl RegistryRef {
    /// Splits `host[:port][/path]` plus a repository into the API origin and repository name.
    pub(crate) fn parse(registry: &str, repository: &str) -> Result<Self, String> {
        let (host, prefix) = match registry.split_once('/') {
            Some((host, prefix)) => (host, Some(prefix)),
            None => (registry, None),
        };
        let host_ok = !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b':' | b'_'));
        if !host_ok {
            return Err(format!(
                "oci registry: want a registry host, got '{registry}'"
            ));
        }
        let host = host.to_ascii_lowercase();
        let docker_hub = host == "docker.io" || host == "index.docker.io";
        let api_host = if docker_hub {
            "registry-1.docker.io".to_owned()
        } else {
            host
        };
        let mut name = match prefix {
            Some(prefix) => format!("{prefix}/{repository}"),
            None => repository.to_owned(),
        };
        if docker_hub && !name.contains('/') {
            name = format!("library/{name}");
        }
        let name_ok = name.split('/').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
                })
        });
        if !name_ok {
            return Err(format!(
                "oci registry: want a lowercase repository path, got '{name}'"
            ));
        }
        Ok(Self {
            endpoint: Endpoint::https(&api_host),
            name,
        })
    }
}

/// Registry credentials: a user and a password or token.
#[derive(Clone, Copy)]
pub(crate) struct Credentials<'a> {
    pub(crate) user: &'a str,
    pub(crate) secret: &'a str,
}

/// One parsed `WWW-Authenticate` challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Challenge {
    Bearer {
        realm: String,
        service: Option<String>,
    },
    Basic,
}

/// Parses one `WWW-Authenticate` value with Bearer or Basic scheme.
pub(crate) fn parse_challenge(header: &str) -> Result<Challenge, String> {
    let header = header.trim();
    let (scheme, rest) = header.split_once(' ').unwrap_or((header, ""));
    if scheme.eq_ignore_ascii_case("basic") {
        return Ok(Challenge::Basic);
    }
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err("registry auth: unsupported challenge scheme".to_owned());
    }
    let params = parse_auth_params(rest)?;
    let get = |key: &str| {
        params
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| value.clone())
    };
    let realm = get("realm")
        .filter(|realm| !realm.is_empty())
        .ok_or_else(|| "registry auth: bearer challenge has no realm".to_owned())?;
    Ok(Challenge::Bearer {
        realm,
        service: get("service"),
    })
}

/// Parses `key=value` and `key="quoted, value"` auth params separated by commas.
fn parse_auth_params(text: &str) -> Result<Vec<(String, String)>, String> {
    let malformed = || "registry auth: malformed challenge".to_owned();
    let mut params = Vec::new();
    let mut chars = text.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| *c == ',' || c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            return Ok(params);
        }
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' {
                break;
            }
            if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                return Err(malformed());
            }
            key.push(c);
            chars.next();
        }
        if key.is_empty() || chars.next() != Some('=') {
            return Err(malformed());
        }
        let mut value = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') => break,
                    Some('\\') => value.push(chars.next().ok_or_else(malformed)?),
                    Some(c) => value.push(c),
                    None => return Err(malformed()),
                }
            }
        } else {
            while let Some(&c) = chars.peek() {
                if c == ',' || c.is_whitespace() {
                    break;
                }
                value.push(c);
                chars.next();
            }
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_some_and(|c| *c != ',') {
            return Err(malformed());
        }
        params.push((key, value));
    }
}

/// Returns the lowercase `scheme://authority` of an http(s) URL, without default ports.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "https" && scheme != "http" {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..end].to_ascii_lowercase();
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    let default_port = if scheme == "https" { ":443" } else { ":80" };
    let authority = authority
        .strip_suffix(default_port)
        .unwrap_or(&authority)
        .to_owned();
    Some(format!("{scheme}://{authority}"))
}

/// Checks that a token realm is https or the registry's own origin.
pub(crate) fn check_realm(endpoint: &Endpoint, realm: &str) -> Result<(), String> {
    match origin_of(realm) {
        Some(origin) if origin.starts_with("https://") || origin == endpoint.origin => Ok(()),
        _ => Err(
            "registry auth: refusing a token realm that is neither https nor the registry origin"
                .to_owned(),
        ),
    }
}

fn transport(what: &str, error: ureq::Error) -> String {
    format!("registry: {what} failed: {error}")
}

fn status_error(what: &str, status: u16) -> String {
    if status == 401 || status == 403 {
        OCI_BAD_CREDENTIALS.to_owned()
    } else {
        format!("registry: {what} returned HTTP {status}")
    }
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn valid_digest(digest: &str) -> bool {
    digest.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn blob_path(layout: &Path, digest: &str) -> Result<PathBuf, String> {
    if !valid_digest(digest) {
        return Err(format!("oci layout: want a sha256 digest, got '{digest}'"));
    }
    Ok(layout
        .join("blobs")
        .join("sha256")
        .join(digest.trim_start_matches("sha256:")))
}

/// One authenticated session against one repository.
struct Session<'a> {
    agent: &'a ureq::Agent,
    target: &'a RegistryRef,
    authorization: Option<String>,
}

impl<'a> Session<'a> {
    fn open(
        agent: &'a ureq::Agent,
        target: &'a RegistryRef,
        creds: Credentials<'_>,
    ) -> Result<Self, String> {
        let origin = &target.endpoint.origin;
        let ping = agent
            .get(format!("{origin}/v2/"))
            .call()
            .map_err(|error| transport("ping", error))?;
        let status = ping.status().as_u16();
        if status == 200 {
            return Ok(Self {
                agent,
                target,
                authorization: None,
            });
        }
        if status != 401 {
            return Err(format!("registry: ping returned HTTP {status}"));
        }
        let challenges: Vec<String> = ping
            .headers()
            .get_all("www-authenticate")
            .iter()
            .filter_map(|value| value.to_str().ok().map(str::to_owned))
            .collect();
        let mut parsed = Vec::new();
        let mut last_error = "registry auth: 401 without a WWW-Authenticate challenge".to_owned();
        for value in &challenges {
            match parse_challenge(value) {
                Ok(challenge) => parsed.push(challenge),
                Err(error) => last_error = error,
            }
        }
        let challenge = parsed
            .iter()
            .find(|challenge| matches!(challenge, Challenge::Bearer { .. }))
            .or_else(|| parsed.first())
            .ok_or(last_error)?;
        let authorization = match challenge {
            Challenge::Basic => basic_auth(creds.user, creds.secret),
            Challenge::Bearer { realm, service } => {
                check_realm(&target.endpoint, realm)?;
                let mut url = realm.clone();
                url.push(if url.contains('?') { '&' } else { '?' });
                if let Some(service) = service {
                    url.push_str(&format!("service={}&", query_escape(service)));
                }
                let scope = format!("repository:{}:pull,push", target.name);
                url.push_str(&format!("scope={}", query_escape(&scope)));
                let mut reply = agent
                    .get(url)
                    .header("Authorization", basic_auth(creds.user, creds.secret))
                    .call()
                    .map_err(|error| transport("token request", error))?;
                let status = reply.status().as_u16();
                if status != 200 {
                    return Err(status_error("token request", status));
                }
                let body = reply
                    .body_mut()
                    .with_config()
                    .limit(1 << 20)
                    .read_to_vec()
                    .map_err(|_| "registry auth: unreadable token response".to_owned())?;
                let json: serde_json::Value = serde_json::from_slice(&body)
                    .map_err(|_| "registry auth: malformed token response".to_owned())?;
                let token = ["token", "access_token"]
                    .iter()
                    .find_map(|key| json[*key].as_str().filter(|token| !token.is_empty()))
                    .ok_or_else(|| "registry auth: token response has no token".to_owned())?;
                format!("Bearer {token}")
            }
        };
        Ok(Self {
            agent,
            target,
            authorization: Some(authorization),
        })
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/v2/{}/{path}",
            self.target.endpoint.origin, self.target.name
        )
    }

    fn authorize<B>(&self, request: ureq::RequestBuilder<B>) -> ureq::RequestBuilder<B> {
        match &self.authorization {
            Some(value) => request.header("Authorization", value),
            None => request,
        }
    }

    /// Resolves an upload `Location` and appends the digest query.
    fn upload_url(&self, location: &str, digest: &str) -> Result<String, String> {
        let origin = &self.target.endpoint.origin;
        let mut url = if location.starts_with("https://") || location.starts_with("http://") {
            location.to_owned()
        } else if let Some(rest) = location.strip_prefix("//") {
            format!("{}://{rest}", self.target.endpoint.scheme())
        } else if location.starts_with('/') {
            format!("{origin}{location}")
        } else {
            return Err("registry: upload location is not an absolute path or URL".to_owned());
        };
        if let Some(hash) = url.find('#') {
            url.truncate(hash);
        }
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&format!("digest={}", query_escape(digest)));
        Ok(url)
    }

    fn push_blob(&self, layout: &Path, digest: &str) -> Result<(), String> {
        let path = blob_path(layout, digest)?;
        let head = self
            .authorize(self.agent.head(self.url(&format!("blobs/{digest}"))))
            .call()
            .map_err(|error| transport("blob check", error))?;
        if head.status().as_u16() == 200 {
            return Ok(());
        }
        let start = self
            .authorize(self.agent.post(self.url("blobs/uploads/")))
            .send_empty()
            .map_err(|error| transport("blob upload start", error))?;
        let status = start.status().as_u16();
        if status != 202 {
            return Err(status_error("blob upload start", status));
        }
        let location = header(&start, "location")
            .ok_or_else(|| "registry: blob upload start returned no Location".to_owned())?;
        let url = self.upload_url(&location, digest)?;
        let origin = origin_of(&url)
            .ok_or_else(|| "registry: upload location is not an http(s) URL".to_owned())?;
        let same_origin = origin == self.target.endpoint.origin;
        if !same_origin && !origin.starts_with(&format!("{}://", self.target.endpoint.scheme())) {
            return Err("registry: refusing an upload location on a weaker scheme".to_owned());
        }
        let file = std::fs::File::open(&path)
            .map_err(|error| format!("oci layout: cannot open blob {digest}: {error}"))?;
        let length = file
            .metadata()
            .map_err(|error| format!("oci layout: cannot stat blob {digest}: {error}"))?
            .len();
        let mut put = self
            .agent
            .put(url)
            .header("Content-Type", "application/octet-stream")
            .header("Content-Length", length.to_string());
        if same_origin {
            put = self.authorize(put);
        }
        let done = put
            .send(file)
            .map_err(|error| transport("blob upload", error))?;
        let status = done.status().as_u16();
        if status != 201 {
            return Err(status_error(&format!("blob upload of {digest}"), status));
        }
        Ok(())
    }

    fn push_manifest(
        &self,
        tag: &str,
        digest: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let done = self
            .authorize(self.agent.put(self.url(&format!("manifests/{tag}"))))
            .header("Content-Type", media_type)
            .send(bytes)
            .map_err(|error| transport("manifest upload", error))?;
        let status = done.status().as_u16();
        if status != 201 {
            return Err(status_error("manifest upload", status));
        }
        if let Some(pushed) = header(&done, "docker-content-digest") {
            if pushed.trim() != digest {
                return Err(format!(
                    "registry: pushed manifest digest {} differs from the staged {digest}",
                    pushed.trim()
                ));
            }
        }
        Ok(())
    }
}

fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 128
        && !tag.starts_with(['.', '-'])
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// One image manifest staged in a layout, with the blobs it references.
struct StagedManifest {
    digest: String,
    media_type: String,
    bytes: Vec<u8>,
    blobs: Vec<String>,
}

fn read_json(path: &Path, what: &str) -> Result<(Vec<u8>, serde_json::Value), String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("oci layout: cannot read {what}: {error}"))?;
    let json = serde_json::from_slice(&bytes)
        .map_err(|error| format!("oci layout: malformed {what}: {error}"))?;
    Ok((bytes, json))
}

fn staged_manifest(
    layout: &Path,
    descriptor: &serde_json::Value,
) -> Result<StagedManifest, String> {
    let digest = descriptor["digest"].as_str().unwrap_or("").to_owned();
    let (bytes, manifest) = read_json(&blob_path(layout, &digest)?, "manifest")?;
    if format!("sha256:{}", dx_digest::sha256_hex(&bytes)) != digest {
        return Err(format!("oci layout: manifest blob does not match {digest}"));
    }
    let media_type = manifest["mediaType"]
        .as_str()
        .or_else(|| descriptor["mediaType"].as_str())
        .unwrap_or("")
        .to_owned();
    if !MANIFEST_TYPES.contains(&media_type.as_str()) {
        return Err(format!(
            "oci layout: want an image manifest, got media type '{media_type}'"
        ));
    }
    let mut blobs = vec![manifest["config"]["digest"]
        .as_str()
        .unwrap_or("")
        .to_owned()];
    for layer in manifest["layers"].as_array().into_iter().flatten() {
        blobs.push(layer["digest"].as_str().unwrap_or("").to_owned());
    }
    for blob in &blobs {
        blob_path(layout, blob)?;
    }
    Ok(StagedManifest {
        digest,
        media_type,
        bytes,
        blobs,
    })
}

/// Pushes the single image of an OCI layout by tag and returns its manifest digest.
pub(crate) fn push_layout(
    agent: &ureq::Agent,
    target: &RegistryRef,
    tag: &str,
    layout: &Path,
    creds: Credentials<'_>,
) -> Result<String, String> {
    if !valid_tag(tag) {
        return Err(format!("oci registry: want a valid tag, got '{tag}'"));
    }
    let (_, index) = read_json(&layout.join("index.json"), "index.json")?;
    let descriptors = index["manifests"].as_array().cloned().unwrap_or_default();
    let manifests = descriptors
        .iter()
        .map(|descriptor| staged_manifest(layout, descriptor))
        .collect::<Result<Vec<_>, _>>()?;
    let [manifest] = manifests.as_slice() else {
        return Err(format!(
            "oci layout: want exactly one manifest in index.json, got {}",
            manifests.len()
        ));
    };
    let session = Session::open(agent, target, creds)?;
    for blob in &manifest.blobs {
        session.push_blob(layout, blob)?;
    }
    session.push_manifest(tag, &manifest.digest, &manifest.media_type, &manifest.bytes)?;
    Ok(manifest.digest.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const USER: &str = "me";
    const SECRET: &str = "s3cret-token-value";

    fn creds() -> Credentials<'static> {
        Credentials {
            user: USER,
            secret: SECRET,
        }
    }

    fn agent() -> ureq::Agent {
        crate::http::upload_agent_with_proxy(None)
    }

    /// Writes a layout with one config and two layers; returns the manifest digest.
    fn write_layout(dir: &Path) -> (PathBuf, String, Vec<String>) {
        let layout = dir.join("layout");
        let blobs = layout.join("blobs").join("sha256");
        std::fs::create_dir_all(&blobs).expect("blobs");
        let mut digests = Vec::new();
        for body in [
            b"{\"os\":\"linux\"}".to_vec(),
            b"layer one".to_vec(),
            vec![7u8; 70_000],
        ] {
            let hex = dx_digest::sha256_hex(&body);
            std::fs::write(blobs.join(&hex), &body).expect("blob");
            digests.push(format!("sha256:{hex}"));
        }
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "config": {"digest": digests[0], "mediaType": "application/vnd.oci.image.config.v1+json", "size": 14},
            "layers": [
                {"digest": digests[1], "mediaType": "application/vnd.oci.image.layer.v1.tar", "size": 9},
                {"digest": digests[2], "mediaType": "application/vnd.oci.image.layer.v1.tar", "size": 70000},
            ],
        }))
        .expect("manifest");
        let hex = dx_digest::sha256_hex(&manifest);
        std::fs::write(blobs.join(&hex), &manifest).expect("manifest blob");
        let digest = format!("sha256:{hex}");
        let index = serde_json::json!({
            "schemaVersion": 2,
            "manifests": [{"digest": digest, "mediaType": "application/vnd.oci.image.manifest.v1+json", "size": manifest.len()}],
        });
        std::fs::write(layout.join("index.json"), index.to_string()).expect("index");
        (layout, digest, digests)
    }

    fn push(fake: &FakeRegistry, layout: &Path) -> Result<String, String> {
        let target = RegistryRef {
            endpoint: Endpoint::for_test(&fake.origin),
            name: "demo/app".to_owned(),
        };
        push_layout(&agent(), &target, "1.2.3", layout, creds())
    }

    fn auth_of(request: &FakeRequest) -> &str {
        request.authorization.as_deref().unwrap_or("")
    }

    #[test]
    fn registry_ref_splits_host_path_and_port() {
        let ghcr = RegistryRef::parse("ghcr.io/example", "app").expect("ghcr");
        assert_eq!(ghcr.endpoint, Endpoint::https("ghcr.io"));
        assert_eq!(ghcr.name, "example/app");
        let local = RegistryRef::parse("localhost:5000", "demo/app").expect("local");
        assert_eq!(local.endpoint.origin, "https://localhost:5000");
        assert_eq!(local.name, "demo/app");
        let hub = RegistryRef::parse("docker.io", "app").expect("hub");
        assert_eq!(hub.endpoint.origin, "https://registry-1.docker.io");
        assert_eq!(hub.name, "library/app");
        let hub = RegistryRef::parse("index.docker.io/org", "app").expect("hub org");
        assert_eq!(hub.endpoint.origin, "https://registry-1.docker.io");
        assert_eq!(hub.name, "org/app");
        for (registry, repository) in [
            ("", "app"),
            ("https://ghcr.io", "app"),
            ("ghcr.io", "App"),
            ("ghcr.io//x", "app"),
            ("gh cr.io", "app"),
            ("ghcr.io", "a?b"),
        ] {
            assert!(
                RegistryRef::parse(registry, repository).is_err(),
                "{registry} {repository}"
            );
        }
    }

    #[test]
    fn registry_challenge_parses_bearer_and_basic() {
        assert_eq!(
            parse_challenge(
                "Bearer realm=\"https://ghcr.io/token\",service=\"ghcr.io\",scope=\"repository:a/b:pull,push\""
            )
            .expect("bearer"),
            Challenge::Bearer {
                realm: "https://ghcr.io/token".to_owned(),
                service: Some("ghcr.io".to_owned()),
            }
        );
        assert_eq!(
            parse_challenge("bearer realm=https://auth.example/t, service=reg").expect("token"),
            Challenge::Bearer {
                realm: "https://auth.example/t".to_owned(),
                service: Some("reg".to_owned()),
            }
        );
        assert_eq!(
            parse_challenge("Basic realm=\"ecr\"").expect("basic"),
            Challenge::Basic
        );
        for garbage in [
            "",
            "Negotiate abc",
            "Bearer service=\"x\"",
            "Bearer realm=\"unterminated",
            "Bearer realm",
            "Bearer realm=\"a\" junk",
        ] {
            assert!(parse_challenge(garbage).is_err(), "{garbage}");
        }
    }

    #[test]
    fn registry_refuses_http_realm_for_https_registry() {
        let endpoint = Endpoint::https("ghcr.io");
        assert!(check_realm(&endpoint, "https://ghcr.io/token").is_ok());
        assert!(check_realm(&endpoint, "https://auth.docker.io/token").is_ok());
        assert!(check_realm(&endpoint, "http://ghcr.io/token").is_err());
        assert!(check_realm(&endpoint, "http://evil.example/token").is_err());
        assert!(check_realm(&endpoint, "ftp://ghcr.io/token").is_err());
        assert!(check_realm(&endpoint, "https://user@evil.example/token").is_err());
        let local = Endpoint::for_test("http://127.0.0.1:5000");
        assert!(check_realm(&local, "http://127.0.0.1:5000/token").is_ok());
        assert!(check_realm(&local, "http://127.0.0.1:5001/token").is_err());
    }

    #[test]
    fn registry_push_uploads_missing_blobs_then_manifest() {
        let scratch = scratch_dir();
        let (layout, digest, blobs) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig {
            existing: vec![blobs[1].clone()],
            ..FakeConfig::default()
        });
        assert_eq!(push(&fake, &layout).expect("push"), digest);
        let requests = fake.requests();
        let lines: Vec<String> = requests
            .iter()
            .map(|r| format!("{} {}", r.method, r.path.split('?').next().unwrap_or("")))
            .collect();
        assert_eq!(
            lines,
            vec![
                "GET /v2/".to_owned(),
                format!("HEAD /v2/demo/app/blobs/{}", blobs[0]),
                "POST /v2/demo/app/blobs/uploads/".to_owned(),
                "PUT /v2/demo/app/blobs/uploads/u1".to_owned(),
                format!("HEAD /v2/demo/app/blobs/{}", blobs[1]),
                format!("HEAD /v2/demo/app/blobs/{}", blobs[2]),
                "POST /v2/demo/app/blobs/uploads/".to_owned(),
                "PUT /v2/demo/app/blobs/uploads/u2".to_owned(),
                "PUT /v2/demo/app/manifests/1.2.3".to_owned(),
            ]
        );
        let manifest = requests.last().expect("manifest");
        assert_eq!(manifest.body_digest, digest);
        assert_eq!(
            manifest.content_type.as_deref(),
            Some("application/vnd.oci.image.manifest.v1+json")
        );
        assert_eq!(requests[3].body_digest, blobs[0]);
        assert_eq!(requests[7].body_digest, blobs[2]);
        assert!(requests.iter().all(|r| r.authorization.is_none()));
    }

    #[test]
    fn registry_token_exchange_scopes_pull_push_and_sends_basic_only_to_realm() {
        let scratch = scratch_dir();
        let (layout, digest, _) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig {
            challenge: FakeChallenge::Bearer,
            ..FakeConfig::default()
        });
        assert_eq!(push(&fake, &layout).expect("push"), digest);
        let requests = fake.requests();
        let basic = crate::http::basic_auth(USER, SECRET);
        let token: Vec<&FakeRequest> = requests
            .iter()
            .filter(|r| r.path.starts_with("/token"))
            .collect();
        assert_eq!(token.len(), 1);
        assert_eq!(auth_of(token[0]), basic);
        assert!(
            token[0]
                .path
                .contains("scope=repository%3Ademo%2Fapp%3Apull%2Cpush"),
            "{}",
            token[0].path
        );
        assert!(token[0].path.contains("service=fake.registry"));
        for request in &requests {
            if request.path.starts_with("/token") {
                continue;
            }
            assert_ne!(auth_of(request), basic, "{}", request.path);
            if request.path != "/v2/" {
                assert_eq!(
                    auth_of(request),
                    format!("Bearer {FAKE_TOKEN}"),
                    "{}",
                    request.path
                );
            }
        }
    }

    #[test]
    fn registry_basic_challenge_uses_basic_auth() {
        let scratch = scratch_dir();
        let (layout, digest, _) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig {
            challenge: FakeChallenge::Basic,
            ..FakeConfig::default()
        });
        assert_eq!(push(&fake, &layout).expect("push"), digest);
        let basic = crate::http::basic_auth(USER, SECRET);
        let requests = fake.requests();
        assert!(requests.iter().all(|r| !r.path.starts_with("/token")));
        assert!(requests
            .iter()
            .skip(1)
            .all(|r| auth_of(r) == basic.as_str()));
    }

    #[test]
    fn registry_bad_credentials_refuses_without_echoing_secret() {
        let scratch = scratch_dir();
        let (layout, _, _) = write_layout(scratch.path());
        for challenge in [FakeChallenge::Bearer, FakeChallenge::Basic] {
            let fake = FakeRegistry::start(FakeConfig {
                challenge,
                secret: "other".to_owned(),
                ..FakeConfig::default()
            });
            let error = push(&fake, &layout).expect_err("bad credentials");
            assert_eq!(error, OCI_BAD_CREDENTIALS);
            assert!(!error.contains(SECRET));
            assert!(fake
                .requests()
                .iter()
                .all(|r| !r.path.contains("/manifests/")));
        }
    }

    #[test]
    fn registry_cross_origin_location_gets_no_authorization() {
        let scratch = scratch_dir();
        let (layout, digest, _) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig {
            challenge: FakeChallenge::Bearer,
            location: FakeLocation::OtherHost,
            ..FakeConfig::default()
        });
        assert_eq!(push(&fake, &layout).expect("push"), digest);
        let other = fake.other_origin.trim_start_matches("http://").to_owned();
        let uploads: Vec<FakeRequest> = fake
            .requests()
            .into_iter()
            .filter(|r| r.host == other)
            .collect();
        assert_eq!(uploads.len(), 3);
        for upload in uploads {
            assert_eq!(upload.method, "PUT");
            assert!(upload.path.contains("sig=signed&digest=sha256%3A"));
            assert!(upload.authorization.is_none(), "{}", upload.path);
        }
    }

    #[test]
    fn registry_relative_location_and_existing_query() {
        let scratch = scratch_dir();
        let (layout, digest, blobs) = write_layout(scratch.path());
        for (location, want) in [
            (
                FakeLocation::RelativeQuery,
                format!(
                    "/v2/demo/app/blobs/uploads/u1?_state=abc&digest=sha256%3A{}",
                    &blobs[0][7..]
                ),
            ),
            (
                FakeLocation::Absolute,
                format!(
                    "/v2/demo/app/blobs/uploads/u1?digest=sha256%3A{}",
                    &blobs[0][7..]
                ),
            ),
        ] {
            let fake = FakeRegistry::start(FakeConfig {
                challenge: FakeChallenge::Basic,
                location,
                ..FakeConfig::default()
            });
            assert_eq!(push(&fake, &layout).expect("push"), digest);
            let put = fake
                .requests()
                .into_iter()
                .find(|r| r.method == "PUT")
                .expect("put");
            assert_eq!(put.path, want);
            assert!(put.authorization.is_some());
        }
        let session_target = RegistryRef {
            endpoint: Endpoint::for_test("http://127.0.0.1:1"),
            name: "a".to_owned(),
        };
        let session = Session {
            agent: &agent(),
            target: &session_target,
            authorization: None,
        };
        assert!(session.upload_url("uploads/u1", "sha256:x").is_err());
        assert_eq!(
            session
                .upload_url("//h.example/u#frag", "d")
                .expect("scheme relative"),
            "http://h.example/u?digest=d"
        );
    }

    #[test]
    fn registry_blob_put_sets_content_length_not_chunked() {
        let scratch = scratch_dir();
        let (layout, _, blobs) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig::default());
        push(&fake, &layout).expect("push");
        let puts: Vec<FakeRequest> = fake
            .requests()
            .into_iter()
            .filter(|r| r.method == "PUT" && r.path.contains("/uploads/"))
            .collect();
        assert_eq!(puts.len(), 3);
        let big = puts
            .iter()
            .find(|r| r.body_digest == blobs[2])
            .expect("big layer");
        assert_eq!(big.content_length.as_deref(), Some("70000"));
        assert!(puts.iter().all(|r| r.transfer_encoding.is_none()));
        assert!(puts
            .iter()
            .all(|r| r.content_type.as_deref() == Some("application/octet-stream")));
    }

    #[test]
    fn registry_digest_header_mismatch_fails() {
        let scratch = scratch_dir();
        let (layout, digest, _) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig {
            digest_header: FakeDigestHeader::Wrong,
            ..FakeConfig::default()
        });
        let error = push(&fake, &layout).expect_err("mismatch");
        assert!(error.contains("differs from the staged"), "{error}");
        assert!(error.contains(&digest), "{error}");
        let fake = FakeRegistry::start(FakeConfig {
            digest_header: FakeDigestHeader::Missing,
            ..FakeConfig::default()
        });
        assert_eq!(push(&fake, &layout).expect("missing header"), digest);
    }

    #[test]
    fn registry_blob_failure_skips_manifest() {
        let scratch = scratch_dir();
        let (layout, _, _) = write_layout(scratch.path());
        for (method, status) in [("POST", 500), ("PUT", 400)] {
            let fake = FakeRegistry::start(FakeConfig {
                fail: vec![(method.to_owned(), "/blobs/uploads/".to_owned(), status)],
                ..FakeConfig::default()
            });
            let error = push(&fake, &layout).expect_err("blob failure");
            assert!(error.contains(&format!("HTTP {status}")), "{error}");
            assert!(fake
                .requests()
                .iter()
                .all(|r| !r.path.contains("/manifests/")));
        }
    }

    #[test]
    fn registry_refuses_layouts_it_cannot_push() {
        let scratch = scratch_dir();
        let (layout, digest, _) = write_layout(scratch.path());
        let fake = FakeRegistry::start(FakeConfig::default());
        let target = RegistryRef {
            endpoint: Endpoint::for_test(&fake.origin),
            name: "demo/app".to_owned(),
        };
        assert!(push_layout(&agent(), &target, "bad tag", &layout, creds()).is_err());
        let index = layout.join("index.json");
        let one = std::fs::read_to_string(&index).expect("index");
        let value: serde_json::Value = serde_json::from_str(&one).expect("json");
        let descriptor = value["manifests"][0].clone();
        let two =
            serde_json::json!({"schemaVersion": 2, "manifests": [descriptor.clone(), descriptor]});
        std::fs::write(&index, two.to_string()).expect("two");
        let error = push_layout(&agent(), &target, "1.2.3", &layout, creds()).expect_err("two");
        assert!(error.contains("exactly one manifest"), "{error}");
        let hex = digest.trim_start_matches("sha256:");
        std::fs::write(&index, one).expect("restore");
        std::fs::write(layout.join("blobs/sha256").join(hex), b"{}").expect("tamper");
        let error = push_layout(&agent(), &target, "1.2.3", &layout, creds()).expect_err("tamper");
        assert!(error.contains("does not match"), "{error}");
        assert!(fake.requests().is_empty());
    }
}
