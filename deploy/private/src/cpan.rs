use crate::archive::basename;
use crate::run::{
    DeployRun, OwnerGate, deploy_build_mode, env_or_empty, pages_redact, placeholder_refusal,
};
use crate::stage::*;
use dx_atomic_fs::path_io::{self, PathIoError};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

pub(crate) const CPAN_GATE: OwnerGate = OwnerGate {
    tool: "cpan_deploy",
    action: "upload",
    credentials: &["CPAN_API_USER", "CPAN_API_TOKEN"],
    approved: "CPAN_PUBLISH_APPROVED",
};

const PAUSE_UPLOAD_URL: &str = "https://pause.perl.org/pause/authenquery?ACTION=add_uri";

#[derive(Debug)]
pub enum CpanError {
    NotTarGz {
        file_name: String,
    },
    BadFileName {
        file_name: String,
    },
    Open {
        path: PathBuf,
        source: io::Error,
    },
    InvalidDist {
        file_name: String,
        source: io::Error,
    },
    NoMeta {
        file_name: String,
        stem: String,
    },
    PackageMismatch {
        actual: String,
        expected: String,
    },
    VersionMismatch {
        actual: String,
        expected: String,
    },
    NoFileName {
        path: PathBuf,
    },
    Read(PathIoError),
    Transport {
        url: String,
        message: String,
    },
    Rejected {
        status: u16,
    },
    Status {
        url: String,
        status: u16,
        body: String,
    },
}

impl fmt::Display for CpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTarGz { file_name } => write!(f, "{file_name} is not a .tar.gz dist"),
            Self::BadFileName { file_name } => {
                write!(f, "{file_name} is not named <Dist>-<version>.tar.gz")
            }
            Self::Open { path, source } => write!(f, "cannot open {}: {source}", path.display()),
            Self::InvalidDist { file_name, source } => {
                write!(f, "{file_name} is not a valid dist: {source}")
            }
            Self::NoMeta { file_name, stem } => {
                write!(f, "{file_name} has no {stem}/META.json or {stem}/META.yml")
            }
            Self::PackageMismatch { actual, expected } => write!(
                f,
                "package mismatch: dist is {actual}, BUILD declares {expected}"
            ),
            Self::VersionMismatch { actual, expected } => write!(
                f,
                "version mismatch: dist is {actual}, BUILD declares {expected}"
            ),
            Self::NoFileName { path } => {
                write!(f, "cpan_deploy: {} has no file name", path.display())
            }
            Self::Read(source) => write!(f, "cpan_deploy: {source}"),
            Self::Transport { url, message } => {
                write!(f, "cpan_deploy: upload to {url} failed: {message}")
            }
            Self::Rejected { status } => write!(
                f,
                "cpan_deploy: PAUSE rejected CPAN_API_USER and CPAN_API_TOKEN (HTTP {status})"
            ),
            Self::Status { url, status, body } => write!(
                f,
                "cpan_deploy: upload to {url} failed with HTTP {status}: {body}"
            ),
        }
    }
}

impl std::error::Error for CpanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Open { source, .. } | Self::InvalidDist { source, .. } => Some(source),
            Self::Read(source) => Some(source),
            Self::NotTarGz { .. }
            | Self::BadFileName { .. }
            | Self::NoMeta { .. }
            | Self::PackageMismatch { .. }
            | Self::VersionMismatch { .. }
            | Self::NoFileName { .. }
            | Self::Transport { .. }
            | Self::Rejected { .. }
            | Self::Status { .. } => None,
        }
    }
}

pub fn cpan_read_identity(dist: &Path) -> Result<(String, String), CpanError> {
    let file_name = dist
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem = file_name
        .strip_suffix(".tar.gz")
        .ok_or_else(|| CpanError::NotTarGz {
            file_name: file_name.to_owned(),
        })?;
    let (name, version) = stem
        .rsplit_once('-')
        .filter(|(name, version)| !name.is_empty() && !version.is_empty())
        .ok_or_else(|| CpanError::BadFileName {
            file_name: file_name.to_owned(),
        })?;
    let wants = [format!("{stem}/META.json"), format!("{stem}/META.yml")];
    let open = std::fs::File::open(dist).map_err(|source| CpanError::Open {
        path: dist.to_path_buf(),
        source,
    })?;
    let mut found = false;
    dx_archive::visit(
        open,
        dx_archive::Format::TarGz,
        crate::archive::LISTING_LIMITS,
        |path, _, _| {
            found |= wants.iter().any(|want| want == path);
            Ok(())
        },
    )
    .map_err(|source| CpanError::InvalidDist {
        file_name: file_name.to_owned(),
        source,
    })?;
    if found {
        return Ok((name.to_owned(), version.to_owned()));
    }
    Err(CpanError::NoMeta {
        file_name: file_name.to_owned(),
        stem: stem.to_owned(),
    })
}

