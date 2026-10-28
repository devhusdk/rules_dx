mod add;
mod affected;
mod audit;
mod build_failures;
mod bump;
mod changed_coverage;
mod clean;
pub mod common;
mod coverage_candidate;
#[cfg(test)]
mod current_dir_guard_tests;
#[cfg(test)]
mod delivery_sink;
mod dep_edit;
mod deploy;
mod docs;
#[cfg(test)]
mod docs_parity_tests;
mod explain;
mod explain_store;
mod failure;
mod failure_reports;
pub mod first_run;
mod generate;
mod guidance;
#[cfg(test)]
mod help_launcher_tests;
mod install;
mod managed;
mod managed_codegen;
mod managed_env;
mod managed_prepare;
mod managed_staging;
mod migrate;
mod notice_codes;
#[cfg(test)]
mod package_identity;
mod pinned_tool;
mod quality;
mod quality_apply;
mod quality_baseline;
mod quality_emit;
mod quality_patch;
mod quality_reconcile;
mod quality_reports;
mod results;
mod run;
mod rustc_stderr;
#[cfg(test)]
mod startup_defaults_launcher_tests;
mod test_filter;
mod test_inventory;
mod test_reports;
#[cfg(test)]
pub(crate) mod test_support;
mod umbrella;
mod update;
mod workflow;
mod worktree;

use dx_plan::args::{Command, Invocation};
#[cfg(test)]
pub(crate) use umbrella::UMBRELLA_PHASES;

pub use audit::AUDIT_TODAY_ENV;
pub use bump::COMMIT_JSON_ENV as BUMP_COMMIT_JSON_ENV;
pub use common::{print_actions, ActionTally, Env};
pub use failure::{cancelled, set_usage_command};
pub(crate) use managed_env::{ENV_ARTIFACTS_DIR, ENV_VALUES_FILE};
pub use worktree::{hook_kind, run_worktree_hook};

/// Runs one invocation; a `dx run` app is stopped once `withdraw` holds.
pub(crate) fn execute_until(
    invocation: &Invocation,
    env: Env<'_>,
    withdraw: &dyn Fn() -> bool,
) -> i32 {
    if invocation.command == Command::Run && !invocation.here {
        return run::execute_run_until(invocation, env, withdraw);
    }
    execute(invocation, env)
}

/// The refusal for a workspace that is a slot leased to another session.
fn lease_refusal(invocation: &Invocation, workspace: &std::path::Path) -> Option<String> {
    let who = dx_worktree::Who::from_env(&|name| {
        std::env::var(name).ok().filter(|value| !value.is_empty())
    });
    lease_refusal_for(invocation.command, workspace, &who)
}

fn slot_exempt(command: Command) -> bool {
    matches!(
        command,
        Command::Worktree
            | Command::Version
            | Command::Completion
            | Command::Explain
            | Command::Install
    )
}

/// Holds the busy lock of the slot `workspace` is in while a slot command runs.
fn hold_slot(
    command: Command,
    workspace: &std::path::Path,
    wait: std::time::Duration,
) -> Option<Result<dx_atomic_fs::lease::SharedLease, String>> {
    if slot_exempt(command) {
        return None;
    }
    dx_worktree::hold_busy(workspace, wait)
}

fn lease_refusal_for(
    command: Command,
    workspace: &std::path::Path,
    who: &dx_worktree::Who,
) -> Option<String> {
    if slot_exempt(command) {
        return None;
    }
    let (path, record) = dx_worktree::lease_for_workspace(workspace)?;
    if !record.refuses(who) {
        return None;
    }
    Some(format!(
        "{} is leased to {}; run dx worktree add <branch> for your own slot",
        path.display(),
        record.holder(true).unwrap_or_default()
    ))
}

/// Runs one command with stdout latched on its first write failure.
pub fn execute(invocation: &Invocation, env: Env<'_>) -> i32 {
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
    let mut latched = common::LatchedOut::new(out);
    let code = dispatch(
        invocation,
        Env {
            workspace,
            runner,
            query_runner,
            temp_dir,
            pid,
            nonce,
            out: &mut latched,
            err,
            ci,
            audit_today,
            actions,
            proc_root,
            bump_commit_json,
        },
    );
    if code == 0 {
        latched.exit_code(code)
    } else {
        code
    }
}

