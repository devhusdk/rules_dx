use super::super::test_support::Harness;
use quality_result::proto::FileSnapshot;
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const MODULE: &str = "module(name = \"acme_renamed\", version = \"1.2.3\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n";

const FAKE_BAZEL: &str = r#"#!/bin/sh
case " $* " in *" --output_groups=dx_shards "*) exit 1 ;; esac
printf '%s\n' "$*" >> "$FAKE_LOG"
case " $* " in
  *" query "*|*" cquery "*)
    case "$FAKE_QUERY" in
      fail) printf 'Loading: 1 package\nINFO: still loading\nERROR: no such target //src:gone\n' >&2; exit 7 ;;
      owner) printf '{"type":"RULE","rule":{"name":"//src:bin","ruleInput":["//:src/main.rs"]}}\n'; exit 0 ;;
      sleep) : > "$FAKE_LOG.started"; sleep 30; exit 0 ;;
      *) exit 0 ;;
    esac ;;
esac
for arg in "$@"; do
  case "$arg" in --build_event_json_file=*) bep="${arg#--build_event_json_file=}" ;; esac
done
case "$FAKE_BUILD" in
  findings)
    if [ -n "$FAKE_GATE_AT" ] && [ "$(grep -c ' build ' "$FAKE_LOG")" -ge "$FAKE_GATE_AT" ]; then
      : > "$FAKE_LOG.started"
      while [ ! -e "$FAKE_LOG.gate" ]; do sleep 0.02; done
    fi
    if [ -n "$DX_GENERATE_INTENDED" ]; then printf '%s' "$FAKE_WITNESS" > "$DX_GENERATE_INTENDED"; fi
    if [ -n "$bep" ]; then
      printf '{"id":{"namedSet":{"id":"0"}},"namedSetOfFiles":{"files":[{"uri":"file://%s"}]}}\n' "$FAKE_RESULT" > "$bep"
      printf '{"id":{"targetCompleted":{"label":"//test:corpus"}},"completed":{"success":true,"outputGroup":[{"name":"dx_results","fileSets":[{"id":"0"}]}]}}\n' >> "$bep"
      printf '{"id":{"buildMetrics":{}},"lastMessage":true}\n' >> "$bep"
    fi ;;
  badbep) printf 'not json\n' > "$bep" ;;
  done) printf '{"id":{"buildMetrics":{}},"lastMessage":true}\n' > "$bep" ;;
  nobep) rm -f "$bep" ;;
  cutbep) printf '%s\n{"id":{"testRes' '{"id":{"started":{}},"started":{}}' > "$bep" ;;
  sleep) if [ -n "$bep" ]; then : > "$bep"; fi; : > "$FAKE_LOG.started"; sleep 30 ;;
  *) : > "$bep" ;;
esac
exit "${FAKE_EXIT:-0}"
"#;

struct Consumer {
    base: PathBuf,
    _scratch: dx_test_scratch::TempDir,
}

struct Outcome {
    status: ExitStatus,
    out: String,
    err: String,
    bazel: Vec<String>,
}

impl Outcome {
    fn events(&self) -> Vec<serde_json::Value> {
        self.out
            .lines()
            .map(|line| serde_json::from_str(line).expect("stdout is NDJSON only"))
            .collect()
    }

    fn kinds(&self) -> Vec<String> {
        self.events()
            .iter()
            .map(|event| event["event"].as_str().expect("event").to_owned())
            .collect()
    }

    fn built(&self) -> bool {
        self.bazel
            .iter()
            .any(|line| line.contains(" build ") || line.contains(" test "))
    }

    fn context(&self) -> String {
        format!(
            "status {:?}\nstdout:\n{}\nstderr:\n{}\nbazel:\n{}",
            self.status,
            self.out,
            self.err,
            self.bazel.join("\n")
        )
    }
}

