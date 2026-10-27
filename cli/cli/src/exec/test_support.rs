use super::{execute, ActionTally, Env};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dx_digest::blake3 as digest;
use dx_plan::args::Invocation;
use dx_plan::plan::GENERATE_ENV_INTENDED;
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};
use dx_setup::{
    commit_pair, setup_hex, GenerationId, SetupPair, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME,
};
use quality_result::proto;
use quality_result::proto::{Capability, Convergence, FileSnapshot, QualityResult, Stage};
use quality_result::{encode_validated, SCHEMA_MAJOR, SCHEMA_MINOR};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::{self};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(unix)]
use std::time::{Duration, Instant};

pub(crate) use crate::test_support::parsed as invocation;
pub(crate) use dx_testing::cli::{event, event_kinds, events_of_kind, graph_rule, json_events};

pub(crate) fn temp_dir(prefix: &str) -> tempfile::TempDir {
    dx_test_scratch::scratch(&format!("dx-exec-test-{prefix}-"))
}

/// A real file system whose atomic writes fail for the listed paths, so tests
/// can make one apply write fail without chmod tricks.
pub(crate) struct FailingWrites {
    failing: Vec<PathBuf>,
}

impl FailingWrites {
    pub(crate) fn new(failing: &[PathBuf]) -> Self {
        Self {
            failing: failing.to_vec(),
        }
    }
}

impl dx_apply::FileSystem for FailingWrites {
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        dx_apply::RealFileSystem.read(path)
    }

    fn write_atomic(&self, path: &Path, content: &[u8]) -> io::Result<()> {
        if self.failing.iter().any(|failing| failing == path) {
            return Err(io::Error::other("injected write failure"));
        }
        dx_apply::RealFileSystem.write_atomic(path, content)
    }
}

/// The event Bazel ends every complete build event stream with.
pub(crate) const LAST_EVENT: &str = r#"{"id":{"buildMetrics":{}},"lastMessage":true}"#;

pub(crate) struct Harness {
    pub(crate) workspace: PathBuf,
    pub(crate) temp: PathBuf,
    pub(crate) cwd: PathBuf,
    pub(crate) _workspace_guard: tempfile::TempDir,
    pub(crate) _temp_guard: tempfile::TempDir,
    pub(crate) results: HashMap<String, Vec<u8>>,
    pub(crate) bazel_code: i32,
    pub(crate) generate_code: Option<i32>,
    pub(crate) fail_target: bool,
    pub(crate) failed_labels: Vec<String>,
    pub(crate) io_error: bool,
    pub(crate) signalled: bool,
    /// The signal a `bazel run` app dies by; the build still exits `bazel_code`.
    pub(crate) app_signal: Option<i32>,
    pub(crate) skip_bep: bool,
    pub(crate) raw_bep: Option<Vec<String>>,
    pub(crate) unterminated_bep: bool,
    /// A `buildMetrics` line that ends the stream in place of `LAST_EVENT`.
    pub(crate) build_metrics: Option<String>,
    pub(crate) query: ScriptQuery,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) patch: Option<Vec<u8>>,
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    pub(crate) free_disk_bytes: Option<u64>,
    pub(crate) audit_today: Option<String>,
    pub(crate) actions: ActionTally,
    /// Process table root `dx clean` observes; point at a missing dir to make observation unknown.
    pub(crate) proc_root: std::path::PathBuf,
    /// Commit JSON path injected for bump GitHub tag resolution.
    pub(crate) bump_commit_json: Option<String>,
    /// The shards the fake shard probe names; `None` makes the probe exit 1 so dx passes every shard.
    pub(crate) probe_shards: Option<Vec<String>>,
    /// An exit code the fake shard probe returns instead of answering.
    pub(crate) probe_code: Option<i32>,
    /// Every shard probe argv the fake runner saw.
    pub(crate) probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
    /// The buildMetrics line the fake shard probe ends its event file with.
    pub(crate) probe_metrics: Option<String>,
    /// Every other argv the fake runner saw.
    pub(crate) build_argv: Rc<RefCell<Vec<Vec<String>>>>,
}

/// The audit date every test harness pins.
pub(crate) const TEST_AUDIT_TODAY: &str = "2026-01-15";

pub(crate) struct ScriptQuery {
    pub(crate) calls: RefCell<Vec<Vec<String>>>,
    pub(crate) expressions: RefCell<Vec<String>>,
    pub(crate) outputs: RefCell<Vec<QueryResult>>,
    pub(crate) info_calls: RefCell<Vec<Vec<String>>>,
    pub(crate) info: RefCell<Option<QueryResult>>,
    pub(crate) free_disk_bytes: Cell<Option<u64>>,
    /// Answers for `git` queries, kept apart from the Bazel queue so git calls
    /// never consume scripted cquery answers. Empty means git fails to launch.
    pub(crate) git_outputs: RefCell<Vec<QueryResult>>,
    pub(crate) git_calls: RefCell<Vec<Vec<String>>>,
}

