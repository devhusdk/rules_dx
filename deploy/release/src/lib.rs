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
use std::path::Path;

pub mod candidate;
pub mod cell;
pub mod inventory;
mod provenance;
mod publish;
mod spdx;
mod tool;

macro_rules! release_repository {
    () => {
        "https://github.com/devhusdk/rules_dx"
    };
}

pub const RELEASE_REPOSITORY: &str = release_repository!();
pub const RELEASE_REMOTE: &str = concat!(release_repository!(), ".git");

pub use publish::{
    publish_dry_run_check, sha256sum_line, PublishDryRunInputs, PublishDryRunOutcome, REPORT_SCHEMA,
};

pub use tool::release_tool_run;

use spdx::SpdxDocument;

fn basename(path: &Path) -> io::Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("path {} has no UTF-8 basename", path.display()),
            )
        })
}

pub(crate) fn render_pretty(value: &serde_json::Value) -> String {
    dx_fingerprint::to_json_ascii_pretty(value).unwrap_or_default()
}

pub fn bin_usage(prog: &str, usage: &str) -> i32 {
    eprintln!("usage: {prog} {usage}");
    1
}

pub fn bin_cannot_write(prog: &str, target: &Path, error: impl std::fmt::Display) -> i32 {
    eprintln!("{prog}: cannot write {}: {error}", target.display());
    1
}

pub fn bin_error(diagnostic: impl std::fmt::Display) -> i32 {
    eprintln!("{diagnostic}");
    1
}

pub fn render_spdx(base: &str, digest: &str, package: &str, supplier: &str) -> String {
    let mut document = spdx::Document::new(
        &format!("{package}-{base}"),
        &format!("{RELEASE_REPOSITORY}/releases/{base}-{digest}"),
        &["Tool: rules_dx-sbom-1.0"],
    );
    let checksum = spdx::SpdxChecksum {
        algorithm: spdx::SHA256.to_owned(),
        value: digest.to_owned(),
    };
    document.files = vec![spdx::SpdxFile {
        id: "SPDXRef-File".to_owned(),
        name: base.to_owned(),
        checksums: vec![checksum.clone()],
    }];
    document.packages = vec![spdx::SpdxPackage {
        id: "SPDXRef-Package".to_owned(),
        name: package.to_owned(),
        supplier: format!("Organization: {supplier}"),
        download_location: spdx::NOASSERTION.to_owned(),
        files_analyzed: Some(false),
        checksums: vec![checksum],
        external_refs: vec![spdx::SpdxExternalRef {
            category: "PACKAGE-MANAGER".to_owned(),
            ref_type: "purl".to_owned(),
            locator: format!("pkg:generic/{package}@{digest}"),
        }],
        ..spdx::SpdxPackage::default()
    }];
    document.relationships = vec![spdx::describes("SPDXRef-Package")];
    let value = serde_json::to_value(&document).unwrap_or(serde_json::Value::Null);
    render_pretty(&value)
}

pub fn render_provenance(base: &str, digest: &str, builder: &str) -> String {
    render_pretty(&serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": format!("{RELEASE_REPOSITORY}/release@v1"),
                "externalParameters": {"artifact": base},
            },
            "runDetails": {
                "builder": {"id": builder},
                "metadata": {"invocationId": "dry-run"},
            },
        },
        "predicateType": "https://slsa.dev/provenance/v1",
        "subject": [{"digest": {"sha256": digest}, "name": base}],
    }))
}

pub fn render_bcr_source(module: &str, version: &str) -> String {
    render_pretty(&serde_json::json!({
        "integrity": "<integrity-filled-at-release>",
        "strip_prefix": bcr_strip_prefix(module, version),
        "url": bcr_source_url(module, version),
    }))
}

pub fn write_spdx(src: &Path, dst: &Path, package: &str, supplier: &str) -> io::Result<()> {
    let digest = dx_digest::sha256_file_hex(src)?;
    let base = basename(src)?;
    std::fs::write(
        dst,
        render_spdx(&base, &digest, package, supplier).as_bytes(),
    )?;
    Ok(())
}

pub fn write_provenance(src: &Path, dst: &Path, builder: &str) -> io::Result<()> {
    if let Err(problem) = provenance_builder_error(builder) {
        return Err(io::Error::other(problem));
    }
    let digest = dx_digest::sha256_file_hex(src)?;
    let base = basename(src)?;
    std::fs::write(dst, render_provenance(&base, &digest, builder).as_bytes())?;
    Ok(())
}

