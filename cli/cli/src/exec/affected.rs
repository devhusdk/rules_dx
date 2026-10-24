use super::common::{refusal, resolve_failed, Env};
use super::execute;
use crate::args::{Command, Invocation};
use crate::resolve::{
    collect_range, collect_working_tree, merge_base, quote_set, resolve_affected, run_label_query,
    QueryRunner, ResolveError, WidenedScope,
};
use dx_output::{command_finished, command_started, FinishedCounts, OutputMode};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

const DEFAULT_BRANCH_CANDIDATES: [&str; 4] = ["origin/main", "main", "origin/master", "master"];

pub(crate) enum PlanError {
    Git(String),
    Resolve(ResolveError),
}

impl From<ResolveError> for PlanError {
    fn from(error: ResolveError) -> Self {
        PlanError::Resolve(error)
    }
}

/// The targets one `--affected` run selects, with the evidence behind them.
pub(crate) struct AffectedPlan {
    pub(crate) base: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) targets: Vec<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) widened: Vec<WidenedScope>,
    pub(crate) unscoped: bool,
}

impl AffectedPlan {
    fn empty(&self) -> bool {
        self.targets.is_empty() && !self.unscoped
    }

    fn json(&self) -> Value {
        let widened: Vec<Value> = self
            .widened
            .iter()
            .map(|entry| json!({"scope": entry.scope, "reason": entry.reason}))
            .collect();
        json!({
            "base": self.base,
            "changed_files": self.changed_files,
            "targets": self.targets,
            "widened": widened,
        })
    }

    fn summary(&self) -> String {
        let base: String = self.base.chars().take(12).collect();
        let mut widened: Vec<String> = self
            .widened
            .iter()
            .map(|entry| format!("{} ({})", entry.scope, entry.reason))
            .collect();
        widened.dedup();
        let suffix = if widened.is_empty() {
            String::new()
        } else {
            format!("; widened to {}", widened.join(", "))
        };
        format!(
            "dx: affected: {} targets from {} changed files since {base}{suffix}",
            self.targets.len(),
            self.changed_files.len()
        )
    }
}

fn tests_only(command: Command) -> bool {
    matches!(
        command,
        Command::Test | Command::Coverage | Command::ListTests
    )
}

fn git_text(runner: &dyn QueryRunner, workspace: &Path, args: &[&str]) -> Option<String> {
    let mut argv = vec!["git".to_owned()];
    argv.extend(args.iter().map(ToString::to_string));
    let result = runner.run_query(&argv, workspace).ok()?;
    if result.code != Some(0) {
        return None;
    }
    let text = String::from_utf8(result.stdout).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn default_branch(runner: &dyn QueryRunner, workspace: &Path) -> Result<String, PlanError> {
    if let Some(head) = git_text(
        runner,
        workspace,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        return Ok(head);
    }
    for candidate in DEFAULT_BRANCH_CANDIDATES {
        let spec = format!("{candidate}^{{commit}}");
        if git_text(
            runner,
            workspace,
            &["rev-parse", "--verify", "--quiet", &spec],
        )
        .is_some()
        {
            return Ok(candidate.to_owned());
        }
    }
    Err(PlanError::Git(
        "cannot find the default branch: expected origin/HEAD, main, or master".to_owned(),
    ))
}

fn expression(scopes: &[String], tests: bool) -> String {
    let set = quote_set(scopes);
    let affected = "$r except attr(\"tags\", \"\\bmanual\\b\", $r)";
    let selected = if tests {
        format!("tests({affected})")
    } else {
        affected.to_owned()
    };
    format!("let r = kind('rule', rdeps(//..., set({set}))) in {selected}")
}

fn audit_owned(workspace: &Path, target: &str) -> bool {
    match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => !dx_adopt::dependency_sets::owning_sets(&registry, target).is_empty(),
        _ => !dx_update::selector::owning_sets(target).is_empty(),
    }
}

