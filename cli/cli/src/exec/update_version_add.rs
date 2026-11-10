use super::common::*;
use super::dep_edit;
use dx_error::DxError;
use dx_plan::args::Invocation;

const ADD_VERB: dep_edit::EditVerb = dep_edit::EditVerb {
    command: "update",
    noun: "add",
    notice_code: CODE_ADD_DECLARED,
    failure_code: CODE_UPDATE_WIDEN_FAILED,
    noop_note: "pinned module lock; no-op success; go.sum is not refreshed",
};

pub(super) struct Io<'a> {
    pub(super) workspace: &'a std::path::Path,
    pub(super) runner: &'a dyn dx_process::Runner,
    pub(super) query_runner: &'a dyn dx_plan::resolve::QueryRunner,
    pub(super) out: &'a mut dyn std::io::Write,
    pub(super) err: &'a mut dyn std::io::Write,
}

/// Declares a package `dx update` could not find; `missing` is the not-found failure.
pub(super) fn add_missing(
    invocation: &Invocation,
    io: Io<'_>,
    update: &dx_update::edit::UpdateRequest,
    missing: &str,
) -> i32 {
    let Io {
        workspace,
        runner,
        query_runner,
        out,
        err,
    } = io;
    if update.set == dx_update::edit::UpdateSet::GithubActions {
        return operational(invocation, out, err, CODE_UPDATE_WIDEN_FAILED, missing);
    }
    let request = dx_update::edit::AddRequest::from_request(update);
    let applies = invocation.applies();
    let verbose = invocation.chatty();
    if applies
        && let Err(exit) = dep_edit::preflight(
            &ADD_VERB,
            invocation,
            out,
            err,
            workspace,
            &request.target,
            &request.selector,
        )
    {
        return exit;
    }
    let manifest = request.target_manifest().to_owned();
    let inserted = match plan_insert(&request, workspace, &manifest) {
        Ok(inserted) => inserted,
        Err(error) => {
            return report_dx_error(
                invocation.output,
                out,
                err,
                CODE_UPDATE_WIDEN_FAILED,
                &error,
            );
        }
    };
    if !applies {
        let message = format!(
            "update pending: {} {} is not declared in {manifest} (run `dx update {} {} --apply`)",
            request.selector,
            request.version.display(),
            request.selector,
            request.version.display(),
        );
        return operational(invocation, out, err, CODE_UPDATE_WIDEN_FAILED, &message);
    }
    let refresh = if request.target.configured.is_some() {
        request.target.refresh_command()
    } else {
        format!("dx update --apply {}", request.refresh_selector())
    };
    let edit = dep_edit::DepEdit {
        verb: &ADD_VERB,
        target: &request.target,
        selector: &request.selector,
        edited: format!(
            "added {} at {} to {manifest}",
            request.selector,
            request.version.display()
        ),
        kept: format!("add kept in {manifest}; run `{refresh}` to lock it"),
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
    request: &dx_update::edit::AddRequest,
    workspace: &std::path::Path,
    manifest: &str,
) -> Result<String, DxError> {
    let original = dx_path::workspace::resolve_in_workspace(workspace, manifest)
        .and_then(std::fs::read)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidInput {
                DxError::failure(format!(
                    "failed to add {}: {manifest} leaves the workspace",
                    request.selector
                ))
            } else {
                DxError::from_io(
                    format!("failed to add {}: cannot read {manifest}", request.selector),
                    &error,
                )
            }
        })?;
    let text = String::from_utf8(original).map_err(|_| {
        DxError::failure(format!(
            "failed to add {}: {manifest} is not valid UTF-8",
            request.selector
        ))
    })?;
    request.plan_insert(&text).map_err(|error| match error {
        declared @ dx_update::edit::AddError::Declared { .. } => {
            DxError::failure(declared.to_string())
        }
        other => DxError::failure(format!("failed to add {}: {other}", request.selector)),
    })
}

#[cfg(test)]
#[path = "update_version_add_tests.rs"]
mod tests;
