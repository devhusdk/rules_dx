use super::*;
use crate::exec::test_support::Harness;
use dx_process::{ChildStatus, PinnedVar, Runner};
use std::cell::RefCell;
use std::io;

const SIGNER: &str = "https://github.com/acme/tool/.github/workflows/release.yml@refs/tags/v1.0.0";

fn der(tag: u8, contents: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = contents.len();
    if len < 0x80 {
        out.push(u8::try_from(len).expect("short"));
    } else {
        out.push(0x82);
        out.extend_from_slice(&u16::try_from(len).expect("len").to_be_bytes());
    }
    out.extend_from_slice(contents);
    out
}

fn certificate(uri: &str, critical: bool) -> Vec<u8> {
    let names = der(0x30, &der(0x86, uri.as_bytes()));
    let mut extension = SAN_OID.to_vec();
    if critical {
        extension.extend(der(0x01, &[0xff]));
    }
    extension.extend(der(0x04, &names));
    let padding = der(0x04, &[0u8; 200]);
    der(0x30, &[padding, der(0x30, &extension)].concat())
}

fn bundle_json(kind: BundleKind, uri: &str) -> String {
    let raw = base64::engine::general_purpose::STANDARD.encode(certificate(uri, false));
    let content = match kind {
        BundleKind::Message => r#""messageSignature":{"signature":"AA=="}"#,
        BundleKind::Attestation => r#""dsseEnvelope":{"payload":"AA=="}"#,
    };
    format!(r#"{{"verificationMaterial":{{"certificate":{{"rawBytes":"{raw}"}}}},{content}}}"#)
}

#[test]
fn san_identity_reads_the_uri_with_and_without_a_critical_flag() {
    assert_eq!(
        san_identity(&certificate(SIGNER, false)).as_deref(),
        Some(SIGNER)
    );
    assert_eq!(
        san_identity(&certificate(SIGNER, true)).as_deref(),
        Some(SIGNER)
    );
    assert_eq!(san_identity(&[0x30, 0x00]), None);
}

#[test]
fn read_bundle_tells_message_from_attestation_bundles() {
    let message = read_bundle(&bundle_json(BundleKind::Message, SIGNER)).expect("message");
    assert_eq!(message.kind, BundleKind::Message);
    assert_eq!(message.signer.as_deref(), Some(SIGNER));
    let attestation =
        read_bundle(&bundle_json(BundleKind::Attestation, SIGNER)).expect("attestation");
    assert_eq!(attestation.kind, BundleKind::Attestation);
    assert!(read_bundle("{}").is_err());
    assert!(read_bundle("not json").is_err());
}

#[test]
fn read_bundle_reads_the_legacy_pem_certificate() {
    let der = certificate(SIGNER, false);
    let engine = base64::engine::general_purpose::STANDARD;
    let pem = format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
        engine.encode(der)
    );
    let text = format!(
        r#"{{"base64Signature":"AA==","cert":"{}"}}"#,
        engine.encode(pem)
    );
    let info = read_bundle(&text).expect("legacy");
    assert_eq!(info.kind, BundleKind::Message);
    assert_eq!(info.signer.as_deref(), Some(SIGNER));
}

#[test]
fn check_repo_wants_owner_and_name() {
    assert_eq!(check_repo("acme/tool.rs").expect("ok"), ("acme", "tool.rs"));
    for bad in ["acme", "acme/", "/tool", "acme/tool/x", "acme/to ol"] {
        assert!(check_repo(bad).is_err(), "{bad}");
    }
    assert_eq!(
        identity_regexp("acme", "tool.rs"),
        "^https://github\\.com/acme/tool\\.rs/"
    );
}

#[test]
fn argv_picks_the_cosign_verb_by_bundle_kind() {
    let cosign = Path::new("/pinned/cosign");
    let (bundle, file) = (Path::new("/a.sigstore.json"), Path::new("/a"));
    let message = cosign_argv(cosign, BundleKind::Message, bundle, "^r", file);
    assert_eq!(message.get(1).map(String::as_str), Some("verify-blob"));
    assert!(message.contains(&"--certificate-identity-regexp".to_owned()));
    let attestation = cosign_argv(cosign, BundleKind::Attestation, bundle, "^r", file);
    assert_eq!(
        attestation.get(1).map(String::as_str),
        Some("verify-blob-attestation")
    );
    assert!(attestation.contains(&PROVENANCE_TYPE.to_owned()));
    assert!(
        !gh_argv(BundleKind::Message, bundle, "acme/tool", file).contains(&"--bundle".to_owned())
    );
    assert!(
        gh_argv(BundleKind::Attestation, bundle, "acme/tool", file)
            .contains(&"--bundle".to_owned())
    );
}

