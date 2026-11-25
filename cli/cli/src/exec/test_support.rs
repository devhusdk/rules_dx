use super::{ActionTally, Env, execute};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use dx_exec_quality_testing as quality;
use dx_plan::args::Invocation;
use dx_plan::plan::GENERATE_ENV_INTENDED;
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_process::{ChildStatus, Runner};
use dx_setup::{
    ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME, GenerationId, SetupPair, commit_pair, setup_hex,
};
use quality_result::proto;
use quality_result::proto::FileSnapshot;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
#[cfg(unix)]
use std::time::{Duration, Instant};

pub(crate) use crate::test_support::parsed as invocation;
pub(crate) use dx_testing::cli::{
    event, event_kinds, events_of_kind, graph_rule, graph_rule_of_kind, json_events,
};

pub(crate) fn temp_dir(prefix: &str) -> tempfile::TempDir {
    dx_test_scratch::scratch(&format!("dx-exec-test-{prefix}-")).unwrap()
}

pub(crate) use dx_exec_quality_testing::{FailingWrites, LAST_EVENT};

#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag switches one fake behavior"
)]
pub(crate) struct Harness {
    pub(crate) workspace: PathBuf,
    pub(crate) temp: PathBuf,
    pub(crate) cwd: PathBuf,
    pub(crate) _workspace_guard: tempfile::TempDir,
    pub(crate) _temp_guard: tempfile::TempDir,
    pub(crate) results: BTreeMap<String, Vec<u8>>,
    pub(crate) bazel_code: i32,
    pub(crate) generate_code: Option<i32>,
    pub(crate) fail_target: bool,
    pub(crate) failed_labels: Vec<String>,
    pub(crate) io_error: bool,
    pub(crate) signalled: bool,
    pub(crate) app_signal: Option<i32>,
    pub(crate) app_code: Option<i32>,
    pub(crate) skip_bep: bool,
    pub(crate) raw_bep: Option<Vec<String>>,
    pub(crate) unterminated_bep: bool,
    pub(crate) build_metrics: Option<String>,
    pub(crate) query: ScriptQuery,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) patch: Option<Vec<u8>>,
    #[expect(clippy::type_complexity, reason = "the type is spelled out once")]
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    pub(crate) seen_cwd: Rc<RefCell<Vec<PathBuf>>>,
    pub(crate) free_disk_bytes: Option<u64>,
    pub(crate) audit_today: Option<String>,
    pub(crate) actions: ActionTally,
    pub(crate) proc_root: PathBuf,
    pub(crate) edit_commit_json: Option<String>,
    pub(crate) probe_shards: Option<Vec<String>>,
    pub(crate) probe_code: Option<i32>,
    pub(crate) probe_aborted: Option<String>,
    pub(crate) probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
    pub(crate) probe_metrics: Option<String>,
    pub(crate) build_argv: Rc<RefCell<Vec<Vec<String>>>>,
    pub(crate) launch_codes: Vec<i32>,
}

pub(crate) const TEST_AUDIT_TODAY: &str = "2026-01-15";

#[derive(Default)]
pub(crate) struct ScriptQuery {
    pub(crate) calls: RefCell<Vec<Vec<String>>>,
    pub(crate) expressions: RefCell<Vec<String>>,
    pub(crate) outputs: RefCell<Vec<QueryResult>>,
    pub(crate) info_calls: RefCell<Vec<Vec<String>>>,
    pub(crate) info: RefCell<Option<QueryResult>>,
    pub(crate) free_disk_bytes: Cell<Option<u64>>,
    pub(crate) git_outputs: RefCell<Vec<QueryResult>>,
    pub(crate) git_calls: RefCell<Vec<Vec<String>>>,
    pub(crate) checkout_git_outputs: RefCell<Vec<QueryResult>>,
    pub(crate) images_outputs: RefCell<Vec<QueryResult>>,
    pub(crate) test_support_outputs: RefCell<Vec<QueryResult>>,
    pub(crate) test_support_expressions: RefCell<Vec<String>>,
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

