use std::io::Write;

use clap::ValueEnum;

use crate::args::help::advertises_passthrough;
use crate::args::{Command, Invocation};
use crate::exec::common::{check_stdout_write, emit_event, emit_started};
use dx_output::{
    capability_event, command_finished, command_started, CapabilityEvent, FinishedCounts,
    OutputMode,
};

use super::{operational, pre_exec, summaries_suppressed};

pub(crate) const CODE_BROKEN_WORKSPACE: &str = "broken_workspace";

fn effect_of(command: Command) -> &'static str {
    if command.is_mutating_by_default() {
        "mutating"
    } else if command.supports_apply() {
        "apply"
    } else {
        "none"
    }
}

fn output_names(command: Command) -> Vec<String> {
    let mut modes = vec!["text".to_owned()];
    if command.supports_diff() {
        modes.push("diff".to_owned());
    }
    if command.supports_json() {
        modes.push("json".to_owned());
    }
    modes
}

fn command_capability(command: Command) -> CapabilityEvent {
    let spec = crate::plan::spec(command);
    CapabilityEvent {
        name: command.name().to_owned(),
        source: "cli-grammar".to_owned(),
        availability: "available".to_owned(),
        detail: command.describe().to_owned(),
        flags: crate::args::grammar::advertised_flags(command)
            .into_iter()
            .map(|flag| format!("--{flag}"))
            .collect(),
        outputs: output_names(command),
        reports: crate::reports::format_names(spec.reports)
            .into_iter()
            .map(ToOwned::to_owned)
            .collect(),
        scope_policy: command.scope_policy().to_owned(),
        effect: effect_of(command).to_owned(),
        passthrough: advertises_passthrough(command),
    }
}

fn workspace_capabilities(workspace: &std::path::Path) -> Vec<CapabilityEvent> {
    let pin = match dx_adopt::read_version_pin(workspace) {
        Ok(pin) => pin,
        Err(error) => {
            return vec![
                CapabilityEvent {
                    name: "workspace:pin".to_owned(),
                    source: "workspace-record".to_owned(),
                    availability: "unavailable".to_owned(),
                    detail: error.to_string(),
                    flags: Vec::new(),
                    outputs: Vec::new(),
                    reports: Vec::new(),
                    scope_policy: "reject".to_owned(),
                    effect: "none".to_owned(),
                    passthrough: false,
                },
                unobserved_tools(),
            ];
        }
    };
    let pin_event = if pin == dx_adopt::MODULE_VERSION {
        CapabilityEvent {
            name: "workspace:pin".to_owned(),
            source: "workspace-record".to_owned(),
            availability: "available".to_owned(),
            detail: format!("pin {pin} matches module {}", dx_adopt::MODULE_VERSION),
            flags: Vec::new(),
            outputs: Vec::new(),
            reports: Vec::new(),
            scope_policy: "reject".to_owned(),
            effect: "none".to_owned(),
            passthrough: false,
        }
    } else if pin.trim().is_empty() {
        CapabilityEvent {
            name: "workspace:pin".to_owned(),
            source: "workspace-record".to_owned(),
            availability: "unavailable".to_owned(),
            detail: "missing pin in .dx/version".to_owned(),
            flags: Vec::new(),
            outputs: Vec::new(),
            reports: Vec::new(),
            scope_policy: "reject".to_owned(),
            effect: "none".to_owned(),
            passthrough: false,
        }
    } else {
        CapabilityEvent {
            name: "workspace:pin".to_owned(),
            source: "workspace-record".to_owned(),
            availability: "unavailable".to_owned(),
            detail: format!("pin {pin} differs from module {}", dx_adopt::MODULE_VERSION),
            flags: Vec::new(),
            outputs: Vec::new(),
            reports: Vec::new(),
            scope_policy: "reject".to_owned(),
            effect: "none".to_owned(),
            passthrough: false,
        }
    };
    vec![pin_event, unobserved_tools()]
}

fn unobserved_tools() -> CapabilityEvent {
    CapabilityEvent {
        name: "workspace:tools".to_owned(),
        source: "unobserved".to_owned(),
        availability: "unknown".to_owned(),
        detail:
            "tool availability is not probed by capabilities; run dx status for observed checks"
                .to_owned(),
        flags: Vec::new(),
        outputs: Vec::new(),
        reports: Vec::new(),
        scope_policy: "reject".to_owned(),
        effect: "none".to_owned(),
        passthrough: false,
    }
}

