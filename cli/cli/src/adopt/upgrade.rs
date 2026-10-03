use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event, emit_started};

use dx_output::{
    command_finished, command_started, notice_event, FinishedCounts, NoticeEvent, OutputMode,
};

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_UPGRADE_FAILED: &str = "upgrade_failed";

pub(crate) const NOTICE_UPGRADE_PLANNED: &str = "upgrade_planned";

pub(crate) fn execute_upgrade(
    invocation: &Invocation,
    _workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let (Some(from), Some(to)) = (invocation.from.as_deref(), invocation.to.as_deref()) else {
        return pre_exec(err, "upgrade needs --from <version> --to <version>");
    };
    let plan = match dx_adopt::plan_upgrade(from, to) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(err, &error.to_string()),
    };
    let summary = format!(
        "Would upgrade {} -> {} via {} (pin {}, migrate, setup)",
        plan.from, plan.to, plan.manifest, plan.to
    );
    let verbose = invocation.chatty();
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Ok(event) = notice_event(&NoticeEvent {
                level: "info".to_owned(),
                code: NOTICE_UPGRADE_PLANNED.to_owned(),
                message: summary,
                related_command: Some("upgrade".to_owned()),
                scope: Some(vec![plan.manifest.clone()]),
                path: Some(plan.manifest.clone()),
                language: None,
                import: None,
            }) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            let finished = command_finished(0, &FinishedCounts::default());
            if let Err(exit) = emit_event(out, &finished) {
                return exit;
            }
        } else if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
                return exit;
            }
        }
        return 0;
    }
    let live_summary = format!(
        "Upgrade {} -> {} via {} (pin {}, migrate, setup)",
        plan.from, plan.to, plan.manifest, plan.to
    );
    let message = format!(
        "no upgrade manifest {} yet (module at 0.0.0, no releases cut); {}",
        plan.manifest, plan.message
    );
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
        return operational(invocation, out, err, CODE_UPGRADE_FAILED, &message);
    }
    if verbose {
        if let Err(exit) = check_stdout_write(writeln!(out, "{live_summary}")) {
            return exit;
        }
    }
    operational(invocation, out, err, CODE_UPGRADE_FAILED, &message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::{execute_adoption, AdoptEnv};
    use crate::args::parse;
    use dx_process::pre_exec_code;
    use std::io;

    fn invocation(words: &[&str]) -> Invocation {
        parse(&words.iter().map(ToString::to_string).collect::<Vec<_>>()).expect("parse")
    }

    struct NullQuery;

    impl crate::resolve::QueryRunner for NullQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &std::path::Path,
        ) -> io::Result<crate::resolve::QueryResult> {
            Ok(crate::resolve::QueryResult {
                code: Some(0),
                stdout: b"//a:one\n".to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    struct NullRunner;

    impl dx_process::Runner for NullRunner {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &std::path::Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            Ok(dx_process::ChildStatus { code: Some(0) })
        }
    }

    #[test]
    fn upgrade_dry_run_plans_composition_without_writing() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        assert!(text.contains("1.2.3 -> 2.0.0"), "{text}");
        assert!(text.contains("migrate-v1-to-v2.json"), "{text}");
        assert!(text.contains("pin 2.0.0"), "{text}");
        assert!(text.contains("setup"), "{text}");
    }

    #[test]
    fn upgrade_dry_run_json_emits_planned_and_finished() {
        let inv = invocation(&[
            "upgrade",
            "--from=1.2.3",
            "--to=2.0.0",
            "--dry-run",
            "--output=json",
        ]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-dry-json-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("out");
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("NDJSON");
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"notice"), "{kinds:?}");
        let notice = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("notice"))
            .expect("upgrade notice");
        assert_eq!(notice["code"], serde_json::json!(NOTICE_UPGRADE_PLANNED));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    #[test]
    fn upgrade_live_fails_closed_with_recovery_pointer() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-live-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 1);
        let err_text = String::from_utf8(err).expect("err");
        assert!(err_text.contains(CODE_UPGRADE_FAILED), "{err_text}");
        assert!(err_text.contains("migrate-v1-to-v2.json"), "{err_text}");
        assert!(
            err_text.contains("dx upgrade --from 1.2.3 --to 2.0.0"),
            "{err_text}"
        );
        assert!(err_text.contains("dx setup"), "{err_text}");
    }

    #[test]
    fn upgrade_live_json_fails_closed_with_error_and_finished() {
        let inv = invocation(&["upgrade", "--from=1.2.3", "--to=2.0.0", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-live-json-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, 1);
        assert!(String::from_utf8(err)
            .expect("err")
            .contains(CODE_UPGRADE_FAILED));
        let text = String::from_utf8(out).expect("out");
        let events: Vec<serde_json::Value> = text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()
            .expect("NDJSON");
        let kinds: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().expect("event"))
            .collect();
        assert_eq!(kinds[0], "command_started");
        assert!(kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("error"))
            .expect("upgrade error");
        assert_eq!(error["code"], serde_json::json!(CODE_UPGRADE_FAILED));
    }

    #[test]
    fn upgrade_codes_are_stable_single_source() {
        assert_eq!(CODE_UPGRADE_FAILED, "upgrade_failed");
        assert_eq!(NOTICE_UPGRADE_PLANNED, "upgrade_planned");
    }

    #[test]
    fn upgrade_rejects_downgrade_pre_exec() {
        let inv = invocation(&["upgrade", "--from=2.0.0", "--to=1.0.0", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-upgrade-downgrade-");
        let root = scratch.path().to_path_buf();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_adoption(
            &inv,
            AdoptEnv {
                workspace: &root,
                query_runner: &NullQuery,
                runner: &NullRunner,
                out: &mut out,
                err: &mut err,
            },
        );
        assert_eq!(code, pre_exec_code());
        assert!(String::from_utf8(err)
            .expect("err")
            .contains("upgrade-only"));
    }
}
