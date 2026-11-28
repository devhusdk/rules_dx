use super::execute;
use super::umbrella::lint_commits;
use dx_exec_core::common::{
    Env, check_stdout_write, emit_built, emit_event, refusal, resolve_failed, set_field,
};
use dx_output::{FinishedCounts, OutputMode, command_finished, command_started};
use dx_plan::args::{Command, Invocation, TagFilter};
use dx_plan::resolve::base::{PlanError, affected_base, since_base};
use dx_plan::resolve::{
    AffectedSelection, QueryRunner, RuleGraph, WidenedScope, collect_range, collect_working_tree,
    exclude, local_override_changes, resolve, resolve_affected,
};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) struct AffectedPlan {
    pub(crate) base: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) targets: Vec<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) coverage_tests: Option<Vec<String>>,
    pub(crate) generate: Vec<String>,
    pub(crate) format: Vec<String>,
    pub(crate) lint: Vec<String>,
    pub(crate) widened: Vec<WidenedScope>,
    pub(crate) unscoped: bool,
    pub(crate) graph: Option<RuleGraph>,
}

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
                    set_field(&mut value, "repos", json!(entry.repos));
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
            set_field(&mut value, "tests", json!(self.tests));
            if let Some(coverage_tests) = &self.coverage_tests {
                set_field(&mut value, "coverage_tests", json!(coverage_tests));
            }
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
    matches!(command, Command::Test | Command::Coverage)
}