impl Consumer {
    fn new() -> Self {
        let scratch = dx_test_scratch::scratch("dx-error-boundary-");
        let base = scratch.path().to_path_buf();
        let workspace = base.join("ws");
        for dir in [
            workspace.join(".dx"),
            workspace.join("src"),
            base.join("bin"),
            base.join("nobin"),
            base.join("tmp"),
        ] {
            fs::create_dir_all(dir).expect("dir");
        }
        fs::write(workspace.join("MODULE.bazel"), MODULE).expect("module");
        fs::write(workspace.join(".dx/version"), "0.0.0\n").expect("pin");
        fs::write(workspace.join("BUILD.bazel"), "").expect("build file");
        fs::write(workspace.join("src/main.rs"), "fn main() {}\n").expect("source");
        let bazel = base.join("bin/bazel");
        fs::write(&bazel, FAKE_BAZEL).expect("fake bazel");
        fs::set_permissions(&bazel, fs::Permissions::from_mode(0o755)).expect("chmod");
        Consumer {
            base,
            _scratch: scratch,
        }
    }

    fn workspace(&self) -> PathBuf {
        self.base.join("ws")
    }

    fn log(&self) -> PathBuf {
        self.base.join("bazel.log")
    }

    fn command(&self, args: &[&str], env: &[(&str, &str)], bazel: bool) -> Command {
        let bin = self.base.join(if bazel { "bin" } else { "nobin" });
        let _ = fs::remove_file(self.log());
        let _ = fs::remove_file(self.base.join("bazel.log.gate"));
        let _ = fs::remove_file(self.base.join("bazel.log.started"));
        let mut command = Command::new(dx_testing::runfiles_root().join("cli/cli/dx"));
        command
            .args(args)
            .current_dir(self.workspace())
            .env_clear()
            .env("BAZEL_TEST", "1")
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HOME", &self.base)
            .env("TMPDIR", self.base.join("tmp"))
            .env("FAKE_LOG", self.log())
            .stdin(Stdio::null());
        for (key, value) in env {
            command.env(key, value);
        }
        command
    }

