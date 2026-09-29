#![cfg_attr(not(test), deny(clippy::expect_used, clippy::unwrap_used))]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dx_digest::blake3 as digest;
use dx_env::{acquire_lock, Error, DX_DIR_NAME};
use dx_roots::{
    build_argv_union, invocation_targets_union, repository_plan, resolve_exact_target,
    ExactScopeError, RepositoryRootPlan,
};

pub const CODEGEN_ASPECT: &str = "//generation:codegen.bzl%dx_codegen_plan_aspect";

pub const ENV_ASPECT: &str = "//env:plan.bzl%dx_env_plan_aspect";

pub const CODEGEN_OUTPUT_GROUP: &str = "dx_codegen_plans";

pub const ENV_OUTPUT_GROUP: &str = "dx_env_plans";

pub const CODEGEN_REPOSITORY_TARGET: &str = "//dx:codegen";

pub const ENV_REPOSITORY_TARGET: &str = "//dx:env";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupScope {
    Repository,
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("expected at most one target, found {count}")]
    MultipleTargets { count: usize },
    #[error("invalid target {value:?}: patterns never select setup")]
    TargetPattern { value: String },
    #[error("invalid target {value:?}: want an exact // or @ label")]
    NotTargetLabel { value: String },
}

pub fn resolve_scope(targets: &[String]) -> Result<SetupScope, ScopeError> {
    match resolve_exact_target(targets) {
        Ok(None) => Ok(SetupScope::Repository),
        Ok(Some(label)) => Ok(SetupScope::Exact(label)),
        Err(ExactScopeError::MultipleTargets { count }) => {
            Err(ScopeError::MultipleTargets { count })
        }
        Err(ExactScopeError::TargetPattern { value }) => Err(ScopeError::TargetPattern { value }),
        Err(ExactScopeError::NotTargetLabel { value }) => Err(ScopeError::NotTargetLabel { value }),
    }
}

pub fn scope_targets(scope: &SetupScope) -> Vec<String> {
    match scope {
        SetupScope::Repository => vec![
            CODEGEN_REPOSITORY_TARGET.to_owned(),
            ENV_REPOSITORY_TARGET.to_owned(),
        ],
        SetupScope::Exact(label) => vec![label.clone()],
    }
}

pub fn request_aspects() -> Vec<String> {
    vec![CODEGEN_ASPECT.to_owned(), ENV_ASPECT.to_owned()]
}

pub fn targets_for_root_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    invocation_targets_union(plan, &[CODEGEN_REPOSITORY_TARGET, ENV_REPOSITORY_TARGET])
}

pub fn plan_request_for_root_plan(plan: &RepositoryRootPlan) -> SetupRequest {
    SetupRequest {
        roots: targets_for_root_plan(plan),
        aspects: request_aspects(),
        output_groups: request_output_groups(),
    }
}

pub fn build_argv_for_plan(plan: &RepositoryRootPlan) -> Vec<String> {
    build_argv_union(
        plan,
        &[CODEGEN_REPOSITORY_TARGET, ENV_REPOSITORY_TARGET],
        &request_aspects(),
        &request_output_groups(),
    )
}

