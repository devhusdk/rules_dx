use std::io::Write;

use crate::args::command::{LabelsPolicy, SkewKind, COMMANDS};
use crate::args::Invocation;
use crate::exec::common::{check_stdout_write, emit_event, emit_started, operational};
use dx_output::{
    capability_event, command_finished, command_started, CapabilityEvent, FinishedCounts,
    OutputMode,
};

use super::summaries_suppressed;

pub(crate) const CODE_WORKSPACE_UNRESOLVED: &str = "workspace_unresolved";

fn skew_name(skew: SkewKind) -> &'static str {
    match skew {
        SkewKind::Proceed => "proceed",
        SkewKind::Warn => "warn",
        SkewKind::Refuse => "refuse",
    }
}

fn labels_name(labels: LabelsPolicy) -> &'static str {
    match labels {
        LabelsPolicy::Always => "always",
        LabelsPolicy::Never => "never",
        LabelsPolicy::OnlyEmpty => "only-empty",
        LabelsPolicy::OnlyNonEmpty => "only-non-empty",
        LabelsPolicy::FewerThanTwo => "fewer-than-two",
    }
}

fn cli_record() -> CapabilityEvent {
    let names: Vec<serde_json::Value> = COMMANDS
        .iter()
        .map(|meta| serde_json::Value::String(meta.name.to_owned()))
        .collect();
    let events: Vec<serde_json::Value> = dx_output::EVENTS
        .iter()
        .map(|event| serde_json::Value::String((*event).to_owned()))
        .collect();
    let schema = dx_output::schema();
    CapabilityEvent {
        kind: "cli".to_owned(),
        name: "dx".to_owned(),
        state: "available".to_owned(),
        detail: format!(
            "dx {} rules_dx {} with {} commands",
            dx_adopt::DX_VERSION,
            dx_adopt::MODULE_VERSION,
            COMMANDS.len()
        ),
        data: serde_json::json!({
            "dx_version": dx_adopt::DX_VERSION,
            "module_version": dx_adopt::MODULE_VERSION,
            "schema": schema,
            "commands": names,
            "events": events,
        }),
    }
}

fn command_record(command: crate::args::Command) -> CapabilityEvent {
    let meta = command.meta();
    let flags: Vec<serde_json::Value> = crate::args::grammar::advertised_flags(command)
        .into_iter()
        .map(serde_json::Value::String)
        .collect();
    let reports: Vec<serde_json::Value> =
        crate::reports::format_names(crate::plan::spec(command).reports)
            .into_iter()
            .map(|name| serde_json::Value::String(name.to_owned()))
            .collect();
    let authorization = if command.supports_apply() {
        "--apply"
    } else {
        "none"
    };
    CapabilityEvent {
        kind: "command".to_owned(),
        name: command.name().to_owned(),
        state: "available".to_owned(),
        detail: meta.describe.to_owned(),
        data: serde_json::json!({
            "scope_policy": meta.scope_policy,
            "flags": flags,
            "outputs": crate::args::help::output_modes(command),
            "reports": reports,
            "supports_here": meta.supports_here,
            "labels": labels_name(meta.labels),
            "skew": skew_name(meta.skew),
            "mutating_by_default": meta.is_mutating_by_default,
            "authorization": authorization,
        }),
    }
}

fn cli_records() -> Vec<CapabilityEvent> {
    use clap::ValueEnum;
    let mut records = Vec::with_capacity(1 + crate::args::Command::value_variants().len());
    records.push(cli_record());
    for command in crate::args::Command::value_variants() {
        records.push(command_record(*command));
    }
    records
}

fn module_record(workspace: &std::path::Path) -> Result<CapabilityEvent, String> {
    let path = workspace.join("MODULE.bazel");
    let text = std::fs::read_to_string(&path).map_err(|_| {
        format!(
            "no MODULE.bazel under {}: dx capabilities --workspace-capabilities needs a workspace (pass --workspace <dir>)",
            workspace.display()
        )
    })?;
    match parse_module_header(&text) {
        Some((name, version)) => Ok(CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "module".to_owned(),
            state: "available".to_owned(),
            detail: format!("module {name} {version}"),
            data: serde_json::json!({"path": "MODULE.bazel", "name": name, "version": version}),
        }),
        None => Ok(CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "module".to_owned(),
            state: "unresolved".to_owned(),
            detail: "MODULE.bazel names no module(name, version) header".to_owned(),
            data: serde_json::json!({"path": "MODULE.bazel"}),
        }),
    }
}

