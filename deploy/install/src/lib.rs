#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io;
use std::path::{Path, PathBuf};

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
    pub install_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgsError(pub String);

impl std::fmt::Display for ArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ArgsError {}

fn help_text() -> String {
    "usage: dx_verify --binary PATH --bundle PATH --identity ID --issuer ISSUER [--attestation PATH] [--owner OWNER] [--sbom PATH --sbom-bundle PATH] [--notice PATH --notice-manifest PATH] [--install-dir DIR]".to_owned()
}

pub fn parse_args(argv: &[String]) -> Result<VerifyArgs, ArgsError> {
    let mut args = VerifyArgs::default();
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--binary" => {
                args.binary = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--bundle" => {
                args.bundle = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--identity" => {
                args.identity = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--issuer" => {
                args.issuer = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--attestation" => {
                args.attestation = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--owner" => {
                args.owner = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--sbom" => {
                args.sbom = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--sbom-bundle" => {
                args.sbom_bundle = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--notice" => {
                args.notice = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--notice-manifest" => {
                args.notice_manifest = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--install-dir" => {
                args.install_dir = argv.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
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
        }
    }
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
    Ok(args)
}

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
        "--certificate-issuer".to_owned(),
        issuer.to_owned(),
        subject.to_string_lossy().into_owned(),
    ]
}

