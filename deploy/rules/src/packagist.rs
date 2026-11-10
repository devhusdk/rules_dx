//! Packagist publisher: composer.json identity check and the update-package API call.

use crate::run::{DeployRun, OwnerGate, deploy_build_mode, env_or_empty};
use std::io::Read;
use std::path::Path;

/// Owner approval gate for packagist_deploy.
pub(crate) const PACKAGIST_GATE: OwnerGate = OwnerGate {
    tool: "packagist_deploy",
    action: "update",
    credentials: &["PACKAGIST_USERNAME", "PACKAGIST_API_TOKEN"],
    approved: "PACKAGIST_PUBLISH_APPROVED",
};

/// Packagist update-package endpoint.
const PACKAGIST_UPDATE_URL: &str = "https://packagist.org/api/update-package";

/// Reads the package name composer.json declares.
pub fn packagist_read_name(composer_json: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(composer_json)
        .map_err(|error| format!("cannot read {}: {error}", composer_json.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not valid JSON: {error}", composer_json.display()))?;
    value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{} declares no name", composer_json.display()))
}

/// Checks the composer.json name against the declared package.
pub fn packagist_check_name(actual: &str, expected: &str) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "package mismatch: composer.json declares {actual}, BUILD declares {expected}"
        ));
    }
    Ok(())
}

/// Triggers the Packagist update of the repository registered for the package.
fn packagist_live_update(package: &str, repository_url: &str, username: &str, token: &str) -> i32 {
    let body = serde_json::json!({"repository": {"url": repository_url}}).to_string();
    let result = crate::http::publish_agent()
        .post(PACKAGIST_UPDATE_URL)
        .header("Authorization", format!("Bearer {username}:{token}"))
        .header("Content-Type", "application/json")
        .send(body.as_bytes());
    match result {
        Ok(mut response) => {
            let status = response.status().as_u16();
            let mut text = Vec::new();
            let _ = response
                .body_mut()
                .as_reader()
                .take(2048)
                .read_to_end(&mut text);
            let text = String::from_utf8_lossy(&text).replace(token, "***");
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
    if PACKAGIST_GATE.requested(env_or_empty) {
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
        let path = dir.path().join("composer.json");
        std::fs::write(&path, r#"{"name": "acme/demo", "require": {}}"#).expect("write");
        let name = packagist_read_name(&path).expect("name");
        assert_eq!(name, "acme/demo");
        assert_eq!(packagist_check_name(&name, "acme/demo"), Ok(()));
        assert!(
            packagist_check_name(&name, "acme/other")
                .expect_err("mismatch")
                .contains("BUILD declares acme/other")
        );
    }

    #[test]
    fn rejects_missing_names_and_bad_json() {
        let dir = scratch_dir();
        let path = dir.path().join("composer.json");
        std::fs::write(&path, "{}").expect("write");
        assert!(
            packagist_read_name(&path)
                .expect_err("no name")
                .contains("declares no name")
        );
        std::fs::write(&path, "not json").expect("write");
        assert!(
            packagist_read_name(&path)
                .expect_err("bad json")
                .contains("not valid JSON")
        );
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
        std::fs::write(runfiles.join("composer.json"), r#"{"name": "acme/demo"}"#).expect("write");
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
                        "composer.json",
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
