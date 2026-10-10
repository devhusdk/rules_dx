use std::io::Write;

use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    command_finished, command_started, status_event, FinishedCounts, OutputMode, StatusEvent,
};

pub(crate) const CODE_STATUS_PIN_MISMATCH: &str = "status_pin_mismatch";

use super::{operational, summaries_suppressed};

pub(crate) fn execute_status(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.dry_run && !invocation.migrate_config {
        if invocation.output == OutputMode::Json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
        } else if !summaries_suppressed(invocation) {
            if let Err(exit) = check_stdout_write(writeln!(out, "would report status")) {
                return exit;
            }
        }
        return 0;
    }
    let pinned = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            let message = error.to_string();
            if invocation.output == OutputMode::Json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
    };
    let mut checks = dx_adopt::default_status_checks(&pinned);
    checks.push(dx_adopt::config_status_check(&invocation.config_summary));
    if invocation.migrate_config {
        return execute_config_migration(invocation, out, err);
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        for check in &checks {
            if let Ok(event) = status_event(&StatusEvent {
                name: check.name.clone(),
                status: check.status.clone(),
                detail: check.detail.clone(),
                hint: check.hint.clone(),
            }) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
        }
        let failed = checks.iter().any(|c| c.status == "error");
        if failed {
            return operational(
                invocation,
                out,
                err,
                CODE_STATUS_PIN_MISMATCH,
                "pin mismatch (see hint)",
            );
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
        return 0;
    }
    if let Err(exit) =
        check_stdout_write(writeln!(out, "{}", dx_adopt::render_status_text(&checks)))
    {
        return exit;
    }
    if checks.iter().any(|c| c.status == "error") {
        return operational(
            invocation,
            out,
            err,
            CODE_STATUS_PIN_MISMATCH,
            "pin mismatch (see hint)",
        );
    }
    0
}

fn migration_lines(migration: &dx_adopt::ConfigMigration, planned: bool) -> Vec<String> {
    let verb = if planned { "would write" } else { "wrote" };
    let remove = if planned { "would remove" } else { "removed" };
    let mut lines = Vec::new();
    lines.push(format!(
        "{} {} ({} key(s) moving{})",
        verb,
        migration.target.display(),
        migration.moving.len(),
        if migration.creates { ", creates file" } else { "" }
    ));
    for (key, _) in &migration.moving {
        lines.push(format!("  move {key}"));
    }
    for (key, value) in &migration.superseded {
        lines.push(format!("  keeps committed {key} (legacy {value} superseded)"));
    }
    lines.push(migration.body.clone());
    for legacy in &migration.removes {
        lines.push(format!("{remove} {}", legacy.display()));
    }
    lines
}

