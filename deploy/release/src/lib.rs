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
mod deploy;
pub mod inventory;
pub mod ledger;
pub mod modules;
mod provenance;
mod publish;
pub mod runtimes;
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
    PublishDryRunInputs, PublishDryRunOutcome, REPORT_SCHEMA, publish_dry_run_check, sha256sum_line,
};

pub use deploy::{
    BcrDeployConfig, ReleaseDeployConfig, SigningDeployConfig, bcr_deploy,
    release_deploy_tool_main, signing_deploy,
};
pub use ledger::{OpRecord, OpState, ledger_exit, render_ledger};
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
    render_spdx_with_crates(base, digest, package, supplier, &[])
}

pub fn render_spdx_with_crates(
    base: &str,
    digest: &str,
    package: &str,
    supplier: &str,
    crates: &[NoticeEntry],
) -> String {
    render_spdx_full(base, digest, package, supplier, crates, &[])
}

pub fn render_spdx_full(
    base: &str,
    digest: &str,
    package: &str,
    supplier: &str,
    crates: &[NoticeEntry],
    runtimes: &[runtimes::RuntimeRecord],
) -> String {
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
    let mut sorted: Vec<&NoticeEntry> = crates.iter().collect();
    sorted.sort_by(|a, b| (&a.package, &a.version).cmp(&(&b.package, &b.version)));
    for entry in sorted {
        let id = crate_spdx_id(&entry.package, &entry.version);
        let ecosystem = match entry.set.as_str() {
            "npm" => "npm",
            "go" => "golang",
            "maven" => "maven",
            "pip" => "pypi",
            _ => "cargo",
        };
        document
            .relationships
            .push(spdx::contains("SPDXRef-Package", &id));
        document.packages.push(spdx::SpdxPackage {
            id,
            name: entry.package.clone(),
            version: entry.version.clone(),
            license_declared: entry.license.clone(),
            download_location: spdx::NOASSERTION.to_owned(),
            external_refs: vec![spdx::SpdxExternalRef {
                category: "PACKAGE-MANAGER".to_owned(),
                ref_type: "purl".to_owned(),
                locator: format!("pkg:{ecosystem}/{}@{}", entry.package, entry.version),
            }],
            ..spdx::SpdxPackage::default()
        });
    }
    add_runtimes(&mut document, runtimes);
    let value = serde_json::to_value(&document).unwrap_or(serde_json::Value::Null);
    render_pretty(&value)
}

fn add_runtimes(document: &mut spdx::Document, runtimes: &[runtimes::RuntimeRecord]) {
    let mut sorted: Vec<&runtimes::RuntimeRecord> = runtimes.iter().collect();
    sorted.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    for record in sorted {
        let id = spdx_id("Runtime", &record.name, &record.version);
        document
            .relationships
            .push(spdx::contains("SPDXRef-Package", &id));
        let os_supplied = record.redistribution == "os_supplied";
        document.packages.push(spdx::SpdxPackage {
            id: id.clone(),
            name: record.name.clone(),
            version: record.version.clone(),
            supplier: format!("Organization: {}", record.source),
            comment: if os_supplied {
                "Supplied by the operating system; no files are shipped.".to_owned()
            } else {
                String::new()
            },
            download_location: spdx::NOASSERTION.to_owned(),
            files_analyzed: Some(false),
            ..spdx::SpdxPackage::default()
        });
        for file in &record.files {
            let file_id = spdx_id(
                "RuntimeFile",
                &format!("{}-{}", record.name, record.version),
                &file.name,
            );
            document.relationships.push(spdx::contains(&id, &file_id));
            document.files.push(spdx::SpdxFile {
                id: file_id,
                name: file.name.clone(),
                checksums: vec![spdx::SpdxChecksum {
                    algorithm: spdx::SHA256.to_owned(),
                    value: file.sha256.clone(),
                }],
            });
        }
    }
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
    let (strip_prefix, url) = match module.strip_prefix("rules_dx_") {
        Some(family) => (
            bcr_module_strip_prefix(
                &modules::ReleaseModule {
                    name: module.to_owned(),
                    root: format!("families/{family}"),
                },
                version,
            ),
            bcr_source_url(BCR_WANT_MODULE, version),
        ),
        None => (
            bcr_strip_prefix(module, version),
            bcr_source_url(module, version),
        ),
    };
    render_pretty(&serde_json::json!({
        "integrity": "<integrity-filled-at-release>",
        "strip_prefix": strip_prefix,
        "url": url,
    }))
}

