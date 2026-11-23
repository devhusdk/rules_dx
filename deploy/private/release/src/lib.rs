#![deny(clippy::indexing_slicing, clippy::string_slice)]

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD};

pub mod candidate;
pub mod cell;
mod deploy;
pub mod inventory;
pub mod ledger;
pub mod modules;
pub mod presubmit;
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
    PublishDryRunInputs, PublishDryRunOutcome, PublishError, REPORT_SCHEMA, publish_dry_run_check,
    sha256sum_line,
};

pub use deploy::{
    BcrDeployConfig, DeployError, ReleaseDeployConfig, SigningDeployConfig, bcr_deploy,
    release_deploy_tool_main, signing_deploy,
};
pub use ledger::{LedgerError, OpRecord, OpState, ledger_exit, render_ledger};
pub use tool::release_tool_run;

use dx_atomic_fs::path_io::{self, PathIoError};
use dx_deploy_tools::signing::{CosignError, check_pin, sign_one};
pub use dx_deploy_tools::signing::{
    CosignPin, cosign_git_version, sign_argv as signing_sign_argv, signing_version_ok,
    verify_argv as signing_verify_argv,
};
use dx_error::DxError;
use dx_fingerprint::FingerprintError;
use spdx::SpdxDocument;

use candidate::{CandidateError, GitError};
use cell::CellError;

#[derive(Debug)]
pub enum BcrError {
    InvalidDigest {
        digest: String,
        source: dx_digest::DigestError,
    },
    UnpublishableVersion,
    InvalidVersion {
        version: String,
    },
    InvalidModule {
        module: String,
    },
    SourceNotJson(serde_json::Error),
    IntegrityMismatch {
        digest: String,
        want: String,
        have: String,
    },
    StripPrefixMismatch {
        want: String,
        have: String,
    },
    UrlMismatch {
        want: String,
        have: String,
    },
    Render(FingerprintError),
    SubmittedByWorkflow,
    Hash {
        path: PathBuf,
        source: io::Error,
    },
    Io(PathIoError),
}

impl fmt::Display for BcrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDigest { digest, .. } => write!(
                f,
                "bcr: invalid sha256 digest {digest:?}: want 64 lowercase hex chars"
            ),
            Self::UnpublishableVersion => f.write_str(
                "bcr: version 0.0.0 is unpublishable (shape check only); candidate source metadata needs an owner-approved SemVer release version",
            ),
            Self::InvalidVersion { version } => write!(
                f,
                "bcr: invalid version '{version}': want SemVer MAJOR.MINOR.PATCH (for example '1.2.3-rc1')"
            ),
            Self::InvalidModule { module } => {
                write!(f, "bcr: invalid module '{module}': want 'rules_dx'")
            }
            Self::SourceNotJson(source) => write!(
                f,
                "bcr: candidate source.json is not JSON (interrupted publication?): {source}"
            ),
            Self::IntegrityMismatch { digest, want, have } => write!(
                f,
                "bcr: candidate source.json integrity mismatch: archive sha256 {digest} wants {want:?}, metadata carries {have:?} (refusing to replace immutable version artifacts)"
            ),
            Self::StripPrefixMismatch { want, have } => write!(
                f,
                "bcr: candidate source.json strip_prefix mismatch: want {want:?}, got {have:?}"
            ),
            Self::UrlMismatch { want, have } => write!(
                f,
                "bcr: candidate source.json url mismatch: want {want:?}, got {have:?}"
            ),
            Self::Render(source) => write!(f, "cannot render JSON: {source}"),
            Self::SubmittedByWorkflow => f.write_str(
                "bcr: this target only checks the shape (BCR_DRY_RUN=1); the bcr job in .github/workflows/release-consumer.yml submits",
            ),
            Self::Hash { path, source } => write!(f, "cannot hash {}: {source}", path.display()),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for BcrError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDigest { source, .. } => Some(source),
            Self::SourceNotJson(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            Self::Io(source) => Some(source),
            Self::UnpublishableVersion
            | Self::SubmittedByWorkflow
            | Self::InvalidVersion { .. }
            | Self::InvalidModule { .. }
            | Self::IntegrityMismatch { .. }
            | Self::StripPrefixMismatch { .. }
            | Self::UrlMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum TagError {
    NotApproved,
    PlaceholderTag,
    NotVPrefixed {
        tag: String,
    },
    NoCommit {
        tag: String,
    },
    Candidate(CandidateError),
    RemoteUnreadable {
        tag: String,
        remote: &'static str,
        source: GitError,
    },
    RemoteTag {
        remote: &'static str,
        source: CandidateError,
    },
    RemoteMismatch {
        tag: String,
        sha: String,
        commit: String,
        remote: &'static str,
    },
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotApproved => f.write_str(
                "release: real release needs RELEASE_APPROVE=1 plus explicit owner approval",
            ),
            Self::PlaceholderTag => f.write_str(
                "release: placeholder tag v0.0.0-dryrun is dry-run only; pass a real SemVer tag already pushed to the remote",
            ),
            Self::NotVPrefixed { tag } => write!(f, "release: tag {tag} must be v<SemVer>"),
            Self::NoCommit { tag } => write!(
                f,
                "release: live release of {tag} needs the candidate commit: release_driver {tag} <commit>"
            ),
            Self::Candidate(source) => write!(f, "release: {source}"),
            Self::RemoteUnreadable {
                tag,
                remote,
                source,
            } => write!(
                f,
                "release: cannot verify remote tag {tag} (git ls-remote --tags {remote} failed: {source}); push it beforehand with owner approval (this program never creates or pushes tags)"
            ),
            Self::RemoteTag { remote, source } => write!(
                f,
                "{source} (git ls-remote --tags {remote}); push it beforehand with owner approval (this program never creates or pushes tags)"
            ),
            Self::RemoteMismatch {
                tag,
                sha,
                commit,
                remote,
            } => write!(
                f,
                "release: remote tag {tag} points at {sha}, candidate is {commit} (git ls-remote --tags {remote})"
            ),
        }
    }
}

