//! CPAN publisher: dist identity checks, local staging and the PAUSE upload.

use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, deploy_build_mode, env_or_empty, pages_redact, placeholder_refusal,
};
use crate::stage::*;
use std::path::Path;

/// Owner approval gate for cpan_deploy.
pub(crate) const CPAN_GATE: OwnerGate = OwnerGate {
    tool: "cpan_deploy",
    action: "upload",
    credentials: &["CPAN_API_USER", "CPAN_API_TOKEN"],
    approved: "CPAN_PUBLISH_APPROVED",
};

/// PAUSE endpoint that takes authenticated uploads.
const PAUSE_UPLOAD_URL: &str = "https://pause.perl.org/pause/authenquery";

/// Reads the distribution name and version from a `<Dist>-<version>.tar.gz` file name and checks the archive.
///
/// The archive must hold `<Dist>-<version>/META.json` or `<Dist>-<version>/META.yml`.
pub fn cpan_read_identity(dist: &Path) -> Result<(String, String), String> {
    let file_name = dist
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".tar.gz")
        .ok_or_else(|| format!("{file_name} is not a .tar.gz dist"))?;
    let (name, version) = stem
        .rsplit_once('-')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .ok_or_else(|| format!("{file_name} is not named <Dist>-<version>.tar.gz"))?;
    let wants = [format!("{stem}/META.json"), format!("{stem}/META.yml")];
    let open = std::fs::File::open(dist)
        .map_err(|error| format!("cannot open {}: {error}", dist.display()))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(open));
    let entries = archive
        .entries()
        .map_err(|error| format!("{file_name} is not a valid dist: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{file_name} is not a valid dist: {error}"))?;
        let path = entry
            .path()
            .map_err(|error| format!("{file_name} has an unreadable entry name: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        if wants.contains(&path) {
            return Ok((name.to_owned(), version.to_owned()));
        }
    }
    Err(format!("{file_name} has no {} or {}", wants[0], wants[1]))
}

/// Checks the dist identity against the declared package and version.
pub fn cpan_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), String> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(format!(
            "package mismatch: dist is {}, BUILD declares {expected_package}",
            actual.0
        ));
    }
    if !expected_version.is_empty() && actual.1 != expected_version {
        return Err(format!(
            "version mismatch: dist is {}, BUILD declares {expected_version}",
            actual.1
        ));
    }
    Ok(())
}

/// Uploads the dist to the PAUSE endpoint at `url` with HTTP basic auth.
fn cpan_live_upload(
    agent: &ureq::Agent,
    url: &str,
    dist: &Path,
    user: &str,
    token: &str,
) -> Result<(), String> {
    use std::io::Read as _;
    let file_name = dist
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("cpan_deploy: {} has no file name", dist.display()))?;
    let bytes = std::fs::read(dist)
        .map_err(|error| format!("cpan_deploy: cannot read {}: {error}", dist.display()))?;
    let hidden_name = user.to_ascii_uppercase();
    let (content_type, body) = crate::http::multipart_form(
        &[
            ("HIDDENNAME", hidden_name.as_str()),
            ("CAN_MULTIPART", "1"),
            ("pause99_add_uri_upload", file_name),
            (
                "SUBMIT_pause99_add_uri_httpupload",
                " Upload this file from my disk ",
            ),
            ("pause99_add_uri_uri", ""),
        ],
        "pause99_add_uri_httpupload",
        file_name,
        &bytes,
    );
    let mut response = agent
        .post(url)
        .header("Authorization", &crate::http::basic_auth(user, token))
        .header("Content-Type", &content_type)
        .send(&body[..])
        .map_err(|error| {
            format!(
                "cpan_deploy: upload to {url} failed: {}",
                pages_redact(&error.to_string(), token)
            )
        })?;
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 401 || status == 403 {
        return Err(format!(
            "cpan_deploy: PAUSE rejected CPAN_API_USER and CPAN_API_TOKEN (HTTP {status})"
        ));
    }
    let mut text = Vec::new();
    let _ = response
        .body_mut()
        .as_reader()
        .take(2048)
        .read_to_end(&mut text);
    Err(format!(
        "cpan_deploy: upload to {url} failed with HTTP {status}: {}",
        pages_redact(&String::from_utf8_lossy(&text), token).trim()
    ))
}

