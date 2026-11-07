//! `dx update advisory`: moves the advisory pins in `.dx/pins` to the newest published release.

use super::common::*;
use super::update_outdated::registry_fetch;
use super::update_pr::{
    ACTIONS_PR_REFUSAL, BOT_EMAIL, BOT_NAME, Tools, WORKFLOW_SKIP, open_update_prs, program, push,
};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, command_started, notice_event,
};
use dx_plan::args::{Command, Invocation};
use dx_update::advisory::{self, BRANCH, Release};
use dx_update::discover::Fetch;
use std::io::Write;
use std::path::Path;

pub(crate) const CODE_UPDATE_ADVISORY: &str = "update_advisory";
pub(crate) const CODE_UPDATE_ADVISORY_FAILED: &str = "update_advisory_failed";

const PINS: &str = ".dx/pins";

/// Whether the invocation is `dx update advisory`.
pub(super) fn selected(invocation: &Invocation) -> bool {
    invocation.command == Command::Update
        && !invocation.outdated
        && invocation.targets == [advisory::SET]
}

pub(crate) fn execute_update_advisory(invocation: &Invocation, env: Env<'_>) -> i32 {
    let fetch = registry_fetch();
    run(invocation, env, fetch.as_ref(), &Tools::system())
}

fn run(invocation: &Invocation, env: Env<'_>, fetch: &dyn Fetch, tools: &Tools) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ci,
        audit_today,
        actions,
        proc_root,
        edit_commit_json,
    } = env;
    let json = invocation.output == OutputMode::Json;
    let fail = |out: &mut dyn Write, err: &mut dyn Write, reason: &str| {
        operational_output(
            invocation.output,
            out,
            err,
            CODE_UPDATE_ADVISORY_FAILED,
            reason,
        )
    };
    let before = match std::fs::read_to_string(workspace.join(PINS)) {
        Ok(text) => text,
        Err(error) => return fail(out, err, &format!("cannot read {PINS}: {error}")),
    };
    let release = match fetch
        .get(advisory::LATEST_RELEASE_URL)
        .and_then(|text| advisory::parse_release(&text))
    {
        Ok(release) => release,
        Err(reason) => {
            return fail(
                out,
                err,
                &format!("cannot read the newest release: {reason}"),
            );
        }
    };
    let after = advisory::rewrite(&before, &release);
    let today = super::audit::today_utc(audit_today);
    let current = advisory::pins(&before);
    let mut lines = advisory::status(&current, &release, &today);
    let diff = advisory::diff(&before, &after);

    if json && let Err(code) = emit_built(out, command_started("update", "default")) {
        return code;
    }
    let mut security = || -> Vec<String> {
        let words = ["security".to_owned(), "--output=json".to_owned()];
        let Ok(inner) = dx_plan::args::parse(&words) else {
            return Vec::new();
        };
        let mut step_out = Vec::new();
        let mut step_err = Vec::new();
        super::dispatch_inner(
            &inner,
            Env {
                workspace,
                runner,
                query_runner,
                temp_dir,
                pid,
                nonce,
                out: &mut step_out,
                err: &mut step_err,
                ci,
                audit_today,
                actions,
                proc_root,
                edit_commit_json,
            },
        );
        vulnerabilities(&String::from_utf8_lossy(&step_out))
    };

    let result = if invocation.pr {
        open_pull_request(
            workspace,
            tools,
            invocation,
            &release,
            &after,
            &diff,
            &mut security,
        )
    } else if invocation.applies() {
        write_pins(workspace, &after).map(|()| {
            if diff.is_empty() {
                "the advisory pins are current".to_owned()
            } else {
                format!("moved the advisory pins to release {}", release.tag)
            }
        })
    } else {
        lines.push(format!("{BRANCH}: would be {}", plan_word(&diff)));
        lines.extend(diff.iter().cloned());
        if !diff.is_empty() {
            let old = security();
            let swap = write_pins(workspace, &after);
            let new = if swap.is_ok() { security() } else { Vec::new() };
            let restore = write_pins(workspace, &before);
            if let Err(reason) = swap.and(restore) {
                return fail(out, err, &reason);
            }
            let found = advisory::new_findings(&old, &new);
            lines.push(format!("new advisories: {}", found.len()));
            lines.extend(found.iter().map(|line| format!("  {line}")));
        }
        Ok(String::new())
    };
    let exit = i32::from(result.is_err());
    match &result {
        Ok(message) if !message.is_empty() => lines.push(message.clone()),
        Err(reason) => lines.push(format!("failed: {reason}")),
        Ok(_) => {}
    }
    for line in &lines {
        let written = if json {
            emit_built(
                out,
                notice_event(&NoticeEvent::info(CODE_UPDATE_ADVISORY, line.clone())),
            )
        } else {
            check_stdout_write(writeln!(out, "{line}"))
        };
        if let Err(code) = written {
            return code;
        }
    }
    if json
        && let Err(code) = emit_built(
            out,
            Ok(command_finished(
                exit,
                &FinishedCounts {
                    results_complete: Some(exit == 0),
                    ..FinishedCounts::default()
                },
            )),
        )
    {
        return code;
    }
    exit
}

