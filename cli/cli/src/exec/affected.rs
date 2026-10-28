use super::common::{emit_built, emit_event, refusal, resolve_failed, Env};
use super::execute;
use dx_adopt::DEFAULT_BRANCH_CANDIDATES;
use dx_output::{command_finished, command_started, FinishedCounts, OutputMode};
use dx_plan::args::{Command, Invocation};
use dx_plan::resolve::{
    collect_range, collect_working_tree, local_override_changes, merge_base, quote_set,
    resolve_affected, run_label_query_file, AffectedSelection, QueryRunner, ResolveError,
    RuleGraph, WidenedScope,
};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

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
    /// External-repo test labels dropped because the test phase rejects them.
    pub(crate) skipped_external: Vec<String>,
}

/// How many widened scopes the stderr summary names before "and N more".
const SUMMARY_WIDENED_LIMIT: usize = 3;

impl AffectedPlan {
    fn empty(&self) -> bool {
        self.targets.is_empty() && !self.unscoped
    }

    fn json(&self, command: Command) -> Value {
        let widened: Vec<Value> = self
            .widened
            .iter()
            .map(|entry| {
                let mut value = json!({"scope": entry.scope, "reason": entry.reason});
                if !entry.repos.is_empty() {
                    value["repos"] = json!(entry.repos);
                }
                value
            })
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
        if !self.skipped_external.is_empty() {
            value["skipped_external"] = json!(self.skipped_external);
        }
        value
    }

    fn summary(&self) -> String {
        let base = short_base(&self.base);
        let mut widened: Vec<(bool, String)> = Vec::new();
        for entry in &self.widened {
            let item = (
                entry.scope != "//...",
                format!("{} ({})", entry.scope, entry.reason),
            );
            if !widened.contains(&item) {
                widened.push(item);
            }
        }
        // Stable sort keeps input order within the `//...` and other groups.
        widened.sort_by_key(|item| item.0);
        let widened: Vec<String> = widened.into_iter().map(|item| item.1).collect();
        let suffix = if widened.is_empty() {
            String::new()
        } else {
            let shown = widened
                .iter()
                .take(SUMMARY_WIDENED_LIMIT)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            match widened.len().saturating_sub(SUMMARY_WIDENED_LIMIT) {
                0 => format!("; widened to {shown}"),
                more => format!("; widened to {shown} and {more} more"),
            }
        };
        let mut line = format!(
            "dx: affected: {} targets from {} changed files since {base}{suffix}",
            self.targets.len(),
            self.changed_files.len()
        );
        if !self.skipped_external.is_empty() {
            line.push_str(&format!(
                "\ndx: affected: skipping {} external-repo tests: {}",
                self.skipped_external.len(),
                self.skipped_external.join(", ")
            ));
        }
        line
    }
}

fn short_base(base: &str) -> String {
    base.chars().take(12).collect()
}

fn resolving_line(base: &str) -> String {
    format!("dx: affected: resolving changes since {}", short_base(base))
}

fn tests_only(command: Command) -> bool {
    matches!(command, Command::Test | Command::Coverage)
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
    named_base(workspace, runner).map(|(_, base)| base)
}

