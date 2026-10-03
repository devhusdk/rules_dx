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

mod spdx;

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

fn render_pretty(value: &serde_json::Value) -> String {
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
    let document = SpdxDocument {
        version: spdx::SPDX_VERSION.to_owned(),
        data_license: spdx::DATA_LICENSE.to_owned(),
        id: spdx::DOCUMENT_ID.to_owned(),
        name: format!("{package}-{base}"),
        namespace: format!("https://github.com/ralvik/rules_dx/releases/{base}-{digest}"),
        creation_info: Some(spdx::SpdxCreationInfo {
            created: "1970-01-01T00:00:00Z".to_owned(),
            creators: vec!["Tool: rules_dx-sbom-1.0".to_owned()],
        }),
        files: vec![spdx::SpdxFile {
            id: "SPDXRef-File".to_owned(),
            name: base.to_owned(),
            checksums: vec![spdx::SpdxChecksum {
                algorithm: spdx::SHA256.to_owned(),
                value: digest.to_owned(),
            }],
        }],
        packages: vec![spdx::SpdxPackage {
            id: "SPDXRef-Package".to_owned(),
            name: package.to_owned(),
            supplier: format!("Organization: {supplier}"),
            download_location: spdx::NOASSERTION.to_owned(),
            files_analyzed: Some(false),
            verification_code: Some(spdx::SpdxVerificationCode {
                value: digest.to_owned(),
            }),
            checksums: vec![spdx::SpdxChecksum {
                algorithm: spdx::SHA256.to_owned(),
                value: digest.to_owned(),
            }],
            external_refs: vec![spdx::SpdxExternalRef {
                category: "PACKAGE-MANAGER".to_owned(),
                ref_type: "purl".to_owned(),
                locator: format!("pkg:generic/{package}@{digest}"),
            }],
            ..spdx::SpdxPackage::default()
        }],
        relationships: Vec::new(),
    };
    let value = serde_json::to_value(&document).unwrap_or(serde_json::Value::Null);
    render_pretty(&value)
}

