mod affected;
mod audit;
mod build_failures;
mod bump;
mod clean;
pub mod common;
#[cfg(test)]
mod delivery_sink;
mod deploy;
mod docs;
mod failure;
mod failure_reports;
mod generate;
mod guidance;
mod managed;
mod managed_codegen;
mod managed_env;
mod managed_prepare;
mod managed_staging;
mod migrate;
#[cfg(test)]
mod package_identity;
mod quality;
mod quality_apply;
mod quality_baseline;
mod quality_emit;
mod quality_patch;
mod quality_reconcile;
mod quality_reports;
mod results;
mod run;
mod test_inventory;
mod test_reports;
#[cfg(test)]
pub(crate) mod test_support;
mod umbrella;
mod update;
mod workflow;
mod worktree;

use crate::args::{Command, Invocation};

pub use common::Env;
pub(crate) use managed_env::{ENV_ARTIFACTS_DIR, ENV_VALUES_FILE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Adoption,
    Watch,
    Umbrella,
    Workflow,
    Generate,
    Clean,
    Managed,
    Audit,
    Update,
    Bump,
    Migrate,
    Docs,
    Quality,
    Worktree,
}

impl Family {
    #[cfg(test)]
    fn name(self) -> &'static str {
        match self {
            Family::Adoption => "adoption",
            Family::Watch => "watch",
            Family::Umbrella => "umbrella",
            Family::Workflow => "workflow",
            Family::Generate => "generate",
            Family::Clean => "clean",
            Family::Managed => "managed",
            Family::Audit => "audit",
            Family::Update => "update",
            Family::Bump => "bump",
            Family::Migrate => "migrate",
            Family::Docs => "docs",
            Family::Quality => "quality",
            Family::Worktree => "worktree",
        }
    }

    fn supports_diff(self) -> bool {
        matches!(
            self,
            Family::Umbrella | Family::Generate | Family::Quality | Family::Watch
        )
    }
}

fn family(command: Command) -> Family {
    match command {
        Command::Security | Command::License => Family::Audit,
        Command::Lint | Command::Typecheck | Command::Format => Family::Quality,
        Command::Generate => Family::Generate,
        Command::Build | Command::Test | Command::Coverage | Command::Run | Command::Deploy => {
            Family::Workflow
        }
        Command::Check | Command::Fix | Command::ListTests => Family::Umbrella,
        Command::Clean => Family::Clean,
        Command::Update => Family::Update,
        Command::Bump => Family::Bump,
        Command::Migrate => Family::Migrate,
        Command::Codegen | Command::Env => Family::Managed,
        Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Rollback
        | Command::Status
        | Command::Version
        | Command::Completion => Family::Adoption,
        Command::Watch => Family::Watch,
        Command::Docs => Family::Docs,
        Command::Worktree => Family::Worktree,
    }
}

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
    let who = crate::worktree::Who::from_env(&|name| {
        std::env::var(name).ok().filter(|value| !value.is_empty())
    });
    lease_refusal_for(invocation.command, workspace, &who)
}

fn lease_refusal_for(
    command: Command,
    workspace: &std::path::Path,
    who: &crate::worktree::Who,
) -> Option<String> {
    if matches!(
        command,
        Command::Worktree | Command::Version | Command::Completion
    ) {
        return None;
    }
    let (path, record) = crate::worktree::lease_for_workspace(workspace)?;
    if !record.held || record.session.is_none() || record.held_by(who) {
        return None;
    }
    Some(format!(
        "{} is leased to {}; run dx worktree add <branch> for your own slot",
        path.display(),
        record.holder(true).unwrap_or_default()
    ))
}