fn dispatch(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.output != dx_output::OutputMode::Diff || invocation.command.supports_diff(),
        "{} rejects --output=diff",
        invocation.command.name()
    );
    if invocation.here {
        let Env { out, err, .. } = env;
        return common::pre_exec(
            invocation.output,
            out,
            err,
            "option \"--here\" needs cwd resolution",
        );
    }
    let busy = match hold_slot(
        invocation.command,
        env.workspace,
        dx_worktree::pool::POOL_LOCK_WAIT,
    ) {
        Some(Err(refusal)) => {
            let Env { out, err, .. } = env;
            return common::refusal(invocation.output, out, err, &refusal);
        }
        held => held
            .and_then(Result::ok)
            .map(dx_atomic_fs::lease::SharedLease::into_file),
    };
    if let Some(refusal) = lease_refusal(invocation, env.workspace) {
        let Env { out, err, .. } = env;
        return common::refusal(invocation.output, out, err, &refusal);
    }
    // The busy handle rides only on app children (`run_app`); builds get none.
    let holding = busy
        .as_ref()
        .map(|file| dx_process::Holding::new(env.runner, vec![file]));
    match &holding {
        Some(holding) => dispatch_command(
            invocation,
            Env {
                workspace: env.workspace,
                runner: holding,
                query_runner: env.query_runner,
                temp_dir: env.temp_dir,
                pid: env.pid,
                nonce: env.nonce,
                out: &mut *env.out,
                err: &mut *env.err,
                ci: env.ci,
                audit_today: env.audit_today,
                actions: env.actions,
                proc_root: env.proc_root,
                bump_commit_json: env.bump_commit_json,
            },
        ),
        None => dispatch_command(invocation, env),
    }
}

fn dispatch_command(invocation: &Invocation, env: Env<'_>) -> i32 {
    if invocation.affected && invocation.command != Command::Watch {
        return affected::execute_affected(invocation, env);
    }
    match invocation.command {
        Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Rollback
        | Command::Status
        | Command::Version
        | Command::Completion => {
            let Env {
                workspace,
                out,
                err,
                ..
            } = env;
            crate::adopt::execute_adoption(
                invocation,
                crate::adopt::AdoptEnv {
                    workspace,
                    out,
                    err,
                },
            )
        }
        Command::Watch => crate::adopt::watch::execute_watch(invocation, env),
        Command::Check | Command::Fix => umbrella::execute_umbrella(invocation, env),
        Command::ListTests => test_inventory::execute_tests(invocation, env),
        Command::Build | Command::Test | Command::Coverage | Command::Run | Command::Deploy => {
            workflow::execute_workflow(invocation, env)
        }
        Command::Generate => generate::execute_generate(invocation, env),
        Command::Clean => clean::execute_clean(invocation, env),
        Command::Codegen | Command::Env => managed::execute_managed(invocation, env),
        Command::Security | Command::License => audit::execute_audit(invocation, env),
        Command::Update => update::execute_update(invocation, env),
        Command::Bump => bump::execute_bump(invocation, env),
        Command::Add => add::execute_add(invocation, env),
        Command::Migrate => migrate::execute_migrate(invocation, env),
        Command::Docs => docs::execute_docs(invocation, env),
        Command::Lint | Command::Typecheck | Command::Format => {
            quality::execute_quality(invocation, env)
        }
        Command::Worktree => worktree::execute_worktree(invocation, env),
        Command::Explain => explain::execute_explain(invocation, env),
        Command::Install => install::execute_install(invocation, env),
    }
}

#[cfg(test)]
mod tests {
    use super::delivery_sink::{Fault, Sink};
    use super::test_support::{invocation, Harness};
    use super::{hold_slot, lease_refusal_for};
    use dx_plan::args::Command;
    use std::io::ErrorKind;
    use std::time::Duration;

