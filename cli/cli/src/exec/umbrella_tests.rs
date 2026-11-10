use super::super::test_support::*;
use super::*;
use dx_plan::args::Invocation;
use dx_plan::plan::GENERATE_ENV_INTENDED;
use dx_process::{ChildStatus, Runner};
use std::cell::{Cell, RefCell};
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

fn setting_args(argv: &[String]) -> std::collections::BTreeSet<String> {
    argv.iter()
        .filter(|arg| arg.starts_with("--@"))
        .cloned()
        .collect()
}

#[test]
fn umbrella_phases_and_test_share_the_bazel_settings() {
    let resolved = dx_plan::resolve::ResolvedScope {
        scope: dx_process::Scope::Repository,
        targets: Vec::new(),
        excluded: Vec::new(),
        kept: Vec::new(),
    };
    let shared: std::collections::BTreeSet<String> = dx_plan::plan::BAZEL_SETTINGS
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut argvs = Vec::new();
    for phase in UMBRELLA_PHASES {
        let plan = if phase == Command::Generate {
            dx_plan::plan::plan_generate(&resolved, &[], true, &[], Some("/tmp/dx.patch"))
        } else {
            dx_plan::plan::plan_build(phase, &resolved, &[], "/tmp/bep.json", &[])
        };
        argvs.push((format!("{phase:?}"), plan.expect("phase plan").argv));
        let probe = dx_plan::plan::plan_probe(phase, &resolved, &[], "/tmp/probe.json", &[], None)
            .expect("probe plan");
        if let Some(probe) = probe {
            argvs.push((format!("{phase:?} probe"), probe.argv));
        }
    }
    let docs = dx_plan::plan::plan_docs(&resolved, &[], "/tmp/bep.json", &[], None).expect("docs plan");
    argvs.push(("docs".to_owned(), docs.argv));
    let test = dx_plan::plan::plan_workflow(
        dx_plan::plan::WorkflowVerb::Test,
        &resolved,
        &[],
        Some("/tmp/bep.json"),
        Some(dx_plan::args::BuildMode::Dev),
        &[],
    )
    .expect("test plan");
    argvs.push(("test".to_owned(), test.argv));
    let build_mode = dx_plan::args::BuildMode::Dev;
    let label = "//app:bin".to_owned();
    let planned = [
        (
            "run",
            dx_plan::plan::plan_run(&label, &[], &[], build_mode, &[]),
        ),
        (
            "run build",
            dx_plan::plan::plan_run_build(std::slice::from_ref(&label), &[], build_mode, &[]),
        ),
        (
            "deploy build",
            dx_plan::plan::plan_deploy_build(&label, build_mode, &[]),
        ),
        (
            "deploy run",
            dx_plan::plan::plan_deploy_run(&label, &[], build_mode, &[]),
        ),
    ];
    for (name, plan) in planned {
        argvs.push((name.to_owned(), plan.expect("plan").argv));
    }
    let plan = dx_plan::plan::plan_managed(
        Command::Sync,
        &dx_setup::SetupScope::Repository,
        &[],
        "/tmp/bep.json",
        &[],
    )
    .expect("managed plan");
    argvs.push(("sync".to_owned(), plan.argv));
    let selection = dx_plan::resolve::SelectionContext::for_test_execution(
        dx_plan::args::BuildMode::Dev,
        &[],
        &[],
    );
    let cquery = selection
        .selection_argv("cquery", &[], "tests(//...)")
        .expect("cquery argv");
    argvs.push(("cquery".to_owned(), cquery));
    for (name, argv) in &argvs {
        assert_eq!(setting_args(argv), shared, "{name}: {argv:?}");
    }
    let query = selection
        .selection_argv("query", &[], "tests(//...)")
        .expect("query argv");
    assert!(setting_args(&query).is_empty(), "{query:?}");
}

/// Names the capture one umbrella phase writes into the harness temp dir.
fn capture_path(harness: &Harness, phase: &str, index: u64) -> PathBuf {
    harness.temp.join(format!(
        "umbrella-{phase}-{}-{index}.sarif",
        std::process::id()
    ))
}

/// Parses one umbrella invocation against a harness workspace.
fn umbrella_invocation(harness: &Harness, words: &[&str]) -> Invocation {
    let parsed = invocation(words);
    dx_plan::args::apply_here(&parsed, &harness.workspace, &harness.cwd).expect("resolve workspace")
}

/// How one phase capture is rewritten once its phase has finished.
#[derive(Clone, Copy)]
enum Rewrite {
    Remove,
    Block,
    Bytes(&'static [u8]),
}

impl Rewrite {
    fn apply(self, path: &Path) {
        match self {
            Rewrite::Remove => {
                std::fs::remove_file(path).expect("remove capture");
            }
            Rewrite::Block => {
                std::fs::remove_file(path).expect("remove capture");
                std::fs::create_dir(path).expect("capture directory");
            }
            Rewrite::Bytes(payload) => {
                std::fs::write(path, payload).expect("rewrite capture");
            }
        }
    }
}

/// Serves the inner runner but exits nonzero from one launch up to an optional end.
struct FailFrom<R> {
    inner: R,
    from: usize,
    to: Option<usize>,
    code: i32,
    launches: Cell<usize>,
}

impl<R: Runner> Runner for FailFrom<R> {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if is_shard_probe(argv) {
            return self.inner.run(argv, cwd, env);
        }
        let launch = self.launches.get();
        self.launches.set(launch + 1);
        let status = self.inner.run(argv, cwd, env)?;
        if launch >= self.from && self.to.is_none_or(|to| launch < to) {
            return Ok(ChildStatus::exited(self.code));
        }
        Ok(status)
    }
}

/// Serves the inner runner but makes one launch a failed compile with exit 1.
struct CompileFailAt<R> {
    inner: R,
    at: usize,
    bep_line: String,
    launches: Cell<usize>,
}

impl<R: Runner> Runner for CompileFailAt<R> {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if is_shard_probe(argv) {
            return self.inner.run(argv, cwd, env);
        }
        let launch = self.launches.get();
        self.launches.set(launch + 1);
        let status = self.inner.run(argv, cwd, env)?;
        if launch != self.at {
            return Ok(status);
        }
        if let Some(bep) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        {
            let rest = std::fs::read_to_string(bep).unwrap_or_default();
            std::fs::write(bep, format!("{}\n{rest}", self.bep_line)).expect("BEP file");
        }
        Ok(ChildStatus::exited(1))
    }
}

fn compile_fail_at_lint(harness: &Harness) -> CompileFailAt<FakeRunner> {
    let uri = stderr_file(harness, "lint-clippy", RENDERED_E0308);
    CompileFailAt {
        inner: harness.runner(),
        at: 2,
        bep_line: failed_action("//app:bin", "c1", "Clippy", "out/bin.clippy", &uri),
        launches: Cell::new(0),
    }
}

/// Rewrites one phase capture once the phase that wrote it has finished.
struct RewriteCapture<R> {
    inner: R,
    capture: PathBuf,
    rewrite: Rewrite,
    done: Cell<bool>,
}

impl<R: Runner> Runner for RewriteCapture<R> {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        let status = self.inner.run(argv, cwd, env)?;
        if !self.done.get() && self.capture.exists() {
            self.done.set(true);
            self.rewrite.apply(&self.capture);
        }
        Ok(status)
    }
}

/// Runs one umbrella command whose lint capture is rewritten mid-run.
fn umbrella_with_broken_capture(
    harness: Harness,
    rewrite: Rewrite,
    words: &[&str],
) -> (Harness, i32, String, String) {
    let capture = capture_path(&harness, "lint", 2);
    let runner = RewriteCapture {
        inner: harness.runner(),
        capture,
        rewrite,
        done: Cell::new(false),
    };
    let inv = umbrella_invocation(&harness, words);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    (harness, code, out, err)
}

/// Serves apply content for the first `verify_from` launches, then
/// verification content, so `check --apply` tests can stage what the
/// read-only re-check observes after the apply pass wrote sources.
/// Quality launches from the `clean_from`th quality launch on serve
/// `clean_files` instead, so a later phase does not re-offer an edit an
/// earlier phase already wrote. Generate launches never count toward it.
struct StagedRunner {
    code: Option<i32>,
    bep_lines: Vec<String>,
    apply_files: Vec<(PathBuf, Vec<u8>)>,
    clean_files: Vec<(PathBuf, Vec<u8>)>,
    clean_from: usize,
    quality_launches: Cell<usize>,
    verify_files: Vec<(PathBuf, Vec<u8>)>,
    apply_intended: Option<Vec<u8>>,
    verify_intended: Option<Vec<u8>>,
    verify_from: usize,
    launches: Cell<usize>,
    #[allow(clippy::type_complexity)]
    seen_env: Rc<RefCell<Vec<Vec<(String, String)>>>>,
}

impl Runner for StagedRunner {
    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        if is_shard_probe(argv) {
            return Ok(ChildStatus {
                code: Some(1),
                signal: None,
            });
        }
        let launch = self.launches.get();
        self.launches.set(launch + 1);
        let generate = env.iter().any(|(key, _)| *key == GENERATE_ENV_INTENDED);
        let quality_launch = self.quality_launches.get();
        if !generate {
            self.quality_launches.set(quality_launch + 1);
        }
        let (files, intended) = if launch >= self.verify_from {
            (&self.verify_files, &self.verify_intended)
        } else if !generate && quality_launch >= self.clean_from {
            (&self.clean_files, &self.apply_intended)
        } else {
            (&self.apply_files, &self.apply_intended)
        };
        for (path, bytes) in files {
            std::fs::write(path, bytes).expect("stage artifact");
        }
        if let Some(path) = env
            .iter()
            .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
        {
            if let Some(witness) = intended {
                std::fs::write(path, witness).expect("stage witness");
            }
        }
        self.seen_env.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        if let Some(bep) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        {
            std::fs::write(bep, self.bep_lines.join("\n")).expect("BEP file");
        }
        Ok(ChildStatus {
            code: self.code,
            signal: None,
        })
    }
}

/// Builds one runner serving `harness` content for the apply pass and
/// `verify_files`/`verify_intended` from launch `verify_from` on.
fn staged_runner(
    harness: &Harness,
    verify_files: Vec<(PathBuf, Vec<u8>)>,
    verify_intended: Option<Vec<u8>>,
    verify_from: usize,
) -> StagedRunner {
    let staged = harness.staged_artifacts();
    let files: Vec<serde_json::Value> = staged
        .iter()
        .map(|(artifact, _)| {
            serde_json::json!({"uri": format!("file://{}", artifact.to_string_lossy())})
        })
        .collect();
    StagedRunner {
        code: Some(0),
        bep_lines: vec![
            serde_json::json!({
                "id": {"namedSet": {"id": "0"}},
                "namedSetOfFiles": {"files": files},
            })
            .to_string(),
            serde_json::json!({
                "id": {"targetCompleted": {"label": "//test:corpus"}},
                "completed": {
                    "success": true,
                    "outputGroup": [{"name": "dx_results", "fileSets": [{"id": "0"}]}],
                },
            })
            .to_string(),
            LAST_EVENT.to_owned(),
        ],
        apply_files: staged,
        clean_files: Vec::new(),
        clean_from: verify_from,
        quality_launches: Cell::new(0),
        verify_files,
        apply_intended: harness.intended.clone(),
        verify_intended,
        verify_from,
        launches: Cell::new(0),
        seen_env: Rc::clone(&harness.seen_env),
    }
}

/// Runs one `check --apply` whose launches from `verify_from` observe
/// `verify_files` and `verify_intended` instead of the apply content.
fn fix_apply_with_staged_verify(
    harness: &Harness,
    verify_files: Vec<(PathBuf, Vec<u8>)>,
    verify_intended: Option<Vec<u8>>,
    verify_from: usize,
    words: &[&str],
) -> (i32, String, String) {
    let runner = staged_runner(harness, verify_files, verify_intended, verify_from);
    let inv = umbrella_invocation(harness, words);
    harness.execute_with(&inv, &runner)
}

/// Like `fix_apply_with_staged_verify`, but only format (the first
/// quality launch) sees the harness edit; lint and typecheck see a clean
/// result, as real tools would once format wrote the file.
fn fix_apply_format_edit_then_verify(
    harness: &Harness,
    verify_files: Vec<(PathBuf, Vec<u8>)>,
    verify_intended: Option<Vec<u8>>,
    verify_from: usize,
    words: &[&str],
) -> (i32, String, String) {
    let mut runner = staged_runner(harness, verify_files, verify_intended, verify_from);
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    runner.clean_files = runner
        .apply_files
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    runner.clean_from = 1;
    let inv = umbrella_invocation(harness, words);
    harness.execute_with(&inv, &runner)
}

