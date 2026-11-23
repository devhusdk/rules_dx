use dx_exec_core::common::{ActionTally, Env};
use dx_plan::args::{Command, Invocation};
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn temp_dir(prefix: &str) -> tempfile::TempDir {
    dx_test_scratch::scratch(&format!("dx-exec-update-test-{prefix}-")).unwrap()
}

pub(crate) struct NoQuery;

impl QueryRunner for NoQuery {
    fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        Ok(QueryResult {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

pub(crate) struct ExitZero;

impl Runner for ExitZero {
    fn run(&self, _argv: &[String], _cwd: &Path, _env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        Ok(ChildStatus::exited(0))
    }
}

pub(crate) struct TestEnv {
    pub(crate) workspace: PathBuf,
    pub(crate) temp: PathBuf,
    _workspace_guard: tempfile::TempDir,
    _temp_guard: tempfile::TempDir,
    query: NoQuery,
    actions: ActionTally,
    proc_root: PathBuf,
}

impl TestEnv {
    pub(crate) fn new(name: &str) -> Self {
        let workspace_guard = temp_dir(&format!("{name}-ws"));
        let temp_guard = temp_dir(&format!("{name}-tmp"));
        let workspace = workspace_guard.path().to_path_buf();
        let git = workspace.join(".git");
        for dir in ["objects", "refs"] {
            std::fs::create_dir_all(git.join(dir)).expect("git dir");
        }
        std::fs::write(git.join("HEAD"), "ref: refs/heads/main\n").expect("git HEAD");
        TestEnv {
            workspace,
            temp: temp_guard.path().to_path_buf(),
            _workspace_guard: workspace_guard,
            _temp_guard: temp_guard,
            query: NoQuery,
            actions: ActionTally::default(),
            proc_root: PathBuf::from("/proc"),
        }
    }

    pub(crate) fn runner(&self) -> ExitZero {
        ExitZero
    }

    pub(crate) fn env<'a>(
        &'a self,
        runner: &'a dyn Runner,
        out: &'a mut Vec<u8>,
        err: &'a mut Vec<u8>,
    ) -> Env<'a> {
        Env {
            workspace: &self.workspace,
            runner,
            query_runner: &self.query,
            temp_dir: &self.temp,
            pid: std::process::id(),
            nonce: 0,
            out,
            err,
            ci: false,
            audit_today: Some("2026-01-15"),
            actions: &self.actions,
            proc_root: &self.proc_root,
            edit_commit_json: None,
        }
    }
}

pub(crate) fn dispatch(invocation: &Invocation, env: Env<'_>) -> i32 {
    if invocation.command == Command::Update {
        crate::execute_update(invocation, env, &dispatch)
    } else {
        1
    }
}