fn audit_owned(workspace: &Path, target: &str) -> bool {
    match dx_adopt::dependency_sets::load(workspace) {
        Ok(Some(registry)) => !dx_adopt::dependency_sets::owning_sets(&registry, target).is_empty(),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LintScope {
    DirectUsers,
    Dependents,
}

fn lint_scope(selection: &AffectedSelection, requested: LintScope) -> LintScope {
    if requested == LintScope::Dependents
        || selection
            .widened
            .iter()
            .any(|entry| !entry.repos.is_empty())
    {
        LintScope::Dependents
    } else {
        LintScope::DirectUsers
    }
}

fn scope_to_graph(
    plan: &mut AffectedPlan,
    selection: &mut AffectedSelection,
    command: Command,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    requested_lint: LintScope,
) -> Result<(), PlanError> {
    let secrets_scan = command == Command::Security;
    if selection.targets.is_empty() {
        plan.unscoped = secrets_scan;
        return Ok(());
    }
    let graph = selection
        .graph
        .take()
        .map_or_else(|| RuleGraph::load(workspace, runner, startup_options), Ok)?;
    let excludes = exclude::load(workspace)?;
    if command == Command::Deploy {
        let mut deployments = graph.deployments(&selection.targets);
        deployments.retain(|rule| !exclude::covered(&excludes, rule));
        plan.targets = deployments;
        return Ok(());
    }
    let mut rules = graph.affected(&selection.targets);
    rules.retain(|rule| !exclude::covered(&excludes, rule));
    let tests = if tests_only(command) || command == Command::Check {
        let mut tests = graph.affected_tests(&selection.targets, &rules);
        tests.retain(|test| !exclude::covered(&excludes, test));
        tests
    } else {
        Vec::new()
    };
    let quality = matches!(command, Command::Format | Command::Lint | Command::Check);
    let mut owners = if quality {
        graph.own_sources(&selection.targets)
    } else {
        Vec::new()
    };
    owners.retain(|rule| !exclude::covered(&excludes, rule));
    let mut lint = if quality && lint_scope(selection, requested_lint) == LintScope::Dependents {
        rules.clone()
    } else if quality {
        graph.own_sources_and_direct_users(&selection.targets)
    } else {
        Vec::new()
    };
    lint.retain(|rule| !exclude::covered(&excludes, rule));
    plan.targets = if tests_only(command) {
        tests.clone()
    } else if command == Command::Format {
        owners.clone()
    } else if command == Command::Lint {
        lint.clone()
    } else {
        rules
    };
    if command == Command::Check && !plan.targets.is_empty() {
        plan.tests = tests;
        plan.generate = generate_scope(workspace, &plan.changed_files);
        if plan.generate.is_empty() {
            plan.generate = dx_plan::plan::generate_package_scope(&plan.targets);
        }
        plan.format = owners;
        plan.lint = lint;
    }
    if matches!(command, Command::Security | Command::License) {
        plan.targets.retain(|target| audit_owned(workspace, target));
        plan.unscoped = secrets_scan && plan.targets.is_empty();
    }
    plan.graph = Some(graph);
    Ok(())
}

pub(crate) fn plan(
    command: Command,
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    aspect_labels: &[String],
    base: String,
    lint: LintScope,
) -> Result<AffectedPlan, PlanError> {
    let git = PathBuf::from("git");
    let mut changes = collect_range(&git, workspace, runner, &base, "HEAD")
        .map_err(|error| PlanError::Git(error.to_string()))?;
    changes.extend(
        collect_working_tree(&git, workspace, runner)
            .map_err(|error| PlanError::Git(error.to_string()))?,
    );
    let changes = dx_adopt::dedupe_changes(changes);
    let overrides = local_override_changes(&git, workspace, runner, &base)
        .map_err(|error| PlanError::Git(error.to_string()))?;
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
        coverage_tests: None,
        generate: Vec::new(),
        format: Vec::new(),
        lint: Vec::new(),
        widened: Vec::new(),
        unscoped: false,
        graph: None,
    };
    if changes.is_empty() && overrides.is_empty() {
        return Ok(plan);
    }
    if command == Command::Generate {
        plan.targets = generate_scope(workspace, &plan.changed_files);
        return Ok(plan);
    }
    let mut selection = if changes.is_empty() {
        AffectedSelection {
            targets: Vec::new(),
            widened: Vec::new(),
            aspect_widened: Vec::new(),
            graph: None,
        }
    } else {
        resolve_affected(
            &changes,
            workspace,
            runner,
            startup_options,
            aspect_labels,
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
    let aspects_widen = runs_aspects(command) && !selection.aspect_widened.is_empty();
    if aspects_widen {
        selection.widened.append(&mut selection.aspect_widened);
    }
    plan.widened.clone_from(&selection.widened);
    if command != Command::Deploy && selection.targets.iter().any(|target| target == "//...") {
        if command == Command::Check {
            plan.tests = vec!["//...".to_owned()];
            plan.generate = vec!["//...".to_owned()];
            plan.format = vec!["//...".to_owned()];
            plan.lint = vec!["//...".to_owned()];
        }
        plan.targets = vec!["//...".to_owned()];
        plan.graph = selection.graph.take();
        return Ok(plan);
    }
    scope_to_graph(
        &mut plan,
        &mut selection,
        command,
        workspace,
        runner,
        startup_options,
        lint,
    )?;
    if aspects_widen {
        widen_aspect_scopes(&mut plan, command);
    }
    Ok(plan)
}

fn runs_aspects(command: Command) -> bool {
    command == Command::Check || !dx_plan::plan::registry::spec(command).aspects.is_empty()
}

fn widen_aspect_scopes(plan: &mut AffectedPlan, command: Command) {
    let everything = vec!["//...".to_owned()];
    if command == Command::Check {
        plan.format.clone_from(&everything);
        plan.lint.clone_from(&everything);
    }
    plan.targets = everything;
}

const BAZEL_LOCK: &str = "MODULE.bazel.lock";

fn generate_scope(workspace: &Path, changed_files: &[String]) -> Vec<String> {
    let labels: Vec<String> = changed_files
        .iter()
        .filter(|file| file.as_str() != BAZEL_LOCK)
        .map(|file| {
            let mut dir = Path::new(file).parent();
            while let Some(path) = dir {
                if workspace.join(path).is_dir() {
                    break;
                }
                dir = path.parent();
            }
            let dir = dir.map(|path| path.to_string_lossy().replace('\\', "/"));
            format!("//{}:all", dir.unwrap_or_default())
        })
        .collect();
    dx_plan::plan::generate_package_scope(&labels)
}

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
        match buf
            .iter()
            .position(|byte| *byte == b'\n')
            .and_then(|end| buf.split_at_checked(end))
        {
            None => {
                self.first.extend_from_slice(buf);
                Ok(buf.len())
            }
            Some((head, rest)) => {
                self.first.extend_from_slice(head);
                let line = std::mem::take(&mut self.first);
                self.done = true;
                let mut rewritten = self.rewrite(&line);
                rewritten.push(b'\n');
                self.inner.write_all(&rewritten)?;
                self.inner
                    .write_all(rest.strip_prefix(b"\n").unwrap_or(rest))?;
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn no_affected_targets(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> std::io::Result<()> {
    if output == OutputMode::Json {
        Ok(())
    } else if output == OutputMode::Diff {
        writeln!(err, "no affected targets")
    } else {
        writeln!(out, "no affected targets")
    }
}

fn finished_empty(
    invocation: &Invocation,
    plan: &AffectedPlan,
    workspace: &Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let lint = invocation.command == Command::Check && !invocation.apply;
    let json = invocation.output == OutputMode::Json;
    if json {
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
    } else if no_affected_targets(invocation.output, out, err).is_err() {
        return 1;
    }
    let mut code = 0;
    if lint {
        match lint_commits(
            &plan.base,
            invocation.command,
            invocation.output,
            workspace,
            out,
            err,
        ) {
            Ok(true) => code = 1,
            Ok(false) => {}
            Err(exit) => return exit,
        }
    }
    if json && let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default()))
    {
        return exit;
    }
    code
}

fn report(
    failure: &PlanError,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match failure {
        PlanError::Git(message) => refusal(output, out, err, message),
        PlanError::Resolve(error) => resolve_failed(output, out, err, error),
    }
}

fn resolve_plan(
    invocation: &Invocation,
    workspace: &Path,
    query_runner: &dyn QueryRunner,
    err: &mut dyn Write,
    text: bool,
) -> Result<AffectedPlan, PlanError> {
    let base = match &invocation.since {
        Some(rev) => since_base(workspace, query_runner, rev),
        None => affected_base(workspace, query_runner, invocation.base.as_deref()),
    }?;
    if text {
        dx_output::write_stderr(err, format_args!("{}\n", resolving_line(&base)));
    }
    let mut plan = plan(
        invocation.command,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
        &dx_plan::plan::registry::all_aspect_labels(),
        base,
        if invocation.lint_closure {
            LintScope::Dependents
        } else {
            LintScope::DirectUsers
        },
    )?;
    if !invocation.targets.is_empty() && !plan.empty() {
        let scope = resolve(
            &invocation.targets,
            workspace,
            query_runner,
            &invocation.bazel_startup_options,
        )?;
        narrow_to_scope(&mut plan, &scope.targets);
    }
    if invocation.command == Command::Check && !plan.empty() {
        split_tests(
            &mut plan,
            invocation,
            workspace,
            query_runner,
            &TagFilter::from_options(invocation.raw_bazel_options()),
        )?;
    }
    Ok(plan)
}

fn split_tests(
    plan: &mut AffectedPlan,
    invocation: &Invocation,
    workspace: &Path,
    runner: &dyn QueryRunner,
    filter: &TagFilter,
) -> Result<(), PlanError> {
    let whole_repo = plan.tests == ["//..."];
    if invocation.coverage && !plan.tests.is_empty() {
        let graph = match plan.graph.take() {
            Some(graph) => graph,
            None => RuleGraph::load(workspace, runner, &invocation.bazel_startup_options)?,
        };
        let candidates = if whole_repo {
            let excludes = exclude::load(workspace)?;
            let mut tests = graph.tests_of(&graph.own_sources(&plan.tests));
            tests.retain(|test| !exclude::covered(&excludes, test));
            tests
        } else {
            plan.tests.clone()
        };
        let (coverage, plain) = graph.coverage_split(&candidates);
        plan.coverage_tests = Some(if whole_repo {
            plan.tests.clone()
        } else {
            coverage
        });
        plan.tests = plain;
        plan.graph = Some(graph);
    }
    if let Some(graph) = &plan.graph {
        if !graph.admits_any(&plan.tests, filter) {
            plan.tests.clear();
        }
        if let Some(coverage) = plan
            .coverage_tests
            .as_mut()
            .filter(|coverage| !graph.admits_any(coverage, filter))
        {
            coverage.clear();
        }
    }
    Ok(())
}

fn narrow_to_scope(plan: &mut AffectedPlan, scope: &[String]) {
    plan.targets = exclude::within(&plan.targets, scope);
    plan.tests = exclude::within(&plan.tests, scope);
    plan.format = exclude::within(&plan.format, scope);
    plan.lint = exclude::within(&plan.lint, scope);
    plan.generate = dx_plan::plan::generate_package_scope_within(&plan.generate, scope);
}

fn scope_handoff(next: &mut Invocation, plan: &AffectedPlan, workspace: &Path) {
    next.targets.clone_from(&plan.targets);
    next.affected_handoff = Some(dx_plan::args::AffectedHandoff {
        tests: plan.tests.clone(),
        coverage_tests: plan.coverage_tests.clone().unwrap_or_default(),
        base: Some(plan.base.clone()),
        generate: plan.generate.clone(),
        format: plan.format.clone(),
        lint: plan.lint.clone(),
        writable: Some(writable_paths(workspace, &plan.changed_files)),
    });
}

const BUILD_FILE_NAMES: [&str; 2] = ["BUILD.bazel", "BUILD"];

pub(crate) fn writable_paths(workspace: &Path, changed_files: &[String]) -> Vec<String> {
    let mut paths = changed_files.to_vec();
    for file in changed_files {
        if workspace.join(file).is_dir() {
            continue;
        }
        let mut dir = Path::new(file).parent();
        while let Some(path) = dir {
            let build = BUILD_FILE_NAMES
                .iter()
                .map(|name| path.join(name))
                .find(|build| workspace.join(build).is_file());
            if let Some(build) = build {
                paths.push(build.to_string_lossy().replace('\\', "/"));
                break;
            }
            dir = path.parent();
        }
    }
    paths.sort();
    paths.dedup();
    paths
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
        edit_commit_json,
    } = env;
    let text = !invocation.quiet && matches!(invocation.output, OutputMode::Text { .. });
    let plan = match resolve_plan(invocation, workspace, query_runner, err, text) {
        Ok(plan) => plan,
        Err(failure) => return report(&failure, invocation.output, out, err),
    };
    if plan.empty() {
        return finished_empty(invocation, &plan, workspace, out, err);
    }
    if text {
        dx_output::write_stderr(err, format_args!("{}\n", plan.summary()));
    }
    let mut next = invocation.clone();
    next.affected = false;
    if invocation.command == Command::Deploy {
        let mut code = 0;
        for label in &plan.targets {
            next.targets = vec![label.clone()];
            let env = Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce,
                out: &mut *out,
                err: &mut *err,
                ci,
                audit_today,
                actions,
                proc_root,
                edit_commit_json,
            };
            let step = execute(&next, env);
            if code == 0 {
                code = step;
            }
        }
        return code;
    }
    scope_handoff(&mut next, &plan, workspace);
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
                edit_commit_json,
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
            edit_commit_json,
        },
    );
    exit_after_finish(code, injector.finish())
}