    #[test]
    fn execute_latches_stdout_for_every_command() {
        for words in [&["clean", "--output=json"][..], &["clean"][..]] {
            let harness = Harness::new("latch-every-command");
            let inv = invocation(words);
            let runner = harness.runner();
            let mut sink = Sink::new(Fault::FlushFails(ErrorKind::BrokenPipe), 0);
            let code = harness.run_streams(&inv, &runner, &mut sink, &mut Vec::new());
            assert_eq!(code, 141, "{words:?}");
            let mut sink = Sink::new(Fault::None, 0);
            let code = harness.run_streams(&inv, &runner, &mut sink, &mut Vec::new());
            assert_eq!(code, 0, "{words:?}");
        }
    }

    #[test]
    fn execute_passes_nonzero_codes_through_a_failed_sink() {
        let harness = Harness::new("latch-nonzero");
        let mut inv = invocation(&["version"]);
        inv.here = true;
        let runner = harness.runner();
        let mut sink = Sink::new(Fault::FlushFails(ErrorKind::BrokenPipe), 0);
        let code = harness.run_streams(&inv, &runner, &mut sink, &mut Vec::new());
        assert_ne!(code, 0);
        assert_ne!(code, 141);
    }

    const QUIET_ROWS: [(Command, &[&str]); 13] = [
        (Command::Generate, &["generate"]),
        (Command::Docs, &["docs"]),
        (Command::Env, &["env"]),
        (Command::Codegen, &["codegen"]),
        (
            Command::Run,
            &[
                "run",
                "--bazel-option=--jobs=4",
                "//app:bin",
                "--",
                "--app-flag",
            ],
        ),
        (Command::Build, &["build"]),
        (Command::Test, &["test"]),
        (Command::Coverage, &["coverage"]),
        (Command::Lint, &["lint"]),
        (Command::Typecheck, &["typecheck"]),
        (Command::Format, &["format"]),
        (Command::Check, &["check"]),
        (Command::Fix, &["fix"]),
    ];

    const NO_BAZEL_BUILD: [Command; 15] = [
        Command::Security,
        Command::License,
        Command::Clean,
        Command::Update,
        Command::Bump,
        Command::Add,
        Command::Migrate,
        Command::Upgrade,
        Command::Status,
        Command::Version,
        Command::Watch,
        Command::ListTests,
        Command::Rollback,
        Command::Worktree,
        Command::Explain,
    ];

    fn bazel_build_argvs(words: &[&str], agent: bool, output: &str) -> Vec<Vec<String>> {
        let mut words = words.to_vec();
        let at = words
            .iter()
            .position(|word| *word == "--")
            .unwrap_or(words.len());
        words.insert(at, output);
        let mut inv = super::test_support::invocation(&words);
        inv.agent = agent;
        let harness = Harness::new(&format!("agent-quiet-{}", words[0]));
        let run = harness.probe_with(&inv, &[Some(0)]);
        run.argv
            .into_iter()
            .filter(|argv| {
                argv.iter()
                    .take_while(|arg| *arg != "--")
                    .any(|arg| matches!(arg.as_str(), "build" | "run" | "test" | "coverage"))
            })
            .collect()
    }

    #[test]
    fn agent_quiet_flags_reach_every_bazel_backed_json_command() {
        use dx_plan::args::command::COMMANDS;
        use dx_plan::args::AGENT_QUIET_FLAGS;
        for (command, words) in QUIET_ROWS {
            let argvs = bazel_build_argvs(words, true, "--output=json");
            assert!(
                !argvs.is_empty(),
                "{} launches no Bazel build",
                command.name()
            );
            for argv in &argvs {
                let end = argv
                    .iter()
                    .position(|arg| arg == "--")
                    .unwrap_or(argv.len());
                for flag in AGENT_QUIET_FLAGS {
                    assert!(
                        argv[..end].contains(&flag.to_owned()),
                        "{}: {flag}: {argv:?}",
                        command.name()
                    );
                    assert!(
                        !argv[end..].contains(&flag.to_owned()),
                        "{}: {flag} after --: {argv:?}",
                        command.name()
                    );
                }
            }
            if command == Command::Run {
                let argv = argvs
                    .iter()
                    .find(|argv| argv.iter().any(|arg| arg == "--"))
                    .expect("app args");
                let end = argv.iter().position(|arg| arg == "--").expect("app args");
                assert_eq!(argv[end + 1..], ["--app-flag".to_owned()], "{argv:?}");
                for argv in &argvs {
                    let position = |word: &str| argv.iter().position(|arg| arg == word);
                    let jobs = position("--jobs=4").expect("user option");
                    assert!(
                        AGENT_QUIET_FLAGS
                            .iter()
                            .all(|flag| position(flag).is_some_and(|at| at < jobs)),
                        "{argv:?}"
                    );
                    assert!(jobs < position("//app:bin").expect("label"), "{argv:?}");
                }
            }
            for (agent, output) in [(true, "--output=text"), (false, "--output=json")] {
                for argv in bazel_build_argvs(words, agent, output) {
                    assert!(
                        !argv.contains(&"--noshow_progress".to_owned()),
                        "{} agent={agent} {output}: {argv:?}",
                        command.name()
                    );
                }
            }
        }
        for meta in COMMANDS.iter().filter(|meta| meta.supports_json) {
            let listed = QUIET_ROWS
                .iter()
                .any(|(command, _)| *command == meta.command)
                || NO_BAZEL_BUILD.contains(&meta.command);
            assert!(listed, "{} supports JSON but is not classified", meta.name);
        }
    }