impl std::error::Error for TagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(source) | Self::RemoteTag { source, .. } => Some(source),
            Self::RemoteUnreadable { source, .. } => Some(source),
            Self::NotApproved
            | Self::PlaceholderTag
            | Self::NotVPrefixed { .. }
            | Self::NoCommit { .. }
            | Self::RemoteMismatch { .. } => None,
        }
    }
}

#[derive(Debug)]
pub enum ReleaseError {
    Candidate(CandidateError),
    Cell(CellError),
    Tag(TagError),
    Ledger(LedgerError),
    Publish(PublishError),
    MissingFlag {
        name: &'static str,
    },
    BadApprove,
    NoWorkspace {
        tool: &'static str,
    },
    Io {
        tool: &'static str,
        source: PathIoError,
    },
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Candidate(source) => source.fmt(f),
            Self::Cell(source) => write!(f, "release_cell: {source}"),
            Self::Tag(source) => source.fmt(f),
            Self::Ledger(source) => source.fmt(f),
            Self::Publish(source) => source.fmt(f),
            Self::MissingFlag { name } => {
                write!(f, "publish_dry_run_check: missing required {name} <path>")
            }
            Self::BadApprove => f.write_str(
                "publish_dry_run_check: --approve wants true/false (dry run records approval, never publishes)",
            ),
            Self::NoWorkspace { tool } => write!(
                f,
                "{tool}: BUILD_WORKSPACE_DIRECTORY must be an absolute path (run with bazel run)"
            ),
            Self::Io { tool, source } => write!(f, "{tool}: {source}"),
        }
    }
}

impl std::error::Error for ReleaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Candidate(source) => Some(source),
            Self::Cell(source) => Some(source),
            Self::Tag(source) => Some(source),
            Self::Ledger(source) => Some(source),
            Self::Publish(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::NoWorkspace { .. } | Self::MissingFlag { .. } | Self::BadApprove => None,
        }
    }
}

impl From<CandidateError> for ReleaseError {
    fn from(source: CandidateError) -> Self {
        Self::Candidate(source)
    }
}

impl From<CellError> for ReleaseError {
    fn from(source: CellError) -> Self {
        Self::Cell(source)
    }
}

impl From<TagError> for ReleaseError {
    fn from(source: TagError) -> Self {
        Self::Tag(source)
    }
}

impl From<LedgerError> for ReleaseError {
    fn from(source: LedgerError) -> Self {
        Self::Ledger(source)
    }
}

impl From<PublishError> for ReleaseError {
    fn from(source: PublishError) -> Self {
        Self::Publish(source)
    }
}

#[derive(Debug)]
pub enum SbomError {
    Render(FingerprintError),
    Hash { path: PathBuf, source: io::Error },
    Basename { path: PathBuf },
    LockKind { kind: String },
    Lock(DxError),
    Manifest(DxError),
    Runtimes(runtimes::RuntimesError),
    Builder { builder: String },
    SpdxJson(serde_json::Error),
    Spdx(spdx::SpdxError),
    ProvenanceJson(serde_json::Error),
    Provenance(provenance::ProvenanceError),
    Io(PathIoError),
}

impl fmt::Display for SbomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Render(source) => write!(f, "cannot render JSON: {source}"),
            Self::Hash { path, source } => write!(f, "cannot hash {}: {source}", path.display()),
            Self::Basename { path } => write!(f, "path {} has no UTF-8 basename", path.display()),
            Self::LockKind { kind } => {
                write!(f, "unknown lock kind '{kind}': want npm, go, maven or pip")
            }
            Self::Lock(source) | Self::Manifest(source) => write!(f, "{source}"),
            Self::Runtimes(source) => write!(f, "{source}"),
            Self::Builder { builder } => write!(
                f,
                "provenance: invalid builder '{builder}': want '{PROVENANCE_BUILDER_RELEASE}'"
            ),
            Self::SpdxJson(source) => write!(f, "sbom SPDX is not a document: {source}"),
            Self::Spdx(source) => write!(f, "{source}"),
            Self::ProvenanceJson(source) => {
                write!(f, "provenance is not an in-toto statement: {source}")
            }
            Self::Provenance(source) => write!(f, "{source}"),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for SbomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render(source) => Some(source),
            Self::Hash { source, .. } => Some(source),
            Self::Lock(source) | Self::Manifest(source) => Some(source),
            Self::Runtimes(source) => Some(source),
            Self::SpdxJson(source) | Self::ProvenanceJson(source) => Some(source),
            Self::Spdx(source) => Some(source),
            Self::Provenance(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Basename { .. } | Self::LockKind { .. } | Self::Builder { .. } => None,
        }
    }
}