pub fn request_output_groups() -> Vec<String> {
    vec![CODEGEN_OUTPUT_GROUP.to_owned(), ENV_OUTPUT_GROUP.to_owned()]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupRequest {
    pub roots: Vec<String>,
    pub aspects: Vec<String>,
    pub output_groups: Vec<String>,
}

pub fn plan_request(scope: &SetupScope) -> SetupRequest {
    match scope {
        SetupScope::Repository => plan_request_for_root_plan(&repository_plan()),
        SetupScope::Exact(label) => SetupRequest {
            roots: vec![label.clone()],
            aspects: request_aspects(),
            output_groups: request_output_groups(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationId(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid generation id {0:?}: want 64 lowercase hex chars")]
pub struct GenerationIdError(String);

impl GenerationId {
    pub fn new(id: &str) -> Result<Self, GenerationIdError> {
        if dx_digest::is_hex(id) {
            Ok(GenerationId(id.to_owned()))
        } else {
            Err(GenerationIdError(id.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupPair {
    pub environment: GenerationId,
    pub generated: GenerationId,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("no capability: selected scope prepared neither environment nor codegen")]
    NoCapability,
}

pub struct PairInputs {
    pub prepared_environment: Option<GenerationId>,
    pub prepared_generated: Option<GenerationId>,
    pub current: Option<SetupPair>,
    pub empty_environment: GenerationId,
    pub empty_generated: GenerationId,
}

pub fn resolve_pair(inputs: PairInputs) -> Result<SetupPair, ResolveError> {
    let PairInputs {
        prepared_environment,
        prepared_generated,
        current,
        empty_environment,
        empty_generated,
    } = inputs;
    match (prepared_environment, prepared_generated) {
        (None, None) => Err(ResolveError::NoCapability),
        (environment, generated) => {
            let (current_environment, current_generated) = match current {
                Some(pair) => (Some(pair.environment), Some(pair.generated)),
                None => (None, None),
            };
            Ok(SetupPair {
                environment: environment
                    .or(current_environment)
                    .unwrap_or(empty_environment),
                generated: generated.or(current_generated).unwrap_or(empty_generated),
            })
        }
    }
}

pub const SETUPS_DIR_NAME: &str = "setups";

pub const CURRENT_LINK_NAME: &str = "current";

pub const CURRENT_STAGE_NAME: &str = "current.next";

pub const ENVIRONMENTS_DIR_NAME: &str = "environments";

pub const GENERATED_DIR_NAME: &str = "generated";

pub const ENVIRONMENT_LINK_NAME: &str = "environment";

pub const GENERATED_LINK_NAME: &str = "generated";

pub const COMMIT_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitOutcome {
    AlreadyCurrent,
    InstalledFresh,
    InstalledReplacement,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    #[error("invalid workspace root {path:?}: missing or not a directory", path = path.display())]
    WorkspaceRoot { path: PathBuf },
    #[error("workspace busy at {path:?}: another command holds the commit lock", path = path.display())]
    Busy { path: PathBuf },
    #[error("cannot lock {path:?}: {reason}", path = path.display())]
    LockFailed { path: PathBuf, reason: String },
    #[error("invalid current selection: {reason}")]
    CurrentInvalid { reason: String },
    #[error("record mismatch: {reason}")]
    RecordMismatch { reason: String },
    #[error("install failed: {reason}")]
    Install { reason: String },
    #[error("no capability: selected scope prepared neither environment nor codegen")]
    NoCapability,
}

pub struct PreparedSides {
    pub prepared_environment: Option<GenerationId>,
    pub prepared_generated: Option<GenerationId>,
    pub empty_environment: GenerationId,
    pub empty_generated: GenerationId,
}

pub fn setup_fingerprint(pair: &SetupPair) -> String {
    format!(
        "dx-setup/v0\n{}\n{}\n",
        pair.environment.as_str(),
        pair.generated.as_str()
    )
}

pub fn setup_digest(pair: &SetupPair) -> [u8; 32] {
    digest(setup_fingerprint(pair).as_bytes())
}

pub fn setup_hex(pair: &SetupPair) -> String {
    dx_digest::to_hex(&setup_digest(pair))
}

fn expected_environment_target(pair: &SetupPair) -> String {
    format!(
        "../../{}/{}/",
        ENVIRONMENTS_DIR_NAME,
        pair.environment.as_str()
    )
}

fn expected_generated_target(pair: &SetupPair) -> String {
    format!("../../{}/{}/", GENERATED_DIR_NAME, pair.generated.as_str())
}

#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(windows))]
fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

fn map_lock_error(error: Error) -> CommitError {
    match error {
        Error::Busy { path } => CommitError::Busy { path },
        Error::LockFailed { path, reason } => CommitError::LockFailed { path, reason },
        other => CommitError::LockFailed {
            path: PathBuf::from(DX_DIR_NAME),
            reason: other.to_string(),
        },
    }
}

fn acquire_commit_lock(dx_dir: &Path, timeout: Duration) -> Result<std::fs::File, CommitError> {
    acquire_lock(dx_dir, timeout).map_err(map_lock_error)
}

fn generation_from_link_target(target: &Path) -> Option<GenerationId> {
    target
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|text| GenerationId::new(text).ok())
}

pub fn read_current_pair(workspace_root: &Path) -> Result<Option<SetupPair>, CommitError> {
    if !workspace_root.is_dir() {
        return Err(CommitError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let current = workspace_root
        .join(DX_DIR_NAME)
        .join(SETUPS_DIR_NAME)
        .join(CURRENT_LINK_NAME);
    let meta = match fs::symlink_metadata(&current) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(CommitError::CurrentInvalid {
                reason: format!("cannot inspect {}: {e}", current.display()),
            });
        }
    };
    if !meta.file_type().is_symlink() {
        return Err(CommitError::CurrentInvalid {
            reason: format!(
                "{} is not a symlink; refusing to adopt foreign state",
                current.display()
            ),
        });
    }
    let pointer_target = fs::read_link(&current).map_err(|e| CommitError::CurrentInvalid {
        reason: format!("cannot read {}: {e}", current.display()),
    })?;
    let environment = fs::read_link(current.join(ENVIRONMENT_LINK_NAME))
        .ok()
        .and_then(|target| generation_from_link_target(&target));
    let generated = fs::read_link(current.join(GENERATED_LINK_NAME))
        .ok()
        .and_then(|target| generation_from_link_target(&target));
    let (environment, generated) = match (environment, generated) {
        (Some(environment), Some(generated)) => (environment, generated),
        _ => {
            return Err(CommitError::CurrentInvalid {
                reason: format!(
                    "{} does not resolve to a complete setup record",
                    current.display()
                ),
            });
        }
    };
    let pair = SetupPair {
        environment,
        generated,
    };
    let want = setup_hex(&pair);
    let pointed = pointer_target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if pointed != want {
        return Err(CommitError::CurrentInvalid {
            reason: format!(
                "{} points at {pointed:?}, want setup {want:?}; refusing digest-spoofed state",
                current.display()
            ),
        });
    }
    Ok(Some(pair))
}

fn ensure_setup_record(setups_dir: &Path, pair: &SetupPair) -> Result<PathBuf, CommitError> {
    let record = setups_dir.join(setup_hex(pair));
    if record.exists() {
        let environment = fs::read_link(record.join(ENVIRONMENT_LINK_NAME)).map_err(|e| {
            CommitError::RecordMismatch {
                reason: format!("existing record {} is malformed: {e}", record.display()),
            }
        })?;
        let generated = fs::read_link(record.join(GENERATED_LINK_NAME)).map_err(|e| {
            CommitError::RecordMismatch {
                reason: format!("existing record {} is malformed: {e}", record.display()),
            }
        })?;
        if environment.as_os_str().to_string_lossy().as_ref() != expected_environment_target(pair)
            || generated.as_os_str().to_string_lossy().as_ref() != expected_generated_target(pair)
        {
            return Err(CommitError::RecordMismatch {
                reason: format!(
                    "existing record {} does not match the committing pair; refusing to replace it",
                    record.display()
                ),
            });
        }
        return Ok(record);
    }
    fs::create_dir_all(&record).map_err(|e| CommitError::Install {
        reason: format!("cannot create {}: {e}", record.display()),
    })?;
    let created = |link: &str, target: &str| {
        symlink_dir(Path::new(target), &record.join(link)).map_err(|e| CommitError::Install {
            reason: format!("cannot stage setup link '{link}': {e}"),
        })
    };
    if let Err(error) = created(ENVIRONMENT_LINK_NAME, &expected_environment_target(pair))
        .and_then(|()| created(GENERATED_LINK_NAME, &expected_generated_target(pair)))
    {
        let _ = fs::remove_dir_all(&record);
        return Err(error);
    }
    Ok(record)
}

fn clear_staged_pointer(stage: &Path) -> Result<(), CommitError> {
    match fs::symlink_metadata(stage) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CommitError::Install {
            reason: format!("cannot inspect stale {}: {e}", stage.display()),
        }),
        Ok(meta) => {
            if meta.file_type().is_dir() && !meta.file_type().is_symlink() {
                Err(CommitError::Install {
                    reason: format!(
                        "stale {} is a directory; refusing to adopt foreign state",
                        stage.display()
                    ),
                })
            } else {
                fs::remove_file(stage).map_err(|e| CommitError::Install {
                    reason: format!("cannot clear stale {}: {e}", stage.display()),
                })
            }
        }
    }
}

fn install_and_swap(
    setups_dir: &Path,
    prior: Option<SetupPair>,
    pair: &SetupPair,
) -> Result<CommitOutcome, CommitError> {
    fs::create_dir_all(setups_dir).map_err(|e| CommitError::Install {
        reason: format!("cannot create {}: {e}", setups_dir.display()),
    })?;
    let current = setups_dir.join(CURRENT_LINK_NAME);
    let stage = setups_dir.join(CURRENT_STAGE_NAME);
    clear_staged_pointer(&stage)?;
    let record_name = setup_hex(pair);
    ensure_setup_record(setups_dir, pair)?;
    if prior.as_ref() == Some(pair) {
        return Ok(CommitOutcome::AlreadyCurrent);
    }
    let fresh = prior.is_none();
    symlink_dir(Path::new(&record_name), &stage).map_err(|e| CommitError::Install {
        reason: format!("cannot stage {}: {e}", stage.display()),
    })?;
    fs::rename(&stage, &current).map_err(|e| CommitError::Install {
        reason: format!(
            "cannot publish {}: {e}; the prior pointer is preserved",
            current.display()
        ),
    })?;
    if fresh {
        Ok(CommitOutcome::InstalledFresh)
    } else {
        Ok(CommitOutcome::InstalledReplacement)
    }
}

pub fn commit_pair(workspace_root: &Path, pair: &SetupPair) -> Result<CommitOutcome, CommitError> {
    commit_pair_with_timeout(workspace_root, pair, COMMIT_LOCK_TIMEOUT)
}

pub fn commit_pair_with_timeout(
    workspace_root: &Path,
    pair: &SetupPair,
    timeout: Duration,
) -> Result<CommitOutcome, CommitError> {
    if !workspace_root.is_dir() {
        return Err(CommitError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    fs::create_dir_all(&dx_dir).map_err(|e| CommitError::Install {
        reason: format!("cannot create {}: {e}", dx_dir.display()),
    })?;
    let _lock = acquire_commit_lock(&dx_dir, timeout)?;
    let setups_dir = dx_dir.join(SETUPS_DIR_NAME);
    let prior = read_current_pair(workspace_root)?;
    install_and_swap(&setups_dir, prior, pair)
}

pub fn commit_prepared(
    workspace_root: &Path,
    sides: PreparedSides,
) -> Result<(SetupPair, CommitOutcome), CommitError> {
    commit_prepared_with_timeout(workspace_root, sides, COMMIT_LOCK_TIMEOUT)
}

pub fn commit_prepared_with_timeout(
    workspace_root: &Path,
    sides: PreparedSides,
    timeout: Duration,
) -> Result<(SetupPair, CommitOutcome), CommitError> {
    if !workspace_root.is_dir() {
        return Err(CommitError::WorkspaceRoot {
            path: workspace_root.to_path_buf(),
        });
    }
    let dx_dir = workspace_root.join(DX_DIR_NAME);
    fs::create_dir_all(&dx_dir).map_err(|e| CommitError::Install {
        reason: format!("cannot create {}: {e}", dx_dir.display()),
    })?;
    let _lock = acquire_commit_lock(&dx_dir, timeout)?;
    let current = read_current_pair(workspace_root)?;
    let PreparedSides {
        prepared_environment,
        prepared_generated,
        empty_environment,
        empty_generated,
    } = sides;
    let pair = resolve_pair(PairInputs {
        prepared_environment,
        prepared_generated,
        current,
        empty_environment,
        empty_generated,
    })
    .map_err(|_| CommitError::NoCapability)?;
    let setups_dir = dx_dir.join(SETUPS_DIR_NAME);
    let prior = read_current_pair(workspace_root)?;
    let outcome = install_and_swap(&setups_dir, prior, &pair)?;
    Ok((pair, outcome))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod lib_tests;