    #[test]
    fn removed_dry_run_flag_launches_nothing() {
        for argv in [
            vec!["build", "--dry-run", "//a:one"],
            vec!["security", "--dry-run"],
            vec!["license", "--dry-run=false"],
            vec!["update", "--dry-run"],
            vec!["bump", "cargo:anyhow", "1.2.3", "--dry-run"],
            vec!["add", "cargo:anyhow", "1.2.3", "--dry-run"],
            vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--dry-run"],
            vec!["upgrade", "--from=1.2.3", "--to=2.0.0", "--dry-run"],
        ] {
            let name = format!("exec-dispatch-{}", argv[0].trim_start_matches('-'));
            let harness = Harness::new(&name);
            let (code, _out, err) = harness.run(&argv);
            assert_ne!(code, 0, "{argv:?}");
            assert!(err.contains("--dry-run"), "{argv:?}: {err}");
            assert!(
                harness.seen_env.borrow().is_empty(),
                "{argv:?} launches nothing"
            );
        }
    }

    #[test]
    fn deferred_families_fail_closed_without_launch() {
        for argv in [vec!["security"]] {
            let name = format!("exec-deferred-{}", argv[0]);
            let harness = Harness::new(&name);
            harness
                .query
                .outputs
                .borrow_mut()
                .push(dx_plan::resolve::QueryResult {
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: b"ERROR: no such repository\n".to_vec(),
                });
            let (code, _out, err) = harness.run(&argv);
            assert_eq!(code, 1, "{argv:?}");
            assert!(err.contains("audit_failed"), "{argv:?} fails closed: {err}");
        }
    }

    fn leased_slot(session: Option<&str>, held: bool) -> tempfile::TempDir {
        leased_slot_of(session, held, None, false, false)
    }

