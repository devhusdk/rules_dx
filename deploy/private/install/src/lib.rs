#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_methods,
        reason = "tests run local programs under the Bazel test timeout"
    )
)]

use std::fmt::{self, Write as _};
use std::io;
use std::path::{Path, PathBuf};

use dx_atomic_fs::path_io::{self, PathIoError};

pub const TRUST_ROOT: &str = "https://tuf-repo-cdn.sigstore.dev";
pub const DEFAULT_ISSUER: &str = "https://token.actions.githubusercontent.com";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VerifyArgs {
    pub binary: String,
    pub bundle: String,
    pub identity: String,
    pub issuer: String,
    pub attestation: String,
    pub owner: String,
    pub sbom: String,
    pub sbom_bundle: String,
    pub notice: String,
    pub notice_manifest: String,
    pub notice_bundle: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgsError(pub String);

impl std::fmt::Display for ArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ArgsError {}

/// Why `dx_verify` refused a binary or its signed SBOM and NOTICE.
#[derive(Debug)]
pub enum InstallError {
    /// A required input file is absent.
    InputNotFound {
        input: &'static str,
        path: String,
    },
    BundleEmpty(String),
    CosignFailed,
    OwnerRequired,
    AttestationFailed,
    NoVerifier {
        cosign: String,
        gh: String,
    },
    /// SBOM or NOTICE verification needs cosign, which is absent.
    CosignUnavailable {
        subject: &'static str,
        cosign: String,
    },
    /// The SBOM or NOTICE signature does not verify.
    SignatureFailed {
        subject: &'static str,
    },
    Unreadable {
        input: &'static str,
        path: String,
        source: PathIoError,
    },
    NoticeEmpty(String),
    NoticeManifest(dx_error::DxError),
    Notice(dx_release_tools::NoticeTextError),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("dx_verify: ")?;
        match self {
            Self::InputNotFound { input, path } => {
                write!(f, "{input} not found: {path}")?;
                if *input == "bundle" {
                    f.write_str(
                        " (missing, invalid, or unavailable verification inputs fail closed)",
                    )?;
                }
                Ok(())
            }
            Self::BundleEmpty(path) => write!(f, "bundle is empty: {path}"),
            Self::CosignFailed => f.write_str(
                "cosign verification failed (tampered bytes, replaced binary/bundle, or unapproved signer fail closed)",
            ),
            Self::OwnerRequired => f.write_str(
                "--owner OWNER is required with --attestation (GitHub attestation path)",
            ),
            Self::AttestationFailed => {
                f.write_str("attestation verification failed (fail closed before install)")
            }
            Self::NoVerifier { cosign, gh } => write!(
                f,
                "no verifier available (want {cosign} verify-blob or {gh} attestation verify); missing, invalid, or unavailable verification inputs fail closed"
            ),
            Self::CosignUnavailable { subject, cosign } => {
                write!(f, "{subject} verification needs {cosign} (unavailable)")
            }
            Self::SignatureFailed { subject } => {
                write!(f, "{subject} verification failed (fail closed before install)")
            }
            Self::Unreadable { input, path, .. } => write!(f, "{input} not readable: {path}"),
            Self::NoticeEmpty(path) => write!(f, "notice is empty: {path}"),
            Self::NoticeManifest(error) => error.fmt(f),
            Self::Notice(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::InputNotFound { .. }
            | Self::BundleEmpty(_)
            | Self::CosignFailed
            | Self::OwnerRequired
            | Self::AttestationFailed
            | Self::NoVerifier { .. }
            | Self::CosignUnavailable { .. }
            | Self::SignatureFailed { .. }
            | Self::NoticeEmpty(_)
            | Self::NoticeManifest(_) => None,
            Self::Notice(source) => Some(source),
        }
    }
}

fn help_text() -> String {
    "usage: dx_verify --binary PATH --bundle PATH --identity ID --issuer ISSUER [--attestation PATH] [--owner OWNER] [--sbom PATH --sbom-bundle PATH] [--notice PATH --notice-manifest PATH --notice-bundle PATH]".to_owned()
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn parse_args(argv: &[String]) -> Result<VerifyArgs, ArgsError> {
    let mut parsed = VerifyArgs::default();
    let mut i = 1;
    while let Some(flag) = argv.get(i) {
        let value = argv.get(i + 1).cloned().unwrap_or_default();
        let slot = match flag.as_str() {
            "--binary" => &mut parsed.binary,
            "--bundle" => &mut parsed.bundle,
            "--identity" => &mut parsed.identity,
            "--issuer" => &mut parsed.issuer,
            "--attestation" => &mut parsed.attestation,
            "--owner" => &mut parsed.owner,
            "--sbom" => &mut parsed.sbom,
            "--sbom-bundle" => &mut parsed.sbom_bundle,
            "--notice" => &mut parsed.notice,
            "--notice-manifest" => &mut parsed.notice_manifest,
            "--notice-bundle" => &mut parsed.notice_bundle,
            "--sha256" | "--checksum" | "--sha256-file" | "--checksum-file" => {
                return Err(ArgsError(
                    "dx_verify: checksum-only verification is not publisher-identity proof; pass --bundle plus --identity/--issuer".to_owned(),
                ));
            }
            "-h" | "--help" => {
                return Err(ArgsError(help_text()));
            }
            other => {
                return Err(ArgsError(format!(
                    "dx_verify: unknown argument '{other}'\n{}",
                    help_text()
                )));
            }
        };
        *slot = value;
        i += 2;
    }
    check_required(&parsed)?;
    Ok(parsed)
}

fn check_required(args: &VerifyArgs) -> Result<(), ArgsError> {
    if args.binary.is_empty() {
        return Err(ArgsError(
            "dx_verify: missing --binary PATH (standalone dx binary to verify)".to_owned(),
        ));
    }
    if args.bundle.is_empty() {
        return Err(ArgsError(
            "dx_verify: missing --bundle PATH; checksum-only verification is not publisher-identity proof".to_owned(),
        ));
    }
    if args.identity.is_empty() {
        return Err(ArgsError(
            "dx_verify: missing --identity ID (expected certificate identity, e.g. GitHub Actions workflow identity)".to_owned(),
        ));
    }
    if args.issuer.is_empty() {
        return Err(ArgsError(format!(
            "dx_verify: missing --issuer ISSUER (expected OIDC issuer, e.g. {DEFAULT_ISSUER})"
        )));
    }
    if !args.sbom.is_empty() && args.sbom_bundle.is_empty() {
        return Err(ArgsError(
            "dx_verify: --sbom needs --sbom-bundle (SBOM verifies through the same cosign path)"
                .to_owned(),
        ));
    }
    if args.sbom.is_empty() && !args.sbom_bundle.is_empty() {
        return Err(ArgsError(
            "dx_verify: --sbom-bundle needs --sbom".to_owned(),
        ));
    }
    if !args.notice.is_empty() && args.notice_manifest.is_empty() {
        return Err(ArgsError(
            "dx_verify: --notice needs --notice-manifest (NOTICE verifies against the audited inventory manifest)"
                .to_owned(),
        ));
    }
    if args.notice.is_empty() && !args.notice_manifest.is_empty() {
        return Err(ArgsError(
            "dx_verify: --notice-manifest needs --notice".to_owned(),
        ));
    }
    if !args.notice.is_empty() && args.notice_bundle.is_empty() {
        return Err(ArgsError(
            "dx_verify: --notice needs --notice-bundle (NOTICE verifies through the same cosign path)"
                .to_owned(),
        ));
    }
    if args.notice.is_empty() && !args.notice_bundle.is_empty() {
        return Err(ArgsError(
            "dx_verify: --notice-bundle needs --notice".to_owned(),
        ));
    }
    Ok(())
}

#[must_use]
pub fn cosign_argv(
    cosign: &str,
    bundle: &Path,
    identity: &str,
    issuer: &str,
    subject: &Path,
) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "verify-blob".to_owned(),
        "--bundle".to_owned(),
        bundle.to_string_lossy().into_owned(),
        "--certificate-identity".to_owned(),
        identity.to_owned(),
        "--certificate-oidc-issuer".to_owned(),
        issuer.to_owned(),
        subject.to_string_lossy().into_owned(),
    ]
}