fn plan_word(diff: &[String]) -> &'static str {
    if diff.is_empty() {
        "left alone: the pins are current"
    } else {
        "opened or refreshed by --pr"
    }
}

fn write_pins(workspace: &Path, text: &str) -> Result<(), String> {
    std::fs::write(workspace.join(PINS), text)
        .map_err(|error| format!("cannot write {PINS}: {error}"))
}

/// The vulnerability messages of a `dx security --output=json` stream.
fn vulnerabilities(stream: &str) -> Vec<String> {
    stream
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|event| event.get("message")?.as_str().map(str::to_owned))
        .filter(|message| message.contains(" vulnerable to ") || message.contains(" advisory "))
        .collect()
}

fn open_pull_request(
    root: &Path,
    tools: &Tools,
    invocation: &Invocation,
    release: &Release,
    after: &str,
    diff: &[String],
    security: &mut dyn FnMut() -> Vec<String>,
) -> Result<String, String> {
    let git = |args: &[&str]| program(&tools.git, root, args);
    let gh = |args: &[&str]| program(&tools.gh, root, args);
    if !git(&["status", "--porcelain"])?.trim().is_empty() {
        return Err("the working tree has changes; commit or stash them first".to_owned());
    }
    let base = git(&["rev-parse", "--abbrev-ref", "HEAD"])?
        .trim()
        .to_owned();
    if base == "HEAD" {
        return Err("HEAD is detached; check out a branch first".to_owned());
    }
    let sha = git(&["rev-parse", "HEAD"])?.trim().to_owned();
    let existing = open_update_prs(tools, root, &base)?
        .into_iter()
        .find(|(_, head)| head == BRANCH)
        .map(|(number, _)| number.to_string());
    if diff.is_empty() {
        return match existing {
            Some(number) => gh(&[
                "pr",
                "close",
                &number,
                "--delete-branch",
                "--comment",
                "dx update: the advisory pins are current.",
            ])
            .map(|_| format!("closed #{number}")),
            None => Ok("the advisory pins are current".to_owned()),
        };
    }
    let old = security();
    git(&["checkout", "-B", BRANCH, &sha])?;
    let built = (|| {
        write_pins(root, after)?;
        let new = security();
        let body = advisory::pr_body(release, diff, &advisory::new_findings(&old, &new));
        let title = format!("Update advisory snapshots to {}", release.tag);
        git(&["add", "-A"])?;
        let mut commit = Vec::new();
        if git(&["config", "user.email"]).map_or(true, |email| email.trim().is_empty()) {
            commit.extend([
                "-c".to_owned(),
                format!("user.name={BOT_NAME}"),
                "-c".to_owned(),
                format!("user.email={BOT_EMAIL}"),
            ]);
        }
        commit.extend(["commit".to_owned(), "-m".to_owned(), title.clone()]);
        let commit: Vec<&str> = commit.iter().map(String::as_str).collect();
        git(&commit)?;
        if !push(tools, root, BRANCH)? {
            return Ok(WORKFLOW_SKIP.to_owned());
        }
        if let Some(number) = &existing {
            gh(&["pr", "edit", number, "--title", &title, "--body", &body])?;
            return Ok(format!("updated #{number}"));
        }
        let labels = invocation.labels.clone().unwrap_or_default();
        let mut args = vec![
            "pr", "create", "--base", &base, "--head", BRANCH, "--title", &title, "--body", &body,
        ];
        for label in labels
            .split(',')
            .map(str::trim)
            .filter(|label| !label.is_empty())
        {
            args.extend(["--label", label]);
        }
        let created = gh(&args).map_err(|error| {
            if error.contains(ACTIONS_PR_REFUSAL) {
                format!("{error}\nturn on the repository setting \"Allow GitHub Actions to create and approve pull requests\" (Settings, Actions, General)")
            } else {
                error
            }
        })?;
        Ok(format!("opened {}", created.trim()))
    })();
    let _ = git(&["checkout", "-f", &base]);
    built
}

