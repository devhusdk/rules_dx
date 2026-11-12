//! Cosign signing of release assets: the pinned binary, its argv and the identity.

use std::path::{Path, PathBuf};

/// The OIDC issuer of GitHub Actions identity tokens.
pub const GITHUB_OIDC_ISSUER: &str = "https://token.actions.githubusercontent.com";

const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// A pinned cosign binary.
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
) -> Result<Option<SignIdentity>, String> {
    match (mode, detect_identity(env)) {
        (SignMode::Off, _) => Ok(None),
        (SignMode::Auto, identity) => Ok(identity),
        (SignMode::Required, Some(identity)) => Ok(Some(identity)),
        (SignMode::Required, None) => Err(
            "sign = \"required\" needs a signing identity (COSIGN_KEY or ACTIONS_ID_TOKEN_REQUEST_URL); publishing nothing"
                .to_owned(),
        ),
    }
}

/// The bundle name for the asset called `base`.
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
pub fn sign_argv(cosign: &str, asset: &str, bundle: &str) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "sign-blob".to_owned(),
        "--yes".to_owned(),
        "--new-bundle-format".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        asset.to_owned(),
    ]
}

/// Formats `cosign sign-blob` with the key in `COSIGN_KEY`.
pub fn sign_key_argv(cosign: &str, asset: &str, bundle: &str) -> Vec<String> {
    let mut argv = sign_argv(cosign, asset, bundle);
    let at = argv.len() - 1;
    argv.splice(at..at, ["--key".to_owned(), "env://COSIGN_KEY".to_owned()]);
    argv
}

/// Formats `cosign verify-blob` against one certificate identity.
pub fn verify_argv(
    cosign: &str,
    asset: &str,
    bundle: &str,
    identity: &str,
    issuer: &str,
) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "verify-blob".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        "--certificate-identity".to_owned(),
        identity.to_owned(),
        "--certificate-oidc-issuer".to_owned(),
        issuer.to_owned(),
        asset.to_owned(),
    ]
}

/// Formats `cosign verify-blob` against a certificate identity regexp.
pub fn verify_regexp_argv(
    cosign: &str,
    asset: &str,
    bundle: &str,
    regexp: &str,
    issuer: &str,
) -> Vec<String> {
    let mut argv = verify_argv(cosign, asset, bundle, regexp, issuer);
    argv[4] = "--certificate-identity-regexp".to_owned();
    argv
}

/// The `GitVersion` line of `cosign version`.
pub fn cosign_git_version(version_output: &str) -> Option<&str> {
    version_output
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("GitVersion:"))
        .map(str::trim)
}

/// Whether `cosign version` printed exactly `want`.
pub fn signing_version_ok(version_output: &str, want: &str) -> bool {
    cosign_git_version(version_output) == Some(want)
}

/// Runs `argv` with the inherited environment and working directory.
pub fn run_output(argv: &[String]) -> Result<dx_process::capture::Captured, String> {
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| "invocation needs a binary".to_owned())?;
    let mut command = std::process::Command::new(binary);
    command.args(args);
    dx_process::capture::run_captured(&mut command, SIGN_TIMEOUT).map_err(|error| error.to_string())
}

/// Checks the cosign binary against its pinned sha256 and version.
pub fn check_pin(pin: &CosignPin<'_>) -> Result<(), String> {
    let actual = dx_digest::sha256_file_hex(Path::new(pin.path))
        .map_err(|error| format!("signing: cannot hash cosign at {}: {error}", pin.path))?;
    if actual != pin.sha256 {
        return Err(format!(
            "signing: cosign at {} has sha256 {actual}, want {}; refusing to sign",
            pin.path, pin.sha256
        ));
    }
    let argv = vec![pin.path.to_owned(), "version".to_owned()];
    let out = run_output(&argv)
        .map_err(|error| format!("signing: cannot run '{} version': {error}", pin.path))?;
    let text = out.stdout_text() + &String::from_utf8_lossy(&out.stderr);
    if !out.success() || !signing_version_ok(&text, pin.version) {
        return Err(format!(
            "signing: cosign GitVersion must be {}; got: {}",
            pin.version,
            cosign_git_version(&text).unwrap_or("none")
        ));
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
) -> Result<(), String> {
    let sign = if keyed {
        sign_key_argv(pin.path, asset, bundle)
    } else {
        sign_argv(pin.path, asset, bundle)
    };
    let out = run_output(&sign)
        .map_err(|error| format!("signing: cannot run '{}': {error}", sign.join(" ")))?;
    if !out.success() {
        return Err(format!(
            "signing: '{}' failed for {asset}; publishing nothing",
            sign.join(" ")
        ));
    }
    if let Some(verify) = verify {
        let out = run_output(&verify)
            .map_err(|error| format!("signing: cannot run '{}': {error}", verify.join(" ")))?;
        if !out.success() {
            return Err(format!(
                "signing: '{}' failed for {asset}; bundle rejected, publishing nothing",
                verify.join(" ")
            ));
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
) -> Result<Vec<PathBuf>, String> {
    check_pin(pin)?;
    std::fs::create_dir_all(outdir)
        .map_err(|error| format!("signing: cannot create {}: {error}", outdir.display()))?;
    let mut bundles = Vec::new();
    for asset in assets {
        let base = asset
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| format!("signing: {} has no file name", asset.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(plan_identity(SignMode::Auto, &none), Ok(None));
        assert_eq!(plan_identity(SignMode::Off, &key), Ok(None));
        assert_eq!(
            plan_identity(SignMode::Required, &key),
            Ok(Some(SignIdentity::Key))
        );
        let error = plan_identity(SignMode::Required, &none).expect_err("no identity");
        assert!(error.contains("needs a signing identity"), "{error}");
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
}
