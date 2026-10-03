use std::io::Write;
use std::time::Instant;

use crate::args::Invocation;
use crate::exec::common::check_stdout_write;

use crate::resolve::{QueryResult, QueryRunner};

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_HOOKS_FAILED: &str = "hooks_failed";

pub(crate) fn execute_hooks(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    runner: &dyn dx_process::Runner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let verb = invocation.targets.first().map(String::as_str).unwrap_or("");
    match verb {
        "install" => {
            if invocation.dry_run {
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install .git/hooks/pre-commit"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install .git/hooks/pre-push"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would install dx.local.toml"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            match dx_adopt::install_hooks(workspace) {
                Ok(installed) => {
                    if !summaries_suppressed(invocation) {
                        for path in installed {
                            if let Err(exit) = check_stdout_write(writeln!(out, "installed {path}"))
                            {
                                return exit;
                            }
                        }
                    }
                    0
                }
                Err(error) => {
                    operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string())
                }
            }
        }
        "uninstall" => {
            if invocation.dry_run {
                if !summaries_suppressed(invocation) {
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would remove .git/hooks/pre-commit"))
                    {
                        return exit;
                    }
                    if let Err(exit) =
                        check_stdout_write(writeln!(out, "would remove .git/hooks/pre-push"))
                    {
                        return exit;
                    }
                }
                return 0;
            }
            match dx_adopt::uninstall_hooks(workspace) {
                Ok(removed) => {
                    if !summaries_suppressed(invocation) {
                        for path in removed {
                            if let Err(exit) = check_stdout_write(writeln!(out, "removed {path}")) {
                                return exit;
                            }
                        }
                    }
                    0
                }
                Err(error) => {
                    operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string())
                }
            }
        }
        "status" => execute_status(invocation, workspace, out, err),
        "run" => execute_run(invocation, workspace, query_runner, runner, out, err),
        _ => pre_exec(
            err,
            &format!("usage: dx hooks <{}>", dx_adopt::hook_verb_pipe()),
        ),
    }
}