impl ScriptQuery {
    pub(crate) fn script_stdout(&self, owners: &str) {
        self.outputs.borrow_mut().push(QueryResult {
            code: Some(0),
            stdout: owners.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }

    pub(crate) fn script_failure(&self, code: i32, stderr: &str) {
        self.outputs.borrow_mut().push(QueryResult {
            code: Some(code),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        });
    }

    pub(crate) fn script_graph(&self, rules: &[String]) {
        self.script_stdout(&dx_testing::cli::graph_stdout(rules));
    }

    pub(crate) fn script_git(&self, code: i32, stdout: &str) {
        self.git_outputs.borrow_mut().push(QueryResult {
            code: Some(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }

    pub(crate) fn script_info(&self, output: &str) {
        self.info.replace(Some(QueryResult {
            code: Some(0),
            stdout: output.as_bytes().to_vec(),
            stderr: Vec::new(),
        }));
    }
}

impl QueryRunner for ScriptQuery {
    fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        if argv.first().map(String::as_str) == Some("git") {
            self.git_calls.borrow_mut().push(argv.to_vec());
            let mut outputs = self.git_outputs.borrow_mut();
            if outputs.is_empty() {
                return Err(io::Error::other("unscripted git query"));
            }
            return Ok(outputs.remove(0));
        }
        let expression = match argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--query_file="))
        {
            Some(path) => std::fs::read_to_string(path)?,
            None => argv.last().cloned().unwrap_or_default(),
        };
        self.expressions.borrow_mut().push(expression);
        self.calls.borrow_mut().push(argv.to_vec());
        let mut outputs = self.outputs.borrow_mut();
        if outputs.is_empty() {
            return Err(io::Error::other("unscripted query"));
        }
        Ok(outputs.remove(0))
    }

    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        self.free_disk_bytes.get()
    }

    fn run_info(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
        self.info_calls.borrow_mut().push(argv.to_vec());
        Ok(self.info.borrow().clone().unwrap_or(QueryResult {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }))
    }
}

impl Harness {
    pub(crate) fn new(name: &str) -> Self {
        let workspace_guard = temp_dir(&format!("{name}-ws"));
        let temp_guard = temp_dir(&format!("{name}-tmp"));
        let workspace = workspace_guard.path().to_path_buf();
        let temp = temp_guard.path().to_path_buf();
        let cwd = workspace.clone();
        Harness {
            workspace,
            temp,
            cwd,
            _workspace_guard: workspace_guard,
            _temp_guard: temp_guard,
            results: HashMap::new(),
            bazel_code: 0,
            generate_code: None,
            fail_target: false,
            failed_labels: Vec::new(),
            io_error: false,
            signalled: false,
            app_signal: None,
            skip_bep: false,
            raw_bep: None,
            unterminated_bep: false,
            build_metrics: None,
            query: ScriptQuery {
                calls: RefCell::new(Vec::new()),
                expressions: RefCell::new(Vec::new()),
                outputs: RefCell::new(Vec::new()),
                info_calls: RefCell::new(Vec::new()),
                info: RefCell::new(None),
                free_disk_bytes: Cell::new(None),
                git_outputs: RefCell::new(Vec::new()),
                git_calls: RefCell::new(Vec::new()),
            },
            intended: None,
            patch: None,
            seen_env: Rc::new(RefCell::new(Vec::new())),
            free_disk_bytes: None,
            audit_today: Some(TEST_AUDIT_TODAY.to_owned()),
            actions: ActionTally::default(),
            proc_root: std::path::PathBuf::from("/proc"),
            bump_commit_json: None,
            probe_shards: None,
            probe_code: None,
            probe_argv: Rc::new(RefCell::new(Vec::new())),
            probe_metrics: None,
            build_argv: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(crate) fn write_source(&self, path: &str, text: &str) {
        let full = self.workspace.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("parent dir");
        std::fs::write(full, text).expect("write source");
    }

    pub(crate) fn valid_result(
        &self,
        initial: Vec<proto::Diagnostic>,
        replacements: Vec<proto::FileEdits>,
    ) -> Vec<u8> {
        let original = std::fs::read(self.workspace.join("src/a.py")).unwrap_or_default();
        self.result_full(
            initial,
            vec![],
            replacements,
            vec![FileSnapshot {
                path: "src/a.py".to_owned(),
                digest: digest(&original).to_vec(),
            }],
        )
    }

    /// A result with one fixable warning and snapshot per path, changes in the order given.
    pub(crate) fn paths_result(&self, paths: &[&str], changes: Vec<proto::FileEdits>) -> Vec<u8> {
        let initial = paths
            .iter()
            .map(|path| {
                Self::diagnostic_with(
                    proto::Severity::Warning as i32,
                    "lint-tool",
                    path,
                    "unused",
                    true,
                )
            })
            .collect();
        let snapshots = paths
            .iter()
            .map(|path| FileSnapshot {
                path: (*path).to_owned(),
                digest: digest(&std::fs::read(self.workspace.join(path)).expect("source")).to_vec(),
            })
            .collect();
        self.result_full(initial, vec![], changes, snapshots)
    }

    pub(crate) fn result_full(
        &self,
        initial: Vec<proto::Diagnostic>,
        terminal: Vec<proto::Diagnostic>,
        replacements: Vec<proto::FileEdits>,
        snapshots: Vec<FileSnapshot>,
    ) -> Vec<u8> {
        let result = QualityResult {
            schema_major: SCHEMA_MAJOR,
            schema_minor: SCHEMA_MINOR,
            producer: "//test:corpus".to_owned(),
            capability: Capability::Lint as i32,
            stages: vec![Stage {
                tool_id: "lint-tool".to_owned(),
                class_ids: vec!["python".to_owned()],
                source_paths: vec!["src/a.py".to_owned()],
            }],
            completed_rounds: 1,
            convergence: Convergence::Stable as i32,
            original_snapshot: snapshots.clone(),
            terminal_snapshot: snapshots,
            initial_diagnostics: initial,
            terminal_diagnostics: terminal,
            replacements,
        };
        encode_validated(&result).expect("encode")
    }

    pub(crate) fn diagnostic(message: &str, fixable: bool) -> proto::Diagnostic {
        Self::diagnostic_with(
            proto::Severity::Warning as i32,
            "lint-tool",
            "src/a.py",
            message,
            fixable,
        )
    }

    pub(crate) fn diagnostic_with(
        severity: i32,
        tool: &str,
        path: &str,
        message: &str,
        fixable: bool,
    ) -> proto::Diagnostic {
        proto::Diagnostic {
            severity,
            message: message.to_owned(),
            tool_id: tool.to_owned(),
            rule_id: format!("{tool}/rule"),
            path: path.to_owned(),
            start_byte: Some(0),
            end_byte: Some(1),
            fixable,
            unfixable_reason: String::new(),
            related: Vec::new(),
        }
    }

    pub(crate) fn replacement(&self, replacement: &[u8]) -> proto::FileEdits {
        self.replacement_in("src/a.py", replacement)
    }

    /// A 0..1 edit of `path` against its current digest.
    pub(crate) fn replacement_in(&self, path: &str, replacement: &[u8]) -> proto::FileEdits {
        let original = std::fs::read(self.workspace.join(path)).expect("source");
        self.replacement_at(
            path,
            digest(&original).to_vec(),
            vec![proto::Edit {
                start_byte: 0,
                end_byte: 1,
                replacement: replacement.to_vec(),
            }],
        )
    }

    pub(crate) fn replacement_at(
        &self,
        path: &str,
        original_digest: Vec<u8>,
        edits: Vec<proto::Edit>,
    ) -> proto::FileEdits {
        proto::FileEdits {
            path: path.to_owned(),
            original_digest,
            edits,
        }
    }

    /// Pairs every canned result label with its BEP artifact path and bytes.
    pub(crate) fn staged_artifacts(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut staged = Vec::new();
        for (label, bytes) in &self.results {
            let safe = label.replace(['/', ':'], "_");
            staged.push((self.temp.join(format!("{safe}.pb")), bytes.clone()));
        }
        staged
    }

    fn last_event(&self) -> String {
        self.build_metrics
            .clone()
            .unwrap_or_else(|| LAST_EVENT.to_owned())
    }

    pub(crate) fn runner(&self) -> FakeRunner {
        if let Some(lines) = &self.raw_bep {
            let mut lines = lines.clone();
            if !self.unterminated_bep {
                lines.push(self.last_event());
            }
            return FakeRunner {
                code: Some(self.bazel_code),
                generate_code: self.generate_code,
                app_signal: self.app_signal,
                bep_lines: lines,
                io_error: self.io_error,
                skip_bep: self.skip_bep,
                intended: self.intended.clone(),
                patch: self.patch.clone(),
                seen_env: Rc::clone(&self.seen_env),
                free_disk_bytes: self.free_disk_bytes,
                probe_shards: self.probe_shards.clone(),
                probe_code: self.probe_code,
                probe_argv: Rc::clone(&self.probe_argv),
                probe_metrics: self.probe_metrics.clone(),
                build_argv: Rc::clone(&self.build_argv),
            };
        }
        let staged = self.staged_artifacts();
        for (artifact, bytes) in &staged {
            std::fs::write(artifact, bytes).expect("artifact");
        }
        let mut lines = Vec::new();
        if staged.is_empty() {
            lines.push(
                serde_json::json!({
                    "id": {"targetCompleted": {"label": "//test:corpus"}},
                    "completed": {"success": !self.fail_target, "outputGroup": [{"name": "dx_results", "fileSets": []}]},
                })
                .to_string(),
            );
        }
        for (index, ((artifact, _), label)) in staged.iter().zip(self.results.keys()).enumerate() {
            let id = index.to_string();
            lines.push(
                serde_json::json!({
                    "id": {"namedSet": {"id": id}},
                    "namedSetOfFiles": {"files": [
                        {"uri": format!("file://{}", artifact.to_string_lossy())}
                    ]},
                })
                .to_string(),
            );
            lines.push(
                serde_json::json!({
                    "id": {"targetCompleted": {"label": label}},
                    "completed": {
                        "success": !self.fail_target && !self.failed_labels.contains(label),
                        "outputGroup": [{"name": "dx_results", "fileSets": [{"id": id}]}],
                    },
                })
                .to_string(),
            );
        }
        if !self.unterminated_bep {
            lines.push(self.last_event());
        }
        FakeRunner {
            code: if self.signalled {
                None
            } else {
                Some(self.bazel_code)
            },
            generate_code: self.generate_code,
            app_signal: self.app_signal,
            bep_lines: lines,
            io_error: self.io_error,
            skip_bep: self.skip_bep,
            intended: self.intended.clone(),
            patch: self.patch.clone(),
            seen_env: Rc::clone(&self.seen_env),
            free_disk_bytes: self.free_disk_bytes,
            probe_shards: self.probe_shards.clone(),
            probe_code: self.probe_code,
            probe_argv: Rc::clone(&self.probe_argv),
            probe_metrics: self.probe_metrics.clone(),
            build_argv: Rc::clone(&self.build_argv),
        }
    }

    pub(crate) fn run(&self, words: &[&str]) -> (i32, String, String) {
        self.run_with_ci(words, false)
    }

    fn parse_words(&self, words: &[&str], ci: bool) -> Result<Invocation, String> {
        let defaults = dx_plan::args::load_startup_defaults(
            &dx_process::workspace_start(&self.cwd),
            &self.cwd,
            None,
            None,
            ci,
        )
        .map_err(|failure| format!("dx: {}\n", failure.detail))?;
        let inv = dx_plan::args::parse_with_ci(
            &dx_testing::cli::strings(words),
            &|_| None,
            &defaults,
            ci,
        )
        .map_err(|error| format!("dx: {error}\n{}\n", dx_plan::args::help::usage_banner()))?;
        dx_plan::args::apply_here(&inv, &self.workspace, &self.cwd)
            .map_err(|detail| format!("dx: {detail}\n"))
    }

    /// Runs one quality command whose apply writes go through `fs`.
    pub(crate) fn run_with_fs(
        &self,
        words: &[&str],
        fs: &dyn dx_apply::FileSystem,
    ) -> (i32, String, String) {
        let inv = match self.parse_words(words, false) {
            Ok(inv) => inv,
            Err(detail) => return (2, String::new(), detail),
        };
        let request = inv.quality_request().expect("quality request");
        let runner = self.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let run = super::quality::execute_quality_phase_with_fs(
            &request,
            self.env(&runner, &mut out, &mut err),
            fs,
        );
        (
            run.code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    pub(crate) fn run_with_ci(&self, words: &[&str], ci: bool) -> (i32, String, String) {
        let inv = match self.parse_words(words, ci) {
            Ok(inv) => inv,
            Err(detail) => return (2, String::new(), detail),
        };
        let runner = self.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut env = self.env(&runner, &mut out, &mut err);
        env.ci = ci;
        let code = execute(&inv, env);
        (
            code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    pub(crate) fn env<'a>(
        &'a self,
        runner: &'a dyn Runner,
        out: &'a mut Vec<u8>,
        err: &'a mut Vec<u8>,
    ) -> Env<'a> {
        self.env_with(runner, out, err)
    }

    pub(crate) fn env_with<'a, O: std::io::Write, E: std::io::Write>(
        &'a self,
        runner: &'a dyn Runner,
        out: &'a mut O,
        err: &'a mut E,
    ) -> Env<'a> {
        self.query.free_disk_bytes.set(self.free_disk_bytes);
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
            audit_today: self.audit_today.as_deref(),
            actions: &self.actions,
            proc_root: &self.proc_root,
            bump_commit_json: self.bump_commit_json.as_deref(),
        }
    }

    /// Runs one invocation against arbitrary streams and returns its exit code.
    pub(crate) fn run_streams<R: Runner, O: std::io::Write, E: std::io::Write>(
        &self,
        inv: &Invocation,
        runner: &R,
        out: &mut O,
        err: &mut E,
    ) -> i32 {
        execute(inv, self.env_with(runner, out, err))
    }

    pub(crate) fn execute_with<R: Runner>(
        &self,
        inv: &Invocation,
        runner: &R,
    ) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(inv, self.env(runner, &mut out, &mut err));
        (
            code,
            String::from_utf8(out).expect("stdout"),
            String::from_utf8(err).expect("stderr"),
        )
    }

    /// Runs one invocation through a recording runner and returns what it saw.
    pub(crate) fn probe_with(&self, inv: &Invocation, codes: &[Option<i32>]) -> ProbeRun {
        let mut out = Vec::new();
        let mut run = self.probe_streams(inv, codes, &mut out, &mut Vec::new());
        run.out = String::from_utf8(out).expect("stdout");
        run
    }

    /// Like `probe_with`, writing to the given streams; `out` in the result stays empty.
    pub(crate) fn probe_streams<O: std::io::Write, E: std::io::Write>(
        &self,
        inv: &Invocation,
        codes: &[Option<i32>],
        out: &mut O,
        err: &mut E,
    ) -> ProbeRun {
        let probe = ArgvProbe {
            codes: codes.to_vec(),
            seen: Rc::new(RefCell::new(Vec::new())),
            seen_env: Rc::clone(&self.seen_env),
            free_disk_bytes: self.free_disk_bytes,
            probe_argv: Rc::clone(&self.probe_argv),
        };
        let code = execute(inv, self.env_with(&probe, out, err));
        let argv = probe.seen.borrow().clone();
        ProbeRun {
            code,
            out: String::new(),
            argv,
        }
    }
}

/// What one probe run saw: the exit code, its stdout and every argv it launched.
#[derive(Debug)]
pub(crate) struct ProbeRun {
    pub(crate) code: i32,
    pub(crate) out: String,
    pub(crate) argv: Vec<Vec<String>>,
}

/// A runner that records each argv and environment and answers each launch with the next code.
/// Shard probe launches go to `probe_argv` and exit 1, so `seen` and `codes` count only the real runs.
struct ArgvProbe {
    codes: Vec<Option<i32>>,
    seen: Rc<RefCell<Vec<Vec<String>>>>,
    seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    free_disk_bytes: Option<u64>,
    probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
}

/// Whether one argv is the quality shard probe.
pub(crate) fn is_shard_probe(argv: &[String]) -> bool {
    argv.iter().any(|arg| arg == "--output_groups=dx_shards")
}

impl Runner for ArgvProbe {
    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        self.free_disk_bytes
    }

    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if is_shard_probe(argv) {
            self.probe_argv.borrow_mut().push(argv.to_vec());
            return Ok(ChildStatus {
                code: Some(1),
                signal: None,
            });
        }
        let launch = self.seen.borrow().len();
        let last = self.codes.len().saturating_sub(1);
        self.seen.borrow_mut().push(argv.to_vec());
        self.seen_env.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        Ok(ChildStatus {
            code: self.codes[launch.min(last)],
            signal: None,
        })
    }
}

pub(crate) struct FakeRunner {
    pub(crate) code: Option<i32>,
    pub(crate) generate_code: Option<i32>,
    pub(crate) app_signal: Option<i32>,
    pub(crate) bep_lines: Vec<String>,
    pub(crate) io_error: bool,
    pub(crate) skip_bep: bool,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) patch: Option<Vec<u8>>,
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    pub(crate) free_disk_bytes: Option<u64>,
    pub(crate) probe_shards: Option<Vec<String>>,
    pub(crate) probe_code: Option<i32>,
    pub(crate) probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
    /// The buildMetrics line the fake shard probe ends its event file with.
    pub(crate) probe_metrics: Option<String>,
    pub(crate) build_argv: Rc<RefCell<Vec<Vec<String>>>>,
}