fn crate_spdx_id(name: &str, version: &str) -> String {
    spdx_id("Crate", name, version)
}

fn spdx_id(kind: &str, name: &str, version: &str) -> String {
    let clean: String = format!("{name}-{version}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("SPDXRef-{kind}-{clean}")
}

pub fn write_spdx(src: &Path, dst: &Path, package: &str, supplier: &str) -> io::Result<()> {
    write_spdx_with_inventory(src, dst, package, supplier, None)
}

pub fn write_spdx_with_inventory(
    src: &Path,
    dst: &Path,
    package: &str,
    supplier: &str,
    inventory: Option<&Path>,
) -> io::Result<()> {
    write_spdx_full(src, dst, package, supplier, inventory, None)
}

pub fn write_spdx_full(
    src: &Path,
    dst: &Path,
    package: &str,
    supplier: &str,
    inventory: Option<&Path>,
    runtimes: Option<&Path>,
) -> io::Result<()> {
    write_spdx_locked(src, dst, package, supplier, inventory, runtimes, &[])
}

/// Reads the packages a lock file pins as SBOM entries; `kind` is npm, go, maven or pip.
pub fn lock_entries(kind: &str, text: &str) -> Result<Vec<NoticeEntry>, String> {
    let (set, packages) = match kind {
        "npm" => ("npm", dx_audit::locks::parse_package_lock(text)?),
        "go" => ("go", dx_audit::locks::go_locked_packages(text)?),
        "maven" => ("maven", dx_audit::locks::parse_maven_install(text)?),
        "pip" => ("pip", dx_audit::locks::parse_uv_lock(text)?),
        other => {
            return Err(format!(
                "unknown lock kind '{other}': want npm, go, maven or pip"
            ));
        }
    };
    Ok(packages
        .into_iter()
        .filter(|package| !package.name.is_empty() && !package.version.is_empty())
        .map(|package| NoticeEntry {
            package: package.name,
            set: set.to_owned(),
            version: package.version,
            license: "NOASSERTION".to_owned(),
            text_path: String::new(),
            text_sha256: String::new(),
        })
        .collect())
}

pub fn write_spdx_locked(
    src: &Path,
    dst: &Path,
    package: &str,
    supplier: &str,
    inventory: Option<&Path>,
    runtimes: Option<&Path>,
    locks: &[(String, std::path::PathBuf)],
) -> io::Result<()> {
    let digest = dx_digest::sha256_file_hex(src)?;
    let base = basename(src)?;
    let mut crates = match inventory {
        Some(path) => {
            parse_notice_manifest(&std::fs::read_to_string(path)?).map_err(io::Error::other)?
        }
        None => Vec::new(),
    };
    for (kind, path) in locks {
        let text = std::fs::read_to_string(path)?;
        crates.extend(lock_entries(kind, &text).map_err(io::Error::other)?);
    }
    let records = match runtimes {
        Some(path) => runtimes::read_records(path)?,
        None => Vec::new(),
    };
    std::fs::write(
        dst,
        render_spdx_full(&base, &digest, package, supplier, &crates, &records).as_bytes(),
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

pub fn bcr_module_strip_prefix(module: &modules::ReleaseModule, version: &str) -> String {
    let prefix = bcr_strip_prefix(BCR_WANT_MODULE, version);
    if module.is_core() {
        prefix
    } else {
        format!("{prefix}/{}", module.root)
    }
}

pub fn render_bcr_source_for_digest(
    module: &str,
    version: &str,
    digest: &str,
) -> Result<String, String> {
    if module != BCR_WANT_MODULE {
        return Err(format!("bcr: invalid module '{module}': want 'rules_dx'"));
    }
    render_bcr_module_source_for_digest(&modules::ReleaseModule::core(), version, digest)
}

/// Renders the `source.json` of one module; every module points at the core archive.
pub fn render_bcr_module_source_for_digest(
    module: &modules::ReleaseModule,
    version: &str,
    digest: &str,
) -> Result<String, String> {
    bcr_candidate_version_error(version)?;
    let integrity = bcr_integrity_of_digest(digest)?;
    Ok(render_pretty(&serde_json::json!({
        "integrity": integrity,
        "strip_prefix": bcr_module_strip_prefix(module, version),
        "url": bcr_source_url(BCR_WANT_MODULE, version),
    })))
}

pub fn write_bcr_source_for_archive(
    dst: &Path,
    module: &modules::ReleaseModule,
    version: &str,
    archive: &Path,
) -> io::Result<String> {
    let digest = dx_digest::sha256_file_hex(archive)?;
    let text =
        render_bcr_module_source_for_digest(module, version, &digest).map_err(io::Error::other)?;
    std::fs::write(dst, text.as_bytes())?;
    bcr_integrity_of_digest(&digest).map_err(io::Error::other)
}

pub fn bcr_candidate_error(
    source_text: &str,
    module: &str,
    version: &str,
    digest: &str,
) -> Result<(), String> {
    let want_prefix = bcr_strip_prefix(module, version);
    bcr_source_error(
        source_text,
        &want_prefix,
        &bcr_source_url(module, version),
        digest,
    )
}

pub fn bcr_module_candidate_error(
    source_text: &str,
    module: &modules::ReleaseModule,
    version: &str,
    digest: &str,
) -> Result<(), String> {
    let want_prefix = bcr_module_strip_prefix(module, version);
    bcr_source_error(
        source_text,
        &want_prefix,
        &bcr_source_url(BCR_WANT_MODULE, version),
        digest,
    )
}

fn bcr_source_error(
    source_text: &str,
    want_prefix: &str,
    want_url: &str,
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
    let prefix = value
        .get("strip_prefix")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if prefix != want_prefix {
        return Err(format!(
            "bcr: candidate source.json strip_prefix mismatch: want {want_prefix:?}, got {prefix:?}"
        ));
    }
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

pub fn release_remote_record(
    candidate: &candidate::Candidate,
    ls_remote: Result<String, String>,
) -> OpRecord {
    let tag = candidate.tag();
    match release_remote_error(candidate, ls_remote) {
        Ok(sha) => OpRecord::new(
            "tag-remote",
            &tag,
            OpState::Validated,
            &format!("remote tag at {sha}"),
        ),
        Err(error) => OpRecord::new("tag-remote", &tag, OpState::Failed, &error),
    }
}

pub const BCR_WANT_MODULE: &str = "rules_dx";
pub const SIGNING_TRUST_ROOT: &str = "https://tuf-repo-cdn.sigstore.dev";
pub const SIGNING_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
pub const SIGNING_ISSUER_DEFAULT: &str = "https://token.actions.githubusercontent.com";
pub const PROVENANCE_BUILDER_RELEASE: &str =
    concat!(release_repository!(), "/.github/workflows/release.yml");

pub fn provenance_builder_error(builder: &str) -> Result<(), String> {
    if builder == PROVENANCE_BUILDER_RELEASE {
        return Ok(());
    }
    Err(format!(
        "provenance: invalid builder '{builder}': want '{PROVENANCE_BUILDER_RELEASE}'"
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

/// Accepts `rules_dx` and `rules_dx_<family>`, the names `bcr_source_error` takes.
fn bcr_module_is_valid(module: &str) -> bool {
    module == BCR_WANT_MODULE
        || module
            .strip_prefix("rules_dx_")
            .is_some_and(|family| !family.is_empty())
}

pub fn bcr_run(
    module: &str,
    version: &str,
    inputs: &[String],
    dry_run: bool,
    approved: bool,
) -> Result<String, String> {
    if !bcr_module_is_valid(module) {
        return Err(format!(
            "bcr: invalid module '{module}': want 'rules_dx' or 'rules_dx_<family>'"
        ));
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
    let detail = format!(
        "owner-approved; open the BCR PR manually with the inputs [{}] (this program never pushes itself)",
        inputs
            .iter()
            .map(|input| basename_of(input))
            .collect::<Vec<_>>()
            .join(", ")
    );
    Ok(render_ledger(&[OpRecord::new(
        "bcr-submit",
        &format!("{module}@{version}"),
        OpState::Validated,
        &detail,
    )]))
}

const BCR_PR_PREFIX: &str = "https://github.com/bazelbuild/bazel-central-registry/pull/";

/// Ledger record for the publish-to-bcr job: executed only for a successful
/// run that reported a pull request on the central registry.
pub fn bcr_submit_record(subject: &str, result: &str, pr_url: &str) -> OpRecord {
    let (state, detail) = match (result, pr_url) {
        ("success", url) if url.starts_with(BCR_PR_PREFIX) => (OpState::Executed, url.to_owned()),
        ("success", _) => (
            OpState::Failed,
            "publish-to-bcr succeeded but reported no BCR pull request".to_owned(),
        ),
        (other, _) => (
            OpState::Failed,
            format!("publish-to-bcr result was '{other}'"),
        ),
    };
    OpRecord::new("bcr-submit", subject, state, &detail)
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
        "  bundle media type: {SIGNING_BUNDLE_MEDIA_TYPE} (Sigstore bundle v0.3)\n"
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
        "--new-bundle-format".to_owned(),
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

const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Runs `argv` with the inherited environment and working directory.
fn run_output(argv: &[String]) -> Result<dx_process::capture::Captured, String> {
    let (binary, args) = argv
        .split_first()
        .ok_or_else(|| "invocation needs a binary".to_owned())?;
    let mut command = std::process::Command::new(binary);
    command.args(args);
    dx_process::capture::run_captured(&mut command, SIGN_TIMEOUT).map_err(|error| error.to_string())
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
    let version_text = version_out.stdout_text() + &String::from_utf8_lossy(&version_out.stderr);
    if !version_out.success() || !signing_version_ok(&version_text, pin.version) {
        return Err(format!(
            "signing: cosign GitVersion must be {} (pinned per deploy/release/cosign_pins.bzl); got: {}",
            pin.version,
            cosign_git_version(&version_text).unwrap_or("none")
        ));
    }
    std::fs::create_dir_all(outdir)
        .map_err(|error| format!("signing: cannot create {}: {error}", outdir.display()))?;
    let records = signing_sign_assets(identity, issuer, assets, pin, outdir);
    if let Some(bad) = records
        .iter()
        .find(|record| matches!(record.state, OpState::Failed | OpState::Partial))
    {
        return Err(bad.detail.clone());
    }
    let mut out = String::new();
    out.push_str(&format!(
        "signing: live sign with cosign {} on {SIGNING_TRUST_ROOT}:\n",
        pin.version
    ));
    for record in &records {
        out.push_str(&format!("  signed: {} (verified)\n", record.detail));
    }
    Ok(out)
}

/// Ledger records for a dry run: nothing is signed, so nothing is authorized.
pub fn signing_dry_run_records(assets: &[String]) -> Vec<OpRecord> {
    assets
        .iter()
        .map(|asset| {
            OpRecord::new(
                "cosign-sign",
                &basename_of(asset),
                OpState::NotAuthorized,
                "dry run; nothing signed",
            )
        })
        .collect()
}

/// Signs and verifies each asset in order, stopping at the first failure.
/// A failure after an earlier asset was signed is `Partial`, otherwise `Failed`.
pub fn signing_sign_assets(
    identity: &str,
    issuer: &str,
    assets: &[String],
    pin: &CosignPin<'_>,
    outdir: &Path,
) -> Vec<OpRecord> {
    let mut records = Vec::new();
    for asset in assets {
        let subject = basename_of(asset);
        let bundle = signing_bundle_for(asset, outdir);
        match signing_sign_one(identity, issuer, asset, &bundle, pin) {
            Ok(()) => records.push(OpRecord::new(
                "cosign-sign",
                &subject,
                OpState::Executed,
                &format!("{asset} -> {bundle}"),
            )),
            Err(detail) => {
                let state = if records.is_empty() {
                    OpState::Failed
                } else {
                    OpState::Partial
                };
                records.push(OpRecord::new("cosign-sign", &subject, state, &detail));
                break;
            }
        }
    }
    records
}

fn signing_sign_one(
    identity: &str,
    issuer: &str,
    asset: &str,
    bundle: &str,
    pin: &CosignPin<'_>,
) -> Result<(), String> {
    let sign_argv = signing_sign_argv(pin.path, asset, bundle);
    let sign_out = run_output(&sign_argv)
        .map_err(|error| format!("signing: cannot run '{}': {error}", sign_argv.join(" ")))?;
    if !sign_out.success() {
        return Err(format!(
            "signing: '{}' failed for {asset}; publishing nothing",
            sign_argv.join(" ")
        ));
    }
    let verify_argv = signing_verify_argv(pin.path, asset, bundle, identity, issuer);
    let verify_out = run_output(&verify_argv)
        .map_err(|error| format!("signing: cannot run '{}': {error}", verify_argv.join(" ")))?;
    if !verify_out.success() {
        return Err(format!(
            "signing: '{}' failed for {asset}; bundle rejected, publishing nothing",
            verify_argv.join(" ")
        ));
    }
    Ok(())
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
        "    1. bazel build //deploy/release:dx_host_cell //cli/cli:man_pages (this host's release cell)\n",
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
        "    5. bazel run //cli/cli:github_draft (stages a notes-only draft with --draft --verify-tag, publishes nothing)\n",
    );
    out.push_str(
        "    6. BCR_DRY_RUN=1 bazel run //deploy/release:bcr_demo (would-submit BCR PR, submits nothing)\n",
    );
    out.push_str(
        "    7. bazel run //deploy/install:dx_verify --binary <dx> --bundle <bundle> --identity <id> --issuer <issuer> --notice <NOTICE> --notice-manifest <manifest> --notice-bundle <NOTICE.bundle> (fail-before-install)\n",
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

pub fn release_live_records(
    tag: &str,
    candidate_commit: Option<&str>,
    approve: &str,
    git_state: impl FnOnce() -> Result<GitState, String>,
) -> Result<Vec<OpRecord>, String> {
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
    let failed = |detail: String| {
        Ok(vec![OpRecord::new(
            "tag-local",
            tag,
            OpState::Failed,
            &detail,
        )])
    };
    let state = match git_state() {
        Ok(state) => state,
        Err(error) => return failed(error),
    };
    if state.tree_dirty {
        return failed("release: tree is dirty; release requires a clean tree".to_owned());
    }
    if state.head != commit {
        return failed(format!(
            "release: HEAD is {}, candidate is {commit}; check out the candidate commit",
            state.head
        ));
    }
    let Some(tag_commit) = state.tag_commit else {
        return failed(format!(
            "release: local tag {tag} missing; push it beforehand with owner approval (this program never creates tags)"
        ));
    };
    if tag_commit != commit {
        return failed(format!(
            "release: local tag {tag} points at {tag_commit}, candidate is {commit}"
        ));
    }
    Ok(vec![OpRecord::new(
        "tag-local",
        tag,
        OpState::Validated,
        &format!("clean tree, HEAD and local tag at {commit}"),
    )])
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
    let records = release_live_records(tag, candidate_commit, approve, git_state)?;
    Ok(render_ledger(&records))
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

pub use dx_audit::notice_manifest::{
    NoticeEntry, notice_text_path_error, notice_text_sha256, parse_notice_manifest,
    render_notice_manifest,
};

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

/// Shared NOTICE verifier for the release and install sides. Errors carry no
/// prefix; callers add their own.
pub fn verify_notice_text(notice_text: &str, entries: &[NoticeEntry]) -> Result<usize, String> {
    let notice_text = runtimes::strip_section(notice_text);
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
    runtimes: Option<&Path>,
) -> io::Result<()> {
    let manifest_text = std::fs::read_to_string(manifest)?;
    let joined = join_notice_entries(&manifest_text, text_files).map_err(io::Error::other)?;
    let mut notice = render_notice(root, &joined);
    if let Some(path) = runtimes {
        notice.push_str(&runtimes::render_section(&runtimes::read_records(path)?));
    }
    std::fs::write(dst, notice.as_bytes())?;
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
#[path = "release_lib_tests.rs"]
mod tests;