#[cfg(test)]
mod tests {
    #![cfg(unix)]
    use super::*;

    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn release_json() -> String {
        let body = format!(
            "advisory cargo 2026-11-05 {SHA_B} https://example.test/2026-11-05/cargo.json\nadvisory trivy 2026-11-05 {SHA_B} https://example.test/2026-11-05/trivy.tar.gz\n"
        );
        serde_json::json!({"tag_name": "2026-11-05", "body": body}).to_string()
    }

    fn pins_text() -> String {
        format!(
            "0.0.0\nadvisory cargo 2026-10-01 {SHA_A} https://example.test/2026-10-01/cargo.json\nadvisory npm 2026-10-01 {SHA_A} https://example.test/2026-10-01/npm.json\n"
        )
    }

    fn run_words(words: &[&str]) -> (i32, String, String) {
        let harness = super::super::test_support::Harness::new("update-advisory");
        let root = &harness.workspace;
        std::fs::create_dir_all(root.join(".dx")).expect("mkdir");
        std::fs::write(root.join(PINS), pins_text()).expect("pins");
        let registry = harness.temp.join("registry");
        let release =
            registry.join("api.github.com/repos/devhusdk/rules_dx-advisory/releases/latest");
        std::fs::create_dir_all(release.parent().expect("parent")).expect("mkdir");
        std::fs::write(&release, release_json()).expect("release");
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        let inv = dx_plan::args::parse(&words).expect("parse");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(
            &inv,
            harness.env(&runner, &mut out, &mut err),
            &super::super::update_outdated::DirFetch(registry),
            &Tools::system(),
        );
        let pins = std::fs::read_to_string(root.join(PINS)).expect("pins");
        (
            code,
            format!(
                "{}{}",
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            ),
            pins,
        )
    }

    #[test]
    fn apply_rewrites_only_the_advisory_lines_with_a_new_pin() {
        let (code, out, pins) = run_words(&["update", "advisory", "--apply"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("moved the advisory pins to release 2026-11-05"),
            "{out}"
        );
        let expected = format!(
            "0.0.0\nadvisory cargo 2026-11-05 {SHA_B} https://example.test/2026-11-05/cargo.json\nadvisory npm 2026-10-01 {SHA_A} https://example.test/2026-10-01/npm.json\n"
        );
        assert_eq!(pins, expected);
    }

    #[test]
    fn check_reports_age_release_and_the_planned_diff_without_writing() {
        let (code, out, pins) = run_words(&["update", "advisory"]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("advisory:cargo 2026-10-01"), "{out}");
        assert!(out.contains("newest release 2026-11-05"), "{out}");
        assert!(
            out.contains("dx-update/advisory: would be opened or refreshed by --pr"),
            "{out}"
        );
        assert!(out.contains("- advisory cargo 2026-10-01"), "{out}");
        assert!(out.contains("+ advisory cargo 2026-11-05"), "{out}");
        assert!(out.contains("new advisories: "), "{out}");
        assert_eq!(pins, pins_text());
    }

    #[test]
    fn vulnerabilities_keep_only_advisory_messages() {
        let stream = "{\"message\":\"a@1 vulnerable to X (severity high; fixed in 2)\"}\n{\"message\":\"done\"}\nnot json\n";
        assert_eq!(
            vulnerabilities(stream),
            vec!["a@1 vulnerable to X (severity high; fixed in 2)".to_owned()]
        );
    }
}