impl FakeRunner {
    /// Answers one shard probe: a dx_shards file naming `probe_shards`, or exit 1 when unset.
    fn run_shard_probe(&self, argv: &[String]) -> io::Result<ChildStatus> {
        self.probe_argv.borrow_mut().push(argv.to_vec());
        if let Some(code) = self.probe_code {
            return Ok(ChildStatus {
                code: Some(code),
                signal: None,
            });
        }
        let (Some(shards), Some(bep)) = (
            &self.probe_shards,
            argv.iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file=")),
        ) else {
            return Ok(ChildStatus {
                code: Some(1),
                signal: None,
            });
        };
        let artifact = format!("{bep}.shards.json");
        std::fs::write(
            &artifact,
            serde_json::json!({ "shards": shards }).to_string(),
        )
        .expect("shard file");
        let lines = [
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": [{"uri": format!("file://{artifact}")}]},
            })
            .to_string(),
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//test:corpus"}},
                "completed": {"success": true, "outputGroup": [{"name": "dx_shards", "fileSets": [{"id": "0"}]}]},
            })
            .to_string(),
            self.probe_metrics
                .clone()
                .unwrap_or_else(|| LAST_EVENT.to_owned()),
        ];
        std::fs::write(bep, lines.join("\n")).expect("probe BEP file");
        Ok(ChildStatus {
            code: Some(0),
            signal: None,
        })
    }
}