/// Runs the CPAN deploy tool.
pub fn cpan_main(
    dist_rloc: &str,
    target_name: &str,
    expected_package: &str,
    expected_version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "cpan_deploy",
        "the upload is exactly the dist pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let Some(dist) = run.resolve(dist_rloc) else {
        return 1;
    };
    let identity = match cpan_read_identity(&dist).and_then(|identity| {
        cpan_check_identity(&identity, expected_package, expected_version).map(|()| identity)
    }) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("cpan_deploy: {target_name}: {error}");
            return 1;
        }
    };
    let (package, version) = (identity.0.as_str(), identity.1.as_str());
    if CPAN_GATE.requested(env_or_empty) {
        if let Some(refusal) = placeholder_refusal("cpan_deploy", version) {
            eprintln!("{refusal}");
            return 1;
        }
        let secrets = match CPAN_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        return match cpan_live_upload(
            &crate::http::publish_agent(),
            PAUSE_UPLOAD_URL,
            &dist,
            secrets.get("CPAN_API_USER"),
            secrets.get("CPAN_API_TOKEN"),
        ) {
            Ok(()) => {
                println!("cpan_deploy: uploaded {package} {version}");
                0
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
    }
    let staged = basename(&dist).and_then(|base| {
        let dir = run.outdir().join(format!("{target_name}-dist"));
        std::fs::create_dir_all(&dir)?;
        let dest = dir.join(base);
        copy_verified(&dist, &dest)?;
        Ok(dest)
    });
    match staged {
        Ok(dest) => {
            println!(
                "cpan_deploy: staged {package} {version} (build_mode: {})",
                deploy_build_mode()
            );
            println!("  dist: {}", dest.display());
            0
        }
        Err(error) => {
            eprintln!("cpan_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use std::path::PathBuf;

    fn write_dist(dir: &Path, file_name: &str, entry: &str) -> PathBuf {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(2);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, entry, &b"{}"[..])
            .expect("append meta");
        let bytes = builder.into_inner().expect("finish tar");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::Write::write_all(&mut encoder, &bytes).expect("gzip");
        let path = dir.join(file_name);
        std::fs::write(&path, encoder.finish().expect("finish gzip")).expect("write dist");
        path
    }

    #[test]
    fn reads_identity_from_the_file_name_and_meta() {
        let dir = scratch_dir();
        let dist = write_dist(dir.path(), "Demo-Dist-1.2.0.tar.gz", "Demo-Dist-1.2.0/META.json");
        let identity = cpan_read_identity(&dist).expect("identity");
        assert_eq!(identity, ("Demo-Dist".to_owned(), "1.2.0".to_owned()));
        assert_eq!(cpan_check_identity(&identity, "Demo-Dist", "1.2.0"), Ok(()));
        assert_eq!(cpan_check_identity(&identity, "", ""), Ok(()));
        assert!(
            cpan_check_identity(&identity, "", "1.2.1")
                .expect_err("version")
                .contains("BUILD declares 1.2.1")
        );
        assert!(
            cpan_check_identity(&identity, "Other", "")
                .expect_err("package")
                .contains("BUILD declares Other")
        );
        let yml = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/META.yml");
        assert!(cpan_read_identity(&yml).is_ok());
    }

    #[test]
    fn rejects_bad_names_and_missing_meta() {
        let dir = scratch_dir();
        let plain = dir.path().join("Demo.zip");
        std::fs::write(&plain, b"x").expect("write");
        assert!(
            cpan_read_identity(&plain)
                .expect_err("suffix")
                .contains("not a .tar.gz")
        );
        let unversioned = write_dist(dir.path(), "Demo.tar.gz", "Demo/META.json");
        assert!(
            cpan_read_identity(&unversioned)
                .expect_err("name")
                .contains("<Dist>-<version>")
        );
        let missing = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/Makefile.PL");
        assert!(
            cpan_read_identity(&missing)
                .expect_err("missing")
                .contains("has no Demo-1.0/META.json")
        );
    }

    #[test]
    fn default_run_stages_the_dist_and_live_run_needs_approval() {
        if env_test_in_child("cpan::tests::default_run_stages_the_dist_and_live_run_needs_approval")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        write_dist(&runfiles, "Demo-1.2.0.tar.gz", "Demo-1.2.0/META.json");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let launch = |live: Option<&str>| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("DX_DEPLOY_LIVE", live),
                    ("CPAN_PUBLISH_APPROVED", None),
                    ("CPAN_API_USER", None),
                    ("CPAN_API_TOKEN", None),
                ],
                || cpan_main("Demo-1.2.0.tar.gz", "demo", "Demo", "1.2.0", &argv),
            )
        };
        assert_eq!(launch(None), 0);
        assert!(outdir.join("demo-dist/Demo-1.2.0.tar.gz").is_file());
        assert_eq!(launch(Some("1")), 1);
    }

    const FAKE_PAUSE_USER: &str = "fakeuser";
    const FAKE_PAUSE_TOKEN: &str = "fake-pause-token";

    /// Serves one request on a loopback port with `status` and `body`, then hands back the raw request.
    fn fake_pause(status: &str, body: &str) -> (String, std::sync::mpsc::Receiver<Vec<u8>>) {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!(
            "http://{}/pause/authenquery",
            listener.local_addr().expect("addr")
        );
        let (send, request) = std::sync::mpsc::channel();
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut seen = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = stream.read(&mut buf).expect("read");
                if n == 0 {
                    break;
                }
                seen.extend_from_slice(&buf[..n]);
                let Some(end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else {
                    continue;
                };
                let length = String::from_utf8_lossy(&seen[..end])
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|value| value.trim().parse::<usize>().expect("length"))
                    })
                    .unwrap_or(0);
                if seen.len() >= end + 4 + length {
                    break;
                }
            }
            stream.write_all(reply.as_bytes()).expect("reply");
            send.send(seen).expect("send");
        });
        (url, request)
    }

    fn upload_to(url: &str, dist: &Path) -> Result<(), String> {
        cpan_live_upload(
            &crate::http::publish_agent_with_proxy(None),
            url,
            dist,
            FAKE_PAUSE_USER,
            FAKE_PAUSE_TOKEN,
        )
    }

    #[test]
    fn live_upload_posts_the_pause_form_with_basic_auth() {
        let dir = scratch_dir();
        let dist = write_dist(dir.path(), "Demo-1.2.0.tar.gz", "Demo-1.2.0/META.json");
        let (url, request) = fake_pause("200 OK", "ok");
        assert_eq!(upload_to(&url, &dist), Ok(()));
        let seen = request.recv().expect("request");
        let text = String::from_utf8_lossy(&seen).into_owned();
        assert!(text.starts_with("POST /pause/authenquery HTTP/1.1\r\n"));
        let lower = text.to_ascii_lowercase();
        let auth = crate::http::basic_auth(FAKE_PAUSE_USER, FAKE_PAUSE_TOKEN).to_ascii_lowercase();
        assert!(lower.contains(&format!("authorization: {auth}\r\n")));
        assert!(lower.contains("content-type: multipart/form-data; boundary=dx-boundary-"));
        assert!(!text.contains(FAKE_PAUSE_TOKEN));
        for part in [
            "name=\"HIDDENNAME\"\r\n\r\nFAKEUSER\r\n",
            "name=\"CAN_MULTIPART\"\r\n\r\n1\r\n",
            "name=\"pause99_add_uri_upload\"\r\n\r\nDemo-1.2.0.tar.gz\r\n",
            "name=\"SUBMIT_pause99_add_uri_httpupload\"\r\n\r\n Upload this file from my disk \r\n",
            "name=\"pause99_add_uri_uri\"\r\n\r\n\r\n",
            "name=\"pause99_add_uri_httpupload\"; filename=\"Demo-1.2.0.tar.gz\"\r\n",
        ] {
            assert!(text.contains(part), "missing {part:?}");
        }
        let bytes = std::fs::read(&dist).expect("dist bytes");
        assert!(seen.windows(bytes.len()).any(|window| window == &bytes[..]));
    }

    #[test]
    fn live_upload_reports_rejected_credentials_and_server_errors() {
        let dir = scratch_dir();
        let dist = write_dist(dir.path(), "Demo-1.2.0.tar.gz", "Demo-1.2.0/META.json");
        let (url, _request) = fake_pause("401 Unauthorized", "denied");
        assert_eq!(
            upload_to(&url, &dist),
            Err(
                "cpan_deploy: PAUSE rejected CPAN_API_USER and CPAN_API_TOKEN (HTTP 401)"
                    .to_owned()
            )
        );
        let (url, _request) = fake_pause(
            "500 Internal Server Error",
            &format!("boom {FAKE_PAUSE_TOKEN}\n"),
        );
        assert_eq!(
            upload_to(&url, &dist),
            Err(format!(
                "cpan_deploy: upload to {url} failed with HTTP 500: boom [redacted]"
            ))
        );
        let (url, _request) = fake_pause("302 Found", "");
        assert!(
            upload_to(&url, &dist)
                .expect_err("redirect")
                .contains("failed with HTTP 302")
        );
    }
}
