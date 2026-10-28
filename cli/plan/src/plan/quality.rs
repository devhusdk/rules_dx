use dx_process::{
    describe_scope, operation_summary, redact_url_credentials, ForwardError, ProtectedFlag, Scope,
};

use super::{
    bazel_argv, bep_guards, bep_options, download_flag, download_guards, exec_log_guards,
    exec_log_options,
    registry::{spec, CommandSpec},
    rules_dx_label, BuildPlan, KEEP_GOING_FLAG, OUTPUT_GROUP, SHARD_OUTPUT_GROUP,
    SKIP_INCOMPATIBLE_FLAG,
};
use crate::args::Command;
use crate::resolve::ResolvedScope;
use std::collections::BTreeSet;

fn joined_labels<'a>(aspects: impl IntoIterator<Item = &'a str>) -> String {
    aspects
        .into_iter()
        .map(rules_dx_label)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
fn required_options(entry: &CommandSpec, bazel_options: &[String], bep_path: &str) -> Vec<String> {
    shaped_options(
        &joined_labels(entry.aspects.iter().copied()),
        OUTPUT_GROUP,
        bazel_options,
        bep_path,
    )
}

/// The required options of one quality build; probe and build differ only in the aspects, output group and event file.
fn shaped_options(
    aspects: &str,
    group: &str,
    bazel_options: &[String],
    bep_path: &str,
) -> Vec<String> {
    let mut required = vec![
        format!("--aspects={aspects}"),
        format!("--output_groups={group}"),
        download_flag(bazel_options).to_owned(),
        KEEP_GOING_FLAG.to_owned(),
    ];
    required.extend(bep_options(bep_path));
    required
}

/// The shard name of one real aspect label: `...real_aspects.bzl%real_rust_lint_aspect` is `real_rust_lint`.
fn shard_name(aspect: &str) -> &str {
    let name = aspect.rsplit_once('%').map_or(aspect, |(_, name)| name);
    name.strip_suffix("_aspect").unwrap_or(name)
}

/// The core shard of each capability runs on every target, so the probe never drops it.
const CORE_SHARDS: [&str; 3] = ["real_lint", "real_format", "real_typecheck"];

/// The registry aspects to pass: all of them, or the core ones plus the shards the probe named.
pub fn select_aspects(entry: &CommandSpec, shards: Option<&BTreeSet<String>>) -> Vec<&'static str> {
    entry
        .aspects
        .iter()
        .copied()
        .filter(|aspect| {
            let name = shard_name(aspect);
            shards.is_none_or(|shards| CORE_SHARDS.contains(&name) || shards.contains(name))
        })
        .collect()
}

pub(crate) fn setting_name(option: &str) -> Result<String, ForwardError> {
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

pub fn protected_flags(required: &[String]) -> Result<Vec<ProtectedFlag>, ForwardError> {
    let keep_going = required
        .iter()
        .find(|option| option.as_str() == KEEP_GOING_FLAG)
        .cloned()
        .ok_or_else(|| ForwardError::InvalidRequiredOption {
            flag: "keep_going".to_owned(),
        })?;
    let mut protected = vec![
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
            name: "keep_going".to_owned(),
            required: Some(keep_going),
            allowed: Vec::new(),
        },
        ProtectedFlag {
            name: "nokeep_going".to_owned(),
            required: None,
            allowed: Vec::new(),
        },
    ];
    protected.extend(bep_guards());
    protected.extend(download_guards());
    Ok(protected)
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
    plan_build_with(
        command,
        resolved,
        bazel_options,
        bep_path,
        startup_options,
        None,
        None,
    )
}

/// Plans the quality build over the shards the probe named; `None` passes every shard.
pub fn plan_build_with(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
    shards: Option<&BTreeSet<String>>,
    exec_log: Option<&str>,
) -> Result<BuildPlan, ForwardError> {
    let entry = spec(command);
    let aspects = joined_labels(select_aspects(&entry, shards));
    let mut required = shaped_options(&aspects, OUTPUT_GROUP, bazel_options, bep_path);
    if let Some(path) = exec_log {
        required.extend(exec_log_options(path));
    }
    shaped_plan(
        command,
        resolved,
        bazel_options,
        startup_options,
        &required,
        exec_log.is_some(),
    )
}