    fn bazel_calls(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(|line| format!(" {line} "))
            .collect()
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)], bazel: bool) -> Outcome {
        let output = self.command(args, env, bazel).output().expect("dx runs");
        Outcome {
            status: output.status,
            out: String::from_utf8(output.stdout).expect("stdout"),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
            bazel: self.bazel_calls(),
        }
    }

    fn run_closed_stdout(&self, args: &[&str], env: &[(&str, &str)]) -> Outcome {
        let mut fds = [0; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe");
        unsafe { libc::close(fds[0]) };
        let stdout = unsafe { <Stdio as std::os::fd::FromRawFd>::from_raw_fd(fds[1]) };
        let output = self
            .command(args, env, true)
            .stdout(stdout)
            .stderr(Stdio::piped())
            .output()
            .expect("dx runs");
        Outcome {
            status: output.status,
            out: String::new(),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
            bazel: self.bazel_calls(),
        }
    }

    fn stage_findings(&self) -> (String, String) {
        let source = b"x = 1\n";
        fs::write(self.workspace().join("src/a.py"), source).expect("source");
        let snapshot = vec![FileSnapshot {
            path: "src/a.py".to_owned(),
            digest: dx_digest::blake3(source).to_vec(),
        }];
        let result = Harness::new("launcher-findings").result_full(
            vec![Harness::diagnostic("unused", false)],
            vec![],
            vec![],
            snapshot,
        );
        let path = self.base.join("findings.pb");
        fs::write(&path, result).expect("result");
        let witness = serde_json::json!({
            "schema_major": 1,
            "schema_minor": 1,
            "mode": "check",
            "result": "clean",
            "scopes": [{"value": "//...", "results_complete": true}],
            "files": [],
            "ignored_imports": [],
        });
        (path.display().to_string(), witness.to_string())
    }

    fn run_hung_up_after_findings(&self, args: &[&str], env: &[(&str, &str)]) -> Outcome {
        let started = self.base.join("bazel.log.started");
        let mut child = self
            .command(args, env, true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("dx starts");
        let pgid = child.id() as i32;
        let done = Arc::new(AtomicBool::new(false));
        let watchdog = Arc::clone(&done);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(90));
            if !watchdog.load(Ordering::SeqCst) {
                unsafe { libc::kill(-pgid, libc::SIGKILL) };
            }
        });
        let mut reader = BufReader::new(child.stdout.take().expect("stdout"));
        let mut seen = String::new();
        loop {
            let mut line = String::new();
            let read = reader.read_line(&mut line).expect("read stdout");
            assert!(read > 0, "stdout ended before any finding:\n{seen}");
            seen.push_str(&line);
            if line.contains("\"event\":\"diagnostic\"") {
                break;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        while !started.exists() {
            assert!(Instant::now() < deadline, "bazel never reached the gate");
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(reader);
        fs::write(self.base.join("bazel.log.gate"), "").expect("gate");
        let output = child.wait_with_output().expect("dx stops");
        done.store(true, Ordering::SeqCst);
        unsafe { libc::kill(-pgid, libc::SIGKILL) };
        Outcome {
            status: output.status,
            out: seen,
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
            bazel: self.bazel_calls(),
        }
    }

    fn run_cancelled(&self, args: &[&str], env: &[(&str, &str)], signo: i32) -> Outcome {
        let started = self.base.join("bazel.log.started");
        let _ = fs::remove_file(&started);
        let child = self
            .command(args, env, true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .expect("dx starts");
        let deadline = Instant::now() + Duration::from_secs(20);
        while !started.exists() {
            assert!(Instant::now() < deadline, "bazel never started");
            std::thread::sleep(Duration::from_millis(20));
        }
        let pgid = child.id() as i32;
        assert_eq!(unsafe { libc::kill(-pgid, signo) }, 0, "signal delivered");
        let output = child.wait_with_output().expect("dx stops");
        unsafe { libc::kill(-pgid, libc::SIGKILL) };
        Outcome {
            status: output.status,
            out: String::from_utf8(output.stdout).expect("stdout"),
            err: String::from_utf8_lossy(&output.stderr).into_owned(),
            bazel: self.bazel_calls(),
        }
    }
}

const JSON_MODES: [(&[&str], &[(&str, &str)]); 4] = [
    (&["--output=json"], &[]),
    (&["--output=json", "-q"], &[]),
    (&[], &[("DX_OUTPUT", "json")]),
    (&["-q"], &[("DX_OUTPUT", "json")]),
];

fn with_mode<'a>(args: &[&'a str], flags: &[&'a str]) -> Vec<&'a str> {
    let mut words = args.to_vec();
    words.extend_from_slice(flags);
    words
}

fn assert_terminal_failure(outcome: &Outcome, code: &str, cause: &str, exit: i32) {
    let context = outcome.context();
    assert_eq!(outcome.status.code(), Some(exit), "{context}");
    let events = outcome.events();
    let kinds = outcome.kinds();
    assert!(kinds.len() >= 2, "{context}");
    assert_eq!(
        kinds[kinds.len() - 2..],
        ["error", "command_finished"],
        "{context}"
    );
    assert_eq!(
        kinds.iter().filter(|kind| *kind == "error").count(),
        1,
        "{context}"
    );
    let error = &events[events.len() - 2];
    assert_eq!(error["code"], code, "{context}");
    assert_eq!(error["cause"], cause, "{context}");
    let finished = &events[events.len() - 1];
    assert_eq!(finished["exit_code"], exit, "{context}");
    assert_eq!(finished["results_complete"], false, "{context}");
    assert!(
        outcome.err.contains(
            error["message"]
                .as_str()
                .expect("message")
                .lines()
                .next()
                .expect("line")
        ),
        "{context}"
    );
}

#[test]
fn input_and_default_failures_end_in_one_error_before_any_bazel_call() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        for (args, code) in [
            (vec!["build", "no/such/path"], "invalid_scope"),
            (vec!["run"], "invalid_scope"),
            (vec!["build", "--cwd", "src"], "invalid_arguments"),
            (vec!["build", "--dry-run"], "invalid_arguments"),
        ] {
            let outcome = consumer.run(&with_mode(&args, flags), env, true);
            assert_terminal_failure(&outcome, code, "input", 2);
            assert!(outcome.bazel.is_empty(), "{}", outcome.context());
            assert_eq!(outcome.kinds(), ["error", "command_finished"]);
        }
    }
    let outcome = consumer.run(
        &["build"],
        &[("DX_OUTPUT", "json"), ("DX_DRY_RUN", "1")],
        true,
    );
    assert_terminal_failure(&outcome, "invalid_defaults", "defaults", 2);
    assert!(outcome.bazel.is_empty(), "{}", outcome.context());
}