pub(crate) fn plan(
    command: Command,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<AffectedPlan, PlanError> {
    let git = PathBuf::from("git");
    let branch = default_branch(runner, workspace)?;
    let base = merge_base(&git, workspace, runner, &branch, "HEAD").map_err(PlanError::Git)?;
    let mut changes =
        collect_range(&git, workspace, runner, &base, "HEAD").map_err(PlanError::Git)?;
    changes.extend(collect_working_tree(&git, workspace, runner).map_err(PlanError::Git)?);
    let changes = dx_adopt::dedupe_changes(changes);
    let mut changed_files: Vec<String> = Vec::new();
    for change in &changes {
        changed_files.push(change.path.clone());
        if let Some(from) = &change.from {
            changed_files.push(from.clone());
        }
    }
    changed_files.sort();
    changed_files.dedup();
    let mut plan = AffectedPlan {
        base,
        changed_files,
        targets: Vec::new(),
        tests: Vec::new(),
        widened: Vec::new(),
        unscoped: false,
    };
    if changes.is_empty() {
        return Ok(plan);
    }
    let selection = resolve_affected(&changes, workspace, runner, startup_options)?;
    plan.widened = selection.widened.clone();
    if selection.targets.iter().any(|target| target == "//...") {
        if command == Command::Check {
            plan.tests = vec!["//...".to_owned()];
        }
        plan.targets = vec!["//...".to_owned()];
        return Ok(plan);
    }
    let secrets_scan = command == Command::Security;
    if selection.targets.is_empty() {
        plan.unscoped = secrets_scan;
        return Ok(plan);
    }
    let only_tests = tests_only(command);
    plan.targets = run_label_query(
        &expression(&selection.targets, only_tests),
        workspace,
        runner,
        startup_options,
    )?;
    if matches!(command, Command::Security | Command::License) {
        plan.targets.retain(|target| audit_owned(workspace, target));
        plan.unscoped = secrets_scan && plan.targets.is_empty();
    }
    if command == Command::Check && !plan.targets.is_empty() {
        plan.tests = run_label_query(
            &expression(&selection.targets, true),
            workspace,
            runner,
            startup_options,
        )?;
    }
    Ok(plan)
}

/// Adds the `affected` object to the first `command_started` line and passes the rest through.
struct InjectAffected<'a> {
    inner: &'a mut dyn Write,
    affected: Value,
    first: Vec<u8>,
    done: bool,
}

impl<'a> InjectAffected<'a> {
    fn new(inner: &'a mut dyn Write, affected: Value) -> Self {
        InjectAffected {
            inner,
            affected,
            first: Vec::new(),
            done: false,
        }
    }

    fn rewrite(&self, line: &[u8]) -> Vec<u8> {
        let Ok(Value::Object(mut event)) = serde_json::from_slice::<Value>(line) else {
            return line.to_vec();
        };
        if event.get("event").and_then(Value::as_str) != Some("command_started") {
            return line.to_vec();
        }
        event.insert("affected".to_owned(), self.affected.clone());
        serde_json::to_vec(&Value::Object(event)).unwrap_or_else(|_| line.to_vec())
    }

    fn finish(&mut self) -> std::io::Result<()> {
        if !self.done && !self.first.is_empty() {
            self.done = true;
            let pending = std::mem::take(&mut self.first);
            self.inner.write_all(&pending)?;
        }
        Ok(())
    }
}

