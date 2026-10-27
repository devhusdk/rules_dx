use super::common::*;
use super::dep_edit;
use dx_output::{command_started, OutputMode};
use dx_plan::args::{Command, Invocation};
use dx_plan::reports::plan_reports;

const ADD_VERB: dep_edit::EditVerb = dep_edit::EditVerb {
    command: "add",
    noun: "add",
    set_action: "add to",
    notice_code: CODE_ADD_DECLARED,
    failure_code: CODE_ADD_FAILED,
    noop_note: "pinned module lock; no-op success; go.sum is not refreshed",
};

pub(crate) fn execute_add(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Add,
        "add dispatch guards commands"
    );
    let Env {
        workspace,
        runner,
        query_runner,
        out,
        err,
        ..
    } = env;
    if let Err(error) = plan_reports(
        workspace,
        invocation.command,
        &invocation.reports,
        &invocation.output,
    ) {
        return pre_exec(invocation.output, out, err, &error.to_string());
    }
    if invocation.targets.len() != 2 {
        return pre_exec(
            invocation.output,
            out,
            err,
            "add needs exactly <selector> <version>",
        );
    }
    let (selector, version) = (&invocation.targets[0], &invocation.targets[1]);
    let parsed = match dep_edit::configured_bazel_set(&ADD_VERB, invocation, out, err, workspace) {
        Ok(Some((set_name, manifest))) => {
            dx_bump::AddRequest::parse_configured(&set_name, &manifest, selector, version)
        }
        Ok(None) => dx_bump::AddRequest::parse(selector, version),
        Err(exit) => return exit,
    };
    let request = match parsed {
        Ok(request) => request,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let verbose = invocation.chatty();
    let applies = invocation.applies();
    if invocation.output == OutputMode::Json {
        let mode = if applies { "default" } else { "check" };
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), mode)) {
            return exit;
        }
    } else if verbose && applies {
        let summary = frozen_summary(
            offline_summary(request.summary(), invocation.offline),
            invocation.frozen,
        );
        if let Err(exit) = check_stdout_write(writeln!(out, "{summary}")) {
            return exit;
        }
    }
    if applies {
        if let Err(exit) = dep_edit::preflight(
            &ADD_VERB,
            invocation,
            out,
            err,
            workspace,
            &request.target,
            &request.selector,
        ) {
            return exit;
        }
    }
    let manifest = request.target_manifest().to_owned();
    let inserted = match plan_insert(&request, workspace, &manifest) {
        Ok(inserted) => inserted,
        Err(message) => return operational(invocation, out, err, CODE_ADD_FAILED, &message),
    };
    if !applies {
        let message = format!(
            "add pending: {} {} is not declared in {manifest} (run `dx add {} {} --apply`)",
            request.selector,
            request.version.display(),
            request.selector,
            request.version.display(),
        );
        return operational(invocation, out, err, CODE_ADD_FAILED, &message);
    }
    let refresh = request.refresh_selector();
    let edit = dep_edit::DepEdit {
        verb: &ADD_VERB,
        target: &request.target,
        selector: &request.selector,
        edited: format!(
            "added {} at {} to {manifest}",
            request.selector,
            request.version.display()
        ),
        kept: format!("add kept in {manifest}; run `dx update --apply {refresh}` to lock it"),
        hint: None,
    };
    let io = dep_edit::EditIo {
        workspace,
        runner,
        query_runner,
        out,
        err,
    };
    dep_edit::write_and_refresh(&edit, invocation, io, &[(manifest, inserted)], verbose)
}

fn plan_insert(
    request: &dx_bump::AddRequest,
    workspace: &std::path::Path,
    manifest: &str,
) -> Result<String, String> {
    let original = std::fs::read(workspace.join(manifest))
        .map_err(|_| format!("failed to add {}: cannot read {manifest}", request.selector))?;
    let text = String::from_utf8(original).map_err(|_| {
        format!(
            "failed to add {}: {manifest} is not valid UTF-8",
            request.selector
        )
    })?;
    request.plan_insert(&text).map_err(|error| match error {
        declared @ dx_bump::AddError::Declared { .. } => declared.to_string(),
        other => format!("failed to add {}: {other}", request.selector),
    })
}

#[cfg(test)]
#[path = "add_tests.rs"]
mod tests;