    fn leased_slot_of(
        session: Option<&str>,
        held: bool,
        agent: Option<&str>,
        unclaimed: bool,
        gone: bool,
    ) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("dx-exec-lease-");
        let slot = scratch.path().join("pool").join("1");
        std::fs::create_dir_all(&slot).expect("slot");
        std::fs::create_dir_all(scratch.path().join("pool").join(".dx")).expect("meta");
        let record = dx_worktree::pool::Record {
            slot: 1,
            held,
            branch: Some("feature/a".to_owned()),
            last_branch: None,
            session: session.map(str::to_owned),
            agent: agent.map(str::to_owned),
            pid: Some(7),
            gone,
            stopped_at: None,
            unclaimed,
            broken: false,
            leased_at: 0,
            last_used: 0,
            lock_digest: None,
        };
        std::fs::write(
            scratch.path().join("pool").join(".dx").join("1.json"),
            serde_json::to_vec(&record).expect("json"),
        )
        .expect("record");
        scratch
    }

    fn who(session: Option<&str>) -> dx_worktree::Who {
        dx_worktree::Who {
            session: session.map(str::to_owned),
            agent: None,
            pid: 99,
        }
    }

    #[test]
    fn commands_in_a_slot_leased_to_another_session_are_refused() {
        let scratch = leased_slot(Some("s1"), true);
        let inside = scratch.path().join("pool").join("1").join("sub");
        std::fs::create_dir_all(&inside).expect("sub");
        let refusal =
            lease_refusal_for(Command::Build, &inside, &who(Some("s2"))).expect("refused");
        assert!(refusal.contains("session s1"), "{refusal}");
        assert!(refusal.contains("dx worktree add"), "{refusal}");
        assert!(lease_refusal_for(Command::Status, &inside, &who(Some("s2"))).is_some());
    }

    fn agent_who(session: &str, agent: &str) -> dx_worktree::Who {
        dx_worktree::Who {
            session: Some(session.to_owned()),
            agent: Some(agent.to_owned()),
            pid: 99,
        }
    }

    #[test]
    fn another_agent_of_the_same_session_is_refused() {
        let scratch = leased_slot_of(Some("s1"), true, Some("a1"), false, false);
        let slot = scratch.path().join("pool").join("1");
        let refusal =
            lease_refusal_for(Command::Build, &slot, &agent_who("s1", "a2")).expect("refused");
        assert!(refusal.contains("agent a1"), "{refusal}");
        assert!(refusal.contains("dx worktree add"), "{refusal}");
        assert!(lease_refusal_for(Command::Status, &slot, &agent_who("s1", "a2")).is_some());
    }

    #[test]
    fn the_same_agent_and_agentless_callers_are_not_refused() {
        let scratch = leased_slot_of(Some("s1"), true, Some("a1"), false, false);
        let slot = scratch.path().join("pool").join("1");
        assert!(lease_refusal_for(Command::Build, &slot, &agent_who("s1", "a1")).is_none());
        assert!(lease_refusal_for(Command::Build, &slot, &who(Some("s1"))).is_none());
    }

    #[test]
    fn unclaimed_and_gone_slots_keep_the_session_rule() {
        for (unclaimed, gone) in [(true, false), (false, true)] {
            let scratch = leased_slot_of(Some("s1"), true, Some("a1"), unclaimed, gone);
            let slot = scratch.path().join("pool").join("1");
            assert!(
                lease_refusal_for(Command::Build, &slot, &agent_who("s1", "a2")).is_none(),
                "unclaimed={unclaimed} gone={gone}"
            );
            assert!(lease_refusal_for(Command::Build, &slot, &who(Some("s2"))).is_some());
        }
    }

    #[test]
    fn execute_in_a_foreign_leased_slot_prints_one_line_and_no_usage() {
        let scratch = leased_slot(Some("session-that-is-not-ours"), true);
        let mut harness = Harness::new("exec-lease-refusal");
        harness.workspace = scratch.path().join("pool").join("1");
        harness.cwd = harness.workspace.clone();
        let (code, out, err) = harness.run(&["build", "//a:one"]);
        assert_eq!(code, dx_process::pre_exec_code());
        assert!(out.is_empty(), "{out}");
        assert!(err.starts_with("dx: "), "{err}");
        assert!(err.contains("dx worktree add"), "{err}");
        assert!(!err.contains("usage:"), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn affected_in_a_foreign_leased_slot_is_refused_before_any_query() {
        let scratch = leased_slot(Some("session-that-is-not-ours"), true);
        let mut harness = Harness::new("exec-affected-lease-refusal");
        harness.workspace = scratch.path().join("pool").join("1");
        harness.cwd = harness.workspace.clone();
        let (code, out, err) = harness.run(&["check", "--affected"]);
        assert_eq!(code, dx_process::pre_exec_code());
        assert!(out.is_empty(), "{out}");
        assert!(err.starts_with("dx: "), "{err}");
        assert!(err.contains("dx worktree add"), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
        assert!(harness.query.calls.borrow().is_empty());
        assert!(harness.query.info_calls.borrow().is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn watch_with_affected_reaches_watch_before_any_affected_query() {
        let harness = Harness::new("exec-watch-affected");
        let (code, out, err) = harness.run_with_ci(&["watch", "test", "--affected"], true);
        assert_eq!(code, dx_process::pre_exec_code(), "{out}{err}");
        assert!(err.contains("refuses CI"), "{err}");
        assert!(harness.query.calls.borrow().is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn affected_state_errors_print_one_line_and_no_usage() {
        let harness = Harness::new("exec-affected-state");
        for _ in 0..5 {
            harness
                .query
                .outputs
                .borrow_mut()
                .push(dx_plan::resolve::QueryResult {
                    code: Some(1),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
        }
        let (code, _out, err) = harness.run(&["build", "--affected"]);
        assert_eq!(code, dx_process::pre_exec_code());
        assert!(err.starts_with("dx: "), "{err}");
        assert!(!err.contains("usage:"), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
    }

    #[test]
    fn a_slot_command_holds_the_busy_lock_until_it_returns() {
        let scratch = leased_slot(Some("s1"), true);
        let slot = scratch.path().join("pool").join("1");
        let busy = scratch.path().join("pool").join(".dx").join("1.busy");
        let held = hold_slot(Command::Test, &slot.join("sub"), Duration::from_secs(10))
            .expect("in a slot")
            .expect("free");
        assert!(dx_atomic_fs::lease::try_lock_file(&busy, false)
            .expect("probe")
            .is_none());
        let nested = hold_slot(Command::Check, &slot, Duration::from_secs(10))
            .expect("in a slot")
            .expect("shared");
        drop(held);
        drop(nested);
        dx_atomic_fs::lease::acquire_exclusive_at(&busy, Duration::from_secs(10)).expect("free");
        for command in [
            Command::Worktree,
            Command::Version,
            Command::Completion,
            Command::Explain,
            Command::Install,
        ] {
            assert!(hold_slot(command, &slot, Duration::ZERO).is_none());
        }
        assert!(hold_slot(Command::Test, scratch.path(), Duration::ZERO).is_none());
    }

    #[test]
    fn a_slot_being_switched_refuses_after_the_wait() {
        let scratch = leased_slot(Some("s1"), true);
        let slot = scratch.path().join("pool").join("1");
        let busy = scratch.path().join("pool").join(".dx").join("1.busy");
        let switching = dx_atomic_fs::lease::acquire_exclusive_at(&busy, Duration::from_secs(10))
            .expect("free");
        let refusal = hold_slot(Command::Build, &slot, Duration::from_millis(100))
            .expect("in a slot")
            .expect_err("refused");
        assert_eq!(
            refusal,
            "slot 1 is being switched by dx worktree; run the command again"
        );
        drop(switching);
    }

    #[test]
    fn the_holder_and_exempt_commands_are_not_refused() {
        let scratch = leased_slot(Some("s1"), true);
        let slot = scratch.path().join("pool").join("1");
        assert!(lease_refusal_for(Command::Build, &slot, &who(Some("s1"))).is_none());
        for command in [
            Command::Worktree,
            Command::Version,
            Command::Completion,
            Command::Explain,
            Command::Install,
        ] {
            assert!(lease_refusal_for(command, &slot, &who(Some("s2"))).is_none());
        }
        assert!(lease_refusal_for(Command::Build, scratch.path(), &who(Some("s2"))).is_none());
    }

    #[test]
    fn a_session_set_by_the_guard_owns_its_lease() {
        let scratch = leased_slot(Some("s1"), true);
        let slot = scratch.path().join("pool").join("1");
        let guarded = |session: &'static str| {
            dx_worktree::Who::from_env(&move |name: &str| {
                (name == "DX_WORKTREE_SESSION").then(|| session.to_owned())
            })
        };
        assert!(lease_refusal_for(Command::Build, &slot, &guarded("s1")).is_none());
        assert!(lease_refusal_for(Command::Build, &slot, &guarded("s2")).is_some());
        assert!(lease_refusal_for(Command::Build, &slot, &who(None)).is_some());
    }

    #[test]
    fn free_slots_and_leases_without_a_session_are_not_refused() {
        let free = leased_slot(Some("s1"), false);
        assert!(lease_refusal_for(
            Command::Build,
            &free.path().join("pool").join("1"),
            &who(Some("s2"))
        )
        .is_none());
        let bare = leased_slot(None, true);
        assert!(lease_refusal_for(
            Command::Build,
            &bare.path().join("pool").join("1"),
            &who(Some("s2"))
        )
        .is_none());
    }
}
