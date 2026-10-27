#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

mod archive;
mod cloudflare;
mod crates;
mod github;
pub mod http;
mod maven;
mod nuget;
mod oci;
mod pypi;
mod run;
mod stage;
#[cfg(test)]
pub(crate) mod test_support;

pub use archive::*;
pub use cloudflare::*;
pub use crates::*;
pub use github::*;
pub use maven::*;
pub use nuget::*;
pub use oci::*;
pub use pypi::*;
pub use run::{deploy_profile, DeployRun};
pub use stage::*;

use run::{
    env_or_empty, scrubbed_command, OwnerGate, PUBLISH_CHILD_ENV_KEEP,
};

use std::ffi::OsString;
use std::io;
use std::path::Path;

/// Maps npm input files to package-relative paths under the package.json directory.
pub fn npm_package_paths(
    srcs: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    package_relative_paths(srcs, "package.json", "npm pack")
}

/// Octopus drop inputs.
pub struct OctopusDrop<'a> {
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to: &'a [String],
    pub space: &'a str,
    pub server: &'a str,
}

/// Builds the argv for `octo push`.
pub fn octopus_push_args(package: &Path, spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "push".into(),
        "--package".into(),
        package.as_os_str().to_owned(),
        "--server".into(),
        spec.server.into(),
    ];
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Builds the argv for `octo create-release`.
pub fn octopus_release_args(spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "create-release".into(),
        "--project".into(),
        spec.project.into(),
        "--channel".into(),
        spec.channel.into(),
        "--version".into(),
        spec.version.into(),
    ];
    for env in spec.deploy_to {
        args.push("--deployTo".into());
        args.push(env.into());
    }
    args.push("--server".into());
    args.push(spec.server.into());
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Renders an octo argv as one display line.
fn octo_line(args: &[OsString]) -> String {
    let words: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    format!("octo {}", words.join(" "))
}

/// Octopus launcher inputs.
pub struct OctopusLaunch<'a> {
    pub package_rloc: &'a str,
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to_raw: &'a str,
    pub space_default: &'a str,
    pub url_default: &'a str,
}

/// Builds one Octopus package drop.
pub fn octopus_build_drop(
    package_src: &Path,
    outdir: &Path,
    spec: &OctopusDrop<'_>,
) -> io::Result<std::path::PathBuf> {
    let src_str = package_src.to_string_lossy();
    if !src_str.ends_with(".tar.gz") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "octopus drop: want exactly one .tar.gz source (archive_deploy output), got '{src_str}'"
            ),
        ));
    }
    if spec.deploy_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty deploy name",
        ));
    }
    if spec.project.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty project",
        ));
    }
    if spec.channel.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty channel",
        ));
    }
    if spec.version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty version",
        ));
    }
    let drop = outdir.join(spec.deploy_name.to_owned() + "-drop");
    std::fs::create_dir_all(&drop)?;
    let base = basename(package_src)?;
    let dest = drop.join(&base);
    let digest = copy_verified(package_src, &dest)?;
    let manifest = format!(
        "# would-run manifest for octopus_deploy (local default publishes nothing)\n{}\n{}\npackage-sha256: {digest}  {base}\n",
        octo_line(&octopus_push_args(Path::new(&base), spec)),
        octo_line(&octopus_release_args(spec)),
    );
    std::fs::write(drop.join("would-run.txt"), manifest.as_bytes())?;
    Ok(drop)
}

/// Owner approval gate for octopus_deploy.
const OCTOPUS_GATE: OwnerGate = OwnerGate {
    tool: "octopus_deploy",
    action: "push",
    credentials: &["OCTOPUS_API_KEY"],
    approved: "OCTOPUS_PUBLISH_APPROVED",
    live: "OCTOPUS_PUBLISH_LIVE",
};

/// Runs one octo step with the key in env only; true on success.
fn octo_run(octo: &Path, args: &[OsString], api_key: &str, step: &str) -> bool {
    let mut cmd = scrubbed_command(
        octo,
        &[PUBLISH_CHILD_ENV_KEEP],
        &[("OCTOPUS_CLI_API_KEY", api_key)],
    );
    cmd.args(args);
    match cmd.status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("octopus_deploy: octo {step} failed with {status}");
            false
        }
        Err(error) => {
            eprintln!("octopus_deploy: octo {step}: {error}");
            false
        }
    }
}

/// Pushes the package, then creates the release and queues each deployment.
pub fn octopus_live_publish(
    octo: &Path,
    package: &Path,
    spec: &OctopusDrop<'_>,
    api_key: &str,
) -> i32 {
    let name = package
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !octo_run(octo, &octopus_push_args(package, spec), api_key, "push") {
        return 1;
    }
    if !octo_run(octo, &octopus_release_args(spec), api_key, "create-release") {
        eprintln!(
            "octopus_deploy: package {name} is already pushed; a rerun will fail on push (FailIfExists)"
        );
        return 1;
    }
    println!("octopus_deploy: pushed package {name}");
    println!(
        "octopus_deploy: created release {} {} on channel {}",
        spec.project, spec.version, spec.channel
    );
    for env in spec.deploy_to {
        println!(
            "octopus_deploy: queued deploy of {} {} to {env}",
            spec.project, spec.version
        );
    }
    0
}

/// Runs the Octopus deploy launcher.
pub fn octopus_main(spec: &OctopusLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "octopus_deploy",
        "the drop is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(package) = run.resolve(spec.package_rloc) else {
        return 1;
    };
    let space = std::env::var("OCTOPUS_SPACE").unwrap_or_else(|_| spec.space_default.to_owned());
    let server = std::env::var("OCTOPUS_URL").unwrap_or_else(|_| spec.url_default.to_owned());
    let deploy_to: Vec<String> = spec
        .deploy_to_raw
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    let drop_spec = OctopusDrop {
        deploy_name: spec.deploy_name,
        project: spec.project,
        channel: spec.channel,
        version: spec.version,
        deploy_to: &deploy_to,
        space: &space,
        server: &server,
    };
    if OCTOPUS_GATE.requested(env_or_empty) {
        if spec.version == "0.0.0" {
            eprintln!("octopus_deploy: live push refuses version 0.0.0; set a real version");
            return 1;
        }
        if server.is_empty() || server == "https://octopus.example.invalid" {
            eprintln!(
                "octopus_deploy: live push needs OCTOPUS_URL plus explicit owner approval (OCTOPUS_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        let secrets = match OCTOPUS_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(octo) = which_on_path("octo") else {
            eprintln!("octopus_deploy: 'octo' CLI not found on PATH");
            return 1;
        };
        return octopus_live_publish(&octo, &package, &drop_spec, secrets.get("OCTOPUS_API_KEY"));
    }
    match octopus_build_drop(&package, outdir, &drop_spec) {
        Ok(drop) => {
            let profile = deploy_profile();
            println!(
                "octopus_deploy: staged {} {} (profile: {profile})",
                spec.project, spec.version
            );
            println!("  drop: {}", drop.display());
            println!("  manifest: {}", drop.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("octopus_deploy: {error}");
            1
        }
    }
}

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
const PROMOTION_GATE: OwnerGate = OwnerGate {
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

/// Creates one deterministic npm pack.
#[allow(clippy::too_many_arguments)]
pub fn npm_create_pack(
    package: &str,
    tag: &str,
    registry: &str,
    access: &str,
    provenance: &str,
    tgz_out: &Path,
    feed_out: &Path,
    srcs: &[std::path::PathBuf],
) -> io::Result<String> {
    if let Some(error) = npm_access_error(package, access)
        .or_else(|| npm_provenance_error(access, registry, provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, error));
    }
    let mut members: std::collections::BTreeMap<String, (Vec<u8>, u32)> =
        std::collections::BTreeMap::new();
    for (src, rel) in npm_package_paths(srcs)? {
        let data = std::fs::read(&src)?;
        let member = format!("package/{rel}");
        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let bits = std::fs::metadata(&src)?.permissions().mode();
                if bits & 0o111 != 0 {
                    0o755
                } else {
                    0o644
                }
            }
            #[cfg(not(unix))]
            {
                0o644
            }
        };
        members.insert(member, (data, mode));
    }
    let tar = tar_archive(
        members
            .iter()
            .map(|(member, (data, mode))| (member.as_str(), data.as_slice(), *mode)),
    )?;
    let gz = gzip_compress(&tar)?;
    if let Some(parent) = tgz_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(tgz_out, &gz)?;
    let digest = dx_digest::sha256_file_hex(tgz_out)?;
    let names: Vec<String> = members.keys().cloned().collect();
    let tgz_base = basename(tgz_out)?;
    let feed = render_json(&serde_json::json!({
        "access": access,
        "files": names,
        "name": package,
        "provenance": provenance,
        "registry": registry,
        "sha256": digest,
        "tag": tag,
        "tarball": tgz_base,
    }))?;
    if let Some(parent) = feed_out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(feed_out, feed.as_bytes())?;
    Ok(digest)
}