#[must_use]
pub fn gh_argv(
    gh: &str,
    subject: &Path,
    attestation: &Path,
    identity: &str,
    issuer: &str,
    owner: &str,
) -> Vec<String> {
    vec![
        gh.to_owned(),
        "attestation".to_owned(),
        "verify".to_owned(),
        subject.to_string_lossy().into_owned(),
        "--bundle".to_owned(),
        attestation.to_string_lossy().into_owned(),
        "--cert-identity".to_owned(),
        identity.to_owned(),
        "--cert-oidc-issuer".to_owned(),
        issuer.to_owned(),
        "--owner".to_owned(),
        owner.to_owned(),
    ]
}

pub trait Verifier {
    fn is_nonempty_file(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn cosign_verify(
        &self,
        cosign: &str,
        bundle: &Path,
        identity: &str,
        issuer: &str,
        subject: &Path,
    ) -> bool;
    fn gh_verify(
        &self,
        gh: &str,
        subject: &Path,
        attestation: &Path,
        identity: &str,
        issuer: &str,
        owner: &str,
    ) -> bool;
    fn cosign_bin(&self) -> String;
    fn gh_bin(&self) -> String;
    fn have(&self, bin: &str) -> bool;
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    fn sha256_file(&self, path: &Path) -> io::Result<String>;
    /// # Errors
    ///
    /// Returns an error when the file cannot be read as UTF-8.
    fn read_text(&self, path: &Path) -> Result<String, PathIoError>;
}

pub struct SystemVerifier;

const SPAWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

fn spawn_success(argv: &[String]) -> bool {
    let Some((binary, rest)) = argv.split_first() else {
        return false;
    };
    let mut command = std::process::Command::new(binary);
    command.args(rest);
    dx_process::capture::run_captured(&mut command, SPAWN_TIMEOUT).is_ok_and(|out| out.success())
}

fn exe_available(bin: &str) -> bool {
    spawn_success(&[bin.to_owned(), "--help".to_owned()])
        || Path::new(bin).is_file()
        || std::env::var_os("PATH")
            .is_some_and(|paths| dx_path::lookup::find_command_in(&paths, bin).is_some())
}

impl Verifier for SystemVerifier {
    fn is_nonempty_file(&self, path: &Path) -> bool {
        std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 0)
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn cosign_verify(
        &self,
        cosign: &str,
        bundle: &Path,
        identity: &str,
        issuer: &str,
        subject: &Path,
    ) -> bool {
        spawn_success(&cosign_argv(cosign, bundle, identity, issuer, subject))
    }

    fn gh_verify(
        &self,
        gh: &str,
        subject: &Path,
        attestation: &Path,
        identity: &str,
        issuer: &str,
        owner: &str,
    ) -> bool {
        spawn_success(&gh_argv(gh, subject, attestation, identity, issuer, owner))
    }

    fn cosign_bin(&self) -> String {
        std::env::var("DX_VERIFY_COSIGN").unwrap_or_else(|_| "cosign".to_owned())
    }

    fn gh_bin(&self) -> String {
        std::env::var("DX_VERIFY_GH").unwrap_or_else(|_| "gh".to_owned())
    }

    fn have(&self, bin: &str) -> bool {
        exe_available(bin)
    }

    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        dx_digest::sha256_file_hex(path)
    }

    fn read_text(&self, path: &Path) -> Result<String, PathIoError> {
        path_io::read_to_string(path)
    }
}

fn verify_notice(notice_text: &str, manifest_text: &str) -> Result<usize, InstallError> {
    let entries = dx_release_tools::parse_notice_manifest(manifest_text)
        .map_err(InstallError::NoticeManifest)?;
    dx_release_tools::verify_notice_text(notice_text, &entries).map_err(InstallError::Notice)
}