impl Runner for FakeRunner {
    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        self.free_disk_bytes
    }

    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
        if is_shard_probe(argv) {
            return self.run_shard_probe(argv);
        }
        self.build_argv.borrow_mut().push(argv.to_vec());
        self.seen_env.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        if !self.skip_bep {
            if let Some(bep) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
            {
                std::fs::write(bep, self.bep_lines.join("\n")).expect("BEP file");
            }
        }
        if let (Some(payload), Some(path)) = (
            &self.patch,
            argv.iter().find_map(|arg| arg.strip_prefix("-patch=")),
        ) {
            std::fs::write(path, payload).expect("patch file");
        }
        if let Some(path) = env
            .iter()
            .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
        {
            if let Some(witness) = &self.intended {
                std::fs::write(path, witness).expect("intended witness");
            }
            if let Some(code) = self.generate_code {
                return Ok(ChildStatus::exited(code));
            }
        }
        if let Some(signo) = self.app_signal {
            if bazel_subcommand(argv) == Some("run") {
                return Ok(ChildStatus::signalled(Some(signo)));
            }
        }
        Ok(ChildStatus {
            code: self.code,
            signal: None,
        })
    }
}

/// The Bazel subcommand in `argv`: the first word after the binary that is not a startup option.
fn bazel_subcommand(argv: &[String]) -> Option<&str> {
    argv.iter()
        .skip(1)
        .map(String::as_str)
        .find(|word| !word.starts_with('-'))
}

