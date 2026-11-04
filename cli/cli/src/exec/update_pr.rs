use super::common::*;
use super::update_outdated::{collect, registry_fetch, Builtins, Collected};
use dx_adopt::dependency_sets;
use dx_output::{
    command_finished, command_started, error_event, notice_event, FinishedCounts, NoticeEvent,
    OutputMode,
};
use dx_plan::args::Invocation;
use dx_update::discover::Fetch;
use dx_update::pr::{body, lock_numstat, plan, Group, GroupPlan, Item};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const CODE_UPDATE_PR: &str = "update_pr";
pub(crate) const CODE_UPDATE_PR_FAILED: &str = "update_pr_failed";
pub(crate) const CODE_UPDATE_PR_LOOKUP: &str = "update_pr_lookup_failed";

const BOT_NAME: &str = "github-actions[bot]";
const BOT_EMAIL: &str = "41898282+github-actions[bot]@users.noreply.github.com";

pub(crate) struct Tools {
    git: PathBuf,
    gh: PathBuf,
}

impl Tools {
    fn system() -> Tools {
        Tools {
            git: PathBuf::from("git"),
            gh: PathBuf::from("gh"),
        }
    }
}

type Step<'a> = dyn FnMut(&[String]) -> Result<(), String> + 'a;

struct Outcome {
    branch: String,
    result: Result<String, String>,
}