fn parse_module_header(text: &str) -> Option<(String, String)> {
    let start = text.find("module(")?;
    let end = text[start..].find(')')?;
    let header = &text[start..start + end];
    Some((attribute(header, "name")?, attribute(header, "version")?))
}

fn attribute(header: &str, key: &str) -> Option<String> {
    let start = header.find(key)?;
    let rest = header[start + key.len()..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn pin_record(workspace: &std::path::Path) -> CapabilityEvent {
    match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "pin".to_owned(),
            state: "available".to_owned(),
            detail: format!("pin {pin}"),
            data: serde_json::json!({
                "path": ".dx/version",
                "pin": pin,
                "matches_module": dx_adopt::version_pin_matches_module(&pin, dx_adopt::MODULE_VERSION),
            }),
        },
        Err(_) => CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "pin".to_owned(),
            state: "unresolved".to_owned(),
            detail: "no readable .dx/version pin".to_owned(),
            data: serde_json::json!({"path": ".dx/version"}),
        },
    }
}

fn config_record(workspace: &std::path::Path) -> CapabilityEvent {
    let present = workspace.join(".dx/config.toml").is_file();
    let detail = if present {
        "workspace defaults file present"
    } else {
        "no workspace defaults file"
    };
    CapabilityEvent {
        kind: "workspace".to_owned(),
        name: "config".to_owned(),
        state: "available".to_owned(),
        detail: detail.to_owned(),
        data: serde_json::json!({"path": ".dx/config.toml", "present": present}),
    }
}

fn boundary_records() -> Vec<CapabilityEvent> {
    vec![
        CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "tool-availability".to_owned(),
            state: "unresolved".to_owned(),
            detail: "tool availability needs Bazel analysis and acquisition; this inventory is declared support only".to_owned(),
            data: serde_json::json!({
                "needs": "bazel analysis",
                "observed_via": "dx status",
            }),
        },
        CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "policy-selection".to_owned(),
            state: "unresolved".to_owned(),
            detail: "the selected quality policy resolves in Bazel configuration, not on disk".to_owned(),
            data: serde_json::json!({
                "needs": "bazel analysis",
                "observed_via": "dx status",
            }),
        },
    ]
}

fn workspace_records(workspace: &std::path::Path) -> Result<Vec<CapabilityEvent>, String> {
    let module = module_record(workspace)?;
    let display = workspace.display().to_string();
    let mut records = vec![
        CapabilityEvent {
            kind: "workspace".to_owned(),
            name: "workspace".to_owned(),
            state: "available".to_owned(),
            detail: format!("workspace {display}"),
            data: serde_json::json!({"path": display}),
        },
        module,
        pin_record(workspace),
        config_record(workspace),
    ];
    records.extend(boundary_records());
    Ok(records)
}

