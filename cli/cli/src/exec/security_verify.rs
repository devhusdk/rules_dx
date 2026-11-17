//! `dx security --verify`: checks one downloaded asset against its Sigstore
//! bundle with the pinned cosign and, when `gh` is installed, against its
//! GitHub attestation.

use super::common::{
    CODE_AUDIT_FAILED, CODE_INVALID_ARGUMENTS, Env, check_stdout_write, emit_built, emit_event,
    report_dx_error,
};
use super::notice_codes::CODE_SECURITY_VERIFIED;
use super::pinned_tool::{PinnedTool, PinnedToolError, QueryBazel, resolve_pinned};
use base64::Engine as _;
use dx_error::DxError;
use dx_output::{FinishedCounts, NoticeEvent, OutputMode, command_finished, notice_event};
use dx_plan::args::Invocation;
use std::path::{Path, PathBuf};

pub(crate) const COSIGN_VAR: &str = "DX_COSIGN_BIN";
pub(crate) const COSIGN_LABEL: &str = "@rules_dx//deploy/private/release:cosign";
pub(crate) const ISSUER: &str = "https://token.actions.githubusercontent.com";
pub(crate) const PROVENANCE_TYPE: &str = "https://slsa.dev/provenance/v1";
pub(crate) const BUNDLE_SUFFIX: &str = ".sigstore.json";

/// What a Sigstore bundle signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BundleKind {
    /// A signature over the blob itself (`cosign sign-blob`).
    Message,
    /// An in-toto statement naming the blob as a subject (`actions/attest`).
    Attestation,
}

/// The parts of a bundle `--verify` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BundleInfo {
    pub(crate) kind: BundleKind,
    pub(crate) signer: Option<String>,
}

/// Checks `repo` is `owner/name` with GitHub's name characters.
pub(crate) fn check_repo(repo: &str) -> Result<(&str, &str), DxError> {
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match repo.split_once('/') {
        Some((owner, name)) if valid(owner) && valid(name) => Ok((owner, name)),
        _ => Err(DxError::usage(format!(
            "--repo wants <owner/name>, got {repo:?}"
        ))),
    }
}

/// The certificate identity regexp a signer from `repo`'s workflows matches.
pub(crate) fn identity_regexp(owner: &str, name: &str) -> String {
    let escape = |part: &str| part.replace('.', "\\.");
    format!("^https://github\\.com/{}/{}/", escape(owner), escape(name))
}

pub(crate) fn cosign_argv(
    cosign: &Path,
    kind: BundleKind,
    bundle: &Path,
    regexp: &str,
    file: &Path,
) -> Vec<String> {
    let mut argv = vec![cosign.to_string_lossy().into_owned()];
    match kind {
        BundleKind::Message => argv.push("verify-blob".to_owned()),
        BundleKind::Attestation => argv.extend([
            "verify-blob-attestation".to_owned(),
            "--type".to_owned(),
            PROVENANCE_TYPE.to_owned(),
        ]),
    }
    argv.extend([
        "--bundle".to_owned(),
        bundle.to_string_lossy().into_owned(),
        "--certificate-identity-regexp".to_owned(),
        regexp.to_owned(),
        "--certificate-oidc-issuer".to_owned(),
        ISSUER.to_owned(),
        file.to_string_lossy().into_owned(),
    ]);
    argv
}

pub(crate) fn gh_argv(kind: BundleKind, bundle: &Path, repo: &str, file: &Path) -> Vec<String> {
    let mut argv = vec![
        "gh".to_owned(),
        "attestation".to_owned(),
        "verify".to_owned(),
        file.to_string_lossy().into_owned(),
        "--repo".to_owned(),
        repo.to_owned(),
        "--cert-oidc-issuer".to_owned(),
        ISSUER.to_owned(),
    ];
    if kind == BundleKind::Attestation {
        argv.extend(["--bundle".to_owned(), bundle.to_string_lossy().into_owned()]);
    }
    argv
}

