//! Shared test fixtures: the env lock, scratch dirs and child-process env tests.

use crate::{gzip_compress, tar_archive};
use std::path::Path;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn scratch_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("scratch")
}

pub(crate) fn with_env<T>(vars: &[(&str, Option<&str>)], body: impl FnOnce() -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut saved: Vec<(&str, Option<String>)> = Vec::new();
    for (key, value) in vars {
        saved.push((*key, std::env::var(key).ok()));
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    let out = body();
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
    out
}

const ENV_CHILD: &str = "DX_DEPLOY_RULES_ENV_CHILD";

pub(crate) fn env_test_in_child(test: &str) -> bool {
    if std::env::var_os(ENV_CHILD).is_some() {
        return false;
    }
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut before: Vec<_> = std::env::vars_os().collect();
    before.sort();
    let run = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([test, "--exact"])
        .env(ENV_CHILD, "1")
        .output()
        .expect("spawn env child");
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stdout.contains("running 1 test"),
        "child did not run exactly one test: {stdout}{stderr}"
    );
    assert!(run.status.success(), "{stdout}{stderr}");
    let mut after: Vec<_> = std::env::vars_os().collect();
    after.sort();
    assert_eq!(after, before, "parent environment changed");
    true
}

pub(crate) fn write_crate_file(root: &Path, rel: &str, body: &str) -> std::path::PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    std::fs::write(&path, body).expect("write");
    path
}

pub(crate) fn write_nupkg(path: &Path, entries: &[(&str, &str)]) {
    use std::io::Write as _;
    let file = std::fs::File::create(path).expect("create nupkg");
    let mut writer = zip::ZipWriter::new(file);
    for (name, body) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(body.as_bytes()).expect("write entry");
    }
    writer.finish().expect("finish nupkg");
}

pub(crate) fn nuspec(id: &str, version: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><package xmlns=\"http://schemas.microsoft.com/packaging/2013/05/nuspec.xsd\"><metadata><id>{id}</id><version>{version}</version><authors>a</authors></metadata></package>"
    )
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[cfg(unix)]
pub(crate) fn mode_bits(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

pub(crate) fn oci_test_layer(gzip: bool) -> Vec<u8> {
    let tar =
        tar_archive([("hello", b"hello world\n".as_slice(), 0o644)].into_iter()).expect("tar");
    if gzip {
        gzip_compress(&tar).expect("gzip")
    } else {
        tar
    }
}

/// Token the fake registry hands out from its realm.
pub(crate) const FAKE_TOKEN: &str = "fake-bearer-token";

/// Auth challenge the fake registry answers `/v2/` with.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FakeChallenge {
    #[default]
    None,
    Bearer,
    Basic,
}

/// Shape of the `Location` the fake registry returns for an upload.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FakeLocation {
    #[default]
    Relative,
    RelativeQuery,
    Absolute,
    OtherHost,
}

/// `Docker-Content-Digest` the fake registry returns for a manifest.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FakeDigestHeader {
    #[default]
    Correct,
    Wrong,
    Missing,
}

/// Behaviour of one fake registry.
pub(crate) struct FakeConfig {
    pub(crate) challenge: FakeChallenge,
    pub(crate) user: String,
    pub(crate) secret: String,
    pub(crate) existing: Vec<String>,
    pub(crate) location: FakeLocation,
    pub(crate) digest_header: FakeDigestHeader,
    /// `(method, path substring, status)` replies forced before anything else.
    pub(crate) fail: Vec<(String, String, u16)>,
}

impl Default for FakeConfig {
    fn default() -> Self {
        Self {
            challenge: FakeChallenge::None,
            user: "me".to_owned(),
            secret: "s3cret-token-value".to_owned(),
            existing: Vec::new(),
            location: FakeLocation::Relative,
            digest_header: FakeDigestHeader::Correct,
            fail: Vec::new(),
        }
    }
}

/// One request the fake registry saw.
#[derive(Clone, Debug)]
pub(crate) struct FakeRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) host: String,
    pub(crate) authorization: Option<String>,
    pub(crate) content_type: Option<String>,
    pub(crate) content_length: Option<String>,
    pub(crate) transfer_encoding: Option<String>,
    pub(crate) body_digest: String,
}

struct FakeState {
    config: FakeConfig,
    origin: String,
    other_origin: String,
    blobs: Vec<String>,
    uploads: usize,
    requests: Vec<FakeRequest>,
}