    pub(crate) fn script_checkout_git(&self, code: i32, stdout: &str) {
        self.checkout_git_outputs.borrow_mut().push(QueryResult {
            code: Some(code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }

    pub(crate) fn script_images(&self, stdout: &str) {
        self.images_outputs.borrow_mut().push(QueryResult {
            code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        });
    }

    pub(crate) fn script_test_support(&self, stdout: &str) {
        self.test_support_outputs.borrow_mut().push(QueryResult {
            code: Some(0),
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
        if argv.first().map(String::as_str) == Some("git")
            && argv.iter().any(|arg| {
                [
                    "--is-inside-work-tree",
                    "--cached",
                    "--porcelain",
                    "HEAD^{commit}",
                ]
                .contains(&arg.as_str())
            })
        {
            let mut outputs = self.checkout_git_outputs.borrow_mut();
            if outputs.is_empty() {
                return Err(io::Error::other("unscripted checkout query"));
            }
            return Ok(outputs.remove(0));
        }
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
        if expression.contains("attr(testonly, 1, $s)") {
            self.test_support_expressions.borrow_mut().push(expression);
            let mut outputs = self.test_support_outputs.borrow_mut();
            if outputs.is_empty() {
                return Ok(QueryResult {
                    code: Some(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            return Ok(outputs.remove(0));
        }
        self.expressions.borrow_mut().push(expression.clone());
        self.calls.borrow_mut().push(argv.to_vec());
        if expression.contains("\"_oci_config\"")
            || argv.iter().any(|arg| arg == "--output_groups=dx_oci_image")
        {
            let mut outputs = self.images_outputs.borrow_mut();
            if outputs.is_empty() {
                return Ok(QueryResult {
                    code: Some(0),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            return Ok(outputs.remove(0));
        }
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
        dx_testing::empty_git_repo(&workspace);
        let base = workspace.join(dx_audit::secrets::BASE_CONFIG_PATH);
        std::fs::create_dir_all(base.parent().expect("parent")).expect("base dir");
        std::fs::write(base, "[extend]\nuseDefault = true\n").expect("write gitleaks base");
        Harness {
            workspace,
            temp,
            cwd,
            _workspace_guard: workspace_guard,
            _temp_guard: temp_guard,
            results: BTreeMap::new(),
            bazel_code: 0,
            generate_code: None,
            fail_target: false,
            failed_labels: Vec::new(),
            io_error: false,
            signalled: false,
            app_signal: None,
            app_code: None,
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
                checkout_git_outputs: RefCell::new(Vec::new()),
                images_outputs: RefCell::new(Vec::new()),
                test_support_outputs: RefCell::new(Vec::new()),
                test_support_expressions: RefCell::new(Vec::new()),
            },
            intended: None,
            patch: None,
            seen_env: Rc::new(RefCell::new(Vec::new())),
            seen_cwd: Rc::new(RefCell::new(Vec::new())),
            free_disk_bytes: None,
            audit_today: Some(TEST_AUDIT_TODAY.to_owned()),
            actions: ActionTally::default(),
            proc_root: PathBuf::from("/proc"),
            edit_commit_json: None,
            probe_shards: None,
            probe_code: None,
            probe_aborted: None,
            probe_argv: Rc::new(RefCell::new(Vec::new())),
            probe_metrics: None,
            build_argv: Rc::new(RefCell::new(Vec::new())),
            launch_codes: Vec::new(),
        }
    }

    pub(crate) fn write_source(&self, path: &str, text: &str) {
        quality::write_source(&self.workspace, path, text);
    }

    pub(crate) fn valid_result(
        &self,
        initial: Vec<proto::Diagnostic>,
        replacements: Vec<proto::FileEdits>,
    ) -> Vec<u8> {
        quality::valid_result(&self.workspace, initial, replacements)
    }

    pub(crate) fn paths_result(&self, paths: &[&str], changes: Vec<proto::FileEdits>) -> Vec<u8> {
        quality::paths_result(&self.workspace, paths, changes)
    }

    #[expect(clippy::unused_self, reason = "Harness methods keep one call shape")]
    pub(crate) fn result_full(
        &self,
        initial: Vec<proto::Diagnostic>,
        terminal: Vec<proto::Diagnostic>,
        replacements: Vec<proto::FileEdits>,
        snapshots: Vec<FileSnapshot>,
    ) -> Vec<u8> {
        quality::result_full(initial, terminal, replacements, snapshots)
    }

    pub(crate) fn diagnostic(message: &str, fixable: bool) -> proto::Diagnostic {
        quality::diagnostic(message, fixable)
    }

    pub(crate) fn diagnostic_with(
        severity: i32,
        tool: &str,
        path: &str,
        message: &str,
        fixable: bool,
    ) -> proto::Diagnostic {
        quality::diagnostic_with(severity, tool, path, message, fixable)
    }

    pub(crate) fn replacement(&self, replacement: &[u8]) -> proto::FileEdits {
        quality::replacement_in(&self.workspace, "src/a.py", replacement)
    }

    pub(crate) fn replacement_in(&self, path: &str, replacement: &[u8]) -> proto::FileEdits {
        quality::replacement_in(&self.workspace, path, replacement)
    }

    #[expect(clippy::unused_self, reason = "Harness methods keep one call shape")]
    pub(crate) fn replacement_at(
        &self,
        path: &str,
        original_digest: Vec<u8>,
        edits: Vec<proto::Edit>,
    ) -> proto::FileEdits {
        quality::replacement_at(path, original_digest, edits)
    }

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
                app_code: self.app_code,
                bep_lines: lines,
                io_error: self.io_error,
                skip_bep: self.skip_bep,
                intended: self.intended.clone(),
                patch: self.patch.clone(),
                seen_env: Rc::clone(&self.seen_env),
                seen_cwd: Rc::clone(&self.seen_cwd),
                free_disk_bytes: self.free_disk_bytes,
                probe_shards: self.probe_shards.clone(),
                probe_code: self.probe_code,
                probe_aborted: self.probe_aborted.clone(),
                probe_argv: Rc::clone(&self.probe_argv),
                probe_metrics: self.probe_metrics.clone(),
                build_argv: Rc::clone(&self.build_argv),
                launch_codes: self.launch_codes.clone(),
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
            app_code: self.app_code,
            bep_lines: lines,
            io_error: self.io_error,
            skip_bep: self.skip_bep,
            intended: self.intended.clone(),
            patch: self.patch.clone(),
            seen_env: Rc::clone(&self.seen_env),
            seen_cwd: Rc::clone(&self.seen_cwd),
            free_disk_bytes: self.free_disk_bytes,
            probe_shards: self.probe_shards.clone(),
            probe_code: self.probe_code,
            probe_aborted: self.probe_aborted.clone(),
            probe_argv: Rc::clone(&self.probe_argv),
            probe_metrics: self.probe_metrics.clone(),
            build_argv: Rc::clone(&self.build_argv),
            launch_codes: self.launch_codes.clone(),
        }
    }

    pub(crate) fn run(&self, words: &[&str]) -> (i32, String, String) {
        self.run_with_ci(words, false)
    }

    fn parse_words(&self, words: &[&str], ci: bool) -> Result<Invocation, String> {
        let inv = dx_plan::args::parse_with_ci(&dx_testing::cli::strings(words), &|_| None, ci)
            .map_err(|error| format!("dx: {error}\n{}\n", dx_plan::args::help::usage_banner()))?;
        dx_plan::args::apply_here(&inv, &self.workspace, &self.cwd)
            .map_err(|detail| format!("dx: {detail}\n"))
    }

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
        let run = dx_exec_quality::execute_quality_phase_with_fs(
            &request,
            self.env(&runner, &mut out, &mut err),
            fs,
            None,
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

    pub(crate) fn env_with<'a, O: io::Write, E: io::Write>(
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
            edit_commit_json: self.edit_commit_json.as_deref(),
        }
    }

    pub(crate) fn run_streams<R: Runner, O: io::Write, E: io::Write>(
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

    pub(crate) fn probe_with(&self, inv: &Invocation, codes: &[Option<i32>]) -> ProbeRun {
        let mut out = Vec::new();
        let mut run = self.probe_streams(inv, codes, &mut out, &mut Vec::new());
        run.out = String::from_utf8(out).expect("stdout");
        run
    }

    pub(crate) fn probe_streams<O: io::Write, E: io::Write>(
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

#[derive(Debug)]
pub(crate) struct ProbeRun {
    pub(crate) code: i32,
    pub(crate) out: String,
    pub(crate) argv: Vec<Vec<String>>,
}

struct ArgvProbe {
    codes: Vec<Option<i32>>,
    seen: Rc<RefCell<Vec<Vec<String>>>>,
    #[expect(clippy::type_complexity, reason = "the type is spelled out once")]
    seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    free_disk_bytes: Option<u64>,
    probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
}

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
    pub(crate) app_code: Option<i32>,
    pub(crate) bep_lines: Vec<String>,
    pub(crate) io_error: bool,
    pub(crate) skip_bep: bool,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) patch: Option<Vec<u8>>,
    #[expect(clippy::type_complexity, reason = "the type is spelled out once")]
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    pub(crate) seen_cwd: Rc<RefCell<Vec<PathBuf>>>,
    pub(crate) free_disk_bytes: Option<u64>,
    pub(crate) probe_shards: Option<Vec<String>>,
    pub(crate) probe_code: Option<i32>,
    pub(crate) probe_aborted: Option<String>,
    pub(crate) probe_argv: Rc<RefCell<Vec<Vec<String>>>>,
    pub(crate) probe_metrics: Option<String>,
    pub(crate) build_argv: Rc<RefCell<Vec<Vec<String>>>>,
    pub(crate) launch_codes: Vec<i32>,
}

impl FakeRunner {
    fn run_shard_probe(&self, argv: &[String]) -> io::Result<ChildStatus> {
        self.probe_argv.borrow_mut().push(argv.to_vec());
        let (Some(shards), Some(bep)) = (
            &self.probe_shards,
            argv.iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file=")),
        ) else {
            return Ok(ChildStatus {
                code: Some(self.probe_code.unwrap_or(1)),
                signal: None,
            });
        };
        let files = shards
            .iter()
            .enumerate()
            .map(|(index, shard)| {
                let artifact = format!("{bep}.shards{index}.json");
                std::fs::write(
                    &artifact,
                    serde_json::json!({ "shards": [shard] }).to_string(),
                )?;
                Ok(serde_json::json!({"uri": format!("file://{artifact}")}))
            })
            .collect::<io::Result<Vec<serde_json::Value>>>()?;
        let mut lines = vec![
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": files},
            })
            .to_string(),
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//test:corpus"}},
                "completed": {"success": true, "outputGroup": [{"name": "dx_shards", "fileSets": [{"id": "0"}]}]},
            })
            .to_string(),
        ];
        if let Some(reason) = &self.probe_aborted {
            lines.push(
                serde_json::json!({
                    "id": {"targetConfigured": {"label": "//test:broken"}},
                    "aborted": {"reason": reason},
                })
                .to_string(),
            );
        }
        lines.push(
            self.probe_metrics
                .clone()
                .unwrap_or_else(|| LAST_EVENT.to_owned()),
        );
        std::fs::write(bep, lines.join("\n"))?;
        Ok(ChildStatus {
            code: Some(self.probe_code.unwrap_or(0)),
            signal: None,
        })
    }
}

impl Runner for FakeRunner {
    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        self.free_disk_bytes
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
        if is_shard_probe(argv) {
            return self.run_shard_probe(argv);
        }
        let launch = self.build_argv.borrow().len();
        self.build_argv.borrow_mut().push(argv.to_vec());
        self.seen_cwd.borrow_mut().push(cwd.to_path_buf());
        self.seen_env.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        if !self.skip_bep
            && let Some(bep) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        {
            std::fs::write(bep, self.bep_lines.join("\n"))?;
        }
        if let (Some(payload), Some(path)) = (
            &self.patch,
            argv.iter().find_map(|arg| arg.strip_prefix("-patch=")),
        ) {
            std::fs::write(path, payload)?;
        }
        if let Some(path) = env
            .iter()
            .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
        {
            if let Some(witness) = &self.intended {
                std::fs::write(path, witness)?;
            }
            if let Some(code) = self.generate_code {
                return Ok(ChildStatus::exited(code));
            }
        }
        if bazel_subcommand(argv) == Some("run")
            || argv.first().map(String::as_str) != Some(dx_process::launcher_argv0())
        {
            if let Some(signo) = self.app_signal {
                return Ok(ChildStatus::signalled(Some(signo)));
            }
            if let Some(code) = self.app_code {
                return Ok(ChildStatus::exited(code));
            }
        }
        let code = match self.launch_codes.last() {
            Some(last) => Some(*self.launch_codes.get(launch).unwrap_or(last)),
            None => self.code,
        };
        Ok(ChildStatus { code, signal: None })
    }
}

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

pub(crate) fn intended_witness_failed(mode: &str, errors: &str) -> Vec<u8> {
    let errors_value: Vec<serde_json::Value> =
        serde_json::from_str(&format!("[{errors}]")).expect("errors JSON");
    serde_json::to_vec(&serde_json::json!({
        "mode": mode,
        "result": "failed",
        "scopes": [{"value": "//...", "results_complete": false}],
        "errors": errors_value,
    }))
    .expect("witness JSON")
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

pub(crate) fn write_manifest_sources(harness: &Harness, manifest: &str) {
    for path in manifest.lines().filter_map(|line| {
        line.strip_prefix("prod source ")
            .or_else(|| line.strip_prefix("test source "))
    }) {
        let file = harness.workspace.join(path);
        if file.exists() {
            continue;
        }
        std::fs::create_dir_all(file.parent().expect("parent")).expect("source dir");
        std::fs::write(file, "x\n".repeat(5)).expect("source");
    }
}

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
                {"name": "linux-sandbox", "count": executed_raw.saturating_sub(disk).saturating_sub(remote), "execKind": "Local"},
            ],
            "actionCacheStatistics": {"hits": local},
        }},
        "lastMessage": true,
    })
    .to_string()
}

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