struct VerifyRunner {
    cosign: i32,
    gh: Option<i32>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl Runner for VerifyRunner {
    fn run(&self, argv: &[String], _cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.calls.borrow_mut().push(argv.to_vec());
        if argv.first().map(String::as_str) == Some("gh") {
            return self
                .gh
                .map(ChildStatus::exited)
                .ok_or_else(|| io::ErrorKind::NotFound.into());
        }
        Ok(ChildStatus::exited(self.cosign))
    }

    fn pinned_tool(&self, var: &str) -> PinnedVar {
        assert_eq!(var, COSIGN_VAR);
        PinnedVar::Path(PathBuf::from("/pinned/cosign"))
    }
}

fn verify(name: &str, runner: &VerifyRunner, extra: &[&str]) -> (i32, String, String) {
    let harness = Harness::new(name);
    let file = harness.temp.join("tool.tar.gz");
    std::fs::write(&file, b"archive").expect("asset");
    let bundle = harness.temp.join("tool.tar.gz.sigstore.json");
    std::fs::write(&bundle, bundle_json(BundleKind::Message, SIGNER)).expect("bundle");
    let file = file.to_string_lossy().into_owned();
    let mut words = vec!["security", "--verify", file.as_str(), "--repo", "acme/tool"];
    words.extend_from_slice(extra);
    let invocation = dx_plan::args::parse(&dx_testing::cli::strings(&words)).expect("parse");
    harness.execute_with(&invocation, runner)
}

#[test]
fn verify_prints_the_signer_and_runs_gh_when_present() {
    let runner = VerifyRunner {
        cosign: 0,
        gh: Some(0),
        calls: RefCell::new(Vec::new()),
    };
    let (code, out, err) = verify("verify_ok", &runner, &[]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains(&format!("cosign verified, signer {SIGNER}")),
        "{out}"
    );
    assert!(
        out.contains("gh attestation verified for acme/tool"),
        "{out}"
    );
    let calls = runner.calls.borrow();
    let cosign = calls.first().expect("cosign call");
    assert_eq!(cosign.first().map(String::as_str), Some("/pinned/cosign"));
    assert!(
        cosign
            .iter()
            .any(|arg| arg.ends_with("tool.tar.gz.sigstore.json"))
    );
    assert!(cosign.contains(&"^https://github\\.com/acme/tool/".to_owned()));
}

#[test]
fn verify_exits_1_when_gh_is_missing() {
    let runner = VerifyRunner {
        cosign: 0,
        gh: None,
        calls: RefCell::new(Vec::new()),
    };
    let (code, out, err) = verify("verify_no_gh", &runner, &[]);
    assert_eq!(code, 1, "{out}");
    assert!(err.contains("gh not found on PATH"), "{err}");
    assert!(err.contains("acme/tool"), "{err}");
    assert!(!out.contains("verified"), "{out}");
}

#[test]
fn verify_exits_1_when_cosign_rejects_the_asset() {
    let runner = VerifyRunner {
        cosign: 1,
        gh: Some(0),
        calls: RefCell::new(Vec::new()),
    };
    let (code, _, err) = verify("verify_cosign_bad", &runner, &[]);
    assert_eq!(code, 1);
    assert!(err.contains("cosign rejected the bundle"), "{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
}

#[test]
fn verify_exits_1_when_gh_rejects_the_asset() {
    let runner = VerifyRunner {
        cosign: 0,
        gh: Some(1),
        calls: RefCell::new(Vec::new()),
    };
    let (code, _, err) = verify("verify_gh_bad", &runner, &[]);
    assert_eq!(code, 1);
    assert!(err.contains("gh attestation verify failed"), "{err}");
}

#[test]
fn verify_exits_1_without_a_bundle() {
    let runner = VerifyRunner {
        cosign: 0,
        gh: Some(0),
        calls: RefCell::new(Vec::new()),
    };
    let (code, _, err) = verify(
        "verify_no_bundle",
        &runner,
        &["--bundle", "/no/such.sigstore.json"],
    );
    assert_eq!(code, 1);
    assert!(err.contains("not a Sigstore bundle"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn verify_flags_parse_only_together() {
    use dx_plan::args::parse;
    use dx_testing::cli::strings;
    assert!(parse(&strings(&["security", "--verify", "a"])).is_err());
    assert!(parse(&strings(&["security", "--repo", "a/b"])).is_err());
    assert!(parse(&strings(&["security", "--bundle", "b"])).is_err());
    assert!(
        parse(&strings(&[
            "security", "--verify", "a", "--repo", "a/b", "//x"
        ]))
        .is_err()
    );
    assert!(parse(&strings(&["license", "--verify", "a", "--repo", "a/b"])).is_err());
    let invocation = parse(&strings(&[
        "security", "--verify", "a", "--repo", "a/b", "--bundle", "b",
    ]))
    .expect("parse");
    assert_eq!(invocation.verify.as_deref(), Some("a"));
    assert_eq!(invocation.repo.as_deref(), Some("a/b"));
    assert_eq!(invocation.bundle.as_deref(), Some("b"));
}
