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

    /// Runs one capturing `bazel info` for output roots.
    fn run_info(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        self.run_query(argv, cwd)
    }
}

/// A query runner that carries one invocation's Bazel startup settings into
/// every query and info launch, before the Bazel verb.
pub struct StartupQueryRunner<'a> {
    inner: &'a dyn QueryRunner,
    startup: &'a [String],
}

impl<'a> StartupQueryRunner<'a> {
    pub fn new(inner: &'a dyn QueryRunner, startup: &'a [String]) -> Self {
        StartupQueryRunner { inner, startup }
    }
}

impl QueryRunner for StartupQueryRunner<'_> {
    fn run_query(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        let mut owned = argv.to_vec();
        dx_process::splice_startup_options(&mut owned, self.startup);
        self.inner.run_query(&owned, cwd)
    }

    fn run_info(&self, argv: &[String], cwd: &Path) -> io::Result<QueryResult> {
        let mut owned = argv.to_vec();
        dx_process::splice_startup_options(&mut owned, self.startup);
        self.inner.run_info(&owned, cwd)
    }
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

#[cfg(test)]
mod startup_tests {
    use super::*;
    use std::cell::RefCell;

    struct Capturing {
        queries: RefCell<Vec<Vec<String>>>,
        infos: RefCell<Vec<Vec<String>>>,
    }

    impl QueryRunner for Capturing {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            self.queries.borrow_mut().push(argv.to_vec());
            Ok(QueryResult {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }

        fn run_info(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            self.infos.borrow_mut().push(argv.to_vec());
            Ok(QueryResult {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn startup_settings_reach_query_and_info_before_the_verb() {
        let inner = Capturing {
            queries: RefCell::new(Vec::new()),
            infos: RefCell::new(Vec::new()),
        };
        let startup = vec!["--output_base=/tmp/dx-base".to_owned()];
        let runner = StartupQueryRunner::new(&inner, &startup);
        let workspace = Path::new("/ws");
        runner
            .run_query(
                &[
                    "bazel".to_owned(),
                    "--nohome_rc".to_owned(),
                    "query".to_owned(),
                    "--".to_owned(),
                ],
                workspace,
            )
            .expect("query");
        runner
            .run_info(
                &["bazel".to_owned(), "info".to_owned(), "bazel-testlogs".to_owned()],
                workspace,
            )
            .expect("info");
        assert_eq!(
            inner.queries.borrow().as_slice(),
            &[vec![
                "bazel".to_owned(),
                "--output_base=/tmp/dx-base".to_owned(),
                "--nohome_rc".to_owned(),
                "query".to_owned(),
                "--".to_owned(),
            ]]
        );
        assert_eq!(
            inner.infos.borrow().as_slice(),
            &[vec![
                "bazel".to_owned(),
                "--output_base=/tmp/dx-base".to_owned(),
                "info".to_owned(),
                "bazel-testlogs".to_owned(),
            ]]
        );
    }

    #[test]
    fn empty_startup_settings_forward_argv_untouched() {
        let inner = Capturing {
            queries: RefCell::new(Vec::new()),
            infos: RefCell::new(Vec::new()),
        };
        let runner = StartupQueryRunner::new(&inner, &[]);
        let argv = vec!["bazel".to_owned(), "cquery".to_owned()];
        runner.run_query(&argv, Path::new("/ws")).expect("query");
        assert_eq!(inner.queries.borrow().as_slice(), &[argv]);
    }
}