#[test]
fn malformed_defaults_keep_the_json_intent_they_name() {
    let consumer = Consumer::new();
    fs::write(
        consumer.workspace().join("dx.toml"),
        "[dx]\noutput = \"json\"\nlog-level = \"debug\"\n",
    )
    .expect("config");
    for flags in [&[][..], &["-q"][..]] {
        let outcome = consumer.run(&with_mode(&["build", "//..."], flags), &[], true);
        assert_terminal_failure(&outcome, "invalid_defaults", "defaults", 2);
        assert_eq!(outcome.kinds(), ["error", "command_finished"]);
        assert!(outcome.err.contains("log-level"), "{}", outcome.context());
        assert!(outcome.bazel.is_empty(), "{}", outcome.context());
    }
}

#[test]
fn unsupported_output_defaults_fall_back_per_command() {
    let consumer = Consumer::new();
    let config = consumer.workspace().join("dx.toml");
    fs::write(&config, "[dx]\noutput = \"json\"\n").expect("config");
    for (args, usage) in [
        (&["completion"][..], "Usage: dx completion "),
        (&["init", "a", "b"][..], "Usage: dx init "),
    ] {
        let outcome = consumer.run(args, &[], false);
        assert_eq!(outcome.status.code(), Some(2), "{}", outcome.context());
        assert!(outcome.out.is_empty(), "{}", outcome.context());
        assert!(outcome.err.starts_with("dx: "), "{}", outcome.context());
        assert!(outcome.err.contains(usage), "{}", outcome.context());
    }
    fs::write(&config, "[dx]\noutput = \"path\"\n").expect("config");
    let outcome = consumer.run(&["build", "//..."], &[("FAKE_BUILD", "done")], true);
    assert!(
        !outcome.err.contains("unknown --output"),
        "{}",
        outcome.context()
    );
    assert!(outcome.built(), "{}", outcome.context());
}

#[test]
fn agent_sessions_keep_text_only_failures_in_text() {
    let consumer = Consumer::new();
    let agent = [("CLAUDECODE", "1")];
    let outcome = consumer.run(&["deploy", "--bogus"], &agent, false);
    assert_eq!(outcome.status.code(), Some(2), "{}", outcome.context());
    assert!(outcome.out.is_empty(), "{}", outcome.context());
    assert!(outcome.err.starts_with("dx: "), "{}", outcome.context());
    assert!(
        outcome.err.contains("Usage: dx deploy "),
        "{}",
        outcome.context()
    );
    fs::write(
        consumer.workspace().join("dx.toml"),
        "[dx]\nlog-level = \"debug\"\n",
    )
    .expect("config");
    let outcome = consumer.run(&["init"], &agent, false);
    assert_eq!(outcome.status.code(), Some(2), "{}", outcome.context());
    assert!(outcome.out.is_empty(), "{}", outcome.context());
    assert!(outcome.err.contains("log-level"), "{}", outcome.context());
    let outcome = consumer.run(&["lint", "//..."], &agent, true);
    assert_terminal_failure(&outcome, "invalid_defaults", "defaults", 2);
    assert!(outcome.bazel.is_empty(), "{}", outcome.context());
}

#[test]
fn owned_and_unowned_file_scopes_resolve_before_building() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let unowned = consumer.run(&with_mode(&["build", "src/main.rs"], flags), env, true);
        assert_terminal_failure(&unowned, "no_owner", "input", 2);
        assert_eq!(unowned.kinds(), ["error", "command_finished"]);
        assert!(!unowned.built(), "{}", unowned.context());
        let mut owner_env = env.to_vec();
        owner_env.push(("FAKE_QUERY", "owner"));
        let owned = consumer.run(
            &with_mode(&["build", "src/main.rs"], flags),
            &owner_env,
            true,
        );
        assert_eq!(owned.status.code(), Some(0), "{}", owned.context());
        assert_eq!(owned.kinds(), ["command_started", "command_finished"]);
        assert!(
            owned
                .bazel
                .last()
                .is_some_and(|line| line.contains(" build ") && line.ends_with(" //src:bin ")),
            "{}",
            owned.context()
        );
    }
}