pub(crate) fn intended_witness(mode: &str, complete: bool, files: &str, ignored: &str) -> Vec<u8> {
    let result = if files.trim().is_empty() {
        "clean"
    } else {
        "diff_found"
    };
    intended_witness_with_result(mode, result, complete, files, ignored)
}

pub(crate) fn intended_witness_with_result(
    mode: &str,
    result: &str,
    complete: bool,
    files: &str,
    ignored: &str,
) -> Vec<u8> {
    let files_value: Vec<serde_json::Value> = if files.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&format!("[{files}]")).expect("files JSON")
    };
    let ignored_value: Vec<serde_json::Value> = if ignored.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&format!("[{ignored}]")).expect("ignored JSON")
    };
    serde_json::to_vec(&serde_json::json!({
        "schema_major": 1,
        "schema_minor": 1,
        "mode": mode,
        "result": result,
        "scopes": [{"value": "//...", "results_complete": complete}],
        "files": files_value,
        "ignored_imports": ignored_value,
    }))
    .expect("witness JSON")
}

pub(crate) fn intended_modify(path: &str, original: &[u8], candidate: &[u8]) -> String {
    serde_json::json!({
        "path": path,
        "scope_index": 0,
        "original_content": STANDARD.encode(original),
        "edits": [{
            "start_byte": 0,
            "end_byte": original.len(),
            "replacement": STANDARD.encode(candidate),
        }],
    })
    .to_string()
}