/// Plans the shard probe; its argv matches `plan_build`'s apart from the aspects, output group and event file.
pub fn plan_probe(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    bep_path: &str,
    startup_options: &[String],
) -> Result<Option<BuildPlan>, ForwardError> {
    let Some(probe) = spec(command).probe else {
        return Ok(None);
    };
    shaped_plan(
        command,
        resolved,
        bazel_options,
        startup_options,
        &shaped_options(
            &rules_dx_label(probe),
            SHARD_OUTPUT_GROUP,
            bazel_options,
            bep_path,
        ),
        false,
    )
    .map(Some)
}

fn shaped_plan(
    command: Command,
    resolved: &ResolvedScope,
    bazel_options: &[String],
    startup_options: &[String],
    required: &[String],
    logged: bool,
) -> Result<BuildPlan, ForwardError> {
    let mut protected = protected_flags(required)?;
    if logged {
        protected.extend(exec_log_guards());
    }
    let (scope, labels) = workflow_scope_labels(resolved);
    let mut options = vec![SKIP_INCOMPATIBLE_FLAG.to_owned()];
    options.extend(bazel_options.iter().cloned());
    let argv = bazel_argv(
        "build",
        &options,
        required,
        &protected,
        &labels,
        startup_options,
    )?;
    let summary = operation_summary(command.name(), "analysis", &scope);
    Ok(BuildPlan { argv, summary })
}

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
        "header",
    ]
}

fn provenance_env_names() -> &'static [&'static str] {
    &[
        "action_env",
        "host_action_env",
        "test_env",
        "repo_env",
        "client_env",
    ]
}

fn provenance_keyed_names() -> &'static [&'static str] {
    &["define", "build_metadata"]
}

/// Matches user-chosen names (define keys, Starlark flag targets) by `_`-separated segment.
fn secret_key(name: &str) -> bool {
    let normalized = name.to_lowercase().replace(['-', '.'], "_");
    normalized.split('_').any(|segment| {
        let word = segment.strip_suffix('s').unwrap_or(segment);
        [segment, word].iter().any(|candidate| {
            matches!(*candidate, "key" | "pat") || provenance_secret_names().contains(candidate)
        })
    })
}

/// True when `text` holds a `KEY=` assignment whose key names a secret, such as `-DAPI_KEY=x`.
fn embedded_secret(text: &str) -> bool {
    text.match_indices('=').any(|(at, _)| {
        let head = &text[..at];
        let start = head
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
            .map_or(0, |index| index + 1);
        let key = &head[start..];
        let key = key
            .strip_prefix("--")
            .or_else(|| key.strip_prefix("-D"))
            .or_else(|| key.strip_prefix('-'))
            .unwrap_or(key);
        !key.is_empty() && secret_key(key)
    })
}

/// Returns the target name of a Starlark flag such as `//pkg:flag` or `no@repo//:flag`.
fn starlark_target(name: &str) -> Option<&str> {
    let rest = match name.strip_prefix("no") {
        Some(rest) if rest.starts_with("//") || rest.starts_with('@') => rest,
        _ => name,
    };
    if !(rest.starts_with("//") || rest.starts_with('@')) {
        return None;
    }
    Some(rest.rsplit([':', '/']).next().unwrap_or(rest))
}

fn provenance_option_name(option: &str) -> &str {
    let bare = option.strip_prefix("--").unwrap_or(option);
    bare.split('=').next().unwrap_or(bare)
}

fn env_assignment(value: &str) -> String {
    match value.split_once('=') {
        Some((name, _)) => format!("{name}=(value)"),
        None => value.to_owned(),
    }
}