pub fn cpan_check_identity(
    actual: &(String, String),
    expected_package: &str,
    expected_version: &str,
) -> Result<(), CpanError> {
    if !expected_package.is_empty() && actual.0 != expected_package {
        return Err(CpanError::PackageMismatch {
            actual: actual.0.clone(),
            expected: expected_package.to_owned(),
        });
    }
    if !expected_version.is_empty() && actual.1 != expected_version {
        return Err(CpanError::VersionMismatch {
            actual: actual.1.clone(),
            expected: expected_version.to_owned(),
        });
    }
    Ok(())
}

fn cpan_live_upload(
    client: &dx_http::Client,
    url: &str,
    dist: &Path,
    user: &str,
    token: &str,
) -> Result<(), CpanError> {
    let file_name = dist
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CpanError::NoFileName {
            path: dist.to_path_buf(),
        })?;
    let bytes = path_io::read(dist).map_err(CpanError::Read)?;
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
    let authorization = crate::http::basic_auth(user, token);
    let response = client
        .send(
            dx_http::Method::Post,
            url,
            &[
                ("Authorization", authorization.as_str()),
                ("Content-Type", content_type.as_str()),
            ],
            dx_http::Body::Bytes(&body),
            2048,
        )
        .map_err(|error| CpanError::Transport {
            url: url.to_owned(),
            message: pages_redact(&error.to_string(), token),
        })?;
    let status = response.status;
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 401 || status == 403 {
        return Err(CpanError::Rejected { status });
    }
    Err(CpanError::Status {
        url: url.to_owned(),
        status,
        body: pages_redact(&response.text(), token).trim().to_owned(),
    })
}

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
    if OwnerGate::requested(env_or_empty) {
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
        let client = match crate::http::client() {
            Ok(client) => client,
            Err(error) => {
                eprintln!("cpan_deploy: {error}");
                return 1;
            }
        };
        return match cpan_live_upload(
            &client,
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
        let target = dir.join(base);
        copy_verified(&dist, &target)?;
        Ok(target)
    });
    match staged {
        Ok(copy) => {
            println!(
                "cpan_deploy: staged {package} {version} (build_mode: {})",
                deploy_build_mode()
            );
            println!("  dist: {}", copy.display());
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
        let mut tar = dx_archive::TarWriter::new();
        tar.add_file(entry, b"{}", 0o644).expect("append");
        let path = dir.join(file_name);
        std::fs::write(&path, tar.finish().expect("finish tar.gz")).expect("write dist");
        path
    }

    #[test]
    fn refuses_a_dist_with_a_dot_dot_member() {
        let dir = scratch_dir();
        let dist = dir.path().join("Demo-Dist-1.2.0.tar.gz");
        std::fs::write(&dist, raw_tar_gz("../META.json", b"{}")).expect("write dist");
        let error = cpan_read_identity(&dist).expect_err("dot dot");
        assert!(matches!(error, CpanError::InvalidDist { .. }), "{error}");
        let error = error.to_string();
        assert!(error.contains("is not a valid dist"), "{error}");
        assert!(error.contains("'..'"), "{error}");
    }

    #[test]
    fn reads_identity_from_the_file_name_and_meta() {
        let dir = scratch_dir();
        let dist = write_dist(
            dir.path(),
            "Demo-Dist-1.2.0.tar.gz",
            "Demo-Dist-1.2.0/META.json",
        );
        let identity = cpan_read_identity(&dist).expect("identity");
        assert_eq!(identity, ("Demo-Dist".to_owned(), "1.2.0".to_owned()));
        assert!(cpan_check_identity(&identity, "Demo-Dist", "1.2.0").is_ok());
        assert!(cpan_check_identity(&identity, "", "").is_ok());
        let version = cpan_check_identity(&identity, "", "1.2.1").expect_err("version");
        assert!(matches!(version, CpanError::VersionMismatch { .. }));
        assert_eq!(
            version.to_string(),
            "version mismatch: dist is 1.2.0, BUILD declares 1.2.1"
        );
        let package = cpan_check_identity(&identity, "Other", "").expect_err("package");
        assert!(matches!(package, CpanError::PackageMismatch { .. }));
        assert_eq!(
            package.to_string(),
            "package mismatch: dist is Demo-Dist, BUILD declares Other"
        );
        let yml = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/META.yml");
        assert!(cpan_read_identity(&yml).is_ok());
    }

    #[test]
    fn rejects_bad_names_and_missing_meta() {
        let dir = scratch_dir();
        let plain = dir.path().join("Demo.zip");
        std::fs::write(&plain, b"x").expect("write");
        let suffix = cpan_read_identity(&plain).expect_err("suffix");
        assert!(matches!(suffix, CpanError::NotTarGz { .. }));
        assert_eq!(suffix.to_string(), "Demo.zip is not a .tar.gz dist");
        let unversioned = write_dist(dir.path(), "Demo.tar.gz", "Demo/META.json");
        let name = cpan_read_identity(&unversioned).expect_err("name");
        assert!(matches!(name, CpanError::BadFileName { .. }));
        assert_eq!(
            name.to_string(),
            "Demo.tar.gz is not named <Dist>-<version>.tar.gz"
        );
        let missing = write_dist(dir.path(), "Demo-1.0.tar.gz", "Demo-1.0/Makefile.PL");
        let meta = cpan_read_identity(&missing).expect_err("missing");
        assert!(matches!(meta, CpanError::NoMeta { .. }));
        assert_eq!(
            meta.to_string(),
            "Demo-1.0.tar.gz has no Demo-1.0/META.json or Demo-1.0/META.yml"
        );
        let absent = dir.path().join("Absent-1.0.tar.gz");
        let open = cpan_read_identity(&absent).expect_err("absent");
        assert!(matches!(&open, CpanError::Open { path, .. } if path == &absent));
        assert!(open.to_string().starts_with("cannot open "), "{open}");
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

    fn fake_pause(status: &str, body: &str) -> (String, std::sync::mpsc::Receiver<Vec<u8>>) {
        use std::io::{Read as _, Write as _};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!(
            "http://{}{}",
            listener.local_addr().expect("addr"),
            PAUSE_UPLOAD_URL
                .strip_prefix("https://pause.perl.org")
                .expect("PAUSE origin")
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

    fn upload_to(url: &str, dist: &Path) -> Result<(), CpanError> {
        cpan_live_upload(
            &crate::http::test_client(),
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
        assert!(upload_to(&url, &dist).is_ok());
        let seen = request.recv().expect("request");
        let text = String::from_utf8_lossy(&seen).into_owned();
        assert!(text.starts_with("POST /pause/authenquery?ACTION=add_uri HTTP/1.1\r\n"));
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
        let rejected = upload_to(&url, &dist).expect_err("401");
        assert!(matches!(rejected, CpanError::Rejected { status: 401 }));
        assert_eq!(
            rejected.to_string(),
            "cpan_deploy: PAUSE rejected CPAN_API_USER and CPAN_API_TOKEN (HTTP 401)"
        );
        let (url, _request) = fake_pause(
            "500 Internal Server Error",
            &format!("boom {FAKE_PAUSE_TOKEN}\n"),
        );
        let failed = upload_to(&url, &dist).expect_err("500");
        assert!(matches!(failed, CpanError::Status { status: 500, .. }));
        assert_eq!(
            failed.to_string(),
            format!("cpan_deploy: upload to {url} failed with HTTP 500: boom [redacted]")
        );
        let (url, _request) = fake_pause("302 Found", "");
        assert!(
            upload_to(&url, &dist)
                .expect_err("redirect")
                .to_string()
                .contains("failed with HTTP 302")
        );
    }

    #[test]
    fn live_upload_reports_an_unreadable_dist() {
        let dir = scratch_dir();
        let missing = dir.path().join("Demo-1.2.0.tar.gz");
        let error = upload_to("http://127.0.0.1:1/", &missing).expect_err("missing");
        assert!(matches!(&error, CpanError::Read(io) if io.path() == missing));
        assert!(
            error.to_string().starts_with("cpan_deploy: cannot read "),
            "{error}"
        );
    }
}