fn check_inputs(
    args: &VerifyArgs,
    verifier: &dyn Verifier,
    binary: &Path,
    bundle: &Path,
) -> Result<(), InstallError> {
    let not_found = |input, path: &str| InstallError::InputNotFound {
        input,
        path: path.to_owned(),
    };
    if !verifier.is_file(binary) {
        return Err(not_found("binary", &args.binary));
    }
    if !verifier.is_file(bundle) {
        return Err(not_found("bundle", &args.bundle));
    }
    for (input, path) in [
        ("attestation", &args.attestation),
        ("sbom", &args.sbom),
        ("sbom bundle", &args.sbom_bundle),
        ("notice", &args.notice),
        ("notice manifest", &args.notice_manifest),
        ("notice bundle", &args.notice_bundle),
    ] {
        if !path.is_empty() && !verifier.is_file(Path::new(path)) {
            return Err(not_found(input, path));
        }
    }
    if !verifier.is_nonempty_file(bundle) {
        return Err(InstallError::BundleEmpty(args.bundle.clone()));
    }
    Ok(())
}

/// # Errors
///
/// Returns an error when the operation fails.
pub fn verify(request: &VerifyArgs, verifier: &dyn Verifier) -> Result<String, InstallError> {
    let mut out = String::new();
    let binary = PathBuf::from(&request.binary);
    let bundle = PathBuf::from(&request.bundle);
    check_inputs(request, verifier, &binary, &bundle)?;
    writeln!(out, "dx_verify: trust root {TRUST_ROOT} (Sigstore TUF public-good; verifiers bootstrapped from the trust root, never alongside the binary)").ok();
    let method = verify_binary(request, verifier, &binary, &bundle, &mut out)?;
    verify_sbom(request, verifier, &mut out)?;
    verify_notice_files(request, verifier, &mut out)?;
    writeln!(
        out,
        "dx_verify: OK (verified via {method}; nothing installed, binary never executed)"
    )
    .ok();
    Ok(out)
}

fn verify_binary(
    request: &VerifyArgs,
    verifier: &dyn Verifier,
    binary: &Path,
    bundle: &Path,
    out: &mut String,
) -> Result<&'static str, InstallError> {
    let cosign = verifier.cosign_bin();
    let gh = verifier.gh_bin();
    let method: &'static str;
    if verifier.have(&cosign) {
        writeln!(
            out,
            "dx_verify: verifying {} against bundle via {cosign} verify-blob",
            binary.display()
        )
        .ok();
        if verifier.cosign_verify(&cosign, bundle, &request.identity, &request.issuer, binary) {
            method = "cosign";
            writeln!(
                out,
                "dx_verify: cosign publisher-identity OK (identity={} issuer={})",
                request.identity, request.issuer
            )
            .ok();
        } else {
            return Err(InstallError::CosignFailed);
        }
    } else if !request.attestation.is_empty() && verifier.have(&gh) {
        if request.owner.is_empty() {
            return Err(InstallError::OwnerRequired);
        }
        writeln!(
            out,
            "dx_verify: verifying {} against attestation via {gh} attestation verify",
            binary.display()
        )
        .ok();
        let attestation = PathBuf::from(&request.attestation);
        if verifier.gh_verify(
            &gh,
            binary,
            &attestation,
            &request.identity,
            &request.issuer,
            &request.owner,
        ) {
            method = "gh-attestation";
            writeln!(
                out,
                "dx_verify: attestation publisher-identity OK (identity={} issuer={} owner={})",
                request.identity, request.issuer, request.owner
            )
            .ok();
        } else {
            return Err(InstallError::AttestationFailed);
        }
    } else {
        return Err(InstallError::NoVerifier { cosign, gh });
    }
    Ok(method)
}

fn verify_sbom(
    request: &VerifyArgs,
    verifier: &dyn Verifier,
    out: &mut String,
) -> Result<(), InstallError> {
    let cosign = verifier.cosign_bin();
    if !request.sbom.is_empty() {
        let sbom = PathBuf::from(&request.sbom);
        let sbom_bundle = PathBuf::from(&request.sbom_bundle);
        if !verifier.have(&cosign) {
            return Err(InstallError::CosignUnavailable {
                subject: "SBOM",
                cosign,
            });
        }
        writeln!(
            out,
            "dx_verify: verifying SBOM {} via {cosign} verify-blob",
            sbom.display()
        )
        .ok();
        if verifier.cosign_verify(
            &cosign,
            &sbom_bundle,
            &request.identity,
            &request.issuer,
            &sbom,
        ) {
            out.push_str("dx_verify: SBOM publisher-identity OK\n");
        } else {
            return Err(InstallError::SignatureFailed { subject: "SBOM" });
        }
    }
    Ok(())
}

