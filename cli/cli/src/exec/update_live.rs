use super::super::test_support::*;
use dx_process::{ChildStatus, Runner};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::rc::Rc;

pub(super) struct ScriptRunner {
    pub(super) calls: Rc<RefCell<Vec<Vec<String>>>>,
    pub(super) codes: RefCell<HashMap<String, Option<i32>>>,
    pub(super) io_error: bool,
    pub(super) hermetic_envs: RefCell<Vec<Vec<(String, String)>>>,
    pub(super) pinned: bool,
    pub(super) version: Option<String>,
    pub(super) launches: RefCell<Vec<Launch>>,
    /// Files the next launch writes, as a backend that changes the tree would.
    pub(super) writes: RefCell<Vec<(std::path::PathBuf, Vec<u8>)>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Launch {
    pub(super) argv: Vec<String>,
    pub(super) cwd: std::path::PathBuf,
    pub(super) env: Vec<(String, String)>,
    pub(super) hermetic: bool,
}

pub(super) fn fake_pinned(name: &str) -> String {
    if cfg!(windows) {
        format!("C:\\hermetic\\{name}")
    } else {
        format!("/hermetic/{name}")
    }
}

impl ScriptRunner {
    pub(super) fn new(codes: &[(&str, Option<i32>)]) -> Self {
        ScriptRunner {
            calls: Rc::new(RefCell::new(Vec::new())),
            codes: RefCell::new(
                codes
                    .iter()
                    .map(|(key, code)| ((*key).to_owned(), *code))
                    .collect(),
            ),
            io_error: false,
            hermetic_envs: RefCell::new(Vec::new()),
            pinned: true,
            version: Some(dx_update::backend::UV_VERSION_LINE.to_owned()),
            launches: RefCell::new(Vec::new()),
            writes: RefCell::new(Vec::new()),
        }
    }

    fn launch(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
        hermetic: bool,
    ) -> io::Result<ChildStatus> {
        if self.io_error {
            return Err(io::Error::other("fake launch failure"));
        }
        self.launches.borrow_mut().push(Launch {
            argv: argv.to_vec(),
            cwd: cwd.to_path_buf(),
            env: env
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            hermetic,
        });
        self.calls.borrow_mut().push(argv.to_vec());
        for (path, bytes) in self.writes.borrow_mut().drain(..) {
            std::fs::write(&path, bytes)?;
        }
        let key = Self::key_for(argv);
        let code = self.codes.borrow().get(&key).copied().unwrap_or(Some(0));
        Ok(ChildStatus { code, signal: None })
    }

    pub(super) fn key_for(argv: &[String]) -> String {
        if argv.first() == Some(&fake_pinned("uv")) {
            let dir = argv
                .iter()
                .skip_while(|arg| *arg != "--directory")
                .nth(1)
                .cloned()
                .unwrap_or_default();
            format!("consumer-uv:{dir}")
        } else if let Some(dir) = argv
            .contains(&"@pnpm//:pnpm".to_owned())
            .then(|| argv.iter().skip_while(|arg| *arg != "--dir").nth(1))
            .flatten()
            .filter(|dir| dir.replace('\\', "/").contains("/consumer-npm/"))
        {
            let name = Path::new(dir)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("consumer-npm:{name}")
        } else if let Some(repository) = argv
            .iter()
            .find_map(|arg| arg.strip_prefix("--repo=@"))
            .filter(|_| argv.contains(&"fetch".to_owned()))
        {
            format!("consumer-cargo:{repository}")
        } else if argv.contains(&"//rust/tests/fixtures/hello:hello".to_owned()) {
            "cargo".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("quality/tools/javascript"))
        {
            "npm-tools".to_owned()
        } else if argv.contains(&"@pnpm//:pnpm".to_owned()) {
            "npm".to_owned()
        } else if argv.contains(&"@maven//:pin".to_owned()) {
            "maven".to_owned()
        } else if argv.iter().any(|arg| arg.contains("paket2bazel")) {
            "nuget".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("python/tests/fixtures/hello"))
        {
            "uv".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv.iter().any(|arg| arg.contains("quality/tools/python"))
        {
            "uv-tools".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv.iter().any(|arg| arg.contains("examples/adopt-python"))
        {
            "uv-adopt".to_owned()
        } else if argv.contains(&"uv".to_owned())
            && argv
                .iter()
                .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "uv-adopt-polyglot".to_owned()
        } else if argv.iter().any(|arg| arg.contains("examples/adopt-js-ts")) {
            "npm-adopt".to_owned()
        } else if argv
            .iter()
            .any(|arg| arg.contains("examples/adopt-polyglot"))
        {
            "npm-adopt-polyglot".to_owned()
        } else {
            argv.join(" ")
        }
    }
}

impl Runner for ScriptRunner {
    fn run(&self, argv: &[String], cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.launch(argv, cwd, env, false)
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.hermetic_envs.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        self.launch(argv, cwd, env, true)
    }

