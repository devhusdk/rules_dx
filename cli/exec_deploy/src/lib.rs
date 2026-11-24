#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok,
    clippy::wildcard_enum_match_arm
)]

//! The `dx deploy` verb: build a deploy target, then dry run or publish it.

use dx_exec_core::common::{
    AppExit, CODE_DEPLOY_NEEDS_HUMAN, Env, Failure, app_signalled_line, pre_exec, report_failure,
    resolve_failed, run_bazel, run_bazel_app,
};
use dx_plan::args::{BuildMode, Command, DX_BUILD_MODE_ENV, Invocation, resolve_build_mode};
use dx_plan::plan::{BuildPlan, plan_deploy_build, plan_deploy_run, shell_join};
use dx_plan::reports::plan_reports;
use dx_plan::resolve::{ResolveError, check_deployable, deploy_label_hint, resolve_deploy};
use std::io::Write;

/// Tells the deploy launcher to publish (1) or dry run (0).
const DEPLOY_LIVE_ENV: &str = "DX_DEPLOY_LIVE";

fn deploy_label(
    invocation: &Invocation,
    workspace: &std::path::Path,
    query_runner: &dyn dx_plan::resolve::QueryRunner,
) -> Result<String, Failure> {
    resolve_deploy(&invocation.targets).map_err(|error| {
        let mut failure = Failure::resolve(&error);
        if matches!(
            error,
            ResolveError::DeployCount { .. } | ResolveError::DeployScope { .. }
        ) {
            failure.message.push_str(&deploy_label_hint(
                workspace,
                query_runner,
                &invocation.bazel_startup_options,
            ));
        }
        failure
    })
}

fn deploy_build_mode(invocation: &Invocation, build_mode_raw: &str) -> BuildMode {
    let attr = if build_mode_raw == "NONE" || build_mode_raw == "None" {
        None
    } else {
        BuildMode::parse_attr(build_mode_raw)
    };
    resolve_build_mode(
        invocation.build_mode_flag(),
        attr,
        BuildMode::default_for(Command::Deploy),
    )
}

fn write_artifact_notes(
    err: &mut dyn Write,
    workspace: &std::path::Path,
    label: &str,
    info: &dx_plan::resolve::DeployInfo,
) {
    if !info.size_budget.is_empty() {
        dx_output::write_stderr(
            err,
            format_args!(
                "Size of {label}: {}\n",
                size_line(workspace, &info.size_stamp, &info.size_budget)
            ),
        );
    }
    if !info.sbom.is_empty() {
        dx_output::write_stderr(
            err,
            format_args!("SBOM of {label}: {}\n", sbom_line(workspace, &info.sbom)),
        );
    }
}

fn deploy_plans(
    invocation: &Invocation,
    label: &str,
    build_mode: BuildMode,
) -> Result<(BuildPlan, BuildPlan), dx_error::DxError> {
    plan_deploy_build(label, build_mode, &invocation.bazel_startup_options).and_then(|build| {
        plan_deploy_run(
            label,
            invocation.app_args(),
            build_mode,
            &invocation.bazel_startup_options,
        )
        .map(|run| (build, run))
    })
}

fn run_deploy_app(
    invocation: &Invocation,
    (out, err): (&mut dyn Write, &mut dyn Write),
    (workspace, runner): (&std::path::Path, &dyn dx_process::Runner),
    label: &str,
    argv: &[String],
    build_mode: BuildMode,
) -> i32 {
    let live = if invocation.applies() { "1" } else { "0" };
    match run_bazel_app(
        invocation,
        out,
        err,
        workspace,
        runner,
        argv,
        &[
            (DX_BUILD_MODE_ENV, build_mode.name()),
            (DEPLOY_LIVE_ENV, live),
        ],
    ) {
        Ok(AppExit {
            code,
            signal: Some(signo),
        }) => {
            app_signalled_line(err, label, signo);
            code
        }
        Ok(app) => app.code,
        Err(exit) => exit,
    }
}