pub fn write_bcr_source(dst: &Path, module: &str, version: &str) -> io::Result<()> {
    std::fs::write(dst, render_bcr_source(module, version).as_bytes())?;
    Ok(())
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = u32::from(chunk[0]);
        let b = if chunk.len() > 1 {
            u32::from(chunk[1])
        } else {
            0
        };
        let c = if chunk.len() > 2 {
            u32::from(chunk[2])
        } else {
            0
        };
        let triple = (a << 16) | (b << 8) | c;
        out.push(BASE64_ALPHABET[((triple >> 18) & 63) as usize] as char);
        out.push(BASE64_ALPHABET[((triple >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[((triple >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(triple & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn bcr_integrity_of_digest(digest: &str) -> Result<String, String> {
    let raw = dx_digest::parse_hex(digest).map_err(|_| {
        format!("bcr: invalid sha256 digest {digest:?}: want 64 lowercase hex chars")
    })?;
    Ok(format!("sha256-{}", base64_encode(&raw)))
}

fn bcr_candidate_version_error(version: &str) -> Result<(), String> {
    if version == "0.0.0" {
        return Err("bcr: version 0.0.0 is unpublishable (shape check only); candidate source metadata needs an owner-approved SemVer release version".to_owned());
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.chars().next().is_none_or(|c| !c.is_ascii_digit()))
    {
        return Err(format!(
            "bcr: invalid version '{version}': want SemVer MAJOR.MINOR.PATCH (for example '1.2.3-rc1')"
        ));
    }
    Ok(())
}

pub fn bcr_source_url(module: &str, version: &str) -> String {
    format!("{RELEASE_REPOSITORY}/releases/download/v{version}/{module}-{version}.tar.gz")
}

pub fn bcr_strip_prefix(module: &str, version: &str) -> String {
    format!("{module}-{version}")
}

pub fn render_bcr_source_for_digest(
    module: &str,
    version: &str,
    digest: &str,
) -> Result<String, String> {
    if module != BCR_WANT_MODULE {
        return Err(format!("bcr: invalid module '{module}': want 'rules_dx'"));
    }
    bcr_candidate_version_error(version)?;
    let integrity = bcr_integrity_of_digest(digest)?;
    Ok(render_pretty(&serde_json::json!({
        "integrity": integrity,
        "strip_prefix": bcr_strip_prefix(module, version),
        "url": bcr_source_url(module, version),
    })))
}

pub fn write_bcr_source_for_archive(
    dst: &Path,
    module: &str,
    version: &str,
    archive: &Path,
) -> io::Result<String> {
    let digest = dx_digest::sha256_file_hex(archive)?;
    let text = render_bcr_source_for_digest(module, version, &digest).map_err(io::Error::other)?;
    std::fs::write(dst, text.as_bytes())?;
    bcr_integrity_of_digest(&digest).map_err(io::Error::other)
}

pub fn bcr_candidate_error(
    source_text: &str,
    module: &str,
    version: &str,
    digest: &str,
) -> Result<(), String> {
    let value: serde_json::Value = serde_json::from_str(source_text).map_err(|error| {
        format!("bcr: candidate source.json is not JSON (interrupted publication?): {error}")
    })?;
    let want_integrity = bcr_integrity_of_digest(digest)?;
    let integrity = value
        .get("integrity")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if integrity != want_integrity {
        return Err(format!(
            "bcr: candidate source.json integrity mismatch: archive sha256 {digest} wants {want_integrity:?}, metadata carries {integrity:?} (refusing to replace immutable version artifacts)"
        ));
    }
    let want_prefix = bcr_strip_prefix(module, version);
    let prefix = value
        .get("strip_prefix")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if prefix != want_prefix {
        return Err(format!(
            "bcr: candidate source.json strip_prefix mismatch: want {want_prefix:?}, got {prefix:?}"
        ));
    }
    let want_url = bcr_source_url(module, version);
    let url = value
        .get("url")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if url != want_url {
        return Err(format!(
            "bcr: candidate source.json url mismatch: want {want_url:?}, got {url:?}"
        ));
    }
    Ok(())
}

pub fn release_remote_error(
    candidate: &candidate::Candidate,
    ls_remote: Result<String, String>,
) -> Result<String, String> {
    let tag = candidate.tag();
    let remote = candidate.remote();
    let output = ls_remote.map_err(|error| format!(
        "release: cannot verify remote tag {tag} (git ls-remote --tags {remote} failed: {error}); push it beforehand with owner approval (this program never creates or pushes tags)"
    ))?;
    let sha = candidate::remote_tag_commit(&output, &tag).map_err(|error| format!(
        "{error} (git ls-remote --tags {remote}); push it beforehand with owner approval (this program never creates or pushes tags)"
    ))?;
    if sha != candidate.commit() {
        return Err(format!(
            "release: remote tag {tag} points at {sha}, candidate is {} (git ls-remote --tags {remote})",
            candidate.commit()
        ));
    }
    Ok(sha)
}

pub const BCR_WANT_MODULE: &str = "rules_dx";
pub const SIGNING_TRUST_ROOT: &str = "https://tuf-repo-cdn.sigstore.dev";
pub const SIGNING_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
pub const SIGNING_ISSUER_DEFAULT: &str = "https://token.actions.githubusercontent.com";
pub const PROVENANCE_BUILDER_DRY_RUN: &str = concat!(
    release_repository!(),
    "/.github/workflows/publish-dry-run.yml"
);
pub const PROVENANCE_BUILDER_RELEASE: &str =
    concat!(release_repository!(), "/.github/workflows/release.yml");

pub fn provenance_builder_error(builder: &str) -> Result<(), String> {
    if builder == PROVENANCE_BUILDER_DRY_RUN || builder == PROVENANCE_BUILDER_RELEASE {
        return Ok(());
    }
    Err(format!(
        "provenance: invalid builder '{builder}': want '{PROVENANCE_BUILDER_DRY_RUN}' (dry-run demo only) or '{PROVENANCE_BUILDER_RELEASE}' (owner-approved release)"
    ))
}

fn basename_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

pub fn render_bcr_dry_run(module: &str, version: &str, inputs: &[String]) -> String {
    let mut out = String::new();
    out.push_str("bcr: dry run (BCR_DRY_RUN=1); would submit, submitting nothing:\n");
    out.push_str(&format!("  module: {module}\n"));
    out.push_str(&format!("  version: {version}\n"));
    for input in inputs {
        out.push_str(&format!("  input: {} ({input})\n", basename_of(input)));
    }
    out.push_str(&format!(
        "  command: gh pr create --repo bazelbuild/bazel-central-registry --title \"Add {module}@{version}\" --body \"Owner-approved BCR submission for {module}@{version}\"\n"
    ));
    out.push_str("  presubmit: bazel test @rules_dx//... (seed host) + BCR presubmit.yml\n");
    out
}

pub fn bcr_run(
    module: &str,
    version: &str,
    inputs: &[String],
    dry_run: bool,
    approved: bool,
) -> Result<String, String> {
    if module != BCR_WANT_MODULE {
        return Err(format!("bcr: invalid module '{module}': want 'rules_dx'"));
    }
    if dry_run {
        return Ok(render_bcr_dry_run(module, version, inputs));
    }
    if version == "0.0.0" {
        return Err(
            "bcr: version 0.0.0 is unpublishable (shape check only); a real submission needs an owner-approved SemVer release version"
                .to_owned(),
        );
    }
    if !approved {
        return Err(
            "bcr: submission needs explicit owner approval (BCR_APPROVE=1); run with BCR_DRY_RUN=1 to print the would-submit PR"
                .to_owned(),
        );
    }
    let mut out = String::new();
    out.push_str(&format!(
        "bcr: owner-approved submission for {module}@{version} (human-run path only)\n"
    ));
    for input in inputs {
        out.push_str(&format!("  input: {} ({input})\n", basename_of(input)));
    }
    out.push_str(
        "bcr: open the BCR PR manually with the inputs above (this program never pushes itself)\n",
    );
    Ok(out)
}

pub struct CosignPin<'a> {
    pub path: &'a str,
    pub version: &'a str,
    pub sha256: &'a str,
}

pub fn render_signing_dry_run(
    identity: &str,
    issuer: &str,
    assets: &[String],
    cosign_version: &str,
    outdir: &Path,
) -> String {
    let mut out = String::new();
    out.push_str("signing: dry run (RELEASE_SIGN_DRY_RUN=1); would sign, publishing nothing:\n");
    out.push_str(&format!("  trust root: {SIGNING_TRUST_ROOT}\n"));
    out.push_str(&format!("  identity: {identity}\n"));
    out.push_str(&format!("  issuer: {issuer}\n"));
    out.push_str(&format!(
        "  cosign: {cosign_version} (pinned and sha256-checked per deploy/release/cosign_pins.bzl)\n"
    ));
    out.push_str(&format!(
        "  bundle media type: {SIGNING_BUNDLE_MEDIA_TYPE} (Sigstore bundle v0.3; 0.1/0.2 only if declared)\n"
    ));
    for asset in assets {
        let base = basename_of(asset);
        let bundle = signing_bundle_for(asset, outdir);
        out.push_str(&format!("  asset: {base} ({asset})\n"));
        out.push_str(&format!(
            "  command: cosign sign-blob --yes --bundle {bundle} {asset}\n"
        ));
        out.push_str(&format!(
            "  command: gh attestation create {asset} --bundle {bundle}\n"
        ));
    }
    out.push_str(&format!(
        "  verify: cosign verify-blob --bundle <bundle> --certificate-identity {identity} --certificate-oidc-issuer {issuer} <binary>\n"
    ));
    out
}

pub fn signing_run(
    identity: &str,
    issuer: &str,
    assets: &[String],
    dry_run: bool,
    pin: &CosignPin<'_>,
    outdir: &Path,
) -> Result<String, String> {
    if assets.is_empty() {
        return Err("signing: need at least one artifact".to_owned());
    }
    if identity.is_empty() {
        return Err(
            "signing: missing SIGNING_IDENTITY (owner-approved release workflow identity)"
                .to_owned(),
        );
    }
    if dry_run {
        return Ok(render_signing_dry_run(
            identity,
            issuer,
            assets,
            pin.version,
            outdir,
        ));
    }
    signing_live(identity, issuer, assets, pin, outdir)
}

pub fn signing_sign_argv(cosign: &str, asset: &str, bundle: &str) -> Vec<String> {
    vec![
        cosign.to_owned(),
        "sign-blob".to_owned(),
        "--yes".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        asset.to_owned(),
    ]
}

pub fn signing_verify_argv(
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

pub fn cosign_git_version(version_output: &str) -> Option<&str> {
    version_output
        .lines()
        .find_map(|line| line.trim_start().strip_prefix("GitVersion:"))
        .map(str::trim)
}

pub fn signing_version_ok(version_output: &str, want: &str) -> bool {
    cosign_git_version(version_output) == Some(want)
}

fn signing_bundle_for(asset: &str, outdir: &Path) -> String {
    outdir
        .join(format!("{}.bundle", basename_of(asset)))
        .to_string_lossy()
        .into_owned()
}

/// Runs `argv` with the inherited environment and working directory.
fn run_output(argv: &[String]) -> io::Result<std::process::Output> {
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invocation needs a binary"))?;
    std::process::Command::new(binary).args(args).output()
}

fn signing_live(
    identity: &str,
    issuer: &str,
    assets: &[String],
    pin: &CosignPin<'_>,
    outdir: &Path,
) -> Result<String, String> {
    let actual = dx_digest::sha256_file_hex(std::path::Path::new(pin.path))
        .map_err(|error| format!("signing: cannot hash cosign at {}: {error}", pin.path))?;
    if actual != pin.sha256 {
        return Err(format!(
            "signing: cosign at {} has sha256 {actual}, want {} (pinned per deploy/release/cosign_pins.bzl); refusing to sign",
            pin.path, pin.sha256
        ));
    }
    let version_argv = vec![pin.path.to_owned(), "version".to_owned()];
    let version_out = run_output(&version_argv)
        .map_err(|error| format!("signing: cannot run '{} version': {error}", pin.path))?;
    let version_text = String::from_utf8_lossy(&version_out.stdout).into_owned()
        + &String::from_utf8_lossy(&version_out.stderr);
    if !version_out.status.success() || !signing_version_ok(&version_text, pin.version) {
        return Err(format!(
            "signing: cosign GitVersion must be {} (pinned per deploy/release/cosign_pins.bzl); got: {}",
            pin.version,
            cosign_git_version(&version_text).unwrap_or("none")
        ));
    }
    std::fs::create_dir_all(outdir)
        .map_err(|error| format!("signing: cannot create {}: {error}", outdir.display()))?;
    let mut out = String::new();
    out.push_str(&format!(
        "signing: live sign with cosign {} on {SIGNING_TRUST_ROOT}:\n",
        pin.version
    ));
    for asset in assets {
        let bundle = signing_bundle_for(asset, outdir);
        let sign_argv = signing_sign_argv(pin.path, asset, &bundle);
        let sign_out = run_output(&sign_argv)
            .map_err(|error| format!("signing: cannot run '{}': {error}", sign_argv.join(" ")))?;
        if !sign_out.status.success() {
            return Err(format!(
                "signing: '{}' failed for {asset}; publishing nothing",
                sign_argv.join(" ")
            ));
        }
        let verify_argv = signing_verify_argv(pin.path, asset, &bundle, identity, issuer);
        let verify_out = run_output(&verify_argv)
            .map_err(|error| format!("signing: cannot run '{}': {error}", verify_argv.join(" ")))?;
        if !verify_out.status.success() {
            return Err(format!(
                "signing: '{}' failed for {asset}; bundle rejected, publishing nothing",
                verify_argv.join(" ")
            ));
        }
        out.push_str(&format!("  signed: {asset} -> {bundle} (verified)\n"));
    }
    Ok(out)
}

pub fn render_release_dry_run(tag: &str, approve: &str) -> String {
    let mut out = String::new();
    out.push_str("release: dry run (RELEASE_DRY_RUN=1); would release, publishing nothing:\n");
    out.push_str(&format!("  tag: {tag}\n"));
    out.push_str(&format!(
        "  approve: {approve} (real release needs RELEASE_APPROVE=1 + owner approval)\n"
    ));
    out.push_str("  steps:\n");
    out.push_str(
        "    1. bazel build //cli/cli:dx //cli/cli:dx_standalone //cli/cli:man_pages (seed matrix cell dx-linux-x86_64)\n",
    );
    out.push_str(
        "    2. bazel build //deploy/release:release_artifacts (packaged releasable unit: audit curator plus binary plus man page plus NOTICE plus SBOM/provenance)\n",
    );
    out.push_str(
        "    3. bazel build //deploy/release:sbom_demo //deploy/release:notice_demo (SPDX-2.3 plus SLSA v1 with subject digest equal to artifact sha256; aggregated NOTICE from the audited inventory, signed alongside the SBOM pair)\n",
    );
    out.push_str(
        "    4. RELEASE_SIGN_DRY_RUN=1 bazel run //deploy/release:signing_demo (would-sign cosign + attestation)\n",
    );
    out.push_str(
        "    5. GH_RELEASE_DRY_RUN=1 bazel run //cli/cli:github_draft (would-create draft --draft --verify-tag)\n",
    );
    out.push_str(
        "    6. BCR_DRY_RUN=1 bazel run //deploy/release:bcr_demo (would-submit BCR PR, submits nothing)\n",
    );
    out.push_str(
        "    7. ghcr.yml dispatch + approve:true (separate workflow, push+cosign sign <digest>)\n",
    );
    out.push_str(
        "    8. bazel run //deploy/install:dx_verify --binary <dx> --bundle <bundle> --identity <id> --issuer <issuer> --notice <NOTICE> --notice-manifest <manifest> --notice-bundle <NOTICE.bundle> (fail-before-install)\n",
    );
    out.push_str(&format!(
        "  tag creation: git tag {tag} must already exist in the remote (pushed beforehand with owner approval); this program never creates or pushes tags\n"
    ));
    out.push_str("  publishing: nothing (dry run never tags, releases, submits, or pushes)\n");
    out.push_str("  ceiling: this program never creates or pushes tags\n");
    out
}

/// Local git state checked before a live release.
#[derive(Debug)]
pub struct GitState {
    pub tree_dirty: bool,
    pub head: String,
    pub tag_commit: Option<String>,
}

/// Returns the candidate a live release tag and commit name.
pub fn release_candidate(
    tag: &str,
    candidate_commit: Option<&str>,
) -> Result<candidate::Candidate, String> {
    let version = tag
        .strip_prefix('v')
        .ok_or_else(|| format!("release: tag {tag} must be v<SemVer>"))?;
    let commit = candidate_commit.ok_or_else(|| {
        format!("release: live release of {tag} needs the candidate commit: release_driver {tag} <commit>")
    })?;
    candidate::Candidate::new(version, commit).map_err(|error| format!("release: {error}"))
}

pub fn release_run(
    tag: &str,
    candidate_commit: Option<&str>,
    approve: &str,
    dry_run: bool,
    git_state: impl FnOnce() -> Result<GitState, String>,
) -> Result<String, String> {
    if dry_run {
        return Ok(render_release_dry_run(tag, approve));
    }
    if approve != "1" {
        return Err(
            "release: real release needs RELEASE_APPROVE=1 plus explicit owner approval".to_owned(),
        );
    }
    if tag == "v0.0.0-dryrun" {
        return Err(
            "release: placeholder tag v0.0.0-dryrun is dry-run only; pass a real SemVer tag already pushed to the remote"
                .to_owned(),
        );
    }
    let candidate = release_candidate(tag, candidate_commit)?;
    let commit = candidate.commit();
    let state = git_state()?;
    if state.tree_dirty {
        return Err("release: tree is dirty; release requires a clean tree".to_owned());
    }
    if state.head != commit {
        return Err(format!(
            "release: HEAD is {}, candidate is {commit}; check out the candidate commit",
            state.head
        ));
    }
    let tag_commit = state.tag_commit.ok_or_else(|| {
        format!(
            "release: local tag {tag} missing; push it beforehand with owner approval (this program never creates tags)"
        )
    })?;
    if tag_commit != commit {
        return Err(format!(
            "release: local tag {tag} points at {tag_commit}, candidate is {commit}"
        ));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "release: owner-approved human-run release for {tag}\n"
    ));
    out.push_str(&format!(
        "release: checked clean tree, explicit approval, HEAD and local tag {tag} at {commit}; executed no release step\n"
    ));
    out.push_str(&format!(
        "release: verify the remote tag SHA with `git ls-remote --tags {RELEASE_REMOTE} {tag}` before any publish; this program never creates or pushes tags\n"
    ));
    Ok(out)
}

pub fn sbom_verify_files(
    artifact: &Path,
    spdx_path: &Path,
    prov: &Path,
    builder: &str,
) -> io::Result<String> {
    let digest = dx_digest::sha256_file_hex(artifact)?;
    let base = basename(artifact)?;
    let spdx_text = std::fs::read_to_string(spdx_path)?;
    let prov_text = std::fs::read_to_string(prov)?;
    let document: SpdxDocument = serde_json::from_str(&spdx_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("sbom SPDX is not a document: {error}"),
        )
    })?;
    spdx::document_error(&document, &base, &digest).map_err(io::Error::other)?;
    let statement: provenance::Statement = serde_json::from_str(&prov_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provenance is not an in-toto statement: {error}"),
        )
    })?;
    provenance::statement_error(&statement, &base, &digest, builder)
        .map_err(|problem| io::Error::new(io::ErrorKind::InvalidData, problem))?;
    Ok(format!("sbom OK: SPDX-2.3 + SLSA v1 bind {digest}\n"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeEntry {
    pub package: String,
    pub set: String,
    pub version: String,
    pub license: String,
    pub text_path: String,
    pub text_sha256: String,
}

pub fn parse_notice_manifest(text: &str) -> Result<Vec<NoticeEntry>, String> {
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 6 {
            return Err(format!(
                "notice manifest line {}: want 6 '|' fields 'package|set|version|license|text|sha256' where text is a per-package relative path (for example 'cargo/serde/LICENSE'), got {} (5-field manifests predate the text sha256 column; regenerate the inventory)",
                index + 1,
                fields.len()
            ));
        }
        let entry = NoticeEntry {
            package: fields[0].trim().to_owned(),
            set: fields[1].trim().to_owned(),
            version: fields[2].trim().to_owned(),
            license: fields[3].trim().to_owned(),
            text_path: fields[4].trim().to_owned(),
            text_sha256: fields[5].trim().to_owned(),
        };
        if entry.package.is_empty()
            || entry.set.is_empty()
            || entry.version.is_empty()
            || entry.license.is_empty()
        {
            return Err(format!(
                "notice manifest line {}: package, set, version, and license must be non-empty",
                index + 1
            ));
        }
        if entry.text_path.is_empty() {
            if !entry.text_sha256.is_empty() {
                return Err(format!(
                    "notice manifest line {}: sha256 must be empty when no text file is named",
                    index + 1
                ));
            }
        } else if let Err(problem) = notice_text_path_error(&entry.text_path) {
            return Err(format!(
                "notice manifest line {}: text '{}' {problem}",
                index + 1,
                entry.text_path
            ));
        } else if !dx_digest::is_lower_hex(&entry.text_sha256, 32) {
            return Err(format!(
                "notice manifest line {}: sha256 for text '{}' must be 64 lowercase hex characters",
                index + 1,
                entry.text_path
            ));
        }
        entries.push(entry);
    }
    entries.sort_by(|a, b| {
        (&a.package, &a.set, &a.version, &a.license)
            .cmp(&(&b.package, &b.set, &b.version, &b.license))
    });
    for pair in entries.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        if (&a.package, &a.set, &a.version, &a.license)
            == (&b.package, &b.set, &b.version, &b.license)
        {
            return Err(format!(
                "notice manifest: duplicate entry {}@{} ({}) [{}]",
                a.package, a.version, a.license, a.set
            ));
        }
    }
    Ok(entries)
}

/// Checks that a manifest text path is a safe per-package relative path:
/// '/'-separated, no leading '/', no '\\', and no empty, '.', or '..'
/// components. Bare basenames are valid (the shortest suffix).
pub fn notice_text_path_error(text_path: &str) -> Result<(), String> {
    if text_path.contains('\\') {
        return Err("must use '/' separators, not '\\'".to_owned());
    }
    if text_path.starts_with('/') {
        return Err("must be a relative path (no leading '/')".to_owned());
    }
    for component in text_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err("must not contain empty, '.', or '..' path components".to_owned());
        }
    }
    Ok(())
}

/// The NOTICE section header one inventory entry renders as.
pub fn notice_entry_header(entry: &NoticeEntry) -> String {
    format!(
        "=== {} {} ({}) [{}] ===",
        entry.package, entry.version, entry.license, entry.set
    )
}

/// How many lines of `text` are exactly `header` (ignoring a trailing '\r').
pub fn notice_header_count(text: &str, header: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_end_matches('\r') == header)
        .count()
}

/// The body text under one NOTICE entry header, up to the next known header.
pub fn notice_entry_body<'a>(
    notice_text: &'a str,
    header: &str,
    headers: &[String],
) -> Option<&'a str> {
    let mut offset = 0;
    let mut start = None;
    for line in notice_text.split_inclusive('\n') {
        let bare = line.trim_end_matches('\n').trim_end_matches('\r');
        match start {
            None if bare == header => start = Some(offset + line.len()),
            Some(begin) if headers.iter().any(|known| known == bare) => {
                return Some(&notice_text[begin..offset]);
            }
            _ => {}
        }
        offset += line.len();
    }
    start.map(|begin| &notice_text[begin..])
}