#[test]
fn a_failed_query_keeps_its_status_and_excerpt_and_never_builds() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let mut fail_env = env.to_vec();
        fail_env.push(("FAKE_QUERY", "fail"));
        let outcome = consumer.run(
            &with_mode(&["build", "src/main.rs"], flags),
            &fail_env,
            true,
        );
        assert_terminal_failure(&outcome, "query_failed", "query", 2);
        assert_eq!(outcome.kinds(), ["error", "command_finished"]);
        let error = &outcome.events()[0];
        assert_eq!(error["status"], 7, "{}", outcome.context());
        assert_eq!(error["phase"], "resolve");
        assert_eq!(error["truncated"], false);
        let excerpt = error["excerpt"].as_str().expect("excerpt");
        assert!(excerpt.starts_with("Loading: 1 package\n"), "{excerpt}");
        assert!(
            excerpt.ends_with("ERROR: no such target //src:gone"),
            "{excerpt}"
        );
        assert!(
            outcome.err.contains("  ERROR: no such target //src:gone\n"),
            "{}",
            outcome.context()
        );
        assert_eq!(outcome.bazel.len(), 1, "{}", outcome.context());
        assert!(!outcome.built(), "{}", outcome.context());
    }
}

#[test]
fn a_query_that_cannot_launch_is_not_an_empty_selection() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let outcome = consumer.run(&with_mode(&["build", "src/main.rs"], flags), env, false);
        assert_terminal_failure(&outcome, "query_launch_failed", "query_launch", 1);
        assert_eq!(outcome.kinds(), ["error", "command_finished"]);
        assert!(outcome.events()[0].get("status").is_none());
    }
}

#[test]
fn an_unavailable_bazel_is_a_tool_failure_after_the_start() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let outcome = consumer.run(&with_mode(&["build", "//..."], flags), env, false);
        assert_terminal_failure(&outcome, "launch_failed", "tool_unavailable", 1);
        assert_eq!(
            outcome.kinds(),
            ["command_started", "error", "command_finished"]
        );
    }
}

#[test]
fn malformed_build_events_are_a_collection_failure() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let mut bad_env = env.to_vec();
        bad_env.push(("FAKE_BUILD", "badbep"));
        let outcome = consumer.run(&with_mode(&["test", "//..."], flags), &bad_env, true);
        assert_terminal_failure(&outcome, "invalid_bep", "collection", 1);
        assert_eq!(outcome.kinds()[0], "command_started");
    }
}

fn assert_bazel_exit_kept(outcome: &Outcome, results: &std::path::Path, code: i32, absent: &str) {
    let context = outcome.context();
    assert_eq!(outcome.status.code(), Some(code), "{context}");
    let events = outcome.events();
    let has = |kind: &str, field: &str, value: &str| {
        events
            .iter()
            .any(|event| event["event"] == kind && event[field] == value)
    };
    assert!(
        has("notice", "code", "build_events_incomplete"),
        "{context}"
    );
    assert!(has("error", "code", "incomplete_results"), "{context}");
    assert!(has("error", "code", "bazel_failed"), "{context}");
    assert!(!has("error", "code", absent), "{context}");
    let report = events
        .iter()
        .find(|event| event["event"] == "report" && event["format"] == "results")
        .unwrap_or_else(|| panic!("results report event\n{context}"));
    assert_eq!(report["results_complete"], false, "{context}");
    let finished = events.last().expect("events");
    assert_eq!(finished["event"], "command_finished", "{context}");
    assert_eq!(finished["exit_code"], code, "{context}");
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(results).expect("results")).expect("json");
    assert_eq!(document["exit_code"], code, "{context}");
    assert_eq!(document["complete"], false, "{context}");
}

#[test]
fn a_missing_event_file_keeps_bazels_usage_exit() {
    let consumer = Consumer::new();
    let results = consumer.workspace().join(".dx/results.json");
    for (flags, env) in JSON_MODES {
        fs::write(&results, "stale").expect("stale results");
        let mut run_env = env.to_vec();
        run_env.extend([("FAKE_BUILD", "nobep"), ("FAKE_EXIT", "2")]);
        let outcome = consumer.run(&with_mode(&["test", "//..."], flags), &run_env, true);
        assert_bazel_exit_kept(&outcome, &results, 2, "unreadable_bep");
    }
}