pub fn execute_deploy(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir: _,
        pid: _,
        nonce: _,
        out,
        err,
        ci,
        audit_today: _,
        actions: _,
        proc_root: _,
        edit_commit_json: _,
    } = env;
    let planned_reports = match plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        Ok(planned) => planned,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    debug_assert!(planned_reports.is_empty(), "dx deploy takes no --report");
    let label = match deploy_label(invocation, workspace, query_runner) {
        Ok(label) => label,
        Err(failure) => return report_failure(invocation.output, out, err, &failure),
    };
    if let Some(code) = refuse_agent_apply(invocation, ci, &label, out, err) {
        return code;
    }
    let info = match check_deployable(
        &label,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(info) => info,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let build_mode = deploy_build_mode(invocation, &info.build_mode_raw);
    let plans = deploy_plans(invocation, &label, build_mode);
    let (build_plan, run_plan) = match plans {
        Ok(plans) => plans,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    if !invocation.quiet {
        let summary = if invocation.applies() {
            format!(
                "Running deploy for {label} (build_mode {})",
                build_mode.name()
            )
        } else {
            build_plan.summary.clone()
        };
        dx_output::write_stderr(err, format_args!("{summary}\n"));
    }
    let built = match run_bazel(
        invocation,
        out,
        err,
        workspace,
        runner,
        &build_plan.argv,
        &[],
    ) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if built != 0 {
        return built;
    }
    if !invocation.applies() && !invocation.quiet {
        write_artifact_notes(err, workspace, &label, &info);
    }
    if !invocation.applies() && !info.has_provider {
        if !invocation.quiet {
            dx_output::write_stderr(
                err,
                format_args!(
                    "Validated {label} (built, not published; publish with: {})\n",
                    publish_command(invocation, &label)
                ),
            );
        }
        return 0;
    }
    let code = run_deploy_app(
        invocation,
        (out, err),
        (workspace, runner),
        &label,
        &run_plan.argv,
        build_mode,
    );
    if code == 0 && !invocation.applies() && !invocation.quiet {
        dx_output::write_stderr(
            err,
            format_args!(
                "Dry run of {label} (nothing published; publish with: {})\n",
                publish_command(invocation, &label)
            ),
        );
    }
    code
}

/// Refuses `dx deploy --apply` in an agent session outside CI; a human or CI publishes.
fn refuse_agent_apply(
    invocation: &Invocation,
    ci: bool,
    label: &str,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Option<i32> {
    if !invocation.applies() || !invocation.agent || ci {
        return None;
    }
    let message = format!(
        "dx deploy --apply publishes {label}, so a human or CI must run it; dry run with: dx deploy {label}"
    );
    Some(report_failure(
        invocation.output,
        out,
        err,
        &Failure::new(CODE_DEPLOY_NEEDS_HUMAN, &message, 1).without_usage(),
    ))
}

fn publish_command(invocation: &Invocation, label: &str) -> String {
    let mut words = vec!["dx".to_owned(), "deploy".to_owned(), "--apply".to_owned()];
    if invocation.debug {
        words.push("--debug".to_owned());
    }
    if invocation.release {
        words.push("--release".to_owned());
    }
    words.push(label.to_owned());
    if !invocation.app_args().is_empty() {
        let (safe, withheld) =
            dx_plan::plan::quality::partition_provenance_options(invocation.app_args());
        words.push("--".to_owned());
        words.extend(safe);
        words.extend(withheld);
    }
    shell_join(&words)
}

fn sbom_line(workspace: &std::path::Path, sbom: &str) -> String {
    let base = sbom.rsplit('/').next().unwrap_or(sbom);
    match dx_sbom::spdx_package_count(&workspace.join(sbom)) {
        Some(count) => format!("{base} {count} packages"),
        None => format!("{base} is not a readable SPDX document"),
    }
}

fn size_line(workspace: &std::path::Path, stamp: &str, budget: &str) -> String {
    let checked = (!stamp.is_empty())
        .then(|| std::fs::read_to_string(workspace.join(stamp)).ok())
        .flatten();
    match checked {
        Some(text) => format!("{} (size_budget {budget})", text.trim()),
        None => format!("size_budget {budget}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{publish_command, size_line};
    use dx_plan::args::parse;

    #[test]
    fn publish_command_keeps_the_mode_flag_and_app_args() {
        let args: Vec<String> = ["deploy", "--release", "//d:prod", "--", "--port=1"]
            .iter()
            .map(ToString::to_string)
            .collect();
        let inv = parse(&args).expect("deploy parses");
        assert_eq!(
            publish_command(&inv, "//d:prod"),
            "dx deploy --apply --release //d:prod -- --port=1"
        );
    }

    #[test]
    fn size_line_without_a_readable_stamp_names_only_the_budget() {
        let dir = std::path::Path::new("/nonexistent-dx-deploy");
        assert_eq!(size_line(dir, "", "1MB"), "size_budget 1MB");
        assert_eq!(size_line(dir, "missing.stamp", "1MB"), "size_budget 1MB");
    }
}