    fn pinned_tool(&self, var: &str) -> Option<std::path::PathBuf> {
        let name = match var {
            "DX_UV_BIN" => "uv",
            "DX_UV_PYTHON" => "python3",
            _ => return None,
        };
        self.pinned.then(|| fake_pinned(name).into())
    }

    fn tool_version(&self, _path: &Path) -> Option<String> {
        self.version.clone()
    }
}

pub(super) fn run_with(argv: &[&str], runner: &ScriptRunner) -> (i32, String, String) {
    use dx_plan::args::parse;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    let invocation = parse(&words).expect("parse");
    let harness = Harness::new(&format!("update-live-{id}"));
    harness.execute_with(&invocation, runner)
}

#[test]
pub(super) fn dry_run_rejects_unknown_and_unowned() {
    let harness = Harness::new("update-dryrun-unknown");
    let (code, _, err) = harness.run(&["update", "crates"]);
    assert_eq!(code, 2, "{err}");
    let harness = Harness::new("update-dryrun-unowned");
    let (code, _, err) = harness.run(&["update", "docs/cli/README.md"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
pub(super) fn live_all_success_reports_per_set_and_exits_zero() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for all dependency sets"),
        "{out}"
    );
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(out.contains("updated npm-tools ("), "{out}");
    assert!(out.contains("updated maven ("), "{out}");
    assert!(out.contains("updated nuget ("), "{out}");
    assert!(out.contains("updated uv ("), "{out}");
    assert!(out.contains("updated uv-tools ("), "{out}");
    assert!(out.contains("updated npm-adopt ("), "{out}");
    assert!(out.contains("updated uv-adopt ("), "{out}");
    assert!(out.contains("go pins are manual"), "{out}");
    assert!(out.contains("ruby pins are manual"), "{out}");
    assert!(out.contains("powershell pins are manual"), "{out}");
    assert!(out.contains("nothing to resolve"), "{out}");
    assert!(out.contains("dx update: 11 updated, 3 pinned"), "{out}");
    assert_eq!(err, "", "{err}");
    assert_eq!(runner.calls.borrow().len(), 11);
}

#[test]
pub(super) fn live_independent_failure_preserves_success_and_exits_one() {
    let runner = ScriptRunner::new(&[("maven", Some(1))]);
    let (code, out, err) = run_with(&["update", "--apply"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("updated cargo ("), "{out}");
    assert!(out.contains("updated npm ("), "{out}");
    assert!(
        out.contains("dx update: 10 updated, 3 pinned, 1 failed"),
        "{out}"
    );
    assert!(err.contains("update_failed"), "{err}");
    assert!(err.contains("failed to update maven"), "{err}");
    assert!(err.contains("update_recovery"), "{err}");
    assert!(err.contains("rerun `dx update --apply maven`"), "{err}");
    assert_eq!(runner.calls.borrow().len(), 11);
}

#[test]
pub(super) fn live_selective_npm_runs_once_with_packages() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "npm:jest", "npm:react"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("Running update for npm:jest, npm:react"),
        "{out}"
    );
    assert!(
        out.contains("updated npm:jest, npm:react (pnpm-lock.yaml)"),
        "{out}"
    );
    assert_eq!(runner.calls.borrow().len(), 1);
    assert!(runner.calls.borrow()[0].contains(&"jest".to_owned()));
    assert!(runner.calls.borrow()[0].contains(&"react".to_owned()));
}

#[test]
#[allow(clippy::disallowed_methods)] // test asserts the launcher head
pub(super) fn live_bazel_backends_carry_startup_options() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(
        &[
            "update",
            "--bazel-startup-option=--output_base=/tmp/a",
            "--apply",
            "maven",
            "npm",
        ],
        &runner,
    );
    assert_eq!(code, 0, "{out}{err}");
    let head = dx_process::startup_argv(&["--output_base=/tmp/a".to_owned()]);
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 2);
    for argv in calls.iter() {
        assert!(argv.starts_with(&head), "{argv:?}");
        let verb = argv.iter().position(|arg| arg == "run").expect("verb");
        let config = argv
            .iter()
            .position(|arg| arg == "--config=dx_dev")
            .expect("profile config");
        assert!(config > verb, "{argv:?}");
    }
}