pub(crate) fn intended_create(path: &str, content: &[u8]) -> String {
    serde_json::json!({
        "path": path,
        "scope_index": 0,
        "create_content": STANDARD.encode(content),
    })
    .to_string()
}

pub(crate) fn intended_ignored(path: &str, language: &str, import: &str) -> String {
    serde_json::json!({
        "path": path,
        "language": language,
        "import": import,
        "scope_index": 0,
    })
    .to_string()
}

pub(crate) fn write_bep_artifact(harness: &Harness, name: &str, bytes: &[u8]) -> String {
    let path = harness.temp.join(name);
    std::fs::write(&path, bytes).expect("artifact");
    format!("file://{}", path.display())
}

/// A final `buildMetrics` event with one action summary, int64 values written as strings.
pub(crate) fn build_metrics_line(
    created: u64,
    executed_raw: u64,
    disk: u64,
    remote: u64,
    local: u64,
) -> String {
    serde_json::json!({
        "id": {"buildMetrics": {}},
        "buildMetrics": {"actionSummary": {
            "actionsCreated": created.to_string(),
            "actionsExecuted": executed_raw.to_string(),
            "runnerCount": [
                {"name": "total", "count": executed_raw},
                {"name": "disk cache hit", "count": disk, "execKind": "Remote"},
                {"name": "remote cache hit", "count": remote, "execKind": "Remote"},
            ],
            "actionCacheStatistics": {"hits": local},
        }},
        "lastMessage": true,
    })
    .to_string()
}

/// The `actions` object of the last `command_finished` event on `out`.
pub(crate) fn finished_actions(out: &str) -> Option<serde_json::Value> {
    out.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["event"] == "command_finished")
        .and_then(|event| event.get("actions").cloned())
}

pub(crate) fn test_result_line(label: &str, entries: &[(String, String)]) -> String {
    test_result_identity_line(label, 1, 1, 1, entries)
}

pub(crate) fn test_result_identity_line(
    label: &str,
    run: u32,
    shard: u32,
    attempt: u32,
    entries: &[(String, String)],
) -> String {
    let outputs: Vec<serde_json::Value> = entries
        .iter()
        .map(|(name, uri)| serde_json::json!({"name": name, "uri": uri}))
        .collect();
    serde_json::json!({
        "id": {"testResult": {"label": label, "run": run, "shard": shard, "attempt": attempt}},
        "testResult": {"status": "PASSED", "testActionOutput": outputs},
    })
    .to_string()
}

