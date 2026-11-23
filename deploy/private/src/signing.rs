//! Cosign signing of release assets: the pinned binary, its argv and the identity.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_process::capture::RunError;

/// The OIDC issuer of GitHub Actions identity tokens.
pub const GITHUB_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// A failed cosign pin check, signature, verification or registry login.
#[derive(Debug)]
pub enum CosignError {
    /// `sign = "required"` and no identity is available.
    NoIdentity,
    /// An invocation has no binary.
    EmptyArgv,
    /// `argv` could not be run.
    Run { argv: String, source: RunError },
    /// The pinned cosign could not be hashed.
    Hash { path: String, source: io::Error },
    /// The cosign binary does not have its pinned sha256.
    Digest {
        path: String,
        actual: String,
        want: String,
    },
    /// `cosign version` did not print the pinned GitVersion.
    Version { want: String, got: String },
    /// `cosign sign-blob` exited nonzero for `asset`.
    SignFailed { argv: String, asset: String },
    /// `cosign verify-blob` rejected the bundle of `asset`.
    VerifyFailed { argv: String, asset: String },
    /// An image sign, attest or verify step exited nonzero.
    ImageStepFailed {
        argv: String,
        reference: String,
        stderr: String,
    },
    /// An asset path has no file name to name its bundle after.
    NoFileName { path: PathBuf },
    /// A directory could not be created.
    CreateDir(PathIoError),
    /// The registry login could not be written.
    WriteDockerConfig { path: PathBuf, source: io::Error },
}

impl fmt::Display for CosignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoIdentity => f.write_str(
                "sign = \"required\" needs a signing identity (COSIGN_KEY or ACTIONS_ID_TOKEN_REQUEST_URL); publishing nothing",
            ),
            Self::EmptyArgv => f.write_str("signing: invocation needs a binary"),
            Self::Run { argv, source } => write!(f, "signing: cannot run '{argv}': {source}"),
            Self::Hash { path, source } => {
                write!(f, "signing: cannot hash cosign at {path}: {source}")
            }
            Self::Digest { path, actual, want } => write!(
                f,
                "signing: cosign at {path} has sha256 {actual}, want {want} (pinned per deploy/private/release/pins.json); refusing to sign"
            ),
            Self::Version { want, got } => write!(
                f,
                "signing: cosign GitVersion must be {want} (pinned per deploy/private/release/pins.json); got: {got}"
            ),
            Self::SignFailed { argv, asset } => {
                write!(f, "signing: '{argv}' failed for {asset}; publishing nothing")
            }
            Self::VerifyFailed { argv, asset } => write!(
                f,
                "signing: '{argv}' failed for {asset}; bundle rejected, publishing nothing"
            ),
            Self::ImageStepFailed {
                argv,
                reference,
                stderr,
            } => write!(f, "signing: '{argv}' failed for {reference}: {stderr}"),
            Self::NoFileName { path } => {
                write!(f, "signing: {} has no file name", path.display())
            }
            Self::CreateDir(source) => write!(f, "signing: {source}"),
            Self::WriteDockerConfig { path, source } => {
                write!(f, "signing: cannot write {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for CosignError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Run { source, .. } => Some(source),
            Self::Hash { source, .. } | Self::WriteDockerConfig { source, .. } => Some(source),
            Self::CreateDir(source) => Some(source),
            Self::NoIdentity
            | Self::EmptyArgv
            | Self::Digest { .. }
            | Self::Version { .. }
            | Self::SignFailed { .. }
            | Self::VerifyFailed { .. }
            | Self::ImageStepFailed { .. }
            | Self::NoFileName { .. } => None,
        }
    }
}

/// A pinned cosign binary.
#[derive(Clone, Copy)]
pub struct CosignPin<'a> {
    pub path: &'a str,
    pub version: &'a str,
    pub sha256: &'a str,
}

/// Whether `github_deploy` signs its assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignMode {
    /// Sign when an identity is available.
    Auto,
    /// Never sign.
    Off,
    /// Fail without an identity.
    Required,
}