impl From<PathIoError> for SbomError {
    fn from(source: PathIoError) -> Self {
        Self::Io(source)
    }
}

fn basename(path: &Path) -> Result<String, SbomError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| SbomError::Basename {
            path: path.to_path_buf(),
        })
}

fn sha256_of(path: &Path) -> Result<String, SbomError> {
    dx_digest::sha256_file_hex(path).map_err(|source| SbomError::Hash {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn pretty_json<T: serde::Serialize + ?Sized>(
    value: &T,
) -> Result<String, dx_fingerprint::FingerprintError> {
    serde_json::to_value(value)
        .map_err(dx_fingerprint::FingerprintError::from)
        .and_then(|value| dx_fingerprint::to_json_ascii_pretty(&value))
}

#[must_use]
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

pub fn render_spdx(
    base: &str,
    digest: &str,
    package: &str,
    supplier: &str,
) -> Result<String, SbomError> {
    render_spdx_with_crates(base, digest, package, supplier, &[])
}

pub fn render_spdx_with_crates(
    base: &str,
    digest: &str,
    package: &str,
    supplier: &str,
    crates: &[NoticeEntry],
) -> Result<String, SbomError> {
    render_spdx_full(base, digest, package, supplier, crates, &[])
}

pub fn render_spdx_full(
    base: &str,
    digest: &str,
    package: &str,
    supplier: &str,
    crates: &[NoticeEntry],
    runtimes: &[runtimes::RuntimeRecord],
) -> Result<String, SbomError> {
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
    pretty_json(&document).map_err(SbomError::Render)
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

pub fn render_provenance(base: &str, digest: &str, builder: &str) -> Result<String, SbomError> {
    pretty_json(&serde_json::json!({
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
    .map_err(SbomError::Render)
}

pub fn render_bcr_source(module: &str, version: &str) -> Result<String, BcrError> {
    pretty_json(&serde_json::json!({
        "integrity": "<integrity-filled-at-release>",
        "strip_prefix": bcr_strip_prefix(module, version),
        "url": bcr_source_url(module, version),
    }))
    .map_err(BcrError::Render)
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

pub fn write_spdx(src: &Path, dst: &Path, package: &str, supplier: &str) -> Result<(), SbomError> {
    write_spdx_with_inventory(src, dst, package, supplier, None)
}

pub fn write_spdx_with_inventory(
    src: &Path,
    dst: &Path,
    package: &str,
    supplier: &str,
    inventory: Option<&Path>,
) -> Result<(), SbomError> {
    write_spdx_full(src, dst, package, supplier, inventory, None)
}

pub fn write_spdx_full(
    src: &Path,
    dst: &Path,
    package: &str,
    supplier: &str,
    inventory: Option<&Path>,
    runtimes: Option<&Path>,
) -> Result<(), SbomError> {
    write_spdx_locked(src, dst, package, supplier, inventory, runtimes, &[])
}

pub fn lock_entries(kind: &str, text: &str) -> Result<Vec<NoticeEntry>, SbomError> {
    let (set, packages) = match kind {
        "npm" => (
            "npm",
            dx_audit::locks::parse_package_lock(text).map_err(SbomError::Lock)?,
        ),
        "go" => (
            "go",
            dx_audit::locks::go_locked_packages(text).map_err(SbomError::Lock)?,
        ),
        "maven" => (
            "maven",
            dx_audit::locks::parse_maven_install(text).map_err(SbomError::Lock)?,
        ),
        "pip" => (
            "pip",
            dx_audit::locks::parse_uv_lock(text).map_err(SbomError::Lock)?,
        ),
        other => {
            return Err(SbomError::LockKind {
                kind: other.to_owned(),
            });
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
    locks: &[(String, PathBuf)],
) -> Result<(), SbomError> {
    let digest = sha256_of(src)?;
    let base = basename(src)?;
    let mut crates = match inventory {
        Some(path) => {
            parse_notice_manifest(&path_io::read_to_string(path)?).map_err(SbomError::Manifest)?
        }
        None => Vec::new(),
    };
    for (kind, path) in locks {
        let text = path_io::read_to_string(path)?;
        crates.extend(lock_entries(kind, &text)?);
    }
    let records = match runtimes {
        Some(path) => runtimes::read_records(path).map_err(SbomError::Runtimes)?,
        None => Vec::new(),
    };
    let text = render_spdx_full(&base, &digest, package, supplier, &crates, &records)?;
    path_io::write_scratch(dst, text.as_bytes())?;
    Ok(())
}

pub fn write_provenance(src: &Path, dst: &Path, builder: &str) -> Result<(), SbomError> {
    provenance_builder_error(builder)?;
    let digest = sha256_of(src)?;
    let base = basename(src)?;
    let text = render_provenance(&base, &digest, builder)?;
    path_io::write_scratch(dst, text.as_bytes())?;
    Ok(())
}

pub fn write_bcr_source(dst: &Path, module: &str, version: &str) -> Result<(), BcrError> {
    let text = render_bcr_source(module, version)?;
    path_io::write_scratch(dst, text.as_bytes()).map_err(BcrError::Io)
}

pub fn bcr_integrity_of_digest(digest: &str) -> Result<String, BcrError> {
    let raw = dx_digest::parse_hex(digest).map_err(|source| BcrError::InvalidDigest {
        digest: digest.to_owned(),
        source,
    })?;
    Ok(format!("sha256-{}", STANDARD.encode(raw)))
}

fn bcr_candidate_version_error(version: &str) -> Result<(), BcrError> {
    if version == "0.0.0" {
        return Err(BcrError::UnpublishableVersion);
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.chars().next().is_none_or(|c| !c.is_ascii_digit()))
    {
        return Err(BcrError::InvalidVersion {
            version: version.to_owned(),
        });
    }
    Ok(())
}

#[must_use]
pub fn bcr_source_url(module: &str, version: &str) -> String {
    format!("{RELEASE_REPOSITORY}/releases/download/v{version}/{module}-{version}.tar.gz")
}

#[must_use]
pub fn bcr_strip_prefix(module: &str, version: &str) -> String {
    format!("{module}-{version}")
}

pub fn render_bcr_source_for_digest(
    module: &str,
    version: &str,
    digest: &str,
) -> Result<String, BcrError> {
    if module != BCR_WANT_MODULE {
        return Err(BcrError::InvalidModule {
            module: module.to_owned(),
        });
    }
    bcr_candidate_version_error(version)?;
    let integrity = bcr_integrity_of_digest(digest)?;
    pretty_json(&serde_json::json!({
        "integrity": integrity,
        "strip_prefix": bcr_strip_prefix(module, version),
        "url": bcr_source_url(module, version),
    }))
    .map_err(BcrError::Render)
}

pub fn write_bcr_source_for_archive(
    dst: &Path,
    version: &str,
    archive: &Path,
) -> Result<String, BcrError> {
    let digest = dx_digest::sha256_file_hex(archive).map_err(|source| BcrError::Hash {
        path: archive.to_path_buf(),
        source,
    })?;
    let text = render_bcr_source_for_digest(BCR_WANT_MODULE, version, &digest)?;
    path_io::write_scratch(dst, text.as_bytes()).map_err(BcrError::Io)?;
    bcr_integrity_of_digest(&digest)
}

pub fn bcr_candidate_error(
    source_text: &str,
    module: &str,
    version: &str,
    digest: &str,
) -> Result<(), BcrError> {
    let want_prefix = bcr_strip_prefix(module, version);
    bcr_source_error(
        source_text,
        &want_prefix,
        &bcr_source_url(module, version),
        digest,
    )
}

fn bcr_source_error(
    source_text: &str,
    want_prefix: &str,
    want_url: &str,
    digest: &str,
) -> Result<(), BcrError> {
    let value: serde_json::Value =
        serde_json::from_str(source_text).map_err(BcrError::SourceNotJson)?;
    let want_integrity = bcr_integrity_of_digest(digest)?;
    let integrity = value
        .get("integrity")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if integrity != want_integrity {
        return Err(BcrError::IntegrityMismatch {
            digest: digest.to_owned(),
            want: want_integrity,
            have: integrity.to_owned(),
        });
    }
    let prefix = value
        .get("strip_prefix")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if prefix != want_prefix {
        return Err(BcrError::StripPrefixMismatch {
            want: want_prefix.to_owned(),
            have: prefix.to_owned(),
        });
    }
    let url = value
        .get("url")
        .and_then(|field| field.as_str())
        .unwrap_or("");
    if url != want_url {
        return Err(BcrError::UrlMismatch {
            want: want_url.to_owned(),
            have: url.to_owned(),
        });
    }
    Ok(())
}

pub fn release_remote_error(
    candidate: &candidate::Candidate,
    ls_remote: Result<String, GitError>,
) -> Result<String, TagError> {
    let tag = candidate.tag();
    let remote = candidate.remote();
    let output = ls_remote.map_err(|source| TagError::RemoteUnreadable {
        tag: tag.clone(),
        remote,
        source,
    })?;
    let sha = candidate::remote_tag_commit(&output, &tag)
        .map_err(|source| TagError::RemoteTag { remote, source })?;
    if sha != candidate.commit() {
        return Err(TagError::RemoteMismatch {
            tag,
            sha,
            commit: candidate.commit().to_owned(),
            remote,
        });
    }
    Ok(sha)
}

#[must_use]
pub fn release_remote_record(
    candidate: &candidate::Candidate,
    ls_remote: Result<String, GitError>,
) -> OpRecord {
    let tag = candidate.tag();
    match release_remote_error(candidate, ls_remote) {
        Ok(sha) => OpRecord::new(
            "tag-remote",
            &tag,
            OpState::Validated,
            &format!("remote tag at {sha}"),
        ),
        Err(error) => OpRecord::new("tag-remote", &tag, OpState::Failed, &error.to_string()),
    }
}

pub const BCR_WANT_MODULE: &str = "rules_dx";
pub const SIGNING_TRUST_ROOT: &str = "https://tuf-repo-cdn.sigstore.dev";
pub const SIGNING_BUNDLE_MEDIA_TYPE: &str = "application/vnd.dev.sigstore.bundle.v0.3+json";
pub const SIGNING_ISSUER_DEFAULT: &str = "https://token.actions.githubusercontent.com";
pub const PROVENANCE_BUILDER_RELEASE: &str =
    concat!(release_repository!(), "/.github/workflows/release.yml");

pub fn provenance_builder_error(builder: &str) -> Result<(), SbomError> {
    if builder == PROVENANCE_BUILDER_RELEASE {
        return Ok(());
    }
    Err(SbomError::Builder {
        builder: builder.to_owned(),
    })
}

fn basename_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

#[must_use]
pub fn render_bcr_dry_run(module: &str, version: &str, inputs: &[String]) -> String {
    let mut out = String::new();
    out.push_str("bcr: dry run (BCR_DRY_RUN=1); would submit, submitting nothing:\n");
    out.push_str("  module: ");
    out.push_str(module);
    out.push_str("\n  version: ");
    out.push_str(version);
    out.push('\n');
    for input in inputs {
        out.push_str("  input: ");
        out.push_str(&basename_of(input));
        out.push_str(" (");
        out.push_str(input);
        out.push_str(")\n");
    }
    out.push_str(
        "  submit: bcr job in .github/workflows/release-consumer.yml (bazel-contrib/publish-to-bcr)\n",
    );
    out.push_str("  presubmit: bazel test @rules_dx//... (seed host) + BCR presubmit.yml\n");
    out
}

pub fn bcr_run(
    module: &str,
    version: &str,
    inputs: &[String],
    dry_run: bool,
) -> Result<String, BcrError> {
    if module != BCR_WANT_MODULE {
        return Err(BcrError::InvalidModule {
            module: module.to_owned(),
        });
    }
    if !dry_run {
        return Err(BcrError::SubmittedByWorkflow);
    }
    Ok(render_bcr_dry_run(module, version, inputs))
}

const BCR_PR_PREFIX: &str = "https://github.com/bazelbuild/bazel-central-registry/pull/";

#[must_use]
pub fn bcr_submit_record(subject: &str, result: &str, pr_url: &str) -> OpRecord {
    let (state, detail) = match (result, pr_url) {
        ("success", url) if url.starts_with(BCR_PR_PREFIX) => (OpState::Executed, url.to_owned()),
        ("success", _) => (
            OpState::Failed,
            "publish-to-bcr succeeded but reported no BCR pull request".to_owned(),
        ),
        ("skipped", _) => (
            OpState::NotAuthorized,
            "bcr job skipped: repository variable BCR_REGISTRY_FORK is unset".to_owned(),
        ),
        (other, _) => (
            OpState::Failed,
            format!("publish-to-bcr result was '{other}'"),
        ),
    };
    OpRecord::new("bcr-submit", subject, state, &detail)
}

#[must_use]
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
        "  cosign: {cosign_version} (pinned and sha256-checked per deploy/private/release/pins.json)\n"
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

#[derive(Debug)]
pub enum SigningError {
    NoAssets,
    NoIdentity,
    Pin(CosignError),
    Outdir(dx_atomic_fs::path_io::PathIoError),
    Cosign(CosignError),
    Sign(String),
}

impl std::fmt::Display for SigningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAssets => f.write_str("signing: need at least one artifact"),
            Self::NoIdentity => f.write_str(
                "signing: missing SIGNING_IDENTITY (owner-approved release workflow identity)",
            ),
            Self::Pin(source) | Self::Cosign(source) => write!(f, "{source}"),
            Self::Sign(message) => f.write_str(message),
            Self::Outdir(source) => write!(f, "signing: {source}"),
        }
    }
}

impl std::error::Error for SigningError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Outdir(source) => Some(source),
            Self::Pin(source) | Self::Cosign(source) => Some(source),
            Self::NoAssets | Self::NoIdentity | Self::Sign(_) => None,
        }
    }
}

pub fn signing_run(
    identity: &str,
    issuer: &str,
    assets: &[String],
    dry_run: bool,
    pin: &CosignPin<'_>,
    outdir: &Path,
) -> Result<String, SigningError> {
    if assets.is_empty() {
        return Err(SigningError::NoAssets);
    }
    if identity.is_empty() {
        return Err(SigningError::NoIdentity);
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

fn signing_bundle_for(asset: &str, outdir: &Path) -> String {
    outdir
        .join(format!("{}.bundle", basename_of(asset)))
        .to_string_lossy()
        .into_owned()
}

fn signing_live(
    identity: &str,
    issuer: &str,
    assets: &[String],
    pin: &CosignPin<'_>,
    outdir: &Path,
) -> Result<String, SigningError> {
    check_pin(pin).map_err(SigningError::Pin)?;
    dx_atomic_fs::path_io::create_dir_all(outdir).map_err(SigningError::Outdir)?;
    let records = signing_sign_assets(identity, issuer, assets, pin, outdir);
    if let Some(bad) = records
        .iter()
        .find(|record| matches!(record.state, OpState::Failed | OpState::Partial))
    {
        return Err(SigningError::Sign(bad.detail.clone()));
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

#[must_use]
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

#[must_use]
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
            Err(error) => {
                let state = if records.is_empty() {
                    OpState::Failed
                } else {
                    OpState::Partial
                };
                records.push(OpRecord::new(
                    "cosign-sign",
                    &subject,
                    state,
                    &error.to_string(),
                ));
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
) -> Result<(), SigningError> {
    let verify = signing_verify_argv(pin.path, asset, bundle, identity, issuer);
    sign_one(pin, asset, bundle, false, Some(verify)).map_err(SigningError::Cosign)
}

#[must_use]
pub fn render_release_dry_run(tag: &str, approve: &str) -> String {
    let mut out = String::new();
    out.push_str("release: dry run (RELEASE_DRY_RUN=1); would release, publishing nothing:\n");
    out.push_str(&format!("  tag: {tag}\n"));
    out.push_str(&format!(
        "  approve: {approve} (real release needs RELEASE_APPROVE=1 + owner approval)\n"
    ));
    out.push_str("  steps:\n");
    out.push_str(
        "    1. bazel build //deploy/private/release:dx_host_cell //cli/cli:man_pages (this host's release cell)\n",
    );
    out.push_str(
        "    2. bazel build //deploy/private/release:release_artifacts (packaged releasable unit: audit curator plus binary plus man page plus NOTICE plus SBOM/provenance)\n",
    );
    out.push_str(
        "    3. bazel build //deploy/private/release:sbom_demo //deploy/private/release:notice_demo (SPDX-2.3 plus SLSA v1 with subject digest equal to artifact sha256; aggregated NOTICE from the audited inventory, signed alongside the SBOM pair)\n",
    );
    out.push_str(
        "    4. RELEASE_SIGN_DRY_RUN=1 bazel run //deploy/private/release:signing_demo (would-sign cosign + attestation)\n",
    );
    out.push_str(
        "    5. bazel run //cli/cli:github_draft (stages a notes-only draft with --draft --verify-tag, publishes nothing)\n",
    );
    out.push_str(
        "    6. BCR_DRY_RUN=1 bazel run //deploy/private/release:bcr_demo (BCR shape check, submits nothing)\n",
    );
    out.push_str(
        "    7. bazel run //deploy/private/install:dx_verify --binary <dx> --bundle <bundle> --identity <id> --issuer <issuer> --notice <NOTICE> --notice-manifest <manifest> --notice-bundle <NOTICE.bundle> (fail-before-install)\n",
    );
    out.push_str(&format!(
        "  tag creation: git tag {tag} must already exist in the remote (pushed beforehand with owner approval); this program never creates or pushes tags\n"
    ));
    out.push_str("  publishing: nothing (dry run never tags, releases, submits, or pushes)\n");
    out.push_str("  ceiling: this program never creates or pushes tags\n");
    out
}

#[derive(Debug)]
pub struct GitState {
    pub tree_dirty: bool,
    pub head: String,
    pub tag_commit: Option<String>,
}

pub fn release_candidate(
    tag: &str,
    candidate_commit: Option<&str>,
) -> Result<candidate::Candidate, TagError> {
    let version = tag
        .strip_prefix('v')
        .ok_or_else(|| TagError::NotVPrefixed {
            tag: tag.to_owned(),
        })?;
    let commit = candidate_commit.ok_or_else(|| TagError::NoCommit {
        tag: tag.to_owned(),
    })?;
    candidate::Candidate::new(version, commit).map_err(TagError::Candidate)
}

pub fn release_live_records(
    tag: &str,
    candidate_commit: Option<&str>,
    approve: &str,
    git_state: impl FnOnce() -> Result<GitState, CandidateError>,
) -> Result<Vec<OpRecord>, TagError> {
    if approve != "1" {
        return Err(TagError::NotApproved);
    }
    if tag == "v0.0.0-dryrun" {
        return Err(TagError::PlaceholderTag);
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
        Err(error) => return failed(error.to_string()),
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
    git_state: impl FnOnce() -> Result<GitState, CandidateError>,
) -> Result<String, TagError> {
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
) -> Result<String, SbomError> {
    let digest = sha256_of(artifact)?;
    let base = basename(artifact)?;
    let spdx_text = path_io::read_to_string(spdx_path)?;
    let prov_text = path_io::read_to_string(prov)?;
    let document: SpdxDocument = serde_json::from_str(&spdx_text).map_err(SbomError::SpdxJson)?;
    spdx::document_error(&document, &base, &digest).map_err(SbomError::Spdx)?;
    let statement: provenance::Statement =
        serde_json::from_str(&prov_text).map_err(SbomError::ProvenanceJson)?;
    provenance::statement_error(&statement, &base, &digest, builder)
        .map_err(SbomError::Provenance)?;
    Ok(format!("sbom OK: SPDX-2.3 + SLSA v1 bind {digest}\n"))
}

pub use dx_audit::notice_manifest::{
    NoticeEntry, notice_text_path_error, notice_text_sha256, parse_notice_manifest,
    render_notice_manifest,
};

#[must_use]
pub fn notice_entry_header(entry: &NoticeEntry) -> String {
    format!(
        "=== {} {} ({}) [{}] ===",
        entry.package, entry.version, entry.license, entry.set
    )
}

#[must_use]
pub fn notice_header_count(text: &str, header: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_end_matches('\r') == header)
        .count()
}

#[must_use]
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
                return notice_text.get(begin..offset);
            }
            _ => {}
        }
        offset += line.len();
    }
    start.and_then(|begin| notice_text.get(begin..))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeTextError {
    NoEntries,
    NoRoot,
    NoText {
        entry: Box<NoticeEntry>,
    },
    RepeatedHeader {
        header: String,
    },
    MissingHeader {
        header: String,
    },
    EmptyBody {
        entry: Box<NoticeEntry>,
    },
    BodyDigest {
        entry: Box<NoticeEntry>,
        got: String,
    },
    Outside,
}

impl fmt::Display for NoticeTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntries => {
                f.write_str("notice manifest lists no packages; refusing empty NOTICE")
            }
            Self::NoRoot => f.write_str("NOTICE missing 'NOTICE for <root>' header"),
            Self::NoText { entry } => write!(
                f,
                "missing-notice-text: {}@{} ({}) ships no LICENSE*/NOTICE* words; record the words in the audited inventory before bundling",
                entry.package, entry.version, entry.license
            ),
            Self::RepeatedHeader { header } => write!(
                f,
                "NOTICE repeats entry header '{header}' (fail closed before install)"
            ),
            Self::MissingHeader { header } => write!(
                f,
                "NOTICE missing entry header '{header}' (fail closed before install)"
            ),
            Self::EmptyBody { entry } => write!(
                f,
                "NOTICE missing words for {}@{} ({}): entry body is empty",
                entry.package, entry.version, entry.license
            ),
            Self::BodyDigest { entry, got } => write!(
                f,
                "NOTICE missing words for {}@{} ({}): body sha256 {got}, manifest records {}",
                entry.package, entry.version, entry.license, entry.text_sha256
            ),
            Self::Outside => f.write_str("NOTICE has content outside the audited entries"),
        }
    }
}

impl std::error::Error for NoticeTextError {}

pub fn verify_notice_text(
    notice_text: &str,
    entries: &[NoticeEntry],
) -> Result<usize, NoticeTextError> {
    let notice_text = runtimes::strip_section(notice_text);
    if entries.is_empty() {
        return Err(NoticeTextError::NoEntries);
    }
    let first = notice_text.lines().next().unwrap_or("");
    let Some(root) = first.strip_prefix("NOTICE for ") else {
        return Err(NoticeTextError::NoRoot);
    };
    let root = root.trim_end_matches('\r');
    if let Some(entry) = entries.iter().find(|entry| entry.text_path.is_empty()) {
        return Err(NoticeTextError::NoText {
            entry: Box::new(entry.clone()),
        });
    }
    let headers: Vec<String> = entries.iter().map(notice_entry_header).collect();
    for header in &headers {
        if notice_header_count(notice_text, header) > 1 {
            return Err(NoticeTextError::RepeatedHeader {
                header: header.clone(),
            });
        }
        if notice_entry_body(notice_text, header, &headers).is_none() {
            return Err(NoticeTextError::MissingHeader {
                header: header.clone(),
            });
        }
    }
    let mut bodies = Vec::with_capacity(entries.len());
    for (entry, header) in entries.iter().zip(&headers) {
        let body = notice_entry_body(notice_text, header, &headers).ok_or_else(|| {
            NoticeTextError::MissingHeader {
                header: header.clone(),
            }
        })?;
        if body.trim().is_empty() {
            return Err(NoticeTextError::EmptyBody {
                entry: Box::new(entry.clone()),
            });
        }
        let got = dx_digest::sha256_hex(body.as_bytes());
        if got != entry.text_sha256 {
            return Err(NoticeTextError::BodyDigest {
                entry: Box::new(entry.clone()),
                got,
            });
        }
        bodies.push((entry.clone(), body.to_owned()));
    }
    if render_notice(root, &bodies) != notice_text {
        return Err(NoticeTextError::Outside);
    }
    Ok(entries.len())
}

#[must_use]
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

#[derive(Debug)]
pub enum NoticeError {
    TextName {
        path: PathBuf,
    },
    Read(PathIoError),
    EmptyText {
        path: PathBuf,
    },
    Manifest(DxError),
    NoEntries,
    NoMatch {
        entry: Box<NoticeEntry>,
    },
    Ambiguous {
        entry: Box<NoticeEntry>,
        paths: Vec<PathBuf>,
    },
    Digest {
        entry: Box<NoticeEntry>,
        got: String,
    },
    NoRoot,
    Text(NoticeTextError),
    Runtimes(runtimes::RuntimesError),
    Io(PathIoError),
}

impl fmt::Display for NoticeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextName { path } => write!(
                f,
                "notice: text input {} has no UTF-8 basename",
                path.display()
            ),
            Self::Read(source) => write!(f, "notice: {source}"),
            Self::EmptyText { path } => write!(
                f,
                "notice: missing-notice-text: {} is empty",
                path.display()
            ),
            Self::Manifest(source) => write!(f, "{source}"),
            Self::NoEntries => {
                f.write_str("notice: manifest lists no packages; refusing empty NOTICE")
            }
            Self::NoMatch { entry } => write!(
                f,
                "notice: missing-notice-text: {}@{} ({}) names text '{}' with no matching input file (matched by whole path-component suffix)",
                entry.package, entry.version, entry.license, entry.text_path
            ),
            Self::Ambiguous { entry, paths } => {
                let paths: Vec<String> = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                write!(
                    f,
                    "notice: ambiguous-notice-text: {}@{} ({}) [{}] text '{}' matches {} inputs: {}; lengthen the manifest path so it names exactly one input",
                    entry.package,
                    entry.version,
                    entry.license,
                    entry.set,
                    entry.text_path,
                    paths.len(),
                    paths.join(", ")
                )
            }
            Self::Digest { entry, got } => write!(
                f,
                "notice: notice-text-digest: {}@{} ({}) text '{}' sha256 {got}, manifest records {}",
                entry.package, entry.version, entry.license, entry.text_path, entry.text_sha256
            ),
            Self::NoRoot => f.write_str("notice: missing 'NOTICE for <root>' header"),
            Self::Text(source) => write!(f, "notice: {source}"),
            Self::Runtimes(source) => write!(f, "{source}"),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for NoticeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) | Self::Io(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Text(source) => Some(source),
            Self::Runtimes(source) => Some(source),
            Self::TextName { .. }
            | Self::EmptyText { .. }
            | Self::NoEntries
            | Self::NoMatch { .. }
            | Self::Ambiguous { .. }
            | Self::Digest { .. }
            | Self::NoRoot => None,
        }
    }
}