/// Rejects an npm access mode the package name cannot publish with.
pub fn npm_access_error(package: &str, access: &str) -> Option<String> {
    let scoped = package.starts_with('@');
    if scoped
        && !package[1..].split_once('/').is_some_and(|(scope, name)| {
            !scope.is_empty() && !name.is_empty() && !name.contains('/')
        })
    {
        return Some(format!(
            "npm_deploy: invalid package '{package}': a scoped name is @scope/name"
        ));
    }
    if !scoped && package.contains('/') {
        return Some(format!(
            "npm_deploy: invalid package '{package}': only scoped names contain '/'"
        ));
    }
    match access {
        "public" => None,
        "restricted" if scoped => None,
        "restricted" => Some(format!(
            "npm_deploy: access 'restricted' needs a scoped package, got unscoped '{package}'"
        )),
        _ => Some(format!(
            "npm_deploy: invalid access '{access}': want 'public' or 'restricted'"
        )),
    }
}

fn npm_registry_host(registry: &str) -> &str {
    registry
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
}

/// Rejects a provenance policy the access mode or registry cannot honor.
pub fn npm_provenance_error(access: &str, registry: &str, provenance: &str) -> Option<String> {
    match provenance {
        "none" => None,
        "required" if access != "public" => Some(format!(
            "npm_deploy: provenance 'required' is not supported with access '{access}'; \
             use access 'public' or provenance 'none'"
        )),
        "required" if npm_registry_host(registry) != "registry.npmjs.org" => Some(format!(
            "npm_deploy: provenance 'required' is not supported by registry '{registry}'; \
             use https://registry.npmjs.org or provenance 'none'"
        )),
        "required" => None,
        _ => Some(format!(
            "npm_deploy: invalid provenance '{provenance}': want 'required' or 'none'"
        )),
    }
}

/// Rejects a required provenance policy when no signing identity is available.
pub fn npm_provenance_unavailable(provenance: &str) -> Option<String> {
    if provenance != "required" {
        return None;
    }
    let present = |key: &str| std::env::var_os(key).is_some_and(|value| !value.is_empty());
    if present("ACTIONS_ID_TOKEN_REQUEST_URL") || present("SIGSTORE_ID_TOKEN") {
        return None;
    }
    Some(
        "npm_deploy: provenance 'required' but no CI identity token is available \
         (ACTIONS_ID_TOKEN_REQUEST_URL or SIGSTORE_ID_TOKEN); publishing nothing"
            .to_owned(),
    )
}

/// Builds the npm publish arguments for one staged tarball.
pub fn npm_publish_args(
    tarball: &Path,
    tag: &str,
    access: &str,
    registry: &str,
    provenance: &str,
) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "publish".into(),
        tarball.into(),
        "--tag".into(),
        tag.into(),
        "--access".into(),
        access.into(),
    ];
    args.push(if provenance == "required" {
        "--provenance".into()
    } else {
        "--provenance=false".into()
    });
    args.push("--registry".into());
    args.push(registry.into());
    args
}

/// Lists one gzip tar member table.
pub fn npm_tar_members(tgz_path: &Path) -> io::Result<Vec<String>> {
    let file = std::fs::File::open(tgz_path)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut members = Vec::new();
    for entry in archive.entries()? {
        let entry = entry?;
        members.push(String::from_utf8_lossy(&entry.path_bytes()).into_owned());
    }
    Ok(members)
}

/// Renders one npmrc auth line.
pub fn npm_npmrc_line(host: &str, token: &str) -> String {
    format!("//{host}/:_authToken={token}\n")
}

/// Writes one owner-only npmrc in a private staged dir.
pub fn npm_write_npmrc(host: &str, token: &str) -> io::Result<std::path::PathBuf> {
    let dir = tempfile_stage_dir("npm-publish-")?;
    match write_secure_file(&dir, ".npmrc", &npm_npmrc_line(host, token)) {
        Ok(path) => Ok(path),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(error)
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct NpmFeed {
    name: String,
    tag: String,
    registry: String,
    access: String,
    provenance: String,
    #[allow(dead_code)]
    tarball: String,
    sha256: String,
    files: Vec<String>,
}

fn parse_npm_feed(body: &str) -> io::Result<NpmFeed> {
    serde_json::from_str(body).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("feed JSON is invalid: {error}"),
        )
    })
}

/// Verifies one npm pack pair.
pub fn npm_verify_pack(
    tgz_path: &Path,
    feed_path: &Path,
) -> io::Result<(String, String, String, String, String)> {
    let body = std::fs::read_to_string(feed_path)?;
    let NpmFeed {
        name,
        tag,
        registry,
        access,
        provenance,
        sha256: expected,
        files,
        ..
    } = parse_npm_feed(&body)?;
    let actual = dx_digest::sha256_file_hex(tgz_path)?;
    if expected != actual {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "checksum mismatch for {} (expected {expected}, actual {actual})",
                tgz_path.display(),
            ),
        ));
    }
    let members = npm_tar_members(tgz_path)?;
    if members != files {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "pack members mismatch for {}: {members:?}",
                tgz_path.display()
            ),
        ));
    }
    if let Some(error) = npm_access_error(&name, &access)
        .or_else(|| npm_provenance_error(&access, &registry, &provenance))
    {
        return Err(io::Error::new(io::ErrorKind::InvalidData, error));
    }
    Ok((name, tag, registry, access, provenance))
}

/// Resolves the npm pack pair from explicit runfiles keys.
pub fn npm_find_pack_inputs(
    feed_rloc: &str,
    tarball_rloc: &str,
) -> io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let feed_path = resolve_runfile(feed_rloc)?;
    let tgz_path = resolve_runfile(tarball_rloc)?;
    Ok((feed_path, tgz_path))
}

/// Runs the npm pack tool.
pub fn npm_pack_main(argv: &[String]) -> i32 {
    if argv.len() < 9 {
        eprintln!(
            "usage: npm_pack <package> <tag> <registry> <access> <provenance> <tgz_out> <feed_out> <src...>"
        );
        return 1;
    }
    let package = argv[1].clone();
    let tag = argv[2].clone();
    let registry = argv[3].clone();
    let access = argv[4].clone();
    let provenance = argv[5].clone();
    let tgz_out = std::path::PathBuf::from(&argv[6]);
    let feed_out = std::path::PathBuf::from(&argv[7]);
    let srcs: Vec<std::path::PathBuf> = argv[8..].iter().map(std::path::PathBuf::from).collect();
    match npm_create_pack(
        &package,
        &tag,
        &registry,
        &access,
        &provenance,
        &tgz_out,
        &feed_out,
        &srcs,
    ) {
        Ok(_) => 0,
        Err(error) => {
            eprintln!("npm_pack: {error}");
            1
        }
    }
}

/// Extra env names `npm` keeps: CA and node settings plus the CI identity for provenance.
const NPM_CHILD_ENV_EXTRA: &[&str] = &[
    "CI",
    "NODE_EXTRA_CA_CERTS",
    "NODE_OPTIONS",
    "ComSpec",
    "GITHUB_ACTIONS",
    "GITHUB_WORKFLOW_REF",
    "GITHUB_REPOSITORY",
    "GITHUB_REPOSITORY_ID",
    "GITHUB_REPOSITORY_OWNER_ID",
    "GITHUB_SERVER_URL",
    "GITHUB_EVENT_NAME",
    "GITHUB_REF",
    "GITHUB_SHA",
    "GITHUB_RUN_ID",
    "GITHUB_RUN_ATTEMPT",
    "RUNNER_ENVIRONMENT",
    "ACTIONS_ID_TOKEN_REQUEST_URL",
    "ACTIONS_ID_TOKEN_REQUEST_TOKEN",
    "GITLAB_CI",
    "SIGSTORE_ID_TOKEN",
    "CI_",
    "GITLAB_",
];

/// Runs npm publish with a temporary owner-only npmrc, in a cleared env.
pub fn npm_run_publish(npm: &Path, host: &str, token: &str, args: &[std::ffi::OsString]) -> i32 {
    let npmrc = match npm_write_npmrc(host, token) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let status = scrubbed_command(npm, &[PUBLISH_CHILD_ENV_KEEP, NPM_CHILD_ENV_EXTRA], &[])
        .args(args)
        .arg("--userconfig")
        .arg(&npmrc)
        .status();
    if let Some(dir) = npmrc.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
    match status {
        Ok(status) if status.success() => 0,
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            1
        }
    }
}

/// Owner approval gate for npm_deploy.
const NPM_GATE: OwnerGate = OwnerGate {
    tool: "npm_deploy",
    action: "publish",
    credentials: &["NPM_TOKEN"],
    approved: "NPM_PUBLISH_APPROVED",
    live: "NPM_PUBLISH_LIVE",
};