impl SignMode {
    /// Reads `auto`, `off` or `required`.
    pub fn parse(text: &str) -> Option<SignMode> {
        match text {
            "auto" => Some(SignMode::Auto),
            "off" => Some(SignMode::Off),
            "required" => Some(SignMode::Required),
            _ => None,
        }
    }
}

/// The identity a signature is made with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignIdentity {
    /// Keyless signing with the GitHub Actions OIDC token of `repository`.
    Oidc { repository: String },
    /// Signing with the key in `COSIGN_KEY`.
    Key,
}

impl SignIdentity {
    /// Names the identity for a dry run.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            SignIdentity::Oidc { repository } if repository.is_empty() => {
                format!("keyless OIDC (issuer {GITHUB_OIDC_ISSUER})")
            }
            SignIdentity::Oidc { repository } => {
                format!("keyless OIDC (issuer {GITHUB_OIDC_ISSUER}, repository {repository})")
            }
            SignIdentity::Key => "key from COSIGN_KEY".to_owned(),
        }
    }
}

/// Picks the identity from the environment: `COSIGN_KEY` first, then the OIDC request URL.
pub fn detect_identity(env: &dyn Fn(&str) -> String) -> Option<SignIdentity> {
    if !env("COSIGN_KEY").is_empty() {
        return Some(SignIdentity::Key);
    }
    if !env("ACTIONS_ID_TOKEN_REQUEST_URL").is_empty() {
        return Some(SignIdentity::Oidc {
            repository: env("GITHUB_REPOSITORY"),
        });
    }
    None
}

/// Resolves the identity `mode` signs with; `None` means do not sign.
pub fn plan_identity(
    mode: SignMode,
    env: &dyn Fn(&str) -> String,
) -> Result<Option<SignIdentity>, CosignError> {
    match (mode, detect_identity(env)) {
        (SignMode::Off, _) => Ok(None),
        (SignMode::Auto, identity) => Ok(identity),
        (SignMode::Required, Some(identity)) => Ok(Some(identity)),
        (SignMode::Required, None) => Err(CosignError::NoIdentity),
    }
}

/// The bundle name for the asset called `base`.
#[must_use]
pub fn bundle_name(base: &str) -> String {
    format!("{base}.sigstore.json")
}

/// The repository whose reusable workflows sign consumer releases.
const REUSABLE_WORKFLOW_REPOSITORY: &str = "devhusdk/rules_dx";

fn escape_regexp(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/' {
            out.push(c);
        } else {
            out.push('\\');
            out.push(c);
        }
    }
    out
}

/// The certificate identity regexp the signatures of `repository` must match: the
/// repository's own workflows or the reusable workflows of `REUSABLE_WORKFLOW_REPOSITORY`.
#[must_use]
pub fn identity_regexp(repository: &str) -> String {
    let reusable = escape_regexp(REUSABLE_WORKFLOW_REPOSITORY);
    if repository.is_empty() {
        return format!("^https://github\\.com/{reusable}/");
    }
    format!(
        "^https://github\\.com/({}|{reusable})/",
        escape_regexp(repository)
    )
}

/// Formats `cosign sign-blob` for one asset.
#[must_use]
pub fn sign_argv(cosign: &str, asset: &str, bundle: &str) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "sign-blob".to_owned(),
        "--yes".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        asset.to_owned(),
    ]
}

/// Formats `cosign sign-blob` with the key in `COSIGN_KEY`.
#[must_use]
pub fn sign_key_argv(cosign: &str, asset: &str, bundle: &str) -> Vec<String> {
    let mut argv = sign_argv(cosign, asset, bundle);
    let at = argv.len() - 1;
    argv.splice(at..at, ["--key".to_owned(), "env://COSIGN_KEY".to_owned()]);
    argv
}

/// Formats `cosign verify-blob` against one certificate identity.
#[must_use]
pub fn verify_argv(
    cosign: &str,
    asset: &str,
    bundle: &str,
    identity: &str,
    issuer: &str,
) -> Vec<String> {
    verify_flag_argv(
        cosign,
        asset,
        bundle,
        "--certificate-identity",
        identity,
        issuer,
    )
}

