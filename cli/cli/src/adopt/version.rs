use std::io::Write;

use crate::exec::common::{check_stdout_write, emit_built, emit_event, emit_started};
use dx_output::{
    command_finished, command_started, status_event, FinishedCounts, OutputMode, StatusEvent,
};
use dx_plan::args::Invocation;

use super::operational;
use super::status::CODE_STATUS_PIN_MISMATCH;

fn json_status(
    out: &mut dyn Write,
    name: &str,
    status: &str,
    detail: &str,
    hint: &str,
) -> Result<(), i32> {
    emit_built(
        out,
        status_event(&StatusEvent {
            name: name.to_owned(),
            status: status.to_owned(),
            detail: detail.to_owned(),
            hint: hint.to_owned(),
        }),
    )?;
    Ok(())
}

pub(crate) fn execute_version(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let is_json = invocation.output == OutputMode::Json;
    let current = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            let message = error.to_string();
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
            }
            return operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message);
        }
    };
    if invocation.check {
        let detail = format!("dx {current} vs module {}", dx_adopt::MODULE_VERSION);
        let hint = dx_adopt::pin_fix_hint(&current, dx_adopt::MODULE_VERSION);
        if dx_adopt::version_pin_matches_module(&current, dx_adopt::MODULE_VERSION) {
            if is_json {
                if let Err(exit) =
                    emit_built(out, command_started(invocation.command.name(), "default"))
                {
                    return exit;
                }
                if let Err(exit) = json_status(out, "pin", "ok", &detail, &hint) {
                    return exit;
                }
                if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default()))
                {
                    return exit;
                }
                return 0;
            }
            if let Err(exit) = check_stdout_write(writeln!(out, "version ok: {current}")) {
                return exit;
            }
            0
        } else {
            let message = format!("version drift: {current} != {}", dx_adopt::MODULE_VERSION);
            if is_json {
                if let Err(exit) = emit_started(invocation, out) {
                    return exit;
                }
                if let Err(exit) = json_status(out, "pin", "error", &detail, &hint) {
                    return exit;
                }
            }
            operational(invocation, out, err, CODE_STATUS_PIN_MISMATCH, &message)
        }
    } else {
        let (dependency, dependency_status) = match dx_adopt::read_module_dependency(workspace) {
            Ok(found) => (
                dx_adopt::render_module_dependency(&found),
                dx_adopt::module_dependency_status(&found),
            ),
            Err(error) => (format!("unknown ({error})"), "warn"),
        };
        if is_json {
            if let Err(exit) =
                emit_built(out, command_started(invocation.command.name(), "default"))
            {
                return exit;
            }
            if let Err(exit) = json_status(out, "binary", "ok", dx_adopt::DX_VERSION, "dx version")
            {
                return exit;
            }
            if let Err(exit) =
                json_status(out, "module", "ok", dx_adopt::MODULE_VERSION, "dx version")
            {
                return exit;
            }
            let pin_status =
                if dx_adopt::version_pin_matches_module(&current, dx_adopt::MODULE_VERSION) {
                    "ok"
                } else {
                    "warn"
                };
            let pin_hint = dx_adopt::pin_fix_hint(&current, dx_adopt::MODULE_VERSION);
            if let Err(exit) = json_status(out, "pin", pin_status, &current, &pin_hint) {
                return exit;
            }
            let dependency_hint = if dependency_status == "ok" {
                "dx version"
            } else {
                "dx init"
            };
            if let Err(exit) = json_status(
                out,
                "dependency",
                dependency_status,
                &dependency,
                dependency_hint,
            ) {
                return exit;
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "dx {}", dx_adopt::DX_VERSION)) {
            return exit;
        }
        if let Err(exit) =
            check_stdout_write(writeln!(out, "rules_dx {}", dx_adopt::MODULE_VERSION))
        {
            return exit;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "pin {current}")) {
            return exit;
        }
        if let Err(exit) = check_stdout_write(writeln!(out, "dependency {dependency}")) {
            return exit;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, events_of_kind, invocation, json_events, run};

    #[test]
    fn version_check_reports_drift() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-check-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let inv = invocation(&["version", "--check"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        assert!(out.contains("version ok"));
        std::fs::write(root.join(".dx/version"), "0.1.0\n").expect("drift");
        let (code, _out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        assert!(err.contains("drift"));
    }

    #[test]
    fn version_missing_pin_fails_closed() {
        for words in [vec!["version", "--check"], vec!["version"]] {
            let scratch = dx_test_scratch::scratch("dx-adopt-version-missing-");
            let root = scratch.path().to_path_buf();
            let inv = invocation(&words);
            let (code, out, err) = run(&inv, &root);
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("read version pin"), "words: {words:?}");
            assert!(!out.contains("version ok"), "words: {words:?}");
        }
    }

    #[test]
    fn version_json_streams_status_envelope() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n",
        )
        .expect("module");
        let inv = invocation(&["version", "--output=json"]);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        assert!(kinds.contains(&"status"), "{kinds:?}");
        assert!(!kinds.contains(&"error"), "{kinds:?}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let names: Vec<&str> = events_of_kind(&events, "status")
            .into_iter()
            .map(|event| event["name"].as_str().expect("name"))
            .collect();
        assert_eq!(
            names,
            vec!["binary", "module", "pin", "dependency"],
            "{names:?}"
        );
        for event in events_of_kind(&events, "status") {
            assert_eq!(event["status"], serde_json::json!("ok"), "{event}");
        }
        assert!(err.is_empty());
    }

    fn status_named<'a>(events: &'a [serde_json::Value], name: &str) -> &'a serde_json::Value {
        events_of_kind(events, "status")
            .into_iter()
            .find(|event| event["name"] == name)
            .expect("status event")
    }

    #[test]
    fn version_json_warns_on_drift_and_unreadable_module() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-json-drift-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("pin");
        std::fs::create_dir_all(root.join("MODULE.bazel")).expect("module dir");
        let inv = invocation(&["version", "--output=json"]);
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        assert!(!event_kinds(&events).contains(&"error"), "{out}");
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let pin = status_named(&events, "pin");
        assert_eq!(pin["status"], serde_json::json!("warn"), "{pin}");
        assert_eq!(pin["detail"], serde_json::json!("9.9.9"), "{pin}");
        let dependency = status_named(&events, "dependency");
        assert_eq!(
            dependency["status"],
            serde_json::json!("warn"),
            "{dependency}"
        );
        assert!(
            dependency["detail"]
                .as_str()
                .expect("detail")
                .starts_with("unknown (read MODULE.bazel"),
            "{dependency}"
        );
        assert!(err.is_empty(), "{err}");
        std::fs::remove_dir_all(root.join("MODULE.bazel")).expect("rm module dir");
        std::fs::write(root.join("MODULE.bazel"), "module(name = \"demo\")\n").expect("module");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let dependency = status_named(&events, "dependency");
        assert_eq!(
            dependency["status"],
            serde_json::json!("warn"),
            "{dependency}"
        );
        assert_eq!(
            dependency["detail"],
            serde_json::json!("unknown (no rules_dx dependency in MODULE.bazel)")
        );
    }

    #[test]
    fn version_reports_the_module_dependency() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-dependency-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n",
        )
        .expect("module");
        let inv = invocation(&["version"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("dependency rules_dx 0.0.0"), "{out}");
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"../rules_dx\")\n",
        )
        .expect("override");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("dependency rules_dx 0.0.0 via local_path_override ../rules_dx"),
            "{out}"
        );
        std::fs::write(
            root.join("MODULE.bazel"),
            "# bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\nbazel_dep(\n    repo_name = 'dx',\n    version = '0.0.0',  # ) (\n    name = 'rules_dx',\n)\n",
        )
        .expect("multiline");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("dependency rules_dx 0.0.0\n"), "{out}");
        std::fs::remove_file(root.join("MODULE.bazel")).expect("remove module");
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("dependency unknown"), "{out}");
    }

    #[test]
    fn version_check_json_reports_drift_with_error() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-check-json-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/version"), "0.0.0\n").expect("pin");
        let inv = invocation(&["version", "--check", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 0);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "status", "command_finished"],
            "{kinds:?}"
        );
        std::fs::write(root.join(".dx/version"), "9.9.9\n").expect("drift");
        let (code, out, err) = run(&inv, &root);
        assert_eq!(code, 1);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "status", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
        assert_eq!(
            events[2]["code"],
            serde_json::json!(CODE_STATUS_PIN_MISMATCH)
        );
        assert!(err.contains("drift"));
    }

    #[test]
    fn version_missing_pin_json_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-adopt-version-missing-json-");
        let root = scratch.path().to_path_buf();
        let inv = invocation(&["version", "--output=json"]);
        let (code, out, _err) = run(&inv, &root);
        assert_eq!(code, 1);
        let text = out;
        let events = json_events(&text);
        let kinds = event_kinds(&events);
        assert_eq!(
            kinds,
            vec!["command_started", "error", "command_finished"],
            "{kinds:?}"
        );
        assert_eq!(
            events[1]["code"],
            serde_json::json!(CODE_STATUS_PIN_MISMATCH)
        );
    }
}