fn text_line(record: &CapabilityEvent) -> String {
    match record.kind.as_str() {
        "cli" => record.detail.clone(),
        "command" => {
            let data = &record.data;
            let flags = data["flags"]
                .as_array()
                .map(|flags| {
                    flags
                        .iter()
                        .filter_map(|flag| flag.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let reports = data["reports"]
                .as_array()
                .map(|reports| {
                    reports
                        .iter()
                        .filter_map(|report| report.as_str())
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .filter(|reports| !reports.is_empty())
                .unwrap_or_else(|| "none".to_owned());
            let effect = if data["mutating_by_default"].as_bool().unwrap_or(false) {
                "mutating by default"
            } else {
                "read-only by default"
            };
            format!(
                "command {}: {}; flags [{}]; outputs {}; reports {}; {}; authorization {}",
                record.name,
                data["scope_policy"].as_str().unwrap_or("reject"),
                flags,
                data["outputs"].as_str().unwrap_or("text|json"),
                reports,
                effect,
                data["authorization"].as_str().unwrap_or("none")
            )
        }
        _ => format!("{} {}: {}", record.state, record.name, record.detail),
    }
}

fn json_dry_run(invocation: &Invocation, out: &mut dyn Write) -> i32 {
    if let Ok(event) = command_started(invocation.command.name(), true, "default") {
        if let Err(exit) = emit_event(out, &event) {
            return exit;
        }
    }
    if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
        return exit;
    }
    0
}

pub(crate) fn execute_capabilities(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if invocation.dry_run {
        if invocation.output == OutputMode::Json {
            return json_dry_run(invocation, out);
        }
        if !summaries_suppressed(invocation) {
            if invocation.workspace_capabilities {
                if let Err(exit) =
                    check_stdout_write(writeln!(out, "would report workspace capabilities"))
                {
                    return exit;
                }
            } else if let Err(exit) = check_stdout_write(writeln!(out, "would report capabilities"))
            {
                return exit;
            }
        }
        return 0;
    }
    let mut records = cli_records();
    if invocation.workspace_capabilities {
        match workspace_records(workspace) {
            Ok(staged) => records.extend(staged),
            Err(message) => {
                if invocation.output == OutputMode::Json {
                    if let Err(exit) = emit_started(invocation, out) {
                        return exit;
                    }
                }
                return operational(invocation, out, err, CODE_WORKSPACE_UNRESOLVED, &message);
            }
        }
    }
    if invocation.output == OutputMode::Json {
        if let Ok(event) = command_started(invocation.command.name(), false, "default") {
            if let Err(exit) = emit_event(out, &event) {
                return exit;
            }
        }
        for record in &records {
            if let Ok(event) = capability_event(record) {
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
    for record in &records {
        if let Err(exit) = check_stdout_write(writeln!(out, "{}", text_line(record))) {
            return exit;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, invocation, json_events, run};
    use crate::resolve::{QueryResult, QueryRunner};
    use std::cell::Cell;
    use std::io;

    struct CountingRunner {
        runs: Cell<usize>,
    }

    impl dx_process::Runner for CountingRunner {
        fn run(
            &self,
            _argv: &[String],
            _cwd: &std::path::Path,
            _env: &[(&str, &str)],
        ) -> io::Result<dx_process::ChildStatus> {
            self.runs.set(self.runs.get() + 1);
            Ok(dx_process::ChildStatus { code: Some(0) })
        }
    }

    struct CountingQuery {
        runs: Cell<usize>,
    }

    impl QueryRunner for CountingQuery {
        fn run_query(&self, _argv: &[String], _cwd: &std::path::Path) -> io::Result<QueryResult> {
            self.runs.set(self.runs.get() + 1);
            Err(io::Error::other("no queries"))
        }
    }

    fn events(out: &str) -> Vec<serde_json::Value> {
        json_events(out)
    }

    fn capabilities(out: &str) -> Vec<serde_json::Value> {
        events(out)
            .into_iter()
            .filter(|event| event["event"] == serde_json::json!("capability"))
            .collect()
    }

    fn by_name<'a>(
        records: &'a [serde_json::Value],
        kind: &str,
        name: &str,
    ) -> &'a serde_json::Value {
        records
            .iter()
            .find(|record| record["kind"] == kind && record["name"] == name)
            .unwrap_or_else(|| panic!("no {kind}/{name} record"))
    }

    #[test]
    fn cli_text_lists_every_command_without_probes() {
        let runner = CountingRunner { runs: Cell::new(0) };
        let query = CountingQuery { runs: Cell::new(0) };
        let scratch = dx_test_scratch::scratch("dx-capabilities-text-");
        let inv = invocation(&["capabilities"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::super::execute_adoption(
            &inv,
            crate::adopt::test_support::env_with(
                scratch.path(),
                &query,
                &runner,
                &mut out,
                &mut err,
            ),
        );
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("stdout");
        assert!(err.is_empty(), "{err:?}");
        use clap::ValueEnum;
        for command in crate::args::Command::value_variants() {
            assert!(
                text.contains(&format!("\ncommand {}:", command.name()))
                    || text.starts_with(&format!("command {}:", command.name()))
                    || text.contains(&format!("command {}:", command.name())),
                "text never names dx {}:\n{text}",
                command.name()
            );
        }
        assert_eq!(runner.runs.get(), 0, "the CLI stage launches nothing");
        assert_eq!(query.runs.get(), 0, "the CLI stage queries nothing");
    }

    #[test]
    fn cli_json_streams_one_capability_event_per_command() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-json-");
        let inv = invocation(&["capabilities", "--output=json"]);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = super::super::execute_adoption(
            &inv,
            crate::adopt::test_support::env(scratch.path(), &mut out, &mut err),
        );
        assert_eq!(code, 0);
        let text = String::from_utf8(out).expect("stdout");
        let streamed = events(&text);
        let kinds = event_kinds(&streamed);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let records = capabilities(&text);
        use clap::ValueEnum;
        assert_eq!(
            records.len(),
            1 + crate::args::Command::value_variants().len(),
            "one cli record plus one record per command"
        );
        for event in &streamed {
            assert!(event.get("schema").is_some(), "{event}");
            assert!(event.get("event").is_some(), "{event}");
        }
        let cli = by_name(&records, "cli", "dx");
        assert_eq!(cli["state"], serde_json::json!("available"));
        assert_eq!(cli["data"]["schema"], dx_output::schema());
        let mut names: Vec<&str> = records
            .iter()
            .filter(|record| record["kind"] == "command")
            .map(|record| record["name"].as_str().expect("name"))
            .collect();
        names.sort_unstable();
        let mut wanted: Vec<&str> = crate::args::Command::value_variants()
            .iter()
            .map(|command| command.name())
            .collect();
        wanted.sort_unstable();
        assert_eq!(names, wanted);
        for command in crate::args::Command::value_variants() {
            let record = by_name(&records, "command", command.name());
            assert_eq!(record["state"], serde_json::json!("available"));
            let mut flags: Vec<&str> = record["data"]["flags"]
                .as_array()
                .expect("flags")
                .iter()
                .map(|flag| flag.as_str().expect("flag"))
                .collect();
            flags.sort_unstable();
            let mut derived: Vec<String> = crate::args::grammar::advertised_flags(*command);
            derived.sort();
            let derived: Vec<&str> = derived.iter().map(String::as_str).collect();
            assert_eq!(flags, derived, "dx {} flags stay derived", command.name());
        }
        let lint = by_name(&records, "command", "lint");
        assert_eq!(lint["data"]["outputs"], serde_json::json!("text|diff|json"));
        assert_eq!(lint["data"]["reports"], serde_json::json!(["sarif"]));
        let own = by_name(&records, "command", "capabilities");
        assert_eq!(own["data"]["outputs"], serde_json::json!("text|json"));
        assert_eq!(own["data"]["reports"], serde_json::json!([]));
        assert_eq!(own["data"]["scope_policy"], serde_json::json!("reject"));
        assert_eq!(own["data"]["authorization"], serde_json::json!("none"));
        let flags: Vec<&str> = own["data"]["flags"]
            .as_array()
            .expect("flags")
            .iter()
            .map(|flag| flag.as_str().expect("flag"))
            .collect();
        assert!(flags.contains(&"workspace-capabilities"), "{flags:?}");
        assert!(err.is_empty(), "{err:?}");
    }

    #[test]
    fn capability_documents_survive_an_old_reader_and_unknown_fields() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-compat-");
        let inv = invocation(&["capabilities", "--output=json"]);
        let (code, out, _err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        for event in capabilities(&out) {
            assert_eq!(event["schema"], dx_output::schema());
            let major = event["schema"]["major"].as_u64().expect("major") as u32;
            assert_eq!(
                dx_schema::check_major(major),
                Ok(()),
                "the emitted schema stays readable"
            );
            assert!(dx_schema::check_major(major + 1).is_err());
            let mut with_unknown = event.clone();
            with_unknown["future_field"] = serde_json::json!("ignored");
            let reparsed: serde_json::Value =
                serde_json::from_value(with_unknown).expect("unknown fields parse");
            assert_eq!(reparsed["name"], event["name"]);
        }
    }

    #[test]
    fn contradictory_combinations_fail_before_execution() {
        use crate::args::{parse, ArgsError};
        use crate::test_support::strings;
        for words in [
            vec!["capabilities", "--check"],
            vec!["capabilities", "--apply"],
            vec!["capabilities", "--output=diff"],
            vec!["capabilities", "--fail-on=error"],
            vec!["capabilities", "--report=sarif=out.sarif"],
            vec!["capabilities", "--", "--jobs=1"],
        ] {
            assert!(
                matches!(
                    parse(&strings(&words)),
                    Err(ArgsError::UnsupportedOption { .. })
                ),
                "{words:?} must fail as unsupported"
            );
        }
        assert!(
            matches!(
                parse(&strings(&["capabilities", "//:demo"])),
                Err(ArgsError::UnsupportedOption { .. })
            ),
            "scopes must fail as unsupported"
        );
        let got = parse(&strings(&["capabilities", "--workspace-capabilities"]))
            .expect("the workspace stage parses");
        assert!(got.workspace_capabilities);
        let bare = parse(&strings(&["capabilities"])).expect("bare parses");
        assert!(!bare.workspace_capabilities);
    }

    fn renamed_workspace(name: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch("dx-capabilities-ws-");
        std::fs::write(
            scratch.path().join("MODULE.bazel"),
            format!("module(name = \"{name}\", version = \"0.0.0\")\n"),
        )
        .expect("module");
        scratch
    }

    #[test]
    fn workspace_stage_reports_selected_facts_from_local_records() {
        let scratch = renamed_workspace("sample");
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
        std::fs::write(scratch.path().join(".dx/version"), "0.0.0\n").expect("pin");
        std::fs::write(
            scratch.path().join(".dx/config.toml"),
            "[dx]\noutput = \"json\"\n",
        )
        .expect("config");
        let inv = invocation(&["capabilities", "--workspace-capabilities", "--output=json"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        let records = capabilities(&out);
        let module = by_name(&records, "workspace", "module");
        assert_eq!(module["state"], serde_json::json!("available"));
        assert_eq!(module["data"]["name"], serde_json::json!("sample"));
        let pin = by_name(&records, "workspace", "pin");
        assert_eq!(pin["data"]["matches_module"], serde_json::json!(true));
        let config = by_name(&records, "workspace", "config");
        assert_eq!(config["data"]["present"], serde_json::json!(true));
        for name in ["tool-availability", "policy-selection"] {
            let boundary = by_name(&records, "workspace", name);
            assert_eq!(boundary["state"], serde_json::json!("unresolved"));
        }
        for record in &records {
            assert!(
                ["available", "declared", "unresolved"]
                    .contains(&record["state"].as_str().expect("state")),
                "{record}"
            );
        }
    }

    #[test]
    fn workspace_stage_outside_a_workspace_fails_with_the_boundary() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-nowhere-");
        for json in [false, true] {
            let words = if json {
                vec!["capabilities", "--workspace-capabilities", "--output=json"]
            } else {
                vec!["capabilities", "--workspace-capabilities"]
            };
            let inv = invocation(&words);
            let (code, out, err) = run(&inv, scratch.path());
            assert_eq!(code, 1, "{words:?}");
            assert!(err.contains(CODE_WORKSPACE_UNRESOLVED), "{words:?}: {err}");
            assert!(err.contains("--workspace-capabilities"), "{words:?}: {err}");
            if json {
                let streamed = events(&out);
                let kinds = event_kinds(&streamed);
                assert_eq!(
                    kinds,
                    vec!["command_started", "error", "command_finished"],
                    "{words:?}"
                );
                assert_eq!(
                    streamed[1]["code"],
                    serde_json::json!(CODE_WORKSPACE_UNRESOLVED)
                );
            } else {
                assert!(out.is_empty(), "{words:?}: {out}");
            }
        }
    }

    #[test]
    fn workspace_stage_marks_gaps_explicit_and_stays_zero() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-gaps-");
        std::fs::write(scratch.path().join("MODULE.bazel"), "# no module header\n")
            .expect("module");
        let inv = invocation(&["capabilities", "--workspace-capabilities"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("unresolved module"), "{out}");
        let scratch = dx_test_scratch::scratch("dx-capabilities-nopin-");
        std::fs::write(
            scratch.path().join("MODULE.bazel"),
            "module(name = \"sample\", version = \"0.0.0\")\n",
        )
        .expect("module");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("unresolved pin"), "{out}");
        assert!(out.contains("available config"), "{out}");
    }

    #[test]
    fn workspace_stage_never_prints_config_contents() {
        let scratch = renamed_workspace("sample");
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
        std::fs::write(scratch.path().join(".dx/version"), "0.0.0\n").expect("pin");
        std::fs::write(
            scratch.path().join(".dx/config.toml"),
            "[dx]\nworkspace = \"/top-secret-path\"\n",
        )
        .expect("config");
        let inv = invocation(&["capabilities", "--workspace-capabilities", "--output=json"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0, "{err}");
        assert!(!out.contains("top-secret-path"), "{out}");
        assert!(!err.contains("top-secret-path"), "{err}");
    }

    #[test]
    fn unknown_config_key_fails_closed_naming_the_key() {
        let rendered = dx_adopt::defaults::parse_file_text("[dx]\npassword = \"s3cret\"\n")
            .expect_err("unknown keys fail");
        let text = rendered.to_string();
        assert!(text.contains("password"), "{text}");
    }

    #[test]
    fn dry_run_plans_without_reading_the_workspace() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-dry-");
        for (words, want) in [
            (
                vec!["capabilities", "--dry-run"],
                "would report capabilities",
            ),
            (
                vec!["capabilities", "--workspace-capabilities", "--dry-run"],
                "would report workspace capabilities",
            ),
        ] {
            let inv = invocation(&words);
            let (code, out, _err) = run(&inv, scratch.path());
            assert_eq!(code, 0, "{words:?}");
            assert!(out.contains(want), "{words:?}: {out}");
        }
        let inv = invocation(&["capabilities", "--dry-run", "--output=json"]);
        let (code, out, _err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        let streamed = events(&out);
        let kinds = event_kinds(&streamed);
        assert_eq!(kinds, vec!["command_started", "command_finished"]);
    }

    #[test]
    fn output_boundaries_propagate_broken_pipes() {
        use crate::adopt::test_support::Truncated;
        let scratch = renamed_workspace("sample");
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
        std::fs::write(scratch.path().join(".dx/version"), "0.0.0\n").expect("pin");
        for words in [
            vec!["capabilities"],
            vec!["capabilities", "--output=json"],
            vec!["capabilities", "--workspace-capabilities"],
            vec!["capabilities", "--workspace-capabilities", "--output=json"],
        ] {
            let inv = invocation(&words);
            let mut baseline = Vec::new();
            let code = super::super::execute_adoption(
                &inv,
                crate::adopt::test_support::env(scratch.path(), &mut baseline, &mut Vec::new()),
            );
            assert_eq!(code, 0, "{words:?}");
            assert!(!baseline.is_empty(), "{words:?}");
            let mut boundaries = vec![0, baseline.len() - 1];
            boundaries.extend(
                baseline
                    .iter()
                    .enumerate()
                    .filter(|(_, byte)| **byte == b'\n')
                    .map(|(index, _)| index + 1)
                    .filter(|offset| *offset < baseline.len()),
            );
            boundaries.sort_unstable();
            boundaries.dedup();
            for remaining in boundaries {
                let mut err = Vec::new();
                let code = super::super::execute_adoption(
                    &inv,
                    crate::adopt::test_support::env(
                        scratch.path(),
                        &mut Truncated::after_bytes(remaining),
                        &mut err,
                    ),
                );
                assert_eq!(code, 141, "{words:?} after {remaining} bytes");
            }
        }
    }

    #[test]
    fn module_header_parses_one_line_and_multiline_forms() {
        assert_eq!(
            parse_module_header("module(name = \"sample\", version = \"0.0.0\")\n"),
            Some(("sample".to_owned(), "0.0.0".to_owned()))
        );
        assert_eq!(
            parse_module_header("module(\n    name = \"other\",\n    version = \"1.2.3\",\n)\n"),
            Some(("other".to_owned(), "1.2.3".to_owned()))
        );
        assert_eq!(parse_module_header("# no header\n"), None);
        assert_eq!(parse_module_header("module(name = \"half\")\n"), None);
    }

    #[test]
    fn quiet_keeps_results_but_plans_silently() {
        let scratch = dx_test_scratch::scratch("dx-capabilities-quiet-");
        let inv = invocation(&["capabilities", "--quiet"]);
        let (code, out, _err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(!out.is_empty());
        let inv = invocation(&["capabilities", "--dry-run", "--quiet"]);
        let (code, out, _err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(out.is_empty());
    }
}