/// A loopback OCI registry with a second listener that acts as another host.
pub(crate) struct FakeRegistry {
    pub(crate) origin: String,
    pub(crate) other_origin: String,
    state: std::sync::Arc<std::sync::Mutex<FakeState>>,
}

impl FakeRegistry {
    pub(crate) fn start(config: FakeConfig) -> Self {
        let main = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let other = std::net::TcpListener::bind("127.0.0.1:0").expect("bind other");
        let origin = format!("http://{}", main.local_addr().expect("addr"));
        let other_origin = format!("http://{}", other.local_addr().expect("addr"));
        let state = std::sync::Arc::new(std::sync::Mutex::new(FakeState {
            blobs: config.existing.clone(),
            config,
            origin: origin.clone(),
            other_origin: other_origin.clone(),
            uploads: 0,
            requests: Vec::new(),
        }));
        for listener in [main, other] {
            let state = state.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { return };
                    fake_serve(stream, &state);
                }
            });
        }
        Self {
            origin,
            other_origin,
            state,
        }
    }

    pub(crate) fn requests(&self) -> Vec<FakeRequest> {
        self.state.lock().expect("state").requests.clone()
    }
}

fn fake_read_request(
    stream: &mut std::net::TcpStream,
) -> Option<(String, Vec<(String, String)>, Vec<u8>)> {
    use std::io::Read as _;
    let mut seen = Vec::new();
    let mut buf = [0u8; 8192];
    let end = loop {
        if let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            return None;
        }
        seen.extend_from_slice(&buf[..n]);
    };
    let head = String::from_utf8_lossy(&seen[..end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let mut rest = seen[end + 4..].to_vec();
    let get = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let mut fill = |rest: &mut Vec<u8>, want: usize| -> Option<()> {
        while rest.len() < want {
            let n = stream.read(&mut buf).ok()?;
            if n == 0 {
                return None;
            }
            rest.extend_from_slice(&buf[..n]);
        }
        Some(())
    };
    let body = if get("transfer-encoding").is_some() {
        let mut body = Vec::new();
        loop {
            let line_end = loop {
                if let Some(pos) = rest.windows(2).position(|w| w == b"\r\n") {
                    break pos;
                }
                let have = rest.len();
                fill(&mut rest, have + 1)?;
            };
            let size = usize::from_str_radix(
                String::from_utf8_lossy(&rest[..line_end])
                    .split(';')
                    .next()?
                    .trim(),
                16,
            )
            .ok()?;
            fill(&mut rest, line_end + 2 + size + 2)?;
            body.extend_from_slice(&rest[line_end + 2..line_end + 2 + size]);
            rest.drain(..line_end + 2 + size + 2);
            if size == 0 {
                break body;
            }
        }
    } else {
        let length = get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        fill(&mut rest, length)?;
        rest.truncate(length);
        rest
    };
    Some((request_line, headers, body))
}

fn fake_serve(mut stream: std::net::TcpStream, state: &std::sync::Mutex<FakeState>) {
    use std::io::Write as _;
    let Some((request_line, headers, body)) = fake_read_request(&mut stream) else {
        return;
    };
    let get = |name: &str| {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    };
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or("").to_owned();
    let path = parts.next().unwrap_or("").to_owned();
    let body_digest = format!("sha256:{}", dx_digest::sha256_hex(&body));
    let mut state = state.lock().expect("state");
    state.requests.push(FakeRequest {
        method: method.clone(),
        path: path.clone(),
        host: get("host").unwrap_or_default(),
        authorization: get("authorization"),
        content_type: get("content-type"),
        content_length: get("content-length"),
        transfer_encoding: get("transfer-encoding"),
        body_digest: body_digest.clone(),
    });
    let (status, extra, reply) = fake_route(
        &mut state,
        &method,
        &path,
        get("authorization"),
        &body_digest,
    );
    drop(state);
    let mut text = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.len()
    );
    for (key, value) in extra {
        text.push_str(&format!("{key}: {value}\r\n"));
    }
    text.push_str("\r\n");
    text.push_str(&reply);
    let _ = stream.write_all(text.as_bytes());
}