/// Runs the npm deploy launcher.
pub fn npm_deploy_main(argv: &[String], feed_rloc: &str, tarball_rloc: &str) -> i32 {
    if std::env::var_os("NPM_PUBLISH_DRY_RUN").is_some() {
        eprintln!(
            "npm_deploy: NPM_PUBLISH_DRY_RUN is not supported; publishing needs NPM_PUBLISH_LIVE=1"
        );
        return 1;
    }
    let Some(run) = DeployRun::start(
        "npm_deploy",
        "the package is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let (feed_path, tgz_path) = match npm_find_pack_inputs(feed_rloc, tarball_rloc) {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let (name, tag, registry, access, provenance) = match npm_verify_pack(&tgz_path, &feed_path) {
        Ok(triple) => triple,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let tgz_base = match basename(&tgz_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let feed_base = match basename(&feed_path) {
        Ok(base) => base,
        Err(error) => {
            eprintln!("npm_deploy: {error}");
            return 1;
        }
    };
    let stem = tgz_base
        .strip_suffix(".tgz")
        .unwrap_or(&tgz_base)
        .to_owned();
    let feed_dir = outdir.join(stem.clone() + "-feed");
    if std::fs::create_dir_all(&feed_dir).is_err() {
        eprintln!("npm_deploy: cannot create {}", feed_dir.display());
        return 1;
    }
    for (src, base) in [(&tgz_path, &tgz_base), (&feed_path, &feed_base)] {
        if dx_atomic_fs::copy_atomic(src, &outdir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
        if dx_atomic_fs::copy_atomic(src, &feed_dir.join(base)).is_err() {
            eprintln!("npm_deploy: cannot copy {}", src.display());
            return 1;
        }
    }
    let profile = deploy_profile();
    println!("npm_deploy: released {name} (tag {tag}, access {access}, profile: {profile})");
    println!("  registry: {registry}");
    println!("  provenance: {provenance}");
    println!("  tarball: {}", outdir.join(&tgz_base).display());
    println!("  feed: {}", feed_dir.display());
    if NPM_GATE.requested(env_or_empty) {
        let secrets = match NPM_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        if let Some(error) = npm_provenance_unavailable(&provenance) {
            eprintln!("{error}");
            return 1;
        }
        let staged_tgz = outdir.join(&tgz_base);
        let Some(npm) = which_on_path("npm") else {
            eprintln!("npm_deploy: 'npm' CLI not found on PATH");
            return 1;
        };
        let host = npm_registry_host(&registry);
        if host.is_empty() {
            eprintln!("npm_deploy: invalid registry '{registry}'");
            return 1;
        }
        return npm_run_publish(
            &npm,
            host,
            secrets.get("NPM_TOKEN"),
            &npm_publish_args(&staged_tgz, &tag, &access, &registry, &provenance),
        );
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn octopus_drop_writes_manifest() {
        let scratch = scratch_dir();
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package-bytes-v1").expect("write package");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let drop = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "0.0.0",
                deploy_to: &["Production".to_owned()],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect("drop");
        assert!(drop.join("release_demo.tar.gz").is_file());
        let manifest = std::fs::read_to_string(drop.join("would-run.txt")).expect("manifest");
        let spec = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "0.0.0",
            deploy_to: &["Production".to_owned()],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let lines: Vec<&str> = manifest.lines().collect();
        assert_eq!(
            lines[1],
            "octo push --package release_demo.tar.gz --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[2],
            "octo create-release --project octopus_demo --channel Default --version 0.0.0 --deployTo Production --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[1],
            octo_line(&octopus_push_args(Path::new("release_demo.tar.gz"), &spec))
        );
        assert_eq!(lines[2], octo_line(&octopus_release_args(&spec)));
    }

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
    fn npm_pack_and_verify_round_trip() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let digest = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(digest, dx_digest::sha256_file_hex(&tgz).expect("hex"));
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, vec!["package/package.json".to_owned()]);
        assert_eq!(
            npm_npmrc_line("registry.npmjs.org", "tok"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        let body = std::fs::read_to_string(&feed).expect("feed");
        assert!(body.ends_with('\n'));
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&body).expect("feed parses"),
            serde_json::json!({
                "access": "public",
                "files": ["package/package.json"],
                "name": "npm-demo",
                "provenance": "required",
                "registry": "https://registry.npmjs.org",
                "sha256": digest,
                "tag": "latest",
                "tarball": "npm_demo.tgz",
            })
        );
    }

    #[test]
    fn npm_tar_members_rejects_a_bad_header_checksum() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n")
            .expect("write src");
        let good = scratch.path().join("good.tgz");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &good,
            &scratch.path().join("good.feed.json"),
            std::slice::from_ref(&src),
        )
        .expect("pack");
        assert_eq!(npm_tar_members(&good).expect("members").len(), 1);
        let mut tar = Vec::new();
        use std::io::Read as _;
        flate2::read::GzDecoder::new(std::fs::File::open(&good).expect("open tgz"))
            .read_to_end(&mut tar)
            .expect("decode");
        tar[100] ^= 1;
        let bad = scratch.path().join("bad.tgz");
        std::fs::write(&bad, gzip_compress(&tar).expect("gz")).expect("write bad");
        assert!(npm_tar_members(&bad).is_err());
    }

    fn nested_npm_package(root: &Path) -> Vec<std::path::PathBuf> {
        vec![
            write_crate_file(
                root,
                "package.json",
                "{\"name\": \"npm-nested\", \"version\": \"1.0.0\", \"main\": \"dist/index.js\", \"types\": \"dist/index.d.ts\", \"exports\": {\".\": \"./dist/index.js\", \"./util\": \"./dist/util/index.js\"}, \"bin\": {\"npm-nested\": \"bin/cli.js\"}}\n",
            ),
            write_crate_file(
                root,
                "dist/index.js",
                "const util = require('./util');\nmodule.exports = { value: 41 + util.one };\n",
            ),
            write_crate_file(root, "dist/index.d.ts", "export const value: number;\n"),
            write_crate_file(root, "dist/util/index.js", "module.exports = { one: 1 };\n"),
            write_crate_file(
                root,
                "bin/cli.js",
                "#!/usr/bin/env node\nconsole.log(require('../dist').value);\n",
            ),
        ]
    }

    fn pack_nested_npm(scratch: &Path) -> std::path::PathBuf {
        let srcs = nested_npm_package(&scratch.join("in/pkg"));
        let tgz = scratch.join("nested.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.join("nested.feed.json"),
            &srcs,
        )
        .expect("pack");
        tgz
    }

    #[test]
    fn npm_pack_preserves_package_paths() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        assert_eq!(
            npm_tar_members(&tgz).expect("members"),
            vec![
                "package/bin/cli.js".to_owned(),
                "package/dist/index.d.ts".to_owned(),
                "package/dist/index.js".to_owned(),
                "package/dist/util/index.js".to_owned(),
                "package/package.json".to_owned(),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_pack_keeps_executable_modes() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = scratch_dir();
        let srcs = nested_npm_package(&scratch.path().join("in/pkg"));
        let cli = &srcs[4];
        std::fs::set_permissions(cli, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let tgz = scratch.path().join("modes.tgz");
        npm_create_pack(
            "npm-nested",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &scratch.path().join("modes.feed.json"),
            &srcs,
        )
        .expect("pack");
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(
            std::fs::File::open(&tgz).expect("open"),
        ));
        let mut modes = std::collections::BTreeMap::new();
        for entry in archive.entries().expect("entries") {
            let entry = entry.expect("entry");
            let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
            modes.insert(name, entry.header().mode().expect("mode"));
        }
        assert_eq!(modes["package/bin/cli.js"], 0o755);
        assert_eq!(modes["package/dist/index.js"], 0o644);
    }

    #[test]
    fn npm_pack_allows_equal_basenames_in_different_directories() {
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let members = npm_tar_members(&tgz).expect("members");
        let indexes = members.iter().filter(|m| m.ends_with("/index.js")).count();
        assert_eq!(indexes, 2);
    }

    #[test]
    fn npm_package_paths_reject_collisions_and_escapes() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let manifest = write_crate_file(&root, "package.json", "{}");
        let outside = write_crate_file(scratch.path(), "other/index.js", "");
        let error = npm_package_paths(&[manifest.clone(), outside]).expect_err("outside");
        assert!(
            error.to_string().contains("outside the package root"),
            "{error}"
        );
        write_crate_file(&root, "index.js", "");
        let error = npm_package_paths(&[manifest.clone(), root.join("src/../index.js")])
            .expect_err("dotdot");
        assert!(error.to_string().contains("unsafe package path"), "{error}");
        let error = npm_package_paths(&[
            manifest.clone(),
            root.join("index.js"),
            root.join("index.js"),
        ])
        .expect_err("dup");
        assert!(error.to_string().contains("path collision"), "{error}");
        let error = npm_package_paths(&[manifest.clone(), root.join("src"), root.join("src/a.js")])
            .expect_err("file+dir");
        assert!(
            error.to_string().contains("both a file and a directory"),
            "{error}"
        );
        let error = npm_package_paths(&[root.join("index.js")]).expect_err("no manifest");
        assert!(error.to_string().contains("no package.json"), "{error}");
        let error = npm_package_paths(&[
            write_crate_file(&root, "a/package.json", "{}"),
            write_crate_file(&root, "b/package.json", "{}"),
        ])
        .expect_err("ambiguous");
        assert!(
            error.to_string().contains("ambiguous package root"),
            "{error}"
        );
        assert!(npm_package_paths(&[]).is_err());
    }

    #[test]
    fn npm_pack_installs_into_a_local_consumer() {
        let ok = |program: &str| {
            std::process::Command::new(program)
                .arg("--version")
                .output()
                .map(|out| out.status.success())
                .unwrap_or(false)
        };
        if !ok("node") || !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let tgz = pack_nested_npm(scratch.path());
        let consumer = scratch.path().join("consumer");
        write_crate_file(
            &consumer,
            "package.json",
            "{\"name\": \"consumer\", \"version\": \"0.0.0\", \"private\": true}\n",
        );
        let install = std::process::Command::new("npm")
            .args(["install", "--offline", "--no-audit", "--no-fund"])
            .arg(&tgz)
            .env("npm_config_cache", scratch.path().join("npm-cache"))
            .current_dir(&consumer)
            .output()
            .expect("npm install");
        assert!(
            install.status.success(),
            "{}",
            String::from_utf8_lossy(&install.stderr)
        );
        let run = std::process::Command::new("node")
            .args([
                "-e",
                "console.log(require('npm-nested').value + require('npm-nested/util').one)",
            ])
            .current_dir(&consumer)
            .output()
            .expect("node");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "43");
        assert!(consumer
            .join("node_modules/npm-nested/dist/index.d.ts")
            .is_file());
        assert!(consumer.join("node_modules/.bin/npm-nested").exists());
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
    fn npm_feed_parser_reads_escapes_brackets_and_unicode() {
        let body = r#"{ "tarball":"a.tgz", "tag":"t", "sha256":"d", "registry":"r", "access":"public", "provenance":"none",
            "name": "a\"b\\c\nd", "extra": [1, {"x": "]"}],
            "files": ["package/[id].js", "package/]x[.js", "y\\z", "w\"q", "\u00e6\u0041", "æ"] }"#;
        let feed = parse_npm_feed(body).expect("feed");
        assert_eq!(feed.name, "a\"b\\c\nd");
        assert_eq!(
            feed.files,
            [
                "package/[id].js",
                "package/]x[.js",
                "y\\z",
                "w\"q",
                "\u{e6}A",
                "\u{e6}"
            ]
        );
    }

    #[test]
    fn npm_feed_parser_rejects_bad_documents() {
        let good = r#"{"access": "public", "files": [], "name": "n", "provenance": "none", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a.tgz"}"#;
        parse_npm_feed(good).expect("good");
        let bad = [
            "",
            "[]",
            r#"{"name": "#,
            r#"{"files": ["a"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t"}"#,
            r#"{"files": [], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": 7, "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [1], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": 7, "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "name": "n", "name": "m", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
            r#"{"files": [], "files": ["x"], "name": "n", "registry": "r", "sha256": "d", "tag": "t", "tarball": "a"}"#,
        ];
        for body in bad {
            let error = parse_npm_feed(body).expect_err(body);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{body}");
        }
    }

    #[test]
    fn octopus_args_builders_cover_optional_segments() {
        let envs = ["Production".to_owned(), "Staging".to_owned()];
        let spec = |deploy_to: &'static [String], space: &'static str| OctopusDrop {
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to,
            space,
            server: "https://octo.invalid",
        };
        let envs: &'static [String] = Box::leak(Box::new(envs));
        let lines = |args: Vec<OsString>| octo_line(&args);
        let pkg = Path::new("demo-1.0.0.tar.gz");
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], ""))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid"
        );
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], "Spaces"))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid --space Spaces"
        );
        assert_eq!(
            lines(octopus_release_args(&spec(&[], ""))),
            "octo create-release --project demo --channel Default --version 1.0.0 --server https://octo.invalid"
        );
        let release = lines(octopus_release_args(&spec(envs, "Spaces")));
        assert_eq!(
            release,
            "octo create-release --project demo --channel Default --version 1.0.0 --deployTo Production --deployTo Staging --server https://octo.invalid --space Spaces"
        );
        let push = lines(octopus_push_args(pkg, &spec(envs, "Spaces")));
        for line in [&push, &release] {
            assert!(!line.contains("--deploy-to") && !line.contains("--apiKey"));
            assert!(!line.to_lowercase().contains("key"));
        }
        assert!(!release.contains("--package"));
    }

    #[test]
    fn command_formatters_cover_optional_segments() {
        assert_eq!(
            promotion_rollback_command("demo", "1.0.1"),
            "rollback demo to 1.0.1 (restore the pinned artifact for that version)"
        );
        assert_eq!(
            github_release_command("v1", &[]),
            "gh release create v1 --draft --verify-tag"
        );
    }

    #[test]
    fn npm_find_pack_inputs_reads_manifest_entries() {
        if env_test_in_child("tests::npm_find_pack_inputs_reads_manifest_entries") {
            return;
        }
        let scratch = scratch_dir();
        let tgz = scratch.path().join("npm_demo.tgz");
        std::fs::write(&tgz, b"tarball-bytes").expect("write tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        std::fs::write(
            &feed,
            b"{\"name\": \"npm-demo\", \"tarball\": \"npm_demo.tgz\"}\n",
        )
        .expect("write feed");
        let manifest = scratch.path().join("manifest.txt");
        std::fs::write(
            &manifest,
            format!(
                "_main/npm/nested/npm_demo.feed.json {}\n_main/npm/nested/npm_demo.tgz {}\n",
                feed.display(),
                tgz.display()
            ),
        )
        .expect("write manifest");
        with_env(
            &[
                ("RUNFILES_DIR", None),
                (
                    "RUNFILES_MANIFEST_FILE",
                    Some(manifest.to_string_lossy().as_ref()),
                ),
            ],
            || {
                let (found_feed, found_tgz) = npm_find_pack_inputs(
                    "_main/npm/nested/npm_demo.feed.json",
                    "_main/npm/nested/npm_demo.tgz",
                )
                .expect("pack pair");
                assert_eq!(found_feed, feed);
                assert_eq!(found_tgz, tgz);
                let error = npm_find_pack_inputs("npm_demo.feed.json", "npm_demo.tgz")
                    .expect_err("basename is not a key");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
                assert!(error.to_string().contains("npm_demo.feed.json"));
            },
        );
        let empty_dir = scratch.path().join("empty-runfiles");
        std::fs::create_dir_all(&empty_dir).expect("empty runfiles");
        with_env(
            &[
                ("RUNFILES_DIR", Some(empty_dir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let error = npm_find_pack_inputs("_main/a.feed.json", "_main/a.tgz")
                    .expect_err("no feed under runfiles");
                assert_eq!(error.kind(), io::ErrorKind::NotFound);
            },
        );
    }

    #[test]
    fn npmrc_written_with_owner_only_permissions() {
        let path = npm_write_npmrc("registry.npmjs.org", "tok").expect("npmrc");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read npmrc"),
            "//registry.npmjs.org/:_authToken=tok\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "npmrc must stay owner-only");
        }
        let parent = path.parent().expect("npmrc parent");
        assert_ne!(parent, std::env::temp_dir().as_path());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(parent)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700, "npmrc dir must stay owner-only");
        }
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn promotion_secret_refs_cover_every_ref_kind() {
        if env_test_in_child("tests::promotion_secret_refs_cover_every_ref_kind") {
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
        if env_test_in_child("tests::promotion_health_cmd_rejects_empty_and_missing_tools") {
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

    #[test]
    fn npm_access_error_covers_scope_and_mode() {
        assert!(npm_access_error("npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "public").is_none());
        assert!(npm_access_error("@s/npm-demo", "restricted").is_none());
        let unscoped = npm_access_error("npm-demo", "restricted").expect("unscoped");
        assert!(unscoped.contains("needs a scoped package"), "{unscoped}");
        let unknown = npm_access_error("npm-demo", "private").expect("unknown");
        assert!(unknown.contains("invalid access"), "{unknown}");
        for bad in ["@scope", "@/name", "@a/b/c", "a/b"] {
            assert!(npm_access_error(bad, "public").is_some(), "{bad}");
        }
    }

    #[test]
    fn npm_publish_args_carry_access_and_no_secret() {
        let tgz = Path::new("/stage/p.tgz");
        let public = npm_publish_args(tgz, "latest", "public", "https://r.example", "required");
        let public: Vec<String> = public
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            public,
            [
                "publish",
                "/stage/p.tgz",
                "--tag",
                "latest",
                "--access",
                "public",
                "--provenance",
                "--registry",
                "https://r.example"
            ]
        );
        let restricted = npm_publish_args(tgz, "next", "restricted", "https://r.example", "none");
        let restricted: Vec<String> = restricted
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(restricted.contains(&"restricted".to_owned()));
        assert!(!restricted.contains(&"--provenance".to_owned()));
        assert!(restricted.contains(&"--provenance=false".to_owned()));
    }

    #[test]
    fn npm_pack_rejects_restricted_unscoped_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"0.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        let error = npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("mismatch");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_verify_pack_reports_restricted_access() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(
            &src,
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[src],
        )
        .expect("pack");
        let (name, _, registry, access, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "@s/npm-demo");
        assert_eq!(registry, "https://registry.npmjs.org");
        assert_eq!(access, "restricted");
        let body = std::fs::read_to_string(&feed).expect("feed");
        std::fs::write(&feed, body.replace("\"@s/npm-demo\"", "\"npm-demo\"")).expect("tamper");
        let error = npm_verify_pack(&tgz, &feed).expect_err("tampered");
        assert!(
            error.to_string().contains("needs a scoped package"),
            "{error}"
        );
    }

    #[test]
    fn npm_deploy_rejects_former_dry_run_request() {
        with_env(&[("NPM_PUBLISH_DRY_RUN", Some("1"))], || {
            assert_eq!(
                npm_deploy_main(&["prog".to_owned()], "a.feed.json", "a.tgz"),
                1
            );
        });
    }

    #[test]
    fn npm_cli_pack_and_publish_dry_run_accept_the_staged_package() {
        let ok = |tool: &str| {
            std::process::Command::new(tool)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if !ok("npm") {
            return;
        }
        let scratch = scratch_dir();
        let pkg = scratch.path().join("pkg");
        std::fs::create_dir_all(&pkg).expect("pkg");
        std::fs::write(
            pkg.join("package.json"),
            b"{\"name\": \"@s/npm-demo\", \"version\": \"0.0.0\"}\n",
        )
        .expect("write");
        let tgz = scratch.path().join("out.tgz");
        let feed = scratch.path().join("out.feed.json");
        npm_create_pack(
            "@s/npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "none",
            &tgz,
            &feed,
            &[pkg.join("package.json")],
        )
        .expect("pack");
        let run = |args: Vec<std::ffi::OsString>| {
            std::process::Command::new("npm")
                .args(args)
                .env("npm_config_cache", scratch.path().join("npm-cache"))
                .env("npm_config_userconfig", scratch.path().join("npmrc"))
                .env("npm_config_update_notifier", "false")
                .current_dir(scratch.path())
                .output()
                .expect("npm")
        };
        let packed = run(vec![
            "pack".into(),
            "--dry-run".into(),
            "--json".into(),
            pkg.clone().into(),
        ]);
        assert!(packed.status.success(), "{packed:?}");
        let mut args = npm_publish_args(
            &tgz,
            "latest",
            "restricted",
            "https://registry.npmjs.org",
            "none",
        );
        args.push("--dry-run".into());
        let published = run(args);
        let text = String::from_utf8_lossy(&published.stderr).into_owned()
            + &String::from_utf8_lossy(&published.stdout);
        assert!(published.status.success(), "{text}");
        assert!(text.contains("@s/npm-demo"), "{text}");
    }

    #[test]
    fn npm_pack_main_requires_full_argv() {
        let short: Vec<String> = ["npm_pack", "a", "b", "c", "d", "e"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(npm_pack_main(&short), 1);
    }

    #[test]
    fn launchers_reject_extra_arguments() {
        let argv = vec!["prog".to_owned(), "out".to_owned(), "extra".to_owned()];
        let spec = OctopusLaunch {
            package_rloc: "demo.tar.gz",
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to_raw: "",
            space_default: "",
            url_default: "https://octopus.example.invalid",
        };
        assert_eq!(archive_main("a", "b", "c", &argv), 1);
        assert_eq!(pypi_main("a", "b", "c", "https://pypi.invalid", &argv), 1);
        assert_eq!(
            nuget_main("a", "b", "", "", "https://nuget.invalid", &argv),
            1
        );
        assert_eq!(crates_main("a", "b", "1.0.0", &argv), 1);
        assert_eq!(github_main("a", "b", "v1.0.0", &argv), 1);
        assert_eq!(
            maven_main(
                "a",
                "b",
                "com.example",
                "demo",
                "1.0.0",
                "https://maven.invalid",
                &argv
            ),
            1
        );
        assert_eq!(
            oci_main(
                "a",
                "registry.invalid",
                "demo/app",
                "1.0.0",
                "linux/amd64",
                &argv
            ),
            1
        );
        assert_eq!(octopus_main(&spec, &argv), 1);
        assert_eq!(
            promotion_main("a", "b", "staging", "production", "1.0.0", &argv),
            1
        );
        assert_eq!(npm_deploy_main(&argv, "a", "b"), 1);
    }

    #[test]
    fn launchers_report_missing_pinned_inputs() {
        if env_test_in_child("tests::launchers_report_missing_pinned_inputs") {
            return;
        }
        let outdir = tempfile_stage_dir("dx-deploy-missing-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(outdir.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
            ],
            || {
                let missing = "dx-absent-pinned-input-xyz";
                assert_eq!(archive_main(missing, missing, missing, &argv), 1);
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1
                );
                assert_eq!(
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
                    1
                );
                assert_eq!(crates_main(missing, "demo", "1.0.0", &argv), 1);
                assert_eq!(github_main(missing, "demo", "v1.0.0", &argv), 1);
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
                assert_eq!(
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    1
                );
                let spec = OctopusLaunch {
                    package_rloc: missing,
                    deploy_name: "demo",
                    project: "demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "",
                    space_default: "",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 1);
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "1.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn owner_gates_accept_only_exact_approval() {
        let gates = [
            &PYPI_GATE,
            &NUGET_GATE,
            &CRATES_GATE,
            &GITHUB_GATE,
            &maven::MAVEN_GATE,
            &OCI_GATE,
            &OCTOPUS_GATE,
            &PROMOTION_GATE,
            &NPM_GATE,
        ];
        let tools: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.tool).collect();
        let approvals: std::collections::BTreeSet<&str> =
            gates.iter().map(|gate| gate.approved).collect();
        assert_eq!(tools.len(), gates.len());
        assert_eq!(approvals.len(), gates.len());
        for gate in gates {
            let base: std::collections::HashMap<&str, &str> =
                gate.credentials.iter().map(|key| (*key, "x")).collect();
            let run = |env: &std::collections::HashMap<&str, &str>| {
                gate.refusal(|key| env.get(key).copied().unwrap_or_default().to_owned())
            };
            let approval_text = format!("{}=1 (owner approval)", gate.approved);
            let message = run(&base).unwrap_or_default();
            assert!(
                message.starts_with(&format!("{}: ", gate.tool)),
                "{message}"
            );
            assert!(message.contains(&approval_text), "{message}");
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.approved, value);
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{value:?}: {message}"
                );
                assert!(message.contains(&approval_text), "{value:?}: {message}");
            }
            let mut env = base.clone();
            env.insert(gate.approved, "1");
            assert_eq!(run(&env), None, "{}", gate.tool);
            for credential in gate.credentials {
                let mut env = env.clone();
                env.insert(credential, "");
                let message = run(&env).unwrap_or_default();
                assert!(
                    message.starts_with(&format!("{}: ", gate.tool)),
                    "{message}"
                );
                assert!(message.contains(credential), "{message}");
                assert!(
                    message.contains(&format!("{}=1", gate.approved)),
                    "{message}"
                );
            }
            let lookup = |env: &std::collections::HashMap<&str, &str>, key: &str| {
                env.get(key).copied().unwrap_or_default().to_owned()
            };
            for env in [base.clone(), env.clone()] {
                let refusal = run(&env);
                match gate.authorize(|key| lookup(&env, key)) {
                    Ok(secrets) => {
                        assert_eq!(refusal, None, "{}", gate.tool);
                        for credential in gate.credentials {
                            assert_eq!(secrets.get(credential), "x");
                        }
                    }
                    Err(error) => assert_eq!(Some(error), refusal, "{}", gate.tool),
                }
            }
            for value in ["", "0", "true", "yes", " 1", "1 "] {
                let mut env = base.clone();
                env.insert(gate.live, value);
                assert!(!gate.requested(|key| lookup(&env, key)), "{value:?}");
            }
            let mut env = base.clone();
            env.insert(gate.live, "1");
            assert!(gate.requested(|key| lookup(&env, key)), "{}", gate.tool);
        }
        let lives: std::collections::BTreeSet<&str> = gates.iter().map(|gate| gate.live).collect();
        assert_eq!(lives.len(), gates.len());
    }

    #[test]
    fn launchers_refuse_live_publish_without_owner_approval() {
        if env_test_in_child("tests::launchers_refuse_live_publish_without_owner_approval") {
            return;
        }
        let outdir = tempfile_stage_dir("dx-deploy-live-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let runfiles = outdir.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        assert!(runfiles.is_dir());
        let missing = "dx-absent-pinned-input-xyz";
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("PYPI_PUBLISH_LIVE", Some("1")),
                ("PYPI_API_TOKEN", Some("")),
                ("PYPI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    pypi_main(missing, missing, "demo", "https://pypi.invalid", &argv),
                    1,
                    "pypi needs a resolvable wheel before the token gate"
                );
            },
        );
        with_env(
            &[
                ("NUGET_PUBLISH_LIVE", Some("1")),
                ("NUGET_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    nuget_main(missing, "demo", "", "", "https://nuget.invalid", &argv),
                    1
                );
            },
        );
        with_env(
            &[
                ("GH_RELEASE_LIVE", Some("1")),
                ("GH_RELEASE_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(github_main(missing, "demo", "v0.0.0-dryrun", &argv), 1);
            },
        );
        with_env(
            &[
                ("MAVEN_PUBLISH_LIVE", Some("1")),
                ("MAVEN_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    maven_main(
                        missing,
                        missing,
                        "com.example",
                        "demo",
                        "0.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    1
                );
            },
        );
        with_env(
            &[
                ("OCI_PUBLISH_LIVE", Some("1")),
                ("OCI_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    oci_main(
                        missing,
                        "registry.invalid",
                        "demo/app",
                        "0.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    1
                );
            },
        );
        with_env(
            &[
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(crates_main(missing, "demo", "0.0.0", &argv), 1);
            },
        );
        let crates_runfiles = outdir.join("crates_runfiles");
        stage_runfiles_inputs(&crates_runfiles);
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(crates_runfiles.to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("CRATES_PUBLISH_LIVE", Some("1")),
                ("CRATES_ALLOW_DIRTY", Some("1")),
                ("CRATES_PUBLISH_APPROVED", None),
                ("CARGO_REGISTRY_TOKEN", None),
            ],
            || {
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
                    1,
                    "CRATES_ALLOW_DIRTY=1 must refuse a live publish"
                );
            },
        );
        with_env(
            &[
                (
                    "RUNFILES_DIR",
                    Some(outdir.join("runfiles").to_string_lossy().as_ref()),
                ),
                ("RUNFILES_MANIFEST_FILE", None),
                ("NPM_PUBLISH_LIVE", Some("1")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("NPM_TOKEN", None),
                ("NPM_PUBLISH_DRY_RUN", None),
            ],
            || {
                assert_eq!(npm_deploy_main(&argv, missing, missing), 1);
            },
        );
        with_env(
            &[
                ("PROMOTION_LIVE", Some("1")),
                ("PROMOTION_APPROVED", Some("1")),
            ],
            || {
                assert_eq!(
                    promotion_main(missing, "demo", "staging", "production", "0.0.0", &argv),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn archive_and_promotion_launchers_stage_locally() {
        if env_test_in_child("tests::archive_and_promotion_launchers_stage_locally") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let app = runfiles.join("app_name.txt");
        std::fs::write(&app, b"demo_app\n").expect("write app");
        let tarball = runfiles.join("release_demo.tar.gz");
        let tarball_bytes = archive_bytes(b"payload", "release_demo.tar.gz", false).expect("tar");
        std::fs::write(&tarball, &tarball_bytes).expect("write tarball");
        let digest = dx_digest::sha256_file_hex(&tarball).expect("digest");
        let checksum = runfiles.join("release_demo.tar.gz.sha256");
        std::fs::write(&checksum, format!("{digest}  release_demo.tar.gz\n")).expect("write sum");

        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
            ],
            || {
                let outdir = tempfile_stage_dir("dx-deploy-archive-").expect("outdir");
                let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
                assert_eq!(
                    archive_main(
                        "app_name.txt",
                        "release_demo.tar.gz",
                        "release_demo.tar.gz.sha256",
                        &argv
                    ),
                    0
                );
                assert!(outdir.join("release_demo.tar.gz").is_file());
                std::fs::remove_dir_all(&outdir).expect("cleanup");

                let promotion_out = tempfile_stage_dir("dx-deploy-promotion-").expect("outdir");
                let promotion_argv = vec![
                    "prog".to_owned(),
                    promotion_out.to_string_lossy().into_owned(),
                ];
                assert_eq!(
                    promotion_main(
                        "release_demo.tar.gz",
                        "promotion_demo",
                        "staging",
                        "production",
                        "1.2.3",
                        &promotion_argv
                    ),
                    0
                );
                assert!(promotion_out.join("promotion_demo-promotion").is_dir());
                std::fs::remove_dir_all(&promotion_out).expect("cleanup");
            },
        );
    }

    #[cfg(unix)]
    #[test]
    fn promotion_live_records_health_and_refuses() {
        use std::os::unix::fs::PermissionsExt;
        if env_test_in_child("tests::promotion_live_records_health_and_refuses") {
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

    fn stage_runfiles_inputs(root: &Path) {
        std::fs::create_dir_all(root).expect("runfiles");
        let maven_pom = crate::maven::tests::central_pom("com.example", "maven_demo", "1.0.0");
        for (name, body) in [
            ("pypi_demo-0.0.0-py3-none-any.whl", &b"wheel"[..]),
            ("pypi_demo-0.0.0.tar.gz", &b"sdist"[..]),
            ("maven_demo-0.0.0.jar", &b"jar"[..]),
            ("maven_demo-0.0.0.pom", maven_pom.as_bytes()),
            ("release_demo.tar.gz", &b"package"[..]),
            (
                "Cargo.toml",
                &b"[package]\nname = \"crates_demo\"\nversion = \"1.0.0\"\n"[..],
            ),
            (
                "lib.rs",
                &b"pub fn hello() -> &'static str { \"hello\" }\n"[..],
            ),
        ] {
            std::fs::write(root.join(name), body).expect("write runfile input");
        }
        let layer = root.join("oci_demo_layer.tar");
        std::fs::write(&layer, oci_test_layer(true)).expect("write oci layer");
        archive_image(
            &[layer.as_path()],
            "linux/amd64",
            &["/app/hello"],
            &root.join("oci_demo.tar"),
        )
        .expect("write oci image");
        write_nupkg(
            &root.join("nuget_demo.0.0.0.nupkg"),
            &[("nuget_demo.nuspec", &nuspec("nuget_demo", "1.0.0"))],
        );
    }

    #[test]
    fn launchers_stage_locally_without_publishing() {
        if env_test_in_child("tests::launchers_stage_locally_without_publishing") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let outdir = tempfile_stage_dir("dx-deploy-stage-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("PYPI_PUBLISH_LIVE", None),
                ("NUGET_PUBLISH_LIVE", None),
                ("CRATES_PUBLISH_LIVE", None),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
                ("MAVEN_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_LIVE", None),
                ("OCI_PUBLISH_DRY_RUN", Some("1")),
                ("OCTOPUS_PUBLISH_LIVE", None),
            ],
            || {
                assert_eq!(
                    pypi_main(
                        "pypi_demo-0.0.0-py3-none-any.whl",
                        "pypi_demo-0.0.0.tar.gz",
                        "pypi_demo",
                        "https://pypi.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    nuget_main(
                        "nuget_demo.0.0.0.nupkg",
                        "nuget_demo",
                        "",
                        "1.0.0",
                        "https://nuget.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    crates_main("Cargo.toml;lib.rs", "crates_demo", "1.0.0", &argv),
                    0
                );
                assert_eq!(
                    github_main("release_demo.tar.gz", "github_demo", "v1.0.0", &argv),
                    0
                );
                assert_eq!(
                    maven_main(
                        "maven_demo-0.0.0.jar",
                        "maven_demo-0.0.0.pom",
                        "com.example",
                        "maven_demo",
                        "1.0.0",
                        "https://maven.invalid",
                        &argv
                    ),
                    0
                );
                assert_eq!(
                    oci_main(
                        "oci_demo.tar",
                        "registry.invalid",
                        "demo/app",
                        "1.0.0",
                        "linux/amd64",
                        &argv
                    ),
                    0
                );
                let spec = OctopusLaunch {
                    package_rloc: "release_demo.tar.gz",
                    deploy_name: "octopus_demo",
                    project: "octopus_demo",
                    channel: "Default",
                    version: "1.0.0",
                    deploy_to_raw: "Production,Staging",
                    space_default: "Spaces",
                    url_default: "https://octopus.example.invalid",
                };
                assert_eq!(octopus_main(&spec, &argv), 0);
            },
        );
        assert!(outdir.join("pypi_demo-wheelhouse").is_dir());
        assert!(outdir.join("maven_demo-repo").is_dir());
        assert!(outdir.join("github_demo-release").is_dir());
        assert!(outdir.join("crates_demo-vendor").is_dir());
        assert!(outdir.join("nuget_demo-feed").is_dir());
        assert!(outdir.join("octopus_demo-drop").is_dir());
        assert!(outdir.join("app-oci-layout").is_dir());
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn launcher_relative_outdir_resolves_against_invocation_dir() {
        if env_test_in_child("tests::launcher_relative_outdir_resolves_against_invocation_dir") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        stage_runfiles_inputs(&runfiles);
        let cwd = scratch.path().join("cwd");
        let invocation = scratch.path().join("inv");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&invocation).expect("inv");
        std::env::set_current_dir(&cwd).expect("chdir");
        let argv = vec!["prog".to_owned(), "dist".to_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                (
                    "BUILD_WORKING_DIRECTORY",
                    Some(invocation.to_string_lossy().as_ref()),
                ),
                ("GH_RELEASE_LIVE", None),
                ("GH_RELEASE_DRY_RUN", Some("1")),
            ],
            || {
                assert_eq!(
                    github_main("release_demo.tar.gz", "demo", "v1.0.0", &argv),
                    0
                );
            },
        );
        assert!(invocation.join("dist").join("demo-release").is_dir());
        assert!(!cwd.join("dist").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let bin = scratch.path().join("bin");
            let workspace = scratch.path().join("ws");
            std::fs::create_dir_all(&bin).expect("bin");
            std::fs::create_dir_all(&workspace).expect("ws");
            let log = scratch.path().join("gh.log");
            let env_log = scratch.path().join("gh.env");
            let script = bin.join("gh");
            std::fs::write(
                &script,
                format!(
                    "#!/bin/sh\npwd -P > '{}'\nprintf '%s\\n' \"$@\" >> '{}'\nenv > '{}'\n",
                    log.display(),
                    log.display(),
                    env_log.display()
                ),
            )
            .expect("script");
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
            std::env::set_current_dir(scratch.path()).expect("chdir");
            let path = format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            );
            with_env(
                &[
                    ("PATH", Some(path.as_str())),
                    ("RUNFILES_DIR", Some("runfiles")),
                    ("RUNFILES_MANIFEST_FILE", None),
                    (
                        "BUILD_WORKING_DIRECTORY",
                        Some(invocation.to_string_lossy().as_ref()),
                    ),
                    (
                        "BUILD_WORKSPACE_DIRECTORY",
                        Some(workspace.to_string_lossy().as_ref()),
                    ),
                    ("GH_REPO", None),
                    ("GH_RELEASE_LIVE", Some("1")),
                    ("GH_RELEASE_APPROVED", Some("1")),
                    ("GH_TOKEN", Some("gh-sekret")),
                    ("DX_LEAK_PROBE", Some("1")),
                ],
                || {
                    assert_eq!(
                        github_main("release_demo.tar.gz", "demo", "v1.0.0", &argv),
                        0
                    );
                },
            );
            let logged = std::fs::read_to_string(&log).expect("gh log");
            let lines: Vec<&str> = logged.lines().collect();
            let workspace_real = std::fs::canonicalize(&workspace).expect("canonical ws");
            assert_eq!(lines[0], workspace_real.to_string_lossy(), "{logged}");
            assert_eq!(&lines[1..4], ["release", "create", "v1.0.0"], "{logged}");
            let asset = Path::new(lines[4]);
            assert!(asset.is_absolute(), "{logged}");
            assert_eq!(
                std::fs::canonicalize(asset).expect("canonical asset"),
                std::fs::canonicalize(runfiles.join("release_demo.tar.gz")).expect("canonical"),
                "{logged}"
            );
            assert_eq!(&lines[5..], ["--draft", "--verify-tag"], "{logged}");
            let env = std::fs::read_to_string(&env_log).expect("gh env");
            assert!(env.lines().any(|line| line == "GH_TOKEN=gh-sekret"));
            assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
            assert!(!logged.contains("gh-sekret"), "{logged}");
        }
    }

    #[test]
    fn build_helpers_reject_unusable_inputs() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let image = scratch.path().join("oci_demo.bin");
        std::fs::write(&image, b"image").expect("write image");
        let error = oci_build_layout(
            &image,
            &outdir,
            "registry.invalid",
            "demo/app",
            "1.0.0",
            "linux/amd64",
        )
        .expect_err("non-tar image");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let package = scratch.path().join("release_demo.zip");
        std::fs::write(&package, b"package").expect("write package");
        let error = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "1.0.0",
                deploy_to: &[],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect_err("non-tar.gz package");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn octopus_drop_rejects_empty_spec_fields() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package").expect("write package");
        let base = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to: &[],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let empties = [
            (
                "deploy_name",
                OctopusDrop {
                    deploy_name: "",
                    ..base
                },
            ),
            (
                "project",
                OctopusDrop {
                    project: "",
                    ..base
                },
            ),
            (
                "channel",
                OctopusDrop {
                    channel: "",
                    ..base
                },
            ),
            (
                "version",
                OctopusDrop {
                    version: "",
                    ..base
                },
            ),
        ];
        for (field, spec) in empties {
            let error = octopus_build_drop(&package, &outdir, &spec).expect_err("empty spec field");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{field}: {error}"
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

    #[test]
    fn npm_bracketed_filename_round_trips_through_verify() {
        let scratch = scratch_dir();
        let src = scratch.path().join("[id].js");
        std::fs::write(&src, b"module.exports = 1;\n").expect("write src");
        let manifest = scratch.path().join("package.json");
        std::fs::write(
            &manifest,
            b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n",
        )
        .expect("write manifest");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            &[manifest, src],
        )
        .expect("pack");
        let (name, _, _, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        let members = npm_tar_members(&tgz).expect("members");
        assert_eq!(members, ["package/[id].js", "package/package.json"]);

        let body = std::fs::read_to_string(&feed).expect("read feed");
        let reordered = scratch.path().join("reordered.feed.json");
        let parsed: serde_json::Value = serde_json::from_str(&body).expect("json");
        std::fs::write(&reordered, serde_json::to_string(&parsed).expect("compact"))
            .expect("write");
        npm_verify_pack(&tgz, &reordered).expect("whitespace does not matter");

        let tampered = scratch.path().join("tampered.feed.json");
        std::fs::write(&tampered, body.replace("package/[id].js", "package/[x].js"))
            .expect("write");
        let error = npm_verify_pack(&tgz, &tampered).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn npm_verify_and_pack_report_feed_problems() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            "required",
            &tgz,
            &feed,
            std::slice::from_ref(&src),
        )
        .expect("pack");
        let (name, tag, registry, _, _) = npm_verify_pack(&tgz, &feed).expect("verify");
        assert_eq!(name, "npm-demo");
        assert_eq!(tag, "latest");
        assert_eq!(registry, "https://registry.npmjs.org");

        let body = std::fs::read_to_string(&feed).expect("read feed");
        for field in ["name", "tag", "registry", "tarball", "sha256", "files"] {
            let stripped = body
                .lines()
                .filter(|line| !line.contains(&format!("\"{field}\"")))
                .collect::<Vec<_>>()
                .join("\n");
            let path = scratch.path().join(format!("missing_{field}.feed.json"));
            std::fs::write(&path, stripped).expect("write feed");
            let error = npm_verify_pack(&tgz, &path).expect_err(field);
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{field}");
        }

        let wrong_members = scratch.path().join("wrong_members.feed.json");
        std::fs::write(
            &wrong_members,
            body.replace("package/package.json", "package/absent.json"),
        )
        .expect("write feed");
        let error = npm_verify_pack(&tgz, &wrong_members).expect_err("member mismatch");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn npm_pack_main_packs_and_reports_failures() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let tgz = scratch.path().join("npm_demo.tgz");
        let feed = scratch.path().join("npm_demo.feed.json");
        let argv = vec![
            "npm_pack".to_owned(),
            "npm-demo".to_owned(),
            "latest".to_owned(),
            "https://registry.npmjs.org".to_owned(),
            "public".to_owned(),
            "required".to_owned(),
            tgz.to_string_lossy().into_owned(),
            feed.to_string_lossy().into_owned(),
            src.to_string_lossy().into_owned(),
        ];
        assert_eq!(npm_pack_main(&argv), 0);
        assert!(tgz.is_file());
        assert!(feed.is_file());
        let mut missing_src = argv.clone();
        missing_src.pop();
        missing_src.push(
            scratch
                .path()
                .join("absent.json")
                .to_string_lossy()
                .into_owned(),
        );
        assert_eq!(npm_pack_main(&missing_src), 1);
    }

    fn stage_npm_demo(root: &Path, provenance: &str) -> std::path::PathBuf {
        let runfiles = root.join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        let src = runfiles.join("package.json");
        std::fs::write(&src, b"{\"name\": \"npm-demo\", \"version\": \"1.0.0\"}\n")
            .expect("write src");
        let bracketed = runfiles.join("[id].js");
        std::fs::write(&bracketed, b"module.exports = 1;\n").expect("write bracketed");
        npm_create_pack(
            "npm-demo",
            "latest",
            "https://registry.npmjs.org",
            "public",
            provenance,
            &runfiles.join("npm_demo.tgz"),
            &runfiles.join("npm_demo.feed.json"),
            &[src, bracketed],
        )
        .expect("pack");
        runfiles
    }

    #[test]
    fn npm_deploy_main_releases_a_staged_pack() {
        if env_test_in_child("tests::npm_deploy_main_releases_a_staged_pack") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_PROFILE", Some("dx_dev")),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("NPM_PUBLISH_LIVE", None),
                ("NPM_PUBLISH_APPROVED", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    0
                );
            },
        );
        assert!(outdir.join("npm_demo.tgz").is_file());
        assert!(outdir
            .join("npm_demo-feed")
            .join("npm_demo.feed.json")
            .is_file());
        assert!(npm_tar_members(&outdir.join("npm_demo.tgz"))
            .expect("members")
            .contains(&"package/[id].js".to_owned()));
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

    #[test]
    fn npm_provenance_policy_rejects_unsupported_combinations() {
        let npmjs = "https://registry.npmjs.org";
        assert!(npm_provenance_error("public", npmjs, "required").is_none());
        assert!(npm_provenance_error("public", npmjs, "none").is_none());
        assert!(npm_provenance_error("restricted", "https://npm.example.com", "none").is_none());
        let restricted = npm_provenance_error("restricted", npmjs, "required").expect("restricted");
        assert!(restricted.contains("not supported with access 'restricted'"));
        let private = npm_provenance_error("public", "https://npm.example.com/repo", "required")
            .expect("registry");
        assert!(private.contains("not supported by registry"));
        let unknown = npm_provenance_error("public", npmjs, "auto").expect("unknown");
        assert!(unknown.contains("want 'required' or 'none'"));
    }

    #[test]
    fn npm_create_pack_refuses_unsupported_provenance_before_writing() {
        let scratch = scratch_dir();
        let src = scratch.path().join("package.json");
        std::fs::write(&src, b"{\"name\": \"x\", \"version\": \"1.0.0\"}\n").expect("write");
        let tgz = scratch.path().join("x.tgz");
        let feed = scratch.path().join("x.feed.json");
        let error = npm_create_pack(
            "@s/x",
            "latest",
            "https://registry.npmjs.org",
            "restricted",
            "required",
            &tgz,
            &feed,
            &[src],
        )
        .expect_err("restricted provenance");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!tgz.exists() && !feed.exists());
    }

    #[test]
    fn npm_publish_args_state_the_provenance_policy() {
        let tgz = Path::new("x.tgz");
        let required = npm_publish_args(tgz, "latest", "public", "https://r", "required");
        assert!(required.iter().any(|arg| arg == "--provenance"));
        let none = npm_publish_args(tgz, "latest", "restricted", "https://r", "none");
        assert!(none.iter().any(|arg| arg == "--provenance=false"));
        assert!(!none.iter().any(|arg| arg == "--provenance"));
    }

    #[test]
    fn npm_provenance_unavailable_without_identity() {
        let none = [
            ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
            ("SIGSTORE_ID_TOKEN", None),
        ];
        with_env(&none, || {
            let error = npm_provenance_unavailable("required").expect("unavailable");
            assert!(error.contains("publishing nothing"));
            assert!(npm_provenance_unavailable("none").is_none());
        });
        with_env(
            &[
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || assert!(npm_provenance_unavailable("required").is_none()),
        );
    }

    #[cfg(unix)]
    fn fake_octo(dir: &Path, fail_on: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("octo");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" '---' >> '{d}/args.txt'\nenv >> '{d}/env.txt'\nif [ \"$1\" = '{fail_on}' ]; then exit 1; fi\nexit 0\n"
        );
        std::fs::write(&path, script).expect("write octo");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    fn octo_spec<'a>(envs: &'a [String], space: &'a str) -> OctopusDrop<'a> {
        OctopusDrop {
            deploy_name: "p",
            project: "p",
            channel: "Default",
            version: "1.0.0",
            deploy_to: envs,
            space,
            server: "https://o.invalid",
        }
    }

    #[cfg(unix)]
    fn octo_calls(dir: &Path) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(dir.join("args.txt")).unwrap_or_default();
        let mut calls = vec![];
        let mut cur = vec![];
        for line in text.lines() {
            if line == "---" {
                calls.push(std::mem::take(&mut cur));
            } else {
                cur.push(line.to_owned());
            }
        }
        calls
    }

    #[cfg(unix)]
    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_runs_push_then_release() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "none");
        let package = Path::new("/stage/p-1.0.0.tar.gz");
        let envs = ["Production".to_owned()];
        with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("OCTOPUS_API_KEY", Some("leaky")),
            ],
            || {
                assert_eq!(
                    octopus_live_publish(&octo, package, &octo_spec(&envs, "S"), "sekret"),
                    0
                );
            },
        );
        let calls = octo_calls(scratch.path());
        let spec = octo_spec(&envs, "S");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], strings(octopus_push_args(package, &spec)));
        assert_eq!(calls[1], strings(octopus_release_args(&spec)));
        assert!(calls[0].contains(&"S".to_owned()));
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!args.contains("sekret") && !args.contains("--apiKey"));
        assert!(env.contains("OCTOPUS_CLI_API_KEY=sekret"));
        assert!(!env.contains("OCTOPUS_API_KEY") && !env.contains("DX_LEAK_PROBE"));
        std::fs::remove_file(scratch.path().join("args.txt")).expect("rm");
        assert_eq!(
            octopus_live_publish(&octo, package, &octo_spec(&envs, ""), "sekret"),
            0
        );
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(!args.contains("--space"));
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_stops_after_failed_push() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "push");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_surfaces_release_failure() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "create-release");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 2);
    }

    #[cfg(unix)]
    fn fake_npm(dir: &Path, exit: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("npm");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{d}/args.txt'\nenv > '{d}/env.txt'\nexit {exit}\n"
        );
        std::fs::write(&path, script).expect("write npm");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_passes_policy_and_keeps_the_token_out_of_args() {
        let scratch = scratch_dir();
        let npm = fake_npm(scratch.path(), 0);
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://registry.npmjs.org",
            "required",
        );
        let code = with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("NPM_TOKEN", Some("sekret")),
                ("GITHUB_TOKEN", Some("gh-sekret")),
                ("CI_JOB_TOKEN", Some("job-sekret")),
                ("CI_PROJECT_URL", Some("https://gitlab.example/p")),
                (
                    "ACTIONS_ID_TOKEN_REQUEST_URL",
                    Some("https://ci.example/token"),
                ),
            ],
            || npm_run_publish(&npm, "registry.npmjs.org", "sekret", &args),
        );
        assert_eq!(code, 0);
        let seen = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(seen.contains("--provenance\n"));
        assert!(seen.contains("--userconfig"));
        assert!(!seen.contains("sekret"));
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!env.contains("DX_LEAK_PROBE"), "{env}");
        assert!(!env.contains("sekret"), "{env}");
        assert!(env
            .lines()
            .any(|line| line == "ACTIONS_ID_TOKEN_REQUEST_URL=https://ci.example/token"));
        assert!(env
            .lines()
            .any(|line| line == "CI_PROJECT_URL=https://gitlab.example/p"));
    }

    #[cfg(unix)]
    #[test]
    fn npm_run_publish_surfaces_tool_failure_and_refusal() {
        let scratch = scratch_dir();
        let args = npm_publish_args(
            Path::new("x.tgz"),
            "latest",
            "public",
            "https://r",
            "required",
        );
        let refused = fake_npm(scratch.path(), 1);
        assert_eq!(npm_run_publish(&refused, "r", "tok", &args), 1);
        let failed = fake_npm(scratch.path(), 7);
        assert_eq!(npm_run_publish(&failed, "r", "tok", &args), 7);
        assert_eq!(
            npm_run_publish(&scratch.path().join("absent"), "r", "tok", &args),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn npm_deploy_main_requires_owner_approval_for_live_publish() {
        if env_test_in_child("tests::npm_deploy_main_requires_owner_approval_for_live_publish") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "none");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        fake_npm(&bin, 0);
        let args = bin.join("args.txt");
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        for (approved, want) in [(None, 1), (Some("0"), 1), (Some("true"), 1), (Some("1"), 0)] {
            let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
            let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
            with_env(
                &[
                    ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                    ("RUNFILES_MANIFEST_FILE", None),
                    ("BUILD_WORKSPACE_DIRECTORY", None),
                    ("PATH", Some(path.as_str())),
                    ("NPM_PUBLISH_LIVE", Some("1")),
                    ("NPM_PUBLISH_DRY_RUN", None),
                    ("NPM_TOKEN", Some("sekret")),
                    ("NPM_PUBLISH_APPROVED", approved),
                ],
                || {
                    assert_eq!(
                        npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                        want,
                        "approval {approved:?}"
                    );
                },
            );
            std::fs::remove_dir_all(&outdir).expect("cleanup");
            if want == 1 {
                assert!(!args.exists(), "npm ran with approval {approved:?}");
            }
        }
        let seen = std::fs::read_to_string(&args).expect("npm ran once approved");
        assert!(seen.contains("publish"));
        assert!(!seen.contains("sekret"));
    }

    #[test]
    fn npm_deploy_main_refuses_required_provenance_without_identity() {
        if env_test_in_child("tests::npm_deploy_main_refuses_required_provenance_without_identity")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = stage_npm_demo(scratch.path(), "required");
        let outdir = tempfile_stage_dir("dx-deploy-npm-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("NPM_PUBLISH_LIVE", Some("1")),
                ("NPM_TOKEN", Some("tok")),
                ("NPM_PUBLISH_APPROVED", Some("1")),
                ("ACTIONS_ID_TOKEN_REQUEST_URL", None),
                ("SIGSTORE_ID_TOKEN", None),
            ],
            || {
                assert_eq!(
                    npm_deploy_main(&argv, "npm_demo.feed.json", "npm_demo.tgz"),
                    1
                );
            },
        );
        std::fs::remove_dir_all(&outdir).expect("cleanup");
    }

}
