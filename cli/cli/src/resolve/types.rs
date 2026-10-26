use std::io;
use std::path::{Path, PathBuf};

use dx_process::Scope;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait QueryRunner {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult>;

    /// Runs one capturing `bazel info` for output roots.
    fn run_info(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        self.run_query(argv, cwd)
    }

    /// Free bytes on the disk holding `path`, or `None` when unknown.
    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        None
    }
}

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
        "no Bazel target owns {files}: add the files to a target srcs list or pass explicit target labels (queried as {labels})",
        files = files.join(" "),
        labels = labels.join(" ")
    )]
    NoOwner {
        files: Vec<String>,
        labels: Vec<String>,
    },
    #[error(
        "no test depends on {owners}: pass an explicit test label or pattern such as //pkg/...",
        owners = owners.join(" ")
    )]
    NoTests { owners: Vec<String> },
    #[error(
        "no executable target owns {scopes}: add an executable rule owning the file or pass an explicit runnable label",
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
    #[error("query {label} failed: {failure}")]
    QueryFailed {
        label: String,
        failure: QueryFailure,
    },
}

const EXCERPT_LINES: usize = 20;
const EXCERPT_BYTES: usize = 4096;

/// Why one Bazel query did not yield a selection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueryFailure {
    /// dx could not build the query or read the scope it asks about.
    #[error("{detail}")]
    Prepare { detail: String },
    /// The query process did not start.
    #[error("could not launch Bazel: {detail}")]
    Launch { detail: String },
    /// The query ran and did not succeed.
    #[error("{}", exit_text(*.status, .excerpt.as_ref()))]
    Exit {
        status: Option<i32>,
        excerpt: Option<dx_output::Excerpt>,
    },
    /// The query exited 36 while a disk it writes to was nearly full.
    #[error("{}", crate::disk::full_message(*.status, .path, *.free))]
    DiskFull {
        status: i32,
        path: PathBuf,
        free: u64,
        excerpt: Option<dx_output::Excerpt>,
    },
    /// The query succeeded but its output is unusable.
    #[error("{detail}")]
    Output { detail: String },
}

fn exit_text(status: Option<i32>, excerpt: Option<&dx_output::Excerpt>) -> String {
    let head = match status {
        Some(code) => format!("Bazel exited {code}"),
        None => "Bazel terminated by signal".to_owned(),
    };
    match excerpt {
        Some(_) => head,
        None => format!("{head}; cause unknown: Bazel printed no diagnostic"),
    }
}

impl QueryFailure {
    /// The failure of a query that ran, with its native status and a bounded stderr tail.
    fn exited(result: &QueryResult) -> Self {
        QueryFailure::Exit {
            status: result.code,
            excerpt: dx_output::diagnostic_excerpt(&result.stderr, EXCERPT_LINES, EXCERPT_BYTES),
        }
    }

    /// The failure of a query that never started.
    pub fn launch(error: &io::Error) -> Self {
        QueryFailure::Launch {
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
impl QueryFailure {
    pub(crate) fn exit_for_test(status: i32, excerpt: &str) -> Self {
        QueryFailure::Exit {
            status: Some(status),
            excerpt: Some(dx_output::Excerpt {
                text: excerpt.to_owned(),
                truncated: false,
            }),
        }
    }
}

impl ResolveError {
    /// A failed query named by its expression.
    pub fn query(label: &str, failure: QueryFailure) -> Self {
        ResolveError::QueryFailed {
            label: label.to_owned(),
            failure,
        }
    }

    fn classify(
        runner: &dyn QueryRunner,
        argv: &[String],
        cwd: &Path,
        result: &QueryResult,
    ) -> QueryFailure {
        let failure = QueryFailure::exited(result);
        let (Some(status), QueryFailure::Exit { excerpt, .. }) = (result.code, &failure) else {
            return failure;
        };
        if status != crate::disk::BAZEL_LOCAL_ENVIRONMENT_EXIT {
            return failure;
        }
        match crate::disk::shortage(&|path| runner.free_disk_bytes(path), cwd, argv) {
            Some((path, free)) => QueryFailure::DiskFull {
                status,
                path,
                free,
                excerpt: excerpt.clone(),
            },
            None => failure,
        }
    }

    /// Runs one query and turns a launch error or an unsuccessful exit into a typed failure.
    pub fn run(
        runner: &dyn QueryRunner,
        argv: &[String],
        cwd: &Path,
        label: &str,
    ) -> Result<QueryResult, ResolveError> {
        let result = runner
            .run_query(argv, cwd)
            .map_err(|error| ResolveError::query(label, QueryFailure::launch(&error)))?;
        if result.code != Some(0) {
            return Err(ResolveError::query(
                label,
                Self::classify(runner, argv, cwd, &result),
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Disk {
        code: i32,
        free: Option<u64>,
        probed: RefCell<Vec<PathBuf>>,
    }

    impl QueryRunner for Disk {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Ok(QueryResult {
                code: Some(self.code),
                stdout: Vec::new(),
                stderr: b"ERROR: boom\n".to_vec(),
            })
        }

        fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
            self.probed.borrow_mut().push(path.to_path_buf());
            self.free
        }
    }

    fn run(code: i32, free: Option<u64>) -> (Result<QueryResult, ResolveError>, Vec<PathBuf>) {
        let runner = Disk {
            code,
            free,
            probed: RefCell::new(Vec::new()),
        };
        let argv = vec!["--output_base=/ob".to_owned(), "query".to_owned()];
        let result = ResolveError::run(&runner, &argv, Path::new("/ws"), "q");
        (result, runner.probed.into_inner())
    }

    fn failure(result: Result<QueryResult, ResolveError>) -> QueryFailure {
        match result.expect_err("failed") {
            ResolveError::QueryFailed { failure, .. } => failure,
            other => panic!("{other}"),
        }
    }

    #[test]
    fn exit_36_with_a_nearly_full_disk_is_disk_full() {
        let (result, probed) = run(36, Some(1 << 20));
        assert!(probed.contains(&PathBuf::from("/ob")), "{probed:?}");
        let QueryFailure::DiskFull { status, free, .. } = failure(result) else {
            panic!("expected DiskFull");
        };
        assert_eq!((status, free), (36, 1 << 20));
    }

    #[test]
    fn exit_36_with_free_space_or_no_reading_stays_an_exit() {
        for free in [Some(100 << 30), None] {
            let (result, _) = run(36, free);
            assert!(matches!(failure(result), QueryFailure::Exit { .. }));
        }
    }

    #[test]
    fn other_exits_never_probe_the_disk() {
        let (result, probed) = run(7, Some(1 << 20));
        assert!(probed.is_empty(), "{probed:?}");
        assert!(matches!(failure(result), QueryFailure::Exit { .. }));
    }
}
