use super::common::*;
use crate::args::{Command, Invocation};
use crate::reports::plan_reports;
use dx_output::{command_started, OutputMode};

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
    ) {
        Ok(_) => {}
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    }
    let (Some(from), Some(to)) = (invocation.from.as_deref(), invocation.to.as_deref()) else {
        return pre_exec(
            invocation.output,
            out,
            err,
            "migrate needs --from <version> --to <version>",
        );
    };
    let plan = match dx_adopt::plan_migrate(from, to) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let verbose = invocation.chatty();
    let mode = if invocation.applies() {
        "default"
    } else {
        "check"
    };
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
    } else if verbose {
        if let Err(exit) = check_stdout_write(writeln!(
            out,
            "Would migrate {} -> {} via {}",
            plan.from, plan.to, plan.manifest
        )) {
            return exit;
        }
    }
    let unshipped = dx_adopt::AdoptError::NoQualifiedManifest {
        from: plan.from,
        to: plan.to,
        manifest: plan.manifest,
    };
    operational(
        invocation,
        out,
        err,
        CODE_MIGRATE_FAILED,
        &unshipped.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_testing::cli::strings;

    #[test]
    fn live_fails_closed_with_migrate_failed() {
        let harness = Harness::new("migrate-live-closed");
        let (code, _, err) = harness.run(&["migrate", "--from=1.2.3", "--to=2.0.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
    }

    #[test]
    fn apply_fails_closed_like_default_without_writing() {
        let harness = Harness::new("migrate-apply-closed");
        let (code, _, err) = harness.run(&["migrate", "--apply", "--from=1.2.3", "--to=2.0.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1-to-v2.json"), "{err}");
    }

    #[test]
    fn unshipped_pair_reports_the_qualification_gap() {
        let harness = Harness::new("migrate-unshipped-text");
        let (code, out, err) = harness.run(&["migrate", "-v", "--from=1.2.3", "--to=2.0.0"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains(
                "no qualified migration 1.2.3 -> 2.0.0: manifest migrate-v1-to-v2.json is not shipped"
            ),
            "{err}"
        );
        assert!(!err.contains("no live"), "{err}");
        assert!(
            out.contains("Would migrate 1.2.3 -> 2.0.0 via migrate-v1-to-v2.json"),
            "{out}"
        );
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
    fn missing_versions_and_non_upgrades_are_pre_exec() {
        use crate::args::parse;
        assert!(matches!(
            parse(&strings(&["migrate", "--from=1.2.3"])),
            Err(crate::args::ArgsError::MissingValue { .. })
        ));
        let harness = Harness::new("migrate-minor");
        let (code, _, err) = harness.run(&["migrate", "--from=1.2.3", "--to=1.3.0"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("migrate_failed"), "{err}");
        assert!(err.contains("migrate-v1.2.3-to-v1.3.0.json"), "{err}");
        let harness = Harness::new("migrate-downgrade");
        let (code, _, err) = harness.run(&["migrate", "--from=2.0.0", "--to=1.0.0"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("upgrade-only"), "{err}");
        let harness = Harness::new("migrate-equal");
        let (code, _, err) = harness.run(&["migrate", "--from=1.2.3", "--to=1.2.3"]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("upgrade-only"), "{err}");
        let harness = Harness::new("migrate-badsemver");
        let (code, _, err) = harness.run(&["migrate", "--from=abc", "--to=2.0.0"]);
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
