//! Environment promotion: record, health check and rollback.

use crate::archive::*;
use crate::run::{deploy_profile, env_or_empty, DeployRun, OwnerGate};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Formats one promotion command.
pub fn promotion_promote_command(
    artifact_base: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
) -> String {
    format!("promote {artifact_base} {from_env} -> {to_env} version {version}")
}

/// Formats one rollback command.
pub fn promotion_rollback_command(deploy_name: &str, rollback_to: &str) -> String {
    format!(
        "rollback {deploy_name} to {rollback_to} (restore the pinned artifact for that version)"
    )
}

/// Health result recorded for one promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromotionHealth {
    Passed,
    NotRequired,
    SkippedLocal,
}

impl PromotionHealth {
    /// Returns the value written to promotion.json.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::NotRequired => "not-required",
            Self::SkippedLocal => "skipped-local",
        }
    }

    /// Returns the health line written to would-run.txt.
    pub fn manifest_line(self) -> &'static str {
        match self {
            Self::Passed => "health: passed (PROMOTION_HEALTH_CMD exited 0)",
            Self::NotRequired => "health: not required; set PROMOTION_REQUIRE_HEALTH=1 plus PROMOTION_HEALTH_CMD to gate the promotion on a health check",
            Self::SkippedLocal => "health: skipped locally; set PROMOTION_REQUIRE_HEALTH=1 plus PROMOTION_HEALTH_CMD to gate the promotion on a health check",
        }
    }
}

/// Builds one promotion staging dir.
#[allow(clippy::too_many_arguments)]
pub fn promotion_build(
    artifact_src: &Path,
    outdir: &Path,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    secret_refs: &[String],
    health: PromotionHealth,
) -> io::Result<std::path::PathBuf> {
    if !artifact_src.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "promotion: want exactly one existing artifact source, got '{}'",
                artifact_src.display()
            ),
        ));
    }
    if deploy_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty deploy name",
        ));
    }
    if from_env.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty source environment",
        ));
    }
    if to_env.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty target environment",
        ));
    }
    if from_env == to_env {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("promotion: source and target environments must differ, got '{from_env}'"),
        ));
    }
    if version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion: need a non-empty version",
        ));
    }
    let promotion = outdir.join(deploy_name.to_owned() + "-promotion");
    std::fs::create_dir_all(&promotion)?;
    let base = basename(artifact_src)?;
    let dest = promotion.join(&base);
    let digest = copy_verified(artifact_src, &dest)?;
    let mut refs: Vec<String> = secret_refs.to_vec();
    refs.sort();
    let record = render_json(&serde_json::json!({
        "artifact": base,
        "artifact_sha256": digest,
        "deploy": deploy_name,
        "from_environment": from_env,
        "health": health.as_str(),
        "registry_auth": "env-only",
        "secret_refs": refs,
        "to_environment": to_env,
        "version": version,
    }))?;
    std::fs::write(promotion.join("promotion.json"), record.as_bytes())?;
    let refs_line = if refs.is_empty() {
        "secret-refs: none".to_owned()
    } else {
        format!("secret-refs: {} (names only, never values)", refs.join(","))
    };
    let manifest = format!(
        "# would-run manifest for promotion_deploy (publishes nothing; live gates need PROMOTION_LIVE=1 plus PROMOTION_APPROVED=1)\n{}\n{}\nrollback: set PROMOTION_ROLLBACK=1 plus PROMOTION_ROLLBACK_TO=<version> to record a rollback pointer (history lives in the promotion dir or registry)\nartifact-sha256: {digest}  {base}\n{refs_line}\nregistry-auth: env-only (PROMOTION_REGISTRY_USER/PROMOTION_REGISTRY_TOKEN), never from BUILD\n",
        promotion_promote_command(&base, from_env, to_env, version),
        health.manifest_line(),
    );
    std::fs::write(promotion.join("would-run.txt"), manifest.as_bytes())?;
    Ok(promotion)
}

/// Records one rollback pointer.
pub fn promotion_record_rollback(
    promotion_dir: &Path,
    deploy_name: &str,
    rollback_to: &str,
    rollback_sha: &str,
) -> io::Result<std::path::PathBuf> {
    if !promotion_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "promotion rollback: want an existing promotion directory, got '{}'",
                promotion_dir.display()
            ),
        ));
    }
    if rollback_to.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion rollback: need PROMOTION_ROLLBACK_TO=<version>",
        ));
    }
    let mut body = format!(
        "# rollback pointer for promotion_deploy (restores, never deletes)\n{}\n",
        promotion_rollback_command(deploy_name, rollback_to),
    );
    if !rollback_sha.is_empty() {
        body.push_str(&format!("expected-sha256: {rollback_sha}\n"));
    }
    let path = promotion_dir.join("rollback.txt");
    std::fs::write(&path, body.as_bytes())?;
    Ok(path)
}