/// The default branch and its merge-base with `HEAD`.
pub(crate) fn named_base(
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Result<(String, String), PlanError> {
    let git = PathBuf::from("git");
    let branch = default_branch(runner, workspace)?;
    let base = merge_base(&git, workspace, runner, &branch, "HEAD").map_err(PlanError::Git)?;
    Ok((branch, base))
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
        skipped_external: Vec::new(),
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
            &dx_plan::plan::registry::all_aspect_labels(),
            Some((git.as_path(), plan.base.as_str())),
        )?
    };
    for path in &overrides {
        selection.widened.push(WidenedScope {
            scope: "//...".to_owned(),
            reason: format!("local_path_override {path}"),
            repos: Vec::new(),
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
        let (tests, external) =
            split_external(tests_of(&rules, workspace, runner, startup_options)?);
        plan.skipped_external = external;
        tests
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

/// Splits off external-repo labels, which the test phase rejects as `invalid_scope`.
/// Main-repo spellings (`@//`, `@@//`) stay.
fn split_external(tests: Vec<String>) -> (Vec<String>, Vec<String>) {
    tests.into_iter().partition(|label| {
        !label.starts_with('@') || label.starts_with("@//") || label.starts_with("@@//")
    })
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
        let started = command_started(invocation.command.name(), invocation.started_mode()).map(
            |mut started| {
                if let Value::Object(map) = &mut started {
                    map.insert("affected".to_owned(), plan.json(invocation.command));
                }
                started
            },
        );
        if let Err(exit) = emit_built(out, started) {
            return exit;
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
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
        audit_today,
        actions,
        proc_root,
        bump_commit_json,
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
    next.affected_handoff = Some(dx_plan::args::AffectedHandoff {
        tests: plan.tests.clone(),
    });
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
                audit_today,
                actions,
                proc_root,
                bump_commit_json,
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
            audit_today,
            actions,
            proc_root,
            bump_commit_json,
        },
    );
    let _ = injector.finish();
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_plan::resolve::QueryResult;
    use dx_test_scratch::write;
    use std::cell::RefCell;
    use std::process::Command as Process;

    fn widened_plan(entries: &[(&str, &str)]) -> AffectedPlan {
        AffectedPlan {
            base: "1a2b3c4d5e6f7890".to_string(),
            changed_files: vec!["quality/fixtures.bzl".to_string()],
            targets: Vec::new(),
            tests: Vec::new(),
            widened: entries
                .iter()
                .map(|(scope, reason)| WidenedScope {
                    scope: (*scope).to_string(),
                    reason: (*reason).to_string(),
                    repos: Vec::new(),
                })
                .collect(),
            unscoped: false,
            skipped_external: Vec::new(),
        }
    }

    #[test]
    fn root_package_labels_are_audited_only_when_a_root_scoped_set_exists() {
        let set = |scope: &str| {
            format!(
                "schema_version = 1\n[[dependency_set]]\nname = \"web\"\necosystem = \"npm\"\nmanifests = [\"package.json\"]\nlocks = [\"pnpm-lock.yaml\"]\nscopes = [\"{scope}\"]\n"
            )
        };
        let root = dx_test_scratch::scratch("dx-affected-root-scope-");
        write(root.path(), "dx.toml", &set("."));
        assert!(audit_owned(root.path(), "//:node_modules"));
        assert!(!audit_owned(root.path(), "//foo:bar"));
        let other = dx_test_scratch::scratch("dx-affected-other-scope-");
        write(other.path(), "dx.toml", &set("apps/web"));
        assert!(!audit_owned(other.path(), "//:node_modules"));
    }

    #[test]
    fn widened_summary_caps_names_and_json_keeps_all() {
        let scopes: Vec<String> = (0..480).map(|n| format!("//p{n}:all")).collect();
        let entries: Vec<(&str, &str)> = scopes
            .iter()
            .map(|scope| (scope.as_str(), "loads quality/fixtures.bzl"))
            .collect();
        let plan = widened_plan(&entries);
        let summary = plan.summary();
        assert!(
            summary.contains("//p0:all (loads quality/fixtures.bzl)"),
            "{summary}"
        );
        assert!(
            summary.contains("//p2:all (loads quality/fixtures.bzl)"),
            "{summary}"
        );
        assert!(summary.contains("and 477 more"), "{summary}");
        assert!(!summary.contains("//p3:all"), "{summary}");
        let json = plan.json(Command::Build);
        assert_eq!(json["widened"].as_array().map(Vec::len), Some(480));
    }

    #[test]
    fn widened_summary_lists_everything_scope_first() {
        let plan = widened_plan(&[
            ("//a:all", "r"),
            ("//a:all", "r"),
            ("//b:all", "r"),
            ("//...", "workspace input MODULE.bazel"),
        ]);
        let summary = plan.summary();
        assert!(
            summary.ends_with(
                "; widened to //... (workspace input MODULE.bazel), //a:all (r), //b:all (r)"
            ),
            "{summary}"
        );
    }

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
    fn empty_selection_reports_mode_for_every_affected_command() {
        use clap::ValueEnum;
        let dir = repo();
        let (plan, _) = run(dir.path(), Command::Test, &[]);
        let cases: [(&[&str], &str); 16] = [
            (&["check", "--affected", "--output=json"], "check"),
            (&["fix", "--affected", "--output=json"], "check"),
            (
                &["fix", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["fix", "--check", "--affected", "--output=json"], "check"),
            (&["lint", "--affected", "--output=json"], "check"),
            (
                &["lint", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["typecheck", "--affected", "--output=json"], "check"),
            (
                &["typecheck", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["format", "--affected", "--output=json"], "check"),
            (
                &["format", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["build", "--affected", "--output=json"], "default"),
            (&["test", "--affected", "--output=json"], "default"),
            (&["coverage", "--affected", "--output=json"], "default"),
            (&["security", "--affected", "--output=json"], "default"),
            (&["license", "--affected", "--output=json"], "default"),
            (
                &["check", "--check", "--affected", "--output=json"],
                "check",
            ),
        ];
        let mut covered: Vec<String> = cases.iter().map(|(w, _)| w[0].to_owned()).collect();
        covered.sort();
        covered.dedup();
        let mut every: Vec<String> = Command::value_variants()
            .iter()
            .filter(|command| {
                **command != Command::Watch
                    && dx_plan::args::grammar::accepts_long(**command, "affected")
            })
            .map(|command| command.name().to_owned())
            .collect();
        every.sort();
        assert_eq!(
            covered, every,
            "cases must cover every command that takes --affected"
        );
        for (words, expected) in cases {
            let invocation =
                dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parses");
            let mut out = Vec::new();
            assert_eq!(finished_empty(&invocation, &plan, &mut out), 0, "{words:?}");
            let text = String::from_utf8(out).expect("utf8");
            let first: Value =
                serde_json::from_str(text.lines().next().expect("line")).expect("json");
            assert_eq!(first["mode"], expected, "{words:?}");
            assert!(first["affected"].is_object(), "{words:?}");
            if words[0] == "check" {
                assert_eq!(first["affected"]["tests"], json!([]), "{words:?}");
            } else if words[0] == "test" {
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
    fn renames_count_both_paths_and_keep_owners() {
        let dir = repo();
        let root = dir.path();
        git(root, &["mv", "a/a.txt", "a/c.txt"]);
        let graph = graph();
        let (plan, _) = run(root, Command::Lint, &[&graph]);
        assert_eq!(plan.changed_files, ["a/a.txt", "a/c.txt"]);
        assert!(plan.widened.is_empty());
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
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
    fn scl_change_selects_loading_package_and_rdeps() {
        let dir = repo();
        let root = dir.path();
        write(root, "defs.scl", "x = 1\n");
        write(root, "a/BUILD.bazel", "load(\"//:defs.scl\", \"x\")\n");
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "defs"]);
        git(root, &["branch", "--force", "main"]);
        write(root, "defs.scl", "x = 2\n");
        let graph = graph();
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["defs.scl"]);
        assert_eq!(
            plan.targets,
            ["//a:android", "//a:lib", "//a:ruby", "//a:t"]
        );
        assert_eq!(plan.widened.len(), 1);
        assert_eq!(plan.widened[0].scope, "//a:all");
        let summary = plan.summary();
        assert!(
            summary.contains("widened to //a:all (loads defs.scl)"),
            "{summary}"
        );
        assert!(!summary.contains("//..."), "{summary}");
        assert_eq!(
            plan.json(Command::Build)["widened"],
            json!([{"scope": "//a:all", "reason": "loads defs.scl"}])
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
    fn build_file_edit_selects_its_package_and_rdeps() {
        let dir = repo();
        let root = dir.path();
        write(root, "a/BUILD.bazel", "# edit\n");
        let graph = graph();
        let (plan, _) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["a/BUILD.bazel"]);
        assert_eq!(
            plan.targets,
            ["//a:android", "//a:lib", "//a:ruby", "//a:t"]
        );
        assert_eq!(
            plan.json(Command::Build)["widened"],
            json!([{"scope": "//a:all", "reason": "build file a/BUILD.bazel"}])
        );
        let summary = plan.summary();
        assert!(
            summary.contains("widened to //a:all (build file a/BUILD.bazel)"),
            "{summary}"
        );
        assert!(!summary.contains("//..."), "{summary}");
        let (plan, _) = run(root, Command::Test, &[&graph, "//a:t\n"]);
        assert_eq!(plan.targets, ["//a:t"]);
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
    fn external_repo_tests_are_skipped_and_reported() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n");
        let graph = graph();
        let answer = "//a:t\n@shared_consumer//:x_test\n@@//a:u\n";
        let (plan, _) = run(dir.path(), Command::Check, &[&graph, answer]);
        assert_eq!(plan.tests, ["//a:t", "@@//a:u"]);
        assert_eq!(plan.skipped_external, ["@shared_consumer//:x_test"]);
        assert_eq!(
            plan.json(Command::Check)["skipped_external"],
            json!(["@shared_consumer//:x_test"])
        );
        assert!(plan
            .summary()
            .contains("skipping 1 external-repo tests: @shared_consumer//:x_test"));
        let (plan, _) = run(dir.path(), Command::Test, &[&graph, answer]);
        assert_eq!(plan.targets, ["//a:t", "@@//a:u"]);
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
            skipped_external: Vec::new(),
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
    fn security_with_no_changes_is_empty() {
        let dir = repo();
        let (plan, _) = run(dir.path(), Command::Security, &[]);
        assert!(plan.changed_files.is_empty());
        assert!(!plan.unscoped);
        assert!(plan.empty());
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

    fn cargo_lock(serde: &str) -> String {
        format!(
            "version = 3\n\n[[package]]\nname = \"serde\"\nversion = \"{serde}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
        )
    }

    #[test]
    fn json_reports_the_crates_a_cargo_lock_bump_selects() {
        let dir = repo();
        let root = dir.path();
        write(
            root,
            "MODULE.bazel",
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"//:Cargo.lock\")\n",
        );
        write(root, "Cargo.lock", &cargo_lock("1.0.1"));
        git(root, &["add", "."]);
        git(root, &["commit", "--quiet", "-m", "crates"]);
        git(root, &["branch", "--force", "main"]);
        write(root, "Cargo.lock", &cargo_lock("1.0.2"));
        let graph = [
            json!({"name": "//a:serde_user", "ruleInput": ["@@rules_rust++crate+crates//:serde"]}),
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}),
        ]
        .iter()
        .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["Cargo.lock"]);
        assert_eq!(plan.targets, ["//a:serde_user"]);
        assert_eq!(
            plan.json(Command::Build)["widened"],
            json!([{
                "scope": "rdeps(//..., @crates//:serde)",
                "reason": "changed crates in Cargo.lock",
                "repos": ["@crates//:serde"],
            }])
        );
        assert!(
            plan.summary().contains(
                "widened to rdeps(//..., @crates//:serde) (changed crates in Cargo.lock)"
            ),
            "{}",
            plan.summary()
        );
        assert_eq!(*runner.expressions.borrow(), ["kind('rule', //...)"]);
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
