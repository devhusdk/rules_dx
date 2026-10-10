use dx_process::{
    build_workflow_argv, describe_scope, operation_summary, ForwardError, ProtectedFlag, Scope,
};

use super::{
    registry::{spec, CommandSpec},
    BuildPlan, BEP_FLAG_NAME, DOWNLOAD_ALL_FLAG, KEEP_GOING_FLAG, OUTPUT_GROUP, VALIDATE_FLAG,
};
use crate::args::Command;
use crate::resolve::ResolvedScope;

pub fn required_options(entry: &CommandSpec, bep_path: &str) -> Vec<String> {
    let mut options = vec![
        format!("--aspects={}", entry.aspects.join(",")),
        format!("--output_groups={OUTPUT_GROUP}"),
        DOWNLOAD_ALL_FLAG.to_owned(),
        VALIDATE_FLAG.to_owned(),
        KEEP_GOING_FLAG.to_owned(),
        format!("--{BEP_FLAG_NAME}={bep_path}"),
    ];
    options.extend(entry.settings.iter().map(ToString::to_string));
    options
}

fn setting_name(option: &str) -> Result<String, ForwardError> {
    let bare = option
        .strip_prefix("--")
        .ok_or_else(|| ForwardError::InvalidSetting {
            option: option.to_owned(),
        })?;
    let name = bare.split('=').next().unwrap_or("");
    if name.is_empty() {
        return Err(ForwardError::InvalidSetting {
            option: option.to_owned(),
        });
    }
    Ok(name.to_owned())
}

pub fn protected_flags(
    required: &[String],
    settings: &[&str],
) -> Result<Vec<ProtectedFlag>, ForwardError> {
    let keep_going = required
        .iter()
        .find(|option| option.as_str() == KEEP_GOING_FLAG)
        .cloned()
        .ok_or_else(|| ForwardError::InvalidRequiredOption {
            flag: "keep_going".to_owned(),
        })?;
    let mut flags = vec![
        ProtectedFlag {
            name: "aspects".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "output_groups".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "remote_download_outputs".to_owned(),
            required: Some(DOWNLOAD_ALL_FLAG.to_owned()),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "@rules_dx//config:validate".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "keep_going".to_owned(),
            required: Some(keep_going),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "nokeep_going".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: BEP_FLAG_NAME.to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ];
    for setting in settings {
        flags.push(ProtectedFlag {
            name: setting_name(setting)?,
            required: None,
            allowed: Vec::new(),
        });
    }
    Ok(flags)
}

pub(crate) fn workflow_scope_labels(resolved: &ResolvedScope) -> (Scope, Vec<String>) {
    if resolved.targets.is_empty() {
        (Scope::Repository, vec![describe_scope(&Scope::Repository)])
    } else {
        (resolved.scope.clone(), resolved.targets.clone())
    }
}

pub fn plan_build(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
) -> Result<BuildPlan, ForwardError> {
    let entry = spec(command);
    let required = required_options(&entry, bep_path);
    let protected = protected_flags(&required, entry.settings)?;
    let (scope, labels) = workflow_scope_labels(resolved);
    let argv = build_workflow_argv(
        "build",
        bazel_options,
        &required,
        &protected,
        &labels,
        startup_options,
    )?;
    let summary = operation_summary(command.name(), "analysis", &scope);
    Ok(BuildPlan { argv, summary })
}

pub const WORKSPACE_POLICY_FLAG: &str = "--@rules_dx//config:workspace=";
const WORKSPACE_POLICY_BARE: &str = "--@rules_dx//config:workspace";

fn provenance_secret_names() -> &'static [&'static str] {
    &[
        "password",
        "passwd",
        "token",
        "secret",
        "apikey",
        "api_key",
        "credential",
        "auth",
        "private_key",
        "oauth",
    ]
}

fn provenance_option_name(option: &str) -> &str {
    let bare = option.strip_prefix("--").unwrap_or(option);
    bare.split('=').next().unwrap_or(bare)
}