pub fn render_provenance(base: &str, digest: &str, builder: &str) -> String {
    render_pretty(&serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "predicate": {
            "buildDefinition": {
                "buildType": "https://github.com/ralvik/rules_dx/release@v1",
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
        "strip_prefix": format!("{module}-{version}"),
        "url": format!("https://github.com/ralvik/rules_dx/releases/download/v{version}/{module}-{version}.tar.gz"),
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

pub const BCR_WANT_MODULE: &str = "rules_dx";
pub const SIGNING_TRUST_ROOT: &str = "https://tuf-repo-cdn.sigstore.dev";
pub const SIGNING_COSIGN_VERSION: &str = "v2.4.1";
pub const SIGNING_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
pub const SIGNING_ISSUER_DEFAULT: &str = "https://token.actions.githubusercontent.com";
pub const PROVENANCE_BUILDER_DRY_RUN: &str =
    "https://github.com/ralvik/rules_dx/.github/workflows/publish-dry-run.yml";
pub const PROVENANCE_BUILDER_RELEASE: &str =
    "https://github.com/ralvik/rules_dx/.github/workflows/release.yml";

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
    if version == "0.0.0" {
        return Err("bcr: version 0.0.0 is unpublishable even with approval".to_owned());
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

pub fn render_signing_dry_run(identity: &str, issuer: &str, assets: &[String]) -> String {
    let mut out = String::new();
    out.push_str("signing: dry run (RELEASE_SIGN_DRY_RUN=1); would sign, publishing nothing:\n");
    out.push_str(&format!("  trust root: {SIGNING_TRUST_ROOT}\n"));
    out.push_str(&format!("  identity: {identity}\n"));
    out.push_str(&format!("  issuer: {issuer}\n"));
    out.push_str(&format!(
        "  cosign: {SIGNING_COSIGN_VERSION} (pinned per deploy/release/signing.bzl SIGNING_COSIGN_VERSION; checksum-verified fetch per .github/workflows/ghcr.yml)\n"
    ));
    out.push_str(&format!(
        "  bundle media type: {SIGNING_BUNDLE_MEDIA_TYPE} (Sigstore bundle v0.3; 0.1/0.2 only if declared)\n"
    ));
    for asset in assets {
        let base = basename_of(asset);
        out.push_str(&format!("  asset: {base} ({asset})\n"));
        out.push_str(&format!(
            "  command: cosign sign-blob --bundle {base}.bundle --certificate-identity {identity} --certificate-issuer {issuer} {asset}\n"
        ));
        out.push_str(&format!(
            "  command: gh attestation create {asset} --bundle {base}.bundle\n"
        ));
    }
    out.push_str(&format!(
        "  verify: cosign verify-blob --bundle <bundle> --certificate-identity {identity} --certificate-issuer {issuer} <binary>\n"
    ));
    out
}

pub fn signing_run(
    identity: &str,
    issuer: &str,
    assets: &[String],
    dry_run: bool,
    cosign_present: bool,
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
        return Ok(render_signing_dry_run(identity, issuer, assets));
    }
    if !cosign_present {
        return Err(
            "signing: 'cosign' CLI not found on PATH; install it to sign releases (owner-approved human-run path only)"
                .to_owned(),
        );
    }
    signing_live(identity, issuer, assets)
}

pub fn signing_sign_argv(asset: &str, bundle: &str, identity: &str, issuer: &str) -> Vec<String> {
    vec![
        "cosign".to_owned(),
        "sign-blob".to_owned(),
        "--yes".to_owned(),
        "--bundle".to_owned(),
        bundle.to_owned(),
        "--certificate-identity".to_owned(),
        identity.to_owned(),
        "--certificate-oidc-issuer".to_owned(),
        issuer.to_owned(),
        asset.to_owned(),
    ]
}

pub fn signing_verify_argv(asset: &str, bundle: &str, identity: &str, issuer: &str) -> Vec<String> {
    vec![
        "cosign".to_owned(),
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

pub fn signing_version_ok(version_output: &str) -> bool {
    version_output.contains(SIGNING_COSIGN_VERSION)
}

fn signing_bundle_for(asset: &str) -> String {
    format!("{}.bundle", basename_of(asset))
}

fn signing_live(identity: &str, issuer: &str, assets: &[String]) -> Result<String, String> {
    let version_argv = vec!["cosign".to_owned(), "version".to_owned()];
    let version_out =
        dx_process::spawn_output(&version_argv, std::path::Path::new("."), &[], false).map_err(
            |error| format!("signing: cannot run 'cosign version' (is cosign on PATH?): {error}"),
        )?;
    let version_text = String::from_utf8_lossy(&version_out.stdout).into_owned()
        + &String::from_utf8_lossy(&version_out.stderr);
    if !version_out.status.success() || !signing_version_ok(&version_text) {
        return Err(format!(
            "signing: cosign version must be {SIGNING_COSIGN_VERSION} (pinned per deploy/release/signing.bzl SIGNING_COSIGN_VERSION); got: {}",
            version_text.trim()
        ));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "signing: live sign with cosign {SIGNING_COSIGN_VERSION} on {SIGNING_TRUST_ROOT}:\n"
    ));
    for asset in assets {
        let bundle = signing_bundle_for(asset);
        let sign_argv = signing_sign_argv(asset, &bundle, identity, issuer);
        let sign_out = dx_process::spawn_output(&sign_argv, std::path::Path::new("."), &[], false)
            .map_err(|error| format!("signing: cannot run '{}': {error}", sign_argv.join(" ")))?;
        if !sign_out.status.success() {
            return Err(format!(
                "signing: '{}' failed for {asset}; publishing nothing",
                sign_argv.join(" ")
            ));
        }
        let verify_argv = signing_verify_argv(asset, &bundle, identity, issuer);
        let verify_out =
            dx_process::spawn_output(&verify_argv, std::path::Path::new("."), &[], false).map_err(
                |error| format!("signing: cannot run '{}': {error}", verify_argv.join(" ")),
            )?;
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
        "    8. bazel run //deploy/install:dx_verify --binary <dx> --bundle <bundle> --identity <id> --issuer <issuer> --notice <NOTICE> --notice-manifest <manifest> (fail-before-install)\n",
    );
    out.push_str(&format!(
        "  tag creation: git tag {tag} must already exist in the remote (pushed beforehand with owner approval); this program never creates or pushes tags\n"
    ));
    out.push_str("  publishing: nothing (dry run never tags, releases, submits, or pushes)\n");
    out.push_str("  ceiling: this program never creates or pushes tags\n");
    out
}

pub fn release_run(
    tag: &str,
    approve: &str,
    dry_run: bool,
    tree_dirty: bool,
    tag_exists: bool,
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
    if tree_dirty {
        return Err("release: tree is dirty; release requires a clean tree".to_owned());
    }
    if !tag_exists {
        return Err(format!(
            "release: local tag {tag} missing; push it beforehand with owner approval (this program never creates tags)"
        ));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "release: owner-approved human-run release for {tag}\n"
    ));
    out.push_str("release: proceeding step by step; any failure stops before publish\n");
    out.push_str(&format!(
        "release: local tag {tag} exists; remote must already carry it (pushed beforehand with owner approval)\n"
    ));
    Ok(out)
}

pub fn sbom_verify_files(artifact: &Path, spdx_path: &Path, prov: &Path) -> io::Result<String> {
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
    if !prov_text.contains("\"_type\": \"https://in-toto.io/Statement/v1\"") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provenance missing in-toto Statement v1",
        ));
    }
    if !prov_text.contains("\"predicateType\": \"https://slsa.dev/provenance/v1\"") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provenance missing SLSA v1 predicate",
        ));
    }
    if !prov_text.contains(&digest) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provenance missing subject digest {digest}"),
        ));
    }
    if !prov_text.contains(PROVENANCE_BUILDER_DRY_RUN)
        && !prov_text.contains(PROVENANCE_BUILDER_RELEASE)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provenance builder.id is not an allowlisted workflow identity (forged builders fail closed)",
        ));
    }
    Ok(format!("sbom OK: SPDX-2.3 + SLSA v1 bind {digest}\n"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoticeEntry {
    pub package: String,
    pub set: String,
    pub version: String,
    pub license: String,
    pub text_basename: String,
}

pub fn parse_notice_manifest(text: &str) -> Result<Vec<NoticeEntry>, String> {
    let mut entries = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() != 5 {
            return Err(format!(
                "notice manifest line {}: want 5 '|' fields 'package|set|version|license|text', got {}",
                index + 1,
                fields.len()
            ));
        }
        let entry = NoticeEntry {
            package: fields[0].trim().to_owned(),
            set: fields[1].trim().to_owned(),
            version: fields[2].trim().to_owned(),
            license: fields[3].trim().to_owned(),
            text_basename: fields[4].trim().to_owned(),
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
        entries.push(entry);
    }
    entries.sort_by(|a, b| {
        (&a.package, &a.set, &a.version, &a.license)
            .cmp(&(&b.package, &b.set, &b.version, &b.license))
    });
    Ok(entries)
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
        out.push_str(&format!(
            "=== {} {} ({}) ===\n",
            entry.package, entry.version, entry.license
        ));
        let trimmed = words.trim_end_matches('\n');
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

fn read_text_map(
    text_files: &[std::path::PathBuf],
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut map = std::collections::BTreeMap::new();
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
        let words = std::fs::read_to_string(path)
            .map_err(|error| format!("notice: cannot read {name}: {error}"))?;
        if words.trim().is_empty() {
            return Err(format!("notice: missing-notice-text: {name} is empty"));
        }
        map.insert(name, words);
    }
    Ok(map)
}

fn join_notice_entries(
    manifest_text: &str,
    text_files: &[std::path::PathBuf],
) -> Result<Vec<(NoticeEntry, String)>, String> {
    let entries = parse_notice_manifest(manifest_text)?;
    if entries.is_empty() {
        return Err("notice: manifest lists no packages; refusing empty NOTICE".to_owned());
    }
    let map = read_text_map(text_files)?;
    let mut joined = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.text_basename.is_empty() {
            return Err(format!(
                "notice: missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words; record the words in the audited inventory before bundling",
                entry.package, entry.version, entry.license
            ));
        }
        match map.get(&entry.text_basename) {
            Some(words) => joined.push((entry, words.clone())),
            None => {
                return Err(format!(
                    "notice: missing-notice-text: {}@{} ({}) names text '{}' with no matching input file",
                    entry.package, entry.version, entry.license, entry.text_basename
                ));
            }
        }
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
    let map = read_text_map(text_files).map_err(io::Error::other)?;
    for entry in &entries {
        if entry.text_basename.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "notice: missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words",
                    entry.package, entry.version, entry.license
                ),
            ));
        }
        let words = map.get(&entry.text_basename).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "notice: missing-notice-text: {}@{} ({}) names text '{}' with no matching input file",
                    entry.package, entry.version, entry.license, entry.text_basename
                ),
            )
        })?;
        let header = format!(
            "=== {} {} ({}) ===",
            entry.package, entry.version, entry.license
        );
        if !notice_text.contains(&header) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("notice: NOTICE missing entry header '{header}'"),
            ));
        }
        let snippet = words.trim();
        let probe = snippet.lines().next().unwrap_or("").trim();
        if !probe.is_empty() && !notice_text.contains(probe) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "notice: NOTICE missing words for {}@{} ({})",
                    entry.package, entry.version, entry.license
                ),
            ));
        }
    }
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
                "https://github.com/ralvik/rules_dx/releases/artifact.bin-{digest}"
            ))
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "  \"SPDXID\": \"SPDXRef-DOCUMENT\",");
        assert!(text.contains("\"filesAnalyzed\": false"));
        assert!(text.contains(&format!("pkg:generic/dx@{digest}")));
        assert!(text.contains("Organization: rules_dx"));
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
            "{\n  \"integrity\": \"<integrity-filled-at-release>\",\n  \"strip_prefix\": \"rules_dx-1.2.3\",\n  \"url\": \"https://github.com/ralvik/rules_dx/releases/download/v1.2.3/rules_dx-1.2.3.tar.gz\"\n}\n"
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
        assert!(bcr_run("rules_dx", "0.0.0", &[], false, true).is_err());
        assert!(bcr_run("rules_dx", "1.2.3", &[], false, false).is_err());
        let live =
            bcr_run("rules_dx", "1.2.3", &["/x/y.json".to_owned()], false, true).expect("live");
        assert!(live.contains("owner-approved submission for rules_dx@1.2.3"));
        assert!(live.contains("never pushes itself"));
    }

    #[test]
    fn signing_dry_run_matches_shell() {
        let assets = ["/tmp/a.bin".to_owned()];
        let out = render_signing_dry_run("IDENT", "ISSUER", &assets);
        assert!(out.contains(
            "signing: dry run (RELEASE_SIGN_DRY_RUN=1); would sign, publishing nothing:"
        ));
        assert!(out.contains(&format!("trust root: {SIGNING_TRUST_ROOT}")));
        assert!(out.contains("cosign sign-blob"));
        assert!(out.contains("gh attestation create"));
        assert!(out.contains("cosign verify-blob"));
        assert!(out.contains(SIGNING_COSIGN_VERSION));
        assert!(out.contains(SIGNING_BUNDLE_MEDIA_TYPE));
        assert!(signing_run("", "ISSUER", &assets, true, true).is_err());
        assert!(signing_run("ID", "ISS", &[], true, true).is_err());
        assert!(signing_run("ID", "ISS", &assets, false, false).is_err());
        let ok = signing_run("ID", "ISS", &assets, true, true).expect("dry");
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
        let ok = release_run("v0.0.0-dryrun", "0", true, false, false).expect("dry");
        assert!(ok.contains("would release, publishing nothing"));
        assert!(release_run("v1.2.3", "0", false, false, true).is_err());
        assert!(release_run("v0.0.0-dryrun", "1", false, false, true).is_err());
        assert!(release_run("v1.2.3", "1", false, true, true).is_err());
        assert!(release_run("v1.2.3", "1", false, false, false).is_err());
        let live = release_run("v1.2.3", "1", false, false, true).expect("live");
        assert!(live.contains("owner-approved human-run release for v1.2.3"));
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
        let ok = sbom_verify_files(&artifact, &spdx, &prov).expect("verify");
        assert!(ok.contains("sbom OK: SPDX-2.3 + SLSA v1 bind"));
        assert!(ok.contains(&digest));
        std::fs::write(&spdx, b"{\"spdxVersion\": \"SPDX-3.0\"}").expect("rewrite");
        assert!(sbom_verify_files(&artifact, &spdx, &prov).is_err());
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
            let result = sbom_verify_files(&artifact, &spdx_path, &others);
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
        assert!(sbom_verify_files(&artifact, &spdx, &prov).is_err());
    }

    #[test]
    fn signing_argv_pins_bundle_and_version() {
        let sign = signing_sign_argv("/tmp/a.bin", "a.bin.bundle", "IDENT", "ISSUER");
        assert_eq!(
            sign,
            vec![
                "cosign",
                "sign-blob",
                "--yes",
                "--bundle",
                "a.bin.bundle",
                "--certificate-identity",
                "IDENT",
                "--certificate-oidc-issuer",
                "ISSUER",
                "/tmp/a.bin",
            ]
        );
        let verify = signing_verify_argv("/tmp/a.bin", "a.bin.bundle", "IDENT", "ISSUER");
        assert_eq!(
            verify,
            vec![
                "cosign",
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
        assert!(signing_version_ok("cosign version v2.4.1 (go1.24)\n"));
        assert!(!signing_version_ok("cosign version v2.4.0 (go1.24)\n"));
        assert!(!signing_version_ok(""));
    }

    fn notice_fixture(dir: &Path) -> (std::path::PathBuf, Vec<std::path::PathBuf>) {
        let manifest = dir.join("inventory.txt");
        std::fs::write(
            &manifest,
            "demo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt\ndemo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n",
        )
        .expect("manifest");
        let a = dir.join("demo-lib-a.txt");
        std::fs::write(&a, "Fixture words for demo-lib-a (MIT placeholder).\n").expect("words a");
        let b = dir.join("demo-lib-b.txt");
        std::fs::write(
            &b,
            "Fixture words for demo-lib-b (Apache-2.0 placeholder).\n",
        )
        .expect("words b");
        (manifest, vec![a, b])
    }

    #[test]
    fn notice_manifest_parses_and_sorts() {
        let entries = parse_notice_manifest(
            "# invented distributed-tier root\n\ndemo-lib-b|npm|2.3.4|Apache-2.0|demo-lib-b.txt\ndemo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n",
        )
        .expect("parse");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].package, "demo-lib-a");
        assert_eq!(entries[1].package, "demo-lib-b");
        assert!(parse_notice_manifest("only|four|fields|here").is_err());
        assert!(parse_notice_manifest("|npm|1.0.0|MIT|a.txt").is_err());
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
            .find("=== demo-lib-a 1.0.0 (MIT) ===")
            .expect("a header");
        let b = text
            .find("=== demo-lib-b 2.3.4 (Apache-2.0) ===")
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
        std::fs::write(&manifest, "demo-lib-a|cargo|1.0.0|MIT|\n").expect("manifest");
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
        assert_eq!(bin_usage("sbom_spdx_gen", "<artifact> <spdx> <prov>"), 1);
        assert_eq!(
            bin_cannot_write(
                "notice_gen",
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
        let err = sbom_verify_files(&artifact, &spdx_bad, &prov_ok).expect_err("spdx digest");
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
            let err = sbom_verify_files(&artifact, &spdx_ok, &path).expect_err(want);
            assert!(err.to_string().contains(want), "want {want}, got {err}");
        }
    }

    #[test]
    fn notice_text_map_fails_actionable() {
        let scratch = scratch_dir();
        let err =
            read_text_map(&[std::path::PathBuf::from("/")]).expect_err("root has no basename");
        assert!(err.contains("no UTF-8 basename"), "got {err}");
        let empty_words = scratch.path().join("empty.txt");
        std::fs::write(&empty_words, "  \n").expect("empty words");
        let err = read_text_map(&[empty_words]).expect_err("empty words");
        assert!(err.contains("missing-notice-text"), "got {err}");
        let err = join_notice_entries("# no packages\n", &[]).expect_err("no entries");
        assert!(err.contains("lists no packages"), "got {err}");
        let other_words = scratch.path().join("demo-lib-b.txt");
        std::fs::write(&other_words, "Fixture words for demo-lib-b.\n").expect("words b");
        let err = join_notice_entries(
            "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n",
            &[other_words],
        )
        .expect_err("no matching input");
        assert!(err.contains("no matching input file"), "got {err}");
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
        std::fs::write(&manifest, "demo-lib-a|cargo|1.0.0|MIT|\n").expect("manifest");
        let err = notice_verify_files(&notice, &manifest, &[]).expect_err("no words");
        assert!(
            err.to_string().contains("ships no LICENSE*/NOTICE* words"),
            "got {err}"
        );
        std::fs::write(&manifest, "demo-lib-a|cargo|1.0.0|MIT|demo-lib-a.txt\n").expect("manifest");
        let err = notice_verify_files(&notice, &manifest, &[]).expect_err("no matching input");
        assert!(
            err.to_string().contains("no matching input file"),
            "got {err}"
        );
        let words = scratch.path().join("demo-lib-a.txt");
        std::fs::write(&words, "Fixture words for demo-lib-a.\n").expect("words");
        let err = notice_verify_files(&notice, &manifest, &[words.clone()]).expect_err("no header");
        assert!(
            err.to_string().contains("missing entry header"),
            "got {err}"
        );
        std::fs::write(
            &notice,
            "NOTICE for //demo:root\n=== demo-lib-a 1.0.0 (MIT) ===\nOther words.\n",
        )
        .expect("notice header only");
        let err = notice_verify_files(&notice, &manifest, &[words]).expect_err("no words bound");
        assert!(
            err.to_string().contains("NOTICE missing words for"),
            "got {err}"
        );
    }

    #[cfg(unix)]
    fn write_fake_cosign(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).expect("bin dir");
        let script = dir.join("cosign");
        std::fs::write(&script, body).expect("write cosign");
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("chmod cosign");
    }

    #[cfg(unix)]
    #[test]
    fn signing_live_enforces_pinned_cosign() {
        let ok = "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.1 (go1.24)\"; exit 0;;\n  sign-blob) exit 0;;\n  verify-blob) exit 0;;\nesac\nexit 0\n";
        let version_drift = "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.0 (go1.24)\"; exit 0;;\nesac\nexit 0\n";
        let sign_fails = "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.1 (go1.24)\"; exit 0;;\n  sign-blob) exit 1;;\nesac\nexit 0\n";
        let verify_fails = "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.1 (go1.24)\"; exit 0;;\n  verify-blob) exit 1;;\nesac\nexit 0\n";
        let gone_at_sign =
            "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.1 (go1.24)\"; /bin/rm -f \"$PATH/cosign\"; exit 0;;\nesac\nexit 0\n";
        let gone_at_verify = "#!/bin/sh\ncase \"$1\" in\n  version) echo \"cosign version v2.4.1 (go1.24)\"; exit 0;;\n  sign-blob) /bin/rm -f \"$PATH/cosign\"; exit 0;;\nesac\nexit 0\n";
        let scratch = scratch_dir();
        let asset = write_artifact(scratch.path(), "artifact.bin", b"artifact bytes\n");
        let assets = [asset.to_string_lossy().into_owned()];
        let original_path = std::env::var_os("PATH");
        let scenario = |name: &str, script: Option<&str>| -> Result<String, String> {
            let dir = scratch.path().join(name);
            std::fs::create_dir_all(&dir).expect("scenario dir");
            if let Some(body) = script {
                write_fake_cosign(&dir, body);
            }
            std::env::set_var("PATH", &dir);
            signing_run(
                "https://github.com/ralvik/rules_dx/.github/workflows/release.yml@refs/tags/v0.0.0-dryrun",
                SIGNING_ISSUER_DEFAULT,
                &assets,
                false,
                true,
            )
        };
        let err = scenario("no-cosign", None).expect_err("cosign absent");
        assert!(err.contains("cannot run 'cosign version'"), "got {err}");
        let err = scenario("version-drift", Some(version_drift)).expect_err("version drift");
        assert!(err.contains("cosign version must be v2.4.1"), "got {err}");
        let err = scenario("sign-fails", Some(sign_fails)).expect_err("sign fails");
        assert!(err.contains("failed for"), "got {err}");
        assert!(err.contains("publishing nothing"), "got {err}");
        let err = scenario("verify-fails", Some(verify_fails)).expect_err("verify fails");
        assert!(err.contains("bundle rejected"), "got {err}");
        let ok = scenario("all-ok", Some(ok)).expect("live sign");
        assert!(ok.contains("signed: "), "got {ok}");
        assert!(ok.contains("(verified)"), "got {ok}");
        let err = scenario("gone-at-sign", Some(gone_at_sign)).expect_err("sign spawn fails");
        assert!(err.contains("cannot run 'cosign sign-blob"), "got {err}");
        let err = scenario("gone-at-verify", Some(gone_at_verify)).expect_err("verify spawn fails");
        assert!(err.contains("cannot run 'cosign verify-blob"), "got {err}");
        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        }
    }
}