#[test]
fn a_cut_off_event_file_keeps_bazels_crash_exit() {
    let consumer = Consumer::new();
    let results = consumer.workspace().join(".dx/results.json");
    for (flags, env) in JSON_MODES {
        fs::write(&results, "stale").expect("stale results");
        let mut run_env = env.to_vec();
        run_env.extend([("FAKE_BUILD", "cutbep"), ("FAKE_EXIT", "37")]);
        let outcome = consumer.run(&with_mode(&["test", "//..."], flags), &run_env, true);
        assert_bazel_exit_kept(&outcome, &results, 37, "invalid_bep");
    }
}

#[test]
fn a_report_that_cannot_be_written_fails_after_the_build() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let outcome = consumer.run(
            &with_mode(
                &["build", "//...", "--report=failures=missing/f.json"],
                flags,
            ),
            env,
            true,
        );
        let context = outcome.context();
        assert_eq!(outcome.status.code(), Some(1), "{context}");
        assert_eq!(
            outcome.kinds(),
            ["command_started", "error", "command_finished"],
            "{context}"
        );
        let events = outcome.events();
        assert_eq!(events[1]["code"], "report_failed", "{context}");
        assert_eq!(events[1]["cause"], "report", "{context}");
        assert_eq!(events[2]["exit_code"], 1, "{context}");
        assert!(outcome.err.contains("dx: report_failed: "), "{context}");
        assert!(outcome.built(), "{context}");
    }
}

#[test]
fn cancellation_ends_without_a_failure_event() {
    let consumer = Consumer::new();
    for signo in [libc::SIGINT, libc::SIGTERM] {
        let outcome = consumer.run_cancelled(
            &["build", "src/main.rs", "--output=json"],
            &[("FAKE_QUERY", "sleep")],
            signo,
        );
        let context = outcome.context();
        assert_eq!(outcome.status.signal(), Some(signo), "{context}");
        assert!(outcome.out.is_empty(), "{context}");
        assert!(!outcome.err.contains("dx: "), "{context}");
        assert!(!outcome.built(), "{context}");
        let outcome = consumer.run_cancelled(
            &["build", "//...", "--output=json"],
            &[("FAKE_BUILD", "sleep")],
            signo,
        );
        let context = outcome.context();
        assert_eq!(outcome.status.signal(), Some(signo), "{context}");
        assert_eq!(outcome.kinds(), ["command_started"], "{context}");
    }
}

#[test]
fn a_closed_stdout_ends_a_failure_with_the_broken_pipe_exit() {
    let consumer = Consumer::new();
    for (flags, env) in JSON_MODES {
        let mut fail_env = env.to_vec();
        fail_env.push(("FAKE_QUERY", "fail"));
        for (args, query) in [
            (vec!["build", "src/main.rs"], true),
            (vec!["build", "no/such/path"], false),
        ] {
            let run_env = if query { &fail_env[..] } else { env };
            let outcome = consumer.run_closed_stdout(&with_mode(&args, flags), run_env);
            assert_eq!(outcome.status.code(), Some(141), "{}", outcome.context());
            assert!(outcome.err.starts_with("dx: "), "{}", outcome.context());
            assert!(!outcome.built(), "{}", outcome.context());
        }
    }
}

#[test]
fn text_mode_failures_keep_stdout_empty() {
    let consumer = Consumer::new();
    let outcome = consumer.run(&["build", "src/main.rs"], &[("FAKE_QUERY", "fail")], true);
    assert_eq!(outcome.status.code(), Some(2), "{}", outcome.context());
    assert!(outcome.out.is_empty(), "{}", outcome.context());
    assert!(
        outcome
            .err
            .starts_with("dx: query_failed: query kind('rule', //...) failed: Bazel exited 7\n"),
        "{}",
        outcome.context()
    );
}