fn verify_flag_argv(
    cosign: &str,
    asset: &str,
    bundle: &str,
    flag: &str,
    identity: &str,
    issuer: &str,
) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "verify-blob".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        flag.to_owned(),
        identity.to_owned(),
        "--certificate-oidc-issuer".to_owned(),
        issuer.to_owned(),
        asset.to_owned(),
    ]
}

/// Formats `cosign verify-blob` against a certificate identity regexp.
#[must_use]
pub fn verify_regexp_argv(
    cosign: &str,
    asset: &str,
    bundle: &str,
    regexp: &str,
    issuer: &str,
) -> Vec<String> {
    verify_flag_argv(
        cosign,
        asset,
        bundle,
        "--certificate-identity-regexp",
        regexp,
        issuer,
    )
}

/// The `GitVersion` line of `cosign version`.
pub fn cosign_git_version(version_output: &str) -> Option<&str> {
    version_output
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("GitVersion:"))
        .map(str::trim)
}

/// Whether `cosign version` printed exactly `want`.
#[must_use]
pub fn signing_version_ok(version_output: &str, want: &str) -> bool {
    cosign_git_version(version_output) == Some(want)
}

/// Runs `argv` with the inherited environment and working directory.
pub fn run_output(argv: &[String]) -> Result<dx_process::capture::Captured, CosignError> {
    let (binary, args) = argv.split_first().ok_or(CosignError::EmptyArgv)?;
    dx_process::capture::run_captured(inherited_command(binary).args(args), SIGN_TIMEOUT).map_err(
        |source| CosignError::Run {
            argv: argv.join(" "),
            source,
        },
    )
}

/// A child with the inherited environment, minus this tool's own deploy config.
fn inherited_command(binary: &str) -> std::process::Command {
    let mut command = std::process::Command::new(binary);
    command.env_remove(crate::config::DEPLOY_CONFIG_ENV);
    command
}

/// Checks the cosign binary against its pinned sha256 and version.
pub fn check_pin(pin: &CosignPin<'_>) -> Result<(), CosignError> {
    let actual =
        dx_digest::sha256_file_hex(Path::new(pin.path)).map_err(|source| CosignError::Hash {
            path: pin.path.to_owned(),
            source,
        })?;
    if actual != pin.sha256 {
        return Err(CosignError::Digest {
            path: pin.path.to_owned(),
            actual,
            want: pin.sha256.to_owned(),
        });
    }
    let argv = vec![pin.path.to_owned(), "version".to_owned()];
    let out = run_output(&argv)?;
    let mut text = out.stdout_text();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.success() || !signing_version_ok(&text, pin.version) {
        return Err(CosignError::Version {
            want: pin.version.to_owned(),
            got: cosign_git_version(&text).unwrap_or("none").to_owned(),
        });
    }
    Ok(())
}

/// Signs `asset` into `bundle` and verifies the bundle against `verify`.
pub fn sign_one(
    pin: &CosignPin<'_>,
    asset: &str,
    bundle: &str,
    keyed: bool,
    verify: Option<Vec<String>>,
) -> Result<(), CosignError> {
    let sign = if keyed {
        sign_key_argv(pin.path, asset, bundle)
    } else {
        sign_argv(pin.path, asset, bundle)
    };
    let out = run_output(&sign)?;
    if !out.success() {
        return Err(CosignError::SignFailed {
            argv: sign.join(" "),
            asset: asset.to_owned(),
        });
    }
    if let Some(verify) = verify {
        let out = run_output(&verify)?;
        if !out.success() {
            return Err(CosignError::VerifyFailed {
                argv: verify.join(" "),
                asset: asset.to_owned(),
            });
        }
    }
    Ok(())
}

