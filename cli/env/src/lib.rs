#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use marker_proto::dx::env::v1::{EnvIdentity, EnvMarker, ToolEntry};
use prost::Message;
use serde::Deserialize;

pub use dx_atomic_fs::{acquire_lock, LockError, DX_DIR_NAME, LOCK_FILE_NAME, LOCK_TIMEOUT};

pub const MARKER_FILE_NAME: &str = ".rules_dx_managed";
pub const MARKER_SCHEMA_VERSION: u32 = 1;
pub const STAGED_METADATA_SCHEMA_VERSION: u32 = 1;
pub const BIN_DIR_NAME: &str = "bin";
pub const STAGE_DIR_NAME: &str = "bin.next";
pub const PREV_DIR_NAME: &str = "bin.prev";
pub const IDENTITY_LEN: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPlan {
    pub bin_name: String,
    pub owner: String,
    pub host_names: Vec<String>,
}

#[derive(Deserialize)]
struct StagedMetadata {
    schema_version: u32,
    tools: Vec<StagedTool>,
}

#[derive(Deserialize)]
struct StagedTool {
    bin_name: String,
    owner: String,
    host_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("workspace root {path} is not a directory", path = path.display())]
    WorkspaceRoot { path: PathBuf },
    #[error("staged tree is unusable: {reason}")]
    Staged { reason: String },
    #[error(
        "staged metadata schema {found} is unsupported (installer handles {STAGED_METADATA_SCHEMA_VERSION}); regenerate the tree"
    )]
    UnsupportedStagedSchema { found: u32 },
    #[error("staged tool is invalid: {reason}")]
    InvalidTool { reason: String },
    #[error("refusing to touch unmanaged {path}: {detail}", path = path.display())]
    Unmanaged { path: PathBuf, detail: String },
    #[error(
        "installed marker schema {found} is unsupported (installer handles {MARKER_SCHEMA_VERSION}); upgrade dx instead of deleting state"
    )]
    UnsupportedMarkerSchema { found: u32 },
    #[error("installed marker is invalid: {reason}")]
    MarkerInvalid { reason: String },
    #[error(
        "another refresh holds {path}; giving up after the commit-lock deadline",
        path = path.display()
    )]
    Busy { path: PathBuf },
    #[error("cannot lock {path}: {reason}", path = path.display())]
    LockFailed { path: PathBuf, reason: String },
    #[error("symlinks are unusable on this host: {detail}")]
    SymlinkUnsupported { detail: String },
    #[error("installation failed: {reason}")]
    Install { reason: String },
}

impl From<LockError> for Error {
    fn from(error: LockError) -> Self {
        match error {
            LockError::Busy { path } => Error::Busy { path },
            LockError::LockFailed { path, reason } => Error::LockFailed { path, reason },
        }
    }
}

pub struct RefreshOptions {
    pub workspace_root: PathBuf,
    pub staged_bin: PathBuf,
    pub staged_metadata: PathBuf,
    pub os: &'static str,
    pub lock_timeout: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    AlreadyCurrent,
    InstalledFresh,
    InstalledReplacement,
}

pub fn canonical_identity_bytes(tools: &[ToolPlan]) -> Vec<u8> {
    let mut sorted: Vec<&ToolPlan> = tools.iter().collect();
    sorted.sort_by(|a, b| {
        a.bin_name
            .cmp(&b.bin_name)
            .then_with(|| a.owner.cmp(&b.owner))
    });
    let identity = EnvIdentity {
        schema_version: MARKER_SCHEMA_VERSION,
        tools: sorted
            .iter()
            .map(|tool| ToolEntry {
                bin_name: tool.bin_name.clone(),
                owner: tool.owner.clone(),
                host_names: tool.host_names.clone(),
            })
            .collect(),
    };
    identity.encode_to_vec()
}

fn identity_digest(canonical: &[u8]) -> [u8; 32] {
    dx_digest::blake3(canonical)
}

pub fn identity_hex(tools: &[ToolPlan]) -> String {
    dx_digest::to_hex(&identity_digest(&canonical_identity_bytes(tools)))
}

pub fn encode_marker(identity: &[u8; 32]) -> Vec<u8> {
    EnvMarker {
        schema_version: MARKER_SCHEMA_VERSION,
        identity: identity.to_vec(),
    }
    .encode_to_vec()
}

pub fn decode_marker(bytes: &[u8]) -> Result<[u8; 32], Error> {
    let marker = EnvMarker::decode(bytes).map_err(|e| Error::MarkerInvalid {
        reason: format!("not a valid marker: {e}"),
    })?;
    if marker.schema_version != MARKER_SCHEMA_VERSION {
        return Err(Error::UnsupportedMarkerSchema {
            found: marker.schema_version,
        });
    }
    if marker.identity.len() != IDENTITY_LEN {
        return Err(Error::MarkerInvalid {
            reason: format!(
                "identity is {} bytes, want {IDENTITY_LEN}",
                marker.identity.len()
            ),
        });
    }
    let mut identity = [0u8; IDENTITY_LEN];
    identity.copy_from_slice(&marker.identity);
    Ok(identity)
}