fn fake_route(
    state: &mut FakeState,
    method: &str,
    path: &str,
    authorization: Option<String>,
    body_digest: &str,
) -> (u16, Vec<(String, String)>, String) {
    let none = Vec::new;
    if let Some((_, _, status)) = state
        .config
        .fail
        .iter()
        .find(|(m, p, _)| m == method && path.contains(p.as_str()))
    {
        return (*status, none(), String::new());
    }
    let basic = crate::http::basic_auth(&state.config.user, &state.config.secret);
    if path.starts_with("/token") {
        if authorization.as_deref() != Some(basic.as_str()) {
            return (401, none(), String::new());
        }
        return (200, none(), format!("{{\"token\":\"{FAKE_TOKEN}\"}}"));
    }
    let authorized = match state.config.challenge {
        FakeChallenge::None => true,
        FakeChallenge::Bearer => authorization.as_deref() == Some(&format!("Bearer {FAKE_TOKEN}")),
        FakeChallenge::Basic => authorization.as_deref() == Some(basic.as_str()),
    };
    let upload_on_other = path.starts_with("/upload/");
    if !authorized && !upload_on_other {
        let challenge = match state.config.challenge {
            FakeChallenge::Bearer => format!(
                "Bearer realm=\"{}/token\",service=\"fake.registry\",scope=\"repository:x:pull\"",
                state.origin
            ),
            _ => "Basic realm=\"fake\"".to_owned(),
        };
        return (
            401,
            vec![("WWW-Authenticate".to_owned(), challenge)],
            String::new(),
        );
    }
    if path == "/v2/" {
        return (200, none(), String::new());
    }
    let Some(rest) = path
        .strip_prefix("/v2/demo/app/")
        .or(upload_on_other.then_some(path))
    else {
        return (404, none(), String::new());
    };
    if method == "HEAD" {
        let digest = rest.trim_start_matches("blobs/");
        let status = if state.blobs.iter().any(|b| b == digest) {
            200
        } else {
            404
        };
        return (status, none(), String::new());
    }
    if method == "POST" && rest == "blobs/uploads/" {
        state.uploads += 1;
        let id = format!("u{}", state.uploads);
        let location = match state.config.location {
            FakeLocation::Relative => format!("/v2/demo/app/blobs/uploads/{id}"),
            FakeLocation::RelativeQuery => format!("/v2/demo/app/blobs/uploads/{id}?_state=abc"),
            FakeLocation::Absolute => format!("{}/v2/demo/app/blobs/uploads/{id}", state.origin),
            FakeLocation::OtherHost => format!("{}/upload/{id}?sig=signed", state.other_origin),
        };
        return (202, vec![("Location".to_owned(), location)], String::new());
    }
    if method == "PUT" && (rest.starts_with("blobs/uploads/") || upload_on_other) {
        let want = path
            .split_once("digest=")
            .map(|(_, d)| d.replace("%3A", ":"))
            .unwrap_or_default();
        if want != body_digest {
            return (400, none(), String::new());
        }
        state.blobs.push(want);
        return (201, none(), String::new());
    }
    if method == "PUT" && rest.starts_with("manifests/") {
        let header = match state.config.digest_header {
            FakeDigestHeader::Correct => {
                vec![("Docker-Content-Digest".to_owned(), body_digest.to_owned())]
            }
            FakeDigestHeader::Wrong => vec![(
                "Docker-Content-Digest".to_owned(),
                format!("sha256:{}", "0".repeat(64)),
            )],
            FakeDigestHeader::Missing => none(),
        };
        return (201, header, String::new());
    }
    (404, none(), String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_child_panic_leaves_parent_environment() {
        if std::env::var_os(ENV_CHILD).is_some() {
            with_env(&[("PATH", Some("poisoned"))], || {
                panic!("child panics inside a mutated environment")
            });
        }
        let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut before: Vec<_> = std::env::vars_os().collect();
        before.sort();
        let run = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args([
                "test_support::tests::env_child_panic_leaves_parent_environment",
                "--exact",
            ])
            .env(ENV_CHILD, "1")
            .output()
            .expect("spawn panicking child");
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(
            stdout.contains("child panics inside a mutated environment"),
            "child must panic inside with_env: {stdout}{stderr}"
        );
        assert!(!run.status.success(), "{stdout}{stderr}");
        let mut after: Vec<_> = std::env::vars_os().collect();
        after.sort();
        assert_eq!(after, before, "parent environment changed");
    }
}