/// Signs every asset into `outdir` as `<name>.sigstore.json` and returns the bundles.
pub fn sign_assets(
    pin: &CosignPin<'_>,
    identity: &SignIdentity,
    assets: &[PathBuf],
    outdir: &Path,
) -> Result<Vec<PathBuf>, CosignError> {
    check_pin(pin)?;
    path_io::create_dir_all(outdir).map_err(CosignError::CreateDir)?;
    let mut bundles = Vec::new();
    for asset in assets {
        let base = asset
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| CosignError::NoFileName {
                path: asset.clone(),
            })?;
        let bundle = outdir.join(bundle_name(&base));
        let (asset_text, bundle_text) = (
            asset.to_string_lossy().into_owned(),
            bundle.to_string_lossy().into_owned(),
        );
        let (keyed, verify) = match identity {
            SignIdentity::Key => (true, None),
            SignIdentity::Oidc { repository } => (
                false,
                Some(verify_regexp_argv(
                    pin.path,
                    &asset_text,
                    &bundle_text,
                    &identity_regexp(repository),
                    GITHUB_OIDC_ISSUER,
                )),
            ),
        };
        sign_one(pin, &asset_text, &bundle_text, keyed, verify)?;
        bundles.push(bundle);
    }
    Ok(bundles)
}

/// The cosign settings of one `oci_deploy`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageSigning {
    /// `auto`, `off` or `required`.
    pub mode: String,
    pub cosign_rloc: String,
    pub cosign_version: String,
    pub cosign_sha256: String,
    /// The SPDX SBOM attested to the pushed digest; empty for none.
    pub sbom_rloc: String,
}

impl Default for ImageSigning {
    fn default() -> Self {
        Self {
            mode: "off".to_owned(),
            cosign_rloc: String::new(),
            cosign_version: String::new(),
            cosign_sha256: String::new(),
            sbom_rloc: String::new(),
        }
    }
}

fn with_key(mut argv: Vec<String>, keyed: bool) -> Vec<String> {
    if keyed {
        argv.splice(2..2, ["--key".to_owned(), "env://COSIGN_KEY".to_owned()]);
    }
    argv
}

/// Formats `cosign sign` for one image reference.
#[must_use]
pub fn image_sign_argv(cosign: &str, reference: &str, keyed: bool) -> Vec<String> {
    with_key(
        vec![
            cosign.to_owned(),
            "sign".to_owned(),
            "--yes".to_owned(),
            reference.to_owned(),
        ],
        keyed,
    )
}

/// Formats `cosign attest` of an SPDX SBOM for one image reference.
#[must_use]
pub fn image_attest_argv(cosign: &str, reference: &str, sbom: &str, keyed: bool) -> Vec<String> {
    with_key(
        vec![
            cosign.to_owned(),
            "attest".to_owned(),
            "--yes".to_owned(),
            "--type".to_owned(),
            "spdxjson".to_owned(),
            "--predicate".to_owned(),
            sbom.to_owned(),
            reference.to_owned(),
        ],
        keyed,
    )
}

/// Formats `cosign verify` or, with `attestation`, `cosign verify-attestation --type spdxjson`
/// against a certificate identity regexp.
#[must_use]
pub fn image_verify_argv(
    cosign: &str,
    reference: &str,
    regexp: &str,
    issuer: &str,
    attestation: bool,
) -> Vec<String> {
    let mut argv = vec![cosign.to_owned()];
    if attestation {
        argv.extend(["verify-attestation", "--type", "spdxjson"].map(str::to_owned));
    } else {
        argv.push("verify".to_owned());
    }
    argv.extend(
        [
            "--certificate-identity-regexp",
            regexp,
            "--certificate-oidc-issuer",
            issuer,
            reference,
        ]
        .map(str::to_owned),
    );
    argv
}

/// The cosign steps for one pushed image: sign, then attest `sbom` when set.
#[must_use]
pub fn image_steps(
    cosign: &str,
    identity: &SignIdentity,
    reference: &str,
    sbom: Option<&str>,
) -> Vec<Vec<String>> {
    let keyed = *identity == SignIdentity::Key;
    let mut steps = vec![image_sign_argv(cosign, reference, keyed)];
    if let Some(sbom) = sbom {
        steps.push(image_attest_argv(cosign, reference, sbom, keyed));
    }
    if let SignIdentity::Oidc { repository } = identity {
        let regexp = identity_regexp(repository);
        steps.push(image_verify_argv(
            cosign,
            reference,
            &regexp,
            GITHUB_OIDC_ISSUER,
            false,
        ));
        if sbom.is_some() {
            steps.push(image_verify_argv(
                cosign,
                reference,
                &regexp,
                GITHUB_OIDC_ISSUER,
                true,
            ));
        }
    }
    steps
}