/// Reads one merged SARIF document out of a harness workspace.
fn merged_runs(harness: &Harness) -> Vec<serde_json::Value> {
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("SARIF JSON");
    assert_eq!(document["version"], serde_json::json!("2.1.0"));
    assert_eq!(
        document["$schema"],
        serde_json::json!("https://json.schemastore.org/sarif-2.1.0.json")
    );
    document["runs"].as_array().expect("runs").to_vec()
}

#[test]
fn check_rejects_build_output_and_names_the_file() {
    let harness = umbrella_clean("umbrella-check-build-output");
    let git = |args: &[&str]| {
        dx_process::capture::run_checked(
            dx_process::capture::git(&harness.workspace).args(args),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    };
    git(&["init", "-q"]);
    harness.write_source("cli/target/debug/dx", "binary");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("cli/target/debug/dx"), "{out}{err}");
    assert!(!out.contains("Running format"), "{out}");
    assert_eq!(harness.seen_env.borrow().len(), 0);
}

fn dirty_build_output(name: &str) -> Harness {
    let harness = umbrella_clean(name);
    dx_process::capture::run_checked(
        dx_process::capture::git(&harness.workspace).args(["init", "-q"]),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    harness.write_source("cli/target/debug/dx", "binary");
    harness
}

#[test]
fn lint_check_rejects_build_output_without_launching_bazel() {
    let harness = dirty_build_output("umbrella-lint-build-output");
    let (code, out, err) = harness.run(&["lint", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("cli/target/debug/dx"), "{out}{err}");
    assert_eq!(harness.seen_env.borrow().len(), 0);
}

#[test]
fn lint_apply_is_not_refused_for_build_output() {
    let harness = dirty_build_output("umbrella-lint-apply-build-output");
    let (_, out, err) = harness.run(&["lint", "--apply", "--output=text"]);
    assert!(!err.contains("build output in change"), "{out}{err}");
    assert!(!out.contains("build output in change"), "{out}{err}");
}

#[test]
fn check_and_fix_json_report_build_output_before_finishing() {
    {
        let command = "check";
        let harness = dirty_build_output(&format!("umbrella-{command}-json-build-output"));
        let (code, out, err) = harness.run(&[command, "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let error = events
            .iter()
            .position(|event| event["code"] == "build_output_in_change")
            .unwrap_or_else(|| panic!("{out}"));
        let last = events.len() - 1;
        assert!(error < last, "{out}");
        assert_eq!(events[last]["event"], "command_finished");
        assert_eq!(events[last]["exit_code"], 1);
        assert_eq!(harness.seen_env.borrow().len(), 0);
    }
}

fn overrides_consumer(name: &str) -> Harness {
    let harness = umbrella_clean(name);
    harness.write_source(
        "MODULE.bazel",
        &format!(
            "module(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"{}\")\n",
            dx_adopt::DX_VERSION
        ),
    );
    harness
}

#[test]
fn check_fails_on_stale_overrides_without_launching_bazel() {
    let harness = overrides_consumer("umbrella-check-overrides-stale");
    dx_adopt::apply_overrides(&harness.workspace).expect("applies");
    harness.write_source(
        "third_party/rules_dx/rules_rust.clippy_metadata_inputs.patch",
        "corrupt\n",
    );
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        json_events(&out)
            .iter()
            .any(|event| event["code"] == "overrides_stale"),
        "{out}"
    );
    assert_eq!(harness.seen_env.borrow().len(), 0);
}

#[test]
fn check_notes_unadopted_overrides_and_continues() {
    let harness = overrides_consumer("umbrella-check-overrides-warn");
    let (_, out, err) = harness.run(&["check", "--output=text"]);
    assert!(err.contains("dx: overrides: "), "{out}{err}");
    assert!(!out.contains("overrides_stale"), "{out}{err}");
    assert!(!err.contains("overrides_stale"), "{out}{err}");
}

#[test]
fn check_is_silent_about_overrides_without_a_rules_dx_dep() {
    let harness = umbrella_clean("umbrella-check-overrides-na");
    let (_, out, err) = harness.run(&["check", "--output=text"]);
    assert!(!err.contains("overrides"), "{out}{err}");
}

#[test]
fn check_ignores_source_named_target_committed_on_main() {
    let harness = umbrella_clean("umbrella-check-source-target");
    let git = |args: &[&str]| {
        dx_process::capture::run_checked(
            dx_process::capture::git(&harness.workspace)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    };
    git(&["init", "-q", "-b", "main"]);
    harness.write_source("src/x/target/Foo.java", "class Foo {}");
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "init"]);
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running format analysis"), "{out}");
}

#[test]
fn check_reports_a_git_failure_from_the_build_output_check() {
    let harness = umbrella_clean("umbrella-check-git-failed");
    let git = |args: &[&str]| {
        dx_process::capture::run_checked(
            dx_process::capture::git(&harness.workspace).args(args),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("{error}"));
    };
    git(&["init", "-q"]);
    std::fs::write(harness.workspace.join(".git/index"), b"garbage").expect("index");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("build-output check failed"), "{out}{err}");
    assert_eq!(harness.seen_env.borrow().len(), 0);
}

#[test]
fn check_clean_runs_every_phase_in_order() {
    let harness = umbrella_clean("umbrella-check-clean");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 0, "{out}{err}");
    let format = out
        .find("Running format analysis for //...")
        .expect("format");
    let lint = out.find("Running lint analysis for //...").expect("lint");
    let typecheck = out
        .find("Running typecheck analysis for //...")
        .expect("typecheck");
    let generate = out.find("Running generate for //...").expect("generate");
    assert!(
        generate < format && format < lint && lint < typecheck,
        "{out}"
    );
    assert_eq!(err, "no findings in 0 target(s)\n".repeat(3), "{err}");
    assert_eq!(
        harness.seen_env.borrow().len(),
        4,
        "one Bazel launch per phase"
    );
}

#[test]
fn check_json_sums_action_counts_across_phases() {
    let mut harness = umbrella_clean("umbrella-check-actions");
    harness.build_metrics = Some(build_metrics_line(10, 9, 2, 3, 5));
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let phases: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["event"] == "phase_finished" && event.get("actions").is_some())
        .collect();
    assert_eq!(phases.len(), 3, "{out}");
    for phase in &phases {
        assert_eq!(
            phase["actions"],
            serde_json::json!({"created": 10, "executed": 4, "cache_hits": {"disk": 2, "remote": 3, "local": 5}}),
            "{phase}"
        );
    }
    assert_eq!(
        finished_actions(&out),
        Some(
            serde_json::json!({"created": 30, "executed": 12, "cache_hits": {"disk": 6, "remote": 9, "local": 15}})
        )
    );
}

#[test]
fn check_json_brackets_phase_lifecycles() {
    let harness = umbrella_clean("umbrella-check-json");
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert_eq!(events.first().expect("first")["event"], "command_started");
    assert_eq!(events.first().expect("first")["command"], "check");
    assert_eq!(events.last().expect("last")["event"], "command_finished");
    assert_eq!(events.last().expect("last")["exit_code"], 0);
    assert_eq!(events_of_kind(&events, "command_started").len(), 1);
    assert_eq!(events_of_kind(&events, "command_finished").len(), 1);
    let boundaries: Vec<(&str, &str)> = events
        .iter()
        .filter(|event| event["event"] == "phase_started" || event["event"] == "phase_finished")
        .map(|event| {
            (
                event["event"].as_str().expect("event"),
                event["phase"].as_str().expect("phase"),
            )
        })
        .collect();
    assert_eq!(
        boundaries,
        vec![
            ("phase_started", "generate"),
            ("phase_finished", "generate"),
            ("phase_started", "format"),
            ("phase_finished", "format"),
            ("phase_started", "lint"),
            ("phase_finished", "lint"),
            ("phase_started", "typecheck"),
            ("phase_finished", "typecheck"),
        ]
    );
    for event in &events[1..events.len() - 1] {
        assert_eq!(event["command"], "check", "{event}");
        let phase = event["phase"].as_str().expect("phase");
        assert_eq!(
            event["correlation"],
            serde_json::json!(format!("check/{phase}")),
            "{event}"
        );
        assert_eq!(event["schema"]["major"], 4, "{event}");
    }
}

fn generate_failed_harness(name: &str) -> Harness {
    let mut harness = umbrella_findings(name);
    harness.bazel_code = 1;
    harness.intended = Some(intended_witness_failed(
        "check",
        r#"{"language":"cc","message":"boom"}"#,
    ));
    harness
}

#[test]
fn check_stops_after_generate_failure() {
    let harness = generate_failed_harness("umbrella-generate-failed");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(
            "dx: phases: generate failed, format skipped, lint skipped, typecheck skipped"
        ),
        "{err}"
    );
    assert_eq!(harness.seen_env.borrow().len(), 1, "only generate launches");
}

#[test]
fn check_json_marks_phases_skipped_after_generate_failure() {
    let harness = generate_failed_harness("umbrella-generate-failed-json");
    let (code, out, _) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let skipped = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_skipped")
        .count();
    assert_eq!(skipped, 3, "{out}");
}

#[test]
fn check_runs_every_phase_after_ordinary_findings() {
    let harness = umbrella_findings("umbrella-continue");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("Running format analysis for //..."), "{out}");
    assert!(out.contains("Running lint analysis for //..."), "{out}");
    assert!(
        out.contains("Running typecheck analysis for //..."),
        "{out}"
    );
    assert!(out.contains("Running generate for //..."), "{out}");
    assert!(
        err.contains(
            "dx: phases: generate failed, format findings, lint findings, typecheck findings"
        ),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n",
        "check mode never mutates"
    );
    assert_eq!(harness.seen_env.borrow().len(), 4, "every phase launches");
}

#[test]
fn check_json_reports_every_phase_after_findings() {
    let harness = umbrella_findings("umbrella-continue-json");
    let (code, out, _) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    assert_eq!(events.first().expect("first")["command"], "check");
    assert_eq!(events.last().expect("last")["event"], "command_finished");
    assert_eq!(events.last().expect("last")["exit_code"], 1);
    assert_eq!(events_of_kind(&events, "command_started").len(), 1);
    assert_eq!(events_of_kind(&events, "command_finished").len(), 1);
    let started: Vec<&str> = events_of_kind(&events, "phase_started")
        .iter()
        .map(|event| event["phase"].as_str().expect("phase"))
        .collect();
    assert_eq!(started, vec!["generate", "format", "lint", "typecheck"]);
    let format_finished = events_of_kind(&events, "phase_finished")
        .into_iter()
        .find(|event| event["phase"] == "format")
        .expect("format finished");
    assert_eq!(format_finished["exit_code"], 1);
    assert_eq!(format_finished["correlation"], "check/format");
    let findings: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_findings")
        .map(|notice| notice["correlation"].as_str().expect("correlation"))
        .collect();
    assert_eq!(
        findings,
        vec!["check/format", "check/lint", "check/typecheck"]
    );
}

#[test]
fn check_without_apply_stays_check_mode() {
    let harness = umbrella_findings("umbrella-fix-check");
    let (code, out, _) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("Running format analysis for //..."), "{out}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n",
        "forced check mode never mutates"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        4,
        "every phase runs after findings"
    );
}

#[test]
fn fix_applies_generate_mutation_after_clean_quality() {
    let mut harness = Harness::new("umbrella-fix");
    harness.write_source("src/a.py", "x = 1\n");
    harness.write_source("rust/tests/fixtures/hello/BUILD.bazel", "xyz\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![]),
    );
    harness.intended = Some(intended_witness(
        "default",
        true,
        &intended_modify("rust/tests/fixtures/hello/BUILD.bazel", b"abc\n", b"xyz\n"),
        "",
    ));
    let staged = harness.staged_artifacts();
    let (code, out, err) = fix_apply_with_staged_verify(
        &harness,
        staged,
        Some(intended_witness("check", true, "", "")),
        4,
        &["check", "--apply", "--output=text"],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Modified rust/tests/fixtures/hello/BUILD.bazel"),
        "{out}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert_eq!(
        out.matches("Running generate").count(),
        1,
        "only quality writes rerun generate: {out}"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        5,
        "four apply launches plus one verification launch of generate"
    );
}

#[test]
fn fix_default_checks_without_writing() {
    let harness = umbrella_findings("umbrella-fix-default");
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("Running format analysis for //..."), "{out}");
    assert!(out.contains("Running lint analysis"), "{out}");
    assert!(out.contains("Running typecheck analysis"), "{out}");
    assert!(out.contains("Running generate"), "{out}");
    assert!(
        err.contains(
            "dx: phases: generate failed, format findings, lint findings, typecheck findings"
        ),
        "{err}"
    );
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n",
        "default fix never mutates"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        4,
        "every phase runs after findings"
    );
}