/// Validates one promotion secret ref list.
pub fn promotion_check_secret_refs(secret_refs: &[String]) -> io::Result<Vec<String>> {
    let mut normalized = Vec::new();
    for r in secret_refs {
        if let Some(path) = r.strip_prefix("file:") {
            if path.is_empty() || !std::path::Path::new(path).exists() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("promotion secrets: secret file not found for ref '{r}'"),
                ));
            }
            normalized.push(r.clone());
        } else if let Some(rest) = r.strip_prefix("cmd:") {
            let tool = rest.split(' ').next().unwrap_or("");
            if tool.is_empty() || which_on_path(tool).is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "promotion secrets: secret tool not installed for ref '{r}' (install it separately and accept its license first)"
                    ),
                ));
            }
            normalized.push(r.clone());
        } else if r.is_empty() || r.chars().any(|c| " \n'\"\\$`".contains(c)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "promotion secrets: want ENV_NAME, file:<path>, or cmd:<tool> refs, got '{r}'"
                ),
            ));
        } else {
            if std::env::var(r).unwrap_or_default().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("promotion secrets: env ref '{r}' is not set"),
                ));
            }
            normalized.push(r.clone());
        }
    }
    Ok(normalized)
}

/// Runs one promotion health command.
pub fn promotion_run_health_cmd(health_cmd: &str) -> io::Result<i32> {
    let argv = shlex_split(health_cmd);
    if argv.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "promotion health: need a non-empty PROMOTION_HEALTH_CMD",
        ));
    }
    if which_on_path(&argv[0]).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("promotion health: command not found on PATH: '{}'", argv[0]),
        ));
    }
    let mut cmd = std::process::Command::new(&argv[0]);
    for arg in &argv[1..] {
        cmd.arg(arg);
    }
    Ok(cmd.status()?.code().unwrap_or(1))
}