/// A BEP `testResult` line carrying the status, cache, and timing facts dx reports.
pub(crate) fn test_result_full_line(
    label: &str,
    run: u32,
    shard: u32,
    attempt: u32,
    status: &str,
    configuration: Option<&str>,
    cached: Option<bool>,
    duration_millis: Option<u64>,
    entries: &[(String, String)],
) -> String {
    let outputs: Vec<serde_json::Value> = entries
        .iter()
        .map(|(name, uri)| serde_json::json!({"name": name, "uri": uri}))
        .collect();
    let mut id =
        serde_json::json!({"label": label, "run": run, "shard": shard, "attempt": attempt});
    if let Some(configuration) = configuration {
        id["configuration"] = serde_json::json!({"id": configuration});
    }
    let mut body = serde_json::json!({"status": status, "testActionOutput": outputs});
    if let Some(cached) = cached {
        body["cachedLocally"] = serde_json::json!(cached);
    }
    if let Some(duration) = duration_millis {
        body["testAttemptDurationMillis"] = serde_json::json!(duration);
    }
    serde_json::json!({"id": {"testResult": id}, "testResult": body}).to_string()
}

pub(crate) fn test_summary_line(label: &str, shard_count: u32) -> String {
    serde_json::json!({
        "id": {"testSummary": {"label": label}},
        "testSummary": {"overallStatus": "PASSED", "shardCount": shard_count},
    })
    .to_string()
}

pub(crate) fn test_flaky_summary_line(label: &str) -> String {
    serde_json::json!({
        "id": {"testSummary": {"label": label}},
        "testSummary": {"overallStatus": "FLAKY", "attemptCount": 2},
    })
    .to_string()
}

pub(crate) fn coverage_harness(name: &str, tracefile: &[u8]) -> Harness {
    let harness = Harness::new(name);
    let uri = write_bep_artifact(&harness, "coverage.dat", tracefile);
    Harness {
        raw_bep: Some(vec![test_result_line(
            "//a:t",
            &[(String::from("test.lcov"), uri)],
        )]),
        ..harness
    }
}

pub(crate) fn generate_witness(workspace_text: &str, name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", workspace_text);
    harness.intended = Some(intended_witness(
        "default",
        true,
        &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
        "",
    ));
    harness
}

pub(crate) fn commit_clean_pair(harness: &Harness, env: char, gen: char) -> String {
    let pair = SetupPair {
        environment: GenerationId::new(&env.to_string().repeat(64)).expect("environment digest"),
        generated: GenerationId::new(&gen.to_string().repeat(64)).expect("generated digest"),
    };
    commit_pair(&harness.workspace, &pair).expect("commit pair");
    for (dir, tag) in [(ENVIRONMENTS_DIR_NAME, env), (GENERATED_DIR_NAME, gen)] {
        std::fs::create_dir_all(
            harness
                .workspace
                .join(".dx")
                .join(dir)
                .join(tag.to_string().repeat(64)),
        )
        .expect("generation dir");
    }
    setup_hex(&pair)
}

pub(crate) fn managed_shard_bep(shard: &Path, group: &str) -> Vec<String> {
    vec![
        serde_json::json!({
            "id": {"namedSet": {"id": "0"}},
            "namedSetOfFiles": {"files": [{"uri": format!("file://{}", shard.to_string_lossy())}]},
        })
        .to_string(),
        serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:one"}},
            "completed": {
                "success": true,
                "outputGroup": [{"name": group, "fileSets": [{"id": "0"}]}],
            },
        })
        .to_string(),
    ]
}

pub(crate) struct ManagedStageFixture {
    pub(crate) workspace_guard: tempfile::TempDir,
    pub(crate) _artifacts_guard: tempfile::TempDir,
    pub(crate) first: PathBuf,
    pub(crate) second: PathBuf,
}

impl ManagedStageFixture {
    pub(crate) fn workspace(&self) -> &Path {
        self.workspace_guard.path()
    }
}

pub(crate) fn managed_stage_fixture(name: &str) -> ManagedStageFixture {
    let workspace_guard = temp_dir(&format!("{name}-ws"));
    let artifacts_guard = temp_dir(&format!("{name}-artifacts"));
    let first = artifacts_guard.path().join("first.txt");
    let second = artifacts_guard.path().join("second.txt");
    std::fs::write(&first, "first").expect("artifact");
    std::fs::write(&second, "second").expect("artifact");
    ManagedStageFixture {
        workspace_guard,
        _artifacts_guard: artifacts_guard,
        first,
        second,
    }
}

pub(crate) fn codegen_entry(logical: &str, artifact: &Path) -> dx_codegen::ProjectionEntry {
    dx_codegen::ProjectionEntry {
        logical_path: logical.to_owned(),
        artifact: artifact.to_string_lossy().into_owned(),
        import_root: String::new(),
        namespace: String::new(),
        replaces: String::new(),
    }
}