#[test]
fn fix_apply_verifies_quality_apply_with_clean_recheck() {
    let mut harness = Harness::new("umbrella-fix-verify");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = harness.staged_artifacts();
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    let (code, out, err) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=text"],
    );
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "the apply pass wrote the validated edit"
    );
    assert!(out.contains("Applied 1 file(s)."), "{out}");
    assert!(
        !err.contains("Not applied"),
        "later phases see the written file and offer no edit: {err}"
    );
    assert_eq!(
        out.matches("Running format analysis for //...").count(),
        1,
        "a clean verification stays silent: {out}"
    );
    assert_eq!(
        out.matches("Running generate").count(),
        2,
        "format wrote a file, so generate runs again after typecheck: {out}"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        7,
        "five apply launches plus generate and format verification"
    );
}

#[test]
fn fix_apply_clean_runs_no_verification() {
    let mut harness = umbrella_clean("umbrella-fix-clean-no-verify");
    harness.intended = Some(intended_witness("default", true, "", ""));
    let (code, out, err) = harness.run(&["check", "--apply", "--output=text"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out.matches("Running format analysis for //...").count(),
        1,
        "{out}"
    );
    assert_eq!(out.matches("Running generate").count(), 1, "{out}");
    assert_eq!(
        harness.seen_env.borrow().len(),
        4,
        "four apply launches and no verification"
    );
}

#[test]
fn fix_apply_json_clean_emits_no_verify_events() {
    let mut harness = umbrella_clean("umbrella-fix-clean-no-verify-json");
    harness.intended = Some(intended_witness("default", true, "", ""));
    let (code, out, err) = harness.run(&["check", "--apply", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert!(
        events.iter().all(|event| !event["correlation"]
            .as_str()
            .is_some_and(|correlation| correlation.starts_with("verify/"))),
        "{out}"
    );
    assert_eq!(
        events.last().expect("last")["exit_code"],
        serde_json::json!(0)
    );
    assert_eq!(harness.seen_env.borrow().len(), 4);
}

#[test]
fn check_apply_unsafe_reaches_the_quality_phases() {
    let mut harness = umbrella_clean("umbrella-unsafe");
    harness.intended = Some(intended_witness("default", true, "", ""));
    let (code, out, err) = harness.run(&["check", "--apply", "--unsafe", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        json_events(&out)[0]["unsafe_fixes"],
        serde_json::json!(true)
    );
    let builds = harness.build_argv.borrow();
    let unsafe_builds: Vec<&Vec<String>> = builds
        .iter()
        .filter(|argv| {
            argv.iter()
                .any(|arg| arg == "--aspects_parameters=unsafe_fixes=true")
        })
        .collect();
    assert_eq!(unsafe_builds.len(), 3, "{builds:?}");
    assert!(
        unsafe_builds
            .iter()
            .all(|argv| argv.iter().any(|arg| arg.starts_with("--aspects=")))
    );
}

#[test]
fn fix_apply_fails_when_verification_finds_drift() {
    let mut harness = Harness::new("umbrella-fix-verify-fails");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = harness.staged_artifacts();
    let drift = harness.valid_result(
        vec![Harness::diagnostic("unused", true)],
        vec![harness.replacement(b"y")],
    );
    let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
        .iter()
        .map(|(path, _)| (path.clone(), drift.clone()))
        .collect();
    let (code, out, err) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=text"],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "only the apply pass writes; verification never does"
    );
    assert!(
        err.contains("dx: verification_failed: format failed verification after apply"),
        "{err}"
    );
    assert!(
        !err.contains("verification skipped"),
        "lint and typecheck touched nothing, so none is left to skip: {err}"
    );
    assert!(out.contains("Running format analysis for //..."), "{out}");
    assert_eq!(
        out.matches("Running format analysis for //...").count(),
        2,
        "apply ran format, then verification re-ran it: {out}"
    );
    assert_eq!(
        out.matches("Running lint analysis for //...").count(),
        1,
        "verification stops after the first failing phase: {out}"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        7,
        "five apply launches plus generate and format verification"
    );
}

#[test]
fn fix_apply_json_reports_verification_failure() {
    let mut harness = Harness::new("umbrella-fix-verify-json");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = harness.staged_artifacts();
    let drift = harness.valid_result(
        vec![Harness::diagnostic("unused", true)],
        vec![harness.replacement(b"y")],
    );
    let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
        .iter()
        .map(|(path, _)| (path.clone(), drift.clone()))
        .collect();
    let (code, out, _) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=json"],
    );
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    assert_eq!(events.first().expect("first")["command"], "check");
    assert_eq!(events.first().expect("first")["mode"], "default");
    let failure = event(&events, "error");
    assert_eq!(failure["code"], serde_json::json!("verification_failed"));
    assert_eq!(failure["phase"], serde_json::json!("format"));
    assert_eq!(failure["correlation"], serde_json::json!("check/format"));
    assert_eq!(
        events.last().expect("last")["exit_code"],
        serde_json::json!(1)
    );
}

#[test]
fn verification_failure_names_the_skipped_phases() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    announce_verification_failure(
        &mut out,
        &mut err,
        OutputMode::Json,
        "check",
        "format",
        &["lint", "typecheck"],
    )
    .expect("announced");
    let err = String::from_utf8(err).expect("stderr");
    assert_eq!(
        err,
        "dx: verification_failed: format failed verification after apply; verification skipped for lint, typecheck\n"
    );
    let events = json_events(&String::from_utf8(out).expect("stdout"));
    assert_eq!(events[0]["code"], serde_json::json!("verification_failed"));
    assert_eq!(events[0]["phase"], serde_json::json!("format"));
}

#[test]
fn verification_collection_failure_is_shown_without_a_failed_phase() {
    let mut verification = Verification {
        failed_phase: None,
        skipped: vec![],
        apply_failed: vec![],
        verified: vec![],
        out: vec![],
        err: vec![],
        readable: true,
        ended: None,
    };
    let forwarded = forward_phase(
        &mut verification.out,
        &mut verification.err,
        OutputMode::Json,
        ("check", "check"),
        "verify/lint",
        (b"not an event\n", b""),
        0,
    );
    assert_eq!(forwarded, Ok(false));
    verification.readable = false;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    deliver_verification(&mut out, &mut err, OutputMode::Json, "check", &verification)
        .expect("delivered");
    let err = String::from_utf8(err).expect("utf8");
    assert!(
        err.contains("dx: collection_failed: verify/lint wrote an unreadable event"),
        "{err}"
    );
    let out = String::from_utf8(out).expect("utf8");
    let events = json_events(&out);
    let failures: Vec<_> = events.iter().filter(|e| e["event"] == "error").collect();
    assert_eq!(failures.len(), 1, "{out}");
    assert_eq!(failures[0]["code"], serde_json::json!("collection_failed"));
    assert_eq!(
        failures[0]["correlation"],
        serde_json::json!("check/verify/lint")
    );
    assert!(!out.contains("verification_failed"), "{out}");
}

#[test]
fn clean_verification_delivers_nothing() {
    let verification = Verification {
        failed_phase: None,
        skipped: vec![],
        apply_failed: vec![],
        verified: vec![],
        out: b"buffered out\n".to_vec(),
        err: b"buffered err\n".to_vec(),
        readable: true,
        ended: None,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    deliver_verification(&mut out, &mut err, OutputMode::Json, "check", &verification)
        .expect("delivered");
    assert!(out.is_empty() && err.is_empty());
}

fn oscillating(harness: &Harness) -> Vec<u8> {
    let mut result = quality_result::decode_validated(&harness.valid_result(vec![], vec![]))
        .expect("valid result");
    result.convergence = quality_result::proto::Convergence::Oscillation as i32;
    result.completed_rounds = 2;
    quality_result::encode_validated(&result).expect("encode")
}

const NOT_CONVERGED: &str = "dx: not_converged: //test:corpus did not converge: \
                             OSCILLATION after 2 round(s) of lint-tool";

#[test]
fn check_and_fix_keep_going_after_a_diagnostic_free_unstable_phase() {
    for words in [["check", "--output=text"], ["check", "--apply"]] {
        let mut harness = Harness::new("umbrella-unstable");
        harness.write_source("src/a.py", "x = 1\n");
        let bytes = oscillating(&harness);
        harness.results.insert("//test:corpus".to_owned(), bytes);
        let (code, out, err) = harness.run(&words);
        assert_eq!(code, 1, "{words:?}: {out}{err}");
        assert!(err.contains(NOT_CONVERGED), "{words:?}: {err}");
        assert!(out.contains("Running lint analysis"), "{words:?}: {out}");
        assert!(out.contains("Running generate"), "{words:?}: {out}");
        assert_eq!(
            std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
            b"x = 1\n",
            "{words:?} keeps the source bytes"
        );
        assert_eq!(harness.seen_env.borrow().len(), 4, "{words:?}");
    }
}

#[test]
fn check_json_forwards_the_unstable_producer() {
    let mut harness = Harness::new("umbrella-unstable-json");
    harness.write_source("src/a.py", "x = 1\n");
    let bytes = oscillating(&harness);
    harness.results.insert("//test:corpus".to_owned(), bytes);
    let (code, out, _) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let failure = events
        .iter()
        .find(|event| event["event"] == "error" && event["correlation"] != "check/generate")
        .expect("error event");
    assert_eq!(failure["code"], "not_converged");
    assert_eq!(
        failure["message"],
        "//test:corpus did not converge: OSCILLATION after 2 round(s) of lint-tool"
    );
    assert_eq!(events.last().expect("last")["exit_code"], 1);
}

#[test]
fn fix_apply_verification_fails_on_an_unstable_recheck() {
    let mut harness = Harness::new("umbrella-fix-verify-unstable");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = harness.staged_artifacts();
    let unstable =
        quality_result::decode_validated(&harness.result_full(vec![], vec![], vec![], vec![]))
            .map(|mut result| {
                result.convergence = quality_result::proto::Convergence::Oscillation as i32;
                result.completed_rounds = 2;
                quality_result::encode_validated(&result).expect("encode")
            })
            .expect("valid result");
    let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
        .iter()
        .map(|(path, _)| (path.clone(), unstable.clone()))
        .collect();
    let (code, out, err) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=text"],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "the stable apply pass wrote its edit"
    );
    assert!(err.contains(NOT_CONVERGED), "{err}");
    assert!(
        err.contains("dx: verification_failed: format failed verification after apply"),
        "{err}"
    );
    assert!(
        !err.contains("verification skipped"),
        "lint and typecheck touched nothing, so none is left to skip: {err}"
    );
}

#[test]
fn fix_keeps_going_when_a_later_phase_goes_stale() {
    let harness = umbrella_findings("umbrella-stale");
    let (code, out, _) = harness.run(&["check", "--apply", "--output=text"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("Running format analysis for //..."), "{out}");
    assert!(out.contains("Running lint analysis for //..."), "{out}");
    assert!(out.contains("Running typecheck analysis"), "{out}");
    assert!(out.contains("Running generate"), "{out}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "format applied before lint went stale"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        5,
        "every phase launches once, then format is re-checked; stale lint is not"
    );
    assert!(
        harness
            .seen_env
            .borrow()
            .iter()
            .flatten()
            .any(|(key, _)| key == GENERATE_ENV_INTENDED),
        "generate dispatch ran"
    );
}

/// A `check --apply` whose format edit goes stale before the apply pass.
fn refused_format(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    harness.write_source("src/a.py", "z = 2\n");
    harness
}

#[test]
fn fix_apply_fails_when_format_refuses_a_stale_source() {
    let harness = refused_format("umbrella-fix-refused");
    let (code, out, err) = harness.run(&["check", "--apply", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"z = 2\n"
    );
    assert!(
        err.contains("Not applied: src/a.py (stale_source)"),
        "{err}"
    );
    assert!(out.contains("Running lint analysis for //..."), "{out}");
    assert!(out.contains("Running typecheck analysis"), "{out}");
    assert!(out.contains("Running generate"), "{out}");
    assert_eq!(
        harness.seen_env.borrow().len(),
        5,
        "every phase launches once, then generate is re-checked; the refused format is not"
    );

    let harness = refused_format("umbrella-fix-refused-json");
    let (code, out, err) = harness.run(&["check", "--apply", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let format_finished = events_of_kind(&events, "phase_finished")
        .into_iter()
        .find(|event| event["phase"] == "format")
        .expect("format finished");
    assert_eq!(format_finished["exit_code"], 1, "{out}");
    assert_eq!(
        format_finished["changes"]["refused"],
        serde_json::json!(1),
        "{out}"
    );
    assert!(
        events.iter().all(|event| !event["correlation"]
            .as_str()
            .is_some_and(|correlation| correlation.starts_with("verify/"))),
        "{out}"
    );
    assert_eq!(
        events.last().expect("last")["exit_code"],
        serde_json::json!(1)
    );
    assert_eq!(harness.seen_env.borrow().len(), 5);
}

#[test]
fn fix_runs_every_phase_after_a_tool_failure() {
    let harness = umbrella_clean("umbrella-fix-tool-failure");
    let runner = FailFrom {
        inner: harness.runner(),
        from: 1,
        to: Some(2),
        code: 1,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(runner.launches.get(), 4, "every phase launches");
    let events = json_events(&out);
    let started: Vec<&str> = events_of_kind(&events, "phase_started")
        .iter()
        .map(|event| event["phase"].as_str().expect("phase"))
        .collect();
    assert_eq!(started, vec!["generate", "format", "lint", "typecheck"]);
    let failures: Vec<&str> = events_of_kind(&events, "error")
        .iter()
        .map(|event| event["correlation"].as_str().expect("correlation"))
        .collect();
    assert_eq!(failures, vec!["check/format"], "{out}");
    assert!(
        events_of_kind(&events, "notice")
            .iter()
            .all(|notice| notice["code"] != "umbrella_phase_skipped"),
        "{out}"
    );
    let finished = *events_of_kind(&events, "command_finished")
        .last()
        .expect("umbrella finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
}

/// Runs `fix --apply --output=<output>` where lint (launch 2) exits 7 and
/// the verification launches see `verify_result`; only format edits.
fn fix_apply_lint_fails(
    harness: &Harness,
    verify_result: Vec<u8>,
    output: &str,
) -> (i32, String, String) {
    let mut staged = staged_runner(
        harness,
        harness
            .staged_artifacts()
            .iter()
            .map(|(path, _)| (path.clone(), verify_result.clone()))
            .collect(),
        Some(intended_witness("check", true, "", "")),
        5,
    );
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    staged.clean_files = staged
        .apply_files
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    staged.clean_from = 1;
    let runner = FailFrom {
        inner: staged,
        from: 2,
        to: Some(3),
        code: 7,
        launches: Cell::new(0),
    };
    let output_flag = format!("--output={output}");
    let inv = umbrella_invocation(harness, &["check", "--apply", output_flag.as_str()]);
    harness.execute_with(&inv, &runner)
}

fn partial_failure_harness(name: &str) -> Harness {
    let mut harness = Harness::new(name);
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    harness
}

#[test]
fn fix_apply_partial_failure_rechecks_passing_phases() {
    let harness = partial_failure_harness("umbrella-fix-partial-recheck");
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let (code, out, err) = fix_apply_lint_fails(&harness, clean, "json");
    let events = json_events(&out);
    let lint_code = events
        .iter()
        .find(|event| event["event"] == "phase_finished" && event["phase"] == "lint")
        .expect("lint finished")["exit_code"]
        .as_i64()
        .expect("lint code");
    assert_ne!(lint_code, 0, "{out}");
    assert_eq!(i64::from(code), lint_code, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "format applied before lint failed"
    );
    let verify: Vec<&str> = events
        .iter()
        .filter_map(|event| event["correlation"].as_str())
        .filter(|correlation| correlation.starts_with("check/verify/"))
        .collect();
    assert!(verify.contains(&"check/verify/format"), "{out}{err}");
    assert!(!verify.contains(&"check/verify/lint"), "{out}");
    assert!(
        err.contains("verification passed after apply for generate, format")
            && err.contains("failed during apply: lint"),
        "{err}"
    );
    assert_eq!(
        events_of_kind(&events, "command_finished").len(),
        1,
        "{out}"
    );
    assert_eq!(
        events.last().expect("last")["exit_code"],
        serde_json::json!(lint_code)
    );
}

#[test]
fn fix_apply_partial_failure_reports_a_failing_recheck() {
    let harness = partial_failure_harness("umbrella-fix-partial-recheck-fails");
    let unstable =
        quality_result::decode_validated(&harness.result_full(vec![], vec![], vec![], vec![]))
            .map(|mut result| {
                result.convergence = quality_result::proto::Convergence::Oscillation as i32;
                result.completed_rounds = 2;
                quality_result::encode_validated(&result).expect("encode")
            })
            .expect("valid result");
    let (code, out, err) = fix_apply_lint_fails(&harness, unstable, "text");
    assert_eq!(code, 7, "the failed phase's code stays: {out}{err}");
    assert!(
        err.contains("dx: verification_failed: format failed verification after apply"),
        "{err}"
    );
}

#[test]
fn fix_apply_does_not_recheck_the_failed_phase() {
    let mut harness = Harness::new("umbrella-fix-failed-no-verify");
    harness.write_source("src/a.py", "x = 1\n");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement(b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = staged_runner(
        &harness,
        harness.staged_artifacts(),
        Some(intended_witness("check", true, "", "")),
        5,
    );
    let runner = FailFrom {
        inner: staged,
        from: 3,
        to: Some(4),
        code: 1,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--apply", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "format applied before typecheck failed"
    );
    assert_eq!(
        runner.launches.get(),
        7,
        "five phase launches plus the generate and format rechecks, never the failed typecheck"
    );
    let events = json_events(&out);
    assert!(
        events.iter().all(|event| !event["correlation"]
            .as_str()
            .is_some_and(|correlation| correlation == "verify/typecheck")),
        "{out}"
    );
    assert_eq!(
        events.last().expect("last")["exit_code"],
        serde_json::json!(1)
    );
}

#[test]
fn check_runs_every_phase_after_a_tool_failure() {
    let harness = umbrella_clean("umbrella-check-tool-failure");
    let runner = FailFrom {
        inner: harness.runner(),
        from: 2,
        to: Some(3),
        code: 1,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--output=text"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        out.contains("Running typecheck analysis for //..."),
        "{out}"
    );
    assert!(out.contains("Running generate for //..."), "{out}");
    assert!(
        err.contains("dx: analysis_failed: lint analysis failed"),
        "{err}"
    );
    assert!(
        err.contains("dx: phases: generate clean, format clean, lint failed, typecheck clean"),
        "{err}"
    );
    assert_eq!(runner.launches.get(), 4, "every phase launches");
}

#[test]
fn check_runs_every_phase_after_a_lint_compile_error() {
    let harness = umbrella_clean("umbrella-check-compile");
    let runner = compile_fail_at_lint(&harness);
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(runner.launches.get(), 4, "every phase launches");
    let events = json_events(&out);
    assert!(
        events_of_kind(&events, "notice")
            .iter()
            .all(|notice| notice["code"] != "umbrella_phase_skipped"),
        "{out}"
    );
    let errors = events_of_kind(&events, "error");
    assert!(
        errors.iter().any(|event| event["code"] == "bazel_failed"
            && event["cause"] == "bazel"
            && event["correlation"] == "check/lint"),
        "{out}"
    );
    assert!(!out.contains("analysis_failed"), "{out}");
    let failures = events_of_kind(&events, "build_failure");
    assert_eq!(failures.len(), 1, "{out}");
    assert_eq!(failures[0]["correlation"], "check/lint");
    assert_eq!(
        failures[0]["diagnostics"]["items"][0]["path"],
        "app/src/main.rs"
    );
    assert_eq!(events.last().expect("last")["exit_code"], 1, "{out}");
}

#[test]
fn fix_stops_after_a_lint_compile_error() {
    let harness = umbrella_clean("umbrella-fix-compile");
    let runner = compile_fail_at_lint(&harness);
    let inv = umbrella_invocation(&harness, &["check", "--apply", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(runner.launches.get(), 3, "typecheck never launches");
    let events = json_events(&out);
    let skipped: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_skipped")
        .filter_map(|notice| notice["message"].as_str())
        .collect();
    assert_eq!(
        skipped,
        vec!["typecheck skipped after lint failed to compile"]
    );
    let errors = events_of_kind(&events, "error");
    assert!(
        errors
            .iter()
            .any(|event| event["code"] == "bazel_failed" && event["correlation"] == "check/lint"),
        "{out}"
    );
}

#[test]
fn a_selection_stop_without_an_error_event_gains_one_before_the_closing_event() {
    let started = br#"{"event":"phase_started","phase":"format"}"#;
    let finished = br#"{"event":"phase_finished","phase":"format","exit_code":2}"#;
    let mut out = [&started[..], b"\n", &finished[..], b"\n"].concat();
    let mut err = b"ERROR: no such target\nINFO: done\n".to_vec();
    explain_selection_stop(OutputMode::Json, "format", 2, (&mut out, &mut err));
    let lines: Vec<Value> = String::from_utf8(out)
        .expect("utf8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("event"))
        .collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert_eq!(lines[1]["event"], "error");
    assert_eq!(lines[1]["code"], "invalid_scope");
    assert_eq!(lines[2]["event"], "phase_finished");
    let message = lines[1]["message"].as_str().expect("message");
    assert!(message.contains("INFO: done"), "{message}");
    assert!(
        message.contains("pass a Bazel label or package"),
        "{message}"
    );
}

#[test]
fn a_selection_stop_that_already_has_an_error_event_is_left_alone() {
    let error = br#"{"event":"error","code":"invalid_arguments","message":"x"}"#;
    let mut out = [&error[..], b"\n"].concat();
    let before = out.clone();
    let mut err = Vec::new();
    explain_selection_stop(OutputMode::Json, "format", 2, (&mut out, &mut err));
    assert_eq!(out, before);
}

#[test]
fn a_silent_text_selection_stop_writes_a_stderr_line() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    explain_selection_stop(
        OutputMode::Text { quiet: false },
        "format",
        2,
        (&mut out, &mut err),
    );
    let text = String::from_utf8(err).expect("utf8");
    assert!(
        text.starts_with("dx: invalid_scope: format rejected"),
        "{text}"
    );
}

#[test]
fn umbrella_stdout_report_is_rejected_pre_exec() {
    let harness = umbrella_clean("umbrella-stdout-report");
    let (code, _, err) = harness.run(&["check", "--report=sarif=-", "--output=text"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("share one stdout document"), "{err}");
    assert!(harness.seen_env.borrow().is_empty(), "no phase launched");
}

#[test]
fn umbrella_unknown_report_is_rejected_pre_exec() {
    let harness = umbrella_clean("umbrella-unknown-report");
    let (code, _, err) = harness.run(&["check", "--report=junit=out.xml"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("unsupported report format"), "{err}");
    assert!(harness.seen_env.borrow().is_empty(), "no phase launched");
}

#[test]
fn umbrella_sarif_merges_executed_phases_in_order() {
    let harness = umbrella_clean("umbrella-sarif");
    let (code, out, err) = harness.run(&["check", "--output=json", "--report=sarif=out.sarif"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let report = *events_of_kind(&events, "report")
        .last()
        .expect("umbrella report");
    assert_eq!(report["format"], serde_json::json!("sarif"));
    assert_eq!(report["path"], serde_json::json!("out.sarif"));
    assert_eq!(
        report["results_complete"],
        serde_json::json!(true),
        "every phase ran and every capture parsed"
    );
    assert_eq!(
        event(&events, "command_finished")["results_complete"],
        serde_json::json!(true)
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("SARIF JSON");
    assert_eq!(
        document["runs"].as_array().expect("runs").len(),
        2,
        "lint and typecheck each contribute one run"
    );
    assert_eq!(
        document["runs"][0], document["runs"][1],
        "both phases replay the shared clean fixture"
    );

    let (code, out, _) = harness.run(&["check", "--output=text", "--report=sarif=out.sarif"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out.lines().last().expect("last line"),
        "Wrote sarif report to out.sarif."
    );
}

#[test]
fn fix_sarif_covers_only_executed_phases() {
    let mut harness = umbrella_findings("umbrella-sarif-stop");
    harness.free_disk_bytes = Some(1 << 20);
    let run = |output: &str| {
        let runner = FailFrom {
            inner: harness.runner(),
            from: 2,
            to: None,
            code: 36,
            launches: Cell::new(0),
        };
        let inv = umbrella_invocation(&harness, &["check", output, "--report=sarif=out.sarif"]);
        harness.execute_with(&inv, &runner)
    };
    let (code, out, err) = run("--output=json");
    assert_eq!(code, 36, "{out}{err}");
    let events = json_events(&out);
    assert_eq!(
        events_of_kind(&events, "report")
            .last()
            .expect("umbrella report")["results_complete"],
        serde_json::json!(false),
        "a run stopped at lint leaves the report incomplete"
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("SARIF JSON");
    assert_eq!(
        document["runs"].as_array().expect("runs").len(),
        0,
        "no SARIF-capable phase finished, no runs"
    );

    let (code, _, err) = run("--diff");
    assert_eq!(code, 36);
    assert!(
        err.ends_with(
            "Wrote sarif report to out.sarif (incomplete: generate failed, \
             lint failed, typecheck skipped).\n\
             dx: phases: generate failed, format findings, lint failed, typecheck skipped\n"
        ),
        "{err}"
    );
}

#[test]
fn umbrella_sarif_skips_phase_that_could_not_render() {
    let harness = umbrella_findings("umbrella-sarif-unrendered");
    let (code, out, err) = harness.run(&[
        "check",
        "--apply",
        "--output=text",
        "--report=sarif=out.sarif",
    ]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("failed to render SARIF report"), "{err}");
    assert!(
        out.contains(
            "Wrote sarif report to out.sarif \
             (incomplete: generate failed, lint failed, typecheck failed)."
        ),
        "{out}"
    );
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(harness.workspace.join("out.sarif")).expect("sarif"))
            .expect("SARIF JSON");
    assert!(
        document["runs"].as_array().expect("runs").is_empty(),
        "the phase that never rendered contributes no runs"
    );
}

#[test]
fn umbrella_report_write_failure_fails() {
    let harness = umbrella_clean("umbrella-report-fail");
    let (code, _, err) = harness.run(&[
        "check",
        "--output=text",
        "--report=sarif=missing-dir/out.sarif",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("report_failed"), "{err}");

    let (code, out, _) = harness.run(&[
        "check",
        "--output=json",
        "--report=sarif=missing-dir/out.sarif",
    ]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"code\":\"report_failed\""), "{out}");
}

#[test]
fn umbrella_report_into_a_directory_ends_with_one_finished() {
    let harness = umbrella_clean("umbrella-report-dir");
    std::fs::create_dir(harness.workspace.join("out.sarif")).expect("directory");
    let (code, out, _) = harness.run(&["check", "--output=json", "--report=sarif=out.sarif"]);
    assert_eq!(code, 1, "{out}");
    let events = json_events(&out);
    let finished = events
        .iter()
        .filter(|event| event["event"] == "command_finished")
        .count();
    assert_eq!(finished, 1, "{out}");
    assert_eq!(
        events.last().expect("last")["event"],
        "command_finished",
        "{out}"
    );
    assert!(
        events.iter().any(|event| event["code"] == "report_failed"),
        "{out}"
    );
}

#[test]
fn umbrella_failed_phase_and_failed_report_still_fails() {
    let harness = umbrella_findings("umbrella-report-fail-stop");
    let (code, _, err) = harness.run(&[
        "check",
        "--output=text",
        "--report=sarif=missing-dir/out.sarif",
    ]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("report_failed"), "{err}");
}

#[test]
fn fix_json_reports_umbrella_lifecycle() {
    let harness = umbrella_clean("umbrella-fix-json");
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert_eq!(events.first().expect("first")["command"], "check");
    assert_eq!(
        events.first().expect("first")["mode"],
        "check",
        "bare fix checks without --apply"
    );
    assert_eq!(events.last().expect("last")["exit_code"], 0);
}

#[test]
fn umbrella_diff_renders_every_phase_patch() {
    let harness = umbrella_findings("umbrella-diff");
    let (code, out, _) = harness.run(&["check", "--diff"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("src/a.py"), "{out}");
    assert_eq!(harness.seen_env.borrow().len(), 4, "every phase launches");
}

#[test]
fn check_json_correlates_one_phase_start_per_phase() {
    let harness = umbrella_clean("umbrella-phase-events");
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert!(events_of_kind(&events, "operation").is_empty());
    let started = events_of_kind(&events, "phase_started");
    let described: Vec<(&str, &str, &str)> = started
        .iter()
        .map(|event| {
            (
                event["command"].as_str().expect("command"),
                event["phase"].as_str().expect("phase"),
                event["mode"].as_str().expect("mode"),
            )
        })
        .collect();
    assert_eq!(
        described,
        vec![
            ("check", "generate", "check"),
            ("check", "format", "check"),
            ("check", "lint", "check"),
            ("check", "typecheck", "check")
        ]
    );
    for (index, phase) in ["generate", "format", "lint", "typecheck"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            started[index]["correlation"],
            serde_json::json!(format!("check/{phase}")),
            "{phase}"
        );
    }
    assert!(
        events_of_kind(&events, "notice").is_empty(),
        "nothing skipped, nothing to notice"
    );
}

#[test]
fn check_skips_the_later_phases_after_bazel_runs_out_of_memory() {
    let harness = umbrella_clean("umbrella-out-of-memory");
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let runner = FailFrom {
        inner: harness.runner(),
        from: 2,
        to: None,
        code: 33,
        launches: Cell::new(0),
    };
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 33, "{out}{err}");
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors[0]["code"], serde_json::json!("server_out_of_memory"));
    assert_eq!(errors[0]["cause"], serde_json::json!("host"));
    let skipped = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == serde_json::json!("umbrella_phase_skipped"))
        .count();
    assert!(skipped > 0, "{out}");
}

#[test]
fn check_reports_a_full_disk_once_and_skips_the_later_phases() {
    let mut harness = umbrella_clean("umbrella-disk-full");
    harness.free_disk_bytes = Some(1 << 20);
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let runner = FailFrom {
        inner: harness.runner(),
        from: 2,
        to: None,
        code: 36,
        launches: Cell::new(0),
    };
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 36, "{out}{err}");
    assert!(err.contains("dx: disk_full: "), "{err}");
    assert!(!err.contains("analysis_failed"), "{err}");
    let events = json_events(&out);
    let errors = events_of_kind(&events, "error");
    assert_eq!(errors.len(), 1, "{out}");
    assert_eq!(errors[0]["code"], serde_json::json!("disk_full"));
    assert_eq!(errors[0]["cause"], serde_json::json!("host"));
    assert_eq!(errors[0]["correlation"], serde_json::json!("check/lint"));
    let skipped: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == serde_json::json!("umbrella_phase_skipped"))
        .filter_map(|notice| notice["message"].as_str())
        .collect();
    assert_eq!(
        skipped,
        vec!["typecheck skipped after lint failed to run its tools"]
    );
}

#[test]
fn check_json_names_every_skipped_phase() {
    let mut harness = umbrella_clean("umbrella-skip-events");
    harness.free_disk_bytes = Some(1 << 20);
    let runner = FailFrom {
        inner: harness.runner(),
        from: 2,
        to: None,
        code: 36,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(
        &harness,
        &["check", "--output=json", "--report=sarif=out.sarif"],
    );
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 36, "{out}{err}");
    let events = json_events(&out);
    let skipped: Vec<(&str, &str)> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] != "disk_low")
        .map(|notice| {
            (
                notice["code"].as_str().expect("code"),
                notice["message"].as_str().expect("message"),
            )
        })
        .collect();
    assert_eq!(
        skipped,
        vec![(
            CODE_UMBRELLA_PHASE_SKIPPED,
            "typecheck skipped after lint failed to run its tools"
        )]
    );
    for notice in events_of_kind(&events, "notice")
        .into_iter()
        .filter(|notice| notice["code"] == "umbrella_phase_skipped")
    {
        assert_eq!(notice["related_command"], serde_json::json!("check"));
        assert!(notice["correlation"].is_string(), "{notice}");
    }
    assert_eq!(
        events_of_kind(&events, "report")
            .last()
            .expect("umbrella report")["results_complete"],
        serde_json::json!(false)
    );
    let finished = *events_of_kind(&events, "command_finished")
        .last()
        .expect("umbrella finished");
    assert_eq!(finished["exit_code"], serde_json::json!(36));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    assert_eq!(
        harness.seen_env.borrow().len(),
        3,
        "unsafe work never starts"
    );
}

#[test]
fn check_missing_capture_fails_the_run_and_keeps_valid_partial_runs() {
    let (harness, code, out, err) = umbrella_with_broken_capture(
        umbrella_clean("umbrella-missing-capture"),
        Rewrite::Remove,
        &["check", "--output=text", "--report=sarif=out.sarif"],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        err.replace("no findings in 0 target(s)\n", ""),
        "dx: collection_failed: lint wrote no sarif capture\n",
        "{err}"
    );
    assert_eq!(
        out.lines().last().expect("last line"),
        "Wrote sarif report to out.sarif (incomplete: lint wrote no sarif capture)."
    );
    for phase in ["format", "lint", "typecheck", "generate"] {
        let marker = match phase {
            "generate" => "Running generate for //...",
            _ => &format!("Running {phase} analysis for //..."),
        };
        assert!(out.contains(marker), "{phase} ran: {out}");
    }
    let runs = merged_runs(&harness);
    assert_eq!(
        runs.len(),
        1,
        "the typecheck capture still lands in the merged document"
    );
    assert!(
        !capture_path(&harness, "lint", 2).exists(),
        "the capture is deleted even when it was never written"
    );
}

#[test]
fn check_missing_capture_reports_an_incomplete_json_document() {
    let (harness, code, out, err) = umbrella_with_broken_capture(
        umbrella_clean("umbrella-missing-capture-json"),
        Rewrite::Remove,
        &["check", "--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let failure = event(&events, "error");
    assert_eq!(failure["code"], serde_json::json!(CODE_COLLECTION_FAILED));
    assert_eq!(
        failure["message"],
        serde_json::json!("lint wrote no sarif capture")
    );
    assert_eq!(failure["phase"], serde_json::json!("lint"));
    assert_eq!(failure["correlation"], serde_json::json!("check/lint"));
    assert_eq!(
        events_of_kind(&events, "report")
            .last()
            .expect("umbrella report")["results_complete"],
        serde_json::json!(false)
    );
    let finished = *events_of_kind(&events, "command_finished")
        .last()
        .expect("umbrella finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    assert_eq!(merged_runs(&harness).len(), 1, "{out}");
}

#[test]
fn check_unreadable_capture_fails_the_run() {
    let (harness, code, out, err) = umbrella_with_broken_capture(
        umbrella_clean("umbrella-unreadable-capture"),
        Rewrite::Block,
        &["check", "--output=text", "--report=sarif=out.sarif"],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        err.replace("no findings in 0 target(s)\n", ""),
        "dx: collection_failed: lint sarif capture is unreadable\n",
        "{err}"
    );
    assert!(
        out.contains("(incomplete: lint sarif capture is unreadable)"),
        "{out}"
    );
    assert_eq!(merged_runs(&harness).len(), 1, "{out}");
}

#[test]
fn fix_apply_mode_shares_the_same_capture_collector() {
    let mut harness = umbrella_clean("umbrella-fix-missing-capture");
    harness.intended = Some(intended_witness("default", true, "", ""));
    let staged = harness.staged_artifacts();
    let runner = staged_runner(
        &harness,
        staged,
        Some(intended_witness("check", true, "", "")),
        4,
    );
    let capture = capture_path(&harness, "lint", 2);
    let breaking = RewriteCapture {
        inner: runner,
        capture,
        rewrite: Rewrite::Remove,
        done: Cell::new(false),
    };
    let inv = umbrella_invocation(
        &harness,
        &[
            "check",
            "--apply",
            "--output=text",
            "--report=sarif=out.sarif",
        ],
    );
    let (code, out, err) = harness.execute_with(&inv, &breaking);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        err.replace("no findings in 0 target(s)\n", ""),
        "dx: collection_failed: lint wrote no sarif capture\n",
        "{err}"
    );
    assert!(
        out.contains("Running generate for //..."),
        "apply mode still runs every phase: {out}"
    );
    assert_eq!(
        out.lines().last().expect("last line"),
        "Wrote sarif report to out.sarif (incomplete: lint wrote no sarif capture)."
    );
    assert_eq!(merged_runs(&harness).len(), 1, "{out}");
}

#[test]
fn check_malformed_capture_fails_the_run() {
    for (name, payload) in [
        ("not json at all", &b"{not json"[..]),
        ("an empty document", &b"{}"[..]),
        ("a document without runs", &br#"{"version":"2.1.0"}"#[..]),
        (
            "a document whose runs are not an array",
            &br#"{"version":"2.1.0","runs":{}}"#[..],
        ),
        (
            "a run without a tool",
            &br#"{"version":"2.1.0","runs":[{"nope":1}]}"#[..],
        ),
        (
            "a tool without a driver",
            &br#"{"version":"2.1.0","runs":[{"tool":{}}]}"#[..],
        ),
        (
            "a driver without a name",
            &br#"{"version":"2.1.0","runs":[{"tool":{"driver":{}}}]}"#[..],
        ),
        ("a bare JSON array", &b"[]"[..]),
    ] {
        let (harness, code, out, err) = umbrella_with_broken_capture(
            umbrella_clean(&format!("umbrella-malformed-{name}")),
            Rewrite::Bytes(payload),
            &["check", "--output=text", "--report=sarif=out.sarif"],
        );
        assert_eq!(code, 1, "{name}: {out}{err}");
        assert_eq!(
            err.replace("no findings in 0 target(s)\n", ""),
            "dx: collection_failed: lint sarif capture is malformed\n",
            "{name}: {err}"
        );
        assert_eq!(merged_runs(&harness).len(), 1, "{name}: {out}");
    }
}

#[test]
fn check_merged_document_is_deterministic() {
    let (harness, code, _, err) = umbrella_with_broken_capture(
        umbrella_clean("umbrella-deterministic"),
        Rewrite::Bytes(br#"{"version":"2.1.0","runs":[{"nope":1}]}"#),
        &["check", "--output=text", "--report=sarif=out.sarif"],
    );
    assert_eq!(code, 1, "{err}");
    let first = std::fs::read(harness.workspace.join("out.sarif")).expect("first");
    let (code, _, err) = harness.run(&["check", "--output=text", "--report=sarif=out.sarif"]);
    assert_eq!(code, 0, "{err}");
    let complete = std::fs::read(harness.workspace.join("out.sarif")).expect("second");
    assert_eq!(merged_runs(&harness).len(), 2);
    assert_ne!(first, complete, "the broken run never reaches the document");
}

#[test]
fn umbrella_broken_pipe_stdout_fails_with_141() {
    struct BrokenPipe;
    impl Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    let harness = umbrella_clean("umbrella-broken-pipe");
    let inv = umbrella_invocation(
        &harness,
        &["check", "--output=text", "--report=sarif=out.sarif"],
    );
    let runner = harness.runner();
    let mut out = BrokenPipe;
    let mut err = Vec::new();
    assert_eq!(harness.run_streams(&inv, &runner, &mut out, &mut err), 141);
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "a broken phase stream stops the run after the phase that filled it"
    );
    assert!(
        !harness.workspace.join("out.sarif").exists(),
        "no report is announced or written after stdout breaks"
    );
}

#[test]
fn umbrella_stdout_write_failure_is_operational() {
    struct FailAfterOne {
        seen: usize,
    }
    impl Write for FailAfterOne {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.seen += 1;
            if self.seen == 1 {
                return Ok(buf.len());
            }
            Err(io::Error::other("boom"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let harness = umbrella_clean("umbrella-stdout-failure");
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let runner = harness.runner();
    let mut out = FailAfterOne { seen: 0 };
    let mut err = Vec::new();
    assert_eq!(
        harness.run_streams(&inv, &runner, &mut out, &mut err),
        1,
        "the first event lands, the phase events do not"
    );
    assert_eq!(
        harness.seen_env.borrow().len(),
        1,
        "no later phase launched after stdout failed"
    );
    assert_eq!(out.seen, 2, "nothing is written after the failed write");
    assert!(
        err.is_empty(),
        "a stdout failure is not a stderr diagnostic"
    );
}

#[test]
fn collector_classifies_every_capture_state() {
    let dir = temp_dir("umbrella-collector");
    let good = dir.path().join("good.sarif");
    let bad = dir.path().join("bad.sarif");
    let missing = dir.path().join("missing.sarif");
    std::fs::write(
        &good,
        dx_plan::reports::render_sarif(
            &["lint-tool".to_owned()],
            &[],
            &std::collections::BTreeMap::new(),
            true,
        )
        .expect("render"),
    )
    .expect("good capture");
    std::fs::write(&bad, b"{}").expect("bad capture");

    let mut collector = PhaseCollector::default();
    for phase in UMBRELLA_PHASES {
        collector.plan(phase, phase.name());
    }
    collector.record_exit(0, 1, PhaseKind::Tool);
    collector.require_capture(1, good.clone());
    collector.record_exit(1, 0, PhaseKind::Clean);
    collector.require_capture(2, bad.clone());
    collector.record_exit(2, 0, PhaseKind::Clean);
    collector.require_capture(3, missing.clone());
    collector.record_exit(3, 0, PhaseKind::Clean);
    collector.skip_remaining(String::new());
    collector.collect();

    assert_eq!(
        collector.collection_failures(),
        vec![
            ("lint", "sarif capture is malformed"),
            ("typecheck", "wrote no sarif capture"),
        ]
    );
    assert_eq!(
        collector.reasons(),
        vec![
            "generate failed".to_owned(),
            "lint sarif capture is malformed".to_owned(),
            "typecheck wrote no sarif capture".to_owned(),
        ]
    );
    assert!(!collector.complete());
    let document: serde_json::Value = serde_json::from_str(&collector.document()).expect("JSON");
    assert_eq!(document["runs"].as_array().expect("runs").len(), 1);

    for path in [&good, &bad, &missing] {
        collector.cleanup();
        assert!(!path.exists(), "{path:?} deleted");
    }
}

#[test]
fn collector_merges_a_complete_run_deterministically() {
    let dir = temp_dir("umbrella-collector-complete");
    let captures: Vec<PathBuf> = ["lint", "typecheck"]
        .iter()
        .map(|name| {
            let path = dir.path().join(format!("{name}.sarif"));
            std::fs::write(
                &path,
                dx_plan::reports::render_sarif(
                    &[format!("{name}-tool")],
                    &[],
                    &std::collections::BTreeMap::new(),
                    true,
                )
                .expect("render"),
            )
            .expect("capture");
            path
        })
        .collect();

    let mut collector = PhaseCollector::default();
    for (index, phase) in UMBRELLA_PHASES.iter().enumerate() {
        collector.plan(*phase, phase.name());
        if let Some(path) = index.checked_sub(2).and_then(|at| captures.get(at)) {
            collector.require_capture(index, path.clone());
        }
        collector.record_exit(index, 0, PhaseKind::Clean);
    }
    collector.collect();
    assert!(collector.complete(), "{:?}", collector.reasons());
    assert!(collector.collection_failures().is_empty());
    assert!(collector.skipped().is_empty());
    assert_eq!(collector.reasons(), Vec::<String>::new());
    let document = collector.document();
    assert_eq!(document, collector.document(), "same input, same bytes");
    let parsed: serde_json::Value = serde_json::from_str(&document).expect("JSON");
    let runs = parsed["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["tool"]["driver"]["name"], "lint-tool");
    assert_eq!(runs[1]["tool"]["driver"]["name"], "typecheck-tool");
}

#[test]
fn test_phase_kind_maps_each_bazel_exit() {
    let cases = [
        (0, true, false, PhaseKind::Clean),
        (3, true, false, PhaseKind::Findings),
        (3, false, false, PhaseKind::Incomplete),
        (1, false, false, PhaseKind::Incomplete),
        (1, true, false, PhaseKind::Incomplete),
        (2, false, false, PhaseKind::Selection),
        (4, true, false, PhaseKind::Selection),
        (8, false, false, PhaseKind::Cancelled),
        (36, false, false, PhaseKind::Tool),
        (36, false, true, PhaseKind::Host),
        (33, false, false, PhaseKind::Host),
        (141, true, false, PhaseKind::Tool),
        (1, false, true, PhaseKind::Host),
        (1, true, true, PhaseKind::Host),
        (0, true, true, PhaseKind::Clean),
    ];
    for (code, complete, host, kind) in cases {
        assert_eq!(
            test_phase_kind(code, complete, host),
            kind,
            "{code} {complete} {host}"
        );
    }
}

#[test]
fn test_phase_results_complete_follows_kind() {
    let collect = |code: i32, kind: PhaseKind| {
        let mut collector = PhaseCollector::default();
        for phase in UMBRELLA_PHASES {
            collector.plan(phase, phase.name());
        }
        collector.plan(Command::Test, "test");
        for index in 0..UMBRELLA_PHASES.len() {
            collector.record_exit(index, 0, PhaseKind::Clean);
        }
        collector.record_exit(UMBRELLA_PHASES.len(), code, kind);
        collector.skip_remaining(String::new());
        collector.collect();
        collector
    };
    let findings = collect(3, PhaseKind::Findings);
    assert!(findings.complete(), "{:?}", findings.reasons());
    assert!(findings.reasons().is_empty());
    assert_eq!(findings.with_findings(), vec!["test"]);
    for code in [1, 3] {
        let incomplete = collect(code, PhaseKind::Incomplete);
        assert!(!incomplete.complete());
        assert_eq!(incomplete.reasons(), vec!["test failed".to_owned()]);
        assert!(incomplete.with_findings().is_empty());
    }
}

/// Runs the umbrella test phase against a harness and returns its outcome.
fn run_test_phase(harness: &Harness) -> (i32, PhaseKind) {
    let inv = umbrella_invocation(harness, &["test", "--output=text"]);
    let runner = harness.runner();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let run = execute_test_phase(&inv, harness.env(&runner, &mut out, &mut err));
    assert_eq!(run.effects, PhaseEffects::default());
    (run.code, run.kind)
}

#[test]
fn test_phase_reads_dx_test_completeness() {
    const FAILING_XML: &str = r#"<testsuites><testsuite name="s"><testcase name="fails" classname="c"><failure message="boom"/></testcase></testsuite></testsuites>"#;
    let failed_line = |uri: String| {
        test_result_full_line(
            "//a:t",
            1,
            1,
            1,
            "FAILED",
            None,
            None,
            None,
            &[(String::from("test.xml"), uri)],
        )
    };
    let harness = Harness::new("umbrella-test-phase-findings");
    let uri = write_bep_artifact(&harness, "test.xml", FAILING_XML.as_bytes());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![failed_line(uri)]),
        ..harness
    };
    assert_eq!(run_test_phase(&harness), (3, PhaseKind::Findings));

    let harness = Harness::new("umbrella-test-phase-missing");
    let missing = format!("file://{}", harness.temp.join("absent.xml").display());
    let harness = Harness {
        bazel_code: 3,
        raw_bep: Some(vec![failed_line(missing)]),
        ..harness
    };
    assert_eq!(run_test_phase(&harness), (3, PhaseKind::Incomplete));

    let harness = Harness {
        bazel_code: 1,
        ..Harness::new("umbrella-test-phase-build-failed")
    };
    assert_eq!(run_test_phase(&harness), (1, PhaseKind::Incomplete));
}

#[test]
fn test_phase_reports_launch_failure_as_host() {
    let harness = Harness {
        io_error: true,
        ..Harness::new("umbrella-test-phase-launch")
    };
    assert_eq!(run_test_phase(&harness), (1, PhaseKind::Host));

    let harness = Harness {
        signalled: true,
        ..Harness::new("umbrella-test-phase-signalled")
    };
    assert_eq!(run_test_phase(&harness), (1, PhaseKind::Host));

    let harness = Harness {
        bazel_code: 1,
        ..Harness::new("umbrella-test-phase-launch-build-failed")
    };
    assert_eq!(run_test_phase(&harness), (1, PhaseKind::Incomplete));
}

#[test]
fn test_phase_reports_full_disk_as_host() {
    let harness = Harness {
        bazel_code: 36,
        free_disk_bytes: Some(1 << 20),
        ..Harness::new("umbrella-test-phase-disk-full")
    };
    assert_eq!(run_test_phase(&harness), (36, PhaseKind::Host));
}

/// Runs `check` where format passes and lint reports findings.
fn check_with_failing_lint(name: &str, words: &[&str]) -> (Harness, i32, String, String) {
    let harness = umbrella_clean(name);
    let (artifact, _) = harness
        .staged_artifacts()
        .into_iter()
        .next()
        .expect("artifact");
    let findings = harness.valid_result(
        vec![Harness::diagnostic("unused", true)],
        vec![harness.replacement(b"y")],
    );
    let staged = staged_runner(
        &harness,
        vec![(artifact, findings)],
        harness.intended.clone(),
        1,
    );
    let inv = umbrella_invocation(&harness, words);
    let (code, out, err) = harness.execute_with(&inv, &staged);
    (harness, code, out, err)
}

#[test]
fn check_affected_hints_name_dx_check_apply_with_the_affected_flag() {
    let harness = umbrella_findings("umbrella-affected-hint");
    harness.write_source("test/BUILD.bazel", "");
    let mut inv = umbrella_invocation(&harness, &["check", "--affected", "--output=text"]);
    inv.affected = false;
    inv.targets = vec!["//test:corpus".to_owned(), "//test:other".to_owned()];
    inv.affected_handoff = Some(dx_plan::args::AffectedHandoff::default());
    let (code, out, err) = harness.execute_with(&inv, &harness.runner());
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("file(s) need changes. Apply them with: dx check --apply --affected\n"),
        "{err}"
    );
    for stale in ["dx format --apply", "dx lint --apply", "//test:"] {
        assert!(!err.contains(stale), "{stale}: {err}");
    }
}

fn commits_harness(name: &str, config: &str, subject: &str) -> (Harness, Invocation) {
    let harness = umbrella_clean(name);
    let git = |args: &[&str]| {
        let output = dx_process::capture::run_checked(
            dx_process::capture::git(&harness.workspace)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(["-c", "commit.gpgsign=false"])
                .args(args),
            dx_process::capture::DEFAULT_TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("{args:?}: {error}"));
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "-m", "base"]);
    let base = git(&["rev-parse", "HEAD"]);
    harness.write_source("dx.toml", config);
    git(&["add", "dx.toml"]);
    git(&["commit", "-q", "-m", subject]);
    let mut inv = umbrella_invocation(&harness, &["check", "--affected", "--output=text"]);
    inv.affected = false;
    inv.targets = vec!["//test:corpus".to_owned()];
    inv.affected_handoff = Some(dx_plan::args::AffectedHandoff {
        base: Some(base),
        ..Default::default()
    });
    (harness, inv)
}

const COMMITS_ON: &str = "schema_version = 1\n[check]\ncommits = \"conventional\"\n";

#[test]
fn check_affected_reports_a_commit_that_is_not_conventional() {
    let (harness, inv) = commits_harness("umbrella-commits-bad", COMMITS_ON, "update stuff");
    harness.write_source("test/BUILD.bazel", "");
    let (code, out, err) = harness.execute_with(&inv, &harness.runner());
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("update stuff: not a conventional commit"),
        "{err}"
    );
}

#[test]
fn check_affected_accepts_conventional_commits() {
    let (harness, inv) = commits_harness("umbrella-commits-good", COMMITS_ON, "fix: update stuff");
    harness.write_source("test/BUILD.bazel", "");
    let (code, out, err) = harness.execute_with(&inv, &harness.runner());
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("conventional"), "{err}");
}

#[test]
fn check_affected_skips_the_commits_phase_without_the_key() {
    let (harness, inv) = commits_harness(
        "umbrella-commits-off",
        "schema_version = 1\n",
        "update stuff",
    );
    harness.write_source("test/BUILD.bazel", "");
    let (code, out, err) = harness.execute_with(&inv, &harness.runner());
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("conventional"), "{err}");
}

#[test]
fn check_affected_generate_runs_on_affected_packages() {
    let harness = umbrella_clean("umbrella-affected-generate");
    harness.write_source("test/BUILD.bazel", "");
    let mut inv = umbrella_invocation(&harness, &["check", "--affected", "--output=text"]);
    inv.affected = false;
    inv.targets = vec!["//test:corpus".to_owned(), "//test:other".to_owned()];
    inv.affected_handoff = Some(dx_plan::args::AffectedHandoff::default());
    let (code, out, err) = harness.execute_with(&inv, &harness.runner());
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running generate for //test/..."), "{out}");
    assert!(!out.contains("Running generate for //..."), "{out}");
    let scopes: Vec<String> = harness
        .seen_env
        .borrow()
        .iter()
        .flatten()
        .filter(|(key, _)| key == dx_plan::plan::GENERATE_ENV_SCOPE)
        .map(|(_, value)| value.clone())
        .collect();
    assert_eq!(
        scopes,
        vec![r#"[{"dirs":["test"],"element":"//test/..."}]"#.to_owned()],
        "{out}"
    );
}

#[test]
fn check_target_hints_name_dx_check_apply_with_the_user_patterns() {
    let harness = umbrella_findings("umbrella-target-hint");
    let (code, out, err) = harness.run(&["check", "//test:corpus", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("file(s) need changes. Apply them with: dx check --apply //test:corpus\n"),
        "{err}"
    );
    assert!(!err.contains("dx format --apply"), "{err}");
}

#[test]
fn check_findings_phases_keep_their_findings_in_a_complete_report() {
    let (harness, code, out, err) = check_with_failing_lint(
        "umbrella-failed-lint",
        &["check", "--output=json", "--report=sarif=out.sarif"],
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let report = *events_of_kind(&events, "report")
        .last()
        .expect("umbrella report");
    assert_eq!(report["results_complete"], serde_json::json!(true));
    let finished = *events_of_kind(&events, "command_finished")
        .last()
        .expect("umbrella finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(true));
    let runs = merged_runs(&harness);
    assert_eq!(runs.len(), 2, "lint and typecheck captures are retained");
    assert!(
        runs[0]["results"]
            .as_array()
            .is_some_and(|results| !results.is_empty()),
        "{runs:?}"
    );
    let skipped: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_skipped")
        .map(|notice| notice["correlation"].as_str().expect("correlation"))
        .collect();
    assert!(skipped.is_empty(), "{skipped:?}");
    assert!(
        events_of_kind(&events, "error").is_empty(),
        "a findings failure is not a collection failure: {out}"
    );
}

#[test]
fn check_failed_phase_with_a_failed_report_write_keeps_its_exit() {
    let (_, code, out, err) = check_with_failing_lint(
        "umbrella-failed-lint-write",
        &[
            "check",
            "--output=text",
            "--report=sarif=missing-dir/out.sarif",
        ],
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("report_failed"), "{err}");
}

#[test]
fn collector_keeps_what_a_failed_phase_wrote() {
    let dir = temp_dir("umbrella-collector-failed");
    let good = dir.path().join("good.sarif");
    let bad = dir.path().join("bad.sarif");
    let missing = dir.path().join("missing.sarif");
    std::fs::write(
        &good,
        dx_plan::reports::render_sarif(
            &["lint-tool".to_owned()],
            &[],
            &std::collections::BTreeMap::new(),
            true,
        )
        .expect("render"),
    )
    .expect("good capture");
    std::fs::write(&bad, b"{").expect("bad capture");
    for (path, expected) in [
        (&good, Vec::new()),
        (&bad, vec![("lint", "sarif capture is malformed")]),
        (&missing, Vec::new()),
    ] {
        let mut collector = PhaseCollector::default();
        collector.plan(Command::Lint, "lint");
        collector.require_capture(0, path.clone());
        collector.record_exit(0, 1, PhaseKind::Tool);
        collector.collect();
        assert_eq!(collector.collection_failures(), expected);
        assert!(!collector.complete());
        let document: serde_json::Value =
            serde_json::from_str(&collector.document()).expect("JSON");
        assert_eq!(
            document["runs"].as_array().expect("runs").len(),
            usize::from(path == &good)
        );
    }
}

#[test]
fn collector_ignores_a_capture_a_failed_phase_never_owed() {
    let mut collector = PhaseCollector::default();
    for phase in UMBRELLA_PHASES {
        collector.plan(phase, phase.name());
    }
    collector.record_exit(0, 0, PhaseKind::Clean);
    collector.require_capture(1, PathBuf::from("/nonexistent/format.sarif"));
    collector.record_exit(1, 1, PhaseKind::Tool);
    collector.skip_remaining(String::new());
    collector.collect();
    assert_eq!(collector.collection_failures(), Vec::new());
    assert_eq!(
        collector.reasons(),
        vec![
            "format failed".to_owned(),
            "lint skipped".to_owned(),
            "typecheck skipped".to_owned(),
        ]
    );
    assert_eq!(collector.skipped(), vec!["lint", "typecheck"]);
    collector.cleanup();
}

#[test]
fn check_stop_after_findings_keeps_both_facts() {
    let mut harness = umbrella_findings("umbrella-stop-after-findings");
    harness.free_disk_bytes = Some(1 << 20);
    let runner = FailFrom {
        inner: harness.runner(),
        from: 3,
        to: None,
        code: 36,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(
        &harness,
        &["check", "--output=text", "--report=sarif=out.sarif"],
    );
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 36, "{out}{err}");
    assert!(
        err.contains(
            "dx: phases: generate failed, format findings, lint findings, typecheck failed"
        ),
        "{err}"
    );
    assert!(
        out.contains("(incomplete: generate failed, typecheck failed)"),
        "{out}"
    );
    assert_eq!(harness.seen_env.borrow().len(), 4, "every phase starts");
    assert_eq!(
        merged_runs(&harness).len(),
        1,
        "lint merges and typecheck never wrote a capture"
    );
}

#[test]
fn check_later_clean_phases_never_clear_earlier_findings() {
    let harness = umbrella_findings("umbrella-later-clean");
    let (artifact, _) = harness
        .staged_artifacts()
        .into_iter()
        .next()
        .expect("artifact");
    let clean = harness.valid_result(vec![], vec![]);
    let witness = intended_witness("check", true, "", "");
    let mut staged = staged_runner(&harness, vec![(artifact, clean)], Some(witness), 2);
    staged.apply_intended = Some(intended_witness("check", true, "", ""));
    let inv = umbrella_invocation(&harness, &["check", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &staged);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let findings: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_findings")
        .map(|notice| notice["correlation"].as_str().expect("correlation"))
        .collect();
    assert_eq!(findings, vec!["check/format"]);
    let finished = *events_of_kind(&events, "command_finished")
        .last()
        .expect("umbrella finished");
    assert_eq!(finished["exit_code"], serde_json::json!(1));
    assert_eq!(finished["results_complete"], serde_json::json!(true));
}

#[test]
fn check_below_threshold_findings_keep_every_phase_clean() {
    let mut harness = umbrella_clean("umbrella-below-threshold");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![Harness::diagnostic("unused", true)], vec![]),
    );
    let (code, out, err) = harness.run(&["check", "--output=text", "--fail-on=error"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!err.contains("dx: phases"), "{err}");
    assert_eq!(harness.seen_env.borrow().len(), 4);
}

#[test]
fn check_stops_on_a_rejected_scope_before_any_later_phase() {
    let harness = umbrella_findings("umbrella-bad-scope");
    let (code, out, err) = harness.run(&["check", "--output=text", "no/such/dir"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(
        err.contains("dx: phases: generate failed, format skipped"),
        "{err}"
    );
    assert!(harness.seen_env.borrow().is_empty(), "{out}{err}");
}

#[test]
fn check_reports_a_generate_exit_without_changes_as_a_tool_failure() {
    let mut harness = umbrella_clean("umbrella-generate-tool-failure");
    harness.generate_code = Some(1);
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("generate failed") && !err.contains("generate findings"),
        "{out}{err}"
    );
}

#[test]
fn check_reports_a_generate_exit_with_changes_but_no_patch_as_a_tool_failure() {
    let mut harness = umbrella_clean("umbrella-generate-no-patch");
    harness.generate_code = Some(1);
    harness.intended = Some(intended_witness(
        "check",
        true,
        &intended_modify("src/BUILD.bazel", b"old\n", b"new\n"),
        "",
    ));
    let (code, out, err) = harness.run(&["check", "--output=text"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("generate failed") && !err.contains("generate findings"),
        "{out}{err}"
    );
}

#[test]
fn check_reports_generate_drift_first_and_runs_every_phase() {
    let mut harness = umbrella_clean("umbrella-generate-drift");
    harness.generate_code = Some(1);
    harness.patch = Some(b"--- a\n+++ b\n".to_vec());
    harness.intended = Some(intended_witness(
        "check",
        true,
        &intended_modify("src/BUILD.bazel", b"old\n", b"new\n"),
        "",
    ));
    let (code, out, err) = harness.run(&["check", "--output=json"]);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let finished: Vec<(String, i64)> = events_of_kind(&events, "phase_finished")
        .iter()
        .map(|event| {
            (
                event["phase"].as_str().expect("phase").to_owned(),
                event["exit_code"].as_i64().expect("exit"),
            )
        })
        .collect();
    assert_eq!(
        finished,
        [
            ("generate".to_owned(), 1),
            ("format".to_owned(), 0),
            ("lint".to_owned(), 0),
            ("typecheck".to_owned(), 0)
        ]
    );
    let changes = events_of_kind(&events, "change");
    assert_eq!(changes.len(), 1, "{out}");
    assert_eq!(changes[0]["path"], "src/BUILD.bazel");
    assert_eq!(changes[0]["effect"], "proposed");
    assert!(events_of_kind(&events, "mutation").is_empty(), "{out}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"x = 1\n"
    );
    assert!(!harness.workspace.join("src/BUILD.bazel").exists());
    let last = events.last().expect("last");
    assert_eq!(last["exit_code"], 1, "{out}");
    assert_eq!(last["results_complete"], true, "{out}");
}

/// Plays Gazelle for `check --apply`: the first generate launch writes
/// `fixed` to `build` and its apply witness, later generate launches
/// report a clean check, and quality launches fail to compile until
/// `build` lists `source`.
struct StaleBuild<R> {
    inner: R,
    build: PathBuf,
    fixed: Vec<u8>,
    source: &'static str,
    apply_witness: Vec<u8>,
    bep_line: String,
    generate_launches: Cell<usize>,
}

impl<R: Runner> Runner for StaleBuild<R> {
    fn free_disk_bytes(&self, path: &Path) -> Option<u64> {
        self.inner.free_disk_bytes(path)
    }

    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        let status = self.inner.run(argv, cwd, env)?;
        if is_shard_probe(argv) {
            return Ok(status);
        }
        if let Some(path) = env
            .iter()
            .find_map(|(key, value)| (*key == GENERATE_ENV_INTENDED).then_some(*value))
        {
            let launch = self.generate_launches.get();
            self.generate_launches.set(launch + 1);
            if launch == 0 {
                std::fs::write(&self.build, &self.fixed).expect("BUILD");
                std::fs::write(path, &self.apply_witness).expect("witness");
            } else {
                std::fs::write(path, intended_witness("check", true, "", "")).expect("witness");
            }
            return Ok(status);
        }
        let build = std::fs::read_to_string(&self.build).unwrap_or_default();
        if build.contains(self.source) {
            return Ok(status);
        }
        if let Some(bep) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--build_event_json_file="))
        {
            std::fs::write(bep, format!("{}\n{LAST_EVENT}", self.bep_line)).expect("BEP");
        }
        Ok(ChildStatus::exited(1))
    }
}

/// The phase names a JSON umbrella run started, in order.
fn started_phases(events: &[serde_json::Value]) -> Vec<String> {
    events_of_kind(events, "phase_started")
        .iter()
        .map(|event| event["phase"].as_str().expect("phase").to_owned())
        .collect()
}

/// Every generate witness path the runner saw, in launch order.
fn intended_paths(harness: &Harness) -> Vec<String> {
    harness
        .seen_env
        .borrow()
        .iter()
        .flatten()
        .filter(|(key, _)| key == GENERATE_ENV_INTENDED)
        .map(|(_, value)| value.clone())
        .collect()
}

#[test]
fn fix_apply_generates_srcs_before_quality_builds_a_new_source() {
    let mut harness = umbrella_clean("umbrella-fix-new-source");
    let stale = b"rust_library(srcs = [\"lib.rs\"])\n";
    let fixed = b"rust_library(srcs = [\"foo.rs\", \"lib.rs\"])\n".to_vec();
    harness.write_source("src/foo.rs", "pub fn foo() {}\n");
    harness.write_source("src/BUILD.bazel", std::str::from_utf8(stale).expect("utf8"));
    harness.intended = None;
    let uri = stderr_file(&harness, "foo-rustc", RENDERED_E0308);
    let runner = StaleBuild {
        inner: harness.runner(),
        build: harness.workspace.join("src/BUILD.bazel"),
        fixed: fixed.clone(),
        source: "foo.rs",
        apply_witness: intended_witness(
            "default",
            true,
            &intended_modify("src/BUILD.bazel", stale, &fixed),
            "",
        ),
        bep_line: failed_action("//src:lib", "c1", "Rustc", "out/lib.rlib", &uri),
        generate_launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--apply", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/BUILD.bazel")).expect("BUILD"),
        fixed
    );
    let events = json_events(&out);
    assert_eq!(
        started_phases(&events),
        vec!["generate", "format", "lint", "typecheck"],
        "{out}"
    );
    assert_eq!(runner.generate_launches.get(), 2, "apply plus its re-check");
}

#[test]
fn fix_apply_reruns_generate_only_after_a_quality_write() {
    let harness = partial_failure_harness("umbrella-fix-rerun-json");
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let verify_files: Vec<(PathBuf, Vec<u8>)> = harness
        .staged_artifacts()
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    let (code, out, err) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=json"],
    );
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    assert_eq!(
        started_phases(&events),
        vec!["generate", "format", "lint", "typecheck", "rerun/generate"],
        "{out}"
    );
    let rerun: Vec<&serde_json::Value> = events
        .iter()
        .filter(|event| event["correlation"] == "check/rerun/generate")
        .collect();
    assert!(!rerun.is_empty(), "{out}");
    assert!(
        rerun.iter().all(|event| event["phase"] == "rerun/generate"),
        "{out}"
    );
    assert!(
        events_of_kind(&events, "notice")
            .iter()
            .all(|notice| notice["code"] != "umbrella_phase_skipped"),
        "{out}"
    );
}

#[test]
fn fix_apply_without_a_write_runs_generate_once() {
    let mut clean = umbrella_clean("umbrella-no-rerun-clean");
    clean.intended = Some(intended_witness("default", true, "", ""));
    let refused = refused_format("umbrella-no-rerun-refused");
    let findings = umbrella_findings("umbrella-no-rerun-fix");
    let checked = umbrella_findings("umbrella-no-rerun-check");
    for (harness, words) in [
        (&clean, &["check", "--apply", "--output=json"][..]),
        (&refused, &["check", "--apply", "--output=json"][..]),
        (&findings, &["check", "--output=json"][..]),
        (&checked, &["check", "--output=json"][..]),
    ] {
        let (_, out, err) = harness.run(words);
        let events = json_events(&out);
        let started = started_phases(&events);
        assert_eq!(
            &started[..4],
            ["generate", "format", "lint", "typecheck"],
            "{words:?}: {out}{err}"
        );
        assert!(
            !started.iter().any(|phase| phase == "rerun/generate"),
            "{words:?}: {out}"
        );
        assert!(!out.contains("rerun/generate"), "{words:?}: {out}");
    }
}

#[test]
fn check_apply_reruns_generate_after_a_lint_compile_error() {
    let harness = partial_failure_harness("umbrella-fix-stop-no-rerun");
    let mut staged = staged_runner(
        &harness,
        harness.staged_artifacts(),
        Some(intended_witness("check", true, "", "")),
        5,
    );
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    staged.clean_files = staged
        .apply_files
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    staged.clean_from = 1;
    let uri = stderr_file(&harness, "lint-clippy", RENDERED_E0308);
    let runner = CompileFailAt {
        inner: staged,
        at: 2,
        bep_line: failed_action("//app:bin", "c1", "Clippy", "out/bin.clippy", &uri),
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--apply", "--output=json"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(
        std::fs::read(harness.workspace.join("src/a.py")).expect("source"),
        b"y = 1\n",
        "format applied before lint stopped the run"
    );
    let events = json_events(&out);
    assert_eq!(
        started_phases(&events),
        vec!["generate", "format", "lint", "rerun/generate"],
        "{out}"
    );
    let skipped: Vec<&str> = events_of_kind(&events, "notice")
        .iter()
        .filter(|notice| notice["code"] == "umbrella_phase_skipped")
        .filter_map(|notice| notice["message"].as_str())
        .collect();
    assert_eq!(
        skipped,
        vec!["typecheck skipped after lint failed to compile"]
    );
    assert_eq!(runner.launches.get(), 4, "{out}");
}

#[test]
fn fix_apply_verification_nonces_differ_from_the_rerun() {
    let harness = partial_failure_harness("umbrella-fix-rerun-nonces");
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let verify_files: Vec<(PathBuf, Vec<u8>)> = harness
        .staged_artifacts()
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    let (code, out, err) = fix_apply_format_edit_then_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=text"],
    );
    assert_eq!(code, 0, "{out}{err}");
    let paths = intended_paths(&harness);
    assert_eq!(
        paths.len(),
        3,
        "generate, rerun and verify/generate: {paths:?}"
    );
    let distinct: std::collections::BTreeSet<&String> = paths.iter().collect();
    assert_eq!(distinct.len(), paths.len(), "{paths:?}");
}

#[test]
fn fix_apply_failed_rerun_is_not_rechecked() {
    let harness = partial_failure_harness("umbrella-fix-rerun-fails");
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let mut staged = staged_runner(
        &harness,
        harness
            .staged_artifacts()
            .iter()
            .map(|(path, _)| (path.clone(), clean.clone()))
            .collect(),
        Some(intended_witness("check", true, "", "")),
        5,
    );
    staged.clean_files = staged
        .apply_files
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    staged.clean_from = 1;
    let runner = FailFrom {
        inner: staged,
        from: 4,
        to: Some(5),
        code: 1,
        launches: Cell::new(0),
    };
    let inv = umbrella_invocation(&harness, &["check", "--apply", "--output=text"]);
    let (code, out, err) = harness.execute_with(&inv, &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains(
            "verification passed after apply for format; not re-checked, \
             failed during apply: rerun/generate"
        ),
        "{err}"
    );
    assert!(err.contains("rerun/generate failed"), "{err}");
    assert_eq!(runner.launches.get(), 6, "{out}{err}");
}

#[test]
fn phase_kind_stops_on_selection_host_cancel_or_failed_generate() {
    for kind in [
        PhaseKind::Clean,
        PhaseKind::Findings,
        PhaseKind::NonConvergence,
        PhaseKind::Tool,
        PhaseKind::Incomplete,
        PhaseKind::Report,
    ] {
        assert!(kind.continues(false), "{kind:?}");
        assert!(kind.continues(true), "{kind:?}");
        assert!(!kind.stop_reason().is_empty());
    }
    for kind in [
        PhaseKind::Selection,
        PhaseKind::Host,
        PhaseKind::Cancelled,
        PhaseKind::GenerateFailed,
    ] {
        assert!(!kind.continues(false), "{kind:?}");
        assert!(!kind.continues(true), "{kind:?}");
        assert!(!kind.stop_reason().is_empty());
    }
    assert!(PhaseKind::Compile.continues(false));
    assert!(!PhaseKind::Compile.continues(true));
    assert_eq!(PhaseKind::Compile.stop_reason(), "failed to compile");
}

#[test]
fn parse_capture_keeps_only_well_formed_documents() {
    let document = dx_plan::reports::render_sarif(
        &["lint-tool".to_owned()],
        &[],
        &std::collections::BTreeMap::new(),
        true,
    )
    .expect("render");
    let parsed = parse_capture(document.as_bytes()).expect("round trip");
    assert_eq!(parsed["version"], serde_json::json!("2.1.0"));
    assert_eq!(parse_capture(b"   "), Err(CaptureState::Malformed));
    assert_eq!(parse_capture(b"null"), Err(CaptureState::Malformed));
    assert_eq!(
        parse_capture(br#"{"runs":[],"$schema":"x"}"#),
        Err(CaptureState::Malformed),
        "a run needs a tool"
    );
    assert_eq!(
        parse_capture(br#"{"runs":[]}"#),
        Err(CaptureState::Malformed),
        "a document without a version is not SARIF 2.1.0"
    );
}

fn graph_calls(harness: &Harness) -> usize {
    harness
        .query
        .calls
        .borrow()
        .iter()
        .filter(|argv| argv.last().map(String::as_str) == Some("kind('rule', //...)"))
        .count()
}

fn pkg_harness(name: &str) -> Harness {
    let harness = umbrella_clean(name);
    harness.write_source("pkg/BUILD.bazel", "");
    harness.write_source("pkg/a.py", "x = 1\n");
    harness
}

#[test]
fn check_loads_the_rule_graph_once_across_phases() {
    let harness = pkg_harness("umbrella-graph-once-check");
    harness.query.script_graph(&[
        graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
        graph_rule("//pkg:extra", &["//pkg:a.py"], &[]),
    ]);
    let (code, out, err) = harness.run(&["check", "--output=text", "pkg/a.py"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(harness.seen_env.borrow().len(), 4, "every phase ran");
    assert_eq!(
        out.matches("for //pkg:extra //pkg:lib").count(),
        3,
        "every analysis phase plans the same owners: {out}"
    );
    assert!(out.contains("Running generate for //pkg/..."), "{out}");
    assert_eq!(graph_calls(&harness), 1, "one graph load for the run");
}

#[test]
fn fix_without_changes_loads_the_rule_graph_once() {
    let harness = pkg_harness("umbrella-graph-once-fix");
    harness
        .query
        .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
    let (code, out, err) = harness.run(&["check", "--output=text", "pkg/a.py"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(graph_calls(&harness), 1, "default fix never writes");
}

#[test]
fn fix_apply_reloads_the_rule_graph_after_a_write() {
    let mut harness = pkg_harness("umbrella-graph-reload");
    harness.results.insert(
        "//test:corpus".to_owned(),
        harness.valid_result(vec![], vec![harness.replacement_in("pkg/a.py", b"y")]),
    );
    harness.intended = Some(intended_witness("default", true, "", ""));
    harness
        .query
        .script_graph(&[graph_rule("//pkg:lib", &["//pkg:a.py"], &[])]);
    harness.query.script_graph(&[
        graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
        graph_rule("//pkg:new", &["//pkg:a.py"], &[]),
    ]);
    harness.query.script_graph(&[
        graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
        graph_rule("//pkg:new", &["//pkg:a.py"], &[]),
    ]);
    harness.query.script_graph(&[
        graph_rule("//pkg:lib", &["//pkg:a.py"], &[]),
        graph_rule("//pkg:new", &["//pkg:a.py"], &[]),
    ]);
    let staged = harness.staged_artifacts();
    let clean = harness.result_full(vec![], vec![], vec![], vec![]);
    let verify_files: Vec<(PathBuf, Vec<u8>)> = staged
        .iter()
        .map(|(path, _)| (path.clone(), clean.clone()))
        .collect();
    let (_, out, err) = fix_apply_with_staged_verify(
        &harness,
        verify_files,
        Some(intended_witness("check", true, "", "")),
        5,
        &["check", "--apply", "--output=text", "pkg/a.py"],
    );
    assert!(
        graph_calls(&harness) >= 2,
        "a phase that applied edits invalidates the graph: {out}{err}"
    );
    assert!(
        out.contains("//pkg:new"),
        "later plans see the new owner: {out}"
    );
}
