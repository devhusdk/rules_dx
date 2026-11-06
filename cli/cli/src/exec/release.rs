//! `dx release`: the next version from conventional commits and the manifests that carry it.

use std::path::Path;
use std::process::Command as Process;

use super::common::*;
use dx_plan::args::Invocation;
use dx_update::release::{
    apply, changelog_section, lint, plan, prepend_section, read_config, tag_message, today,
};

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
    if let Some(range) = invocation.commits.as_deref() {
        return lint_commits(invocation, workspace, &config, range, out, err);
    }
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
    if let Err(exit) = check_stdout_write(out.write_all(report.render().as_bytes())) {
        return exit;
    }
    if !invocation.apply {
        if let (true, Some(next)) = (config.changelog, report.next.as_deref()) {
            let section = changelog_section(next, &today(), &log);
            let text = format!("CHANGELOG.md would gain:\n\n{section}");
            if let Err(exit) = check_stdout_write(out.write_all(text.as_bytes())) {
                return exit;
            }
        }
        return 0;
    }
    let Some(next) = report.next.as_deref() else {
        return refusal(
            invocation.output,
            out,
            err,
            "nothing to release: no feat, fix or breaking commits; pass --to <x.y.z> to force a version",
        );
    };
    match commit_and_tag(workspace, &config, next, &log, invocation.push) {
        Ok(tag) => {
            let line = if invocation.push {
                format!("committed, tagged and pushed {tag}")
            } else {
                format!("committed and tagged {tag}\nnext: git push --follow-tags")
            };
            match check_stdout_write(writeln!(out, "{line}")) {
                Ok(()) => 0,
                Err(exit) => exit,
            }
        }
        Err(message) => refusal(invocation.output, out, err, &message),
    }
}

fn lint_commits(
    invocation: &Invocation,
    workspace: &Path,
    config: &dx_update::release::Config,
    range: &str,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> i32 {
    if invocation.apply || invocation.to.is_some() {
        return refusal(
            invocation.output,
            out,
            err,
            "--commits only checks; it cannot be combined with --apply or --to",
        );
    }
    let Some(log) = git(
        workspace,
        &["log", "--no-merges", "--format=%H%x00%s%x00%b%x1e", range],
    ) else {
        return operational(
            invocation,
            out,
            err,
            "release_git_failed",
            &format!("cannot read the commits in {range}; check the range and that this is a git checkout"),
        );
    };
    let bad = lint(config, &log);
    if bad.is_empty() {
        return 0;
    }
    let mut text = String::from("commits that do not follow conventional commits:\n");
    for (hash, subject) in &bad {
        text.push_str(&format!("  {} {subject}\n", &hash[..hash.len().min(12)]));
    }
    if let Err(exit) = check_stdout_write(out.write_all(text.as_bytes())) {
        return exit;
    }
    1
}

fn refresh_locks(workspace: &Path, changed: &[String]) -> Result<Vec<String>, String> {
    let mut locks = Vec::new();
    for path in changed
        .iter()
        .filter(|path| path.rsplit('/').next() == Some("MODULE.bazel"))
    {
        let dir = Path::new(path).parent().unwrap_or(Path::new(""));
        let status = Process::new("bazel")
            .args(["mod", "deps", "--lockfile_mode=update"])
            .current_dir(workspace.join(dir))
            .status()
            .map_err(|error| format!("cannot run bazel mod deps for {path}: {error}"))?;
        if !status.success() {
            return Err(format!(
                "bazel mod deps failed for {path}; the manifests are already rewritten"
            ));
        }
        let lock = dir.join("MODULE.bazel.lock");
        if workspace.join(&lock).is_file() {
            locks.push(lock.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(locks)
}

fn commit_and_tag(
    workspace: &Path,
    config: &dx_update::release::Config,
    version: &str,
    log: &str,
    push: bool,
) -> Result<String, String> {
    let tag = format!("{}{version}", config.tag_prefix);
    let branch = git(workspace, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .ok_or("HEAD is detached; check out a branch first")?;
    let status = git(workspace, &["status", "--porcelain"]).ok_or("cannot run git status")?;
    if !status.trim().is_empty() {
        return Err("the working tree has uncommitted changes; commit or stash them first".into());
    }
    if git(
        workspace,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/tags/{tag}"),
        ],
    )
    .is_some()
    {
        return Err(format!("tag {tag} already exists"));
    }
    let changed = apply(workspace, version)?;
    let mut files = changed.clone();
    if config.changelog {
        let path = workspace.join("CHANGELOG.md");
        let existing = std::fs::read_to_string(&path).ok();
        let section = changelog_section(version, &today(), log);
        std::fs::write(&path, prepend_section(existing.as_deref(), &section))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        files.push("CHANGELOG.md".to_owned());
    }
    files.extend(refresh_locks(workspace, &changed)?);
    let mut add = vec!["add", "--"];
    add.extend(files.iter().map(String::as_str));
    git(workspace, &add).ok_or("git add failed")?;
    let message = format!("release: {tag}");
    git(workspace, &["commit", "-m", &message]).ok_or("git commit failed")?;
    let notes = tag_message(&tag, log);
    git(workspace, &["tag", "-a", &tag, "-m", &notes])
        .ok_or_else(|| format!("git tag {tag} failed"))?;
    if push {
        git(workspace, &["push", "origin", &branch, &tag])
            .ok_or_else(|| format!("git push of {branch} and {tag} failed"))?;
    }
    Ok(tag)
}