pub fn parse_staged(text: &str) -> Result<Vec<ToolPlan>, Error> {
    let metadata: StagedMetadata = serde_json::from_str(text).map_err(|e| Error::Staged {
        reason: format!("metadata is not valid JSON: {e}"),
    })?;
    if metadata.schema_version != STAGED_METADATA_SCHEMA_VERSION {
        return Err(Error::UnsupportedStagedSchema {
            found: metadata.schema_version,
        });
    }
    if metadata.tools.is_empty() {
        return Err(Error::InvalidTool {
            reason: "metadata carries no tools".to_string(),
        });
    }
    metadata
        .tools
        .iter()
        .map(|tool| {
            if tool.bin_name.is_empty() {
                return Err(Error::InvalidTool {
                    reason: "tool with empty bin_name".to_string(),
                });
            }
            if tool.owner.is_empty() {
                return Err(Error::InvalidTool {
                    reason: format!("tool '{}' has an empty owner", tool.bin_name),
                });
            }
            if tool.host_names.is_empty() {
                return Err(Error::InvalidTool {
                    reason: format!("tool '{}' has no host names", tool.bin_name),
                });
            }
            for name in &tool.host_names {
                validate_host_name(&tool.bin_name, name)?;
            }
            Ok(ToolPlan {
                bin_name: tool.bin_name.clone(),
                owner: tool.owner.clone(),
                host_names: tool.host_names.clone(),
            })
        })
        .collect()
}

fn validate_host_name(bin_name: &str, name: &str) -> Result<(), Error> {
    let bad = |why: &str| Error::InvalidTool {
        reason: format!("tool '{bin_name}' has invalid host name '{name}': {why}"),
    };
    if name.is_empty() {
        return Err(bad("must be a non-empty single path component"));
    }
    if name == "." || name == ".." {
        return Err(bad("must not be '.' or '..'"));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(bad("must not contain '/' or '\\'"));
    }
    let lower = name.to_lowercase();
    if lower == MARKER_FILE_NAME {
        return Err(bad("is reserved for the provenance marker"));
    }
    if let Some(suffix) = dx_path::host::executable_suffix(&lower) {
        return Err(bad(&format!(
            "must not carry an executable suffix (found '{suffix}')"
        )));
    }
    if dx_path::host::is_reserved_device(name) {
        let stem = lower.split('.').next().unwrap_or_default();
        return Err(bad(&format!("stem '{stem}' is reserved on Windows")));
    }
    Ok(())
}

pub fn probe_symlink(dir: &Path) -> io::Result<()> {
    let link = dir.join("symlink.probe");
    symlink_entry(&PathBuf::from("symlink.target"), &link)?;
    fs::remove_file(&link)
}

#[cfg(windows)]
fn symlink_entry(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(not(windows))]
fn symlink_entry(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn symlink_guidance(os: &str) -> &'static str {
    if os == "windows" {
        return "enable Developer Mode or grant SeBackupPrivilege so symlink creation works, then retry; no changes were made";
    }
    "symlink creation failed: check the filesystem and permissions, then retry; no changes were made"
}

pub fn refresh(
    options: &RefreshOptions,
    probe: &dyn Fn(&Path) -> io::Result<()>,
) -> Result<RefreshOutcome, Error> {
    let text = fs::read_to_string(&options.staged_metadata).map_err(|e| Error::Staged {
        reason: format!(
            "cannot read staged metadata {}: {e}",
            options.staged_metadata.display()
        ),
    })?;
    let tools = parse_staged(&text)?;
    let staged = read_staged_links(&options.staged_bin, &tools)?;
    if !options.workspace_root.is_dir() {
        return Err(Error::WorkspaceRoot {
            path: options.workspace_root.clone(),
        });
    }
    let dx_dir = options.workspace_root.join(DX_DIR_NAME);
    fs::create_dir_all(&dx_dir).map_err(|e| Error::Install {
        reason: format!("cannot create {}: {e}", dx_dir.display()),
    })?;
    probe(&dx_dir).map_err(|e| Error::SymlinkUnsupported {
        detail: format!("{}: {e}", symlink_guidance(options.os)),
    })?;
    let _lock = acquire_lock(&dx_dir, options.lock_timeout)?;
    let bin_dir = dx_dir.join(BIN_DIR_NAME);
    let prev_dir = dx_dir.join(PREV_DIR_NAME);
    let stage_dir = dx_dir.join(STAGE_DIR_NAME);
    recover_crashed_swap(&prev_dir, &bin_dir)?;
    if prev_dir.exists() {
        fs::remove_dir_all(&prev_dir).map_err(|e| Error::Install {
            reason: format!("cannot clear stale {}: {e}", prev_dir.display()),
        })?;
    }
    let current = read_current_identity(&bin_dir)?;
    let candidate = identity_digest(&canonical_identity_bytes(&tools));
    if current == Some(candidate) {
        clean_stage(&stage_dir);
        return Ok(RefreshOutcome::AlreadyCurrent);
    }
    let fresh = current.is_none();
    stage_tree(&stage_dir, &staged, &candidate)?;
    commit_swap(&bin_dir, &prev_dir, &stage_dir)?;
    let _ = fs::remove_dir_all(&prev_dir);
    if fresh {
        Ok(RefreshOutcome::InstalledFresh)
    } else {
        Ok(RefreshOutcome::InstalledReplacement)
    }
}

