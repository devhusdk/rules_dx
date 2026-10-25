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

fn trusting_cosign(scratch: &Path) -> PathBuf {
    let wrapper = scratch.join("cosign-fixture-trust");
    let script = format!(
        "#!/bin/sh\nexec '{}' \"$@\" --certificate-chain '{}' --insecure-ignore-tlog --insecure-ignore-sct\n",
        cosign(),
        fixture("ca.pem").display()
    );
    std::fs::write(&wrapper, script).expect("write wrapper");
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    wrapper
}

fn verify(identity: &str, issuer: &str, subject: &Path) -> Output {
    let scratch = dx_testing::mkscratch("cosign_verify").expect("scratch");
    let wrapper = trusting_cosign(&scratch);
    let dx_verify = dx_testing::resolve_runfiles(
        &std::env::var("DX_VERIFY_BIN").expect("DX_VERIFY_BIN names dx_verify"),
    );
    std::process::Command::new(dx_verify)
        .env("DX_VERIFY_COSIGN", wrapper)
        .arg("--binary")
        .arg(subject)
        .arg("--bundle")
        .arg(fixture("artifact.bundle"))
        .args(["--identity", identity, "--issuer", issuer])
        .output()
        .expect("spawn dx_verify")
}

#[test]
fn pinned_cosign_reports_the_pinned_git_version() {
    let want = std::env::var("DX_COSIGN_VERSION").expect("DX_COSIGN_VERSION");
    let output = run(&[cosign(), "version".to_owned()]);
    let text = String::from_utf8_lossy(&output.stdout).into_owned() + &stderr(&output);
    assert!(output.status.success(), "{text}");
    assert!(dx_release_tools::signing_version_ok(&text, &want), "{text}");
}

#[test]
fn valid_signature_verifies() {
    let output = verify(IDENTITY, ISSUER, &fixture("artifact.bin"));
    assert!(output.status.success(), "{}", stderr(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cosign publisher-identity OK"), "{stdout}");
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
    assert!(
        stderr(&output).contains("cosign verification failed"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn wrong_identity_fails() {
    let output = verify(
        "https://github.com/evil/x.yml@refs/heads/main",
        ISSUER,
        &fixture("artifact.bin"),
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cosign verification failed"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn wrong_issuer_fails() {
    let output = verify(IDENTITY, "https://issuer.example", &fixture("artifact.bin"));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cosign verification failed"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn sign_blob_argv_parses_with_pinned_cosign() {
    let scratch = dx_testing::mkscratch("cosign_sign").expect("scratch");
    let bundle = scratch.join("out.bundle");
    let mut argv = dx_release_tools::signing_sign_argv(
        &cosign(),
        fixture("artifact.bin").to_str().expect("utf8"),
        bundle.to_str().expect("utf8"),
    );
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
    assert!(
        stderr(&output).contains("getting key from Fulcio"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains("unknown flag"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains("unknown shorthand"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn unsupported_flags_are_rejected_by_pinned_cosign() {
    let mut sign = dx_release_tools::signing_sign_argv(&cosign(), "a", "b");
    sign.insert(2, "--certificate-identity=x".to_owned());
    let output = run(&sign);
    assert!(
        stderr(&output).contains("unknown flag: --certificate-identity"),
        "{}",
        stderr(&output)
    );

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
    assert!(
        stderr(&output).contains("unknown flag: --certificate-issuer"),
        "{}",
        stderr(&output)
    );
}
