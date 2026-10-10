use super::common::*;
use crate::args::{Command, Invocation};
use crate::reports::plan_reports;
use dx_output::{command_started, write_event, OutputMode};

pub(crate) fn execute_migrate(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Migrate,
        "migrate dispatch guards commands"
    );
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
        invocation.dry_run,
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(err, &error.to_string()),
    }
    let (Some(from), Some(to)) = (invocation.from.as_deref(), invocation.to.as_deref()) else {
        return pre_exec(err, "migrate needs --from <version> --to <version>");
    };
    let plan = match dx_adopt::plan_migrate(from, to) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let verbose = invocation.chatty();
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), invocation.dry_run, "default")
        {
            let _ = write_event(out, &event);
        }
    } else if verbose && !invocation.dry_run {
        let _ = writeln!(
            out,
            "Would migrate {} -> {} via {}",
            plan.from, plan.to, plan.manifest
        );
    }
    operational(
        invocation,
        out,
        err,
        CODE_MIGRATE_FAILED,
        &format!(
            "no migrate manifest {} yet (module at 0.0.0, no releases cut)",
            plan.manifest
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::test_support::strings;

    #[test]
    fn dry_run_fails_closed_without_manufacturing_a_plan() {
        let harness = Harness::new("migrate-dryrun");
        let (code, out, err) = harness.run(&["migrate", "--from=1.2.3", "--to=2.0.0", "--dry-run"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
        assert!(!out.contains("Would migrate"), "{out}");
    }

    #[test]
    fn dry_run_json_fails_closed_with_error_and_finished() {
        let harness = Harness::new("migrate-dryrun-json");
        let (code, out, err) = harness.run(&[
            "migrate",
            "--from=1.2.3",
            "--to=2.0.0",
            "--dry-run",
            "--output=json",
        ]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert!(err.contains("migrate_failed"), "{err}");
    }

    #[test]
    fn live_fails_closed_with_migrate_failed() {
        let harness = Harness::new("migrate-live-closed");
        let (code, _, err) = harness.run(&["migrate", "--from=1.2.3", "--to=2.0.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
    }

    #[test]
    fn live_json_fails_closed_with_error_and_finished() {
        let harness = Harness::new("migrate-live-json");
        let (code, out, err) =
            harness.run(&["migrate", "--from=1.2.3", "--to=2.0.0", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
    }

    #[test]
    fn explicit_check_matches_the_bare_default() {
        let harness = Harness::new("migrate-check");
        let (code, out, err) = harness.run(&["migrate", "--from=1.2.3", "--to=2.0.0", "--check"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
    }

    #[test]
    fn missing_versions_and_non_upgrades_are_pre_exec() {
        use crate::args::parse;
        assert!(matches!(
            parse(&strings(&["migrate", "--from=1.2.3", "--dry-run"])),
            Err(crate::args::ArgsError::MissingValue { .. })
        ));
        let harness = Harness::new("migrate-minor");
        let (code, _out, err) =
            harness.run(&["migrate", "--from=1.2.3", "--to=1.3.0", "--dry-run"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1.2.3-to-v1.3.0.json"), "{err}");
        let harness = Harness::new("migrate-downgrade");
        let (code, _, err) = harness.run(&["migrate", "--from=2.0.0", "--to=1.0.0", "--dry-run"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("upgrade-only"), "{err}");
        let harness = Harness::new("migrate-equal");
        let (code, _, err) = harness.run(&["migrate", "--from=1.2.3", "--to=1.2.3", "--dry-run"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("upgrade-only"), "{err}");
        let harness = Harness::new("migrate-badsemver");
        let (code, _, err) = harness.run(&["migrate", "--from=abc", "--to=2.0.0", "--dry-run"]);
        assert_eq!(code, 2, "{err}");
    }

    #[test]
    fn from_to_belong_to_migrate_only() {
        use crate::args::parse;
        assert!(matches!(
            parse(&strings(&["lint", "--from=1.0.0"])),
            Err(crate::args::ArgsError::UnsupportedOption { .. })
        ));
        assert!(matches!(
            parse(&strings(&["build", "//a:one", "--to=2.0.0"])),
            Err(crate::args::ArgsError::UnsupportedOption { .. })
        ));
    }
}
