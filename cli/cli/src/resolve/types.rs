use std::io;
use std::path::Path;

use dx_process::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait QueryRunner {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult>;
}

// LCOV_EXCL_START - reason: prod spawn, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
pub struct ProcessQueryRunner;

impl QueryRunner for ProcessQueryRunner {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        let output = dx_process::spawn_output(argv, cwd, &[], false).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidInput {
                io::Error::new(io::ErrorKind::InvalidInput, "query needs a binary")
            } else {
                error
            }
        })?;
        Ok(QueryResult {
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}
// LCOV_EXCL_STOP - reason: end prod spawn, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

// LCOV_EXCL_START - reason: test guard, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
#[cfg(test)]
pub struct NeverQuery;

#[cfg(test)]
impl QueryRunner for NeverQuery {
    fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        panic!("resolve tests must not run queries");
    }
}
// LCOV_EXCL_STOP - reason: end test guard, issue: 1055, policy: docs/cli/commands/build-test-coverage.md

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedScope {
    pub scope: Scope,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("empty scope: pass a label, pattern, file, or directory")]
    EmptyScope,
    #[error(
        "unsupported scope {scope:?}: package-relative labels resolve against the current directory; spell the workspace label starting with //"
    )]
    RelativeLabel { scope: String },
    #[error(
        "unsupported scope {scope:?}: workflow commands accept main-workspace labels, patterns, files, and directories only"
    )]
    ExternalScope { scope: String },
    #[error("unsupported scope {scope:?}: pass a workspace-relative path without .. escapes")]
    OutsideWorkspace { scope: String },
    #[error("unknown path {scope:?}: no such file or directory under the workspace")]
    PathNotFound { scope: String },
    #[error("unsupported path {scope:?}: scope paths must be regular files or directories")]
    NotFileOrDir { scope: String },
    #[error(
        "not a package {scope:?}: no enclosing Bazel package holds the file; add a BUILD file for its directory or pass an explicit target label"
    )]
    NotAPackage { scope: String },
    #[error(
        "unsupported path {scope:?}: filenames with control characters cannot resolve through Bazel query"
    )]
    UnsupportedName { scope: String },
    #[error(
        "no Bazel target owns {file:?} (queried as {label}): add the file to a target srcs list or pass an explicit target label"
    )]
    NoOwner { file: String, label: String },
    #[error(
        "no test depends on {owners}: pass an explicit test label or pattern such as //pkg/...",
        owners = owners.join(" ")
    )]
    NoTests { owners: Vec<String> },
    #[error(
        "no executable target owns {scopes}: add a *_binary rule owning the file or pass an explicit runnable label",
        scopes = scopes.join(" ")
    )]
    NoRunnable { scopes: Vec<String> },
    #[error(
        "multiple executable targets own the scope ({candidates}): pass one explicit runnable label",
        candidates = candidates.join(" ")
    )]
    AmbiguousRunnable { candidates: Vec<String> },
    #[error(
        "dx deploy needs exactly one label, got {count}: pass a deploy label such as //deploy:production"
    )]
    DeployCount { count: usize },
    #[error(
        "unsupported deploy scope {scope:?}: pass exactly one deploy label such as //deploy:production (patterns like //..., files, and directories are not deployable)"
    )]
    DeployScope { scope: String },
    #[error(
        "not_deployable {label}: target provides no DxDeployInfo and is not executable; pass a dx_deployment target or an executable"
    )]
    NotDeployable { label: String },
    #[error("ownership query for {label} failed: {detail}")]
    QueryFailed { label: String, detail: String },
}