impl From<PathIoError> for NoticeError {
    fn from(source: PathIoError) -> Self {
        Self::Io(source)
    }
}

fn read_texts(text_files: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, NoticeError> {
    let mut texts: Vec<(PathBuf, String)> = Vec::with_capacity(text_files.len());
    for path in text_files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_owned();
        if name.is_empty() {
            return Err(NoticeError::TextName { path: path.clone() });
        }
        if texts.iter().any(|(seen, _)| seen == path) {
            continue;
        }
        let words = path_io::read_to_string(path).map_err(NoticeError::Read)?;
        if words.trim().is_empty() {
            return Err(NoticeError::EmptyText { path: path.clone() });
        }
        texts.push((path.clone(), words));
    }
    texts.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(texts)
}

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
    text_files: &[PathBuf],
) -> Result<Vec<(NoticeEntry, String)>, NoticeError> {
    let entries = parse_notice_manifest(manifest_text).map_err(NoticeError::Manifest)?;
    if entries.is_empty() {
        return Err(NoticeError::NoEntries);
    }
    let texts = read_texts(text_files)?;
    let mut joined = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.text_path.is_empty() {
            return Err(NoticeError::Text(NoticeTextError::NoText {
                entry: Box::new(entry),
            }));
        }
        let found = notice_text_matches(&entry.text_path, &texts);
        let words = match found.as_slice() {
            [] => {
                return Err(NoticeError::NoMatch {
                    entry: Box::new(entry),
                });
            }
            [(_, words)] => words.clone(),
            many => {
                let paths = many.iter().map(|(path, _)| path.clone()).collect();
                return Err(NoticeError::Ambiguous {
                    entry: Box::new(entry),
                    paths,
                });
            }
        };
        let got = notice_text_sha256(&words);
        if got != entry.text_sha256 {
            return Err(NoticeError::Digest {
                entry: Box::new(entry),
                got,
            });
        }
        joined.push((entry, words));
    }
    Ok(joined)
}

