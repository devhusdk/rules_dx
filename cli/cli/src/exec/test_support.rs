use super::{execute, Env};
use crate::args::Invocation;
use crate::plan::GENERATE_ENV_INTENDED;
use crate::resolve::{QueryResult, QueryRunner};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dx_digest::blake3 as digest;
use dx_process::{ChildStatus, Runner};
use dx_setup::{
    commit_pair, setup_hex, GenerationId, SetupPair, ENVIRONMENTS_DIR_NAME, GENERATED_DIR_NAME,
};
use quality_result::proto;
use quality_result::proto::{Capability, Convergence, FileSnapshot, QualityResult, Stage};
use quality_result::{encode_validated, SCHEMA_MAJOR, SCHEMA_MINOR};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

pub(crate) use crate::args::parsed as invocation;
pub(crate) use crate::test_support::{event, event_kinds, events_of_kind, json_events};

pub(crate) fn temp_dir(prefix: &str) -> tempfile::TempDir {
    dx_test_scratch::scratch(&format!("dx-exec-test-{prefix}-"))
}

pub(crate) struct Harness {
    pub(crate) workspace: PathBuf,
    pub(crate) temp: PathBuf,
    pub(crate) cwd: PathBuf,
    pub(crate) _workspace_guard: tempfile::TempDir,
    pub(crate) _temp_guard: tempfile::TempDir,
    pub(crate) results: HashMap<String, Vec<u8>>,
    pub(crate) bazel_code: i32,
    pub(crate) fail_target: bool,
    pub(crate) io_error: bool,
    pub(crate) signalled: bool,
    pub(crate) skip_bep: bool,
    pub(crate) raw_bep: Option<Vec<String>>,
    pub(crate) query: ScriptQuery,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
}

pub(crate) struct ScriptQuery {
    pub(crate) calls: RefCell<Vec<Vec<String>>>,
    pub(crate) outputs: RefCell<Vec<QueryResult>>,
    pub(crate) info_calls: RefCell<Vec<Vec<String>>>,
    pub(crate) info: RefCell<Option<QueryResult>>,
}

impl ScriptQuery {
    pub(crate) fn script_owners(&self, owners: &str) {
        self.outputs.borrow_mut().push(QueryResult {
            code: Some(0),
            stdout: owners.as_bytes().to_vec(),
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
        self.calls.borrow_mut().push(argv.to_vec());
        Ok(self.outputs.borrow_mut().remove(0))
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
            fail_target: false,
            io_error: false,
            signalled: false,
            skip_bep: false,
            raw_bep: None,
            query: ScriptQuery {
                calls: RefCell::new(Vec::new()),
                outputs: RefCell::new(Vec::new()),
                info_calls: RefCell::new(Vec::new()),
                info: RefCell::new(None),
            },
            intended: None,
            seen_env: Rc::new(RefCell::new(Vec::new())),
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
        }
    }

    pub(crate) fn replacement(&self, replacement: &[u8]) -> proto::FileEdits {
        let original = std::fs::read(self.workspace.join("src/a.py")).expect("source");
        self.replacement_at(
            "src/a.py",
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

    pub(crate) fn runner(&self) -> FakeRunner {
        if let Some(lines) = &self.raw_bep {
            return FakeRunner {
                code: Some(self.bazel_code),
                bep_lines: lines.clone(),
                io_error: self.io_error,
                skip_bep: self.skip_bep,
                intended: self.intended.clone(),
                seen_env: Rc::clone(&self.seen_env),
            };
        }
        let staged = self.staged_artifacts();
        for (artifact, bytes) in &staged {
            std::fs::write(artifact, bytes).expect("artifact");
        }
        let files: Vec<serde_json::Value> = staged
            .iter()
            .map(|(artifact, _)| {
                serde_json::json!({"uri": format!("file://{}", artifact.to_string_lossy())})
            })
            .collect();
        let mut lines = Vec::new();
        lines.push(
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": files},
            })
            .to_string(),
        );
        let success = !self.fail_target;
        lines.push(
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//test:corpus"}},
                "completed": {
                    "success": success,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                },
            })
            .to_string(),
        );
        FakeRunner {
            code: if self.signalled {
                None
            } else {
                Some(self.bazel_code)
            },
            bep_lines: lines,
            io_error: self.io_error,
            skip_bep: self.skip_bep,
            intended: self.intended.clone(),
            seen_env: Rc::clone(&self.seen_env),
        }
    }

    pub(crate) fn run(&self, words: &[&str]) -> (i32, String, String) {
        self.run_with_ci(words, false)
    }

    pub(crate) fn run_with_ci(&self, words: &[&str], ci: bool) -> (i32, String, String) {
        let inv = match crate::args::parse(&crate::test_support::strings(words)) {
            Ok(inv) => inv,
            Err(error) => {
                return (
                    2,
                    String::new(),
                    format!("dx: {error}\n{}\n", crate::args::help::usage_banner()),
                );
            }
        };
        let inv = match crate::args::apply_here(&inv, &self.workspace, &self.cwd) {
            Ok(resolved) => resolved,
            Err(detail) => return (2, String::new(), format!("dx: {detail}\n")),
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
        let probe = ArgvProbe {
            codes: codes.to_vec(),
            seen: Rc::new(RefCell::new(Vec::new())),
            seen_env: Rc::clone(&self.seen_env),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(inv, self.env(&probe, &mut out, &mut err));
        let argv = probe.seen.borrow().clone();
        ProbeRun {
            code,
            out: String::from_utf8(out).expect("stdout"),
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
struct ArgvProbe {
    codes: Vec<Option<i32>>,
    seen: Rc<RefCell<Vec<Vec<String>>>>,
    seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
}

impl Runner for ArgvProbe {
    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
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
        })
    }
}

pub(crate) struct FakeRunner {
    pub(crate) code: Option<i32>,
    pub(crate) bep_lines: Vec<String>,
    pub(crate) io_error: bool,
    pub(crate) skip_bep: bool,
    pub(crate) intended: Option<Vec<u8>>,
    pub(crate) seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
}

impl Runner for FakeRunner {
    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
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
        if let Some(path) = env
            .iter()
            .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
        {
            if let Some(witness) = &self.intended {
                std::fs::write(path, witness).expect("intended witness");
            }
        }
        Ok(ChildStatus { code: self.code })
    }
}

pub(crate) fn intended_witness(mode: &str, complete: bool, files: &str, ignored: &str) -> Vec<u8> {
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
        "schema_minor": 0,
        "mode": mode,
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

pub(crate) fn test_summary_line(label: &str, shard_count: u32) -> String {
    serde_json::json!({
        "id": {"testSummary": {"label": label}},
        "testSummary": {"status": "PASSED", "shardCount": shard_count},
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