#[test]
pub(super) fn live_unsupported_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "cargo:anyhow"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_set_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_nuget_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "--apply", "nuget:FSharp.Core"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_set_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("rerun `dx update --apply nuget`"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_unsupported_go_selective_fails_without_launch() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(
        &["update", "--apply", "go:github.com/google/go-cmp/cmp"],
        &runner,
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("update_set_unsupported"), "{err}");
    assert!(err.contains("unsupported"), "{err}");
    assert!(err.contains("dx bump go:"), "{err}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_target_resolves_to_owning_set_only() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(
        &["update", "--apply", "//go/tests/fixtures/hello:hello"],
        &runner,
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running update for go"), "{out}");
    assert!(out.contains("go pins are manual"), "{out}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
pub(super) fn live_json_emits_per_set_notices_and_finished() {
    let runner = ScriptRunner::new(&[("npm", Some(2))]);
    let (code, out, err) = run_with(&["update", "--apply", "--output=json"], &runner);
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
    assert!(kinds.contains(&"notice"));
    assert!(kinds.contains(&"error"));
    assert!(err.contains("update_failed"), "{err}");
    assert!(out.contains("\"code\":\"update_recovery\""), "{out}");
    assert!(err.contains("update_recovery"), "{err}");
    for event in &events {
        let code = event
            .get("code")
            .and_then(|code| code.as_str())
            .unwrap_or("");
        if code == "update_set_success"
            || code == "update_set_blocked"
            || event.get("code").is_none() && event["event"] == serde_json::json!("error")
        {
            let correlation = event["correlation"].as_str().expect("correlation");
            assert!(correlation.starts_with("update:"), "{event}");
        }
    }
    assert!(out.contains("\"correlation\":\"update:cargo\""), "{out}");
    assert!(out.contains("\"correlation\":\"update:npm\""), "{out}");
}

#[test]
pub(super) fn default_checks_like_explicit_check_and_apply_updates() {
    let runner = ScriptRunner::new(&[]);
    let (code, out, err) = run_with(&["update", "uv"], &runner);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
    assert!(
        runner.calls.borrow()[0].contains(&"--check".to_owned()),
        "default must run check backend: {:?}",
        runner.calls.borrow()[0]
    );
    let runner_check = ScriptRunner::new(&[]);
    let (code_check, out_check, err_check) = run_with(&["update", "--check", "uv"], &runner_check);
    assert_eq!(code, code_check);
    assert_eq!(out, out_check);
    assert_eq!(err, err_check);
    let runner_apply = ScriptRunner::new(&[]);
    let (code_apply, _, _) = run_with(&["update", "--apply", "uv"], &runner_apply);
    assert_eq!(code_apply, 0);
    assert_eq!(runner_apply.calls.borrow().len(), 1);
    assert!(
        !runner_apply.calls.borrow()[0].contains(&"--check".to_owned()),
        "apply must run update backend: {:?}",
        runner_apply.calls.borrow()[0]
    );
}

fn run_with_busy_slot(argv: &[&str], json: bool) -> (i32, String, String) {
    let scratch = dx_test_scratch::scratch("dx-update-slot-");
    let dir = scratch.path().join("slots");
    let held = dx_plan::slots::acquire(&dir, 1, "tree-0", &mut |_| {}).expect("fill");
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(600));
        drop(held);
    });
    let gate = dx_plan::slots::Gate::new(dir, 1, "me".to_owned(), json);
    let runner = ScriptRunner::new(&[]);
    let result = dx_plan::slots::with_gate(Some(gate), || run_with(argv, &runner));
    releaser.join().expect("join");
    result
}

#[test]
pub(super) fn bazel_backends_see_the_held_slot() {
    let marked = (dx_plan::slots::SLOT_ENV_VAR.to_owned(), "1".to_owned());
    for (set, bazel) in [("cargo", true), ("uv", false)] {
        let scratch = dx_test_scratch::scratch("dx-update-slot-env-");
        let gate =
            dx_plan::slots::Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), false);
        let runner = ScriptRunner::new(&[]);
        let (code, out, err) = dx_plan::slots::with_gate(Some(gate), || {
            run_with(&["update", "--apply", set], &runner)
        });
        assert_eq!(code, 0, "{out}{err}");
        let launches = runner.launches.borrow();
        assert!(!launches.is_empty(), "{set}");
        for launch in launches.iter() {
            assert_eq!(launch.env.contains(&marked), bazel, "{set}: {launch:?}");
        }
    }
}

#[test]
pub(super) fn apply_takes_a_slot_only_for_bazel_backends() {
    let (code, out, err) = run_with_busy_slot(&["update", "--apply", "cargo"], false);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        err.contains("waiting for a build slot (held by tree-0)"),
        "{err}"
    );
    for argv in [
        &["update", "--apply", "uv"][..],
        &["update", "uv"][..],
        &["update", "--check"][..],
    ] {
        let (_, out, err) = run_with_busy_slot(argv, false);
        assert!(!err.contains("waiting for a build slot"), "{argv:?}: {err}");
        assert!(!out.contains("waiting for a build slot"), "{argv:?}: {out}");
    }
    let (code, out, err) =
        run_with_busy_slot(&["update", "--apply", "--output=json", "cargo"], true);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out.matches("waiting_for_build_slot").count(), 1, "{out}");
    let started = out.find("\"command_started\"").expect("command_started");
    let waiting = out.find("waiting_for_build_slot").expect("notice");
    assert!(started < waiting, "{out}");
}