fn execute_config_migration(
    invocation: &Invocation,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let Some(migration) = dx_adopt::plan_config_migration(&invocation.config_summary.config)
    else {
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_started(invocation, out) {
                return exit;
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "no legacy config to migrate")) {
            return exit;
        }
        return 0;
    };
    if !invocation.apply {
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_started(invocation, out) {
                return exit;
            }
            for line in migration_lines(&migration, true) {
                if let Ok(event) = dx_output::notice_event(&dx_output::NoticeEvent {
                    level: "info".to_owned(),
                    code: "config_migration_planned".to_owned(),
                    message: line,
                    related_command: Some("status".to_owned()),
                    scope: None,
                    path: None,
                    language: None,
                    import: None,
                }) {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        for line in migration_lines(&migration, true) {
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                return exit;
            }
        }
        return 0;
    }
    if let Err(error) = dx_adopt::apply_config_migration(&migration) {
        let message = error.to_string();
        if invocation.output == OutputMode::Json {
            if let Err(exit) = emit_started(invocation, out) {
                return exit;
            }
        }
        return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
    }
    if invocation.output == OutputMode::Json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
        for line in migration_lines(&migration, false) {
            if let Ok(event) = dx_output::notice_event(&dx_output::NoticeEvent {
                level: "info".to_owned(),
                code: "config_migrated".to_owned(),
                message: line,
                related_command: Some("status".to_owned()),
                scope: None,
                path: None,
                language: None,
                import: None,
            }) {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
        return 0;
    }
    for line in migration_lines(&migration, false) {
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run};
    use std::io;

    #[test]
    fn status_reports_pin_and_checks() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-status-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("pin: ok"));
    }

    #[test]
    fn status_json_streams_envelope() {
        let inv = invocation(&["status", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        assert!(!events.is_empty());
        for event in &events {
            assert!(event.get("schema").is_some(), "{event}");
            assert!(event.get("event").is_some(), "{event}");
        }
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"status"), "{kinds:?}");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(
            events[0]["dry_run"],
            serde_json::Value::Bool(false),
            "{events:?}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let status = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("pin")
            })
            .expect("pin status event");
        assert_eq!(status["status"], serde_json::json!("ok"));
        assert!(err.is_empty());
    }

    #[test]
    fn status_pin_mismatch_fails_operational() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-mismatch-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("pin mismatch"), "words: {words:?}");
            if words.contains(&"--output=json") {
                let text = out;
                let events = json_events(&text);
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
                let kinds = event_kinds(&events);
                assert_eq!(kinds[0], "command_started");
                assert_eq!(kinds[kinds.len() - 1], "command_finished");
                assert!(kinds.contains(&"status"), "{kinds:?}");
                let error_index = kinds
                    .iter()
                    .position(|kind| *kind == "error")
                    .expect("status failure emits error");
                assert_eq!(kinds[error_index + 1..], ["command_finished"]);
                let error = &events[error_index];
                assert_eq!(error["code"], serde_json::json!("status_pin_mismatch"));
            }
        }
    }

    #[test]
    fn status_missing_pin_fails_closed() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-missing-");
            let root = scratch.path().to_path_buf();
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("read version pin"), "words: {words:?}");
            let stdout = out;
            assert!(!stdout.contains("pin: ok"), "words: {words:?}");
            if words.contains(&"--output=json") {
                let events = json_events(&stdout);
                assert_eq!(events[0]["event"], serde_json::json!("command_started"));
                assert_eq!(
                    events.last().expect("finished")["exit_code"],
                    serde_json::json!(1)
                );
                let kinds = event_kinds(&events);
                assert_eq!(kinds[0], "command_started");
                assert_eq!(kinds[kinds.len() - 1], "command_finished");
                assert!(!kinds.contains(&"status"), "{kinds:?}");
                let error_index = kinds
                    .iter()
                    .position(|kind| *kind == "error")
                    .expect("missing pin emits error");
                assert_eq!(kinds[error_index + 1..], ["command_finished"]);
                assert_eq!(
                    events[error_index]["code"],
                    serde_json::json!("status_pin_mismatch")
                );
            }
        }
    }

    #[test]
    fn status_empty_pin_reports_error() {
        let inv = invocation(&["status"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-empty-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "\n").expect("empty pin");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("pin mismatch"));
        assert!(!out.contains("pin: ok"));
    }

    #[test]
    fn status_dry_run_plans_without_executing() {
        let inv = invocation(&["status", "--dry-run"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-dry-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("would report status"));
        let inv = invocation(&["status", "--dry-run", "--quiet"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.is_empty());
    }

    #[test]
    fn status_dry_run_json_emits_lifecycle_only() {
        let inv = invocation(&["status", "--dry-run", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-adopt-status-dry-json-");
        let root = scratch.path().to_path_buf();
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds, vec!["command_started", "command_finished"]);
        assert_eq!(events[0]["dry_run"], serde_json::json!(true));
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
    }

    struct BrokenPipeWriter;

    impl io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "broken pipe"))
        }
    }

    #[test]
    fn status_broken_pipe_returns_141() {
        for words in [vec!["status"], vec!["status", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-adopt-status-broken-");
            let root = scratch.path().to_path_buf();
            std::fs::create_dir_all(root.join(".dx")).expect("dx");
            std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
            let mut err = Vec::new();
            let code = execute_status(&inv, &root, &mut BrokenPipeWriter, &mut err);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }

    fn invocation_with_config(words: &[&str], root: &std::path::Path) -> Invocation {
        let config = dx_adopt::load_consumer_config(root, false).expect("loads test config");
        crate::args::parse_with_config(
            &crate::test_support::strings(words),
            &|_| None,
            &config,
        )
        .expect("parse")
    }

    fn pinned_root(name: &str) -> (dx_test_scratch::TempDir, std::path::PathBuf) {
        let scratch = dx_test_scratch::scratch(name);
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        (scratch, root)
    }

    #[test]
    fn status_reports_config_origins() {
        let (scratch, root) = pinned_root("dx-adopt-status-origins-");
        std::fs::write(
            root.join("dx.toml"),
            "[dx]\noutput = \"json\"\nverbose = true\n",
        )
        .expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\ncolor = \"never\"\n").expect("local");
        let inv = invocation_with_config(&["status"], &root);
        assert_eq!(inv.config_summary.origins.get("output"), dx_adopt::DefaultOrigin::Committed);
        assert_eq!(inv.config_summary.origins.get("color"), dx_adopt::DefaultOrigin::Local);
        assert_eq!(
            inv.config_summary.origins.get("quiet"),
            dx_adopt::DefaultOrigin::Builtin
        );
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("config: ok"), "{out}");
        assert!(out.contains("files: dx.toml + dx.local.toml"), "{out}");
        assert!(out.contains("output:committed"), "{out}");
        assert!(out.contains("color:local"), "{out}");
        assert!(out.contains("quiet:built-in"), "{out}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_config_origins_rank_flag_over_env_over_file() {
        let (scratch, root) = pinned_root("dx-adopt-status-rank-");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        let config = dx_adopt::load_consumer_config(&root, false).expect("loads");
        let env = |name: &str| {
            (name == dx_adopt::DX_OUTPUT_ENV).then(|| "diff".to_owned())
        };
        let inv = crate::args::parse_with_config(
            &crate::test_support::strings(&["status"]),
            &env,
            &config,
        )
        .expect("parse");
        assert_eq!(inv.config_summary.origins.get("output"), dx_adopt::DefaultOrigin::Env);
        let inv = crate::args::parse_with_config(
            &crate::test_support::strings(&["status", "--output=text"]),
            &env,
            &config,
        )
        .expect("parse");
        assert_eq!(inv.config_summary.origins.get("output"), dx_adopt::DefaultOrigin::Flag);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("output:flag"), "{out}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_legacy_config_warns_with_migration_hint() {
        let (scratch, root) = pinned_root("dx-adopt-status-legacy-");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("legacy");
        let inv = invocation_with_config(&["status"], &root);
        assert_eq!(inv.config_summary.origins.get("output"), dx_adopt::DefaultOrigin::Legacy);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "legacy still resolves");
        assert!(out.contains("config: warn"), "{out}");
        assert!(out.contains("files: config.toml"), "{out}");
        assert!(out.contains("output:legacy"), "{out}");
        assert!(out.contains("dx status --migrate-config --apply"), "{out}");
        let inv = invocation_with_config(&["status", "--output=json"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let config = events
            .iter()
            .find(|event| {
                event["event"] == serde_json::json!("status")
                    && event["name"] == serde_json::json!("config")
            })
            .expect("config status event");
        assert_eq!(config["status"], serde_json::json!("warn"));
        assert!(config["detail"].as_str().unwrap_or("").contains("output:legacy"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_legacy_conflict_fails_closed_until_migrated() {
        let (scratch, root) = pinned_root("dx-adopt-status-conflict-");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("legacy");
        std::fs::write(root.join("dx.toml"), "[dx]\nverbose = true\n").expect("committed");
        let config = dx_adopt::load_consumer_config(&root, false).expect("loads");
        assert!(config.conflict.is_some());
        let error = crate::args::parse_with_config(
            &crate::test_support::strings(&["status"]),
            &|_| None,
            &config,
        )
        .expect_err("conflict fails");
        let rendered = error.to_string();
        assert!(rendered.contains("config.toml"), "{rendered}");
        assert!(rendered.contains("dx.toml"), "{rendered}");
        assert!(rendered.contains("dx status --migrate-config"), "{rendered}");
        let error = crate::args::parse_with_config(
            &crate::test_support::strings(&["lint", "--check", "//..."]),
            &|_| None,
            &config,
        )
        .expect_err("every command fails, not just status");
        assert!(error.to_string().contains("config.toml"));
        crate::args::parse_with_config(
            &crate::test_support::strings(&["status", "--migrate-config"]),
            &|_| None,
            &config,
        )
        .expect("migration bypasses the conflict");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_migrate_config_plans_without_writing_then_applies() {
        let (scratch, root) = pinned_root("dx-adopt-status-migrate-");
        std::fs::write(
            root.join(".dx/config.toml"),
            "[dx]\noutput = \"json\"\nverbose = true\n",
        )
        .expect("legacy");
        let inv = invocation_with_config(&["status", "--migrate-config"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("would write"), "{out}");
        assert!(out.contains("dx.toml"), "{out}");
        assert!(out.contains("would remove"), "{out}");
        assert!(out.contains("config.toml"), "{out}");
        assert!(!root.join("dx.toml").exists(), "check writes nothing");
        assert!(root.join(".dx/config.toml").exists(), "legacy stays");
        let inv = invocation_with_config(&["status", "--migrate-config", "--apply"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("wrote"), "{out}");
        assert!(out.contains("removed"), "{out}");
        assert!(!root.join(".dx/config.toml").exists(), "legacy removed");
        let body = std::fs::read_to_string(root.join("dx.toml")).expect("committed written");
        assert!(body.contains("output = \"json\""), "{body}");
        assert!(body.contains("verbose = true"), "{body}");
        let inv = invocation_with_config(&["status"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("config: ok"), "{out}");
        assert!(out.contains("output:committed"), "{out}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_migrate_config_reports_nothing_without_legacy() {
        let (scratch, root) = pinned_root("dx-adopt-status-nomigrate-");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        let inv = invocation_with_config(&["status", "--migrate-config"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("no legacy config to migrate"), "{out}");
        let inv = invocation_with_config(&["status", "--migrate-config", "--apply"], &root);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("no legacy config to migrate"), "{out}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn status_migrate_config_json_streams_notices() {
        let (scratch, root) = pinned_root("dx-adopt-status-migrate-json-");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("legacy");
        let inv = invocation_with_config(
            &["status", "--migrate-config", "--output=json"],
            &root,
        );
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let notice = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("notice"))
            .expect("migration notice");
        assert_eq!(notice["code"], serde_json::json!("config_migration_planned"));
        assert!(root.join(".dx/config.toml").exists(), "plan writes nothing");
        let inv = invocation_with_config(
            &["status", "--migrate-config", "--apply", "--output=json"],
            &root,
        );
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let events = json_events(&out);
        let notice = events
            .iter()
            .find(|event| event["event"] == serde_json::json!("notice"))
            .expect("migrated notice");
        assert_eq!(notice["code"], serde_json::json!("config_migrated"));
        assert!(!root.join(".dx/config.toml").exists(), "apply removes legacy");
        scratch.close().expect("cleanup");
    }
}