pub fn partition_provenance_options(options: &[String]) -> (Vec<String>, Vec<String>) {
    let mut safe = Vec::new();
    let mut withheld = Vec::new();
    let mut skip_value = false;
    for option in options {
        if skip_value {
            skip_value = false;
            if option.starts_with('-') {
                safe.push(option.clone());
            } else {
                withheld.push("(value)".to_owned());
            }
            continue;
        }
        let name = provenance_option_name(option).to_lowercase();
        let secret = provenance_secret_names()
            .iter()
            .any(|secret| name == *secret || name.ends_with(&format!("_{secret}")));
        if !secret {
            safe.push(option.clone());
            continue;
        }
        withheld.push(format!("--{}", provenance_option_name(option)));
        if !option.contains('=') {
            skip_value = true;
        }
    }
    (safe, withheld)
}

pub fn policy_origin(bazel_options: &[String]) -> (String, String) {
    let mut origin: Option<String> = None;
    let mut index = 0;
    while index < bazel_options.len() {
        let option = &bazel_options[index];
        if let Some(value) = option.strip_prefix(WORKSPACE_POLICY_FLAG) {
            if !value.is_empty() {
                origin = Some(value.to_owned());
            }
        } else if option == WORKSPACE_POLICY_BARE {
            if let Some(next) = bazel_options.get(index + 1) {
                if !next.starts_with('-') && !next.is_empty() {
                    origin = Some(next.clone());
                }
            }
        }
        index += 1;
    }
    match origin {
        Some(label) => (label, "bazel-option".to_owned()),
        None => ("default".to_owned(), "builtin-default".to_owned()),
    }
}

