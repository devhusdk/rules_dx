use super::common::{refusal, resolve_failed, Env};
use super::execute;
use crate::args::{Command, Invocation};
use crate::resolve::{
    collect_range, collect_working_tree, local_override_changes, merge_base, quote_set,
    resolve_affected, run_label_query_file, AffectedSelection, QueryRunner, ResolveError,
    RuleGraph, WidenedScope,
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

    fn json(&self, command: Command) -> Value {
        let widened: Vec<Value> = self
            .widened
            .iter()
            .map(|entry| json!({"scope": entry.scope, "reason": entry.reason}))
            .collect();
        let mut value = json!({
            "base": self.base,
            "changed_files": self.changed_files,
            "targets": self.targets,
            "widened": widened,
        });
        if command == Command::Check {
            value["tests"] = json!(self.tests);
        }
        value
    }

    fn summary(&self) -> String {
        let base = short_base(&self.base);
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

fn short_base(base: &str) -> String {
    base.chars().take(12).collect()
}

fn resolving_line(base: &str) -> String {
    format!("dx: affected: resolving changes since {}", short_base(base))
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

fn tests_expression(rules: &[String]) -> String {
    format!("tests(set({}))", quote_set(rules))
}

fn tests_of(
    rules: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
) -> Result<Vec<String>, ResolveError> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    run_label_query_file(
        &tests_expression(rules),
        &format!("tests of {} affected rules", rules.len()),
        workspace,
        runner,
        startup_options,
    )
}

fn audit_owned(workspace: &Path, target: &str) -> bool {
    match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => !dx_adopt::dependency_sets::owning_sets(&registry, target).is_empty(),
        _ => !dx_update::selector::owning_sets(target).is_empty(),
    }
}

pub(crate) fn affected_base(
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Result<String, PlanError> {
    let git = PathBuf::from("git");
    let branch = default_branch(runner, workspace)?;
    merge_base(&git, workspace, runner, &branch, "HEAD").map_err(PlanError::Git)
}

pub(crate) fn plan(
    command: Command,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    base: String,
) -> Result<AffectedPlan, PlanError> {
    let git = PathBuf::from("git");
    let mut changes =
        collect_range(&git, workspace, runner, &base, "HEAD").map_err(PlanError::Git)?;
    changes.extend(collect_working_tree(&git, workspace, runner).map_err(PlanError::Git)?);
    let changes = dx_adopt::dedupe_changes(changes);
    let overrides =
        local_override_changes(&git, workspace, runner, &base).map_err(PlanError::Git)?;
    let mut changed_files: Vec<String> = overrides.clone();
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
    if changes.is_empty() && overrides.is_empty() {
        return Ok(plan);
    }
    let mut selection = if changes.is_empty() {
        AffectedSelection {
            targets: Vec::new(),
            widened: Vec::new(),
            graph: None,
        }
    } else {
        resolve_affected(
            &changes,
            workspace,
            runner,
            startup_options,
            &crate::plan::registry::all_aspect_labels(),
        )?
    };
    for path in &overrides {
        selection.widened.push(WidenedScope {
            scope: "//...".to_owned(),
            reason: format!("local_path_override {path}"),
        });
    }
    if !overrides.is_empty() {
        selection.targets = vec!["//...".to_owned()];
    }
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
    let graph = selection
        .graph
        .take()
        .map_or_else(|| RuleGraph::load(workspace, runner, startup_options), Ok)?;
    let rules = graph.affected(&selection.targets);
    let tests = if tests_only(command) || command == Command::Check {
        tests_of(&rules, workspace, runner, startup_options)?
    } else {
        Vec::new()
    };
    plan.targets = if tests_only(command) {
        tests.clone()
    } else {
        rules
    };
    if command == Command::Check && !plan.targets.is_empty() {
        plan.tests = tests;
    }
    if matches!(command, Command::Security | Command::License) {
        plan.targets.retain(|target| audit_owned(workspace, target));
        plan.unscoped = secrets_scan && plan.targets.is_empty();
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
        let Ok(Value::Object(mut started)) =
            command_started(invocation.command.name(), invocation.started_mode())
        else {
            return 1;
        };
        started.insert("affected".to_owned(), plan.json(invocation.command));
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
    let text = !invocation.quiet && matches!(invocation.output, OutputMode::Text { .. });
    let base = match affected_base(workspace, query_runner) {
        Ok(base) => base,
        Err(PlanError::Git(message)) => return refusal(invocation.output, out, err, &message),
        Err(PlanError::Resolve(error)) => {
            return resolve_failed(invocation.output, out, err, &error)
        }
    };
    if text {
        let _ = writeln!(err, "{}", resolving_line(&base));
    }
    let plan = match plan(
        invocation.command,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
        base,
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
    if text {
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
    let mut injector = InjectAffected::new(out, plan.json(invocation.command));
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
            let mut seen = argv.last().cloned().unwrap_or_default();
            if let Some(path) = argv
                .iter()
                .find_map(|arg| arg.strip_prefix("--query_file="))
            {
                seen = std::fs::read_to_string(path).expect("query file");
            }
            self.expressions.borrow_mut().push(seen);
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

    fn graph() -> String {
        let rules = [
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt", "//a:b.txt", "//a:new.txt", "//a:key.txt", "//a:c.txt"]}),
            json!({"name": "//a:t", "ruleInput": ["//a:lib"]}),
            json!({"name": "//a:m", "ruleInput": ["//a:lib"], "attribute": [{"name": "tags", "stringListValue": ["manual"]}]}),
            json!({"name": "//a:android", "ruleInput": ["@@rules_android++sdk+androidsdk//:core"]}),
            json!({"name": "//a:ruby", "ruleInput": ["@@rules_ruby++ruby+ruby_x86_64_windows//:ruby"]}),
        ];
        rules
            .iter()
            .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn run(root: &Path, command: Command, answers: &[&str]) -> (AffectedPlan, GitRunner) {
        let runner = GitRunner::new(answers);
        let base = affected_base(root, &runner).ok().expect("base");
        let plan = plan(command, root, &runner, &[], base).ok().expect("plan");
        assert!(runner.bazel.borrow().is_empty(), "unused bazel answers");
        (plan, runner)
    }

    #[test]
    fn empty_selection_reports_check_unless_apply() {
        let dir = repo();
        let (plan, _) = run(dir.path(), Command::Test, &[]);
        let cases: [(&[&str], &str); 8] = [
            (&["fix", "--affected", "--output=json"], "check"),
            (
                &["fix", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["fix", "--check", "--affected", "--output=json"], "check"),
            (&["check", "--affected", "--output=json"], "check"),
            (&["format", "--affected", "--output=json"], "check"),
            (
                &["lint", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["test", "--affected", "--output=json"], "default"),
            (&["list-tests", "--affected", "--output=json"], "default"),
        ];
        for (words, expected) in cases {
            let invocation =
                crate::args::parse(&crate::test_support::strings(words)).expect("parses");
            let mut out = Vec::new();
            assert_eq!(finished_empty(&invocation, &plan, &mut out), 0, "{words:?}");
            let text = String::from_utf8(out).expect("utf8");
            let first: Value =
                serde_json::from_str(text.lines().next().expect("line")).expect("json");
            assert_eq!(first["mode"], expected, "{words:?}");
            assert!(first["affected"].is_object(), "{words:?}");
            if words[0] == "check" {
                assert_eq!(first["affected"]["tests"], json!([]), "{words:?}");
            } else if matches!(words[0], "test" | "list-tests") {
                assert!(first["affected"].get("tests").is_none(), "{words:?}");
            }
        }
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
        let graph = graph();
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["a/a.txt", "a/b.txt", "a/new.txt"]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert!(plan.widened.is_empty());
        assert_eq!(runner.expressions.borrow().len(), 1);
    }

    #[test]
    fn the_universe_query_never_names_rdeps_or_external_repos() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (_, runner) = run(dir.path(), Command::Build, &[&graph]);
        for expression in runner.expressions.borrow().iter() {
            assert_eq!(expression, "kind('rule', //...)");
        }
    }

    #[test]
    fn rules_that_reach_unloadable_or_other_platform_repos_do_not_matter() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Build, &[&graph]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert!(!plan
            .targets
            .iter()
            .any(|target| target == "//a:android" || target == "//a:ruby"));
    }

    #[test]
    fn renames_count_both_paths() {
        let dir = repo();
        let root = dir.path();
        git(root, &["mv", "a/a.txt", "a/c.txt"]);
        let graph = graph();
        let (plan, _) = run(root, Command::Lint, &[&graph]);
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
    fn bzl_change_selects_loading_package_and_rdeps() {
        let dir = repo();
        let root = dir.path();
        write(root, "defs.bzl", "x = 1\n");
        write(root, "a/BUILD.bazel", "load(\"//:defs.bzl\", \"x\")\n");
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "defs"]);
        git(root, &["branch", "--force", "main"]);
        write(root, "defs.bzl", "x = 2\n");
        let graph = graph();
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["defs.bzl"]);
        assert_eq!(
            plan.targets,
            ["//a:android", "//a:lib", "//a:ruby", "//a:t"]
        );
        assert_eq!(plan.widened.len(), 1);
        assert_eq!(plan.widened[0].scope, "//a:all");
        let summary = plan.summary();
        assert!(
            summary.contains("widened to //a:all (loads defs.bzl)"),
            "{summary}"
        );
        assert!(!summary.contains("//..."), "{summary}");
        assert_eq!(
            plan.json(Command::Build)["widened"],
            json!([{"scope": "//a:all", "reason": "loads defs.bzl"}])
        );
        let expressions = runner.expressions.borrow();
        assert_eq!(expressions.len(), 1);
        assert!(
            expressions
                .iter()
                .all(|expression| !expression.contains("rbuildfiles")),
            "{expressions:?}"
        );
    }

    #[test]
    fn tests_only_commands_query_tests_and_check_adds_a_test_list() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (plan, runner) = run(dir.path(), Command::Test, &[&graph, "//a:t\n"]);
        assert_eq!(plan.targets, ["//a:t"]);
        assert_eq!(
            runner.expressions.borrow()[1],
            "tests(set(\"//a:lib\" \"//a:t\"))"
        );

        let (plan, runner) = run(dir.path(), Command::Check, &[&graph, "//a:t\n"]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert_eq!(plan.tests, ["//a:t"]);
        assert_eq!(runner.expressions.borrow().len(), 2);
    }

    #[test]
    fn check_json_reports_the_selected_tests() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Check, &[&graph, "//a:t\n"]);
        assert_eq!(plan.json(Command::Check)["tests"], json!(["//a:t"]));
    }

    #[test]
    fn resolving_line_uses_the_summary_base() {
        let base = "0123456789abcdef0123456789abcdef01234567";
        let line = resolving_line(base);
        assert_eq!(line, "dx: affected: resolving changes since 0123456789ab");
        let plan = AffectedPlan {
            base: base.to_owned(),
            changed_files: Vec::new(),
            targets: Vec::new(),
            tests: Vec::new(),
            widened: Vec::new(),
            unscoped: false,
        };
        assert!(plan
            .summary()
            .ends_with(&format!("since {}", short_base(base))));
        assert!(line.ends_with(&short_base(base)));
    }

    #[test]
    fn manual_rules_are_not_selected() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Build, &[&graph]);
        assert!(!plan.targets.contains(&"//a:m".to_owned()));
    }

    #[test]
    fn security_with_no_owned_targets_still_runs_the_secrets_scan() {
        let dir = repo();
        write(dir.path(), "a/key.txt", "secret\n");
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Security, &[&graph]);
        assert!(plan.targets.is_empty());
        assert!(!plan.empty());
    }

    #[test]
    fn json_reports_base_files_targets_and_widening() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Build, &[&graph]);
        let value = plan.json(Command::Build);
        assert!(value.get("tests").is_none());
        assert_eq!(value["changed_files"], json!(["a/a.txt"]));
        assert_eq!(value["targets"], json!(["//a:lib", "//a:t"]));
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
            affected_base(dir.path(), &runner),
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
