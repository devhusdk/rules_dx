mod audit;
mod bazel;
mod bump;
mod clean;
pub mod common;
mod deploy;
mod docs;
mod generate;
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
mod quality_emit;
mod quality_patch;
mod quality_reports;
mod results;
mod run;
mod test_reports;
#[cfg(test)]
mod test_support;
mod umbrella;
mod update;
mod workflow;

use crate::args::{Command, Invocation};

pub use common::Env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Adoption,
    Umbrella,
    Workflow,
    Bazel,
    Generate,
    Clean,
    Managed,
    Audit,
    Update,
    Bump,
    Migrate,
    Docs,
    Quality,
}

impl Family {
    #[cfg(test)]
    fn name(self) -> &'static str {
        match self {
            Family::Adoption => "adoption",
            Family::Umbrella => "umbrella",
            Family::Workflow => "workflow",
            Family::Bazel => "bazel",
            Family::Generate => "generate",
            Family::Clean => "clean",
            Family::Managed => "managed",
            Family::Audit => "audit",
            Family::Update => "update",
            Family::Bump => "bump",
            Family::Migrate => "migrate",
            Family::Docs => "docs",
            Family::Quality => "quality",
        }
    }

    fn supports_diff(self) -> bool {
        matches!(self, Family::Umbrella | Family::Generate | Family::Quality)
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
        Command::Check | Command::Fix => Family::Umbrella,
        Command::Clean => Family::Clean,
        Command::Update => Family::Update,
        Command::Bump => Family::Bump,
        Command::Migrate => Family::Migrate,
        Command::Codegen | Command::Env | Command::Setup => Family::Managed,
        Command::Init
        | Command::New
        | Command::Upgrade
        | Command::Hooks
        | Command::Status
        | Command::Version
        | Command::Watch
        | Command::Owners
        | Command::Deps
        | Command::Why
        | Command::Completion
        | Command::Capabilities => Family::Adoption,
        Command::Docs => Family::Docs,
        Command::Bazel => Family::Bazel,
    }
}

pub fn execute(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.output != dx_output::OutputMode::Diff
            || family(invocation.command).supports_diff(),
        "{} reaches a family whose commands reject --output=diff",
        invocation.command.name()
    );
    if invocation.here {
        let Env { err, .. } = env;
        return common::pre_exec(err, "option \"--here/--cwd\" needs cwd resolution");
    }
    if invocation.command == Command::Watch {
        return crate::adopt::watch::execute_watch(invocation, env);
    }
    match family(invocation.command) {
        Family::Adoption => {
            let Env {
                workspace,
                runner,
                query_runner,
                out,
                err,
                ..
            } = env;
            crate::adopt::execute_adoption(
                invocation,
                crate::adopt::AdoptEnv {
                    workspace,
                    query_runner,
                    runner,
                    out,
                    err,
                },
            )
        }
        Family::Umbrella => umbrella::execute_umbrella(invocation, env),
        Family::Workflow => workflow::execute_workflow(invocation, env),
        Family::Bazel => bazel::execute_bazel(invocation, env),
        Family::Generate => generate::execute_generate(invocation, env),
        Family::Clean => clean::execute_clean(invocation, env),
        Family::Managed => managed::execute_managed(invocation, env),
        Family::Audit => audit::execute_audit(invocation, env),
        Family::Update => update::execute_update(invocation, env),
        Family::Bump => bump::execute_bump(invocation, env),
        Family::Migrate => migrate::execute_migrate(invocation, env),
        Family::Docs => docs::execute_docs(invocation, env),
        Family::Quality => quality::execute_quality(invocation, env),
    }
}

#[cfg(test)]
mod tests {
    use super::family;
    use super::test_support::Harness;
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
        assert_eq!(any_supports_diff.len(), 13, "every family classified");
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
    fn every_command_has_exactly_one_family() {
        use clap::ValueEnum;
        let mut counts = std::collections::BTreeMap::new();
        for command in Command::value_variants() {
            *counts.entry(family(*command).name()).or_insert(0usize) += 1;
        }
        assert_eq!(
            counts.values().sum::<usize>(),
            34,
            "every variant classified"
        );
        assert_eq!(counts.get("adoption"), Some(&12));
        assert_eq!(counts.get("workflow"), Some(&5));
        assert_eq!(counts.get("quality"), Some(&3));
        assert_eq!(counts.get("managed"), Some(&3));
        assert_eq!(counts.get("audit"), Some(&2));
        assert_eq!(counts.get("umbrella"), Some(&2));
        assert_eq!(counts.get("generate"), Some(&1));
        assert_eq!(counts.get("clean"), Some(&1));
        assert_eq!(counts.get("update"), Some(&1));
        assert_eq!(counts.get("bump"), Some(&1));
        assert_eq!(counts.get("migrate"), Some(&1));
        assert_eq!(counts.get("docs"), Some(&1));
        assert_eq!(counts.get("bazel"), Some(&1));
    }

    #[test]
    fn dry_run_families_launch_nothing() {
        for argv in [
            vec!["build", "--dry-run", "//a:one"],
            vec!["security", "--dry-run"],
            vec!["license", "--dry-run"],
            vec!["update", "--dry-run"],
            vec!["bump", "cargo:anyhow", "1.2.3", "--dry-run"],
            vec!["migrate", "--from=1.2.3", "--to=2.0.0", "--dry-run"],
        ] {
            let name = format!("exec-dispatch-{}", argv[0].trim_start_matches('-'));
            let harness = Harness::new(&name);
            let (code, _out, err) = harness.run(&argv);
            assert_eq!(code, 0, "{argv:?}");
            assert_eq!(err, "", "{argv:?}");
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
}