fn verify_notice_files(
    request: &VerifyArgs,
    verifier: &dyn Verifier,
    out: &mut String,
) -> Result<(), InstallError> {
    let cosign = verifier.cosign_bin();
    if !request.notice.is_empty() {
        let notice_path = PathBuf::from(&request.notice);
        let notice_bundle = PathBuf::from(&request.notice_bundle);
        if !verifier.have(&cosign) {
            return Err(InstallError::CosignUnavailable {
                subject: "NOTICE",
                cosign,
            });
        }
        writeln!(
            out,
            "dx_verify: verifying NOTICE {} via {cosign} verify-blob",
            notice_path.display()
        )
        .ok();
        if verifier.cosign_verify(
            &cosign,
            &notice_bundle,
            &request.identity,
            &request.issuer,
            &notice_path,
        ) {
            out.push_str("dx_verify: NOTICE publisher-identity OK\n");
        } else {
            return Err(InstallError::SignatureFailed { subject: "NOTICE" });
        }
        let manifest_path = PathBuf::from(&request.notice_manifest);
        let unreadable = |input, path: &str| {
            let path = path.to_owned();
            move |source| InstallError::Unreadable {
                input,
                path,
                source,
            }
        };
        let notice_text = verifier
            .read_text(&notice_path)
            .map_err(unreadable("notice", &request.notice))?;
        if notice_text.trim().is_empty() {
            return Err(InstallError::NoticeEmpty(request.notice.clone()));
        }
        let manifest_text = verifier
            .read_text(&manifest_path)
            .map_err(unreadable("notice manifest", &request.notice_manifest))?;
        let count = verify_notice(&notice_text, &manifest_text)?;
        writeln!(out, "dx_verify: NOTICE OK ({count} entries bundled)").ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn self_exe() -> String {
        std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn spawn_success_reports_exit_status() {
        assert!(!spawn_success(&[]));
        assert!(!spawn_success(&["/nonexistent-dx-tool".to_owned()]));
        assert!(spawn_success(&[self_exe(), "--list".to_owned()]));
    }

    #[test]
    fn exe_available_covers_help_and_file() {
        assert!(!exe_available(""));
        assert!(!exe_available("/nonexistent-dx-tool-xyz"));
        assert!(exe_available(&self_exe()));
    }

    struct FakeVerifier {
        files: HashMap<PathBuf, Vec<u8>>,
        cosign_ok: bool,
        gh_ok: bool,
        absent: Vec<&'static str>,
        gh_policy: Option<(&'static str, &'static str)>,
    }

    impl FakeVerifier {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
                cosign_ok: true,
                gh_ok: true,
                absent: Vec::new(),
                gh_policy: None,
            }
        }

        fn write(&mut self, path: &str, data: &[u8]) {
            self.files.insert(PathBuf::from(path), data.to_vec());
        }
    }

    impl Verifier for FakeVerifier {
        fn is_nonempty_file(&self, path: &Path) -> bool {
            self.files.get(path).is_some_and(|data| !data.is_empty())
        }

        fn is_file(&self, path: &Path) -> bool {
            self.files.contains_key(path)
        }

        fn cosign_verify(
            &self,
            _cosign: &str,
            bundle: &Path,
            _identity: &str,
            _issuer: &str,
            subject: &Path,
        ) -> bool {
            if !self.cosign_ok {
                return false;
            }
            let subject_data = self.files.get(subject);
            let bundle_data = self.files.get(bundle);
            match (subject_data, bundle_data) {
                (Some(subject_bytes), Some(bundle_bytes)) => {
                    bundle_bytes
                        == format!("bundle-for-{}", dx_digest::sha256_hex(subject_bytes)).as_bytes()
                }
                _ => false,
            }
        }

        fn gh_verify(
            &self,
            _gh: &str,
            _subject: &Path,
            _attestation: &Path,
            identity: &str,
            issuer: &str,
            _owner: &str,
        ) -> bool {
            self.gh_ok
                && self
                    .gh_policy
                    .as_ref()
                    .is_none_or(|p| *p == (identity, issuer))
        }

        fn cosign_bin(&self) -> String {
            "cosign".to_owned()
        }

        fn gh_bin(&self) -> String {
            "gh".to_owned()
        }

        fn have(&self, bin: &str) -> bool {
            !self.absent.contains(&bin)
        }

        fn sha256_file(&self, path: &Path) -> io::Result<String> {
            let data = self
                .files
                .get(path)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing file"))?;
            Ok(dx_digest::sha256_hex(data))
        }

        fn read_text(&self, path: &Path) -> Result<String, PathIoError> {
            let read_error = |kind, detail| PathIoError::Read {
                path: path.to_path_buf(),
                source: io::Error::new(kind, detail),
            };
            let data = self
                .files
                .get(path)
                .ok_or_else(|| read_error(io::ErrorKind::NotFound, "missing file"))?;
            String::from_utf8(data.clone())
                .map_err(|_| read_error(io::ErrorKind::InvalidData, "not UTF-8"))
        }
    }

    fn valid_args() -> VerifyArgs {
        VerifyArgs {
            binary: "/tmp/dx-fake".to_owned(),
            bundle: "/tmp/dx-fake.bundle".to_owned(),
            identity: "IDENT".to_owned(),
            issuer: "ISSUER".to_owned(),
            ..Default::default()
        }
    }

    fn valid_fake() -> FakeVerifier {
        let mut fake = FakeVerifier::new();
        let subject = b"standalone-dx-bytes-v1";
        fake.write("/tmp/dx-fake", subject);
        let digest = dx_digest::sha256_hex(subject);
        fake.write(
            "/tmp/dx-fake.bundle",
            format!("bundle-for-{digest}").as_bytes(),
        );
        fake
    }

    #[test]
    fn verifier_argv_are_thin_configs_over_spawn_owner() {
        assert_eq!(
            cosign_argv("cosign", Path::new("/u"), "ID", "ISS", Path::new("/b"),),
            vec![
                "cosign",
                "verify-blob",
                "--bundle",
                "/u",
                "--certificate-identity",
                "ID",
                "--certificate-oidc-issuer",
                "ISS",
                "/b",
            ]
        );
        assert_eq!(
            gh_argv("gh", Path::new("/b"), Path::new("/a"), "ID", "ISS", "owner",),
            vec![
                "gh",
                "attestation",
                "verify",
                "/b",
                "--bundle",
                "/a",
                "--cert-identity",
                "ID",
                "--cert-oidc-issuer",
                "ISS",
                "--owner",
                "owner",
            ]
        );
    }

    #[test]
    fn parses_required_flags() {
        let argv = [
            "dx_verify",
            "--binary",
            "/b",
            "--bundle",
            "/u",
            "--identity",
            "ID",
            "--issuer",
            "ISS",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        let parsed = parse_args(&argv).expect("parse");
        assert_eq!(parsed.binary, "/b");
        assert_eq!(parsed.bundle, "/u");
        assert_eq!(parsed.identity, "ID");
        assert_eq!(parsed.issuer, "ISS");
    }

    #[test]
    fn rejects_checksum_only_and_missing_inputs() {
        let base = [
            "dx_verify",
            "--binary",
            "/b",
            "--bundle",
            "/u",
            "--identity",
            "ID",
            "--issuer",
            "ISS",
        ];
        let with_sha = base
            .iter()
            .chain(["--sha256", "/x"].iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let err = parse_args(&with_sha).expect_err("sha rejected");
        assert!(err.0.contains("checksum-only"));
        for (flag, want) in [
            ("--binary", "missing --binary"),
            ("--bundle", "missing --bundle"),
            ("--identity", "missing --identity"),
            ("--issuer", "missing --issuer"),
        ] {
            let mut argv: Vec<String> = base.iter().map(ToString::to_string).collect();
            let pos = argv.iter().position(|arg| arg == flag).expect("flag");
            argv.drain(pos..pos + 2);
            let err = parse_args(&argv).expect_err("missing rejected");
            assert!(err.0.contains(want), "want {want}, got {}", err.0);
        }
        let sbom_only = [
            "dx_verify",
            "--binary",
            "/b",
            "--bundle",
            "/u",
            "--identity",
            "ID",
            "--issuer",
            "ISS",
            "--sbom",
            "/s",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        assert!(parse_args(&sbom_only).is_err());
        let notice_only = [
            "dx_verify",
            "--binary",
            "/b",
            "--bundle",
            "/u",
            "--identity",
            "ID",
            "--issuer",
            "ISS",
            "--notice",
            "/n",
        ]
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
        assert!(parse_args(&notice_only).is_err());
    }

    #[test]
    fn accepts_valid_and_installs_without_exec() {
        let args = valid_args();
        let fake = valid_fake();
        let out = verify(&args, &fake).expect("valid");
        assert!(out.contains("tuf-repo-cdn.sigstore.dev"));
        assert!(out.contains("cosign publisher-identity OK"));
        assert!(out.contains("nothing installed, binary never executed"));
    }

    #[test]
    fn rejects_tampered_and_unapproved_signer() {
        let args = valid_args();
        let mut fake = valid_fake();
        fake.write("/tmp/dx-fake", b"tampered");
        assert!(verify(&args, &fake).is_err());
        let mut fake = valid_fake();
        fake.cosign_ok = false;
        assert!(verify(&args, &fake).is_err());
    }

    #[test]
    fn fails_closed_without_verifier() {
        let args = valid_args();
        let mut fake = valid_fake();
        fake.absent = vec!["cosign", "gh"];
        let err = verify(&args, &fake).expect_err("no verifier");
        assert!(matches!(err, InstallError::NoVerifier { .. }), "{err:?}");
        assert!(err.to_string().contains("no verifier available"));
    }

    #[test]
    fn binds_sbom_bytes() {
        let mut args = valid_args();
        args.sbom = "/tmp/sbom".to_owned();
        args.sbom_bundle = "/tmp/sbom.bundle".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/sbom", b"sbom-bytes");
        let digest = dx_digest::sha256_hex(b"sbom-bytes");
        fake.write(
            "/tmp/sbom.bundle",
            format!("bundle-for-{digest}").as_bytes(),
        );
        let out = verify(&args, &fake).expect("sbom valid");
        assert!(out.contains("SBOM publisher-identity OK"));
        fake.write("/tmp/sbom", b"tampered-sbom");
        assert!(verify(&args, &fake).is_err());
    }

    const NOTICE_WORDS_A: &str = "Fixture words for demo-lib-a.\n";
    const NOTICE_WORDS_B: &str = "Fixture words for demo-lib-b.\n";

    fn notice_manifest_text() -> String {
        format!(
            "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt|{}\ndemo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt|{}\n",
            dx_release_tools::notice_text_sha256(NOTICE_WORDS_A),
            dx_release_tools::notice_text_sha256(NOTICE_WORDS_B)
        )
    }

    fn notice_text() -> String {
        format!(
            "NOTICE for //demo:root\nGenerated by rules_dx notice_bundle from the audited license inventory; do not edit.\n\n=== demo-lib-a 1.0.0 (MIT) [cargo] ===\n{NOTICE_WORDS_A}=== demo-lib-b 2.3.4 (Apache-2.0) [npm] ===\n{NOTICE_WORDS_B}"
        )
    }

    /// Writes the NOTICE bytes plus a bundle the fake cosign accepts for them.
    fn write_signed_notice(fake: &mut FakeVerifier, bytes: &[u8]) {
        fake.write("/tmp/NOTICE", bytes);
        let digest = dx_digest::sha256_hex(bytes);
        fake.write(
            "/tmp/NOTICE.bundle",
            format!("bundle-for-{digest}").as_bytes(),
        );
    }

    fn notice_fake() -> (VerifyArgs, FakeVerifier) {
        let mut request = valid_args();
        request.notice = "/tmp/NOTICE".to_owned();
        request.notice_manifest = "/tmp/inventory.txt".to_owned();
        request.notice_bundle = "/tmp/NOTICE.bundle".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/inventory.txt", notice_manifest_text().as_bytes());
        write_signed_notice(&mut fake, notice_text().as_bytes());
        (request, fake)
    }

    #[test]
    fn binds_notice_to_manifest() {
        let (request, fake) = notice_fake();
        let out = verify(&request, &fake).expect("notice valid");
        assert!(out.contains("NOTICE publisher-identity OK"));
        assert!(out.contains("NOTICE OK (2 entries bundled)"));
    }

    fn notice_err(edit: impl FnOnce(&mut FakeVerifier)) -> String {
        let (request, mut fake) = notice_fake();
        edit(&mut fake);
        verify(&request, &fake)
            .expect_err("notice must fail closed")
            .to_string()
    }

    #[test]
    fn notice_headers_without_words_fail() {
        let err = notice_err(|fake| {
            write_signed_notice(
                fake,
                b"NOTICE for //demo:root\n=== demo-lib-a 1.0.0 (MIT) [cargo] ===\n=== demo-lib-b 2.3.4 (Apache-2.0) [npm] ===\n",
            );
        });
        assert!(err.contains("entry body is empty"), "got {err}");
    }

    #[test]
    fn notice_body_digest_mismatch_fails() {
        let err = notice_err(|fake| {
            let edited = notice_text().replace(
                "Fixture words for demo-lib-a",
                "Fixture word for demo-lib-a",
            );
            write_signed_notice(fake, edited.as_bytes());
        });
        assert!(
            err.contains("NOTICE missing words for demo-lib-a@1.0.0 (MIT)"),
            "got {err}"
        );
        assert!(err.contains("manifest records"), "got {err}");
    }

    #[test]
    fn notice_extra_section_fails() {
        let err = notice_err(|fake| {
            let extra = format!(
                "{}=== other 1.0.0 (MIT) [cargo] ===\nSneaky.\n",
                notice_text()
            );
            write_signed_notice(fake, extra.as_bytes());
        });
        assert!(err.contains("NOTICE missing words"), "got {err}");
        let err = notice_err(|fake| {
            let edited = notice_text().replacen("do not edit.", "do not edit. More.", 1);
            write_signed_notice(fake, edited.as_bytes());
        });
        assert!(err.contains("outside the audited entries"), "got {err}");
    }

    #[test]
    fn notice_without_valid_bundle_fails() {
        let mut argv = base_argv();
        argv.extend(["--notice", "/n", "--notice-manifest", "/nm"].map(str::to_owned));
        let err = parse_args(&argv).expect_err("no bundle flag");
        assert!(
            err.to_string().contains("--notice needs --notice-bundle"),
            "got {err}"
        );
        let mut argv = base_argv();
        argv.extend(["--notice-bundle", "/nb"].map(str::to_owned));
        let err = parse_args(&argv).expect_err("unpaired bundle");
        assert!(
            err.to_string().contains("--notice-bundle needs --notice"),
            "got {err}"
        );

        let err = notice_err(|fake| fake.write("/tmp/NOTICE.bundle", b"bundle-for-wrong"));
        assert!(err.contains("NOTICE verification failed"), "got {err}");
        let (mut request, mut fake) = notice_fake();
        request.attestation = "/tmp/dx-fake.attestation".to_owned();
        request.owner = "owner".to_owned();
        fake.write("/tmp/dx-fake.attestation", b"attestation-bytes");
        fake.absent = vec!["cosign"];
        fake.gh_ok = true;
        let err = verify(&request, &fake)
            .expect_err("no cosign for NOTICE")
            .to_string();
        assert!(err.contains("needs cosign"), "got {err}");
        let err = notice_err(|fake| {
            fake.files.remove(Path::new("/tmp/NOTICE.bundle"));
        });
        assert!(err.contains("notice bundle not found"), "got {err}");

        let (request, mut fake) = notice_fake();
        fake.write("/tmp/NOTICE", &[0xff]);
        let err = verify(&request, &fake)
            .expect_err("bad signature")
            .to_string();
        assert!(err.contains("NOTICE verification failed"), "got {err}");
    }

    #[test]
    fn notice_missing_text_fails_actionable() {
        let (request, mut fake) = notice_fake();
        fake.write("/tmp/inventory.txt", b"demo-lib-a|cargo|1.0.0|MIT||\n");
        let err = verify(&request, &fake)
            .expect_err("missing text")
            .to_string();
        assert!(err.contains("missing-notice-text"), "got {err}");
        let (request, mut fake) = notice_fake();
        write_signed_notice(&mut fake, b"tampered notice\n");
        let err = verify(&request, &fake)
            .expect_err("tampered notice")
            .to_string();
        assert!(err.contains("NOTICE"), "got {err}");
    }

    fn base_argv() -> Vec<String> {
        [
            "dx_verify",
            "--binary",
            "/b",
            "--bundle",
            "/u",
            "--identity",
            "ID",
            "--issuer",
            "ISS",
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn parses_optional_flags_and_renders_arg_diagnostics() {
        let mut argv = base_argv();
        for (flag, value) in [
            ("--attestation", "/a"),
            ("--owner", "owner"),
            ("--sbom", "/s"),
            ("--sbom-bundle", "/sb"),
            ("--notice", "/n"),
            ("--notice-manifest", "/nm"),
            ("--notice-bundle", "/nb"),
        ] {
            argv.push(flag.to_owned());
            argv.push(value.to_owned());
        }
        let parsed = parse_args(&argv).expect("all flags parse");
        assert_eq!(parsed.attestation, "/a");
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.sbom_bundle, "/sb");
        assert_eq!(parsed.notice_manifest, "/nm");
        assert_eq!(parsed.notice_bundle, "/nb");
        let help = parse_args(&["dx_verify".to_owned(), "--help".to_owned()]).expect_err("help");
        assert!(help.to_string().contains("usage: dx_verify --binary PATH"));
        let unknown =
            parse_args(&["dx_verify".to_owned(), "--frobnicate".to_owned()]).expect_err("unknown");
        let rendered = unknown.to_string();
        assert!(
            rendered.contains("unknown argument '--frobnicate'"),
            "got {rendered}"
        );
        assert!(rendered.contains("usage: dx_verify"));
        assert_eq!(ArgsError("boom".to_owned()).to_string(), "boom");
        for (flag, want) in [
            ("--sbom-bundle", "--sbom-bundle needs --sbom"),
            ("--notice-manifest", "--notice-manifest needs --notice"),
            ("--notice-bundle", "--notice-bundle needs --notice"),
        ] {
            let mut unpaired = base_argv();
            unpaired.push(flag.to_owned());
            unpaired.push("/value".to_owned());
            let err = parse_args(&unpaired).expect_err("unpaired flag");
            assert!(err.to_string().contains(want), "want {want}, got {err}");
        }
    }

    #[test]
    fn verify_fails_closed_on_missing_or_empty_inputs() {
        let check = |request: VerifyArgs, fake: FakeVerifier, want: &str| {
            let err = verify(&request, &fake).expect_err(want).to_string();
            assert!(err.contains(want), "want {want}, got {err}");
        };
        let mut fake = valid_fake();
        fake.files.remove(Path::new("/tmp/dx-fake"));
        check(valid_args(), fake, "binary not found");
        let mut fake = valid_fake();
        fake.files.remove(Path::new("/tmp/dx-fake.bundle"));
        check(valid_args(), fake, "bundle not found");
        let mut request = valid_args();
        request.attestation = "/tmp/dx-fake.attestation".to_owned();
        check(request, valid_fake(), "attestation not found");
        let mut request = valid_args();
        request.sbom = "/tmp/sbom".to_owned();
        check(request, valid_fake(), "sbom not found");
        let mut request = valid_args();
        request.sbom = "/tmp/sbom".to_owned();
        request.sbom_bundle = "/tmp/sbom.bundle".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/sbom", b"sbom-bytes");
        check(request, fake, "sbom bundle not found");
        let mut request = valid_args();
        request.notice = "/tmp/NOTICE".to_owned();
        check(request, valid_fake(), "notice not found");
        let mut request = valid_args();
        request.notice = "/tmp/NOTICE".to_owned();
        request.notice_manifest = "/tmp/inventory.txt".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/NOTICE", b"NOTICE for //demo:root\n");
        check(request, fake, "notice manifest not found");
        let mut request = valid_args();
        request.notice = "/tmp/NOTICE".to_owned();
        request.notice_manifest = "/tmp/inventory.txt".to_owned();
        request.notice_bundle = "/tmp/NOTICE.bundle".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/NOTICE", b"NOTICE for //demo:root\n");
        fake.write("/tmp/inventory.txt", b"# empty\n");
        check(request, fake, "notice bundle not found");
        let mut fake = valid_fake();
        fake.write("/tmp/dx-fake.bundle", b"");
        check(valid_args(), fake, "bundle is empty");
        let fake = valid_fake();
        assert!(!fake.cosign_verify(
            "cosign",
            Path::new("/tmp/missing.bundle"),
            "ID",
            "ISS",
            Path::new("/tmp/dx-fake")
        ));
    }

    #[test]
    fn attestation_path_verifies_without_cosign() {
        let attestation_args = || {
            let mut args = valid_args();
            args.attestation = "/tmp/dx-fake.attestation".to_owned();
            args.owner = "owner".to_owned();
            args
        };
        let attestation_fake = |cosign: bool, gh_ok: bool| {
            let mut fake = valid_fake();
            if !cosign {
                fake.absent.push("cosign");
            }
            fake.gh_ok = gh_ok;
            fake.write("/tmp/dx-fake.attestation", b"attestation-bytes");
            fake
        };
        let out = verify(&attestation_args(), &attestation_fake(false, true)).expect("gh path");
        assert!(out.contains(
            "attestation publisher-identity OK (identity=IDENT issuer=ISSUER owner=owner)"
        ));
        let mut no_owner = attestation_args();
        no_owner.owner.clear();
        let err = verify(&no_owner, &attestation_fake(false, true))
            .expect_err("owner required")
            .to_string();
        assert!(err.contains("--owner OWNER is required"), "got {err}");
        let err = verify(&attestation_args(), &attestation_fake(false, false))
            .expect_err("attestation rejected")
            .to_string();
        assert!(err.contains("attestation verification failed"), "got {err}");
        let mut wrong_signer = attestation_fake(false, true);
        wrong_signer.gh_policy = Some(("OTHER", "ISSUER"));
        let err = verify(&attestation_args(), &wrong_signer)
            .expect_err("identity mismatch")
            .to_string();
        assert!(err.contains("attestation verification failed"), "got {err}");
        let mut wrong_issuer = attestation_fake(false, true);
        wrong_issuer.gh_policy = Some(("IDENT", "OTHER"));
        verify(&attestation_args(), &wrong_issuer).expect_err("issuer mismatch");
        let mut right_signer = attestation_fake(false, true);
        right_signer.gh_policy = Some(("IDENT", "ISSUER"));
        verify(&attestation_args(), &right_signer).expect("policy forwarded");
        let mut with_sbom = attestation_args();
        with_sbom.sbom = "/tmp/sbom".to_owned();
        with_sbom.sbom_bundle = "/tmp/sbom.bundle".to_owned();
        let mut fake = attestation_fake(false, true);
        fake.write("/tmp/sbom", b"sbom-bytes");
        fake.write("/tmp/sbom.bundle", b"sbom-bundle");
        let err = verify(&with_sbom, &fake)
            .expect_err("no cosign for sbom")
            .to_string();
        assert!(err.contains("SBOM verification needs cosign"), "got {err}");
    }

    #[test]
    fn notice_manifest_failures_are_actionable() {
        let words = "Fixture words for demo-lib-a.\n";
        let digest = dx_release_tools::notice_text_sha256(words);
        let manifest =
            &format!("# audited inventory\n\ndemo-lib-a|cargo|1.0.0|MIT|demo.txt|{digest}\n");
        let notice = dx_release_tools::render_notice(
            "//demo:root",
            &[(
                dx_release_tools::NoticeEntry {
                    package: "demo-lib-a".to_owned(),
                    set: "cargo".to_owned(),
                    version: "1.0.0".to_owned(),
                    license: "MIT".to_owned(),
                    text_path: "demo.txt".to_owned(),
                    text_sha256: digest.clone(),
                },
                words.to_owned(),
            )],
        );
        assert_eq!(
            verify_notice(&notice, manifest).expect("generated NOTICE verifies"),
            1
        );
        let err = verify_notice(&notice, "demo-lib-a|cargo|1.0.0\n")
            .expect_err("field count")
            .to_string();
        assert!(err.contains("want 6 '|' fields"), "got {err}");
        let err = verify_notice(&notice, &format!(" |cargo|1.0.0|MIT|demo.txt|{digest}\n"))
            .expect_err("no package")
            .to_string();
        assert!(err.contains("must be non-empty"), "got {err}");
        let err = verify_notice(
            &notice,
            &format!("demo-lib-a||1.0.0|MIT|demo.txt|{digest}\n"),
        )
        .expect_err("no set")
        .to_string();
        assert!(err.contains("must be non-empty"), "got {err}");
        let err =
            verify_notice(&notice, "demo-lib-a|cargo|1.0.0|MIT||\n").expect_err("no text file");
        assert!(
            matches!(
                err,
                InstallError::Notice(dx_release_tools::NoticeTextError::NoText { .. })
            ),
            "got {err:?}"
        );
        let err = err.to_string();
        assert!(err.contains("missing-notice-text"), "got {err}");
        let err = verify_notice(
            &notice,
            &format!("demo-lib-a|cargo|1.0.0|MIT|../demo.txt|{digest}\n"),
        )
        .expect_err("unsafe text path")
        .to_string();
        assert!(err.contains("text '../demo.txt'"), "got {err}");
        let err = verify_notice(
            &notice,
            &format!("{manifest}demo-lib-a|cargo|1.0.0|MIT|other/demo.txt|{digest}\n"),
        )
        .expect_err("duplicate identity")
        .to_string();
        assert!(
            err.contains("duplicate entry demo-lib-a@1.0.0 (MIT) [cargo]"),
            "got {err}"
        );
        let err = verify_notice("NOTICE for //demo:root\n", "# no packages\n")
            .expect_err("empty manifest");
        assert!(
            matches!(
                err,
                InstallError::Notice(dx_release_tools::NoticeTextError::NoEntries)
            ),
            "got {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "dx_verify: notice manifest lists no packages; refusing empty NOTICE"
        );
        let err = verify_notice("NOTICE for //demo:root\n", manifest).expect_err("missing entry");
        assert!(
            matches!(
                err,
                InstallError::Notice(dx_release_tools::NoticeTextError::MissingHeader { .. })
            ),
            "got {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "dx_verify: NOTICE missing entry header '=== demo-lib-a 1.0.0 (MIT) [cargo] ===' (fail closed before install)"
        );
    }

    #[test]
    fn notice_inputs_fail_closed_when_unreadable() {
        let (request, mut fake) = notice_fake();
        write_signed_notice(&mut fake, b"   \n");
        let err = verify(&request, &fake)
            .expect_err("empty notice")
            .to_string();
        assert!(err.contains("notice is empty"), "got {err}");
        let (request, mut fake) = notice_fake();
        fake.write("/tmp/inventory.txt", &[0xff, 0xfe]);
        let err = verify(&request, &fake)
            .expect_err("manifest not utf-8")
            .to_string();
        assert!(err.contains("notice manifest not readable"), "got {err}");
        let (request, mut fake) = notice_fake();
        write_signed_notice(&mut fake, &[0xff]);
        let err = verify(&request, &fake).expect_err("notice not utf-8");
        assert!(
            matches!(
                err,
                InstallError::Unreadable {
                    input: "notice",
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(std::error::Error::source(&err).is_some());
        assert!(err.to_string().contains("notice not readable"), "got {err}");
    }

    #[test]
    fn system_verifier_io_uses_real_files() {
        let scratch = tempfile::tempdir().expect("scratch");
        let file = scratch.path().join("dx");
        std::fs::write(&file, b"standalone-dx-bytes-v1").expect("write file");
        let empty = scratch.path().join("empty");
        std::fs::write(&empty, b"").expect("write empty");
        let verifier = SystemVerifier;
        assert!(verifier.is_file(&file));
        assert!(!verifier.is_file(&scratch.path().join("missing")));
        assert!(verifier.is_nonempty_file(&file));
        assert!(!verifier.is_nonempty_file(&empty));
        assert!(!verifier.is_nonempty_file(scratch.path()));
        assert!(!verifier.is_nonempty_file(&scratch.path().join("missing")));
        assert_eq!(
            verifier.sha256_file(&file).expect("hash"),
            dx_digest::sha256_hex(b"standalone-dx-bytes-v1")
        );
        assert!(
            verifier
                .sha256_file(&scratch.path().join("missing"))
                .is_err()
        );
        assert_eq!(
            verifier.read_text(&file).expect("read"),
            "standalone-dx-bytes-v1"
        );
        let missing = scratch.path().join("missing");
        let error = verifier.read_text(&missing).expect_err("missing");
        assert_eq!(error.path(), missing);
        assert!(!verifier.have("/nonexistent-dx-tool"));
        #[cfg(unix)]
        {
            assert!(verifier.have("/usr/bin/true"));
            assert!(verifier.cosign_verify("/usr/bin/true", &file, "ID", "ISS", &file));
            assert!(!verifier.cosign_verify("/usr/bin/false", &file, "ID", "ISS", &file));
            assert!(verifier.gh_verify("/usr/bin/true", &file, &file, "ID", "ISS", "owner"));
            assert!(!verifier.gh_verify("/usr/bin/false", &file, &file, "ID", "ISS", "owner"));
        }
    }

    #[test]
    #[expect(
        unsafe_code,
        reason = "set_var and remove_var are unsafe in edition 2024"
    )]
    fn system_verifier_bin_names_follow_env_overrides() {
        if let Some(raw) = std::env::var_os("DX_INSTALL_VERIFY_CHILD") {
            let raw = raw.to_string_lossy().into_owned();
            let (mode, sentinel) = raw.split_once(':').expect("mode:path");
            // SAFETY: the process is single-threaded here or the env lock is held.
            unsafe { std::env::set_var("DX_VERIFY_COSIGN", "pinned-cosign") };
            // SAFETY: the process is single-threaded here or the env lock is held.
            unsafe { std::env::set_var("DX_VERIFY_GH", "pinned-gh") };
            let verifier = SystemVerifier;
            assert_eq!(verifier.cosign_bin(), "pinned-cosign");
            assert_eq!(verifier.gh_bin(), "pinned-gh");
            assert!(mode != "panic", "intentional child failure");
            std::fs::write(sentinel, b"ok").expect("sentinel");
            // SAFETY: the process is single-threaded here or the env lock is held.
            unsafe { std::env::remove_var("DX_VERIFY_COSIGN") };
            // SAFETY: the process is single-threaded here or the env lock is held.
            unsafe { std::env::remove_var("DX_VERIFY_GH") };
            return;
        }
        let verifier = SystemVerifier;
        assert_eq!(verifier.cosign_bin(), "cosign");
        assert_eq!(verifier.gh_bin(), "gh");
        let before = (
            std::env::var_os("DX_VERIFY_COSIGN"),
            std::env::var_os("DX_VERIFY_GH"),
        );
        let scratch = tempfile::tempdir().expect("scratch");
        let sentinel = scratch.path().join("ran");
        let spawn = |mode: &str| {
            std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "tests::system_verifier_bin_names_follow_env_overrides",
                    "--exact",
                ])
                .env(
                    "DX_INSTALL_VERIFY_CHILD",
                    format!("{mode}:{}", sentinel.display()),
                )
                .output()
                .expect("spawn env child")
        };
        let poisoned = spawn("panic");
        assert!(
            !poisoned.status.success(),
            "panicking child must fail: {}",
            String::from_utf8_lossy(&poisoned.stdout)
        );
        assert_eq!(
            (
                std::env::var_os("DX_VERIFY_COSIGN"),
                std::env::var_os("DX_VERIFY_GH")
            ),
            before,
            "child env mutation dies with the child"
        );
        let ok = spawn("run");
        assert!(
            ok.status.success(),
            "child failed: {}{}",
            String::from_utf8_lossy(&ok.stdout),
            String::from_utf8_lossy(&ok.stderr)
        );
        assert!(sentinel.exists(), "child ran the assertions");
        assert_eq!(
            (
                std::env::var_os("DX_VERIFY_COSIGN"),
                std::env::var_os("DX_VERIFY_GH")
            ),
            before,
            "child env mutation dies with the child"
        );
        scratch.close().expect("cleanup");
    }
}