fn keyed_secret(assignment: &str) -> bool {
    assignment
        .split_once('=')
        .is_some_and(|(key, _)| secret_key(key))
}

enum Pending {
    None,
    Secret,
    Env(String),
    Keyed(String),
}

/// Splits Bazel options into ones safe to echo and redacted stand-ins for secrets.
pub fn partition_provenance_options(options: &[String]) -> (Vec<String>, Vec<String>) {
    let mut safe = Vec::new();
    let mut withheld = Vec::new();
    let mut pending = Pending::None;
    for option in options {
        match std::mem::replace(&mut pending, Pending::None) {
            Pending::Secret if !option.starts_with('-') => {
                withheld.push("(value)".to_owned());
                continue;
            }
            Pending::Env(name) if !option.starts_with('-') => {
                withheld.push(format!("--{name} {}", env_assignment(option)));
                continue;
            }
            Pending::Keyed(name) if !option.starts_with('-') => {
                if keyed_secret(option) {
                    withheld.push(format!("--{name} {}", env_assignment(option)));
                } else {
                    safe.push(format!("--{name}"));
                    safe.push(redact_url_credentials(option));
                }
                continue;
            }
            Pending::Env(name) | Pending::Keyed(name) => safe.push(format!("--{name}")),
            _ => {}
        }
        let name = provenance_option_name(option).to_lowercase();
        if provenance_env_names().contains(&name.as_str()) {
            match option.split_once('=') {
                Some((flag, value)) if value.contains('=') => {
                    withheld.push(format!("{flag}={}", env_assignment(value)));
                }
                Some(_) => safe.push(redact_url_credentials(option)),
                None => pending = Pending::Env(name),
            }
            continue;
        }
        if provenance_keyed_names().contains(&name.as_str()) {
            match option.split_once('=') {
                Some((flag, value)) if keyed_secret(value) => {
                    withheld.push(format!("{flag}={}", env_assignment(value)));
                }
                Some(_) => safe.push(redact_url_credentials(option)),
                None => pending = Pending::Keyed(name),
            }
            continue;
        }
        if let Some(target) = starlark_target(&name) {
            let value_secret = option
                .split_once('=')
                .is_some_and(|(_, v)| embedded_secret(v));
            if secret_key(target) || value_secret {
                withheld.push(format!("--{}", provenance_option_name(option)));
            } else {
                safe.push(redact_url_credentials(option));
            }
            continue;
        }
        let secret = provenance_secret_names()
            .iter()
            .any(|secret| name == *secret || name.ends_with(&format!("_{secret}")));
        if !secret {
            if !option.starts_with("--") {
                if embedded_secret(option) {
                    withheld.push("(value)".to_owned());
                } else {
                    safe.push(redact_url_credentials(option));
                }
            } else if option
                .split_once('=')
                .is_some_and(|(_, v)| embedded_secret(v))
            {
                withheld.push(format!("--{}", provenance_option_name(option)));
            } else {
                safe.push(redact_url_credentials(option));
            }
            continue;
        }
        withheld.push(format!("--{}", provenance_option_name(option)));
        if !option.contains('=') {
            pending = Pending::Secret;
        }
    }
    if let Pending::Env(name) | Pending::Keyed(name) = pending {
        safe.push(format!("--{name}"));
    }
    (safe, withheld)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{parse, ArgsError};
    use crate::plan::BAZEL_SETTINGS;

    #[test]
    fn partition_provenance_options_cases() {
        let cases: &[(&[&str], &[&str], &[&str])] = &[
            (
                &["--define=API_TOKEN=hunter2"],
                &[],
                &["--define=API_TOKEN=(value)"],
            ),
            (
                &["--define", "API_TOKEN=hunter2"],
                &[],
                &["--define API_TOKEN=(value)"],
            ),
            (
                &["--define=DB_PASSWORD=hunter2"],
                &[],
                &["--define=DB_PASSWORD=(value)"],
            ),
            (
                &["--define=AWS_SECRET_ACCESS_KEY=hunter2"],
                &[],
                &["--define=AWS_SECRET_ACCESS_KEY=(value)"],
            ),
            (
                &["--define=GH_PAT=hunter2"],
                &[],
                &["--define=GH_PAT=(value)"],
            ),
            (&["--copt=-DAPI_KEY=hunter2"], &[], &["--copt"]),
            (
                &["--host_jvm_args=-Dauth.token=hunter2"],
                &[],
                &["--host_jvm_args"],
            ),
            (&["--test_arg=--token=hunter2"], &[], &["--test_arg"]),
            (&["--copt", "-DAPI_KEY=hunter2"], &["--copt"], &["(value)"]),
            (
                &["--host_jvm_args=-Dfile.encoding=UTF-8"],
                &["--host_jvm_args=-Dfile.encoding=UTF-8"],
                &[],
            ),
            (&["--copt=-DNDEBUG"], &["--copt=-DNDEBUG"], &[]),
            (&["--config=ci"], &["--config=ci"], &[]),
            (
                &["--remote_cache=grpcs://cache.example/x"],
                &["--remote_cache=grpcs://cache.example/x"],
                &[],
            ),
            (&["--define=version=1.2"], &["--define=version=1.2"], &[]),
            (
                &["--define", "version=1.2"],
                &["--define", "version=1.2"],
                &[],
            ),
            (
                &["--define=DB=postgres://u:hunter2@h"],
                &["--define=DB=postgres://(credentials)@h"],
                &[],
            ),
            (
                &["--build_metadata=AUTH_TOKEN=hunter2"],
                &[],
                &["--build_metadata=AUTH_TOKEN=(value)"],
            ),
            (&["--//pkg:token=hunter2"], &[], &["--//pkg:token"]),
            (&["--@repo//:secret=hunter2"], &[], &["--@repo//:secret"]),
            (
                &["--@repo//pkg:api-token=hunter2"],
                &[],
                &["--@repo//pkg:api-token"],
            ),
            (&["--//pkg/token=hunter2"], &[], &["--//pkg/token"]),
            (&["--no//pkg:token"], &[], &["--no//pkg:token"]),
            (
                &["--//pkg:token", "//target"],
                &["//target"],
                &["--//pkg:token"],
            ),
            (
                &["--@rules_dx//config:workspace=//consumer:policy"],
                &["--@rules_dx//config:workspace=//consumer:policy"],
                &[],
            ),
            (
                &["--@rules_dx//config:validate=false"],
                &["--@rules_dx//config:validate=false"],
                &[],
            ),
            (
                &["--remote_header=A=Bearer hunter2"],
                &[],
                &["--remote_header"],
            ),
            (
                &["--bes_header", "x=hunter2"],
                &[],
                &["--bes_header", "(value)"],
            ),
            (
                &["--action_env=API_KEY=hunter2"],
                &[],
                &["--action_env=API_KEY=(value)"],
            ),
            (
                &["--test_env", "DB=hunter2"],
                &[],
                &["--test_env DB=(value)"],
            ),
            (&["--repo_env=HOME"], &["--repo_env=HOME"], &[]),
            (
                &["--remote_cache=grpcs://ci:hunter2@cache.example"],
                &["--remote_cache=grpcs://(credentials)@cache.example"],
                &[],
            ),
            (&["--token=abc"], &[], &["--token"]),
            (&["--define"], &["--define"], &[]),
        ];
        for (input, want_safe, want_withheld) in cases {
            let options: Vec<String> = input.iter().map(|s| (*s).to_owned()).collect();
            let (safe, withheld) = partition_provenance_options(&options);
            assert_eq!(&safe, want_safe, "safe for {input:?}");
            assert_eq!(&withheld, want_withheld, "withheld for {input:?}");
            let all = format!("{safe:?}{withheld:?}");
            assert!(!all.contains("hunter2"), "{input:?}: {all}");
        }
    }
    use crate::resolve::{resolve, FakeQuery};
    use dx_process::Scope;
    use dx_testing::cli::{graph_rule, graph_stdout, strings};
    use std::path::PathBuf;

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
    fn probe_argv_differs_from_build_argv_only_in_aspects_group_and_event_file() {
        let user = strings(&["--config=ci", "--jobs=4"]);
        let startup = strings(&["--output_base=/tmp/ob"]);
        for command in [Command::Lint, Command::Format, Command::Typecheck] {
            let scope = resolved(&["//pkg:lib"]);
            let build =
                plan_build(command, &scope, &user, "/tmp/bep.json", &startup).expect("build");
            let probe = plan_probe(command, &scope, &user, "/tmp/probe.json", &startup)
                .expect("probe")
                .expect("quality commands carry a probe");
            assert_eq!(build.argv.len(), probe.argv.len());
            let differing: Vec<(&String, &String)> = build
                .argv
                .iter()
                .zip(&probe.argv)
                .filter(|(left, right)| left != right)
                .collect();
            let probe_label = rules_dx_label(spec(command).probe.expect("probe label"));
            assert_eq!(
                differing,
                vec![
                    (
                        &format!(
                            "--aspects={}",
                            joined_labels(spec(command).aspects.iter().copied())
                        ),
                        &format!("--aspects={probe_label}")
                    ),
                    (
                        &"--output_groups=dx_results".to_owned(),
                        &"--output_groups=dx_shards".to_owned()
                    ),
                    (
                        &"--build_event_json_file=/tmp/bep.json".to_owned(),
                        &"--build_event_json_file=/tmp/probe.json".to_owned()
                    ),
                ],
                "{command:?}"
            );
        }
        assert!(
            plan_probe(Command::Build, &resolved(&[]), &[], "/tmp/p.json", &[])
                .expect("plan")
                .is_none()
        );
    }

    #[test]
    fn select_aspects_keeps_core_shards_and_the_probed_ones() {
        let lint = spec(Command::Lint);
        assert_eq!(select_aspects(&lint, None), lint.aspects.to_vec());
        let rust: BTreeSet<String> = ["real_rust_lint".to_owned()].into();
        assert_eq!(
            select_aspects(&lint, Some(&rust)),
            vec![
                "quality:real_aspects.bzl%real_lint_aspect",
                "quality:real_aspects.bzl%real_rust_lint_aspect",
            ]
        );
        let unknown: BTreeSet<String> = ["real_nonexistent_lint".to_owned()].into();
        assert_eq!(
            select_aspects(&lint, Some(&unknown)),
            vec!["quality:real_aspects.bzl%real_lint_aspect"]
        );
        let format = spec(Command::Format);
        assert_eq!(
            select_aspects(&format, Some(&BTreeSet::new())),
            vec!["quality:real_aspects.bzl%real_format_aspect"]
        );
    }

    #[test]
    fn selected_shards_reach_the_build_argv() {
        let rust: BTreeSet<String> = ["real_rust_lint".to_owned()].into();
        let plan = plan_build_with(
            Command::Lint,
            &resolved(&[]),
            &[],
            "/tmp/bep.json",
            &[],
            Some(&rust),
            None,
        )
        .expect("plan");
        assert!(plan.argv.contains(
            &"--aspects=@rules_dx//quality:real_aspects.bzl%real_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_rust_lint_aspect".to_owned()
        ));
    }

    #[test]
    fn protected_flags_find_keep_going_by_value() {
        let entry = spec(Command::Lint);
        let required = required_options(&entry, &[], "/tmp/bep.json");
        let flags = protected_flags(&required).expect("flags");
        let keep_going = flags
            .iter()
            .find(|flag| flag.name == "keep_going")
            .expect("keep_going guard");
        assert_eq!(keep_going.required, Some(KEEP_GOING_FLAG.to_owned()));
    }

    #[test]
    fn protected_flags_reject_miswired_required_and_settings() {
        let entry = spec(Command::Lint);
        let mut required = required_options(&entry, &[], "/tmp/bep.json");
        required.retain(|option| option.as_str() != KEEP_GOING_FLAG);
        let err = protected_flags(&required).expect_err("missing keep_going");
        assert!(
            matches!(err, ForwardError::InvalidRequiredOption { .. }),
            "missing keep_going produced {err:?}"
        );
        let err = setting_name("clippy_output_diagnostics=true").expect_err("malformed setting");
        assert!(
            matches!(err, ForwardError::InvalidSetting { .. }),
            "malformed setting produced {err:?}"
        );
    }

    #[test]
    fn lint_plan_argv_places_required_after_user_options() {
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
            argv[..11],
            [
                "bazel",
                "--nohome_rc",
                "--nosystem_rc",
                "build",
                "--skip_incompatible_explicit_targets",
                "--jobs=4",
                "--@rules_dx//config:clippy_output_diagnostics=true",
                "--@rules_dx//config:rustc_output_diagnostics=true",
                "--aspects=@rules_dx//quality:real_aspects.bzl%real_cc_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_dockerfile_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_make_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_terraform_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_dotnet_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_flatbuffers_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_python_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_protobuf_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_powershell_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_jvm_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_ruby_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_rust_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_scala_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_text_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_html_template_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_yaml_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_nix_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_kubernetes_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_ansible_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_cloudformation_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_rego_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_jsonnet_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_fortran_lint_aspect,@rules_dx//quality:real_aspects.bzl%real_haskell_lint_aspect",
                "--output_groups=dx_results",
                "--remote_download_outputs=toplevel",
            ]
        );
        assert_eq!(
            argv[11..],
            [
                "--keep_going",
                "--build_event_json_file=/tmp/bep.json",
                "--build_event_json_file_path_conversion=false",
                "//...",
            ]
        );
        assert_eq!(plan.summary, "Running lint analysis for //...");
    }

    #[test]
    fn format_plan_lets_user_override_incompatible_skip() {
        let plan = plan_build(
            Command::Format,
            &resolved(&["//a:x"]),
            &strings(&["--noskip_incompatible_explicit_targets"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let position = |flag: &str| {
            plan.argv
                .iter()
                .position(|arg| arg == flag)
                .unwrap_or_else(|| panic!("{flag} missing: {plan:?}"))
        };
        let dx_flag = position("--skip_incompatible_explicit_targets");
        let user_flag = position("--noskip_incompatible_explicit_targets");
        let label = position("//a:x");
        assert!(dx_flag < user_flag, "dx flag precedes override: {plan:?}");
        assert!(user_flag < label, "override precedes labels: {plan:?}");
    }

    #[test]
    fn quality_plans_share_the_bazel_settings() {
        for command in [Command::Lint, Command::Typecheck, Command::Format] {
            let plan =
                plan_build(command, &resolved(&[]), &[], "/tmp/bep.json", &[]).expect("plan");
            for setting in BAZEL_SETTINGS {
                assert_eq!(
                    plan.argv.iter().filter(|arg| *arg == setting).count(),
                    1,
                    "{command:?} carries {setting}: {plan:?}"
                );
            }
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

    #[test]
    fn shuffled_query_orders_yield_identical_build_argv() {
        let scratch = dx_test_scratch::scratch("dx-quality-query-order-");
        let root: PathBuf = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("pkg")).expect("pkg dir");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("BUILD file");
        std::fs::write(root.join("pkg/a.py"), "x = 1\n").expect("source file");
        let lib = graph_rule("//pkg:lib", &["//pkg:a.py"], &[]);
        let extra = graph_rule("//pkg:extra", &["//pkg:a.py"], &[]);
        let forward = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[
            lib.clone(),
            extra.clone(),
        ]))]);
        let reversed = FakeQuery::new(vec![FakeQuery::ok(&graph_stdout(&[extra, lib]))]);
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
                .contains("@rules_dx//quality:real_aspects.bzl%real_typecheck_aspect")
                && arg.contains("@rules_dx//quality:real_aspects.bzl%real_rust_typecheck_aspect")
                && arg
                    .contains("@rules_dx//quality:real_aspects.bzl%real_dotnet_typecheck_aspect")
                && arg.contains("@rules_dx//quality:real_aspects.bzl%real_js_typecheck_aspect")),
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
            "--nokeep_going",
            "--build_event_json_file=/tmp/other.json",
            "--build_event_json_file_path_conversion",
            "--build_event_json_file_path_conversion=true",
            "--remote_download_minimal",
            "--remote_download_outputs=minimal",
            "--@rules_rust//rust/settings:clippy_output_diagnostics=false",
            "--@rules_rust//rust/settings:clippy_output_diagnostics",
            "--@rules_dx//config:clippy_output_diagnostics=false",
            "--@rules_dx//config:clippy_output_diagnostics",
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
            "--@rules_dx//config:rustc_output_diagnostics=false",
            "--@rules_dx//config:rustc_output_diagnostics",
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
    fn quality_required_flags_follow_user_config() {
        let plan = plan_build(
            Command::Lint,
            &resolved(&[]),
            &strings(&["--config=x"]),
            "/tmp/bep.json",
            &[],
        )
        .expect("plan");
        let position = |flag: &str| {
            plan.argv
                .iter()
                .position(|arg| arg == flag)
                .unwrap_or_else(|| panic!("{flag} is planned: {plan:?}"))
        };
        let config = position("--config=x");
        for flag in [
            crate::plan::DOWNLOAD_TOPLEVEL_FLAG,
            KEEP_GOING_FLAG,
            "--build_event_json_file=/tmp/bep.json",
            "--build_event_json_file_path_conversion=false",
        ] {
            assert!(position(flag) > config, "{flag}: {plan:?}");
        }
    }

    #[test]
    fn quality_plan_download_mode_follows_last_user_choice() {
        use crate::plan::{DOWNLOAD_ALL_FLAG, DOWNLOAD_TOPLEVEL_FLAG};
        for command in [Command::Lint, Command::Typecheck, Command::Format] {
            for (options, expected) in [
                (vec!["--remote_download_toplevel"], DOWNLOAD_TOPLEVEL_FLAG),
                (
                    vec!["--remote_download_outputs=toplevel"],
                    DOWNLOAD_TOPLEVEL_FLAG,
                ),
                (vec!["--remote_download_all"], DOWNLOAD_ALL_FLAG),
                (vec!["--remote_download_outputs=all"], DOWNLOAD_ALL_FLAG),
                (
                    vec!["--remote_download_all", "--remote_download_toplevel"],
                    DOWNLOAD_TOPLEVEL_FLAG,
                ),
            ] {
                let mut user = strings(&["--config=x"]);
                user.extend(strings(&options));
                let plan =
                    plan_build(command, &resolved(&[]), &user, "/tmp/bep.json", &[]).expect("plan");
                let last = plan
                    .argv
                    .iter()
                    .rposition(|arg| arg.starts_with("--remote_download"))
                    .expect("download flag");
                let config = plan
                    .argv
                    .iter()
                    .position(|arg| arg == "--config=x")
                    .expect("config");
                assert_eq!(plan.argv[last], expected, "{command:?} {options:?}");
                assert!(last > config, "{command:?} {options:?}: {plan:?}");
            }
        }
    }

    #[test]
    fn quality_plan_accepts_local_build_event_paths() {
        for option in [
            "--nobuild_event_json_file_path_conversion",
            "--build_event_json_file_path_conversion=false",
        ] {
            let plan = plan_build(
                Command::Lint,
                &resolved(&[]),
                &strings(&[option]),
                "/tmp/bep.json",
                &[],
            )
            .expect("local paths keep the build event file readable");
            assert!(plan.argv.iter().any(|arg| arg == option), "{plan:?}");
            assert_eq!(plan.argv.last(), Some(&"//...".to_owned()), "{plan:?}");
            assert!(plan
                .argv
                .iter()
                .any(|arg| arg == "--build_event_json_file_path_conversion=false"));
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
}