fn read_file_opt(root: &std::path::Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn execute_status(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "would show hooks status")) {
                return exit;
            }
        }
        return 0;
    }
    let baseline_opt = read_file_opt(workspace, dx_adopt::HOOK_BASELINE_REL);
    let overlay_opt = read_file_opt(workspace, dx_adopt::HOOK_OVERLAY_REL);
    let timings_opt = read_file_opt(workspace, dx_adopt::HOOK_TIMINGS_REL);
    let baseline_res = dx_adopt::load_hooks_config(baseline_opt.as_deref(), None);
    let overlay_res = dx_adopt::load_hooks_config(None, overlay_opt.as_deref());
    let timings_res = dx_adopt::load_hook_timings(timings_opt.as_deref());
    let shows_baseline = baseline_res.is_ok();
    let shows_overlay = overlay_res.is_ok();
    let shows_timings = timings_res.is_ok();
    if !dx_adopt::hook_status_shows_merged(shows_baseline, shows_overlay, shows_timings) {
        let mut detail = String::new();
        if let Err(error) = &baseline_res {
            detail.push_str(&error.to_string());
            detail.push_str("; ");
        }
        if let Err(error) = &overlay_res {
            detail.push_str(&error.to_string());
            detail.push_str("; ");
        }
        if let Err(error) = &timings_res {
            detail.push_str(&error.to_string());
        }
        let detail = detail.trim_end_matches("; ");
        return operational(
            invocation,
            out,
            err,
            CODE_HOOKS_FAILED,
            &format!("hooks status missing merged layer: {detail}"),
        );
    }
    let merged = match dx_adopt::load_hooks_config(baseline_opt.as_deref(), overlay_opt.as_deref())
    {
        Ok(config) => config,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let timings = match timings_res {
        Ok(timings) => timings,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let baseline_src = if baseline_opt.is_some() {
        dx_adopt::HOOK_BASELINE_REL.to_owned()
    } else {
        "defaults (no dx.hooks.toml)".to_owned()
    };
    let overlay_src = if overlay_opt.is_some() {
        dx_adopt::HOOK_OVERLAY_REL.to_owned()
    } else {
        "absent (no dx.local.toml)".to_owned()
    };
    let view = dx_adopt::render_hooks_status_merged(&merged, &timings, &baseline_src, &overlay_src);
    if let Err(exit) = check_stdout_write(write!(out, "{view}")) {
        return exit;
    }
    0
}

fn execute_run(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
    runner: &dyn dx_process::Runner,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let trigger = invocation.targets.get(1).map(String::as_str).unwrap_or("");
    if !dx_adopt::is_hook_trigger(trigger) {
        return pre_exec(
            err,
            &format!("usage: dx hooks run <{}>", dx_adopt::hook_trigger_pipe()),
        );
    }
    if invocation.dry_run {
        if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "would run {trigger}")) {
                return exit;
            }
        }
        return 0;
    }
    let git_path = runner.git_tool();
    let uses_hermetic = git_path
        .as_ref()
        .is_some_and(|path| dx_adopt::hook_git_path_is_hermetic(path));
    if !dx_adopt::hook_git_is_hermetic(uses_hermetic, false) {
        return operational(
            invocation,
            out,
            err,
            CODE_HOOKS_FAILED,
            &format!(
                "hook git must be hermetic: set {} to an absolute managed Git path; ambient PATH lookup is rejected",
                dx_adopt::HOOK_GIT_ENV_VAR
            ),
        );
    }
    let git = git_path.unwrap_or_else(|| std::path::PathBuf::from("git"));
    let baseline_opt = read_file_opt(workspace, dx_adopt::HOOK_BASELINE_REL);
    let overlay_opt = read_file_opt(workspace, dx_adopt::HOOK_OVERLAY_REL);
    let config = match dx_adopt::load_hooks_config(baseline_opt.as_deref(), overlay_opt.as_deref())
    {
        Ok(config) => config,
        Err(error) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &error.to_string());
        }
    };
    let checks = dx_adopt::checks_for_trigger(&config, trigger);
    if checks.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "ran {trigger}: ok (no checks configured)"))
            {
                return exit;
            }
        }
        return 0;
    }
    let staged = match staged_files(&git, workspace, query_runner) {
        Ok(files) => files,
        Err(detail) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
        }
    };
    if staged.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "ran {trigger}: ok (no staged files)"))
            {
                return exit;
            }
        }
        return 0;
    }
    let targets = match affected_targets(&staged, workspace, query_runner) {
        Ok(targets) => targets,
        Err(detail) => {
            return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
        }
    };
    if targets.is_empty() {
        if !summaries_suppressed(invocation) {
            if let Err(exit) =
                check_stdout_write(writeln!(out, "ran {trigger}: ok (no affected targets)"))
            {
                return exit;
            }
        }
        return 0;
    }
    let dx_exe = match std::env::current_exe() {
        Ok(exe) => exe.to_string_lossy().into_owned(),
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_HOOKS_FAILED,
                &format!("hook dx executable unavailable: {error}"),
            );
        }
    };
    let mut measured: Vec<(String, f64)> = Vec::with_capacity(checks.len());
    for check in &checks {
        let argv = check_argv(&dx_exe, check, &targets);
        let start = Instant::now();
        let status = match runner.run(&argv, workspace, &[]) {
            Ok(status) => status,
            Err(error) => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} launch failed: {error}"),
                );
            }
        };
        let elapsed = start.elapsed().as_secs_f64();
        if dx_adopt::hook_check_timed_out(elapsed, config.budget_secs) {
            return operational(
                invocation,
                out,
                err,
                CODE_HOOKS_FAILED,
                &format!(
                    "hook check {check:?} exceeded budget (took {elapsed:.2}s, budget {}s)",
                    config.budget_secs
                ),
            );
        }
        match status.code {
            Some(0) => {
                measured.push((check.clone(), elapsed));
            }
            Some(code) => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} failed with exit {code}"),
                );
            }
            None => {
                return operational(
                    invocation,
                    out,
                    err,
                    CODE_HOOKS_FAILED,
                    &format!("hook check {check:?} terminated by signal"),
                );
            }
        }
    }
    if let Err(detail) = record_timings(workspace, &measured) {
        return operational(invocation, out, err, CODE_HOOKS_FAILED, &detail);
    }
    if !summaries_suppressed(invocation) {
        for (check, elapsed) in &measured {
            let _ = writeln!(
                out,
                "ran {trigger}: {check} ok ({elapsed:.2}s / budget {}s)",
                config.budget_secs
            );
        }
    }
    0
}