#[expect(
    clippy::too_many_arguments,
    reason = "each argument is a distinct input"
)]
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

pub(crate) fn commit_clean_pair(harness: &Harness, env: char, gen_id: char) -> String {
    let pair = SetupPair {
        environment: GenerationId::new(&env.to_string().repeat(64)).expect("environment digest"),
        generated: GenerationId::new(&gen_id.to_string().repeat(64)).expect("generated digest"),
    };
    commit_pair(&harness.workspace, &pair).expect("commit pair");
    for (dir, tag) in [(ENVIRONMENTS_DIR_NAME, env), (GENERATED_DIR_NAME, gen_id)] {
        std::fs::create_dir_all(
            dx_path::state::local_dir(&harness.workspace)
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

pub(crate) fn unused_build_dep(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source(
        "p/BUILD.bazel",
        "rust_library(\n    name = \"unused_dep\",\n    deps = [\n        \":clean\",\n        \":used\",\n    ],\n)\n",
    );
    let mut diagnostic = Harness::diagnostic("unused_crate_dependencies", false);
    diagnostic.path = String::new();
    diagnostic.start_byte = None;
    diagnostic.end_byte = None;
    diagnostic.build_dep = Some(proto::BuildDep {
        package: "p".to_owned(),
        target: "unused_dep".to_owned(),
        dependency: "clean".to_owned(),
        ..Default::default()
    });
    let result = harness.result_full(vec![diagnostic.clone()], vec![diagnostic], vec![], vec![]);
    harness.results.insert("//test:corpus".to_owned(), result);
    harness
}

pub(crate) const RENDERED_E0308: &str =
    "\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror[E0308]\u{1b}[0m\u{1b}[1m: mismatched types\u{1b}[0m
\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m--> \u{1b}[0mapp/src/main.rs:4:26
\u{1b}[0m  \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m
\u{1b}[0m4\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m fn _dx_broken() -> u32 { \"x\" }
\u{1b}[0m \u{1b}[0m\u{1b}[1m\u{1b}[38;5;12m|\u{1b}[0m                    \u{1b}[1m\u{1b}[38;5;9m^^^\u{1b}[0m expected `u32`, found `&str`

\u{1b}[0m\u{1b}[1m\u{1b}[38;5;9merror\u{1b}[0m\u{1b}[1m: aborting due to 1 previous error\u{1b}[0m
";

pub(crate) fn stderr_file(harness: &Harness, name: &str, text: &str) -> String {
    let dir = harness.temp.join("stderr");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("stderr");
    format!("file://{}", path.to_string_lossy())
}

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

#[cfg(unix)]
#[cfg(unix)]
pub(crate) fn reader_gone(writer: &io::PipeWriter, wait: Duration) -> bool {
    dx_os_sys::fd::wait_readable(writer, wait).expect("poll")
}

#[cfg(unix)]
pub(crate) fn hang_up(reader: io::PipeReader, writer: &io::PipeWriter) {
    drop(reader);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !reader_gone(writer, Duration::from_secs(1)) {
        assert!(Instant::now() < deadline, "pipe reader still held");
    }
}

#[cfg(unix)]
pub(crate) fn closed_pipe() -> io::PipeWriter {
    let (reader, writer) = io::pipe().expect("pipe");
    hang_up(reader, &writer);
    writer
}

#[cfg(windows)]
pub(crate) fn closed_pipe() -> io::PipeWriter {
    let (reader, writer) = io::pipe().expect("pipe");
    drop(reader);
    writer
}

pub(crate) fn quiet_proc(harness: &Harness) -> PathBuf {
    let proc_root = harness.workspace.join("quiet-proc");
    std::fs::create_dir_all(&proc_root).expect("empty process table");
    proc_root
}

pub(crate) const FORMAT_ASPECT: &str =
    r#"@@rules_dx+//core/quality:families/core.bzl%real_format_aspect[unsafe_fixes="false"]"#;
pub(crate) const LINT_ASPECT: &str =
    r#"@@rules_dx+//core/quality:families/core.bzl%real_lint_aspect[unsafe_fixes="false"]"#;
pub(crate) const TYPECHECK_ASPECT: &str =
    r#"@@rules_dx+//core/quality:families/core.bzl%real_typecheck_aspect[unsafe_fixes="false"]"#;

pub(crate) struct AspectRunner<R> {
    pub(crate) inner: R,
    pub(crate) events: Vec<(Option<&'static str>, String)>,
    pub(crate) builds: RefCell<Vec<Vec<String>>>,
}

impl<R: Runner> Runner for AspectRunner<R> {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        let aspects = argv.iter().find_map(|arg| arg.strip_prefix("--aspects="));
        let Some(aspects) = aspects.filter(|_| !is_shard_probe(argv)) else {
            return self.inner.run(argv, cwd, env);
        };
        self.builds.borrow_mut().push(argv.to_vec());
        let passed: BTreeSet<&str> = aspects.split(',').map(dx_bep::aspect_key).collect();
        let scope = build_labels(argv);
        let built = |line: &str| {
            let event: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
            let label = ["targetCompleted", "actionCompleted"]
                .iter()
                .find_map(|kind| event["id"][kind]["label"].as_str());
            scope.is_empty()
                || label.is_none_or(|label| {
                    scope.iter().any(|arg| {
                        dx_bep::target_key(arg)
                            .is_none_or(|key| Some(key) == dx_bep::target_key(label))
                    })
                })
        };
        let mut lines = Vec::new();
        let mut failed = false;
        for (owner, line) in &self.events {
            if owner.is_none_or(|owner| passed.contains(dx_bep::aspect_key(owner))) && built(line) {
                failed |= line.contains(r#""success":false"#);
                lines.push(line.clone());
            }
        }
        lines.push(LAST_EVENT.to_owned());
        if let Some(bep) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        {
            std::fs::write(bep, lines.join("\n"))?;
        }
        Ok(ChildStatus::exited(i32::from(failed)))
    }
}

pub(crate) fn build_labels(argv: &[String]) -> Vec<String> {
    argv.iter()
        .filter(|arg| !arg.starts_with('-') && arg.contains("//"))
        .cloned()
        .collect()
}

pub(crate) fn aspect_events(
    harness: &Harness,
    aspect: &'static str,
    finding: &str,
    success: bool,
) -> Vec<(Option<&'static str>, String)> {
    target_aspect_events(harness, "//test:corpus", aspect, finding, success)
}

pub(crate) fn target_aspect_events(
    harness: &Harness,
    label: &str,
    aspect: &'static str,
    finding: &str,
    success: bool,
) -> Vec<(Option<&'static str>, String)> {
    let tool = dx_bep::aspect_key(aspect)
        .rsplit('%')
        .next()
        .expect("aspect name");
    let slug: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let name = format!("{tool}{slug}");
    let name = name.as_str();
    let result = if finding.is_empty() {
        harness.valid_result(vec![], vec![])
    } else {
        harness.valid_result(vec![Harness::diagnostic(finding, false)], vec![])
    };
    let result_uri = write_bep_artifact(harness, &format!("{name}.pb"), &result);
    let note = format!("{tool}-tool: skipped, not pinned for this platform\n");
    let skip_uri = write_bep_artifact(harness, &format!("{name}.skip"), note.as_bytes());
    let set = |id: String, uri: &str| {
        serde_json::json!({"id": {"namedSet": {"id": id}}, "namedSetOfFiles": {"files": [{"uri": uri}]}})
            .to_string()
    };
    let completed = serde_json::json!({
        "id": {"targetCompleted": {"label": label, "aspect": aspect}},
        "completed": {"success": success, "outputGroup": [
            {"name": "dx_results", "fileSets": [{"id": format!("{name}-r")}]},
            {"name": "dx_skips", "fileSets": [{"id": format!("{name}-s")}]},
        ]},
    })
    .to_string();
    let mut events = vec![
        set(format!("{name}-r"), &result_uri),
        set(format!("{name}-s"), &skip_uri),
        completed,
    ];
    if !success {
        let stderr = stderr_file(harness, name, "error: the tool failed\n");
        events.push(failed_action(
            label,
            "c1",
            "Lint",
            "out/corpus.lint",
            &stderr,
        ));
    }
    events
        .into_iter()
        .map(|line| (Some(aspect), line))
        .collect()
}

pub(crate) fn failed_target(label: &str, aspect: &str) -> String {
    serde_json::json!({
        "id": {"targetCompleted": {"label": label, "aspect": aspect}},
        "completed": {"success": false},
    })
    .to_string()
}

#[cfg(test)]
mod scope_exclude_tests {
    use super::*;

    const DX_TOML: &str = "schema_version = 1\n[scope]\nexclude = [\"//third_party/...\"]\n";

    fn excluding_harness(name: &str) -> Harness {
        let mut harness = Harness::new(name);
        harness.write_source("dx.toml", DX_TOML);
        harness.write_source("src/a.py", "x = 1\n");
        harness.results.insert(
            "//test:corpus".to_owned(),
            harness.valid_result(vec![], vec![]),
        );
        harness
    }

    fn labels_of(argv: &[String]) -> Vec<String> {
        let start = argv.iter().position(|arg| arg == "--").expect("separator") + 1;
        argv[start..].to_vec()
    }

    #[test]
    fn every_family_leaves_excluded_packages_out_of_bazel() {
        for command in ["build", "test", "coverage", "lint", "format"] {
            let harness = excluding_harness(&format!("exclude-{command}"));
            let (_, _, err) = harness.run(&[command, "--output=text"]);
            let builds = harness.build_argv.borrow();
            let argv = builds.last().unwrap_or_else(|| panic!("{command}: {err}"));
            assert_eq!(
                labels_of(argv),
                ["//...", "-//third_party/..."],
                "{command}: {argv:?}"
            );
            assert!(
                !argv
                    .iter()
                    .any(|arg| arg.starts_with("--") && arg.contains("third_party")),
                "{command}: no Bazel flag names the exclude: {argv:?}"
            );
        }
    }

    #[test]
    fn a_named_label_under_an_exclude_runs_with_a_notice() {
        let harness = excluding_harness("exclude-named");
        let (_, _, err) = harness.run(&["build", "//third_party/x:y", "--output=text"]);
        let builds = harness.build_argv.borrow();
        let argv = builds.last().expect("build ran");
        assert_eq!(argv.last().map(String::as_str), Some("//third_party/x:y"));
        assert!(!argv.contains(&"--".to_owned()), "{argv:?}");
        assert!(
            err.contains("dx: scope_excluded: //third_party/x:y"),
            "{err}"
        );
    }

    #[test]
    fn a_bad_exclude_fails_before_bazel_runs() {
        let harness = Harness::new("exclude-bad");
        harness.write_source(
            "dx.toml",
            "schema_version = 1\n[scope]\nexclude = [\"oops\"]\n",
        );
        let (code, _, err) = harness.run(&["build", "--output=text"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("invalid dx.toml"), "{err}");
        assert!(harness.build_argv.borrow().is_empty());
    }
}