#[test]
fn check_keeps_every_phase_finding_when_the_report_cannot_be_written() {
    let consumer = Consumer::new();
    let (result, witness) = consumer.stage_findings();
    fs::create_dir_all(consumer.workspace().join("rep.sarif")).expect("destination dir");
    let outcome = consumer.run(
        &[
            "check",
            "//...",
            "--output=json",
            "--report",
            "sarif=rep.sarif",
        ],
        &[
            ("FAKE_BUILD", "findings"),
            ("FAKE_RESULT", result.as_str()),
            ("FAKE_WITNESS", witness.as_str()),
        ],
        true,
    );
    let context = outcome.context();
    assert_eq!(outcome.status.code(), Some(1), "{context}");
    let events = outcome.events();
    let finished: Vec<(String, i64)> = events
        .iter()
        .filter(|event| event["event"] == "phase_finished")
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
            ("generate".to_owned(), 0),
            ("format".to_owned(), 1),
            ("lint".to_owned(), 1),
            ("typecheck".to_owned(), 1)
        ],
        "{context}"
    );
    let findings: Vec<&str> = events
        .iter()
        .filter(|event| event["event"] == "diagnostic")
        .map(|event| event["correlation"].as_str().expect("correlation"))
        .collect();
    assert_eq!(
        findings,
        ["check/format", "check/lint", "check/typecheck"],
        "{context}"
    );
    let errors: Vec<&str> = events
        .iter()
        .filter(|event| event["event"] == "error")
        .map(|event| event["code"].as_str().expect("code"))
        .collect();
    assert_eq!(errors, ["report_failed"], "{context}");
    let last = events.last().expect("events");
    assert_eq!(last["event"], "command_finished", "{context}");
    assert_eq!(last["exit_code"], 1, "{context}");
    assert_eq!(last["results_complete"], true, "{context}");
    assert!(outcome.err.contains("dx: report_failed: "), "{context}");
}

#[test]
fn check_cancellation_stops_before_any_later_phase() {
    let consumer = Consumer::new();
    for signo in [libc::SIGINT, libc::SIGTERM] {
        let outcome = consumer.run_cancelled(
            &["check", "//...", "--output=json"],
            &[("FAKE_BUILD", "sleep")],
            signo,
        );
        let context = outcome.context();
        assert_eq!(outcome.status.signal(), Some(signo), "{context}");
        assert_eq!(outcome.kinds(), ["command_started"], "{context}");
        let builds = outcome
            .bazel
            .iter()
            .filter(|line| line.contains(" build ") || line.contains(" run "))
            .count();
        assert_eq!(builds, 1, "{context}");
    }
}

#[test]
fn check_cancellation_after_findings_stops_before_any_later_phase() {
    let consumer = Consumer::new();
    let (result, witness) = consumer.stage_findings();
    for signo in [libc::SIGINT, libc::SIGTERM] {
        let outcome = consumer.run_cancelled(
            &["check", "//...", "--output=json"],
            &[
                ("FAKE_BUILD", "findings"),
                ("FAKE_RESULT", result.as_str()),
                ("FAKE_WITNESS", witness.as_str()),
                ("FAKE_GATE_AT", "2"),
            ],
            signo,
        );
        let context = outcome.context();
        assert_eq!(outcome.status.signal(), Some(signo), "{context}");
        assert!(
            outcome.out.contains("\"correlation\":\"check/format\""),
            "{context}"
        );
        assert!(
            outcome.out.contains("\"correlation\":\"check/generate\""),
            "{context}"
        );
        assert!(!outcome.out.contains("check/typecheck"), "{context}");
        let builds = outcome
            .bazel
            .iter()
            .filter(|line| line.contains(" build "))
            .count();
        assert_eq!(builds, 2, "{context}");
    }
}

#[test]
fn check_with_a_stdout_closed_after_findings_ends_with_the_broken_pipe_exit() {
    let consumer = Consumer::new();
    let (result, witness) = consumer.stage_findings();
    let outcome = consumer.run_hung_up_after_findings(
        &["check", "//...", "--output=json"],
        &[
            ("FAKE_BUILD", "findings"),
            ("FAKE_RESULT", result.as_str()),
            ("FAKE_WITNESS", witness.as_str()),
            ("FAKE_GATE_AT", "2"),
        ],
    );
    let context = outcome.context();
    assert_eq!(outcome.status.code(), Some(141), "{context}");
    assert!(
        outcome.out.contains("\"correlation\":\"check/format\""),
        "{context}"
    );
    let builds = outcome
        .bazel
        .iter()
        .filter(|line| line.contains(" build "))
        .count();
    assert_eq!(builds, 2, "{context}");
}