pub fn execute(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.output != dx_output::OutputMode::Diff
            || family(invocation.command).supports_diff(),
        "{} reaches a family whose commands reject --output=diff",
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
    if invocation.affected {
        return affected::execute_affected(invocation, env);
    }
    if let Some(refusal) = lease_refusal(invocation, env.workspace) {
        let Env { out, err, .. } = env;
        return common::pre_exec(invocation.output, out, err, &refusal);
    }
    match family(invocation.command) {
        Family::Adoption => {
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
        Family::Watch => crate::adopt::watch::execute_watch(invocation, env),
        Family::Umbrella => {
            if invocation.command == Command::ListTests {
                test_inventory::execute_tests(invocation, env)
            } else {
                umbrella::execute_umbrella(invocation, env)
            }
        }
        Family::Workflow => workflow::execute_workflow(invocation, env),
        Family::Generate => generate::execute_generate(invocation, env),
        Family::Clean => clean::execute_clean(invocation, env),
        Family::Managed => managed::execute_managed(invocation, env),
        Family::Audit => audit::execute_audit(invocation, env),
        Family::Update => update::execute_update(invocation, env),
        Family::Bump => bump::execute_bump(invocation, env),
        Family::Migrate => migrate::execute_migrate(invocation, env),
        Family::Docs => docs::execute_docs(invocation, env),
        Family::Quality => quality::execute_quality(invocation, env),
        Family::Worktree => worktree::execute_worktree(invocation, env),
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::Harness;
    use super::{family, lease_refusal_for};
    use crate::args::Command;

    #[test]
    fn diff_families_match_the_registry() {
        use crate::args::command::COMMANDS;
        use std::collections::BTreeMap;
        let mut any_supports_diff: BTreeMap<&str, bool> = BTreeMap::new();
        for meta in COMMANDS {
            *any_supports_diff
                .entry(family(meta.command).name())
                .or_insert(false) |= meta.supports_diff;
        }
        for meta in COMMANDS {
            let name = family(meta.command).name();
            assert_eq!(
                family(meta.command).supports_diff(),
                any_supports_diff[name],
                "family {name} disagrees with COMMANDS supports_diff"
            );
        }
    }

    #[test]
    fn every_command_has_its_expected_family() {
        use clap::ValueEnum;
        let expected = [
            (Command::Security, "audit"),
            (Command::License, "audit"),
            (Command::Lint, "quality"),
            (Command::Typecheck, "quality"),
            (Command::Format, "quality"),
            (Command::Generate, "generate"),
            (Command::Build, "workflow"),
            (Command::Test, "workflow"),
            (Command::Coverage, "workflow"),
            (Command::Run, "workflow"),
            (Command::Deploy, "workflow"),
            (Command::Check, "umbrella"),
            (Command::Fix, "umbrella"),
            (Command::ListTests, "umbrella"),
            (Command::Clean, "clean"),
            (Command::Update, "update"),
            (Command::Bump, "bump"),
            (Command::Migrate, "migrate"),
            (Command::Codegen, "managed"),
            (Command::Env, "managed"),
            (Command::Init, "adoption"),
            (Command::New, "adoption"),
            (Command::Upgrade, "adoption"),
            (Command::Rollback, "adoption"),
            (Command::Status, "adoption"),
            (Command::Version, "adoption"),
            (Command::Completion, "adoption"),
            (Command::Watch, "watch"),
            (Command::Docs, "docs"),
            (Command::Worktree, "worktree"),
        ];
        for command in Command::value_variants() {
            let row = expected.iter().find(|(c, _)| c == command);
            let (_, name) =
                row.unwrap_or_else(|| panic!("{} has no expected family", command.name()));
            assert_eq!(family(*command).name(), *name, "{}", command.name());
        }
        assert_eq!(expected.len(), Command::value_variants().len());
    }

    #[test]
    fn removed_dry_run_flag_launches_nothing() {
        for argv in [
            vec!["build", "--dry-run", "//a:one"],
            vec!["security", "--dry-run"],
            vec!["license", "--dry-run=false"],
            vec!["update", "--dry-run"],
            vec!["bump", "cargo:anyhow", "1.2.3", "--dry-run"],
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
            let (code, _out, err) = harness.run(&argv);
            assert_eq!(code, 1, "{argv:?}");
            assert!(err.contains("audit_failed"), "{argv:?} fails closed: {err}");
        }
    }

    fn leased_slot(session: Option<&str>, held: bool) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("dx-exec-lease-");
        let slot = scratch.path().join("pool").join("1");
        std::fs::create_dir_all(&slot).expect("slot");
        std::fs::create_dir_all(scratch.path().join("pool").join(".dx")).expect("meta");
        let record = crate::worktree::pool::Record {
            slot: 1,
            held,
            branch: Some("feature/a".to_owned()),
            last_branch: None,
            session: session.map(str::to_owned),
            pid: Some(7),
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

    fn who(session: Option<&str>) -> crate::worktree::Who {
        crate::worktree::Who {
            session: session.map(str::to_owned),
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

    #[test]
    fn the_holder_and_exempt_commands_are_not_refused() {
        let scratch = leased_slot(Some("s1"), true);
        let slot = scratch.path().join("pool").join("1");
        assert!(lease_refusal_for(Command::Build, &slot, &who(Some("s1"))).is_none());
        for command in [Command::Worktree, Command::Version, Command::Completion] {
            assert!(lease_refusal_for(command, &slot, &who(Some("s2"))).is_none());
        }
        assert!(lease_refusal_for(Command::Build, scratch.path(), &who(Some("s2"))).is_none());
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