/// Reads the bundle kind and the signing certificate's identity.
pub(crate) fn read_bundle(text: &str) -> Result<BundleInfo, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|error| format!("not JSON: {error}"))?;
    let kind = if value.get("dsseEnvelope").is_some() {
        BundleKind::Attestation
    } else if value.pointer("/messageSignature").is_some() || value.get("base64Signature").is_some()
    {
        BundleKind::Message
    } else {
        return Err("neither a messageSignature nor a dsseEnvelope".to_owned());
    };
    Ok(BundleInfo {
        kind,
        signer: certificate_der(&value).and_then(|der| san_identity(&der)),
    })
}

fn certificate_der(value: &serde_json::Value) -> Option<Vec<u8>> {
    let engine = base64::engine::general_purpose::STANDARD;
    let raw = value
        .pointer("/verificationMaterial/certificate/rawBytes")
        .or_else(|| {
            value.pointer("/verificationMaterial/x509CertificateChain/certificates/0/rawBytes")
        })
        .and_then(serde_json::Value::as_str);
    if let Some(raw) = raw {
        return engine.decode(raw).ok();
    }
    let pem = engine.decode(value.get("cert")?.as_str()?).ok()?;
    let pem = String::from_utf8(pem).ok()?;
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();
    engine.decode(body.trim()).ok()
}

/// One DER element: its tag, its contents and the bytes after it.
fn der_element(bytes: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let (&tag, rest) = bytes.split_first()?;
    let (&first, rest) = rest.split_first()?;
    let (len, rest) = if first < 0x80 {
        (usize::from(first), rest)
    } else {
        let count = usize::from(first & 0x7f);
        if count == 0 || count > 4 {
            return None;
        }
        let (len_bytes, rest) = rest.split_at_checked(count)?;
        let len = len_bytes
            .iter()
            .fold(0usize, |len, &byte| (len << 8) | usize::from(byte));
        (len, rest)
    };
    let (contents, rest) = rest.split_at_checked(len)?;
    Some((tag, contents, rest))
}

const SAN_OID: [u8; 5] = [0x06, 0x03, 0x55, 0x1d, 0x11];

/// The first URI or email in the certificate's subject alternative names.
pub(crate) fn san_identity(der: &[u8]) -> Option<String> {
    let at = der
        .windows(SAN_OID.len())
        .position(|window| window == SAN_OID)?;
    let (mut tag, mut contents, after) = der_element(der.get(at + SAN_OID.len()..)?)?;
    if tag == 0x01 {
        (tag, contents, _) = der_element(after)?;
    }
    if tag != 0x04 {
        return None;
    }
    let (tag, mut names, _) = der_element(contents)?;
    if tag != 0x30 {
        return None;
    }
    while let Some((tag, value, next)) = der_element(names) {
        if matches!(tag, 0x86 | 0x81) {
            return String::from_utf8(value.to_vec()).ok();
        }
        names = next;
    }
    None
}

fn absolute(path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    dx_process::current_directory().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
}

fn emit_lines(output: OutputMode, out: &mut dyn std::io::Write, lines: &[String]) -> i32 {
    if output == OutputMode::Json {
        for line in lines {
            let notice = notice_event(&NoticeEvent {
                related_command: Some("security".to_owned()),
                ..NoticeEvent::info(CODE_SECURITY_VERIFIED, line.clone())
            });
            if let Err(exit) = emit_built(out, notice) {
                return exit;
            }
        }
        return match emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            Ok(()) => 0,
            Err(exit) => exit,
        };
    }
    for line in lines {
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    0
}