/// The `DOCKER_CONFIG` auth key cosign looks up for `registry`.
#[must_use]
pub fn docker_auth_key(registry: &str) -> String {
    let host = registry
        .split('/')
        .next()
        .unwrap_or(registry)
        .to_ascii_lowercase();
    if host == "docker.io" {
        return "https://index.docker.io/v1/".to_owned();
    }
    host
}

/// Writes a `DOCKER_CONFIG` directory holding one basic login for `registry`.
pub fn write_docker_config(
    dir: &Path,
    registry: &str,
    user: &str,
    secret: &str,
) -> Result<(), CosignError> {
    use base64::Engine as _;
    path_io::create_dir_all(dir).map_err(CosignError::CreateDir)?;
    let auth = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{secret}"));
    let body = serde_json::json!({"auths": {docker_auth_key(registry): {"auth": auth}}});
    let path = dir.join("config.json");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(source) => return Err(CosignError::WriteDockerConfig { path, source }),
    };
    std::io::Write::write_all(&mut file, body.to_string().as_bytes())
        .map_err(|source| CosignError::WriteDockerConfig { path, source })
}

/// Signs and attests one pushed image with the registry login in `docker_config`.
pub fn sign_image(
    pin: &CosignPin<'_>,
    identity: &SignIdentity,
    reference: &str,
    sbom: Option<&str>,
    docker_config: &Path,
) -> Result<(), CosignError> {
    check_pin(pin)?;
    for step in image_steps(pin.path, identity, reference, sbom) {
        let (binary, args) = step.split_first().ok_or(CosignError::EmptyArgv)?;
        let mut command = std::process::Command::new(binary);
        command.args(args).env("DOCKER_CONFIG", docker_config);
        let out =
            dx_process::capture::run_captured(&mut command, SIGN_TIMEOUT).map_err(|source| {
                CosignError::Run {
                    argv: step.join(" "),
                    source,
                }
            })?;
        if !out.success() {
            return Err(CosignError::ImageStepFailed {
                argv: step.join(" "),
                reference: reference.to_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_children_do_not_see_the_deploy_config() {
        let command = inherited_command("cosign");
        let envs: Vec<_> = command.get_envs().collect();
        assert_eq!(
            envs,
            [(std::ffi::OsStr::new(crate::config::DEPLOY_CONFIG_ENV), None)]
        );
    }

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> String + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map_or_else(String::new, |(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn sign_modes_parse() {
        assert_eq!(SignMode::parse("auto"), Some(SignMode::Auto));
        assert_eq!(SignMode::parse("off"), Some(SignMode::Off));
        assert_eq!(SignMode::parse("required"), Some(SignMode::Required));
        assert_eq!(SignMode::parse("on"), None);
    }

    #[test]
    fn the_key_wins_over_oidc() {
        let env = env_of(&[
            ("COSIGN_KEY", "k"),
            ("ACTIONS_ID_TOKEN_REQUEST_URL", "https://x"),
        ]);
        assert_eq!(detect_identity(&env), Some(SignIdentity::Key));
    }

    #[test]
    fn oidc_carries_the_repository() {
        let env = env_of(&[
            ("ACTIONS_ID_TOKEN_REQUEST_URL", "https://x"),
            ("GITHUB_REPOSITORY", "o/r"),
        ]);
        assert_eq!(
            detect_identity(&env),
            Some(SignIdentity::Oidc {
                repository: "o/r".to_owned()
            })
        );
    }

    #[test]
    fn modes_plan_an_identity() {
        let none = env_of(&[]);
        let key = env_of(&[("COSIGN_KEY", "k")]);
        assert_eq!(plan_identity(SignMode::Auto, &none).ok(), Some(None));
        assert_eq!(plan_identity(SignMode::Off, &key).ok(), Some(None));
        assert_eq!(
            plan_identity(SignMode::Required, &key).ok(),
            Some(Some(SignIdentity::Key))
        );
        let error = plan_identity(SignMode::Required, &none).expect_err("no identity");
        assert!(matches!(error, CosignError::NoIdentity), "{error}");
        assert!(
            error.to_string().contains("needs a signing identity"),
            "{error}"
        );
    }

    #[test]
    fn bundles_and_identities_are_named() {
        assert_eq!(bundle_name("a.tar.gz"), "a.tar.gz.sigstore.json");
        assert_eq!(
            identity_regexp("o/r.x"),
            "^https://github\\.com/(o/r\\.x|devhusdk/rules_dx)/"
        );
        assert_eq!(
            identity_regexp(""),
            "^https://github\\.com/devhusdk/rules_dx/"
        );
        assert!(SignIdentity::Key.describe().contains("COSIGN_KEY"));
        assert!(
            SignIdentity::Oidc {
                repository: "o/r".to_owned()
            }
            .describe()
            .contains(GITHUB_OIDC_ISSUER)
        );
    }

    #[test]
    fn argv_names_the_bundle_and_the_key() {
        let plain = sign_argv("c", "a", "b");
        assert_eq!(plain.last().map(String::as_str), Some("a"));
        assert!(!plain.contains(&"--key".to_owned()));
        let keyed = sign_key_argv("c", "a", "b");
        assert_eq!(
            &keyed[keyed.len() - 3..],
            ["--key", "env://COSIGN_KEY", "a"]
        );
        let verify = verify_regexp_argv("c", "a", "b", "^r", "i");
        assert!(verify.contains(&"--certificate-identity-regexp".to_owned()));
        assert!(!verify.contains(&"--certificate-identity".to_owned()));
    }

    #[test]
    fn the_version_must_match_exactly() {
        let real = "GitVersion:    v9.8.7\n";
        assert_eq!(cosign_git_version(real), Some("v9.8.7"));
        assert!(signing_version_ok(real, "v9.8.7"));
        assert!(!signing_version_ok("GitVersion:    v9.8.70\n", "v9.8.7"));
        assert!(!signing_version_ok("", "v9.8.7"));
    }

    #[test]
    fn image_steps_sign_attest_and_verify() {
        let oidc = SignIdentity::Oidc {
            repository: "o/r".to_owned(),
        };
        let steps = image_steps("c", &oidc, "r/p@sha256:1", Some("s.spdx.json"));
        let heads: Vec<&str> = steps.iter().map(|step| step[1].as_str()).collect();
        assert_eq!(heads, ["sign", "attest", "verify", "verify-attestation"]);
        assert_eq!(steps[0], ["c", "sign", "--yes", "r/p@sha256:1"]);
        assert_eq!(
            steps[1],
            [
                "c",
                "attest",
                "--yes",
                "--type",
                "spdxjson",
                "--predicate",
                "s.spdx.json",
                "r/p@sha256:1"
            ]
        );
        assert!(steps[3].contains(&"spdxjson".to_owned()));
        let keyed = image_steps("c", &SignIdentity::Key, "r/p@sha256:1", None);
        assert_eq!(
            keyed,
            [[
                "c",
                "sign",
                "--key",
                "env://COSIGN_KEY",
                "--yes",
                "r/p@sha256:1"
            ]]
        );
    }

    #[test]
    fn docker_config_keys_the_registry_host() {
        assert_eq!(docker_auth_key("ghcr.io/example"), "ghcr.io");
        assert_eq!(docker_auth_key("docker.io"), "https://index.docker.io/v1/");
        let dir = tempfile::tempdir().expect("tempdir");
        write_docker_config(dir.path(), "localhost:5000", "u", "p").expect("write");
        let body: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("config.json")).expect("read"))
                .expect("json");
        assert_eq!(body["auths"]["localhost:5000"]["auth"], "dTpw");
    }
}