pub fn quality_provenance(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    workspace_override: Option<&str>,
    applies: bool,
    build_argv: &[String],
) -> serde_json::Value {
    let entry = spec(command);
    let (policy_label, policy_source) = policy_origin(bazel_options);
    let (scope_kind, scope_targets) = if resolved.targets.is_empty() {
        (
            "repository".to_owned(),
            vec![describe_scope(&Scope::Repository)],
        )
    } else {
        let kind = match &resolved.scope {
            Scope::Repository => "repository",
            Scope::Pattern(_) => "pattern",
            Scope::Labels(_) => "labels",
            Scope::ResolvedOwners(_) => "resolved-owners",
            Scope::Count(_) => "count",
        }
        .to_owned();
        let mut targets = resolved.targets.clone();
        targets.sort();
        targets.dedup();
        (kind, targets)
    };
    let mut aspects: Vec<String> = entry.aspects.iter().map(ToString::to_string).collect();
    aspects.sort();
    let mut settings: Vec<String> = entry.settings.iter().map(ToString::to_string).collect();
    settings.sort();
    let mut reports: Vec<String> = entry
        .reports
        .iter()
        .map(|format| format.name().to_owned())
        .collect();
    reports.sort();
    let (workspace_origin, workspace_selected) = match workspace_override {
        Some(path) => ("flag".to_owned(), path.to_owned()),
        None => ("discovered".to_owned(), "discovered".to_owned()),
    };
    let mode = if applies { "default" } else { "check" };
    let (safe_argv, withheld) = partition_provenance_options(build_argv);
    serde_json::json!({
        "command": command.name(),
        "operation": "plan",
        "mode": mode,
        "dry_run": true,
        "validation_performed": false,
        "scope": {"kind": scope_kind, "targets": scope_targets},
        "policy": {"origin": policy_label, "source": policy_source},
        "workspace": {"origin": workspace_origin, "selected": workspace_selected},
        "aspects": aspects,
        "settings": settings,
        "reports": reports,
        "capability": {"name": command.name(), "availability": "selected"},
        "execution": {"platform": "unknown", "toolchain": "unknown", "reason": "requires Bazel analysis"},
        "skipped": [],
        "unresolved": ["execution-platform", "toolchain", "policy-disabled"],
        "inputs": {"bazel_argv": safe_argv, "withheld_options": withheld},
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{parse, ArgsError};
    use crate::plan::{CLIPPY_DIAGNOSTICS_FLAG, RUSTC_DIAGNOSTICS_FLAG};
    use crate::resolve::{resolve, QueryResult, QueryRunner};
    use crate::test_support::strings;
    use dx_process::Scope;
    use std::cell::RefCell;
    use std::io;
    use std::path::{Path, PathBuf};

    fn resolved(targets: &[&str]) -> ResolvedScope {
        ResolvedScope {
            scope: if targets.is_empty() {
                Scope::Repository
            } else {
                Scope::Labels(strings(targets))
            },
            targets: strings(targets),
        }
    }

    #[test]
    fn protected_flags_find_keep_going_by_value() {
        let entry = spec(Command::Lint);
        let required = required_options(&entry, "/tmp/bep.json");
        let flags = protected_flags(&required, entry.settings).expect("flags");
        let keep_going = flags
            .iter()
            .find(|flag| flag.name == "keep_going")
            .expect("keep_going guard");
        assert_eq!(keep_going.required, Some(KEEP_GOING_FLAG.to_owned()));
    }

    #[test]
    fn protected_flags_reject_miswired_required_and_settings() {
        let entry = spec(Command::Lint);
        let mut required = required_options(&entry, "/tmp/bep.json");
        required.retain(|option| option.as_str() != KEEP_GOING_FLAG);
        let err = protected_flags(&required, entry.settings).expect_err("missing keep_going");
        assert!(
            matches!(err, ForwardError::InvalidRequiredOption { .. }),
            "missing keep_going produced {err:?}"
        );
        let required = required_options(&entry, "/tmp/bep.json");
        let err = protected_flags(&required, &["clippy_output_diagnostics=true"])
            .expect_err("malformed setting");
        assert!(
            matches!(err, ForwardError::InvalidSetting { .. }),
            "malformed setting produced {err:?}"
        );
    }

    #[test]
    fn lint_plan_argv_places_required_before_user_options() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--jobs=4"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(
            argv[..7],
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--aspects=//quality:real_aspects.bzl%real_lint_aspect,//quality:real_aspects.bzl%real_js_lint_aspect,//quality:real_aspects.bzl%real_python_lint_aspect,//quality:real_aspects.bzl%real_jvm_lint_aspect,//quality:real_aspects.bzl%real_rust_lint_aspect,//quality:real_aspects.bzl%real_shell_lint_aspect,//quality:real_aspects.bzl%real_text_lint_aspect",
                "--output_groups=dx_results",
                "--remote_download_outputs=all",
            ]
        );
        assert_eq!(
            argv[7..],
            [
                "--@rules_dx//config:validate=false",
                "--keep_going",
                "--build_event_json_file=/tmp/bep.json",
                "--@rules_rust//rust/settings:clippy_output_diagnostics=true",
                "--jobs=4",
                "//...",
            ]
        );
        assert_eq!(plan.summary, "Running lint analysis for //...");
    }

    #[test]
    fn lint_plan_enables_upstream_clippy_diagnostics() {
        let plan =
            plan_build(Command::Lint, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg == CLIPPY_DIAGNOSTICS_FLAG),
            "lint sets the upstream diagnostics setting: {plan:?}"
        );
        for command in [Command::Typecheck, Command::Format] {
            let plan =
                plan_build(command, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
            assert!(
                plan.argv.iter().all(|arg| arg != CLIPPY_DIAGNOSTICS_FLAG),
                "{command:?} leaves the clippy setting off: {plan:?}"
            );
        }
    }

    #[test]
    fn typecheck_plan_enables_upstream_rustc_diagnostics() {
        let plan = plan_build(
            Command::Typecheck,
            &resolved(&[]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg == RUSTC_DIAGNOSTICS_FLAG),
            "typecheck sets the upstream diagnostics setting: {plan:?}"
        );
        for command in [Command::Lint, Command::Format] {
            let plan =
                plan_build(command, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
            assert!(
                plan.argv.iter().all(|arg| arg != RUSTC_DIAGNOSTICS_FLAG),
                "{command:?} leaves the rustc setting off: {plan:?}"
            );
        }
    }

    #[test]
    fn explicit_targets_replace_repository_scope() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&["//a:one", "//b/..."]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(argv[argv.len() - 2..], ["//a:one", "//b/..."]);
        assert_eq!(plan.summary, "Running lint analysis for //a:one //b/...");
    }

    #[test]
    fn resolved_owners_render_sorted_owners_in_summary() {
        let scope = ResolvedScope {
            scope: Scope::ResolvedOwners(strings(&["//a:a", "//b:b"])),
            targets: strings(&["//a:a", "//b:b"]),
        };
        let plan = plan_build(Command::Lint, &scope, &[], "/tmp/bep.json", &[]).expect("plan");
        let argv: Vec<&str> = plan.argv.iter().map(String::as_str).collect();
        assert_eq!(argv[argv.len() - 2..], ["//a:a", "//b:b"]);
        assert_eq!(plan.summary, "Running lint analysis for //a:a //b:b");
    }

    struct FakeQuery {
        outputs: RefCell<Vec<QueryResult>>,
    }

    impl FakeQuery {
        fn new(outputs: Vec<QueryResult>) -> Self {
            FakeQuery {
                outputs: RefCell::new(outputs),
            }
        }

        fn ok(lines: &str) -> QueryResult {
            QueryResult {
                code: Some(0),
                stdout: lines.as_bytes().to_vec(),
                stderr: Vec::new(),
            }
        }
    }

    impl QueryRunner for FakeQuery {
        fn run_query(&self, _argv: &[String], _cwd: &Path) -> io::Result<QueryResult> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    #[test]
    fn shuffled_query_orders_yield_identical_build_argv() {
        let scratch = dx_test_scratch::scratch("dx-quality-query-order-");
        let root: PathBuf = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("pkg")).expect("pkg dir");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("BUILD file");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source file");
        let forward = FakeQuery::new(vec![FakeQuery::ok("//pkg:lib\n//pkg:extra\n")]);
        let reversed = FakeQuery::new(vec![FakeQuery::ok("//pkg:extra\n//pkg:lib\n")]);
        let first = resolve(&strings(&["pkg/a.py"]), &root, &forward, &[]).expect("forward");
        let second = resolve(&strings(&["pkg/a.py"]), &root, &reversed, &[]).expect("reversed");
        assert_eq!(first.targets, second.targets);
        assert_eq!(first.targets, strings(&["//pkg:extra", "//pkg:lib"]));
        let first_plan =
            plan_build(Command::Lint, &first, &[], "/tmp/bep.json", &[]).expect("forward plan");
        let second_plan =
            plan_build(Command::Lint, &second, &[], "/tmp/bep.json", &[]).expect("reversed plan");
        assert_eq!(first_plan.argv, second_plan.argv);
    }

    #[test]
    fn typecheck_plan_carries_typecheck_aspect_and_format_summary() {
        let plan = plan_build(
            Command::Typecheck,
            &resolved(&[]),
            &[],
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(
            plan.argv.iter().any(|arg| arg
                .contains("//quality:real_aspects.bzl%real_typecheck_aspect")
                && arg.contains("//quality:real_aspects.bzl%real_rust_typecheck_aspect")),
            "typecheck selects the real typecheck aspects: {plan:?}"
        );
        assert_eq!(plan.summary, "Running typecheck analysis for //...");
        let plan =
            plan_build(Command::Format, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
        assert_eq!(plan.summary, "Running format analysis for //...");
    }

    #[test]
    fn keep_going_repetition_is_accepted() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--keep_going"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert!(plan.argv.iter().any(|arg| arg == "--keep_going"));
    }

    #[test]
    fn consumer_policy_selection_reaches_bazel() {
        let selection = "--@rules_dx//config:workspace=//consumer:policy";
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&[selection]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        assert_eq!(
            plan.argv.iter().filter(|arg| *arg == selection).count(),
            1,
            "the consumer policy selection passes through once: {plan:?}"
        );
    }

    #[test]
    fn conflicting_workflow_options_fail_before_execution() {
        for conflicting in [
            "--aspects=//other.bzl%aspect",
            "--output_groups=other",
            "--@rules_dx//config:validate=true",
            "--nokeep_going",
            "--build_event_json_file=/tmp/other.json",
            "--@rules_rust//rust/settings:clippy_output_diagnostics=false",
            "--@rules_rust//rust/settings:clippy_output_diagnostics",
        ] {
            let err = plan_build(
                Command::Lint,
                &resolved(&[]),
                &strings(&[conflicting]),
                "/tmp/bep.json",
                &[],
            )
            .expect_err("conflict must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
        for conflicting in [
            "--@rules_rust//rust/settings:rustc_output_diagnostics=false",
            "--@rules_rust//rust/settings:rustc_output_diagnostics",
        ] {
            let err = plan_build(
                Command::Typecheck,
                &resolved(&[]),
                &strings(&[conflicting]),
                "/tmp/bep.json",
                &[],
            )
            .expect_err("conflict must fail");
            assert!(
                matches!(err, ForwardError::ConflictingOption { .. }),
                "{conflicting} produced {err:?}"
            );
        }
    }

    #[test]
    fn startup_options_are_rejected_as_command_options() {
        let err = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--home_rc"]),
            "/tmp/bep.json",
            &[],
        )
        .expect_err("startup option must fail");
        assert!(matches!(err, ForwardError::StartupOption { .. }));
    }

    #[test]
    fn unsupported_report_format_fails_with_command_registry() {
        let invocation = parse(&strings(&["lint", "--report=junit=out.xml"])).expect("parse");
        let entry = spec(invocation.command);
        assert!(
            !entry.accepts_report(&invocation.reports[0].format),
            "junit is not a lint report: {invocation:?}"
        );
        assert_eq!(
            parse(&strings(&["format", "--report=sarif=out.sarif"])),
            Err(crate::args::ArgsError::UnsupportedOption {
                command: "format",
                option: "--report=sarif=out.sarif".to_owned(),
            })
        );
        let words = ["lint", "--report=sarif"];
        let error = parse(&strings(&words)).unwrap_err();
        let ArgsError::Usage { text } = &error else {
            panic!("--report=sarif: want Usage, got {error:?}");
        };
        assert!(text.contains("report"), "{text}");
    }

    fn provenance_argv() -> Vec<String> {
        vec![
            "bazel".to_owned(),
            "build".to_owned(),
            "--aspects=//quality:real_aspects.bzl%real_lint_aspect".to_owned(),
        ]
    }

    #[test]
    fn provenance_defaults_to_builtin_policy_and_unknown_execution() {
        let provenance = quality_provenance(
            Command::Lint,
            &resolved(&[]),
            &[],
            None,
            false,
            &provenance_argv(),
        );
        assert_eq!(provenance["command"], serde_json::json!("lint"));
        assert_eq!(provenance["operation"], serde_json::json!("plan"));
        assert_eq!(provenance["mode"], serde_json::json!("check"));
        assert_eq!(provenance["dry_run"], serde_json::json!(true));
        assert_eq!(provenance["validation_performed"], serde_json::json!(false));
        assert_eq!(provenance["scope"]["kind"], serde_json::json!("repository"));
        assert_eq!(provenance["scope"]["targets"], serde_json::json!(["//..."]));
        assert_eq!(provenance["policy"]["origin"], serde_json::json!("default"));
        assert_eq!(
            provenance["policy"]["source"],
            serde_json::json!("builtin-default")
        );
        assert_eq!(
            provenance["workspace"]["origin"],
            serde_json::json!("discovered")
        );
        assert_eq!(
            provenance["execution"]["platform"],
            serde_json::json!("unknown")
        );
        assert_eq!(
            provenance["execution"]["toolchain"],
            serde_json::json!("unknown")
        );
        assert_eq!(
            provenance["execution"]["reason"],
            serde_json::json!("requires Bazel analysis")
        );
        assert_eq!(provenance["skipped"], serde_json::json!([]));
        assert!(provenance["unresolved"]
            .as_array()
            .expect("unresolved")
            .contains(&serde_json::json!("execution-platform")));
        assert_eq!(
            provenance["capability"]["availability"],
            serde_json::json!("selected")
        );
    }

    #[test]
    fn provenance_reports_explicit_workspace_policy() {
        let options = strings(&["--@rules_dx//config:workspace=//consumer:policy"]);
        let provenance = quality_provenance(
            Command::Lint,
            &resolved(&[]),
            &options,
            Some("/ws"),
            false,
            &provenance_argv(),
        );
        assert_eq!(
            provenance["policy"]["origin"],
            serde_json::json!("//consumer:policy")
        );
        assert_eq!(
            provenance["policy"]["source"],
            serde_json::json!("bazel-option")
        );
        assert_eq!(provenance["workspace"]["origin"], serde_json::json!("flag"));
        assert_eq!(
            provenance["workspace"]["selected"],
            serde_json::json!("/ws")
        );
    }

    #[test]
    fn provenance_last_policy_wins_and_bare_form_resolves() {
        let options = strings(&[
            "--@rules_dx//config:workspace=//a:one",
            "--@rules_dx//config:workspace",
            "//b:two",
        ]);
        let provenance = quality_provenance(
            Command::Lint,
            &resolved(&[]),
            &options,
            None,
            false,
            &provenance_argv(),
        );
        assert_eq!(provenance["policy"]["origin"], serde_json::json!("//b:two"));
    }

    #[test]
    fn provenance_sorts_targets_aspects_and_reports() {
        let scope = ResolvedScope {
            scope: Scope::Labels(strings(&["//b:b", "//a:a", "//a:a"])),
            targets: strings(&["//b:b", "//a:a", "//a:a"]),
        };
        let provenance =
            quality_provenance(Command::Lint, &scope, &[], None, false, &provenance_argv());
        assert_eq!(provenance["scope"]["kind"], serde_json::json!("labels"));
        assert_eq!(
            provenance["scope"]["targets"],
            serde_json::json!(["//a:a", "//b:b"])
        );
        let aspects = provenance["aspects"].as_array().expect("aspects");
        let mut sorted = aspects.clone();
        sorted.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        assert_eq!(aspects, &sorted);
        assert!(aspects.iter().any(|aspect| aspect
            .as_str()
            .expect("aspect")
            .contains("real_lint_aspect")));
    }

    #[test]
    fn provenance_preserves_bzlmod_labels_verbatim() {
        let scope = ResolvedScope {
            scope: Scope::Labels(strings(&["@@rules_dx//quality:default"])),
            targets: strings(&["@@rules_dx//quality:default"]),
        };
        let provenance =
            quality_provenance(Command::Lint, &scope, &[], None, false, &provenance_argv());
        assert_eq!(
            provenance["scope"]["targets"],
            serde_json::json!(["@@rules_dx//quality:default"])
        );
    }

    #[test]
    fn provenance_redacts_secret_option_values() {
        let argv = strings(&[
            "bazel",
            "build",
            "--token=secret123",
            "--jobs=4",
            "--api_key",
            "hunter2",
        ]);
        let provenance = quality_provenance(Command::Lint, &resolved(&[]), &[], None, false, &argv);
        let safe = provenance["inputs"]["bazel_argv"].as_array().expect("argv");
        assert!(safe
            .iter()
            .all(|entry| entry.as_str().expect("arg") != "--token=secret123"));
        assert!(safe.iter().any(|entry| entry == "--jobs=4"));
        let withheld = provenance["inputs"]["withheld_options"]
            .as_array()
            .expect("withheld");
        assert!(withheld.contains(&serde_json::json!("--token")));
        assert!(withheld.contains(&serde_json::json!("--api_key")));
    }

    #[test]
    fn provenance_distinguishes_default_from_apply_mode() {
        let check = quality_provenance(
            Command::Lint,
            &resolved(&[]),
            &[],
            None,
            false,
            &provenance_argv(),
        );
        let apply = quality_provenance(
            Command::Lint,
            &resolved(&[]),
            &[],
            None,
            true,
            &provenance_argv(),
        );
        assert_eq!(check["mode"], serde_json::json!("check"));
        assert_eq!(apply["mode"], serde_json::json!("default"));
        assert_eq!(check["dry_run"], serde_json::json!(true));
        assert_eq!(apply["validation_performed"], serde_json::json!(false));
    }
}