fn text_lines(capabilities: &[CapabilityEvent]) -> Vec<String> {
    capabilities
        .iter()
        .map(|capability| {
            let flags = if capability.flags.is_empty() {
                "none".to_owned()
            } else {
                capability.flags.join(" ")
            };
            let outputs = if capability.outputs.is_empty() {
                "none".to_owned()
            } else {
                capability.outputs.join("|")
            };
            let reports = if capability.reports.is_empty() {
                "none".to_owned()
            } else {
                capability.reports.join("|")
            };
            format!(
                "capability {}: {} [source: {}; availability: {}; flags: {flags}; output: {outputs}; reports: {reports}; scopes: {}; effect: {}; passthrough: {}]",
                capability.name,
                capability.detail,
                capability.source,
                capability.availability,
                capability.scope_policy,
                capability.effect,
                if capability.passthrough { "yes" } else { "no" },
            )
        })
        .collect()
}

pub(crate) fn execute_capabilities(
    invocation: &Invocation,
    workspace: &std::path::Path,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if !invocation.targets.is_empty() {
        return pre_exec(err, "capabilities takes no scopes");
    }
    let is_json = invocation.output == OutputMode::Json;
    if invocation.dry_run {
        if is_json {
            if let Ok(event) = command_started(invocation.command.name(), true, "default") {
                if let Err(exit) = emit_event(out, &event) {
                    return exit;
                }
            }
            if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
                return exit;
            }
            return 0;
        }
        if !summaries_suppressed(invocation) {
            let line = if invocation.workspace_capabilities {
                "would report capabilities with workspace facts"
            } else {
                "would report capabilities"
            };
            if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
                return exit;
            }
        }
        return 0;
    }
    if invocation.workspace_capabilities && !workspace.join("MODULE.bazel").is_file() {
        if is_json {
            if let Err(exit) = emit_started(invocation, out) {
                return exit;
            }
        }
        return operational(
            invocation,
            out,
            err,
            CODE_BROKEN_WORKSPACE,
            &format!(
                "workspace {} has no MODULE.bazel: --workspace-capabilities needs a workspace checkout",
                workspace.display()
            ),
        );
    }
    let mut capabilities: Vec<CapabilityEvent> = Command::value_variants()
        .iter()
        .map(|command| command_capability(*command))
        .collect();
    if invocation.workspace_capabilities {
        capabilities.extend(workspace_capabilities(workspace));
    }
    if is_json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
        for capability in &capabilities {
            match capability_event(capability) {
                Ok(event) => {
                    if let Err(exit) = emit_event(out, &event) {
                        return exit;
                    }
                }
                Err(error) => {
                    return operational(invocation, out, err, "invalid_result", &error.to_string());
                }
            }
        }
        if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            return exit;
        }
        return 0;
    }
    for line in text_lines(&capabilities) {
        if let Err(exit) = check_stdout_write(writeln!(out, "{line}")) {
            return exit;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adopt::test_support::{event_kinds, events_of_kind, invocation, json_events, run};
    use crate::test_support::strings;
    use std::io;

    fn scratch_workspace(name: &str, pin: Option<&str>) -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch(name);
        std::fs::write(scratch.path().join("MODULE.bazel"), "").expect("module");
        if let Some(pin) = pin {
            std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
            std::fs::write(scratch.path().join(".dx/version"), pin).expect("pin");
        }
        scratch
    }

    #[test]
    fn command_records_derive_from_grammar_and_registry() {
        for command in Command::value_variants() {
            let record = command_capability(*command);
            assert_eq!(record.name, command.name());
            assert_eq!(record.source, "cli-grammar");
            assert_eq!(record.availability, "available");
            assert_eq!(record.detail, command.describe());
            assert_eq!(record.scope_policy, command.scope_policy());
            let mut flags = crate::args::grammar::advertised_flags(*command);
            flags.sort();
            flags.dedup();
            let mut recorded: Vec<String> = record
                .flags
                .iter()
                .map(|flag| flag.trim_start_matches('-').to_owned())
                .collect();
            recorded.sort();
            assert_eq!(recorded, flags, "dx {} flags", command.name());
            let passthrough = crate::args::help::advertises_passthrough(*command);
            assert_eq!(record.passthrough, passthrough, "dx {}", command.name());
            let help = crate::args::help::render_command_help(*command);
            assert!(help.contains(command.describe()), "dx {}", command.name());
            capability_event(&record).expect("valid event");
        }
    }

    #[test]
    fn text_lists_every_command_outside_a_workspace() {
        let inv = invocation(&["capabilities"]);
        let scratch = dx_test_scratch::scratch("dx-cap-text-outside-");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
        for command in Command::value_variants() {
            assert!(
                out.contains(&format!("capability {}:", command.name())),
                "{out}"
            );
        }
        assert!(out.contains("--workspace-capabilities"), "{out}");
        assert!(!out.contains("workspace:pin"), "{out}");
    }

    #[test]
    fn json_streams_one_capability_per_command() {
        let inv = invocation(&["capabilities", "--output=json"]);
        let scratch = dx_test_scratch::scratch("dx-cap-json-outside-");
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let capabilities = events_of_kind(&events, "capability");
        assert_eq!(capabilities.len(), Command::value_variants().len());
        for event in &events {
            assert!(event.get("schema").is_some(), "{event}");
        }
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(0)
        );
        let lint = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("lint"))
            .expect("lint record");
        assert_eq!(lint["source"], serde_json::json!("cli-grammar"));
        assert_eq!(lint["availability"], serde_json::json!("available"));
        let flags = lint["flags"].as_array().expect("flags");
        assert!(flags.contains(&serde_json::json!("--check")), "{lint}");
        assert!(flags.contains(&serde_json::json!("--output")), "{lint}");
    }

    #[test]
    fn workspace_stage_reports_pin_and_marks_tools_unobserved() {
        let scratch = scratch_workspace(
            "dx-cap-workspace-",
            Some(&format!("{}\n", dx_adopt::MODULE_VERSION)),
        );
        let inv = invocation(&["capabilities", "--workspace-capabilities"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
        assert!(out.contains("capability workspace:pin"), "{out}");
        assert!(out.contains("availability: available"), "{out}");
        assert!(out.contains("capability workspace:tools"), "{out}");
        assert!(out.contains("availability: unknown"), "{out}");
        let inv = invocation(&["capabilities", "--workspace-capabilities", "--output=json"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
        let events = json_events(&out);
        let capabilities = events_of_kind(&events, "capability");
        assert_eq!(
            capabilities.len(),
            Command::value_variants().len() + 2,
            "{capabilities:?}"
        );
        let pin = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:pin"))
            .expect("pin record");
        assert_eq!(pin["source"], serde_json::json!("workspace-record"));
        assert_eq!(pin["availability"], serde_json::json!("available"));
        let tools = capabilities
            .iter()
            .find(|event| event["name"] == serde_json::json!("workspace:tools"))
            .expect("tools record");
        assert_eq!(tools["source"], serde_json::json!("unobserved"));
        assert_eq!(tools["availability"], serde_json::json!("unknown"));
    }

    #[test]
    fn workspace_pin_mismatch_reports_unavailable_without_failing() {
        let scratch = scratch_workspace("dx-cap-mismatch-", Some("9.9.9\n"));
        let inv = invocation(&["capabilities", "--workspace-capabilities"]);
        let (code, out, err) = run(&inv, scratch.path());
        assert_eq!(code, 0);
        assert!(err.is_empty(), "{err}");
        assert!(out.contains("capability workspace:pin"), "{out}");
        assert!(out.contains("availability: unavailable"), "{out}");
    }

    #[test]
    fn workspace_stage_without_module_fails_operational() {
        for words in [
            vec!["capabilities", "--workspace-capabilities"],
            vec!["capabilities", "--workspace-capabilities", "--output=json"],
        ] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-cap-broken-");
            let (code, out, err) = run(&inv, scratch.path());
            assert_eq!(code, 1, "words: {words:?}");
            assert!(err.contains("broken_workspace"), "words: {words:?} {err}");
            if words.contains(&"--output=json") {
                let events = json_events(&out);
                let kinds = event_kinds(&events);
                assert_eq!(
                    kinds,
                    vec!["command_started", "error", "command_finished"],
                    "words: {words:?}"
                );
                assert_eq!(events[1]["code"], serde_json::json!("broken_workspace"));
            }
        }
    }

    #[test]
    fn rejected_combinations_name_the_flag_or_scope() {
        assert_eq!(
            crate::args::parse(&strings(&["capabilities", "extra"])),
            Err(crate::args::ArgsError::UnsupportedOption {
                command: "capabilities",
                option: "extra".to_owned(),
            })
        );
        assert!(matches!(
            crate::args::parse(&strings(&["capabilities", "--check"])),
            Err(crate::args::ArgsError::UnsupportedOption { .. })
        ));
        assert_eq!(
            crate::args::parse(&strings(&["capabilities", "--output=diff"])),
            Err(crate::args::ArgsError::UnsupportedOption {
                command: "capabilities",
                option: "--output=diff".to_owned(),
            })
        );
        assert_eq!(
            crate::args::parse(&strings(&["capabilities", "--", "--jobs=4"])),
            Err(crate::args::ArgsError::UnsupportedOption {
                command: "capabilities",
                option: "--".to_owned(),
            })
        );
        assert!(matches!(
            crate::args::parse(&strings(&["lint", "--workspace-capabilities"])),
            Err(crate::args::ArgsError::UnsupportedOption { .. })
        ));
        let got = crate::args::parse(&strings(&["capabilities", "--workspace-capabilities"]))
            .expect("flag parses");
        assert!(got.workspace_capabilities);
        let bare = crate::args::parse(&strings(&["capabilities"])).expect("bare parses");
        assert!(!bare.workspace_capabilities);
    }

    #[test]
    fn dry_run_plans_without_reading_the_workspace() {
        let scratch = dx_test_scratch::scratch("dx-cap-dry-");
        for words in [
            vec!["capabilities", "--dry-run"],
            vec!["capabilities", "--dry-run", "--output=json"],
            vec![
                "capabilities",
                "--workspace-capabilities",
                "--dry-run",
                "--output=json",
            ],
        ] {
            let inv = invocation(&words);
            let (code, out, err) = run(&inv, scratch.path());
            assert_eq!(code, 0, "words: {words:?}");
            assert!(err.is_empty(), "words: {words:?} {err}");
            if words.contains(&"--output=json") {
                let events = json_events(&out);
                assert_eq!(
                    event_kinds(&events),
                    vec!["command_started", "command_finished"],
                    "words: {words:?}"
                );
            } else {
                assert!(
                    out.contains("would report capabilities"),
                    "words: {words:?} {out}"
                );
            }
        }
    }

    #[test]
    fn secret_config_values_never_reach_capability_output() {
        let scratch = dx_test_scratch::scratch("dx-cap-secret-");
        std::fs::create_dir_all(scratch.path().join(".dx")).expect("dx");
        std::fs::write(
            scratch.path().join(".dx/config.toml"),
            "[dx]\noutput = \"text\"\n# redacted-secret-token-xyz\n",
        )
        .expect("config");
        for words in [
            vec!["capabilities"],
            vec!["capabilities", "--output=json"],
            vec!["capabilities", "--workspace-capabilities"],
        ] {
            let inv = invocation(&words);
            let (code, out, _) = run(&inv, scratch.path());
            if words.contains(&"--workspace-capabilities") {
                assert_eq!(code, 1, "words: {words:?}");
                continue;
            }
            assert_eq!(code, 0, "words: {words:?}");
            assert!(
                !out.contains("redacted-secret-token-xyz"),
                "words: {words:?}"
            );
        }
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
    fn broken_pipe_returns_141() {
        for words in [vec!["capabilities"], vec!["capabilities", "--output=json"]] {
            let inv = invocation(&words);
            let scratch = dx_test_scratch::scratch("dx-cap-broken-pipe-");
            let mut err = Vec::new();
            let code = execute_capabilities(&inv, scratch.path(), &mut BrokenPipeWriter, &mut err);
            assert_eq!(code, 128 + 13, "words: {words:?}");
        }
    }
}