/// The sha256 of the body `render_notice` writes under an entry header
/// (the words with trailing newlines replaced by exactly one `\n`).
pub fn notice_text_sha256(words: &str) -> String {
    let mut body = words.trim_end_matches('\n').to_owned();
    body.push('\n');
    dx_digest::sha256_hex(body.as_bytes())
}

/// Shared NOTICE verifier for the release and install sides. Errors carry no
/// prefix; callers add their own.
pub fn verify_notice_text(notice_text: &str, entries: &[NoticeEntry]) -> Result<usize, String> {
    if entries.is_empty() {
        return Err("notice manifest lists no packages; refusing empty NOTICE".to_owned());
    }
    let first = notice_text.lines().next().unwrap_or("");
    let Some(root) = first.strip_prefix("NOTICE for ") else {
        return Err("NOTICE missing 'NOTICE for <root>' header".to_owned());
    };
    let root = root.trim_end_matches('\r');
    for entry in entries {
        if entry.text_path.is_empty() {
            return Err(format!(
                "missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words; record the words in the audited inventory before bundling",
                entry.package, entry.version, entry.license
            ));
        }
    }
    let headers: Vec<String> = entries.iter().map(notice_entry_header).collect();
    for header in &headers {
        if notice_header_count(notice_text, header) > 1 {
            return Err(format!(
                "NOTICE repeats entry header '{header}' (fail closed before install)"
            ));
        }
        if notice_entry_body(notice_text, header, &headers).is_none() {
            return Err(format!(
                "NOTICE missing entry header '{header}' (fail closed before install)"
            ));
        }
    }
    let mut bodies = Vec::with_capacity(entries.len());
    for (entry, header) in entries.iter().zip(&headers) {
        let body = notice_entry_body(notice_text, header, &headers).ok_or_else(|| {
            format!("NOTICE missing entry header '{header}' (fail closed before install)")
        })?;
        if body.trim().is_empty() {
            return Err(format!(
                "NOTICE missing words for {}@{} ({}): entry body is empty",
                entry.package, entry.version, entry.license
            ));
        }
        let got = dx_digest::sha256_hex(body.as_bytes());
        if got != entry.text_sha256 {
            return Err(format!(
                "NOTICE missing words for {}@{} ({}): body sha256 {got}, manifest records {}",
                entry.package, entry.version, entry.license, entry.text_sha256
            ));
        }
        bodies.push((entry.clone(), body.to_owned()));
    }
    if render_notice(root, &bodies) != notice_text {
        return Err("NOTICE has content outside the audited entries".to_owned());
    }
    Ok(entries.len())
}

pub fn render_notice(root: &str, entries: &[(NoticeEntry, String)]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| {
        (&a.0.package, &a.0.set, &a.0.version, &a.0.license).cmp(&(
            &b.0.package,
            &b.0.set,
            &b.0.version,
            &b.0.license,
        ))
    });
    let mut out = String::new();
    out.push_str(&format!("NOTICE for {root}\n"));
    out.push_str(
        "Generated by rules_dx notice_bundle from the audited license inventory; do not edit.\n",
    );
    out.push('\n');
    for (entry, words) in &sorted {
        out.push_str(&format!("{}\n", notice_entry_header(entry)));
        let trimmed = words.trim_end_matches('\n');
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

fn read_texts(
    text_files: &[std::path::PathBuf],
) -> Result<Vec<(std::path::PathBuf, String)>, String> {
    let mut texts: Vec<(std::path::PathBuf, String)> = Vec::with_capacity(text_files.len());
    for path in text_files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_owned();
        if name.is_empty() {
            return Err(format!(
                "notice: text input {} has no UTF-8 basename",
                path.display()
            ));
        }
        if texts.iter().any(|(seen, _)| seen == path) {
            continue;
        }
        let words = std::fs::read_to_string(path)
            .map_err(|error| format!("notice: cannot read {}: {error}", path.display()))?;
        if words.trim().is_empty() {
            return Err(format!(
                "notice: missing-notice-text: {} is empty",
                path.display()
            ));
        }
        texts.push((path.clone(), words));
    }
    texts.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(texts)
}

/// The inputs whose path ends with `text_path`, compared by whole path
/// components (so `NOTLICENSE` never matches `LICENSE`), sorted by path.
pub fn notice_text_matches<'a, T>(
    text_path: &str,
    inputs: &'a [(std::path::PathBuf, T)],
) -> Vec<&'a (std::path::PathBuf, T)> {
    let wanted = Path::new(text_path);
    let mut found: Vec<_> = inputs
        .iter()
        .filter(|(path, _)| path.ends_with(wanted))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

pub(crate) fn join_notice_entries(
    manifest_text: &str,
    text_files: &[std::path::PathBuf],
) -> Result<Vec<(NoticeEntry, String)>, String> {
    let entries = parse_notice_manifest(manifest_text)?;
    if entries.is_empty() {
        return Err("notice: manifest lists no packages; refusing empty NOTICE".to_owned());
    }
    let texts = read_texts(text_files)?;
    let mut joined = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.text_path.is_empty() {
            return Err(format!(
                "notice: missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words; record the words in the audited inventory before bundling",
                entry.package, entry.version, entry.license
            ));
        }
        let found = notice_text_matches(&entry.text_path, &texts);
        let words = match found.as_slice() {
            [] => {
                return Err(format!(
                    "notice: missing-notice-text: {}@{} ({}) names text '{}' with no matching input file (matched by whole path-component suffix)",
                    entry.package, entry.version, entry.license, entry.text_path
                ));
            }
            [(_, words)] => words,
            many => {
                let paths: Vec<String> = many
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect();
                return Err(format!(
                    "notice: ambiguous-notice-text: {}@{} ({}) [{}] text '{}' matches {} inputs: {}; lengthen the manifest path so it names exactly one input",
                    entry.package,
                    entry.version,
                    entry.license,
                    entry.set,
                    entry.text_path,
                    many.len(),
                    paths.join(", ")
                ));
            }
        };
        let got = notice_text_sha256(words);
        if got != entry.text_sha256 {
            return Err(format!(
                "notice: notice-text-digest: {}@{} ({}) text '{}' sha256 {got}, manifest records {}",
                entry.package, entry.version, entry.license, entry.text_path, entry.text_sha256
            ));
        }
        joined.push((entry, words.clone()));
    }
    Ok(joined)
}

pub fn write_notice(
    manifest: &Path,
    dst: &Path,
    root: &str,
    text_files: &[std::path::PathBuf],
) -> io::Result<()> {
    let manifest_text = std::fs::read_to_string(manifest)?;
    let joined = join_notice_entries(&manifest_text, text_files).map_err(io::Error::other)?;
    std::fs::write(dst, render_notice(root, &joined).as_bytes())?;
    Ok(())
}