pub(crate) fn execute_verify(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    let output = invocation.output;
    let usage = |out: &mut dyn std::io::Write, err: &mut dyn std::io::Write, error: &DxError| {
        report_dx_error(output, out, err, CODE_INVALID_ARGUMENTS, error)
    };
    let failed = |out: &mut dyn std::io::Write, err: &mut dyn std::io::Write, message: String| {
        report_dx_error(
            output,
            out,
            err,
            CODE_AUDIT_FAILED,
            &DxError::failure(message),
        )
    };
    let (Some(file), Some(repo)) = (invocation.verify.as_deref(), invocation.repo.as_deref())
    else {
        return usage(
            out,
            err,
            &DxError::usage("--verify needs --repo <owner/name>"),
        );
    };
    let (owner, name) = match check_repo(repo) {
        Ok(parts) => parts,
        Err(error) => return usage(out, err, &error),
    };
    let file = absolute(file);
    let bundle = invocation.bundle.as_deref().map_or_else(
        || PathBuf::from(format!("{}{BUNDLE_SUFFIX}", file.display())),
        absolute,
    );
    if !file.is_file() {
        return failed(out, err, format!("{}: no such file", file.display()));
    }
    let info = match std::fs::read_to_string(&bundle)
        .map_err(|error| error.to_string())
        .and_then(|text| read_bundle(&text))
    {
        Ok(info) => info,
        Err(detail) => {
            return failed(
                out,
                err,
                format!("{}: not a Sigstore bundle ({detail})", bundle.display()),
            );
        }
    };
    let bazel = QueryBazel {
        query_runner,
        workspace,
        startup: &invocation.bazel_startup_options,
    };
    let tool = PinnedTool {
        var: COSIGN_VAR,
        label: COSIGN_LABEL,
    };
    let cosign = match resolve_pinned(runner, &bazel, &tool, invocation.offline) {
        Ok(path) => path,
        Err(PinnedToolError::Invalid) => {
            return failed(
                out,
                err,
                format!("{COSIGN_VAR} must be an absolute path to the pinned {COSIGN_LABEL}"),
            );
        }
        Err(PinnedToolError::Unresolved { detail }) => {
            return failed(
                out,
                err,
                format!("could not resolve {COSIGN_LABEL} through Bazel ({detail})"),
            );
        }
    };
    let regexp = identity_regexp(owner, name);
    let cwd = file.parent().unwrap_or(workspace);
    let cosign_argv = cosign_argv(&cosign, info.kind, &bundle, &regexp, &file);
    match runner.run(&cosign_argv, cwd, &[]) {
        Ok(status) if status.code == Some(0) => {}
        Ok(_) => {
            return failed(
                out,
                err,
                format!(
                    "{}: cosign rejected the bundle {} for a signer matching {regexp}",
                    file.display(),
                    bundle.display()
                ),
            );
        }
        Err(error) => return failed(out, err, format!("could not run cosign: {error}")),
    }
    let signer = info.signer.unwrap_or_else(|| regexp.clone());
    let mut lines = vec![format!(
        "{}: cosign verified, signer {signer} (issuer {ISSUER})",
        file.display()
    )];
    match runner.run(&gh_argv(info.kind, &bundle, repo, &file), cwd, &[]) {
        Ok(status) if status.code == Some(0) => {
            lines.push(format!(
                "{}: gh attestation verified for {repo}",
                file.display()
            ));
        }
        Ok(_) => {
            return failed(
                out,
                err,
                format!(
                    "{}: gh attestation verify failed for {repo}",
                    file.display()
                ),
            );
        }
        // A missing gh fails instead of skipping: a green exit must mean both
        // the Sigstore bundle and the GitHub attestation were checked.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return failed(
                out,
                err,
                format!(
                    "{}: gh not found on PATH; install the GitHub CLI so `gh attestation verify` can check the asset against {repo}",
                    file.display()
                ),
            );
        }
        Err(error) => return failed(out, err, format!("could not run gh: {error}")),
    }
    emit_lines(output, out, &lines)
}

#[cfg(test)]
#[path = "security_verify_tests.rs"]
mod tests;