impl Write for InjectAffected<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.done {
            return self.inner.write(buf);
        }
        match buf.iter().position(|byte| *byte == b'\n') {
            None => {
                self.first.extend_from_slice(buf);
                Ok(buf.len())
            }
            Some(end) => {
                self.first.extend_from_slice(&buf[..end]);
                let line = std::mem::take(&mut self.first);
                self.done = true;
                let mut rewritten = self.rewrite(&line);
                rewritten.push(b'\n');
                self.inner.write_all(&rewritten)?;
                self.inner.write_all(&buf[end + 1..])?;
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn finished_empty(invocation: &Invocation, plan: &AffectedPlan, out: &mut dyn Write) -> i32 {
    if invocation.output == OutputMode::Json {
        let mode = if invocation.command == Command::Check || invocation.check {
            "check"
        } else {
            "default"
        };
        let Ok(Value::Object(mut started)) = command_started(invocation.command.name(), mode)
        else {
            return 1;
        };
        started.insert("affected".to_owned(), plan.json());
        let finished = command_finished(0, &FinishedCounts::default());
        for event in [Value::Object(started), finished] {
            if dx_output::write_event(out, &event).is_err() {
                return 1;
            }
        }
        return 0;
    }
    if writeln!(out, "no affected targets").is_err() {
        return 1;
    }
    0
}

pub(crate) fn execute_affected(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ci,
    } = env;
    let plan = match plan(
        invocation.command,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(PlanError::Git(message)) => return refusal(invocation.output, out, err, &message),
        Err(PlanError::Resolve(error)) => {
            return resolve_failed(invocation.output, out, err, &error)
        }
    };
    if plan.empty() {
        return finished_empty(invocation, &plan, out);
    }
    if !invocation.quiet && matches!(invocation.output, OutputMode::Text { .. }) {
        let _ = writeln!(err, "{}", plan.summary());
    }
    let mut next = invocation.clone();
    next.affected = false;
    next.targets = plan.targets.clone();
    next.test_targets = Some(plan.tests.clone());
    if invocation.output != OutputMode::Json {
        return execute(
            &next,
            Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce,
                out,
                err,
                ci,
            },
        );
    }
    let mut injector = InjectAffected::new(out, plan.json());
    let code = execute(
        &next,
        Env {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid,
            nonce,
            out: &mut injector,
            err,
            ci,
        },
    );
    let _ = injector.finish();
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::QueryResult;
    use std::cell::RefCell;
    use std::process::Command as Process;

    struct GitRunner {
        bazel: RefCell<Vec<String>>,
        expressions: RefCell<Vec<String>>,
    }

    impl GitRunner {
        fn new(answers: &[&str]) -> Self {
            GitRunner {
                bazel: RefCell::new(answers.iter().map(ToString::to_string).collect()),
                expressions: RefCell::new(Vec::new()),
            }
        }
    }

    impl QueryRunner for GitRunner {
        fn run_query(&self, argv: &[String], cwd: &Path) -> std::io::Result<QueryResult> {
            if argv[0] == "git" {
                let out = Process::new("git")
                    .args(&argv[1..])
                    .current_dir(cwd)
                    .output()?;
                return Ok(QueryResult {
                    code: out.status.code(),
                    stdout: out.stdout,
                    stderr: out.stderr,
                });
            }
            self.expressions
                .borrow_mut()
                .push(argv.last().cloned().unwrap_or_default());
            let answer = self.bazel.borrow_mut().remove(0);
            Ok(QueryResult {
                code: Some(0),
                stdout: answer.into_bytes(),
                stderr: Vec::new(),
            })
        }
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Process::new("git")
            .args(["-c", "user.name=dx", "-c", "user.email=dx@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(root)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let full = root.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(full, text).expect("write");
    }

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        git(root, &["init", "--quiet", "--initial-branch=main"]);
        write(root, "MODULE.bazel", "module(name = \"x\")\n");
        write(root, "a/BUILD.bazel", "");
        write(root, "a/a.txt", "a\n");
        write(root, "a/b.txt", "b\n");
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "base"]);
        git(root, &["checkout", "--quiet", "-b", "topic"]);
        dir
    }

    fn run(root: &Path, command: Command, answers: &[&str]) -> (AffectedPlan, GitRunner) {
        let runner = GitRunner::new(answers);
        let plan = plan(command, root, &runner, &[]).ok().expect("plan");
        (plan, runner)
    }

    #[test]
    fn clean_tree_selects_nothing() {
        let dir = repo();
        let (plan, runner) = run(dir.path(), Command::Test, &[]);
        assert!(plan.empty());
        assert!(plan.changed_files.is_empty());
        assert!(runner.expressions.borrow().is_empty());
    }

    #[test]
    fn merge_base_diff_working_tree_and_untracked_files_count() {
        let dir = repo();
        let root = dir.path();
        write(root, "a/a.txt", "committed\n");
        git(root, &["commit", "--quiet", "-am", "topic"]);
        write(root, "a/b.txt", "unstaged\n");
        write(root, "a/new.txt", "new\n");
        let (plan, runner) = run(
            root,
            Command::Build,
            &[
                "//a:lib\n",
                "//a:a.txt\n//a:b.txt\n//a:new.txt\n",
                "//a:lib\n//a:t\n",
            ],
        );
        assert_eq!(plan.changed_files, ["a/a.txt", "a/b.txt", "a/new.txt"]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert!(plan.widened.is_empty());
        assert_eq!(runner.expressions.borrow().len(), 3);
        assert!(runner.expressions.borrow()[2].contains("rdeps(//..."));
    }

    #[test]
    fn renames_count_both_paths() {
        let dir = repo();
        let root = dir.path();
        git(root, &["mv", "a/a.txt", "a/c.txt"]);
        let (plan, _) = run(root, Command::Lint, &["//a:lib\n"]);
        assert_eq!(plan.changed_files, ["a/a.txt", "a/c.txt"]);
        assert_eq!(plan.widened.len(), 1);
        assert_eq!(plan.widened[0].reason, "renamed from a/a.txt");
    }

    #[test]
    fn workspace_inputs_widen_to_everything_with_a_reason() {
        let dir = repo();
        write(dir.path(), "MODULE.bazel", "module(name = \"y\")\n");
        let (plan, runner) = run(dir.path(), Command::Test, &[]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(plan.widened[0].scope, "//...");
        assert_eq!(plan.widened[0].reason, "workspace input MODULE.bazel");
        assert!(plan.summary().contains("widened to //..."));
        assert!(runner.expressions.borrow().is_empty());
    }

    #[test]
    fn tests_only_commands_query_tests_and_check_adds_a_test_list() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let (plan, runner) = run(dir.path(), Command::Test, &["//a:lib\n", "//a:t\n"]);
        assert_eq!(plan.targets, ["//a:t"]);
        assert!(runner.expressions.borrow()[1].contains("tests("));

        let (plan, runner) = run(
            dir.path(),
            Command::Check,
            &["//a:lib\n", "//a:lib\n//a:t\n", "//a:t\n"],
        );
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert_eq!(plan.tests, ["//a:t"]);
        assert_eq!(runner.expressions.borrow().len(), 3);
    }

    #[test]
    fn manual_filter_uses_a_single_backslash_word_boundary() {
        let text = expression(&["//a:lib".to_owned()], false);
        assert!(text.contains("\"\\bmanual\\b\""), "{text}");
        assert!(!text.contains("\\\\b"), "{text}");
    }

    #[test]
    fn security_with_no_owned_targets_still_runs_the_secrets_scan() {
        let dir = repo();
        write(dir.path(), "a/key.txt", "secret\n");
        let (plan, _) = run(dir.path(), Command::Security, &["//a:lib\n", "//a:lib\n"]);
        assert!(plan.targets.is_empty());
        assert!(!plan.empty());
    }

    #[test]
    fn json_reports_base_files_targets_and_widening() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let (plan, _) = run(dir.path(), Command::Build, &["//a:lib\n", "//a:lib\n"]);
        let value = plan.json();
        assert_eq!(value["changed_files"], json!(["a/a.txt"]));
        assert_eq!(value["targets"], json!(["//a:lib"]));
        assert_eq!(value["widened"], json!([]));
        assert_eq!(value["base"].as_str().map(str::len), Some(40));
    }

    #[test]
    fn missing_default_branch_is_a_git_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        git(dir.path(), &["init", "--quiet", "--initial-branch=dev"]);
        write(dir.path(), "x", "x");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "--quiet", "-m", "x"]);
        let runner = GitRunner::new(&[]);
        assert!(matches!(
            plan(Command::Test, dir.path(), &runner, &[]),
            Err(PlanError::Git(_))
        ));
    }

    #[test]
    fn injector_adds_affected_to_the_first_line_only() {
        let mut sink: Vec<u8> = Vec::new();
        {
            let mut injector = InjectAffected::new(&mut sink, json!({"base": "b"}));
            injector
                .write_all(b"{\"event\":\"command_started\"}\n{\"event\":\"x\"}\n")
                .expect("write");
            injector.finish().expect("finish");
        }
        let text = String::from_utf8(sink).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].contains("\"affected\":{\"base\":\"b\"}"));
        assert_eq!(lines[1], "{\"event\":\"x\"}");
    }
}