pub fn notice_verify_files(
    notice: &Path,
    manifest: &Path,
    text_files: &[std::path::PathBuf],
) -> io::Result<String> {
    let notice_text = std::fs::read_to_string(notice)?;
    let manifest_text = std::fs::read_to_string(manifest)?;
    let entries = parse_notice_manifest(&manifest_text).map_err(io::Error::other)?;
    if entries.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "notice: manifest lists no packages; refusing empty NOTICE",
        ));
    }
    if !notice_text.starts_with("NOTICE for ") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "notice: missing 'NOTICE for <root>' header",
        ));
    }
    join_notice_entries(&manifest_text, text_files).map_err(io::Error::other)?;
    verify_notice_text(&notice_text, &entries)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("notice: {error}")))?;
    Ok(format!("notice OK: {} entries bundled\n", entries.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORGED_BUILDER_ID: &str = "https://invalid.test/builder";

    fn scratch_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("scratch")
    }

    fn write_artifact(dir: &Path, name: &str, data: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, data).expect("write artifact");
        path
    }

    #[test]
    fn basename_rejects_missing_name() {
        assert!(basename(Path::new("/")).is_err());
        assert_eq!(
            basename(Path::new("/tmp/artifact.bin")).expect("base"),
            "artifact.bin"
        );
    }

    #[test]
    fn spdx_bytes_match_python_golden() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text = render_spdx("artifact.bin", digest, "dx", "rules_dx");
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\r\n"));
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["spdxVersion"], serde_json::json!("SPDX-2.3"));
        assert_eq!(value["dataLicense"], serde_json::json!("CC0-1.0"));
        assert_eq!(value["SPDXID"], serde_json::json!("SPDXRef-DOCUMENT"));
        assert_eq!(value["name"], serde_json::json!("dx-artifact.bin"));
        assert_eq!(
            value["documentNamespace"],
            serde_json::json!(format!(
                "https://github.com/devhusdk/rules_dx/releases/artifact.bin-{digest}"
            ))
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "  \"SPDXID\": \"SPDXRef-DOCUMENT\",");
        assert!(text.contains("\"filesAnalyzed\": false"));
        assert!(text.contains(&format!("pkg:generic/dx@{digest}")));
        assert!(text.contains("Organization: rules_dx"));
        assert!(!text.contains("verificationCode"), "{text}");
        assert!(!text.contains("packageVerificationCode"), "{text}");
        assert!(!text.contains("null"), "{text}");
        assert_eq!(
            value["relationships"],
            serde_json::json!([{
                "relatedSpdxElement": "SPDXRef-Package",
                "relationshipType": "DESCRIBES",
                "spdxElementId": "SPDXRef-DOCUMENT",
            }])
        );
    }

    #[test]
    fn release_spdx_passes_spdx_2_3_schema() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text = render_spdx("artifact.bin", digest, "dx", "rules_dx");
        assert_eq!(dx_sbom_schema::spdx_errors(&text), Vec::<String>::new());
    }

    #[test]
    fn provenance_bytes_match_python_golden() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text = render_provenance("artifact.bin", digest, PROVENANCE_BUILDER_DRY_RUN);
        assert!(text.ends_with('\n'));
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(
            value["_type"],
            serde_json::json!("https://in-toto.io/Statement/v1")
        );
        assert_eq!(
            value["predicateType"],
            serde_json::json!("https://slsa.dev/provenance/v1")
        );
        assert_eq!(
            value["subject"][0]["digest"]["sha256"],
            serde_json::json!(digest)
        );
        assert_eq!(
            value["subject"][0]["name"],
            serde_json::json!("artifact.bin")
        );
        assert_eq!(
            value["predicate"]["runDetails"]["builder"]["id"],
            serde_json::json!(PROVENANCE_BUILDER_DRY_RUN)
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[1],
            "  \"_type\": \"https://in-toto.io/Statement/v1\","
        );
    }

    #[test]
    fn bcr_bytes_match_python_golden() {
        let text = render_bcr_source("rules_dx", "1.2.3");
        assert_eq!(
            text,
            "{\n  \"integrity\": \"<integrity-filled-at-release>\",\n  \"strip_prefix\": \"rules_dx-1.2.3\",\n  \"url\": \"https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/rules_dx-1.2.3.tar.gz\"\n}\n"
        );
    }

    #[test]
    fn render_escapes_non_ascii_like_python() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text = render_spdx("artifact.bin", digest, "dx", "caf\u{e9}");
        assert!(text.contains("caf\\u00e9"));
        assert!(!text.contains("caf\u{e9}"));
        let astral = render_bcr_source("rules_dx\u{1F600}", "1.2.3");
        assert!(astral.contains("\\ud83d\\ude00"));
        assert_eq!(dx_fingerprint::ensure_ascii("caf\u{e9}"), "caf\\u00e9");
        let value = serde_json::json!({"name": "caf\u{e9}"});
        let central = dx_fingerprint::to_json_ascii_pretty(&value).expect("central pretty");
        assert!(central.contains("caf\\u00e9"));
        assert_eq!(render_pretty(&value), central);
    }

    #[test]
    fn write_helpers_round_trip_digests() {
        let scratch = scratch_dir();
        let src = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&src).expect("digest");
        let spdx_out = scratch.path().join("out.spdx.json");
        write_spdx(&src, &spdx_out, "dx", "rules_dx").expect("write spdx");
        let spdx_text = std::fs::read_to_string(&spdx_out).expect("read spdx");
        assert_eq!(
            spdx_text,
            render_spdx("artifact.bin", &digest, "dx", "rules_dx")
        );
        let prov_out = scratch.path().join("out.prov.json");
        write_provenance(&src, &prov_out, PROVENANCE_BUILDER_DRY_RUN).expect("write prov");
        let prov_text = std::fs::read_to_string(&prov_out).expect("read prov");
        assert_eq!(
            prov_text,
            render_provenance("artifact.bin", &digest, PROVENANCE_BUILDER_DRY_RUN)
        );
        let bcr_out = scratch.path().join("out.source.json");
        write_bcr_source(&bcr_out, "rules_dx", "1.2.3").expect("write bcr");
        assert_eq!(
            std::fs::read_to_string(&bcr_out).expect("read bcr"),
            render_bcr_source("rules_dx", "1.2.3")
        );
        assert!(write_spdx(&scratch.path().join("missing"), &spdx_out, "dx", "s").is_err());
        assert!(write_provenance(&scratch.path().join("missing"), &prov_out, "b").is_err());
    }

    #[test]
    fn bcr_dry_run_matches_shell() {
        let out = render_bcr_dry_run("rules_dx", "0.0.0", &["/tmp/a.spdx.json".to_owned()]);
        assert!(out.contains("bcr: dry run (BCR_DRY_RUN=1); would submit, submitting nothing:"));
        assert!(out.contains("  module: rules_dx"));
        assert!(out.contains("  version: 0.0.0"));
        assert!(out.contains("  input: a.spdx.json (/tmp/a.spdx.json)"));
        assert!(out.contains("gh pr create --repo bazelbuild/bazel-central-registry"));
        assert!(out.contains("presubmit: bazel test @rules_dx//..."));
        let ok = bcr_run("rules_dx", "0.0.0", &[], true, false).expect("dry");
        assert!(ok.contains("would submit, submitting nothing"));
        assert!(bcr_run("other", "0.0.0", &[], true, false).is_err());
        assert!(bcr_run("rules_dx", "0.0.0", &[], false, false).is_err());
        let err = bcr_run("rules_dx", "0.0.0", &[], false, true).expect_err("0.0.0");
        assert!(err.contains("unpublishable (shape check only)"));
        assert!(bcr_run("rules_dx", "1.2.3", &[], false, false).is_err());
        let live =
            bcr_run("rules_dx", "1.2.3", &["/x/y.json".to_owned()], false, true).expect("live");
        assert!(live.contains("owner-approved submission for rules_dx@1.2.3"));
        assert!(live.contains("never pushes itself"));
    }

    #[test]
    fn signing_dry_run_matches_shell() {
        let assets = ["/tmp/a.bin".to_owned()];
        let pin = CosignPin {
            path: "/nonexistent/cosign",
            version: "v9.8.7",
            sha256: "0",
        };
        let outdir = Path::new("/out");
        let out = render_signing_dry_run("IDENT", "ISSUER", &assets, pin.version, outdir);
        assert!(out.contains(
            "signing: dry run (RELEASE_SIGN_DRY_RUN=1); would sign, publishing nothing:"
        ));
        assert!(out.contains(&format!("trust root: {SIGNING_TRUST_ROOT}")));
        assert!(out.contains("cosign sign-blob --yes --bundle /out/a.bin.bundle /tmp/a.bin"));
        assert!(out.contains("gh attestation create /tmp/a.bin --bundle /out/a.bin.bundle"));
        assert!(out.contains("cosign verify-blob"));
        assert!(out.contains("cosign: v9.8.7 "));
        assert!(out.contains(SIGNING_BUNDLE_MEDIA_TYPE));
        assert!(signing_run("", "ISSUER", &assets, true, &pin, outdir).is_err());
        assert!(signing_run("ID", "ISS", &[], true, &pin, outdir).is_err());
        let err =
            signing_run("ID", "ISS", &assets, false, &pin, outdir).expect_err("missing cosign");
        assert!(
            err.contains("cannot hash cosign at /nonexistent/cosign"),
            "got {err}"
        );
        let ok = signing_run("ID", "ISS", &assets, true, &pin, outdir).expect("dry");
        assert!(ok.contains("would sign, publishing nothing"));
    }

    #[test]
    fn release_dry_run_matches_shell() {
        let out = render_release_dry_run("v0.0.0-dryrun", "0");
        assert!(out.contains("release: dry run (RELEASE_DRY_RUN=1)"));
        assert!(out.contains("never creates or pushes tags"));
        assert!(out.contains("publishing nothing"));
        assert!(out.contains("dx_verify"));
        assert!(out.contains("notice_demo"));
        assert!(out.contains("release_artifacts"));
        assert!(out.contains("subject digest equal to artifact sha256"));
        assert!(out.contains("signed alongside the SBOM pair"));
        let package = out.find("release_artifacts").expect("packaging step");
        let notice = out.find("notice_demo").expect("notice step");
        let sign = out.find("signing_demo").expect("sign step");
        let draft = out.find("github_draft").expect("draft step");
        assert!(package < notice);
        assert!(notice < sign);
        assert!(sign < draft);
        let ok = release_run("v0.0.0-dryrun", None, "0", true, git_unread).expect("dry");
        assert!(ok.contains("would release, publishing nothing"));
        assert!(release_run("v1.2.3", Some(SHA), "0", false, git_unread).is_err());
        assert!(release_run("v0.0.0-dryrun", Some(SHA), "1", false, git_unread).is_err());
        assert_eq!(
            release_run("v1.2.3", Some(SHA), "1", false, || git_ok(
                true,
                SHA,
                Some(SHA)
            )),
            Err("release: tree is dirty; release requires a clean tree".to_owned())
        );
        assert_eq!(
            release_run("v1.2.3", Some(SHA), "1", false, || git_ok(false, SHA, None)),
            Err("release: local tag v1.2.3 missing; push it beforehand with owner approval (this program never creates tags)".to_owned())
        );
        let live = release_run("v1.2.3", Some(SHA), "1", false, || {
            git_ok(false, SHA, Some(SHA))
        })
        .expect("live");
        assert!(live.contains("owner-approved human-run release for v1.2.3"));
    }

    const SHA: &str = "a948904f2f0f479b8f8197694b30184b0d2ed1c1";
    const OTHER: &str = "b948904f2f0f479b8f8197694b30184b0d2ed1c1";

    #[test]
    fn release_run_needs_a_valid_candidate_before_reading_git() {
        let error = release_run("v1.2.3", None, "1", false, git_unread).expect_err("no commit");
        assert!(error.contains("needs the candidate commit"), "{error}");
        let error =
            release_run("1.2.3", Some(SHA), "1", false, git_unread).expect_err("no v prefix");
        assert!(error.contains("must be v<SemVer>"), "{error}");
        let error =
            release_run("v1.2", Some(SHA), "1", false, git_unread).expect_err("bad version");
        assert!(error.contains("invalid version '1.2'"), "{error}");
        let error =
            release_run("v1.2.3", Some("a948904"), "1", false, git_unread).expect_err("short");
        assert!(error.contains("invalid commit"), "{error}");
        let upper = SHA.to_uppercase();
        let error = release_run("v1.2.3", Some(&upper), "1", false, git_unread).expect_err("upper");
        assert!(error.contains("invalid commit"), "{error}");
    }

    #[test]
    fn release_run_rejects_a_stale_head() {
        assert_eq!(
            release_run("v1.2.3", Some(SHA), "1", false, || git_ok(
                false,
                OTHER,
                Some(SHA)
            )),
            Err(format!(
                "release: HEAD is {OTHER}, candidate is {SHA}; check out the candidate commit"
            ))
        );
    }

    #[test]
    fn release_run_rejects_a_local_tag_pointing_elsewhere() {
        assert_eq!(
            release_run("v1.2.3", Some(SHA), "1", false, || git_ok(
                false,
                SHA,
                Some(OTHER)
            )),
            Err(format!(
                "release: local tag v1.2.3 points at {OTHER}, candidate is {SHA}"
            ))
        );
    }

    fn git_unread() -> Result<GitState, String> {
        panic!("git state read before the dry-run, approval, or placeholder gate")
    }

    #[test]
    #[should_panic(expected = "git state read")]
    fn release_reads_git_after_the_gates() {
        let _ = release_run("v1.2.3", Some(SHA), "1", false, git_unread);
    }

    fn git_ok(tree_dirty: bool, head: &str, tag: Option<&str>) -> Result<GitState, String> {
        Ok(GitState {
            tree_dirty,
            head: head.to_owned(),
            tag_commit: tag.map(str::to_owned),
        })
    }

    #[test]
    fn release_git_error_blocks_live_release() {
        assert_eq!(
            release_run("v1.2.3", Some(SHA), "1", false, || Err(
                "release: cannot read git state: x".to_owned()
            )),
            Err("release: cannot read git state: x".to_owned())
        );
    }

    #[test]
    fn release_live_reports_checks_not_execution() {
        let live = release_run("v1.2.3", Some(SHA), "1", false, || {
            git_ok(false, SHA, Some(SHA))
        })
        .expect("live");
        assert!(live.contains("executed no release step"));
        assert!(
            live.contains("git ls-remote --tags https://github.com/devhusdk/rules_dx.git v1.2.3")
        );
        assert!(!live.contains("origin"));
        assert!(live.contains("never creates or pushes tags"));
        assert!(!live.contains("proceeding step by step"));
    }

    #[test]
    fn base64_encodes_short_inputs_with_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn bcr_integrity_comes_from_the_archive_sha256() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        assert_eq!(
            bcr_integrity_of_digest(digest).expect("integrity"),
            "sha256-qUiQTy8PR5uPgZdpSzAYSw0u0cHNKh7A+4XSmaGSpEc="
        );
        assert!(bcr_integrity_of_digest("").is_err());
        assert!(bcr_integrity_of_digest("xyz").is_err());
        assert!(bcr_integrity_of_digest(&"0".repeat(63)).is_err());
        assert!(bcr_integrity_of_digest(&"0".repeat(64)).is_ok());
        assert!(bcr_integrity_of_digest(
            "A948904F2F0F479B8F8197694B30184B0D2ED1C1CD2A1EC0FB85D299A192A447"
        )
        .is_err());
    }

    #[test]
    fn candidate_source_metadata_comes_from_one_sha() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text =
            render_bcr_source_for_digest("rules_dx", "1.2.3-rc1", digest).expect("candidate");
        assert_eq!(
            text,
            "{\n  \"integrity\": \"sha256-qUiQTy8PR5uPgZdpSzAYSw0u0cHNKh7A+4XSmaGSpEc=\",\n  \"strip_prefix\": \"rules_dx-1.2.3-rc1\",\n  \"url\": \"https://github.com/devhusdk/rules_dx/releases/download/v1.2.3-rc1/rules_dx-1.2.3-rc1.tar.gz\"\n}\n"
        );
        bcr_candidate_error(&text, "rules_dx", "1.2.3-rc1", digest).expect("self-validates");
        assert_eq!(
            bcr_source_url("rules_dx", "1.2.3"),
            "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/rules_dx-1.2.3.tar.gz"
        );
        assert_eq!(bcr_strip_prefix("rules_dx", "1.2.3"), "rules_dx-1.2.3");
    }

    #[test]
    fn candidate_metadata_rejects_shape_versions_and_foreign_modules() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        assert!(render_bcr_source_for_digest("other", "1.2.3", digest).is_err());
        assert!(render_bcr_source_for_digest("rules_dx", "0.0.0", digest).is_err());
        for bad in ["", "v1", "1.2", "1.2.3.4", "1..3", ".2.3"] {
            assert!(
                render_bcr_source_for_digest("rules_dx", bad, digest).is_err(),
                "version {bad:?} must fail"
            );
        }
    }

    #[test]
    fn candidate_source_rejects_a_swapped_archive_digest() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text =
            render_bcr_source_for_digest("rules_dx", "1.2.3-rc1", digest).expect("candidate");
        let zeros = "0".repeat(64);
        let error =
            bcr_candidate_error(&text, "rules_dx", "1.2.3-rc1", &zeros).expect_err("swapped");
        assert!(error.contains("integrity mismatch"), "got {error}");
        assert!(error.contains("immutable version artifacts"), "got {error}");
    }

    #[test]
    fn candidate_source_rejects_wrong_prefix_and_url() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text =
            render_bcr_source_for_digest("rules_dx", "1.2.3-rc1", digest).expect("candidate");
        let error =
            bcr_candidate_error(&text, "rules_dx", "1.2.4", digest).expect_err("wrong version");
        assert!(error.contains("strip_prefix mismatch"), "got {error}");
        let other_module =
            render_bcr_source_for_digest("rules_dx", "1.2.3-rc1", digest).expect("candidate");
        let error = bcr_candidate_error(&other_module, "other_rules", "1.2.3-rc1", digest)
            .expect_err("wrong module");
        assert!(error.contains("strip_prefix mismatch"), "got {error}");
        let tampered = text.replace("v1.2.3-rc1/rules_dx-1.2.3-rc1", "v9.9.9/rules_dx-9.9.9");
        let error =
            bcr_candidate_error(&tampered, "rules_dx", "1.2.3-rc1", digest).expect_err("tampered");
        assert!(error.contains("url mismatch"), "got {error}");
    }

    #[test]
    fn interrupted_candidate_publication_fails_closed() {
        let digest = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447";
        let text =
            render_bcr_source_for_digest("rules_dx", "1.2.3-rc1", digest).expect("candidate");
        let cut = text.len() / 2;
        let error = bcr_candidate_error(&text[..cut], "rules_dx", "1.2.3-rc1", digest)
            .expect_err("truncated");
        assert!(error.contains("not JSON"), "got {error}");
        let error =
            bcr_candidate_error("{}", "rules_dx", "1.2.3-rc1", digest).expect_err("empty object");
        assert!(error.contains("integrity mismatch"), "got {error}");
        let error =
            bcr_candidate_error(&text, "rules_dx", "1.2.3-rc1", "not-hex").expect_err("bad digest");
        assert!(error.contains("invalid sha256 digest"), "got {error}");
    }

    #[test]
    fn write_candidate_source_round_trips_the_archive() {
        let scratch = scratch_dir();
        let src = write_artifact(scratch.path(), "candidate.tar.gz", b"candidate bytes\n");
        let digest = dx_digest::sha256_file_hex(&src).expect("digest");
        let dst = scratch.path().join("candidate.source.json");
        let integrity =
            write_bcr_source_for_archive(&dst, "rules_dx", "1.2.3-rc1", &src).expect("write");
        assert_eq!(
            integrity,
            bcr_integrity_of_digest(&digest).expect("integrity")
        );
        let text = std::fs::read_to_string(&dst).expect("read");
        bcr_candidate_error(&text, "rules_dx", "1.2.3-rc1", &digest).expect("validates");
        assert!(write_bcr_source_for_archive(&dst, "rules_dx", "0.0.0", &src).is_err());
        assert!(write_bcr_source_for_archive(
            &dst,
            "rules_dx",
            "1.2.3-rc1",
            &scratch.path().join("missing")
        )
        .is_err());
    }

    #[test]
    fn remote_tag_error_fails_closed_without_network_or_tag() {
        let candidate = candidate::Candidate::new("1.2.3", SHA).expect("candidate");
        let output = format!("{SHA}\trefs/tags/v1.2.3\n");
        assert_eq!(
            release_remote_error(&candidate, Ok(output.clone())).expect("remote"),
            SHA
        );
        let error = release_remote_error(&candidate, Err("fatal: unable to access".to_owned()))
            .expect_err("no network");
        assert!(
            error.contains("cannot verify remote tag v1.2.3"),
            "got {error}"
        );
        assert!(error.contains("fatal: unable to access"), "got {error}");
        assert!(error.contains(RELEASE_REMOTE), "got {error}");
        assert!(
            error.contains("never creates or pushes tags"),
            "got {error}"
        );
        let other = candidate::Candidate::new("9.9.9", SHA).expect("candidate");
        let error = release_remote_error(&other, Ok(output)).expect_err("missing tag");
        assert!(error.contains("remote has no tag v9.9.9"), "got {error}");
        assert!(!error.contains("origin"), "got {error}");
    }

    #[test]
    fn remote_tag_error_rejects_a_tag_pointing_elsewhere() {
        let candidate = candidate::Candidate::new("1.2.3", SHA).expect("candidate");
        let output = format!("{OTHER}\trefs/tags/v1.2.3\n");
        assert_eq!(
            release_remote_error(&candidate, Ok(output)),
            Err(format!("release: remote tag v1.2.3 points at {OTHER}, candidate is {SHA} (git ls-remote --tags {RELEASE_REMOTE})"))
        );
        let error =
            release_remote_error(&candidate, Ok("garbage\n".to_owned())).expect_err("malformed");
        assert!(error.contains("malformed"), "got {error}");
    }

    #[test]
    fn release_identity_comes_from_one_repository() {
        assert_eq!(RELEASE_REPOSITORY, "https://github.com/devhusdk/rules_dx");
        assert_eq!(RELEASE_REMOTE, "https://github.com/devhusdk/rules_dx.git");
        assert_eq!(
            PROVENANCE_BUILDER_DRY_RUN,
            "https://github.com/devhusdk/rules_dx/.github/workflows/publish-dry-run.yml"
        );
        assert_eq!(
            PROVENANCE_BUILDER_RELEASE,
            "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml"
        );
    }

    #[test]
    fn sbom_verify_binds_digest() {
        let scratch = scratch_dir();
        let artifact = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&artifact).expect("digest");
        let spdx = scratch.path().join("artifact.spdx.json");
        write_spdx(&artifact, &spdx, "dx", "rules_dx").expect("spdx");
        let prov = scratch.path().join("artifact.prov.json");
        write_provenance(&artifact, &prov, PROVENANCE_BUILDER_DRY_RUN).expect("prov");
        let ok =
            sbom_verify_files(&artifact, &spdx, &prov, PROVENANCE_BUILDER_DRY_RUN).expect("verify");
        assert!(ok.contains("sbom OK: SPDX-2.3 + SLSA v1 bind"));
        assert!(ok.contains(&digest));
        std::fs::write(&spdx, b"{\"spdxVersion\": \"SPDX-3.0\"}").expect("rewrite");
        assert!(sbom_verify_files(&artifact, &spdx, &prov, PROVENANCE_BUILDER_DRY_RUN).is_err());
    }

    #[test]
    fn sbom_verify_reads_spdx_fields_not_text() {
        let scratch = scratch_dir();
        let artifact = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&artifact).expect("digest");
        let spdx_path = scratch.path().join("artifact.spdx.json");
        write_spdx(&artifact, &spdx_path, "dx", "rules_dx").expect("spdx");
        let prov = scratch.path().join("artifact.prov.json");
        write_provenance(&artifact, &prov, PROVENANCE_BUILDER_DRY_RUN).expect("prov");
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&spdx_path).expect("read"))
                .expect("json");
        let compact = serde_json::to_string(&value).expect("compact");
        assert!(
            !compact.contains("\"spdxVersion\": \"SPDX-2.3\""),
            "compact differs"
        );
        let others = scratch.path().join("artifact.prov.json");
        for (label, text, ok) in [
            ("reordered", reordered_spdx(&value), true),
            ("compact", compact, true),
            (
                "reordered wrong digest",
                reordered_spdx(&replaced_digest(&value, &digest, &"0".repeat(64))),
                false,
            ),
        ] {
            std::fs::write(&spdx_path, text.as_bytes()).expect("rewrite");
            let result =
                sbom_verify_files(&artifact, &spdx_path, &others, PROVENANCE_BUILDER_DRY_RUN);
            assert_eq!(result.is_ok(), ok, "{label}: {result:?}");
        }
    }

    /// Renders a document with its top-level keys in reverse order and compact values.
    fn reordered_spdx(value: &serde_json::Value) -> String {
        let entries: Vec<String> = value
            .as_object()
            .expect("object")
            .iter()
            .rev()
            .map(|(key, field)| {
                format!(
                    "  {key:?}: {}",
                    serde_json::to_string(field).expect("field")
                )
            })
            .collect();
        format!("{{\n{}\n}}", entries.join(",\n"))
    }

    #[test]
    fn sbom_verify_reads_provenance_fields_not_text() {
        let scratch = scratch_dir();
        let artifact = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&artifact).expect("digest");
        let spdx = scratch.path().join("artifact.spdx.json");
        write_spdx(&artifact, &spdx, "dx", "rules_dx").expect("spdx");
        let prov = scratch.path().join("artifact.prov.json");
        let base = render_provenance("artifact.bin", &digest, PROVENANCE_BUILDER_DRY_RUN);
        let value: serde_json::Value = serde_json::from_str(&base).expect("json");
        let with = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut copy = value.clone();
            edit(&mut copy);
            serde_json::to_string(&copy).expect("compact")
        };
        let release = PROVENANCE_BUILDER_RELEASE;
        let evil = format!("{release}.evil");
        let duplicate = base.replacen(
            "\"_type\"",
            "\"_type\": \"https://in-toto.io/Statement/v1\",\n  \"_type\"",
            1,
        );
        let cases: Vec<(&str, String, &str, Result<(), &str>)> = vec![
            ("compact", with(&|_| {}), PROVENANCE_BUILDER_DRY_RUN, Ok(())),
            (
                "reordered",
                reordered_spdx(&value),
                PROVENANCE_BUILDER_DRY_RUN,
                Ok(()),
            ),
            (
                "prefix builder",
                with(&|v| v["predicate"]["runDetails"]["builder"]["id"] = evil.clone().into()),
                release,
                Err("is not an allowlisted"),
            ),
            (
                "builder only in externalParameters",
                with(&|v| {
                    v["predicate"]["runDetails"]["builder"]["id"] = "https://evil.example".into();
                    v["predicate"]["buildDefinition"]["externalParameters"]["b"] = release.into();
                }),
                release,
                Err("is not an allowlisted"),
            ),
            (
                "digest only in externalParameters",
                with(&|v| {
                    v["predicate"]["buildDefinition"]["externalParameters"]["d"] =
                        digest.clone().into();
                    v["subject"][0]["digest"]["sha256"] = "0".repeat(64).into();
                }),
                PROVENANCE_BUILDER_DRY_RUN,
                Err("missing subject digest"),
            ),
            (
                "other subject name",
                with(&|v| v["subject"][0]["name"] = "other.bin".into()),
                PROVENANCE_BUILDER_DRY_RUN,
                Err("missing subject digest"),
            ),
            (
                "dry-run provenance for release check",
                with(&|_| {}),
                release,
                Err("want"),
            ),
            (
                "missing _type",
                with(&|v| {
                    v.as_object_mut().expect("object").remove("_type");
                }),
                PROVENANCE_BUILDER_DRY_RUN,
                Err("missing in-toto Statement v1"),
            ),
            (
                "not json",
                "not json".to_owned(),
                PROVENANCE_BUILDER_DRY_RUN,
                Err("not an in-toto statement"),
            ),
            (
                "duplicate _type",
                duplicate,
                PROVENANCE_BUILDER_DRY_RUN,
                Err("not an in-toto statement"),
            ),
        ];
        for (label, text, builder, want) in cases {
            std::fs::write(&prov, text.as_bytes()).expect("write");
            let result = sbom_verify_files(&artifact, &spdx, &prov, builder);
            match want {
                Ok(()) => assert!(result.is_ok(), "{label}: {result:?}"),
                Err(part) => {
                    let err = result.expect_err(label).to_string();
                    assert!(err.contains(part), "{label}: want {part}, got {err}");
                }
            }
        }
    }

    fn replaced_digest(
        value: &serde_json::Value,
        digest: &str,
        replacement: &str,
    ) -> serde_json::Value {
        let text = serde_json::to_string(value).expect("compact");
        serde_json::from_str(&text.replace(digest, replacement)).expect("json")
    }

    #[test]
    fn provenance_builder_allowlist_rejects_forgeries() {
        assert!(provenance_builder_error(PROVENANCE_BUILDER_DRY_RUN).is_ok());
        assert!(provenance_builder_error(PROVENANCE_BUILDER_RELEASE).is_ok());
        assert!(provenance_builder_error(FORGED_BUILDER_ID).is_err());
        assert!(provenance_builder_error("https://example.com/builder").is_err());
        assert!(provenance_builder_error("").is_err());
        let scratch = scratch_dir();
        let src = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let dst = scratch.path().join("out.prov.json");
        assert!(write_provenance(&src, &dst, FORGED_BUILDER_ID).is_err());
        assert!(!dst.exists());
    }

    #[test]
    fn sbom_verify_rejects_forged_builder() {
        let scratch = scratch_dir();
        let artifact = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&artifact).expect("digest");
        let spdx = scratch.path().join("artifact.spdx.json");
        write_spdx(&artifact, &spdx, "dx", "rules_dx").expect("spdx");
        let prov = scratch.path().join("forged.prov.json");
        let forged = render_provenance("artifact.bin", &digest, FORGED_BUILDER_ID);
        std::fs::write(&prov, forged.as_bytes()).expect("write forged");
        assert!(sbom_verify_files(&artifact, &spdx, &prov, PROVENANCE_BUILDER_DRY_RUN).is_err());
    }

    #[test]
    fn signing_argv_pins_bundle_and_version() {
        let sign = signing_sign_argv("/pin/cosign", "/tmp/a.bin", "a.bin.bundle");
        assert_eq!(
            sign,
            vec![
                "/pin/cosign",
                "sign-blob",
                "--yes",
                "--bundle",
                "a.bin.bundle",
                "/tmp/a.bin",
            ]
        );
        let verify = signing_verify_argv(
            "/pin/cosign",
            "/tmp/a.bin",
            "a.bin.bundle",
            "IDENT",
            "ISSUER",
        );
        assert_eq!(
            verify,
            vec![
                "/pin/cosign",
                "verify-blob",
                "--bundle",
                "a.bin.bundle",
                "--certificate-identity",
                "IDENT",
                "--certificate-oidc-issuer",
                "ISSUER",
                "/tmp/a.bin",
            ]
        );
        let real =
            "  ______   ______\nGitVersion:    v9.8.7\nGitCommit:     abc\nGoVersion:     go1.25\n";
        assert_eq!(cosign_git_version(real), Some("v9.8.7"));
        assert!(signing_version_ok(real, "v9.8.7"));
        assert!(!signing_version_ok("GitVersion:    v9.8.70\n", "v9.8.7"));
        assert!(!signing_version_ok("GitVersion:    v9.8.6\n", "v9.8.7"));
        assert!(!signing_version_ok(
            "cosign version v9.8.7 (go1.25)\n",
            "v9.8.7"
        ));
        assert!(!signing_version_ok(
            "GitCommit: v9.8.7\nGitVersion: v9.8.8\n",
            "v9.8.7"
        ));
        assert!(!signing_version_ok("", "v9.8.7"));
    }

    const FIXTURE_WORDS_A: &str = "Fixture words for demo-lib-a (MIT placeholder).\n";
    const FIXTURE_WORDS_B: &str = "Fixture words for demo-lib-b (Apache-2.0 placeholder).\n";

    fn fixture_manifest() -> String {
        format!(
            "demo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt|{}\ndemo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt|{}\n",
            notice_text_sha256(FIXTURE_WORDS_B),
            notice_text_sha256(FIXTURE_WORDS_A)
        )
    }

    fn notice_fixture(dir: &Path) -> (std::path::PathBuf, Vec<std::path::PathBuf>) {
        let manifest = dir.join("inventory.txt");
        std::fs::write(&manifest, fixture_manifest()).expect("manifest");
        let a = dir.join("demo-lib-a.txt");
        std::fs::write(&a, FIXTURE_WORDS_A).expect("words a");
        let b = dir.join("demo-lib-b.txt");
        std::fs::write(&b, FIXTURE_WORDS_B).expect("words b");
        (manifest, vec![a, b])
    }

    #[test]
    fn notice_manifest_parses_and_sorts() {
        let entries = parse_notice_manifest(&format!(
            "# invented distributed-tier root\n\n{}",
            fixture_manifest()
        ))
        .expect("parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].package, "demo-lib-a");
        assert_eq!(entries[1].package, "demo-lib-b");
        assert!(parse_notice_manifest("only|four|fields|here|x").is_err());
        assert!(
            parse_notice_manifest(&format!("|npm|1.0.0|MIT|a.txt|{}", "0".repeat(64))).is_err()
        );
    }

    #[test]
    fn notice_render_is_deterministic_over_input_order() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let manifest_text = std::fs::read_to_string(&manifest).expect("read");
        let first = join_notice_entries(&manifest_text, &texts).expect("join");
        let mut rev = texts.clone();
        rev.reverse();
        let second = join_notice_entries(&manifest_text, &rev).expect("join rev");
        assert_eq!(
            render_notice("//demo:root", &first),
            render_notice("//demo:root", &second)
        );
        let text = render_notice("//demo:root", &first);
        assert!(text.starts_with("NOTICE for //demo:root\n"));
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\r\n"));
        let a = text
            .find("=== demo-lib-a 1.0.0 (MIT) [cargo] ===")
            .expect("a header");
        let b = text
            .find("=== demo-lib-b 2.3.4 (Apache-2.0) [npm] ===")
            .expect("b header");
        assert!(a < b);
        assert!(text.contains("Fixture words for demo-lib-a"));
    }

    #[test]
    fn notice_write_rebuilds_byte_identical() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let first = scratch.path().join("first.NOTICE");
        write_notice(&manifest, &first, "//demo:root", &texts).expect("first");
        let second = scratch.path().join("second.NOTICE");
        write_notice(&manifest, &second, "//demo:root", &texts).expect("second");
        assert_eq!(
            std::fs::read(&first).expect("read first"),
            std::fs::read(&second).expect("read second")
        );
        let ok = notice_verify_files(&first, &manifest, &texts).expect("verify");
        assert!(ok.contains("notice OK: 2 entries bundled"));
    }

    #[test]
    fn notice_missing_text_fails_actionable() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join("inventory.txt");
        std::fs::write(&manifest, "demo-lib-a|cargo|1.0.0|MIT||\n").expect("manifest");
        let out = scratch.path().join("out.NOTICE");
        let err = write_notice(&manifest, &out, "//demo:root", &[]).expect_err("missing");
        assert!(err.to_string().contains("missing-notice-text"));
        let (good_manifest, texts) = notice_fixture(scratch.path());
        let notice = scratch.path().join("good.NOTICE");
        write_notice(&good_manifest, &notice, "//demo:root", &texts).expect("good");
        std::fs::write(&notice, "tampered bytes\n").expect("tamper");
        let err = notice_verify_files(&notice, &good_manifest, &texts).expect_err("tampered");
        assert!(err.to_string().contains("NOTICE"));
    }

    #[test]
    fn bin_shims_share_failure_rendering() {
        assert_eq!(
            bin_usage("release_tool spdx", "<artifact> <spdx> <prov>"),
            1
        );
        assert_eq!(
            bin_cannot_write(
                "release_tool notice",
                Path::new("/nope/notice.NOTICE"),
                "read-only filesystem"
            ),
            1
        );
        assert_eq!(bin_error("release: owner approval missing"), 1);
    }

    #[test]
    fn write_spdx_fails_when_target_unwritable() {
        let scratch = scratch_dir();
        let src = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        assert!(write_spdx(&src, scratch.path(), "dx", "rules_dx").is_err());
    }

    #[test]
    fn sbom_verify_rejects_broken_bindings() {
        let scratch = scratch_dir();
        let artifact = write_artifact(scratch.path(), "artifact.bin", b"hello world\n");
        let digest = dx_digest::sha256_file_hex(&artifact).expect("digest");
        let zeros = "0".repeat(digest.len());
        let spdx_ok = scratch.path().join("ok.spdx.json");
        write_spdx(&artifact, &spdx_ok, "dx", "rules_dx").expect("spdx ok");
        let prov_ok = scratch.path().join("ok.prov.json");
        write_provenance(&artifact, &prov_ok, PROVENANCE_BUILDER_DRY_RUN).expect("prov ok");
        let spdx_bad = scratch.path().join("bad.spdx.json");
        std::fs::write(
            &spdx_bad,
            render_spdx("artifact.bin", &zeros, "dx", "rules_dx"),
        )
        .expect("spdx bad");
        let err = sbom_verify_files(&artifact, &spdx_bad, &prov_ok, PROVENANCE_BUILDER_DRY_RUN)
            .expect_err("spdx digest");
        assert!(
            err.to_string().contains("SPDX missing artifact digest"),
            "got {err}"
        );
        let prov_text = std::fs::read_to_string(&prov_ok).expect("prov text");
        for (variant, want) in [
            (
                prov_text.replace(
                    "https://in-toto.io/Statement/v1",
                    "https://in-toto.io/Statement/v2",
                ),
                "provenance missing in-toto Statement v1",
            ),
            (
                prov_text.replace(
                    "https://slsa.dev/provenance/v1",
                    "https://slsa.dev/provenance/v2",
                ),
                "provenance missing SLSA v1 predicate",
            ),
            (
                prov_text.replace(&digest, &zeros),
                "provenance missing subject digest",
            ),
        ] {
            let path = scratch.path().join("variant.prov.json");
            std::fs::write(&path, variant).expect("variant");
            let err = sbom_verify_files(&artifact, &spdx_ok, &path, PROVENANCE_BUILDER_DRY_RUN)
                .expect_err(want);
            assert!(err.to_string().contains(want), "want {want}, got {err}");
        }
    }

    #[test]
    fn notice_text_map_fails_actionable() {
        let scratch = scratch_dir();
        let err = read_texts(&[std::path::PathBuf::from("/")]).expect_err("root has no basename");
        assert!(err.contains("no UTF-8 basename"), "got {err}");
        let empty_words = scratch.path().join("empty.txt");
        std::fs::write(&empty_words, "  \n").expect("empty words");
        let err = read_texts(&[empty_words]).expect_err("empty words");
        assert!(err.contains("missing-notice-text"), "got {err}");
        let err = join_notice_entries("# no packages\n", &[]).expect_err("no entries");
        assert!(err.contains("lists no packages"), "got {err}");
        let other_words = scratch.path().join("demo-lib-b.txt");
        std::fs::write(&other_words, "Fixture words for demo-lib-b.\n").expect("words b");
        let err = join_notice_entries(
            &format!(
                "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt|{}\n",
                "0".repeat(64)
            ),
            &[other_words],
        )
        .expect_err("no matching input");
        assert!(err.contains("no matching input file"), "got {err}");
        assert!(err.contains("whole path-component suffix"), "got {err}");
        let first = scratch.path().join("a").join("LICENSE");
        let second = scratch.path().join("b").join("LICENSE");
        for (path, words) in [(&first, "Words for a.\n"), (&second, "Words for b.\n")] {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("license dir");
            std::fs::write(path, words).expect("license words");
        }
        let both = [first.clone(), second.clone()];
        assert_eq!(read_texts(&both).expect("same basename reads").len(), 2);
        let twice = [first.clone(), first.clone()];
        assert_eq!(read_texts(&twice).expect("same path twice").len(), 1);
        let joined = join_notice_entries(
            &format!(
                "demo-lib-a|cargo|1.0.0|MIT|a/LICENSE|{}\ndemo-lib-b|npm|1.0.0|MIT|b/LICENSE|{}\n",
                notice_text_sha256("Words for a.\n"),
                notice_text_sha256("Words for b.\n")
            ),
            &both,
        )
        .expect("per-package paths join");
        assert_eq!(joined[0].1, "Words for a.\n");
        assert_eq!(joined[1].1, "Words for b.\n");
        let mut rev = both.clone();
        rev.reverse();
        for inputs in [&both[..], &rev[..]] {
            let err = join_notice_entries(
                &format!(
                    "demo-lib-a|cargo|1.0.0|MIT|LICENSE|{}\n",
                    notice_text_sha256("Words for a.\n")
                ),
                inputs,
            )
            .expect_err("bare basename is ambiguous");
            assert!(err.contains("ambiguous-notice-text"), "got {err}");
            let want = format!(
                "matches 2 inputs: {}, {}",
                first.display(),
                second.display()
            );
            assert!(err.contains(&want), "want {want}, got {err}");
            assert!(err.contains("lengthen the manifest path"), "got {err}");
        }
    }

    #[test]
    fn notice_text_suffix_match_is_component_wise() {
        let scratch = scratch_dir();
        let inputs: Vec<(std::path::PathBuf, ())> = ["x/a/LICENSE", "x/ba/LICENSE", "y/NOTLICENSE"]
            .iter()
            .map(|path| (std::path::PathBuf::from(path), ()))
            .chain(std::iter::once((
                scratch.path().join("abs").join("COPYING"),
                (),
            )))
            .collect();
        let found = |want: &str| -> Vec<String> {
            notice_text_matches(want, &inputs)
                .iter()
                .map(|(path, _)| path.display().to_string())
                .collect()
        };
        assert_eq!(found("a/LICENSE"), vec!["x/a/LICENSE"]);
        assert_eq!(found("x/a/LICENSE"), vec!["x/a/LICENSE"]);
        assert_eq!(found("LICENSE"), vec!["x/a/LICENSE", "x/ba/LICENSE"]);
        assert!(found("ICENSE").is_empty());
        assert_eq!(found("NOTLICENSE"), vec!["y/NOTLICENSE"]);
        assert_eq!(
            found("abs/COPYING"),
            vec![scratch
                .path()
                .join("abs")
                .join("COPYING")
                .display()
                .to_string()]
        );
        let mut rev = inputs.clone();
        rev.reverse();
        let rev_found: Vec<String> = notice_text_matches("LICENSE", &rev)
            .iter()
            .map(|(path, _)| path.display().to_string())
            .collect();
        assert_eq!(rev_found, found("LICENSE"));
    }

    #[test]
    fn notice_manifest_rejects_unsafe_text_paths() {
        for bad in ["/abs", "a/../b", "a//b", "./a", "a\\b", "a/", ".."] {
            let err = parse_notice_manifest(&format!(
                "# header\ndemo-lib-a|cargo|1.0.0|MIT|{bad}|{}\n",
                "0".repeat(64)
            ))
            .expect_err(bad);
            assert!(err.contains("line 2"), "{bad}: got {err}");
            assert!(err.contains(&format!("text '{bad}'")), "{bad}: got {err}");
        }
        for good in ["LICENSE", "cargo/serde/LICENSE", "a.b/LICENSE-MIT"] {
            parse_notice_manifest(&format!(
                "demo-lib-a|cargo|1.0.0|MIT|{good}|{}\n",
                "0".repeat(64)
            ))
            .expect(good);
        }
    }

    #[test]
    fn notice_manifest_rejects_duplicate_identity() {
        let sha = "0".repeat(64);
        let err = parse_notice_manifest(&format!(
            "shared|npm|1.0.0|MIT|npm/LICENSE|{sha}\nother|npm|1.0.0|MIT|o/LICENSE|{sha}\nshared|npm|1.0.0|MIT|x/LICENSE|{sha}\n"
        ))
        .expect_err("duplicate identity");
        assert!(
            err.contains("duplicate entry shared@1.0.0 (MIT) [npm]"),
            "got {err}"
        );
        let entries = parse_notice_manifest(&format!(
            "shared|npm|1.0.0|MIT|npm/shared/LICENSE|{sha}\nshared|cargo|1.0.0|MIT|cargo/shared/LICENSE|{sha}\n"
        ))
        .expect("same package in two sets");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].set, "cargo");
        assert_eq!(entries[1].set, "npm");
    }

    #[test]
    fn notice_verify_rejects_repeated_header() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let manifest_text = std::fs::read_to_string(&manifest).expect("read");
        let entries = parse_notice_manifest(&manifest_text).expect("parse");
        let joined = join_notice_entries(&manifest_text, &texts).expect("join");
        let text = render_notice("//demo:root", &joined);
        let header = "=== demo-lib-b 2.3.4 (Apache-2.0) [npm] ===";
        assert_eq!(notice_header_count(&text, header), 1);
        let repeated = format!("{text}{header}\n{FIXTURE_WORDS_B}");
        assert_eq!(notice_header_count(&repeated, header), 2);
        let err = verify_notice_text(&repeated, &entries).expect_err("repeated header");
        assert!(
            err.contains(&format!("NOTICE repeats entry header '{header}'")),
            "got {err}"
        );
    }

    #[test]
    fn notice_same_package_two_sets_renders_two_headers() {
        let scratch = scratch_dir();
        let cargo = scratch.path().join("cargo").join("shared").join("LICENSE");
        let npm = scratch.path().join("npm").join("shared").join("LICENSE");
        for (path, words) in [
            (&cargo, "Cargo words for shared.\n"),
            (&npm, "Npm words for shared.\n"),
        ] {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
            std::fs::write(path, words).expect("words");
        }
        let manifest_text = format!(
            "shared|npm|1.0.0|MIT|npm/shared/LICENSE|{}\nshared|cargo|1.0.0|MIT|cargo/shared/LICENSE|{}\n",
            notice_text_sha256("Npm words for shared.\n"),
            notice_text_sha256("Cargo words for shared.\n")
        );
        let entries = parse_notice_manifest(&manifest_text).expect("parse");
        let joined =
            join_notice_entries(&manifest_text, &[npm.clone(), cargo.clone()]).expect("join");
        let text = render_notice("//demo:root", &joined);
        assert_eq!(verify_notice_text(&text, &entries), Ok(2));
        let headers: Vec<String> = entries.iter().map(notice_entry_header).collect();
        assert_eq!(
            headers,
            vec![
                "=== shared 1.0.0 (MIT) [cargo] ===".to_owned(),
                "=== shared 1.0.0 (MIT) [npm] ===".to_owned()
            ]
        );
        assert_eq!(
            notice_entry_body(&text, &headers[0], &headers),
            Some("Cargo words for shared.\n")
        );
        assert_eq!(
            notice_entry_body(&text, &headers[1], &headers),
            Some("Npm words for shared.\n")
        );
    }

    #[test]
    fn notice_verify_rejects_drift_after_first_line() {
        let scratch = scratch_dir();
        let manifest = scratch.path().join("inventory.txt");
        let drift_words = "First line of the words.\nSecond line of the words.\n";
        std::fs::write(
            &manifest,
            format!(
                "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt|{}\n",
                notice_text_sha256(drift_words)
            ),
        )
        .expect("manifest");
        let words = scratch.path().join("demo-lib-a.txt");
        std::fs::write(&words, drift_words).expect("words");
        let texts = [words];
        let notice = scratch.path().join("out.NOTICE");
        write_notice(&manifest, &notice, "//demo:root", &texts).expect("write");
        notice_verify_files(&notice, &manifest, &texts).expect("verify");
        let text = std::fs::read_to_string(&notice).expect("read notice");
        let drifted = text.replace("Second line of the words.", "Other second line.");
        assert_ne!(drifted, text);
        std::fs::write(&notice, &drifted).expect("drifted");
        let err = notice_verify_files(&notice, &manifest, &texts).expect_err("drifted line 2");
        assert!(
            err.to_string().contains("NOTICE missing words for"),
            "got {err}"
        );
        std::fs::write(&notice, format!("{text}Extra appended line.\n")).expect("appended");
        let err = notice_verify_files(&notice, &manifest, &texts).expect_err("extra line");
        assert!(
            err.to_string().contains("NOTICE missing words for"),
            "got {err}"
        );
    }

    #[test]
    fn notice_manifest_requires_six_fields() {
        let err = parse_notice_manifest("demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n")
            .expect_err("five fields");
        assert!(err.contains("want 6"), "got {err}");
        assert!(err.contains("line 1"), "got {err}");
        assert!(err.contains("5-field manifests predate"), "got {err}");
    }

    #[test]
    fn notice_manifest_rejects_bad_digest() {
        for bad in ["", "abc", &"A".repeat(64), &"g".repeat(64)] {
            let err = parse_notice_manifest(&format!("demo-lib-a|cargo|1.0.0|MIT|a.txt|{bad}\n"))
                .expect_err("bad digest");
            assert!(
                err.contains("line 1") && err.contains("sha256"),
                "got {err}"
            );
        }
        let err =
            parse_notice_manifest(&format!("demo-lib-a|cargo|1.0.0|MIT||{}\n", "0".repeat(64)))
                .expect_err("digest without text");
        assert!(err.contains("sha256 must be empty"), "got {err}");
    }

    #[test]
    fn notice_text_sha256_matches_rendered_body() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let manifest_text = std::fs::read_to_string(&manifest).expect("read");
        let joined = join_notice_entries(&manifest_text, &texts).expect("join");
        let text = render_notice("//demo:root", &joined);
        let headers: Vec<String> = joined.iter().map(|(e, _)| notice_entry_header(e)).collect();
        for (entry, _) in &joined {
            let body =
                notice_entry_body(&text, &notice_entry_header(entry), &headers).expect("body");
            assert_eq!(dx_digest::sha256_hex(body.as_bytes()), entry.text_sha256);
        }
        assert_eq!(notice_text_sha256("x\n\n\n"), notice_text_sha256("x"));
    }

    #[test]
    fn notice_gen_rejects_text_digest_drift() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        std::fs::write(&texts[0], "Edited words.\n").expect("drift");
        let out = scratch.path().join("out.NOTICE");
        let err = write_notice(&manifest, &out, "//demo:root", &texts).expect_err("drift");
        let err = err.to_string();
        assert!(err.contains("notice-text-digest"), "got {err}");
        assert!(err.contains("demo-lib-a@1.0.0 (MIT)"), "got {err}");
        assert!(err.contains("manifest records"), "got {err}");
        assert!(!out.exists());
    }

    #[test]
    fn verify_notice_text_binds_bodies_and_rejects_extras() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let manifest_text = std::fs::read_to_string(&manifest).expect("read");
        let entries = parse_notice_manifest(&manifest_text).expect("parse");
        let joined = join_notice_entries(&manifest_text, &texts).expect("join");
        let text = render_notice("//demo:root", &joined);
        assert_eq!(verify_notice_text(&text, &entries), Ok(2));

        let headers_only = "NOTICE for //demo:root\n=== demo-lib-a 1.0.0 (MIT) [cargo] ===\n=== demo-lib-b 2.3.4 (Apache-2.0) [npm] ===\n";
        let err = verify_notice_text(headers_only, &entries).expect_err("headers only");
        assert!(err.contains("entry body is empty"), "got {err}");

        let changed = text.replace(
            "Fixture words for demo-lib-a",
            "Fixture word for demo-lib-a",
        );
        let err = verify_notice_text(&changed, &entries).expect_err("one word");
        assert!(
            err.contains("NOTICE missing words for demo-lib-a@1.0.0 (MIT)"),
            "got {err}"
        );
        assert!(err.contains("manifest records"), "got {err}");

        let extra = format!("{text}=== other 1.0.0 (MIT) [cargo] ===\nSneaky.\n");
        let err = verify_notice_text(&extra, &entries).expect_err("extra section");
        assert!(err.contains("NOTICE missing words"), "got {err}");

        let preamble = text.replacen("do not edit.", "do not edit. Extra.", 1);
        let err = verify_notice_text(&preamble, &entries).expect_err("preamble");
        assert!(err.contains("outside the audited entries"), "got {err}");

        let crlf = text.replace('\n', "\r\n");
        assert!(verify_notice_text(&crlf, &entries).is_err());

        let err = verify_notice_text("plain\n", &entries).expect_err("no header");
        assert!(err.contains("NOTICE for"), "got {err}");
        let err = verify_notice_text(&text, &[]).expect_err("no entries");
        assert!(err.contains("lists no packages"), "got {err}");
    }

    #[test]
    fn notice_entry_body_bounds_by_next_header() {
        let scratch = scratch_dir();
        let (manifest, texts) = notice_fixture(scratch.path());
        let manifest_text = std::fs::read_to_string(&manifest).expect("read");
        let joined = join_notice_entries(&manifest_text, &texts).expect("join");
        let text = render_notice("//demo:root", &joined);
        let headers: Vec<String> = joined
            .iter()
            .map(|(entry, _)| notice_entry_header(entry))
            .collect();
        for (entry, words) in &joined {
            let body = notice_entry_body(&text, &notice_entry_header(entry), &headers)
                .expect("entry body");
            assert_eq!(body, words.as_str());
        }
        assert_eq!(
            notice_entry_body(&text, "=== demo-lib-z 9.9.9 (MIT) [cargo] ===", &headers),
            None
        );
        assert_eq!(notice_entry_body(&text, "demo-lib-a 1.0.0", &headers), None);
    }

    #[test]
    fn notice_verify_rejects_unbound_bundles() {
        let scratch = scratch_dir();
        let notice = scratch.path().join("out.NOTICE");
        std::fs::write(&notice, "NOTICE for //demo:root\n").expect("notice");
        let manifest = scratch.path().join("inventory.txt");
        std::fs::write(&manifest, "# no packages\n").expect("manifest");
        let err = notice_verify_files(&notice, &manifest, &[]).expect_err("empty manifest");
        assert!(err.to_string().contains("lists no packages"), "got {err}");
        std::fs::write(&manifest, "demo-lib-a|cargo|1.0.0|MIT||\n").expect("manifest");
        let err = notice_verify_files(&notice, &manifest, &[]).expect_err("no words");
        assert!(
            err.to_string().contains("ships no LICENSE*/NOTICE* words"),
            "got {err}"
        );
        let bound_words = "Fixture words for demo-lib-a.\n";
        std::fs::write(
            &manifest,
            format!(
                "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt|{}\n",
                notice_text_sha256(bound_words)
            ),
        )
        .expect("manifest");
        let err = notice_verify_files(&notice, &manifest, &[]).expect_err("no matching input");
        assert!(
            err.to_string().contains("no matching input file"),
            "got {err}"
        );
        let words = scratch.path().join("demo-lib-a.txt");
        std::fs::write(&words, bound_words).expect("words");
        let err = notice_verify_files(&notice, &manifest, &[words.clone()]).expect_err("no header");
        assert!(
            err.to_string().contains("missing entry header"),
            "got {err}"
        );
        std::fs::write(
            &notice,
            "NOTICE for //demo:root\n=== demo-lib-a 1.0.0 (MIT) [cargo] ===\nOther words.\n",
        )
        .expect("notice header only");
        let err = notice_verify_files(&notice, &manifest, &[words]).expect_err("no words bound");
        assert!(
            err.to_string().contains("NOTICE missing words for"),
            "got {err}"
        );
    }

    #[cfg(unix)]
    fn write_fake_cosign(dir: &Path, body: &str) -> std::path::PathBuf {
        std::fs::create_dir_all(dir).expect("bin dir");
        let script = dir.join("cosign");
        std::fs::write(&script, body).expect("write cosign");
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod cosign");
        script
    }

    #[cfg(unix)]
    fn fake_cosign(version_line: &str, sign: &str, verify: &str) -> String {
        format!(
            "#!/bin/sh\ncase \"$1\" in\n  version) echo \"{version_line}\"; exit 0;;\n  sign-blob) {sign};;\n  verify-blob) {verify};;\nesac\nexit 0\n"
        )
    }

    #[cfg(unix)]
    #[test]
    fn signing_live_enforces_pinned_cosign() {
        let scratch = scratch_dir();
        let asset = write_artifact(scratch.path(), "artifact.bin", b"artifact bytes\n");
        let assets = [asset.to_string_lossy().into_owned()];
        let good = "GitVersion:    v9.8.7";
        let outdir = scratch.path().join("out");
        let scenario = |name: &str, body: &str, sha: Option<&str>| -> Result<String, String> {
            let script = write_fake_cosign(&scratch.path().join(name), body);
            let actual = dx_digest::sha256_file_hex(&script).expect("hash fake cosign");
            let path = script.to_string_lossy().into_owned();
            signing_run(
                "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml@refs/tags/v0.0.0-dryrun",
                SIGNING_ISSUER_DEFAULT,
                &assets,
                false,
                &CosignPin {
                    path: &path,
                    version: "v9.8.7",
                    sha256: sha.unwrap_or(&actual),
                },
                &outdir,
            )
        };
        let ok_body = fake_cosign(good, "exit 0", "exit 0");
        let err = scenario("wrong-sha", &ok_body, Some(&"0".repeat(64))).expect_err("wrong sha");
        assert!(err.contains("has sha256"), "got {err}");
        assert!(err.contains("refusing to sign"), "got {err}");
        let err = scenario(
            "longer-version",
            &fake_cosign("GitVersion:    v9.8.70", "exit 0", "exit 0"),
            None,
        )
        .expect_err("v9.8.70");
        assert!(err.contains("GitVersion must be v9.8.7"), "got {err}");
        assert!(err.contains("got: v9.8.70"), "got {err}");
        let err = scenario(
            "version-elsewhere",
            &fake_cosign("cosign version v9.8.7 (go1.25)", "exit 0", "exit 0"),
            None,
        )
        .expect_err("no GitVersion line");
        assert!(err.contains("got: none"), "got {err}");
        let err = scenario("sign-fails", &fake_cosign(good, "exit 1", "exit 0"), None)
            .expect_err("sign fails");
        assert!(err.contains("failed for"), "got {err}");
        assert!(err.contains("publishing nothing"), "got {err}");
        let err = scenario("verify-fails", &fake_cosign(good, "exit 0", "exit 1"), None)
            .expect_err("verify fails");
        assert!(err.contains("bundle rejected"), "got {err}");
        let bundle = outdir.join("artifact.bin.bundle");
        let ok = scenario(
            "all-ok",
            &fake_cosign(good, "touch \"$4\"; exit 0", "test -f \"$3\""),
            None,
        )
        .expect("live sign");
        assert!(bundle.is_file(), "missing {}", bundle.display());
        assert!(ok.contains("live sign with cosign v9.8.7"), "got {ok}");
        assert!(
            ok.contains(&format!("signed: {} -> {}", assets[0], bundle.display())),
            "got {ok}"
        );
        assert!(ok.contains("(verified)"), "got {ok}");
        let err = scenario(
            "gone-at-sign",
            &format!("#!/bin/sh\necho \"{good}\"\n/bin/rm -f \"$0\"\nexit 0\n"),
            None,
        )
        .expect_err("sign spawn fails");
        assert!(err.contains("cosign sign-blob"), "got {err}");
        assert!(err.starts_with("signing: cannot run '"), "got {err}");
        let err = scenario(
            "gone-at-verify",
            &fake_cosign(good, "/bin/rm -f \"$0\"; exit 0", "exit 0"),
            None,
        )
        .expect_err("verify spawn fails");
        assert!(err.contains("cannot run '"), "got {err}");
        assert!(err.contains("cosign verify-blob"), "got {err}");
        scratch.close().expect("cleanup");
    }
}
