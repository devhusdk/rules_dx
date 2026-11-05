//! `dx release`: the next version from conventional commits and the manifests that carry it.

use std::path::Path;
use std::process::Command as Process;

use super::common::*;
use dx_plan::args::Invocation;
use dx_update::release::{plan, read_config};

fn git(workspace: &Path, args: &[&str]) -> Option<String> {
    let output = Process::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(crate) fn execute_release(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    let config = match read_config(workspace) {
        Ok(config) => config,
        Err(message) => return refusal(invocation.output, out, err, &message),
    };
    let pattern = format!("{}*", config.tag_prefix);
    let tag = git(
        workspace,
        &["describe", "--tags", "--abbrev=0", "--match", &pattern],
    )
    .map(|tag| tag.trim().to_owned())
    .filter(|tag| !tag.is_empty());
    let range = tag.as_ref().map(|tag| format!("{tag}..HEAD"));
    let mut args = vec!["log", "--format=%H%x00%s%x00%b%x1e"];
    args.extend(range.as_deref());
    let Some(log) = git(workspace, &args) else {
        return operational(
            invocation,
            out,
            err,
            "release_git_failed",
            "cannot read the git log; run dx release inside a git checkout",
        );
    };
    let report = match plan(
        workspace,
        &config,
        tag.as_deref(),
        &log,
        invocation.to.as_deref(),
    ) {
        Ok(report) => report,
        Err(message) => return refusal(invocation.output, out, err, &message),
    };
    match check_stdout_write(out.write_all(report.render().as_bytes())) {
        Ok(()) => 0,
        Err(exit) => exit,
    }
}