pub fn gh_argv(gh: &str, subject: &Path, owner: &str) -> Vec<String> {
    vec![
        gh.to_owned(),
        "attestation".to_owned(),
        "verify".to_owned(),
        subject.to_string_lossy().into_owned(),
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
    fn gh_verify(&self, gh: &str, subject: &Path, owner: &str) -> bool;
    fn cosign_bin(&self) -> String;
    fn gh_bin(&self) -> String;
    fn have(&self, bin: &str) -> bool;
    fn sha256_file(&self, path: &Path) -> io::Result<String>;
    fn install_copy(&self, src: &Path, dst: &Path) -> io::Result<()>;
    fn read_text(&self, path: &Path) -> Option<String>;
}

pub struct SystemVerifier;

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
        dx_process::spawn_success(&cosign_argv(cosign, bundle, identity, issuer, subject))
    }

    fn gh_verify(&self, gh: &str, subject: &Path, owner: &str) -> bool {
        dx_process::spawn_success(&gh_argv(gh, subject, owner))
    }

    fn cosign_bin(&self) -> String {
        std::env::var("DX_VERIFY_COSIGN").unwrap_or_else(|_| "cosign".to_owned())
    }

    fn gh_bin(&self) -> String {
        std::env::var("DX_VERIFY_GH").unwrap_or_else(|_| "gh".to_owned())
    }

    fn have(&self, bin: &str) -> bool {
        dx_process::exe_available(bin)
    }

    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        dx_digest::sha256_file_hex(path)
    }

    fn install_copy(&self, src: &Path, dst: &Path) -> io::Result<()> {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(dst, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    fn read_text(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

fn verify_notice(notice_text: &str, manifest_text: &str) -> Result<usize, String> {
    let entries = dx_release_tools::parse_notice_manifest(manifest_text)
        .map_err(|error| format!("dx_verify: {error}"))?;
    if entries.is_empty() {
        return Err(
            "dx_verify: notice manifest lists no packages; refusing empty NOTICE".to_owned(),
        );
    }
    if !notice_text.starts_with("NOTICE for ") {
        return Err("dx_verify: NOTICE missing 'NOTICE for <root>' header".to_owned());
    }
    for entry in &entries {
        if entry.text_basename.is_empty() {
            return Err(format!(
                "dx_verify: missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words; record the words in the audited inventory before bundling",
                entry.package, entry.version, entry.license
            ));
        }
        let header = dx_release_tools::notice_entry_header(entry);
        if !notice_text.contains(&header) {
            return Err(format!(
                "dx_verify: NOTICE missing entry '{header}' (fail closed before install)"
            ));
        }
    }
    Ok(entries.len())
}

pub fn verify(args: &VerifyArgs, verifier: &dyn Verifier) -> Result<String, String> {
    let mut out = String::new();
    let binary = PathBuf::from(&args.binary);
    let bundle = PathBuf::from(&args.bundle);
    if !verifier.is_file(&binary) {
        return Err(format!("dx_verify: binary not found: {}", args.binary));
    }
    if !verifier.is_file(&bundle) {
        return Err(format!(
            "dx_verify: bundle not found: {} (missing, invalid, or unavailable verification inputs fail closed)",
            args.bundle
        ));
    }
    if !args.attestation.is_empty() && !verifier.is_file(Path::new(&args.attestation)) {
        return Err(format!(
            "dx_verify: attestation not found: {}",
            args.attestation
        ));
    }
    if !args.sbom.is_empty() && !verifier.is_file(Path::new(&args.sbom)) {
        return Err(format!("dx_verify: sbom not found: {}", args.sbom));
    }
    if !args.sbom_bundle.is_empty() && !verifier.is_file(Path::new(&args.sbom_bundle)) {
        return Err(format!(
            "dx_verify: sbom bundle not found: {}",
            args.sbom_bundle
        ));
    }
    if !args.notice.is_empty() && !verifier.is_file(Path::new(&args.notice)) {
        return Err(format!("dx_verify: notice not found: {}", args.notice));
    }
    if !args.notice_manifest.is_empty() && !verifier.is_file(Path::new(&args.notice_manifest)) {
        return Err(format!(
            "dx_verify: notice manifest not found: {}",
            args.notice_manifest
        ));
    }
    if !verifier.is_nonempty_file(&bundle) {
        return Err(format!("dx_verify: bundle is empty: {}", args.bundle));
    }
    out.push_str(&format!(
        "dx_verify: trust root {TRUST_ROOT} (Sigstore TUF public-good; verifiers bootstrapped from the trust root, never alongside the binary)\n"
    ));
    let cosign = verifier.cosign_bin();
    let gh = verifier.gh_bin();
    let verified: String;
    if verifier.have(&cosign) {
        out.push_str(&format!(
            "dx_verify: verifying {} against bundle via {cosign} verify-blob\n",
            binary.display()
        ));
        if verifier.cosign_verify(&cosign, &bundle, &args.identity, &args.issuer, &binary) {
            verified = "cosign".to_owned();
            out.push_str(&format!(
                "dx_verify: cosign publisher-identity OK (identity={} issuer={})\n",
                args.identity, args.issuer
            ));
        } else {
            return Err(
                "dx_verify: cosign verification failed (tampered bytes, replaced binary/bundle, or unapproved signer fail closed)".to_owned(),
            );
        }
    } else if !args.attestation.is_empty() && verifier.have(&gh) {
        if args.owner.is_empty() {
            return Err(
                "dx_verify: --owner OWNER is required with --attestation (GitHub attestation path)"
                    .to_owned(),
            );
        }
        out.push_str(&format!(
            "dx_verify: verifying {} against attestation via {gh} attestation verify\n",
            binary.display()
        ));
        if verifier.gh_verify(&gh, &binary, &args.owner) {
            verified = "gh-attestation".to_owned();
            out.push_str(&format!(
                "dx_verify: attestation publisher-identity OK (owner={})\n",
                args.owner
            ));
        } else {
            return Err(
                "dx_verify: attestation verification failed (fail closed before install)"
                    .to_owned(),
            );
        }
    } else {
        return Err(format!(
            "dx_verify: no verifier available (want {cosign} verify-blob or {gh} attestation verify); missing, invalid, or unavailable verification inputs fail closed"
        ));
    }
    if !args.sbom.is_empty() {
        let sbom = PathBuf::from(&args.sbom);
        let sbom_bundle = PathBuf::from(&args.sbom_bundle);
        if !verifier.have(&cosign) {
            return Err(format!(
                "dx_verify: SBOM verification needs {cosign} (unavailable)"
            ));
        }
        out.push_str(&format!(
            "dx_verify: verifying SBOM {} via {cosign} verify-blob\n",
            sbom.display()
        ));
        if verifier.cosign_verify(&cosign, &sbom_bundle, &args.identity, &args.issuer, &sbom) {
            out.push_str("dx_verify: SBOM publisher-identity OK\n");
        } else {
            return Err(
                "dx_verify: SBOM verification failed (fail closed before install)".to_owned(),
            );
        }
    }
    if !args.notice.is_empty() {
        let notice_path = PathBuf::from(&args.notice);
        let manifest_path = PathBuf::from(&args.notice_manifest);
        let notice_text = verifier
            .read_text(&notice_path)
            .ok_or_else(|| format!("dx_verify: notice not readable: {}", args.notice))?;
        if notice_text.trim().is_empty() {
            return Err(format!("dx_verify: notice is empty: {}", args.notice));
        }
        let manifest_text = verifier.read_text(&manifest_path).ok_or_else(|| {
            format!(
                "dx_verify: notice manifest not readable: {}",
                args.notice_manifest
            )
        })?;
        let count = verify_notice(&notice_text, &manifest_text)?;
        out.push_str(&format!("dx_verify: NOTICE OK ({count} entries bundled)\n"));
    }
    if !args.install_dir.is_empty() {
        let base = binary
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("dx");
        let digest = verifier
            .sha256_file(&binary)
            .map_err(|error| format!("dx_verify: cannot hash {}: {error}", binary.display()))?;
        let dst = Path::new(&args.install_dir).join(base);
        verifier
            .install_copy(&binary, &dst)
            .map_err(|error| format!("dx_verify: cannot install {}: {error}", dst.display()))?;
        out.push_str(&format!(
            "dx_verify: installed {base} to {} (sha256 {digest}, verified via {verified})\n",
            args.install_dir
        ));
    } else {
        out.push_str(&format!(
            "dx_verify: OK (verified via {verified}; nothing installed, binary never executed)\n"
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FakeVerifier {
        files: HashMap<PathBuf, Vec<u8>>,
        cosign_ok: bool,
        gh_ok: bool,
        have_cosign: bool,
        have_gh: bool,
    }

    impl FakeVerifier {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
                cosign_ok: true,
                gh_ok: true,
                have_cosign: true,
                have_gh: true,
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

        fn gh_verify(&self, _gh: &str, _subject: &Path, _owner: &str) -> bool {
            self.gh_ok
        }

        fn cosign_bin(&self) -> String {
            "cosign".to_owned()
        }

        fn gh_bin(&self) -> String {
            "gh".to_owned()
        }

        fn have(&self, bin: &str) -> bool {
            if bin == "cosign" {
                self.have_cosign
            } else {
                self.have_gh
            }
        }

        fn sha256_file(&self, path: &Path) -> io::Result<String> {
            let data = self
                .files
                .get(path)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing file"))?;
            Ok(dx_digest::sha256_hex(data))
        }

        fn install_copy(&self, _src: &Path, _dst: &Path) -> io::Result<()> {
            Ok(())
        }

        fn read_text(&self, path: &Path) -> Option<String> {
            self.files
                .get(path)
                .and_then(|data| String::from_utf8(data.clone()).ok())
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
            cosign_argv(
                "cosign",
                std::path::Path::new("/u"),
                "ID",
                "ISS",
                std::path::Path::new("/b"),
            ),
            vec![
                "cosign",
                "verify-blob",
                "--bundle",
                "/u",
                "--certificate-identity",
                "ID",
                "--certificate-issuer",
                "ISS",
                "/b",
            ]
        );
        assert_eq!(
            gh_argv("gh", std::path::Path::new("/b"), "owner",),
            vec!["gh", "attestation", "verify", "/b", "--owner", "owner",]
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
        let args = parse_args(&argv).expect("parse");
        assert_eq!(args.binary, "/b");
        assert_eq!(args.bundle, "/u");
        assert_eq!(args.identity, "ID");
        assert_eq!(args.issuer, "ISS");
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
        fake.have_cosign = false;
        fake.have_gh = false;
        let err = verify(&args, &fake).expect_err("no verifier");
        assert!(err.contains("no verifier available"));
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

    #[test]
    fn installs_with_digest() {
        let mut args = valid_args();
        args.install_dir = "/tmp/install".to_owned();
        let fake = valid_fake();
        let out = verify(&args, &fake).expect("install");
        assert!(out.contains("installed dx-fake to /tmp/install"));
        assert!(out.contains("sha256"));
    }

    fn notice_fake() -> (VerifyArgs, FakeVerifier) {
        let mut args = valid_args();
        args.notice = "/tmp/NOTICE".to_owned();
        args.notice_manifest = "/tmp/inventory.txt".to_owned();
        let mut fake = valid_fake();
        fake.write(
            "/tmp/inventory.txt",
            b"demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\ndemo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt\n",
        );
        fake.write(
            "/tmp/NOTICE",
            b"NOTICE for //demo:root\nGenerated by rules_dx notice_bundle from the audited license inventory; do not edit.\n\n=== demo-lib-a 1.0.0 (MIT) ===\nFixture words for demo-lib-a.\n=== demo-lib-b 2.3.4 (Apache-2.0) ===\nFixture words for demo-lib-b.\n",
        );
        (args, fake)
    }

    #[test]
    fn binds_notice_to_manifest() {
        let (args, fake) = notice_fake();
        let out = verify(&args, &fake).expect("notice valid");
        assert!(out.contains("NOTICE OK (2 entries bundled)"));
    }

    #[test]
    fn notice_missing_text_fails_actionable() {
        let (args, mut fake) = notice_fake();
        fake.write("/tmp/inventory.txt", b"demo-lib-a|cargo|1.0.0|MIT|\n");
        let err = verify(&args, &fake).expect_err("missing text");
        assert!(err.contains("missing-notice-text"), "got {err}");
        let (args, mut fake) = notice_fake();
        fake.write("/tmp/NOTICE", b"tampered notice\n");
        let err = verify(&args, &fake).expect_err("tampered notice");
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
            ("--install-dir", "/d"),
        ] {
            argv.push(flag.to_owned());
            argv.push(value.to_owned());
        }
        let args = parse_args(&argv).expect("all flags parse");
        assert_eq!(args.attestation, "/a");
        assert_eq!(args.owner, "owner");
        assert_eq!(args.sbom_bundle, "/sb");
        assert_eq!(args.notice_manifest, "/nm");
        assert_eq!(args.install_dir, "/d");
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
        let check = |args: VerifyArgs, fake: FakeVerifier, want: &str| {
            let err = verify(&args, &fake).expect_err(want);
            assert!(err.contains(want), "want {want}, got {err}");
        };
        let mut fake = valid_fake();
        fake.files.remove(Path::new("/tmp/dx-fake"));
        check(valid_args(), fake, "binary not found");
        let mut fake = valid_fake();
        fake.files.remove(Path::new("/tmp/dx-fake.bundle"));
        check(valid_args(), fake, "bundle not found");
        let mut args = valid_args();
        args.attestation = "/tmp/dx-fake.attestation".to_owned();
        check(args, valid_fake(), "attestation not found");
        let mut args = valid_args();
        args.sbom = "/tmp/sbom".to_owned();
        check(args, valid_fake(), "sbom not found");
        let mut args = valid_args();
        args.sbom = "/tmp/sbom".to_owned();
        args.sbom_bundle = "/tmp/sbom.bundle".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/sbom", b"sbom-bytes");
        check(args, fake, "sbom bundle not found");
        let mut args = valid_args();
        args.notice = "/tmp/NOTICE".to_owned();
        check(args, valid_fake(), "notice not found");
        let mut args = valid_args();
        args.notice = "/tmp/NOTICE".to_owned();
        args.notice_manifest = "/tmp/inventory.txt".to_owned();
        let mut fake = valid_fake();
        fake.write("/tmp/NOTICE", b"NOTICE for //demo:root\n");
        check(args, fake, "notice manifest not found");
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
            fake.have_cosign = cosign;
            fake.gh_ok = gh_ok;
            fake.write("/tmp/dx-fake.attestation", b"attestation-bytes");
            fake
        };
        let out = verify(&attestation_args(), &attestation_fake(false, true)).expect("gh path");
        assert!(out.contains("attestation publisher-identity OK (owner=owner)"));
        let mut no_owner = attestation_args();
        no_owner.owner.clear();
        let err = verify(&no_owner, &attestation_fake(false, true)).expect_err("owner required");
        assert!(err.contains("--owner OWNER is required"), "got {err}");
        let err = verify(&attestation_args(), &attestation_fake(false, false))
            .expect_err("attestation rejected");
        assert!(err.contains("attestation verification failed"), "got {err}");
        let mut with_sbom = attestation_args();
        with_sbom.sbom = "/tmp/sbom".to_owned();
        with_sbom.sbom_bundle = "/tmp/sbom.bundle".to_owned();
        let mut fake = attestation_fake(false, true);
        fake.write("/tmp/sbom", b"sbom-bytes");
        fake.write("/tmp/sbom.bundle", b"sbom-bundle");
        let err = verify(&with_sbom, &fake).expect_err("no cosign for sbom");
        assert!(err.contains("SBOM verification needs cosign"), "got {err}");
    }

    #[test]
    fn notice_manifest_failures_are_actionable() {
        let manifest = "# audited inventory\n\ndemo-lib-a|cargo|1.0.0|MIT|demo.txt\n";
        let notice = dx_release_tools::render_notice(
            "//demo:root",
            &[(
                dx_release_tools::NoticeEntry {
                    package: "demo-lib-a".to_owned(),
                    set: "cargo".to_owned(),
                    version: "1.0.0".to_owned(),
                    license: "MIT".to_owned(),
                    text_basename: "demo.txt".to_owned(),
                },
                "Fixture words for demo-lib-a.\n".to_owned(),
            )],
        );
        assert_eq!(
            verify_notice(&notice, manifest).expect("generated NOTICE verifies"),
            1
        );
        let err = verify_notice(&notice, "demo-lib-a|cargo|1.0.0\n").expect_err("field count");
        assert!(err.contains("want 5 '|' fields"), "got {err}");
        let err = verify_notice(&notice, " |cargo|1.0.0|MIT|demo.txt\n").expect_err("no package");
        assert!(err.contains("must be non-empty"), "got {err}");
        let err = verify_notice(&notice, "demo-lib-a||1.0.0|MIT|demo.txt\n").expect_err("no set");
        assert!(err.contains("must be non-empty"), "got {err}");
        let err =
            verify_notice(&notice, "demo-lib-a|cargo|1.0.0|MIT|\n").expect_err("no text file");
        assert!(err.contains("missing-notice-text"), "got {err}");
        let err = verify_notice("NOTICE for //demo:root\n", "# no packages\n")
            .expect_err("empty manifest");
        assert!(err.contains("lists no packages"), "got {err}");
        let err = verify_notice("NOTICE for //demo:root\n", manifest).expect_err("missing entry");
        assert!(err.contains("NOTICE missing entry"), "got {err}");
    }

    #[test]
    fn notice_inputs_fail_closed_when_unreadable() {
        let (args, mut fake) = notice_fake();
        fake.write("/tmp/NOTICE", b"   \n");
        let err = verify(&args, &fake).expect_err("empty notice");
        assert!(err.contains("notice is empty"), "got {err}");
        let (args, mut fake) = notice_fake();
        fake.write("/tmp/inventory.txt", &[0xff, 0xfe]);
        let err = verify(&args, &fake).expect_err("manifest not utf-8");
        assert!(err.contains("notice manifest not readable"), "got {err}");
        let (args, mut fake) = notice_fake();
        fake.write("/tmp/NOTICE", &[0xff]);
        let err = verify(&args, &fake).expect_err("notice not utf-8");
        assert!(err.contains("notice not readable"), "got {err}");
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
        assert!(!verifier.is_nonempty_file(&scratch.path()));
        assert!(!verifier.is_nonempty_file(&scratch.path().join("missing")));
        assert_eq!(
            verifier.sha256_file(&file).expect("hash"),
            dx_digest::sha256_hex(b"standalone-dx-bytes-v1")
        );
        assert!(verifier
            .sha256_file(&scratch.path().join("missing"))
            .is_err());
        let dst = scratch.path().join("nested/bin/dx");
        verifier.install_copy(&file, &dst).expect("copy");
        assert_eq!(
            std::fs::read(&dst).expect("read copy"),
            b"standalone-dx-bytes-v1"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&dst)
                .expect("copy metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o755);
        }
        assert!(verifier.install_copy(&file, Path::new("/")).is_err());
        assert_eq!(
            verifier.read_text(&file).as_deref(),
            Some("standalone-dx-bytes-v1")
        );
        assert!(verifier
            .read_text(&scratch.path().join("missing"))
            .is_none());
        assert!(!verifier.have("/nonexistent-dx-tool"));
        #[cfg(unix)]
        {
            assert!(verifier.have("/usr/bin/true"));
            assert!(verifier.cosign_verify("/usr/bin/true", &file, "ID", "ISS", &file));
            assert!(!verifier.cosign_verify("/usr/bin/false", &file, "ID", "ISS", &file));
            assert!(verifier.gh_verify("/usr/bin/true", &file, "owner"));
            assert!(!verifier.gh_verify("/usr/bin/false", &file, "owner"));
        }
    }

    #[test]
    fn system_verifier_bin_names_follow_env_overrides() {
        if let Some(raw) = std::env::var_os("DX_INSTALL_VERIFY_CHILD") {
            let raw = raw.to_string_lossy().into_owned();
            let (mode, sentinel) = raw.split_once(':').expect("mode:path");
            std::env::set_var("DX_VERIFY_COSIGN", "pinned-cosign");
            std::env::set_var("DX_VERIFY_GH", "pinned-gh");
            let verifier = SystemVerifier;
            assert_eq!(verifier.cosign_bin(), "pinned-cosign");
            assert_eq!(verifier.gh_bin(), "pinned-gh");
            if mode == "panic" {
                panic!("intentional child failure");
            }
            std::fs::write(sentinel, b"ok").expect("sentinel");
            std::env::remove_var("DX_VERIFY_COSIGN");
            std::env::remove_var("DX_VERIFY_GH");
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