fn exit_after_finish(code: i32, finished: std::io::Result<()>) -> i32 {
    if code != 0 {
        return code;
    }
    match check_stdout_write(finished) {
        Ok(()) => 0,
        Err(exit) => exit,
    }
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
            changed_files: vec!["core/quality/fixtures.bzl".to_string()],
            targets: Vec::new(),
            tests: Vec::new(),
            coverage_tests: None,
            generate: Vec::new(),
            format: Vec::new(),
            lint: Vec::new(),
            widened: entries
                .iter()
                .map(|(scope, reason)| WidenedScope {
                    scope: (*scope).to_string(),
                    reason: (*reason).to_string(),
                    repos: Vec::new(),
                })
                .collect(),
            unscoped: false,
            graph: None,
        }
    }

    #[test]
    fn root_package_labels_are_audited_only_when_a_root_scoped_set_exists() {
        let lock = dx_adopt::dependency_sets::Ecosystem::Npm.lockfiles()[0].as_str();
        let set = |package: &str| {
            format!(
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"web\", pnpm_lock = \"//{package}:{lock}\")\n"
            )
        };
        let root = dx_test_scratch::scratch("dx-affected-root-scope-").unwrap();
        write(root.path(), "MODULE.bazel", &set("")).unwrap();
        assert!(audit_owned(root.path(), "//:node_modules"));
        assert!(!audit_owned(root.path(), "//foo:bar"));
        let other = dx_test_scratch::scratch("dx-affected-other-scope-").unwrap();
        write(other.path(), "MODULE.bazel", &set("apps/web")).unwrap();
        assert!(!audit_owned(other.path(), "//:node_modules"));
    }

    #[test]
    fn widened_summary_caps_names_and_json_keeps_all() {
        let scopes: Vec<String> = (0..480).map(|n| format!("//p{n}:all")).collect();
        let entries: Vec<(&str, &str)> = scopes
            .iter()
            .map(|scope| (scope.as_str(), "loads core/quality/fixtures.bzl"))
            .collect();
        let plan = widened_plan(&entries);
        let summary = plan.summary();
        assert!(
            summary.contains("//p0:all (loads core/quality/fixtures.bzl)"),
            "{summary}"
        );
        assert!(
            summary.contains("//p2:all (loads core/quality/fixtures.bzl)"),
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
                seen = std::fs::read_to_string(path)?;
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

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_test_scratch::git(root, root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        write(root, "MODULE.bazel", "module(name = \"x\")\n").unwrap();
        write(root, "a/BUILD.bazel", "").unwrap();
        write(root, "a/a.txt", "a\n").unwrap();
        write(root, "a/b.txt", "b\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "base"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-b", "topic"]).unwrap();
        dir
    }

    fn graph() -> String {
        let rules = [
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt", "//a:b.txt", "//a:new.txt", "//a:key.txt", "//a:c.txt"]}),
            json!({"name": "//a:t", "ruleClass": "sh_test", "ruleInput": ["//a:lib"]}),
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
        run_linting(root, command, LintScope::DirectUsers, answers)
    }

    fn run_linting(
        root: &Path,
        command: Command,
        lint: LintScope,
        answers: &[&str],
    ) -> (AffectedPlan, GitRunner) {
        run_with_aspects(root, command, lint, &[], answers)
    }

    fn run_with_aspects(
        root: &Path,
        command: Command,
        lint: LintScope,
        aspects: &[String],
        answers: &[&str],
    ) -> (AffectedPlan, GitRunner) {
        let runner = GitRunner::new(answers);
        let base = affected_base(root, &runner, None).expect("base");
        let plan = plan(command, root, &runner, &[], aspects, base, lint).expect("plan");
        assert!(runner.bazel.borrow().is_empty(), "unused bazel answers");
        (plan, runner)
    }

    #[test]
    fn generate_scope_skips_the_bazel_lock() {
        let dir = dx_test_scratch::scratch("dx-affected-generate-lock-").unwrap();
        write(dir.path(), "svc/a.py", "x = 1\n").unwrap();
        let files = |names: &[&str]| -> Vec<String> {
            names.iter().map(|name| (*name).to_owned()).collect()
        };
        assert_eq!(
            generate_scope(dir.path(), &files(&["MODULE.bazel.lock", "svc/a.py"])),
            ["//svc:all"]
        );
        assert!(
            generate_scope(dir.path(), &files(&["MODULE.bazel.lock"])).is_empty(),
            "{:?}",
            generate_scope(dir.path(), &files(&["MODULE.bazel.lock"]))
        );
        assert_eq!(
            generate_scope(dir.path(), &files(&["MODULE.bazel", "svc/a.py"])),
            ["//:all", "//svc:all"]
        );
    }

    #[test]
    fn generate_scope_uses_the_nearest_existing_directory() {
        let dir = dx_test_scratch::scratch("dx-affected-generate-scope-").unwrap();
        write(dir.path(), "svc/new.py", "x = 1\n").unwrap();
        write(dir.path(), "lib/a/b.py", "x = 1\n").unwrap();
        let files = |names: &[&str]| -> Vec<String> {
            names.iter().map(|name| (*name).to_owned()).collect()
        };
        assert_eq!(
            generate_scope(
                dir.path(),
                &files(&["svc/new.py", "svc/gone/x.py", "lib/a/b.py"])
            ),
            files(&["//lib/a:all", "//svc:all"])
        );
        assert_eq!(
            generate_scope(dir.path(), &files(&["README.md", "svc/new.py"])),
            files(&["//:all", "//svc:all"])
        );
    }

    #[test]
    fn writable_paths_are_the_changed_files_and_their_package_build_files() {
        let dir = dx_test_scratch::scratch("dx-affected-writable-").unwrap();
        write(dir.path(), "BUILD.bazel", "").unwrap();
        write(dir.path(), "svc/BUILD", "").unwrap();
        write(dir.path(), "svc/src/a.rs", "").unwrap();
        write(dir.path(), "svc/sub/BUILD.bazel", "").unwrap();
        write(dir.path(), "lib/b.py", "").unwrap();
        let files = |names: &[&str]| -> Vec<String> {
            names.iter().map(|name| (*name).to_owned()).collect()
        };
        assert_eq!(
            writable_paths(
                dir.path(),
                &files(&["svc/src/a.rs", "lib/b.py", "svc/gone/c.rs", "README.md"])
            ),
            files(&[
                "BUILD.bazel",
                "README.md",
                "lib/b.py",
                "svc/BUILD",
                "svc/gone/c.rs",
                "svc/src/a.rs",
            ])
        );
    }

    #[test]
    fn empty_selection_reports_mode_for_every_affected_command() {
        use clap::ValueEnum;
        let dir = repo();
        let (plan, _) = run(dir.path(), Command::Test, &[]);
        let cases: [(&[&str], &str); 17] = [
            (&["check", "--affected", "--output=json"], "check"),
            (
                &["check", "--apply", "--affected", "--output=json"],
                "default",
            ),
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
            (&["generate", "--affected", "--output=json"], "check"),
            (
                &["generate", "--apply", "--affected", "--output=json"],
                "default",
            ),
            (&["build", "--affected", "--output=json"], "default"),
            (&["test", "--affected", "--output=json"], "default"),
            (&["coverage", "--affected", "--output=json"], "default"),
            (&["fetch", "--affected", "--output=json"], "default"),
            (&["security", "--affected", "--output=json"], "default"),
            (&["license", "--affected", "--output=json"], "default"),
            (&["check", "--affected", "--output=json"], "check"),
        ];
        let mut covered: Vec<String> = cases.iter().map(|(w, _)| w[0].to_owned()).collect();
        covered.sort();
        covered.dedup();
        let mut every: Vec<String> = Command::value_variants()
            .iter()
            .filter(|command| {
                !matches!(**command, Command::Watch | Command::Deploy)
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
            assert_eq!(
                finished_empty(
                    &invocation,
                    &plan,
                    Path::new("/nonexistent"),
                    &mut out,
                    &mut Vec::new()
                ),
                0,
                "{words:?}"
            );
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

    const COMMITS_ON: &str = "schema_version = 1\n[check]\ncommits = \"conventional\"\n";

    fn commits_repo(subject: &str) -> tempfile::TempDir {
        let dir = repo();
        let root = dir.path();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "main"]).unwrap();
        write(root, "dx.toml", COMMITS_ON).unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "chore: config"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-B", "topic"]).unwrap();
        dx_test_scratch::git(
            root,
            root,
            &["commit", "--quiet", "--allow-empty", "-m", subject],
        )
        .unwrap();
        dir
    }

    fn empty_exit(root: &Path, words: &[&str]) -> (i32, String, String) {
        let (plan, _) = run(root, Command::Check, &[]);
        assert!(plan.empty());
        let invocation = dx_plan::args::parse(&dx_testing::cli::strings(words)).expect("parses");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = finished_empty(&invocation, &plan, root, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("utf8"),
            String::from_utf8(err).expect("utf8"),
        )
    }

    #[test]
    fn empty_selection_in_diff_mode_leaves_stdout_empty() {
        let dir = repo();
        for words in [
            ["format", "--affected", "--diff"],
            ["lint", "--affected", "--diff"],
            ["check", "--affected", "--diff"],
        ] {
            let (code, out, err) = empty_exit(dir.path(), &words);
            assert_eq!(code, 0, "{words:?}");
            assert_eq!(out, "", "{words:?}");
            assert_eq!(err, "no affected targets\n", "{words:?}");
        }
        let (code, out, err) = empty_exit(dir.path(), &["format", "--affected"]);
        assert_eq!(code, 0);
        assert_eq!(out, "no affected targets\n");
        assert_eq!(err, "");
    }

    #[test]
    fn finished_empty_check_lints_commit_subjects() {
        let dir = commits_repo("update stuff");
        let (code, out, err) = empty_exit(dir.path(), &["check", "--affected"]);
        assert_eq!(code, 1);
        assert!(out.contains("no affected targets"), "{out}");
        assert!(
            err.contains("update stuff: not a conventional commit"),
            "{err}"
        );
    }

    #[test]
    fn finished_empty_check_lints_commit_subjects_in_json() {
        let dir = commits_repo("update stuff");
        let (code, out, _) = empty_exit(dir.path(), &["check", "--affected", "--output=json"]);
        assert_eq!(code, 1);
        let notice = out.find("commit_not_conventional").expect("notice");
        let finished = out.find("command_finished").expect("finished");
        assert!(notice < finished, "{out}");
        assert!(out.contains("\"exit_code\":1"), "{out}");
    }

    #[test]
    fn finished_empty_skips_the_lint_for_apply_and_other_commands() {
        let dir = commits_repo("update stuff");
        for words in [
            &["check", "--apply", "--affected"][..],
            &["lint", "--affected"][..],
        ] {
            let (code, _, err) = empty_exit(dir.path(), words);
            assert_eq!(code, 0, "{words:?}");
            assert!(err.is_empty(), "{words:?}: {err}");
        }
    }

    #[test]
    fn finished_empty_check_passes_conventional_subjects() {
        let dir = commits_repo("fix: update stuff");
        let (code, _, err) = empty_exit(dir.path(), &["check", "--affected"]);
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
    }

    #[test]
    fn clean_tree_selects_nothing() {
        let dir = repo();
        let (plan, runner) = run(dir.path(), Command::Test, &[]);
        assert!(plan.empty());
        assert!(plan.changed_files.is_empty(), "{:?}", plan.changed_files);
        assert!(runner.expressions.borrow().is_empty());
    }

    #[test]
    fn merge_base_diff_working_tree_and_untracked_files_count() {
        let dir = repo();
        let root = dir.path();
        write(root, "a/a.txt", "committed\n").unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-am", "topic"]).unwrap();
        write(root, "a/b.txt", "unstaged\n").unwrap();
        write(root, "a/new.txt", "new\n").unwrap();
        let graph = graph();
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["a/a.txt", "a/b.txt", "a/new.txt"]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert!(plan.widened.is_empty(), "{:?}", plan.widened);
        assert_eq!(runner.expressions.borrow().len(), 1);
    }

    #[test]
    fn since_replaces_the_merge_base() {
        let dir = repo();
        let root = dir.path();
        write(root, "a/a.txt", "first\n").unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-am", "first"]).unwrap();
        write(root, "a/b.txt", "second\n").unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-am", "second"]).unwrap();
        let runner = GitRunner::new(&[]);
        let base = since_base(root, &runner, "HEAD~1").expect("base");
        let graph = graph();
        let runner = GitRunner::new(&[&graph]);
        let plan = plan(
            Command::Build,
            root,
            &runner,
            &[],
            &[],
            base.clone(),
            LintScope::DirectUsers,
        )
        .expect("plan");
        assert_eq!(plan.changed_files, ["a/b.txt"]);
        assert_eq!(plan.base, base);
        let (all, _) = run(root, Command::Build, &[&graph]);
        assert_eq!(all.changed_files, ["a/a.txt", "a/b.txt"]);
    }

    #[test]
    fn the_universe_query_never_names_rdeps_or_external_repos() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (_, runner) = run(dir.path(), Command::Build, &[&graph]);
        for expression in runner.expressions.borrow().iter() {
            assert_eq!(expression, "kind('rule', //...)");
        }
    }

    #[test]
    fn rules_that_reach_unloadable_or_other_platform_repos_do_not_matter() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Build, &[&graph]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert!(
            !plan
                .targets
                .iter()
                .any(|target| target == "//a:android" || target == "//a:ruby")
        );
    }

    #[test]
    fn renames_count_both_paths_and_keep_owners() {
        let dir = repo();
        let root = dir.path();
        dx_test_scratch::git(root, root, &["mv", "a/a.txt", "a/c.txt"]).unwrap();
        let graph = graph();
        let (plan, _) = run(root, Command::Lint, &[&graph]);
        assert_eq!(plan.changed_files, ["a/a.txt", "a/c.txt"]);
        assert!(plan.widened.is_empty(), "{:?}", plan.widened);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
    }

    #[test]
    fn workspace_inputs_widen_to_everything_with_a_reason() {
        let dir = repo();
        write(dir.path(), "MODULE.bazel", "module(name = \"y\")\n").unwrap();
        let (plan, runner) = run(dir.path(), Command::Test, &[]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(plan.widened[0].scope, "//...");
        assert_eq!(plan.widened[0].reason, "MODULE.bazel changes module");
        assert!(plan.summary().contains("widened to //..."));
        assert!(runner.expressions.borrow().is_empty());
    }

    fn deploy_graph(ship_tags: &[&str]) -> String {
        [
            json!({"name": "//a:ship", "ruleClass": "deployment", "ruleInput": ["//a:lib"], "attribute": [{"name": "tags", "stringListValue": ship_tags}]}),
            json!({"name": "//a:stage", "ruleClass": "deployment", "ruleInput": ["//a:lib"], "attribute": [{"name": "tags", "stringListValue": ["manual"]}]}),
        ]
        .iter()
        .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }

    #[test]
    fn widened_deploy_selects_every_release_deployment() {
        let dir = repo();
        write(dir.path(), "MODULE.bazel", "module(name = \"y\")\n").unwrap();
        let graph = deploy_graph(&["release"]);
        let (plan, runner) = run(dir.path(), Command::Deploy, &[&graph]);
        assert_eq!(plan.targets, ["//a:ship"]);
        assert_eq!(plan.widened[0].scope, "//...");
        assert_eq!(*runner.expressions.borrow(), ["kind('rule', //...)"]);
    }

    #[test]
    fn widened_deploy_without_release_deployments_selects_nothing() {
        let dir = repo();
        write(dir.path(), "MODULE.bazel", "module(name = \"y\")\n").unwrap();
        let graph = deploy_graph(&["release-notes"]);
        let (plan, _) = run(dir.path(), Command::Deploy, &[&graph]);
        assert!(plan.targets.is_empty(), "{:?}", plan.targets);
        assert_eq!(plan.widened[0].scope, "//...");
    }

    #[test]
    fn bzl_change_selects_loading_package_and_rdeps() {
        let dir = repo();
        let root = dir.path();
        write(root, "defs.bzl", "x = 1\n").unwrap();
        write(root, "a/BUILD.bazel", "load(\"//:defs.bzl\", \"x\")\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "defs"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "--force", "main"]).unwrap();
        write(root, "defs.bzl", "x = 2\n").unwrap();
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
        write(root, "defs.scl", "x = 1\n").unwrap();
        write(root, "a/BUILD.bazel", "load(\"//:defs.scl\", \"x\")\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "defs"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "--force", "main"]).unwrap();
        write(root, "defs.scl", "x = 2\n").unwrap();
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
        write(root, "a/BUILD.bazel", "# edit\n").unwrap();
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
        let (plan, _) = run(root, Command::Test, &[&graph]);
        assert_eq!(plan.targets, ["//a:t"]);
    }

    fn fixture_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_test_scratch::git(root, root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        write(root, "MODULE.bazel", "module(name = \"rules_dx\")\n").unwrap();
        write(
            root,
            ".bazelignore",
            "go/tests/consumer\nlua/tests/consumer\n",
        )
        .unwrap();
        for lang in ["go", "lua"] {
            write(root, &format!("{lang}/lang.json"), "{}\n").unwrap();
            write(root, &format!("{lang}/tests/consumer/BUILD.bazel"), "").unwrap();
        }
        write(root, "lua/tests/consumer/hello.lua", "print(1)\n").unwrap();
        write(root, "tools/testing/consumer/BUILD.bazel", "").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "base"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-b", "topic"]).unwrap();
        dir
    }

    fn fixture_graph(snapshots: bool) -> String {
        let fixtures = "@dx_consumer_fixtures//";
        let snapshot = |file: &str| {
            if snapshots {
                vec![format!("{fixtures}:{file}")]
            } else {
                Vec::new()
            }
        };
        let rules = [
            json!({"name": "//tools/testing/consumer:go_consumer_test", "ruleClass": "rust_test", "ruleInput": snapshot("go.json")}),
            json!({"name": "//tools/testing/consumer:lua_consumer_test", "ruleClass": "rust_test", "ruleInput": snapshot("lua.json")}),
            json!({"name": "//tools/testing/consumer:in_graph_guard_test", "ruleClass": "rust_test", "ruleInput": snapshot("consumer.json")}),
            json!({"name": "//tools/testing/consumer:lib_test", "ruleClass": "rust_test", "ruleInput": []}),
        ];
        rules
            .iter()
            .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_consumer_fixture_edit_selects_the_tests_that_read_its_snapshot() {
        let dir = fixture_repo();
        let root = dir.path();
        write(root, "lua/tests/consumer/hello.lua", "print(2)\n").unwrap();
        let graph = fixture_graph(true);
        let (plan, _) = run(root, Command::Test, &[&graph]);
        assert_eq!(plan.changed_files, ["lua/tests/consumer/hello.lua"]);
        assert_eq!(
            plan.targets,
            [
                "//tools/testing/consumer:in_graph_guard_test",
                "//tools/testing/consumer:lua_consumer_test",
            ]
        );
        assert_eq!(
            plan.json(Command::Test)["widened"][0]["reason"],
            json!("consumer fixture lua/tests/consumer/hello.lua")
        );
    }

    #[test]
    fn a_consumer_fixture_no_rule_reads_widens_to_the_repository() {
        let dir = fixture_repo();
        let root = dir.path();
        write(root, "lua/tests/consumer/hello.lua", "print(2)\n").unwrap();
        let graph = fixture_graph(false);
        let (plan, _) = run(root, Command::Test, &[&graph]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(
            plan.json(Command::Test)["widened"],
            json!([{
                "scope": "//...",
                "reason": "consumer fixture lua/tests/consumer/hello.lua, and no rule reads its snapshot",
            }])
        );
    }

    #[test]
    fn an_unowned_file_under_a_package_without_rules_widens_to_the_repository() {
        let dir = repo();
        let root = dir.path();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "main"]).unwrap();
        write(root, "vendored/BUILD.bazel", "").unwrap();
        write(root, "vendored/notes.txt", "a\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "vendored"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-B", "topic"]).unwrap();
        write(root, "vendored/notes.txt", "b\n").unwrap();
        let graph = graph();
        let (plan, _) = run(root, Command::Test, &[&graph]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(
            plan.json(Command::Test)["widened"],
            json!([{
                "scope": "//...",
                "reason": "unowned vendored/notes.txt, and no rule is in //vendored/...",
            }])
        );
    }

    #[test]
    fn a_change_only_under_an_excluded_package_selects_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_test_scratch::git(root, root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        write(root, "MODULE.bazel", "module(name = \"x\")\n").unwrap();
        write(
            root,
            "dx.toml",
            "schema_version = 1\n[scope]\nexclude = [\"//a/...\"]\n",
        )
        .unwrap();
        write(root, "a/BUILD.bazel", "").unwrap();
        write(root, "a/a.txt", "a\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "base"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-b", "topic"]).unwrap();
        write(root, "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        for command in [Command::Build, Command::Test, Command::Check, Command::Lint] {
            let (plan, runner) = run(root, command, &[&graph]);
            assert!(plan.targets.is_empty(), "{command:?}: {:?}", plan.targets);
            assert!(plan.tests.is_empty(), "{command:?}");
            assert_eq!(runner.expressions.borrow().len(), 1, "no tests query");
        }
    }

    #[test]
    fn tests_come_from_the_graph_and_check_adds_a_test_list() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, runner) = run(dir.path(), Command::Test, &[&graph]);
        assert_eq!(plan.targets, ["//a:t"]);
        assert_eq!(runner.expressions.borrow().len(), 1, "only the graph query");

        let (plan, runner) = run(dir.path(), Command::Check, &[&graph]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert_eq!(plan.tests, ["//a:t"]);
        assert_eq!(runner.expressions.borrow().len(), 1, "only the graph query");
    }

    #[test]
    fn check_scopes_generate_to_changed_packages_and_format_to_owners() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Check, &[&graph]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        assert_eq!(plan.generate, ["//a:all"]);
        assert_eq!(plan.format, ["//a:lib"]);
    }

    #[test]
    fn format_selects_owners_and_lint_adds_their_direct_users() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let (plan, _) = run(dir.path(), Command::Format, &[&graph()]);
        assert_eq!(plan.targets, ["//a:lib"]);
        let (plan, _) = run(dir.path(), Command::Lint, &[&graph()]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
        let (plan, _) = run(dir.path(), Command::Typecheck, &[&graph()]);
        assert_eq!(plan.targets, ["//a:lib", "//a:t"]);
    }

    fn rules_graph(rules: &[Value]) -> String {
        rules
            .iter()
            .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn check_lints_a_changed_library_and_its_direct_dependents_and_typechecks_all_users() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = rules_graph(&[
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}),
            json!({"name": "//a:unit", "ruleClass": "rust_test", "ruleInput": ["//a:lib"],
                "attribute": [{"name": "crate", "stringValue": "//a:lib"}]}),
            json!({"name": "//a:bin", "ruleInput": ["//a:lib"]}),
            json!({"name": "//a:t", "ruleClass": "sh_test", "ruleInput": ["//a:bin"]}),
        ]);
        let (plan, _) = run(dir.path(), Command::Check, &[&graph]);
        assert_eq!(plan.targets, ["//a:bin", "//a:lib", "//a:t", "//a:unit"]);
        assert_eq!(plan.tests, ["//a:t", "//a:unit"]);
        assert_eq!(plan.format, ["//a:lib", "//a:unit"]);
        assert_eq!(plan.lint, ["//a:bin", "//a:lib", "//a:unit"]);
        let (plan, _) = run(dir.path(), Command::Lint, &[&graph]);
        assert_eq!(plan.targets, ["//a:bin", "//a:lib", "//a:unit"]);
    }

    #[test]
    fn closure_lint_scope_reaches_users_two_hops_from_a_change() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = rules_graph(&[
            json!({"name": "//a:a", "ruleInput": ["//a:a.txt"]}),
            json!({"name": "//b:b", "ruleInput": ["//a:a"]}),
            json!({"name": "//c:c", "ruleInput": ["//b:b"]}),
        ]);
        let (plan, _) = run(dir.path(), Command::Lint, &[&graph]);
        assert_eq!(plan.targets, ["//a:a", "//b:b"]);
        let (plan, _) = run_linting(dir.path(), Command::Lint, LintScope::Dependents, &[&graph]);
        assert_eq!(plan.targets, ["//a:a", "//b:b", "//c:c"]);
        let (plan, _) = run(dir.path(), Command::Check, &[&graph]);
        assert_eq!(plan.lint, ["//a:a", "//b:b"]);
        let (plan, _) = run_linting(dir.path(), Command::Check, LintScope::Dependents, &[&graph]);
        assert_eq!(plan.format, ["//a:a"]);
        assert_eq!(plan.lint, ["//a:a", "//b:b", "//c:c"]);
        assert_eq!(plan.targets, ["//a:a", "//b:b", "//c:c"]);
    }

    #[test]
    fn check_and_lint_select_nothing_to_lint_when_every_owner_is_manual() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let manual = json!([{"name": "tags", "stringListValue": ["manual"]}]);
        let graph = rules_graph(&[
            json!({"name": "//a:hand", "ruleInput": ["//a:a.txt"], "attribute": manual}),
            json!({"name": "//a:mid", "ruleInput": ["//a:hand"], "attribute": manual}),
            json!({"name": "//a:bin", "ruleInput": ["//a:mid"]}),
        ]);
        let (plan, _) = run(dir.path(), Command::Check, &[&graph]);
        assert_eq!(plan.targets, ["//a:bin"]);
        assert!(plan.format.is_empty(), "{:?}", plan.format);
        assert!(plan.lint.is_empty(), "{:?}", plan.lint);
        let (plan, _) = run(dir.path(), Command::Lint, &[&graph]);
        assert!(plan.empty(), "{:?}", plan.targets);
    }

    #[test]
    fn a_bazelrc_global_or_a_toolchain_widens_lint_and_tests() {
        let dir = repo();
        let root = dir.path();
        write(root, ".bazelrc", "build --extra_toolchains=//a:policy\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "policy"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "--force", "main"]).unwrap();
        write(root, "a/a.txt", "changed\n").unwrap();
        for (user, reason) in [
            (
                json!({"name": "//a:policy", "ruleInput": ["//a:lib"]}),
                "changed files reach //a:policy, which bazelrc flags use",
            ),
            (
                json!({"name": "//a:tc", "ruleClass": "toolchain", "ruleInput": ["//a:lib"]}),
                "changed files reach toolchain //a:tc",
            ),
        ] {
            let graph =
                rules_graph(&[json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}), user]);
            let (plan, _) = run(root, Command::Check, &[&graph]);
            assert_eq!(plan.widened[0].reason, reason);
            assert_eq!(plan.targets, ["//..."]);
            assert_eq!(plan.lint, ["//..."]);
            assert_eq!(plan.tests, ["//..."]);
            let (plan, _) = run(root, Command::Lint, &[&graph]);
            assert_eq!(plan.targets, ["//..."]);
            let (plan, _) = run(root, Command::Test, &[&graph]);
            assert_eq!(plan.targets, ["//..."]);
        }
    }

    #[test]
    fn an_aspect_tool_widens_lint_but_tests_stay_with_the_graph() {
        let dir = repo();
        let root = dir.path();
        write(
            root,
            "lint/aspect.bzl",
            "lint_aspect = aspect(attrs = {\"_tool\": attr.label(default = \"//a:tool\")})\n",
        )
        .unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "aspect"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "--force", "main"]).unwrap();
        write(root, "a/a.txt", "changed\n").unwrap();
        let graph = rules_graph(&[
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}),
            json!({"name": "//a:tool", "ruleInput": ["//a:lib"]}),
            json!({"name": "//a:tool_test", "ruleClass": "sh_test", "ruleInput": ["//a:tool"]}),
            json!({"name": "//b:other", "ruleInput": ["//b:b.txt"]}),
            json!({"name": "//b:other_test", "ruleClass": "sh_test", "ruleInput": ["//b:other"]}),
        ]);
        let aspects = ["//lint:aspect.bzl%lint_aspect".to_owned()];
        let run = |command| {
            run_with_aspects(root, command, LintScope::DirectUsers, &aspects, &[&graph]).0
        };
        let plan = run(Command::Test);
        assert_eq!(plan.targets, ["//a:tool_test"]);
        assert!(plan.widened.is_empty(), "{:?}", plan.widened);
        let plan = run(Command::Build);
        assert_eq!(plan.targets, ["//a:lib", "//a:tool", "//a:tool_test"]);
        let plan = run(Command::Check);
        assert_eq!(plan.tests, ["//a:tool_test"]);
        assert_eq!(plan.lint, ["//..."]);
        assert_eq!(plan.format, ["//..."]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(
            plan.widened,
            [WidenedScope {
                scope: "//...".to_owned(),
                reason: "changed files reach //a:tool, which aspects use".to_owned(),
                repos: Vec::new(),
            }]
        );
        let plan = run(Command::Lint);
        assert_eq!(plan.targets, ["//..."]);
    }

    #[test]
    fn check_widened_to_everything_keeps_generate_and_format_repo_wide() {
        let dir = repo();
        write(dir.path(), "MODULE.bazel", "module(name = \"changed\")\n").unwrap();
        let (plan, _) = run(dir.path(), Command::Check, &[]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(plan.generate, ["//..."]);
        assert_eq!(plan.format, ["//..."]);
        assert_eq!(plan.lint, ["//..."]);
    }

    #[test]
    fn check_json_reports_the_selected_tests() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Check, &[&graph]);
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
            coverage_tests: None,
            generate: Vec::new(),
            format: Vec::new(),
            lint: Vec::new(),
            widened: Vec::new(),
            unscoped: false,
            graph: None,
        };
        assert!(
            plan.summary()
                .ends_with(&format!("since {}", short_base(base)))
        );
        assert!(line.ends_with(&short_base(base)));
    }

    #[test]
    fn manual_rules_are_not_selected() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Build, &[&graph]);
        assert!(!plan.targets.contains(&"//a:m".to_owned()));
    }

    #[test]
    fn security_with_no_owned_targets_still_runs_the_secrets_scan() {
        let dir = repo();
        write(dir.path(), "a/key.txt", "secret\n").unwrap();
        let graph = graph();
        let (plan, _) = run(dir.path(), Command::Security, &[&graph]);
        assert!(plan.targets.is_empty(), "{:?}", plan.targets);
        assert!(!plan.empty());
    }

    #[test]
    fn security_with_no_changes_is_empty() {
        let dir = repo();
        let (plan, _) = run(dir.path(), Command::Security, &[]);
        assert!(plan.changed_files.is_empty(), "{:?}", plan.changed_files);
        assert!(!plan.unscoped);
        assert!(plan.empty());
    }

    #[test]
    fn json_reports_base_files_targets_and_widening() {
        let dir = repo();
        write(dir.path(), "a/a.txt", "changed\n").unwrap();
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

    fn bumped_cargo_lock() -> (tempfile::TempDir, &'static str) {
        let dir = repo();
        let root = dir.path();
        let lock = dx_ecosystems::Ecosystem::Cargo.lockfiles()[0].as_str();
        write(
            root,
            "MODULE.bazel",
            &format!(
                "bazel_dep(name = \"rules_rust\", version = \"1.0\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"//:{lock}\")\n"
            ),
        ).unwrap();
        let rules_rust = "module(name = \"rules_rust\")\n";
        let hash = dx_digest::sha256_hex(rules_rust.as_bytes());
        write(
            root,
            &format!("cache/content_addressable/sha256/{hash}/file"),
            rules_rust,
        )
        .unwrap();
        write(
            root,
            ".bazelrc",
            &format!("common --repository_cache={}/cache\n", root.display()),
        )
        .unwrap();
        write(
            root,
            "MODULE.bazel.lock",
            &json!({"registryFileHashes": {"https://bcr.bazel.build/modules/rules_rust/1.0/MODULE.bazel": hash}}).to_string(),
        )
        .unwrap();
        write(root, lock, &cargo_lock("1.0.1")).unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "crates"]).unwrap();
        dx_test_scratch::git(root, root, &["branch", "--force", "main"]).unwrap();
        write(root, lock, &cargo_lock("1.0.2")).unwrap();
        (dir, lock)
    }

    #[test]
    fn json_reports_the_crates_a_cargo_lock_update_selects() {
        let (dir, lock) = bumped_cargo_lock();
        let root = dir.path();
        let graph = [
            json!({"name": "//a:serde_user", "ruleInput": ["@@rules_rust++crate+crates//:serde"]}),
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}),
        ]
        .iter()
        .map(|rule| json!({"type": "RULE", "rule": rule}).to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let (plan, runner) = run(root, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, [lock]);
        assert_eq!(plan.targets, ["//a:serde_user"]);
        let serde = format!(
            "@{}//:serde",
            dx_families::hub("cargo")
                .expect("setup.hubs entry cargo")
                .hub
        );
        assert_eq!(
            plan.json(Command::Build)["widened"],
            json!([{
                "scope": format!("rdeps(//..., {serde})"),
                "reason": format!("changed crates in {lock}"),
                "repos": [serde],
            }])
        );
        assert!(
            plan.summary().contains(&format!(
                "widened to rdeps(//..., {serde}) (changed crates in {lock})"
            )),
            "{}",
            plan.summary()
        );
        assert_eq!(*runner.expressions.borrow(), ["kind('rule', //...)"]);
    }

    #[test]
    fn check_lints_dependents_when_a_lock_update_changes_external_crates() {
        let (dir, _) = bumped_cargo_lock();
        let root = dir.path();
        write(root, "a/a.txt", "changed\n").unwrap();
        let graph = rules_graph(&[
            json!({"name": "//a:serde_user", "ruleInput": ["@@rules_rust++crate+crates//:serde"]}),
            json!({"name": "//a:lib", "ruleInput": ["//a:a.txt"]}),
        ]);
        let (plan, _) = run(root, Command::Check, &[&graph]);
        assert_eq!(plan.targets, ["//a:lib", "//a:serde_user"]);
        assert_eq!(plan.format, ["//a:lib"]);
        assert_eq!(plan.lint, ["//a:lib", "//a:serde_user"]);
    }

    #[test]
    fn scopes_narrow_the_affected_plan() {
        let labels = |items: &[&str]| -> Vec<String> {
            items.iter().map(|item| (*item).to_owned()).collect()
        };
        let mut plan = AffectedPlan {
            base: "b".to_owned(),
            changed_files: labels(&["svc/a.rs", "web/b.ts"]),
            targets: labels(&["//svc/api:lib", "//svc:bin", "//web:app"]),
            tests: labels(&["//svc/api:test", "//web:test"]),
            coverage_tests: None,
            generate: labels(&["//svc/api:all", "//web:all", "//..."]),
            format: labels(&["//svc/api:lib", "//web:app"]),
            lint: labels(&["//svc/api:lib", "//svc/api:test", "//web:app"]),
            widened: Vec::new(),
            unscoped: false,
            graph: None,
        };
        narrow_to_scope(&mut plan, &labels(&["//svc/api/...", "//svc:bin"]));
        assert_eq!(plan.targets, labels(&["//svc/api:lib", "//svc:bin"]));
        assert_eq!(plan.tests, labels(&["//svc/api:test"]));
        assert_eq!(plan.format, labels(&["//svc/api:lib"]));
        assert_eq!(plan.lint, labels(&["//svc/api:lib", "//svc/api:test"]));
        assert_eq!(plan.generate, labels(&["//svc:all", "//svc/api/..."]));
        narrow_to_scope(&mut plan, &labels(&["//svc:bin"]));
        assert_eq!(plan.targets, labels(&["//svc:bin"]));
        assert!(plan.format.is_empty(), "{:?}", plan.format);
        assert!(plan.lint.is_empty(), "{:?}", plan.lint);
        assert!(!plan.empty());
        narrow_to_scope(&mut plan, &labels(&["//web/..."]));
        assert!(plan.empty());
    }

    #[test]
    fn exit_after_finish_keeps_the_code_or_reports_the_stdout_failure() {
        assert_eq!(exit_after_finish(0, Ok(())), 0);
        let broken = || Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        assert_eq!(exit_after_finish(0, broken()), dx_process::EXIT_BROKEN_PIPE);
        assert_eq!(exit_after_finish(3, broken()), 3);
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

    fn nested_repo(module: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        dx_test_scratch::git(root, root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        write(root, "sub/MODULE.bazel", module).unwrap();
        write(root, "sub/a/BUILD.bazel", "").unwrap();
        write(root, "sub/a/a.txt", "a\n").unwrap();
        write(root, "frontend/x.ts", "x\n").unwrap();
        write(root, "lib/MODULE.bazel", "module(name = \"lib\")\n").unwrap();
        write(root, "lib/l.txt", "l\n").unwrap();
        dx_test_scratch::git(root, root, &["add", "."]).unwrap();
        dx_test_scratch::git(root, root, &["commit", "--quiet", "-m", "base"]).unwrap();
        dx_test_scratch::git(root, root, &["checkout", "--quiet", "-b", "topic"]).unwrap();
        let workspace = root.join("sub");
        (dir, workspace)
    }

    const OVERRIDE: &str = "module(name = \"x\")\nbazel_dep(name = \"lib\", version = \"0\")\nlocal_path_override(module_name = \"lib\", path = \"../lib\")\n";

    #[test]
    fn workspace_below_the_git_root_maps_changes_without_widening() {
        let (dir, workspace) = nested_repo("module(name = \"x\")\n");
        write(dir.path(), "sub/a/a.txt", "changed\n").unwrap();
        let graph = graph();
        let (plan, _) = run(&workspace, Command::Build, &[&graph]);
        assert_eq!(plan.changed_files, ["a/a.txt"]);
        assert!(plan.widened.is_empty(), "{:?}", plan.widened);
    }

    #[test]
    fn changes_outside_a_nested_workspace_select_nothing() {
        let (dir, workspace) = nested_repo("module(name = \"x\")\n");
        write(dir.path(), "frontend/x.ts", "changed\n").unwrap();
        write(dir.path(), "frontend/new.ts", "new\n").unwrap();
        let (plan, runner) = run(&workspace, Command::Test, &[]);
        assert!(plan.empty());
        assert!(runner.expressions.borrow().is_empty());
    }

    #[test]
    fn local_path_override_change_outside_the_workspace_widens() {
        let (dir, workspace) = nested_repo(OVERRIDE);
        write(dir.path(), "lib/l.txt", "changed\n").unwrap();
        let (plan, _) = run(&workspace, Command::Test, &[]);
        assert_eq!(plan.targets, ["//..."]);
        assert_eq!(plan.widened[0].reason, "local_path_override ../lib/l.txt");
    }
}