pub(crate) fn execute_update_pr(invocation: &Invocation, env: Env<'_>) -> i32 {
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
    let registry = match dependency_sets::load(workspace) {
        Ok(Some(registry)) => registry,
        Ok(None) => return pre_exec(invocation.output, out, err, "no dependency sets"),
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let resolved = match dependency_sets::resolve(&registry, &invocation.targets) {
        Ok(resolved) => resolved,
        Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
    };
    let json = invocation.output == OutputMode::Json;
    if json {
        if let Err(exit) = emit_built(out, command_started("update", "default")) {
            return exit;
        }
    }
    let Collected {
        found, registries, ..
    } = collect(workspace, &resolved, &Builtins::default(), fetch);
    let items: Vec<Item> = found
        .candidates
        .iter()
        .filter_map(|candidate| {
            registries.get(&candidate.set).map(|registry| Item {
                candidate: candidate.clone(),
                registry: *registry,
            })
        })
        .collect();
    let group = invocation
        .group
        .as_deref()
        .and_then(Group::parse)
        .unwrap_or_default();
    let labels: Vec<String> = invocation
        .labels
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .collect();
    let plans = plan(&items, group);
    let mut step = |words: &[String]| -> Result<(), String> {
        let inner = dx_plan::args::parse(words).map_err(|error| format!("{error:?}"))?;
        let mut step_out = Vec::new();
        let mut step_err = Vec::new();
        let code = super::dispatch_command(
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
        if code == 0 {
            return Ok(());
        }
        let mut text = String::from_utf8_lossy(&step_err).into_owned();
        text.push_str(&String::from_utf8_lossy(&step_out));
        Err(format!(
            "dx {} exited {code}: {}",
            words.join(" "),
            tail(&text)
        ))
    };
    let outcomes = match publish(workspace, tools, &plans, &labels, &mut step) {
        Ok(outcomes) => outcomes,
        Err(reason) => {
            return operational_output(invocation.output, out, err, CODE_UPDATE_PR_FAILED, &reason);
        }
    };
    let failed = outcomes.iter().any(|outcome| outcome.result.is_err());
    let exit = i32::from(failed || !found.errors.is_empty());
    for error in &found.errors {
        let _ = writeln!(
            err,
            "dx: {CODE_UPDATE_PR_LOOKUP}: {}:{} ({})",
            error.set, error.package, error.reason
        );
    }
    if let Err(code) = emit(out, &outcomes, json, exit) {
        return code;
    }
    exit
}

fn tail(text: &str) -> String {
    let text = text.trim();
    let start = text.len().saturating_sub(300);
    let start = (start..=text.len())
        .find(|i| text.is_char_boundary(*i))
        .unwrap_or(text.len());
    text[start..].to_owned()
}

fn emit(out: &mut dyn Write, outcomes: &[Outcome], json: bool, exit: i32) -> Result<(), i32> {
    if outcomes.is_empty() && !json {
        return check_stdout_write(writeln!(out, "nothing is outdated"));
    }
    for outcome in outcomes {
        match (&outcome.result, json) {
            (Ok(message), true) => emit_built(
                out,
                notice_event(&NoticeEvent {
                    scope: Some(vec![outcome.branch.clone()]),
                    ..NoticeEvent::info(CODE_UPDATE_PR, format!("{}: {message}", outcome.branch))
                }),
            )?,
            (Err(reason), true) => emit_built(
                out,
                error_event(
                    CODE_UPDATE_PR_FAILED,
                    &format!("{}: {reason}", outcome.branch),
                    None,
                    None,
                    Some("execute"),
                ),
            )?,
            (Ok(message), false) => {
                check_stdout_write(writeln!(out, "{}: {message}", outcome.branch))?;
            }
            (Err(reason), false) => {
                check_stdout_write(writeln!(out, "{}: failed: {reason}", outcome.branch))?;
            }
        }
    }
    if json {
        emit_built(
            out,
            Ok(command_finished(
                exit,
                &FinishedCounts {
                    results_complete: Some(exit == 0),
                    ..FinishedCounts::default()
                },
            )),
        )?;
    }
    Ok(())
}

fn program(program: &Path, root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("{} {}: {error}", program.display(), args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "{} {} failed: {}",
            program.display(),
            args.join(" "),
            tail(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn publish(
    root: &Path,
    tools: &Tools,
    plans: &[GroupPlan],
    labels: &[String],
    step: &mut Step<'_>,
) -> Result<Vec<Outcome>, String> {
    if plans.is_empty() {
        return Ok(Vec::new());
    }
    let git = |args: &[&str]| program(&tools.git, root, args);
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
    let mut outcomes = Vec::new();
    for group in plans {
        let result = publish_one(root, tools, group, labels, &base, &sha, step);
        if result.is_err() {
            let _ = git(&["reset", "--hard", &sha]);
            let _ = git(&["clean", "-fd"]);
        }
        outcomes.push(Outcome {
            branch: group.branch.clone(),
            result,
        });
    }
    git(&["checkout", &base])?;
    Ok(outcomes)
}

fn publish_one(
    root: &Path,
    tools: &Tools,
    group: &GroupPlan,
    labels: &[String],
    base: &str,
    sha: &str,
    step: &mut Step<'_>,
) -> Result<String, String> {
    let git = |args: &[&str]| program(&tools.git, root, args);
    let branch = group.branch.as_str();
    git(&["checkout", "-B", branch, sha])?;
    let mut sets: Vec<&str> = Vec::new();
    for item in &group.items {
        let c = &item.candidate;
        step(&[
            "update".to_owned(),
            format!("{}:{}", c.set, c.package),
            c.latest.clone(),
            "--apply".to_owned(),
        ])?;
        if !sets.contains(&c.set.as_str()) {
            sets.push(&c.set);
        }
    }
    for set in sets {
        step(&["update".to_owned(), "--apply".to_owned(), set.to_owned()])?;
    }
    step(&["check".to_owned(), "--affected".to_owned()])?;
    step(&["test".to_owned(), "--affected".to_owned()])?;
    if git(&["status", "--porcelain"])?.trim().is_empty() {
        return Ok("nothing to change".to_owned());
    }
    git(&["add", "-A"])?;
    let identity = git(&["config", "user.email"]).map_or(true, |email| email.trim().is_empty());
    let mut commit = Vec::new();
    if identity {
        commit.extend([
            "-c".to_owned(),
            format!("user.name={BOT_NAME}"),
            "-c".to_owned(),
            format!("user.email={BOT_EMAIL}"),
        ]);
    }
    commit.extend(["commit".to_owned(), "-m".to_owned(), group.title.clone()]);
    let commit: Vec<&str> = commit.iter().map(String::as_str).collect();
    git(&commit)?;
    let numstat = git(&["diff", "--numstat", sha, "HEAD"])?;
    let text = body(group, &lock_numstat(&numstat));
    let _ = git(&[
        "fetch",
        "origin",
        &format!("+refs/heads/{branch}:refs/remotes/origin/{branch}"),
    ]);
    git(&["push", "--force-with-lease", "origin", branch])?;
    let gh = |args: &[&str]| program(&tools.gh, root, args);
    let listed = gh(&[
        "pr", "list", "--head", branch, "--state", "open", "--json", "number",
    ])?;
    let number = serde_json::from_str::<serde_json::Value>(&listed)
        .ok()
        .and_then(|value| value.get(0)?.get("number")?.as_u64());
    if let Some(number) = number {
        let number = number.to_string();
        gh(&[
            "pr",
            "edit",
            &number,
            "--title",
            &group.title,
            "--body",
            &text,
        ])?;
        return Ok(format!("updated #{number}"));
    }
    let mut args = vec![
        "pr",
        "create",
        "--base",
        base,
        "--head",
        branch,
        "--title",
        &group.title,
        "--body",
        &text,
    ];
    for label in labels {
        args.extend(["--label", label]);
    }
    let created = gh(&args)?;
    Ok(format!("opened {}", created.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_update::discover::{Candidate, Registry};
    use dx_update::pr::Group;

    fn sh(root: &Path, args: &[&str]) -> String {
        program(Path::new("git"), root, args).expect("git")
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        work: PathBuf,
        origin: PathBuf,
        tools: Tools,
        argv: PathBuf,
    }

    fn fixture(list_output: &str) -> Fixture {
        let dir = super::super::test_support::temp_dir("update-pr");
        let origin = dir.path().join("origin.git");
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).expect("mkdir");
        sh(
            dir.path(),
            &["init", "-q", "--bare", "-b", "main", "origin.git"],
        );
        sh(&work, &["init", "-q", "-b", "main"]);
        sh(&work, &["config", "user.email", "t@example.com"]);
        sh(&work, &["config", "user.name", "t"]);
        std::fs::write(work.join("Cargo.lock"), "a\n").expect("write");
        sh(&work, &["add", "-A"]);
        sh(&work, &["commit", "-q", "-m", "init"]);
        sh(
            &work,
            &["remote", "add", "origin", origin.to_str().expect("utf8")],
        );
        sh(&work, &["push", "-q", "origin", "main"]);
        let argv = dir.path().join("gh-argv");
        let script = dir.path().join("gh");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\nif [ \"$2\" = list ]; then echo '{list_output}'; fi\nif [ \"$2\" = create ]; then echo https://example.com/pr/9; fi\n",
                argv.display()
            ),
        )
        .expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        Fixture {
            tools: Tools {
                git: PathBuf::from("git"),
                gh: script,
            },
            _dir: dir,
            work,
            origin,
            argv,
        }
    }

    fn plans() -> Vec<GroupPlan> {
        plan(
            &[Item {
                candidate: Candidate {
                    set: "cargo".into(),
                    package: "serde".into(),
                    current: "1.0.0".into(),
                    latest: "2.0.0".into(),
                    published: None,
                },
                registry: Registry::Crates,
            }],
            Group::Set,
        )
    }

    fn editing(
        work: PathBuf,
        fail: Option<&'static str>,
    ) -> impl FnMut(&[String]) -> Result<(), String> {
        move |words: &[String]| {
            if fail == Some(words[0].as_str()) {
                return Err(format!("{} failed", words[0]));
            }
            if words.contains(&"--apply".to_owned()) && words.len() == 3 {
                std::fs::write(work.join("Cargo.lock"), "a\nb\n").map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    #[test]
    fn creates_a_pull_request_with_labels() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        let got =
            publish(&fx.work, &fx.tools, &plans(), &["deps".into()], &mut step).expect("publish");
        assert_eq!(got[0].result, Ok("opened https://example.com/pr/9".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr create --base main --head dx-update/cargo"));
        assert!(argv.contains("--label deps"));
        assert!(argv.contains("cargo:serde 1.0.0 -> 2.0.0"));
        assert!(argv.contains("| Cargo.lock | 1 | 0 |"));
        let pushed = sh(&fx.origin, &["rev-parse", "--verify", "dx-update/cargo"]);
        assert!(!pushed.trim().is_empty());
        assert_eq!(
            sh(&fx.work, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
            "main"
        );
    }

    #[test]
    fn edits_an_open_pull_request() {
        let fx = fixture("[{\"number\":7}]");
        let mut step = editing(fx.work.clone(), None);
        let got = publish(&fx.work, &fx.tools, &plans(), &[], &mut step).expect("publish");
        assert_eq!(got[0].result, Ok("updated #7".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr edit 7 --title Update cargo dependencies"));
        assert!(!argv.contains("pr create"));
    }

    #[test]
    fn a_failed_verify_pushes_nothing() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), Some("test"));
        let got = publish(&fx.work, &fx.tools, &plans(), &[], &mut step).expect("publish");
        assert!(got[0].result.is_err());
        assert!(!fx.argv.exists());
        let remote = sh(&fx.origin, &["branch", "--list", "dx-update/*"]);
        assert!(remote.trim().is_empty());
        assert_eq!(sh(&fx.work, &["status", "--porcelain"]).trim(), "");
        assert_eq!(
            sh(&fx.work, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
            "main"
        );
    }

    #[test]
    fn a_dirty_tree_is_refused() {
        let fx = fixture("[]");
        std::fs::write(fx.work.join("Cargo.lock"), "dirty\n").expect("write");
        let mut step = editing(fx.work.clone(), None);
        let got = publish(&fx.work, &fx.tools, &plans(), &[], &mut step);
        assert!(got.is_err());
    }

    #[test]
    fn nothing_outdated_prints_one_line() {
        let mut out = Vec::new();
        emit(&mut out, &[], false, 0).expect("emit");
        assert_eq!(String::from_utf8_lossy(&out), "nothing is outdated\n");
    }
}
