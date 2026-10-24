use std::path::{Path, PathBuf};
use std::process::Output;

const IDENTITY: &str = "https://github.com/dx-test/release.yml@refs/heads/main";
const ISSUER: &str = "https://token.actions.githubusercontent.com";

fn cosign() -> String {
    let rel = std::env::var("DX_COSIGN_BIN").expect("DX_COSIGN_BIN names the pinned cosign");
    dx_testing::resolve_runfiles(&rel)
        .to_string_lossy()
        .into_owned()
}

fn fixture(name: &str) -> PathBuf {
    let rel = std::env::var("DX_COSIGN_FIXTURES").expect("DX_COSIGN_FIXTURES names ca.pem");
    let anchor = dx_testing::resolve_runfiles(&rel);
    anchor.parent().expect("fixture dir").join(name)
}

fn run(argv: &[String]) -> Output {
    std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .expect("spawn cosign")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn verify(identity: &str, issuer: &str, subject: &Path) -> Output {
    let mut argv = dx_install_tools::cosign_argv(
        &cosign(),
        &fixture("artifact.bundle"),
        identity,
        issuer,
        subject,
    );
    let subject_arg = argv.pop().expect("subject");
    argv.extend(
        [
            "--certificate-chain",
            fixture("ca.pem").to_str().expect("utf8"),
            "--insecure-ignore-tlog",
            "--insecure-ignore-sct",
        ]
        .map(str::to_owned),
    );
    argv.push(subject_arg);
    run(&argv)
}

#[test]
fn pinned_cosign_is_v2_4_1() {
    let output = run(&[cosign(), "version".to_owned()]);
    let text = String::from_utf8_lossy(&output.stdout).into_owned() + &stderr(&output);
    assert!(output.status.success(), "{text}");
    assert!(dx_release_tools::signing_version_ok(&text), "{text}");
}

#[test]
fn valid_signature_verifies() {
    let output = verify(IDENTITY, ISSUER, &fixture("artifact.bin"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Verified OK"), "{}", stderr(&output));
}

#[test]
fn tampered_bytes_fail() {
    let scratch = dx_testing::mkscratch("cosign_tamper").expect("scratch");
    let tampered = scratch.join("artifact.bin");
    let mut bytes = std::fs::read(fixture("artifact.bin")).expect("read");
    bytes.extend_from_slice(b"tampered");
    std::fs::write(&tampered, bytes).expect("write");
    let output = verify(IDENTITY, ISSUER, &tampered);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("invalid signature"), "{}", stderr(&output));
}

#[test]
fn wrong_identity_fails() {
    let output = verify("https://github.com/evil/x.yml@refs/heads/main", ISSUER, &fixture("artifact.bin"));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("none of the expected identities matched"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn wrong_issuer_fails() {
    let output = verify(IDENTITY, "https://issuer.example", &fixture("artifact.bin"));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("none of the expected identities matched"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn sign_blob_argv_parses_with_pinned_cosign() {
    let scratch = dx_testing::mkscratch("cosign_sign").expect("scratch");
    let bundle = scratch.join("out.bundle");
    let mut argv = dx_release_tools::signing_sign_argv(
        fixture("artifact.bin").to_str().expect("utf8"),
        bundle.to_str().expect("utf8"),
    );
    argv[0] = cosign();
    argv.splice(
        2..2,
        [
            "--identity-token=not-a-token",
            "--fulcio-url=http://127.0.0.1:1",
            "--rekor-url=http://127.0.0.1:1",
        ]
        .map(str::to_owned),
    );
    let output = run(&argv);
    assert!(!output.status.success());
    assert!(!stderr(&output).contains("unknown flag"), "{}", stderr(&output));
    assert!(!stderr(&output).contains("unknown shorthand"), "{}", stderr(&output));
}

#[test]
fn unsupported_flags_are_rejected_by_pinned_cosign() {
    let mut sign = dx_release_tools::signing_sign_argv("a", "b");
    sign[0] = cosign();
    sign.insert(2, "--certificate-identity=x".to_owned());
    let output = run(&sign);
    assert!(stderr(&output).contains("unknown flag: --certificate-identity"), "{}", stderr(&output));

    let mut verify_argv = dx_install_tools::cosign_argv(
        &cosign(),
        &fixture("artifact.bundle"),
        IDENTITY,
        ISSUER,
        &fixture("artifact.bin"),
    );
    let position = verify_argv
        .iter()
        .position(|arg| arg == "--certificate-oidc-issuer")
        .expect("issuer flag");
    verify_argv[position] = "--certificate-issuer".to_owned();
    let output = run(&verify_argv);
    assert!(stderr(&output).contains("unknown flag: --certificate-issuer"), "{}", stderr(&output));
}