fn read_staged_links(
    staged_bin: &Path,
    tools: &[ToolPlan],
) -> Result<Vec<(String, PathBuf)>, Error> {
    let mut staged = Vec::new();
    for tool in tools {
        for name in &tool.host_names {
            let link = staged_bin.join(name);
            let target = fs::read_link(&link).map_err(|e| Error::Staged {
                reason: format!(
                    "staged link {} is unreadable or not a symlink: {e}",
                    link.display()
                ),
            })?;
            let absolute = link.parent().unwrap_or(staged_bin).join(&target);
            let canonical = fs::canonicalize(&absolute).map_err(|e| Error::Staged {
                reason: format!(
                    "staged link {} has an unresolvable target: {e}",
                    link.display()
                ),
            })?;
            staged.push((name.clone(), canonical));
        }
    }
    Ok(staged)
}

fn read_current_identity(bin_dir: &Path) -> Result<Option<[u8; 32]>, Error> {
    let meta = match fs::symlink_metadata(bin_dir) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Error::Unmanaged {
                path: bin_dir.to_path_buf(),
                detail: format!("cannot inspect installed tree: {e}"),
            });
        }
    };
    if meta.file_type().is_symlink() {
        return Err(unmanaged(
            bin_dir,
            "is a symlink; refusing to adopt foreign state",
        ));
    }
    if !meta.is_dir() {
        return Err(unmanaged(
            bin_dir,
            "is not a directory; refusing to adopt foreign state",
        ));
    }
    let marker_path = bin_dir.join(MARKER_FILE_NAME);
    if !marker_path.exists() {
        return Err(unmanaged(bin_dir, "exists without a provenance marker"));
    }
    let bytes = fs::read(&marker_path).map_err(|e| Error::Unmanaged {
        path: bin_dir.to_path_buf(),
        detail: format!("provenance marker is unreadable: {e}"),
    })?;
    match decode_marker(&bytes) {
        Ok(identity) => Ok(Some(identity)),
        Err(Error::UnsupportedMarkerSchema { found }) => {
            Err(Error::UnsupportedMarkerSchema { found })
        }
        Err(other) => Err(Error::Unmanaged {
            path: bin_dir.to_path_buf(),
            detail: format!("provenance marker is not ours: {other}"),
        }),
    }
}

fn unmanaged(bin_dir: &Path, detail: &str) -> Error {
    Error::Unmanaged {
        path: bin_dir.to_path_buf(),
        detail: format!(
            "{detail}; remove or rename it so the installer can start from a clean state"
        ),
    }
}

fn recover_crashed_swap(prev_dir: &Path, bin_dir: &Path) -> Result<(), Error> {
    if !prev_dir.exists() {
        return Ok(());
    }
    if bin_dir.exists() {
        return Ok(());
    }
    fs::rename(prev_dir, bin_dir).map_err(|e| Error::Install {
        reason: format!("cannot restore interrupted tree: {e}"),
    })?;
    Ok(())
}

fn stage_tree(
    stage_dir: &Path,
    staged: &[(String, PathBuf)],
    identity: &[u8; 32],
) -> Result<(), Error> {
    if stage_dir.exists() {
        fs::remove_dir_all(stage_dir).map_err(|e| Error::Install {
            reason: format!("cannot clear stale staging {}: {e}", stage_dir.display()),
        })?;
    }
    fs::create_dir_all(stage_dir).map_err(|e| Error::Install {
        reason: format!("cannot create staging {}: {e}", stage_dir.display()),
    })?;
    fs::write(stage_dir.join(MARKER_FILE_NAME), encode_marker(identity)).map_err(|e| {
        // LCOV_EXCL_START - reason: freshly created staging dir, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
        Error::Install {
            reason: format!("cannot stage provenance marker: {e}"),
        }
    })?;
    // LCOV_EXCL_STOP - reason: end freshly created staging dir, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
    for (name, target) in staged {
        symlink_entry(target, &stage_dir.join(name)).map_err(|e| Error::Install {
            reason: format!("cannot stage host name '{name}': {e}"),
        })?;
    }
    Ok(())
}

fn commit_swap(bin_dir: &Path, prev_dir: &Path, stage_dir: &Path) -> Result<(), Error> {
    if bin_dir.exists() {
        fs::rename(bin_dir, prev_dir).map_err(|e| Error::Install {
            reason: format!("cannot retire current tree: {e}"),
        })?;
    }
    fs::rename(stage_dir, bin_dir).map_err(|e| Error::Install {
        reason: format!("cannot publish staged tree: {e}"),
    })?;
    Ok(())
}

fn clean_stage(stage_dir: &Path) {
    if stage_dir.exists() {
        let _ = fs::remove_dir_all(stage_dir);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