fn shlex_split(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = cmd.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            } else {
                current.push(c);
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == '\\' {
            if let Some(next) = chars.next() {
                current.push(next);
            }
        } else if c.is_whitespace() {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Owner approval gate for promotion_deploy.
pub(crate) const PROMOTION_GATE: OwnerGate = OwnerGate {
    tool: "promotion_deploy",
    action: "promote",
    credentials: &[],
    approved: "PROMOTION_APPROVED",
    live: "PROMOTION_LIVE",
};

/// Runs the promotion deploy launcher.
pub fn promotion_main(
    artifact_rloc: &str,
    deploy_name: &str,
    from_env: &str,
    to_env: &str,
    version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "promotion_deploy",
        "the promotion is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(artifact) = run.resolve(artifact_rloc) else {
        return 1;
    };
    let raw_refs = std::env::var("PROMOTION_SECRET_REFS").unwrap_or_default();
    let secret_refs: Vec<String> = raw_refs
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if std::env::var("PROMOTION_ROLLBACK").unwrap_or_default() == "1" {
        let promotion = match promotion_build(
            &artifact,
            outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
            PromotionHealth::SkippedLocal,
        ) {
            Ok(promotion) => promotion,
            Err(error) => {
                eprintln!("promotion_deploy: {error}");
                return 1;
            }
        };
        let rollback_to = std::env::var("PROMOTION_ROLLBACK_TO").unwrap_or_default();
        let rollback_sha = std::env::var("PROMOTION_ROLLBACK_SHA").unwrap_or_default();
        if let Err(error) =
            promotion_record_rollback(&promotion, deploy_name, &rollback_to, &rollback_sha)
        {
            eprintln!("promotion_deploy: {error}");
            return 1;
        }
        println!("promotion_deploy: recorded rollback for {deploy_name} to {rollback_to}");
        println!("  promotion: {}", promotion.display());
        return 0;
    }
    if PROMOTION_GATE.requested(env_or_empty) {
        if version == "0.0.0" {
            eprintln!("promotion_deploy: live promote refuses version 0.0.0; set a real version");
            return 1;
        }
        if let Some(error) = PROMOTION_GATE.refusal(env_or_empty) {
            eprintln!("{error}");
            return 1;
        }
        if std::env::var("PROMOTION_REGISTRY_REQUIRED").unwrap_or_default() == "1"
            && (std::env::var("PROMOTION_REGISTRY_USER")
                .unwrap_or_default()
                .is_empty()
                || std::env::var("PROMOTION_REGISTRY_TOKEN")
                    .unwrap_or_default()
                    .is_empty())
        {
            eprintln!(
                "promotion_deploy: live promote needs PROMOTION_REGISTRY_USER plus PROMOTION_REGISTRY_TOKEN (env only, never BUILD); refusing"
            );
            return 1;
        }
        if let Err(error) = promotion_check_secret_refs(&secret_refs) {
            eprintln!("promotion_deploy: {error}");
            return 1;
        }
        let mut health = PromotionHealth::NotRequired;
        if std::env::var("PROMOTION_REQUIRE_HEALTH").unwrap_or_default() == "1" {
            let health_cmd = std::env::var("PROMOTION_HEALTH_CMD").unwrap_or_default();
            if health_cmd.is_empty() {
                eprintln!(
                    "promotion_deploy: PROMOTION_REQUIRE_HEALTH=1 needs PROMOTION_HEALTH_CMD; refusing"
                );
                return 1;
            }
            match promotion_run_health_cmd(&health_cmd) {
                Ok(0) => health = PromotionHealth::Passed,
                Ok(code) => {
                    eprintln!(
                        "promotion_deploy: health check failed (exit {code}); refusing to promote"
                    );
                    return 1;
                }
                Err(error) => {
                    eprintln!("promotion_deploy: {error}");
                    return 1;
                }
            }
        }
        match promotion_build(
            &artifact,
            outdir,
            deploy_name,
            from_env,
            to_env,
            version,
            &secret_refs,
            health,
        ) {
            Ok(promotion) => {
                eprintln!(
                    "promotion_deploy: live promotion {from_env} -> {to_env} version {version} is not implemented; gates passed and nothing was published"
                );
                eprintln!("  promotion: {}", promotion.display());
                return 1;
            }
            Err(error) => {
                eprintln!("promotion_deploy: {error}");
                return 1;
            }
        }
    }
    if std::env::var("PROMOTION_REQUIRE_HEALTH").unwrap_or_default() == "1"
        || std::env::var("PROMOTION_REGISTRY_REQUIRED").unwrap_or_default() == "1"
    {
        eprintln!(
            "promotion_deploy: local default stages only; health and registry gates apply to the live path (PROMOTION_LIVE=1); staged without running them"
        );
    }
    match promotion_build(
        &artifact,
        outdir,
        deploy_name,
        from_env,
        to_env,
        version,
        &secret_refs,
        PromotionHealth::SkippedLocal,
    ) {
        Ok(promotion) => {
            let profile = deploy_profile();
            println!(
                "promotion_deploy: staged {from_env} -> {to_env} version {version} (profile: {profile})"
            );
            println!("  promotion: {}", promotion.display());
            println!("  manifest: {}", promotion.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("promotion_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn promotion_builds_record_and_rollback() {
        let scratch = scratch_dir();
        let artifact = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&artifact, b"promotion-bytes-v1").expect("write artifact");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let promotion = promotion_build(
            &artifact,
            &outdir,
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &[],
            PromotionHealth::SkippedLocal,
        )
        .expect("promotion");
        let record = std::fs::read_to_string(promotion.join("promotion.json")).expect("record");
        let artifact_digest = dx_digest::sha256_file_hex(&artifact).expect("hex");
        assert!(record.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&record).expect("record parses"),
            serde_json::json!({
                "artifact": basename(&artifact).expect("base"),
                "artifact_sha256": artifact_digest,
                "deploy": "promotion_demo",
                "from_environment": "staging",
                "health": "skipped-local",
                "registry_auth": "env-only",
                "secret_refs": [],
                "to_environment": "production",
                "version": "1.2.3",
            })
        );
        assert!(promotion.join("would-run.txt").is_file());
        let passed_out = scratch.path().join("passed");
        std::fs::create_dir_all(&passed_out).expect("passed outdir");
        let passed = promotion_build(
            &artifact,
            &passed_out,
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &[],
            PromotionHealth::Passed,
        )
        .expect("passed promotion");
        let passed_record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(passed.join("promotion.json")).expect("passed record"),
        )
        .expect("passed record parses");
        assert_eq!(passed_record["health"], "passed");
        let manifest = std::fs::read_to_string(passed.join("would-run.txt")).expect("manifest");
        assert!(manifest.contains("health: passed"));
        assert!(!manifest.contains("local default"));
        let rollback =
            promotion_record_rollback(&promotion, "promotion_demo", "1.2.2", "").expect("rollback");
        assert!(rollback.is_file());
        assert_eq!(
            promotion_promote_command("a.tar.gz", "staging", "production", "1.2.3"),
            "promote a.tar.gz staging -> production version 1.2.3"
        );
    }

    #[test]
    fn shlex_split_handles_quotes_escapes_and_spacing() {
        assert_eq!(shlex_split("a b c"), vec!["a", "b", "c"]);
        assert_eq!(shlex_split("  a   b  "), vec!["a", "b"]);
        assert_eq!(shlex_split(""), Vec::<String>::new());
        assert_eq!(shlex_split("   "), Vec::<String>::new());
        assert_eq!(shlex_split("'one two'"), vec!["one two"]);
        assert_eq!(shlex_split("\"one two\""), vec!["one two"]);
        assert_eq!(shlex_split("a\\ b"), vec!["a b"]);
        assert_eq!(shlex_split("\"a\\\"b\""), vec!["a\"b"]);
        assert_eq!(shlex_split("'a\\b'"), vec!["a\\b"]);
        assert_eq!(shlex_split("'unterminated"), vec!["unterminated"]);
        assert_eq!(shlex_split("a\\"), vec!["a"]);
    }

    #[test]
    fn promotion_secret_refs_cover_every_ref_kind() {
        if env_test_in_child("promotion::tests::promotion_secret_refs_cover_every_ref_kind") {
            return;
        }
        let scratch = scratch_dir();
        let secret = scratch.path().join("secret.txt");
        std::fs::write(&secret, b"token").expect("write secret");
        let file_ref = format!("file:{}", secret.display());
        with_env(&[("DX_TEST_PROMOTION_SECRET", None)], || {
            let normalized = promotion_check_secret_refs(&[file_ref.clone()]).expect("file ref");
            assert_eq!(normalized, vec![file_ref.clone()]);
        });
        let missing = promotion_check_secret_refs(&["file:/nope/absent-xyz".to_owned()])
            .expect_err("missing secret file");
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        let empty_file =
            promotion_check_secret_refs(&["file:".to_owned()]).expect_err("empty path");
        assert_eq!(empty_file.kind(), io::ErrorKind::NotFound);
        let missing_tool = promotion_check_secret_refs(&["cmd:dx-no-such-tool-xyz".to_owned()])
            .expect_err("missing tool");
        assert_eq!(missing_tool.kind(), io::ErrorKind::NotFound);
        let empty_tool = promotion_check_secret_refs(&["cmd:".to_owned()]).expect_err("empty tool");
        assert_eq!(empty_tool.kind(), io::ErrorKind::NotFound);
        for bad in ["", "has space", "quote'", "dollar$", "back`tick`"] {
            let error = promotion_check_secret_refs(&[bad.to_owned()]).expect_err("invalid ref");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{bad}");
        }
        let unset = promotion_check_secret_refs(&["DX_TEST_PROMOTION_SECRET".to_owned()])
            .expect_err("unset env ref");
        assert_eq!(unset.kind(), io::ErrorKind::NotFound);
        with_env(&[("DX_TEST_PROMOTION_SECRET", Some("value"))], || {
            assert_eq!(
                promotion_check_secret_refs(&["DX_TEST_PROMOTION_SECRET".to_owned()])
                    .expect("env ref"),
                vec!["DX_TEST_PROMOTION_SECRET".to_owned()]
            );
        });
    }

    #[test]
    fn promotion_health_cmd_rejects_empty_and_missing_tools() {
        if env_test_in_child("promotion::tests::promotion_health_cmd_rejects_empty_and_missing_tools") {
            return;
        }
        let empty = promotion_run_health_cmd("   ").expect_err("empty health cmd");
        assert_eq!(empty.kind(), io::ErrorKind::InvalidInput);
        with_env(&[("PATH", Some(""))], || {
            let missing = promotion_run_health_cmd("dx-no-such-health-tool-xyz")
                .expect_err("missing health tool");
            assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        });
    }

    #[test]
    fn promotion_rollback_requires_directory_and_version() {
        let scratch = scratch_dir();
        let absent = scratch.path().join("absent");
        let missing_dir = promotion_record_rollback(&absent, "demo", "1.0.1", "")
            .expect_err("missing promotion dir");
        assert_eq!(missing_dir.kind(), io::ErrorKind::InvalidInput);
        let artifact = scratch.path().join("demo.tar.gz");
        std::fs::write(&artifact, b"bytes").expect("write artifact");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let promotion = promotion_build(
            &artifact,
            &outdir,
            "demo",
            "staging",
            "production",
            "1.0.2",
            &[],
            PromotionHealth::SkippedLocal,
        )
        .expect("promotion");
        let missing_version =
            promotion_record_rollback(&promotion, "demo", "", "").expect_err("missing version");
        assert_eq!(missing_version.kind(), io::ErrorKind::InvalidInput);
        let pinned = promotion_record_rollback(&promotion, "demo", "1.0.1", "abc123")
            .expect("rollback record");
        let body = std::fs::read_to_string(&pinned).expect("read rollback");
        assert!(body.contains("rollback demo to 1.0.1"));
        assert!(body.contains("expected-sha256: abc123"));
    }

    #[cfg(unix)]
    #[test]
    fn promotion_live_records_health_and_refuses() {
        use std::os::unix::fs::PermissionsExt;
        if env_test_in_child("promotion::tests::promotion_live_records_health_and_refuses") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let tarball = runfiles.join("release_demo.tar.gz");
        std::fs::write(
            &tarball,
            archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar"),
        )
        .expect("write tarball");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let script = bin.join("dx_fake_health");
        std::fs::write(&script, "#!/bin/sh\nexit 0\n").expect("write health");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        for (require, expected) in [(Some("1"), "passed"), (None, "not-required")] {
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("BUILD_WORKSPACE_DIRECTORY", None),
                    ("PROMOTION_LIVE", Some("1")),
                    ("PROMOTION_APPROVED", Some("1")),
                    ("PROMOTION_REQUIRE_HEALTH", require),
                    ("PROMOTION_HEALTH_CMD", Some("dx_fake_health")),
                    ("PATH", Some(bin.to_string_lossy().as_ref())),
                    ("PROMOTION_SECRET_REFS", None),
                    ("PROMOTION_REGISTRY_REQUIRED", None),
                    ("PROMOTION_ROLLBACK", None),
                ],
                || {
                    let outdir = tempfile_stage_dir("dx-deploy-promotion-live-").expect("outdir");
                    let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
                    assert_eq!(
                        promotion_main(
                            "release_demo.tar.gz",
                            "promotion_demo",
                            "staging",
                            "production",
                            "1.2.3",
                            &argv
                        ),
                        1
                    );
                    let record: serde_json::Value = serde_json::from_str(
                        &std::fs::read_to_string(
                            outdir.join("promotion_demo-promotion/promotion.json"),
                        )
                        .expect("record"),
                    )
                    .expect("record parses");
                    assert_eq!(record["health"], expected);
                    std::fs::remove_dir_all(&outdir).expect("cleanup");
                },
            );
        }
    }

    #[test]
    fn promotion_rejects_unusable_environments_and_names() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let artifact = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&artifact, b"promotion-bytes-v1").expect("write artifact");
        let build = |deploy, from, to, version, src: &Path| {
            promotion_build(
                src,
                &outdir,
                deploy,
                from,
                to,
                version,
                &[],
                PromotionHealth::SkippedLocal,
            )
        };
        let baseline = build(
            "promotion_demo",
            "staging",
            "production",
            "1.2.3",
            &artifact,
        )
        .expect("baseline promotion");
        assert!(baseline.join("promotion.json").is_file());
        let absent = scratch.path().join("absent.tar.gz");
        let error = build("promotion_demo", "staging", "production", "1.2.3", &absent)
            .expect_err("absent artifact");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let rejects = [
            (
                "deploy",
                build("", "staging", "production", "1.2.3", &artifact),
            ),
            (
                "from",
                build("promotion_demo", "", "production", "1.2.3", &artifact),
            ),
            (
                "to",
                build("promotion_demo", "staging", "", "1.2.3", &artifact),
            ),
            (
                "same",
                build("promotion_demo", "prod", "prod", "1.2.3", &artifact),
            ),
            (
                "version",
                build("promotion_demo", "staging", "production", "", &artifact),
            ),
        ];
        for (field, result) in rejects {
            let error = result.expect_err("unusable promotion field");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{field}: {error}"
            );
        }
    }
}
