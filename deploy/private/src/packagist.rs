//! Packagist publisher: Composer manifest identity check and the update-package API call.

use crate::run::{DeployRun, OwnerGate, deploy_build_mode, env_or_empty};
use dx_atomic_fs::path_io::{self, PathIoError};
use dx_ecosystems::Ecosystem;
use std::fmt;
use std::path::{Path, PathBuf};

/// The file name of a Composer manifest.
fn manifest_name() -> &'static str {
    Ecosystem::Composer.manifest_name()
}

/// Owner approval gate for packagist_deploy.
pub(crate) const PACKAGIST_GATE: OwnerGate = OwnerGate {
    tool: "packagist_deploy",
    action: "update",
    credentials: &["PACKAGIST_USERNAME", "PACKAGIST_API_TOKEN"],
    approved: "PACKAGIST_PUBLISH_APPROVED",
};

/// Packagist update-package endpoint.
const PACKAGIST_UPDATE_URL: &str = "https://packagist.org/api/update-package";

/// Why the Composer manifest does not name the declared package.
#[derive(Debug)]
pub enum PackagistError {
    Read(PathIoError),
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    NoName {
        path: PathBuf,
    },
    Mismatch {
        actual: String,
        expected: String,
    },
}

impl fmt::Display for PackagistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(source) => write!(f, "{source}"),
            Self::Json { path, source } => {
                write!(f, "{} is not valid JSON: {source}", path.display())
            }
            Self::NoName { path } => write!(f, "{} declares no name", path.display()),
            Self::Mismatch { actual, expected } => write!(
                f,
                "package mismatch: {} declares {actual}, BUILD declares {expected}",
                manifest_name()
            ),
        }
    }
}

impl std::error::Error for PackagistError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::NoName { .. } | Self::Mismatch { .. } => None,
        }
    }
}

/// Reads the package name the Composer manifest declares.
pub fn packagist_read_name(composer_json: &Path) -> Result<String, PackagistError> {
    let text = path_io::read_to_string(composer_json).map_err(PackagistError::Read)?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| PackagistError::Json {
            path: composer_json.to_path_buf(),
            source,
        })?;
    value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| PackagistError::NoName {
            path: composer_json.to_path_buf(),
        })
}

/// Checks the Composer manifest name against the declared package.
pub fn packagist_check_name(actual: &str, expected: &str) -> Result<(), PackagistError> {
    if actual != expected {
        return Err(PackagistError::Mismatch {
            actual: actual.to_owned(),
            expected: expected.to_owned(),
        });
    }
    Ok(())
}

/// Triggers the Packagist update of the repository registered for the package.
fn packagist_live_update(package: &str, repository_url: &str, username: &str, token: &str) -> i32 {
    let body = serde_json::json!({"repository": {"url": repository_url}}).to_string();
    let authorization = format!("Bearer {username}:{token}");
    let result = crate::http::client().and_then(|client| {
        client
            .send(
                dx_http::Method::Post,
                PACKAGIST_UPDATE_URL,
                &[
                    ("Authorization", authorization.as_str()),
                    ("Content-Type", "application/json"),
                ],
                dx_http::Body::Bytes(body.as_bytes()),
                2048,
            )
            .map_err(|error| error.to_string())
    });
    match result {
        Ok(response) => {
            let status = response.status;
            let text = response.text().replace(token, "***");
            if (200..300).contains(&status) {
                println!("packagist_deploy: update requested for {package} (HTTP {status})");
                0
            } else {
                eprintln!("packagist_deploy: HTTP {status}: {}", text.trim());
                1
            }
        }
        Err(error) => {
            let error = error.to_string().replace(token, "***");
            eprintln!("packagist_deploy: update of {package} failed: {error}");
            1
        }
    }
}

/// Runs the Packagist deploy tool.
pub fn packagist_main(
    composer_rloc: &str,
    target_name: &str,
    expected_package: &str,
    repository_url: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "packagist_deploy",
        "the update is for exactly the package pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let Some(composer) = run.resolve(composer_rloc) else {
        return 1;
    };
    let package = match packagist_read_name(&composer)
        .and_then(|name| packagist_check_name(&name, expected_package).map(|()| name))
    {
        Ok(name) => name,
        Err(error) => {
            eprintln!("packagist_deploy: {target_name}: {error}");
            return 1;
        }
    };
    if OwnerGate::requested(env_or_empty) {
        let secrets = match PACKAGIST_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        return packagist_live_update(
            &package,
            repository_url,
            secrets.get("PACKAGIST_USERNAME"),
            secrets.get("PACKAGIST_API_TOKEN"),
        );
    }
    println!(
        "packagist_deploy: would request an update of {package} from {repository_url} (build_mode: {})",
        deploy_build_mode()
    );
    println!("  endpoint: {PACKAGIST_UPDATE_URL}");
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn reads_and_checks_the_package_name() {
        let dir = scratch_dir();
        let path = dir.path().join(manifest_name());
        std::fs::write(&path, r#"{"name": "acme/demo", "require": {}}"#).expect("write");
        let name = packagist_read_name(&path).expect("name");
        assert_eq!(name, "acme/demo");
        packagist_check_name(&name, "acme/demo").expect("same name");
        assert!(
            packagist_check_name(&name, "acme/other")
                .expect_err("mismatch")
                .to_string()
                .contains("BUILD declares acme/other")
        );
    }

    #[test]
    fn rejects_missing_names_and_bad_json() {
        let dir = scratch_dir();
        let path = dir.path().join(manifest_name());
        std::fs::write(&path, "{}").expect("write");
        assert!(
            packagist_read_name(&path)
                .expect_err("no name")
                .to_string()
                .contains("declares no name")
        );
        std::fs::write(&path, "not json").expect("write");
        assert!(
            packagist_read_name(&path)
                .expect_err("bad json")
                .to_string()
                .contains("not valid JSON")
        );
        let missing = dir.path().join("missing.json");
        let error = packagist_read_name(&missing).expect_err("missing");
        assert!(matches!(&error, PackagistError::Read(io) if io.path() == missing));
        assert!(error.to_string().starts_with("cannot read "), "{error}");
    }

    #[test]
    fn default_run_prints_the_plan_and_live_run_needs_approval() {
        if env_test_in_child(
            "packagist::tests::default_run_prints_the_plan_and_live_run_needs_approval",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        std::fs::write(runfiles.join(manifest_name()), r#"{"name": "acme/demo"}"#).expect("write");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles_dir = runfiles.to_string_lossy().into_owned();
        let launch = |live: Option<&str>, package: &str| {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles_dir.as_str())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("DX_DEPLOY_LIVE", live),
                    ("PACKAGIST_PUBLISH_APPROVED", None),
                    ("PACKAGIST_USERNAME", None),
                    ("PACKAGIST_API_TOKEN", None),
                ],
                || {
                    packagist_main(
                        manifest_name(),
                        "demo",
                        package,
                        "https://github.com/acme/demo",
                        &argv,
                    )
                },
            )
        };
        assert_eq!(launch(None, "acme/demo"), 0);
        assert_eq!(launch(None, "acme/other"), 1);
        assert_eq!(launch(Some("1"), "acme/demo"), 1);
    }
}