fn staged_files(
    git: &std::path::Path,
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
) -> Result<Vec<String>, String> {
    let argv = vec![
        git.to_string_lossy().into_owned(),
        "diff".to_owned(),
        "--cached".to_owned(),
        "--name-only".to_owned(),
    ];
    let result: QueryResult = query_runner
        .run_query(&argv, workspace)
        .map_err(|error| format!("hook git diff failed: {error}"))?;
    if result.code != Some(0) {
        let detail = first_line(&result.stderr);
        return Err(format!("hook git diff failed: {detail}"));
    }
    let text = String::from_utf8(result.stdout)
        .map_err(|error| format!("hook git diff output is not UTF-8: {error}"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn affected_targets(
    staged: &[String],
    workspace: &std::path::Path,
    query_runner: &dyn QueryRunner,
) -> Result<Vec<String>, String> {
    let existing: Vec<String> = staged
        .iter()
        .filter(|path| workspace.join(path).is_file())
        .cloned()
        .collect();
    if existing.is_empty() {
        return Ok(Vec::new());
    }
    crate::resolve::resolve(&existing, workspace, query_runner)
        .map(|resolved| resolved.targets)
        .map_err(|error| error.to_string())
}

fn check_argv(dx_exe: &str, check: &str, targets: &[String]) -> Vec<String> {
    let mut argv = vec![dx_exe.to_owned()];
    argv.extend(check.split_whitespace().map(ToOwned::to_owned));
    argv.extend(targets.iter().cloned());
    argv
}

fn record_timings(workspace: &std::path::Path, measured: &[(String, f64)]) -> Result<(), String> {
    let path = workspace.join(dx_adopt::HOOK_TIMINGS_REL);
    let existing = std::fs::read_to_string(&path).ok();
    let mut timings =
        dx_adopt::load_hook_timings(existing.as_deref()).map_err(|error| error.to_string())?;
    for (check, elapsed) in measured {
        timings.secs_by_check.insert(check.clone(), *elapsed);
    }
    let body = dx_adopt::render_hook_timings(&timings).map_err(|error| error.to_string())?;
    dx_atomic_fs::write_atomic(&path, body.as_bytes())
        .map_err(|error| format!("write timings: {error}"))?;
    Ok(())
}

fn first_line(bytes: &[u8]) -> String {
    const LIMIT: usize = 200;
    let text = String::from_utf8_lossy(bytes);
    match text.lines().next().map(str::trim) {
        Some(line) if !line.is_empty() => dx_output::truncate_line(line, LIMIT),
        _ => "no Git diagnostic".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::execute_adoption;
    use crate::adopt::test_support::{
        env, env_with, env_with_query, invocation, NullQuery, NullRunner,
    };
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

    #[test]
    fn hooks_install_uninstall_and_collisions_are_reported() {
        let scratch = dx_test_scratch::scratch("hooks-install-cycle-");
        let root = scratch.path();
        std::fs::create_dir(root.join(".git")).expect("git");
        for verb in ["install", "uninstall"] {
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &invocation(&["hooks", verb]),
                    root,
                    &NullQuery,
                    &NullRunner,
                    &mut out,
                    &mut err
                ),
                0
            );
            assert!(String::from_utf8(out)
                .expect("out")
                .contains(if verb == "install" {
                    "installed"
                } else {
                    "removed"
                }));
            assert!(err.is_empty());
        }
        let foreign = dx_test_scratch::scratch("hooks-install-collision-");
        std::fs::create_dir_all(foreign.path().join(".git/hooks")).expect("hooks");
        std::fs::write(foreign.path().join(".git/hooks/pre-commit"), "foreign hook")
            .expect("foreign hook");
        for verb in ["install", "uninstall"] {
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &invocation(&["hooks", verb]),
                    foreign.path(),
                    &NullQuery,
                    &NullRunner,
                    &mut Vec::new(),
                    &mut err
                ),
                1
            );
            assert!(!err.is_empty());
        }
    }

    #[test]
    fn unknown_verb_and_trigger_usage_name_every_verb_and_trigger() {
        let scratch = dx_test_scratch::scratch("hooks-usage-");
        let cases = [
            (
                vec!["hooks", "frobnicate"],
                format!("usage: dx hooks <{}>", dx_adopt::hook_verb_pipe()),
            ),
            (
                vec!["hooks", "run", "pre-rebase"],
                format!("usage: dx hooks run <{}>", dx_adopt::hook_trigger_pipe()),
            ),
        ];
        for (words, expected) in cases {
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &invocation(&words),
                    scratch.path(),
                    &NullQuery,
                    &NullRunner,
                    &mut out,
                    &mut err
                ),
                2
            );
            assert!(out.is_empty());
            assert!(
                String::from_utf8(err)
                    .expect("err")
                    .contains(&format!("dx: {expected}")),
                "dx {} must print `{expected}`",
                words.join(" ")
            );
        }
    }

    #[test]
    fn hook_process_launch_failures_stop_before_recording_timings() {
        struct MissingProcess;
        impl QueryRunner for MissingProcess {
            fn run_query(&self, _: &[String], _: &Path) -> io::Result<QueryResult> {
                Err(io::Error::other("missing executable"))
            }
        }
        impl dx_process::Runner for MissingProcess {
            fn git_tool(&self) -> Option<PathBuf> {
                Some(PathBuf::from("/hermetic/git"))
            }
            fn run(
                &self,
                _: &[String],
                _: &Path,
                _: &[(&str, &str)],
            ) -> io::Result<dx_process::ChildStatus> {
                Err(io::Error::other("missing executable"))
            }
        }
        let scratch = dx_test_scratch::scratch("hooks-spawn-failure-");
        write_workspace(scratch.path());
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        for query in [
            &MissingProcess as &dyn QueryRunner,
            &ScriptQuery::staged_then_owners("pkg/a.py\n", "//pkg:lib\n"),
        ] {
            let mut err = Vec::new();
            assert_eq!(
                execute_hooks(
                    &inv,
                    scratch.path(),
                    query,
                    &MissingProcess,
                    &mut Vec::new(),
                    &mut err
                ),
                1
            );
            assert!(String::from_utf8(err)
                .expect("err")
                .contains("missing executable"));
            assert!(!scratch.path().join(dx_adopt::HOOK_TIMINGS_REL).exists());
        }
        assert_eq!(first_line(b"\nignored"), "no Git diagnostic");
        assert_eq!(
            first_line("x".repeat(201).as_bytes()),
            format!("{}...", "x".repeat(200))
        );
    }

    #[test]
    fn a_wide_character_on_the_truncation_edge_is_not_split() {
        let mut stderr = "x".repeat(198);
        stderr.push('€');
        stderr.push_str("git: fatal: bad object HEAD");
        let got = first_line(stderr.as_bytes());
        assert_eq!(got, format!("{}...", "x".repeat(198)));
        assert!(got.len() <= 203, "bounded: {}", got.len());
        assert_eq!(first_line(b"  \nreal error"), "no Git diagnostic");
    }

    #[test]
    fn hooks_status_shows_merged_layers() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("baseline:"));
        assert!(text.contains("overlay:"));
        assert!(text.contains("timings:"));
        assert!(text.contains("effective:"));
        assert!(!text.contains("p95"));
    }

    #[test]
    fn hooks_status_rejects_invalid_toml() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-bad-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.hooks.toml"), "not toml = [").expect("bad baseline");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 1);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("hooks status"));
    }

    #[test]
    fn hooks_status_shows_measured_timings() {
        let inv = invocation(&["hooks", "status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-status-timed-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(
            root.join(".dx/hooks-timings.toml"),
            "[timings]\n\"format --check\" = 1.23\n",
        )
        .expect("timings");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("1.23s (measured)"));
        assert!(!text.contains("p95 12s"));
    }

    #[test]
    fn hooks_dry_run_plans_without_mutating() {
        for (words, want) in [
            (vec!["hooks", "install", "--dry-run"], "would install"),
            (vec!["hooks", "uninstall", "--dry-run"], "would remove"),
            (vec!["hooks", "status", "--dry-run"], "would show"),
            (
                vec!["hooks", "run", "pre-commit", "--dry-run"],
                "would run pre-commit",
            ),
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-hooks-dry-");
            let root = scratch.path().to_path_buf();
            let mut out = Vec::new();
            let mut err = Vec::new();
            let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
            assert_eq!(code, 0, "words: {words:?}");
            assert!(
                String::from_utf8(out).expect("out").contains(want),
                "words: {words:?}"
            );
            assert!(!root.join(".git/hooks/pre-commit").exists());
        }
        let inv = invocation(&["hooks", "status", "--dry-run", "--quiet"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-dry-quiet-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env(&root, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out).expect("out").is_empty());
    }

    struct ScriptQuery {
        outputs: RefCell<Vec<crate::resolve::QueryResult>>,
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptQuery {
        fn staged_then_owners(staged: &str, owners: &str) -> Self {
            Self {
                outputs: RefCell::new(vec![
                    crate::resolve::QueryResult {
                        code: Some(0),
                        stdout: staged.as_bytes().to_vec(),
                        stderr: Vec::new(),
                    },
                    crate::resolve::QueryResult {
                        code: Some(0),
                        stdout: owners.as_bytes().to_vec(),
                        stderr: Vec::new(),
                    },
                ]),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl QueryRunner for ScriptQuery {
        fn run_query(&self, argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            self.seen.borrow_mut().push(argv.to_vec());
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    struct ScriptRunner {
        git: Option<PathBuf>,
        codes: RefCell<Vec<Option<i32>>>,
        seen: RefCell<Vec<Vec<String>>>,
    }

    impl ScriptRunner {
        fn git_with_codes(git: &str, codes: Vec<Option<i32>>) -> Self {
            Self {
                git: Some(PathBuf::from(git)),
                codes: RefCell::new(codes),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl dx_process::Runner for ScriptRunner {
        fn git_tool(&self) -> Option<PathBuf> {
            self.git.clone()
        }

        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.seen.borrow_mut().push(argv.to_vec());
            Ok(dx_process::ChildStatus {
                code: self.codes.borrow_mut().remove(0),
            })
        }
    }

    fn write_workspace(root: &Path) {
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("build");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source");
    }

    #[test]
    fn hooks_run_handles_empty_selection_signal_and_invalid_state() {
        for (scenario, want_code, want_detail) in [
            ("no-checks", 0, "no checks configured"),
            ("deleted-file", 0, "no affected targets"),
            ("bad-config", 1, ""),
            ("signal", 1, "terminated by signal"),
            ("bad-timings", 1, ""),
            ("timings-collision", 1, "write timings"),
            ("query-failed", 1, "hook git diff failed"),
            ("query-utf8", 1, "not UTF-8"),
            ("owner-failed", 1, "query"),
        ] {
            let scratch = dx_test_scratch::scratch("hooks-failure-");
            let root = scratch.path();
            write_workspace(root);
            let query = ScriptQuery::staged_then_owners("pkg/a.py\n", "//pkg:lib\n");
            let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
            match scenario {
                "no-checks" => {
                    std::fs::write(root.join("dx.hooks.toml"), "[hooks]\npre_commit = []\n")
                        .expect("config")
                }
                "deleted-file" => {
                    std::fs::remove_file(root.join("pkg/a.py")).expect("delete source")
                }
                "bad-config" => {
                    std::fs::write(root.join("dx.hooks.toml"), "[broken").expect("config")
                }
                "signal" => runner.codes.borrow_mut()[0] = None,
                "bad-timings" => {
                    std::fs::create_dir(root.join(".dx")).expect("dx");
                    std::fs::write(root.join(dx_adopt::HOOK_TIMINGS_REL), "[broken")
                        .expect("timings");
                }
                "timings-collision" => {
                    std::fs::create_dir_all(root.join(dx_adopt::HOOK_TIMINGS_REL))
                        .expect("collision")
                }
                "query-failed" => query.outputs.borrow_mut()[0].code = Some(1),
                "query-utf8" => query.outputs.borrow_mut()[0].stdout = vec![0xff],
                "owner-failed" => query.outputs.borrow_mut()[1].code = Some(1),
                _ => unreachable!(),
            }
            let mut out = Vec::new();
            let mut err = Vec::new();
            let inv = invocation(&["hooks", "run", "pre-commit"]);
            assert_eq!(
                execute_hooks(&inv, root, &query, &runner, &mut out, &mut err),
                want_code,
                "{scenario}"
            );
            let detail = format!(
                "{}{}",
                String::from_utf8(out).expect("out"),
                String::from_utf8(err).expect("err")
            );
            assert!(detail.contains(want_detail), "{scenario}: {detail}");
            if want_code == 1 {
                assert!(!detail.is_empty());
            }
            if scenario == "signal" {
                assert_eq!(runner.seen.borrow().len(), 1);
            }
        }
    }

    #[test]
    fn hooks_status_rejects_invalid_overlay_and_timings() {
        for rel in [dx_adopt::HOOK_OVERLAY_REL, dx_adopt::HOOK_TIMINGS_REL] {
            let scratch = dx_test_scratch::scratch("hooks-status-invalid-");
            std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
            std::fs::write(scratch.path().join(rel), "[broken").expect("invalid layer");
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                execute_status(
                    &invocation(&["hooks", "status"]),
                    scratch.path(),
                    &mut out,
                    &mut err
                ),
                1
            );
            assert!(out.is_empty());
            assert!(String::from_utf8(err)
                .expect("err")
                .contains("missing merged layer"));
        }
    }

    #[test]
    fn hooks_run_executes_checks_and_records_measured_timings() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("pkg/a.py\n", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env_with(&root, &query, &runner, &mut out, &mut err));
        assert_eq!(code, 0);
        assert_eq!(runner.seen.borrow().len(), 2);
        assert!(runner.seen.borrow()[0][1..].contains(&"format".to_owned()));
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("format --check ok"));
        assert!(!text.contains("budget 120s)") || text.contains("/ budget 120s)"));
        assert!(!text.contains("ran pre-commit: ok (budget 120s)"));
        let timings =
            std::fs::read_to_string(root.join(".dx/hooks-timings.toml")).expect("timings");
        assert!(timings.contains("format --check"));
        assert!(!timings.contains("p95"));
        assert_eq!(query.seen.borrow().len(), 2);
        assert!(
            query.seen.borrow()[0][0].ends_with("git")
                || query.seen.borrow()[0][0] == "/hermetic/git"
        );
    }

    #[test]
    fn hooks_run_blocks_on_check_failure() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-fail-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery::staged_then_owners("pkg/a.py\n", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(1), Some(0)]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env_with(&root, &query, &runner, &mut out, &mut err));
        assert_eq!(code, 1);
        assert!(String::from_utf8(err).expect("err").contains("failed"));
    }

    #[test]
    fn hooks_run_blocks_on_budget_timeout() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-timeout-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        std::fs::write(root.join("dx.hooks.toml"), "[hooks]\nbudget_secs = 0\n")
            .expect("zero budget");
        let query = ScriptQuery::staged_then_owners("pkg/a.py\n", "//pkg:lib\n");
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![Some(0), Some(0)]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env_with(&root, &query, &runner, &mut out, &mut err));
        assert_eq!(code, 1);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("exceeded budget"));
    }

    #[test]
    fn hooks_run_needs_hermetic_git() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-nogit-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let runner = ScriptRunner {
            git: None,
            codes: RefCell::new(vec![]),
            seen: RefCell::new(Vec::new()),
        };
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env_with_query(&root, &NullQuery, &mut out, &mut err));
        assert_eq!(code, 1);
        assert!(String::from_utf8(err).expect("err").contains("hermetic"));
        assert!(runner.seen.borrow().is_empty());
    }

    #[test]
    fn hooks_run_passes_with_no_staged_files() {
        let inv = invocation(&["hooks", "run", "pre-commit"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-hooks-run-empty-");
        let root = scratch.path().to_path_buf();
        write_workspace(&root);
        let query = ScriptQuery {
            outputs: RefCell::new(vec![crate::resolve::QueryResult {
                code: Some(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            }]),
            seen: RefCell::new(Vec::new()),
        };
        let runner = ScriptRunner::git_with_codes("/hermetic/git", vec![]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(&inv, env_with(&root, &query, &runner, &mut out, &mut err));
        assert_eq!(code, 0);
        assert!(String::from_utf8(out)
            .expect("out")
            .contains("no staged files"));
        assert!(runner.seen.borrow().is_empty());
    }
}