pub fn write_notice(
    manifest: &Path,
    dst: &Path,
    root: &str,
    text_files: &[PathBuf],
    runtimes: Option<&Path>,
) -> Result<(), NoticeError> {
    let manifest_text = path_io::read_to_string(manifest)?;
    let joined = join_notice_entries(&manifest_text, text_files)?;
    let mut notice = render_notice(root, &joined);
    if let Some(path) = runtimes {
        let records = runtimes::read_records(path).map_err(NoticeError::Runtimes)?;
        notice.push_str(&runtimes::render_section(&records));
    }
    path_io::write_scratch(dst, notice.as_bytes())?;
    Ok(())
}

pub fn notice_verify_files(
    notice: &Path,
    manifest: &Path,
    text_files: &[PathBuf],
) -> Result<String, NoticeError> {
    let notice_text = path_io::read_to_string(notice)?;
    let manifest_text = path_io::read_to_string(manifest)?;
    let entries = parse_notice_manifest(&manifest_text).map_err(NoticeError::Manifest)?;
    if entries.is_empty() {
        return Err(NoticeError::NoEntries);
    }
    if !notice_text.starts_with("NOTICE for ") {
        return Err(NoticeError::NoRoot);
    }
    join_notice_entries(&manifest_text, text_files)?;
    verify_notice_text(&notice_text, &entries).map_err(NoticeError::Text)?;
    Ok(format!("notice OK: {} entries bundled\n", entries.len()))
}

#[cfg(test)]
#[path = "release_lib_tests.rs"]
mod tests;