pub(crate) fn codegen_replacement_entry(
    logical: &str,
    artifact: &Path,
) -> dx_codegen::ProjectionEntry {
    dx_codegen::ProjectionEntry {
        logical_path: logical.to_owned(),
        artifact: artifact.to_string_lossy().into_owned(),
        import_root: String::new(),
        namespace: String::new(),
        replaces: logical.to_owned(),
    }
}

pub(crate) fn env_entry(key: &str, value: &str, artifact: &Path) -> dx_env_plan::ProjectionEntry {
    dx_env_plan::ProjectionEntry {
        key: key.to_owned(),
        value: value.to_owned(),
        artifact: artifact.to_string_lossy().into_owned(),
    }
}

#[cfg(unix)]
pub(crate) fn set_mode(path: &Path, mode: u32) {
    let mut permissions = std::fs::metadata(path)
        .expect("mode metadata")
        .permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, mode);
    std::fs::set_permissions(path, permissions).expect("set mode");
}

pub(crate) fn umbrella_clean(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    harness.intended = Some(intended_witness("check", true, "", ""));
    harness
}

pub(crate) fn umbrella_findings(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(
            vec![Harness::diagnostic("unused", true)],
            vec![harness.replacement(b"y")],
        ),
    );
    harness
}

/// Two sources, src/a.py and src/b.py, each with a fixable finding and a one-byte edit.
pub(crate) fn two_file_replacements(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.write_source("src/b.py", "a = 1\n");
    let changes = vec![
        harness.replacement_in("src/a.py", b"y"),
        harness.replacement_in("src/b.py", b"b"),
    ];
    let result = harness.paths_result(&["src/a.py", "src/b.py"], changes);
    harness.results.insert("//test:corpus".to_owned(), result);
    harness
}

/// Rendered rustc E0308 output with ANSI colour codes.
pub(crate) const RENDERED_E0308: &str =
    "\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror[E0308]\u{1b}[0m\u{1b}[1m: mismatched types\u{1b}[0m
\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m--> \u{1b}[0mapp/src/main.rs:4:26
\u{1b}[0m  \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m
\u{1b}[0m4\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m fn _dx_broken() -> u32 { \"x\" }
\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m                    \u{1b}[1m\u{1b}[38;5;9m^^^\u{1b}[0m expected `u32`, found `&str`

\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror\u{1b}[0m\u{1b}[1m: aborting due to 1 previous error\u{1b}[0m
";

/// Writes action stderr into the harness and returns its file URI.
pub(crate) fn stderr_file(harness: &Harness, name: &str, text: &str) -> String {
    let dir = harness.temp.join("stderr");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("stderr");
    format!("file://{}", path.to_string_lossy())
}

/// A failed actionCompleted event whose stderr is at `uri`.
pub(crate) fn failed_action(
    label: &str,
    config: &str,
    mnemonic: &str,
    output: &str,
    uri: &str,
) -> String {
    serde_json::json!({
        "id": {"actionCompleted": {
            "primaryOutput": output,
            "label": label,
            "configuration": {"id": config},
        }},
        "action": {
            "success": false,
            "type": mnemonic,
            "exitCode": 1,
            "stderr": {"name": "stderr", "uri": uri},
            "label": label,
            "configuration": {"id": config},
            "failureDetail": {
                "message": "Rustc failed: exec env - SECRET_TOKEN=hunter2 /bin/rustc",
                "spawn": {"code": "NON_ZERO_EXIT"},
            },
        },
    })
    .to_string()
}

// Pipe helpers: dropping a reader is not enough when a forked child may
// still hold a copy of the read end, so wait until the kernel reports it gone.
#[cfg(unix)]
/// Reports whether the pipe behind `writer` has no reader left in any process.
#[cfg(unix)]
pub(crate) fn reader_gone(writer: &io::PipeWriter, timeout_ms: i32) -> bool {
    let mut fd = libc::pollfd {
        fd: writer.as_raw_fd(),
        events: 0,
        revents: 0,
    };
    let ready = unsafe { libc::poll(&mut fd, 1, timeout_ms) };
    ready > 0 && fd.revents & (libc::POLLERR | libc::POLLHUP) != 0
}

/// Drops `reader` and waits until no process holds a copy of the read end.
#[cfg(unix)]
pub(crate) fn hang_up(reader: io::PipeReader, writer: &io::PipeWriter) {
    drop(reader);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !reader_gone(writer, 1000) {
        assert!(Instant::now() < deadline, "pipe reader still held");
    }
}

/// Returns the write end of a pipe whose read end is closed everywhere.
#[cfg(unix)]
pub(crate) fn closed_pipe() -> io::PipeWriter {
    let (reader, writer) = io::pipe().expect("pipe");
    hang_up(reader, &writer);
    writer
}

/// Windows has no poll on pipes, so dropping the only reader is the best available.
#[cfg(windows)]
pub(crate) fn closed_pipe() -> io::PipeWriter {
    let (reader, writer) = io::pipe().expect("pipe");
    drop(reader);
    writer
}
