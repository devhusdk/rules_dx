use super::common::*;
use super::update_outdated::{Collected, collect, registry_fetch, registry_for, split_builtin};
use dx_adopt::dependency_sets::{self, Selection};
use dx_output::{
    FinishedCounts, NoticeEvent, OutputMode, command_finished, command_started, error_event,
    notice_event,
};
use dx_plan::args::Invocation;
use dx_update::discover::Fetch;
use dx_update::policy::{Level, Policy, automerge, filter};
use dx_update::pr::{GroupPlan, Item, Scope, body, lock_numstat, pick_tags, plan};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const CODE_UPDATE_PR: &str = "update_pr";
pub(crate) const CODE_UPDATE_PR_FAILED: &str = "update_pr_failed";
pub(crate) const CODE_UPDATE_PR_SKIPPED: &str = "update_pr_skipped";
pub(crate) const CODE_UPDATE_PR_LOOKUP: &str = "update_pr_lookup_failed";

pub(super) const BOT_NAME: &str = "github-actions[bot]";
pub(super) const BOT_EMAIL: &str = "41898282+github-actions[bot]@users.noreply.github.com";

pub(crate) struct Tools {
    pub(super) git: PathBuf,
    pub(super) gh: PathBuf,
}

impl Tools {
    pub(super) fn system() -> Tools {
        Tools {
            git: PathBuf::from("git"),
            gh: PathBuf::from("gh"),
        }
    }
}

const DEFAULT_MIN_AGE: u32 = 3;
const DEFAULT_MAX_PRS: usize = 5;

struct Rules {
    automerge: Option<Level>,
    max_prs: usize,
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
    let (builtins, selectors) = split_builtin(&invocation.targets);
    let only_builtins = !invocation.targets.is_empty() && selectors.is_empty();
    let resolved = if only_builtins {
        Vec::new()
    } else {
        match dependency_sets::load(workspace) {
            Ok(Some(registry)) => match dependency_sets::resolve(&registry, &selectors) {
                Ok(resolved) => resolved,
                Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
            },
            Ok(None) if selectors.is_empty() => Vec::new(),
            Ok(None) => {
                let message = super::update::no_sets_message(workspace);
                return pre_exec(invocation.output, out, err, &message);
            }
            Err(error) => return pre_exec(invocation.output, out, err, &error.to_string()),
        }
    };
    let json = invocation.output == OutputMode::Json;
    if json && let Err(exit) = emit_built(out, command_started("update", "default")) {
        return exit;
    }
    let Collected {
        found, registries, ..
    } = collect(workspace, &resolved, &builtins, false, fetch);
    let min_age = invocation.min_age.unwrap_or(DEFAULT_MIN_AGE);
    let cutoff = (min_age > 0)
        .then(|| {
            let today = super::audit::today_utc(audit_today);
            let date = chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d").ok()?;
            let back = date.checked_sub_days(chrono::Days::new(u64::from(min_age)))?;
            Some(back.format("%Y-%m-%d").to_string())
        })
        .flatten();
    let policy = Policy {
        allow_major: invocation.allow_major,
        cutoff,
    };
    let (kept, skipped) = filter(&found.candidates, &policy);
    for skip in &skipped {
        let c = &skip.candidate;
        let line = format!(
            "skipped {}:{} {} -> {}: {}",
            c.set, c.package, c.current, c.latest, skip.reason
        );
        let written = if json {
            emit_built(
                out,
                notice_event(&NoticeEvent {
                    scope: Some(vec![format!("{}:{}", c.set, c.package)]),
                    ..NoticeEvent::info(CODE_UPDATE_PR_SKIPPED, line)
                }),
            )
        } else {
            check_stdout_write(writeln!(out, "{line}"))
        };
        if let Err(exit) = written {
            return exit;
        }
    }
    let rules = Rules {
        automerge: Level::limit(invocation.automerge.as_deref().unwrap_or("patch")),
        max_prs: invocation.max_prs.map_or(DEFAULT_MAX_PRS, usize::from),
    };
    let mut items: Vec<Item> = Vec::new();
    let mut listings: BTreeMap<String, String> = BTreeMap::new();
    for candidate in &kept {
        let registry = registry_for(&registries, candidate).ok_or("no registry");
        let registry = match registry {
            Ok(registry) => registry,
            Err(reason) => {
                let c = candidate;
                let line = format!(
                    "skipped {}:{} {} -> {}: {}",
                    c.set, c.package, c.current, c.latest, reason
                );
                let written = if json {
                    emit_built(
                        out,
                        notice_event(&NoticeEvent {
                            scope: Some(vec![format!("{}:{}", c.set, c.package)]),
                            ..NoticeEvent::info(CODE_UPDATE_PR_SKIPPED, line)
                        }),
                    )
                } else {
                    check_stdout_write(writeln!(out, "{line}"))
                };
                if let Err(exit) = written {
                    return exit;
                }
                continue;
            }
        };
        let mut candidate = candidate.clone();
        let mut untagged = false;
        if let (Some(repo), None) = (&candidate.repo, &candidate.tags) {
            let listing = listings.entry(repo.clone()).or_insert_with(|| {
                let url = format!("https://github.com/{repo}");
                program(
                    &tools.git,
                    workspace,
                    &["ls-remote", "--tags", "--refs", &url],
                )
                .unwrap_or_default()
            });
            candidate.tags = pick_tags(
                listing,
                &candidate.package,
                &candidate.current,
                &candidate.latest,
            );
            untagged = candidate.tags.is_none();
        }
        items.push(Item {
            candidate,
            registry,
            untagged,
        });
    }
    let plans = plan(&items);
    let locks: Vec<String> = resolved
        .iter()
        .flat_map(|target| target.set.locks.iter().cloned())
        .collect();
    let mut full_sets: Vec<String> = resolved
        .iter()
        .filter(|target| matches!(target.request, Selection::Full))
        .map(|target| target.set.name.clone())
        .collect();
    if builtins.selects_all_actions() {
        full_sets.push(dx_update::discover::github::ACTIONS_SET.to_owned());
    }
    if builtins.selects_all_containers() {
        full_sets.push(dx_update::discover::containers::SET.to_owned());
    }
    let scope = Scope::new(full_sets);
    let mut step = |words: &[String]| -> Result<(), String> {
        let inner = dx_plan::args::parse(words).map_err(|error| format!("{error:?}"))?;
        let mut step_out = Vec::new();
        let mut step_err = Vec::new();
        let code = super::dispatch_inner(
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
    let outcomes = match publish(workspace, tools, &plans, &scope, &rules, &locks, &mut step) {
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

pub(super) fn tail(text: &str) -> String {
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

const NETWORK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

pub(super) fn program(program: &Path, root: &Path, args: &[&str]) -> Result<String, String> {
    let label = format!("{} {}", program.display(), args.join(" "));
    dx_process::capture::run_checked_with(
        Command::new(program).args(args).current_dir(root),
        NETWORK_TIMEOUT,
        dx_process::capture::RunOptions {
            foreground: true,
            ..Default::default()
        },
    )
    .map(|output| output.stdout_text())
    .map_err(|error| match error {
        dx_process::capture::RunError::Failed { stderr, .. } => {
            format!("{label} failed: {}", tail(&stderr))
        }
        other => format!("{label}: {other}"),
    })
}

pub(super) const WORKFLOW_SKIP: &str = "skipped: token cannot push workflow files";

pub(super) const ACTIONS_PR_REFUSAL: &str = "not permitted to create or approve pull requests";
const AUTO_MERGE_HINT: &str = "turn on Allow auto-merge or pass --automerge=none";

pub(super) fn enable_auto_merge(
    gh: &dyn Fn(&[&str]) -> Result<String, String>,
    pr: &str,
) -> String {
    match gh(&["pr", "merge", pr, "--auto", "--squash"]) {
        Ok(_) => String::new(),
        Err(error) => format!("; auto-merge not enabled: {error} ({AUTO_MERGE_HINT})"),
    }
}

fn refused_workflow(stderr: &str) -> bool {
    let text = stderr.to_lowercase();
    text.contains("refusing to allow") && text.contains("workflow")
}

pub(super) fn push(tools: &Tools, root: &Path, branch: &str) -> Result<bool, String> {
    let args = ["push", "--force-with-lease", "origin", branch];
    let label = format!("{} {}", tools.git.display(), args.join(" "));
    let result = dx_process::capture::run_checked_with(
        Command::new(&tools.git).args(args).current_dir(root),
        NETWORK_TIMEOUT,
        dx_process::capture::RunOptions {
            foreground: true,
            ..Default::default()
        },
    );
    match result {
        Ok(_) => Ok(true),
        Err(dx_process::capture::RunError::Failed { stderr, .. }) => {
            if refused_workflow(&stderr) {
                return Ok(false);
            }
            Err(format!("{label} failed: {}", tail(&stderr)))
        }
        Err(other) => Err(format!("{label}: {other}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn publish(
    root: &Path,
    tools: &Tools,
    plans: &[GroupPlan],
    scope: &Scope,
    rules: &Rules,
    locks: &[String],
    step: &mut Step<'_>,
) -> Result<Vec<Outcome>, String> {
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
    let mut open = open_update_prs(tools, root, &base)?;
    let (stale, kept): (Vec<_>, Vec<_>) = open
        .drain(..)
        .partition(|(_, head)| scope.covers(head) && !plans.iter().any(|p| p.branch == *head));
    open = kept;
    let mut outcomes = Vec::new();
    for group in plans {
        let existing = open
            .iter()
            .find(|(_, head)| *head == group.branch)
            .map(|(number, _)| *number);
        if existing.is_none() && open.len() >= rules.max_prs {
            outcomes.push(Outcome {
                branch: group.branch.clone(),
                result: Ok(format!("skipped: --max-prs={} reached", rules.max_prs)),
            });
            continue;
        }
        let ctx = Target {
            base: &base,
            sha: &sha,
            existing,
            rules,
            locks,
        };
        let result = publish_one(root, tools, group, &ctx, step);
        if existing.is_none() && result.as_ref().is_ok_and(|text| text.starts_with("opened")) {
            open.push((0, group.branch.clone()));
        }
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
    for (number, head) in stale {
        let number = number.to_string();
        let result = program(
            &tools.gh,
            root,
            &[
                "pr",
                "close",
                &number,
                "--delete-branch",
                "--comment",
                "dx update: no updates left for this group.",
            ],
        )
        .map(|_| format!("closed #{number}"));
        outcomes.push(Outcome {
            branch: head,
            result,
        });
    }
    Ok(outcomes)
}

struct Target<'a> {
    base: &'a str,
    sha: &'a str,
    existing: Option<u64>,
    rules: &'a Rules,
    locks: &'a [String],
}

/// Lists open pull requests on `dx-update/` branches as number and branch.
pub(super) fn open_update_prs(
    tools: &Tools,
    root: &Path,
    base: &str,
) -> Result<Vec<(u64, String)>, String> {
    let listed = program(
        &tools.gh,
        root,
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--base",
            base,
            "--limit",
            "1000",
            "--json",
            "number,headRefName,isCrossRepository",
        ],
    )?;
    let value: serde_json::Value = serde_json::from_str(&listed).map_err(|e| e.to_string())?;
    Ok(value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|pr| {
            let head = pr.get("headRefName")?.as_str()?;
            let cross = pr.get("isCrossRepository").and_then(|v| v.as_bool());
            if !head.starts_with("dx-update/") || cross == Some(true) {
                return None;
            }
            Some((pr.get("number")?.as_u64()?, head.to_owned()))
        })
        .collect())
}

fn publish_one(
    root: &Path,
    tools: &Tools,
    group: &GroupPlan,
    target: &Target<'_>,
    step: &mut Step<'_>,
) -> Result<String, String> {
    let Target {
        base,
        sha,
        existing,
        rules,
        locks,
    } = *target;
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
        let builtin = c.set == dx_update::discover::github::ACTIONS_SET
            || c.set == dx_update::discover::containers::SET
            || (c.set == dx_update::discover::github::BAZELVERSION_SET
                && c.package == dx_update::discover::github::BAZELVERSION_PACKAGE);
        if !builtin && !sets.contains(&c.set.as_str()) {
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
    let text = body(group, &lock_numstat(&numstat, locks));
    let tracking = format!("refs/remotes/origin/{branch}");
    let fetched = git(&[
        "fetch",
        "origin",
        &format!("+refs/heads/{branch}:{tracking}"),
    ]);
    if fetched.is_err() {
        // The remote branch is gone; a stale tracking ref would fail the lease.
        let _ = git(&["update-ref", "-d", &tracking]);
    }
    if let Some(number) = existing {
        let remote = format!("origin/{branch}");
        let same_tree = git(&["rev-parse", &format!("{remote}^{{tree}}")])
            .ok()
            .zip(git(&["rev-parse", "HEAD^{tree}"]).ok())
            .is_some_and(|(a, b)| a == b);
        let same_parent = git(&["rev-parse", &format!("{remote}^")]).is_ok_and(|p| p.trim() == sha);
        if same_tree && same_parent {
            return Ok(format!("unchanged #{number}"));
        }
    }
    if !push(tools, root, branch)? {
        return Ok(WORKFLOW_SKIP.to_owned());
    }
    let gh = |args: &[&str]| program(&tools.gh, root, args);
    let merge = automerge(rules.automerge, group);
    if let Some(number) = existing {
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
        let mut note = String::new();
        if merge {
            note = enable_auto_merge(&gh, &number);
        } else {
            let _ = gh(&["pr", "merge", &number, "--disable-auto"]);
        }
        return Ok(format!("updated #{number}{note}"));
    }
    let args = vec![
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
    let created = gh(&args).map_err(|error| {
        if error.contains(ACTIONS_PR_REFUSAL) {
            format!("{error}\nturn on the repository setting \"Allow GitHub Actions to create and approve pull requests\" (Settings, Actions, General)")
        } else {
            error
        }
    })?;
    let note = if merge {
        enable_auto_merge(&gh, created.trim())
    } else {
        String::new()
    };
    Ok(format!("opened {}{note}", created.trim()))
}

#[cfg(test)]
mod tests {
    #![cfg(unix)]
    use super::*;
    use dx_update::discover::{Candidate, Registry};

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
        fixture_with_hook(list_output, None)
    }

    fn fixture_with_hook(list_output: &str, hook: Option<&str>) -> Fixture {
        fixture_failing(list_output, hook, None)
    }

    fn fixture_failing(
        list_output: &str,
        hook: Option<&str>,
        fail: Option<(&str, &str)>,
    ) -> Fixture {
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
        if let Some(message) = hook {
            let path = origin.join("hooks/pre-receive");
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\nwhile read old new ref; do\n  if [ \"$ref\" = refs/heads/dx-update/github-actions ]; then\n    echo '{message}' >&2\n    exit 1\n  fi\ndone\n"
                ),
            )
            .expect("hook");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("chmod");
            }
        }
        let argv = dir.path().join("gh-argv");
        let script = dir.path().join("gh");
        let failing = fail.map_or(String::new(), |(sub, message)| {
            format!("if [ \"$2\" = {sub} ]; then echo '{message}' >&2; exit 1; fi\n")
        });
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" >> '{}'\n{failing}if [ \"$2\" = list ]; then echo '{list_output}'; fi\nif [ \"$2\" = create ]; then echo https://example.com/pr/9; fi\n",
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
        plan(&[Item {
            candidate: Candidate {
                set: "cargo".into(),
                package: "serde".into(),
                current: "1.0.0".into(),
                latest: "2.0.0".into(),
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            },
            registry: Registry::Crates,
            untagged: false,
        }])
    }

    fn scope() -> Scope {
        Scope::new(vec!["cargo".into(), "npm".into()])
    }

    fn rules() -> Rules {
        Rules {
            automerge: Some(Level::Patch),
            max_prs: 5,
        }
    }

    fn plans_to(latest: &str) -> Vec<GroupPlan> {
        let mut got = plans();
        got[0].items[0].candidate.latest = latest.into();
        got
    }

    fn editing(
        work: PathBuf,
        fail: Option<&'static str>,
    ) -> impl FnMut(&[String]) -> Result<(), String> {
        move |words: &[String]| {
            if fail == Some(words[0].as_str()) {
                return Err(format!("{} failed", words[0]));
            }
            if words.contains(&"--apply".to_owned()) && words.len() >= 3 {
                std::fs::write(work.join("Cargo.lock"), "a\nb\n").map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    fn editing_version(work: PathBuf) -> impl FnMut(&[String]) -> Result<(), String> {
        move |words: &[String]| {
            if words.len() == 4 && words[3] == "--apply" {
                std::fs::write(work.join("Cargo.lock"), format!("a\n{}\n", words[2]))
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    #[test]
    fn creates_a_pull_request() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &["Cargo.lock".to_owned()],
            &mut step,
        )
        .expect("publish");
        assert_eq!(got[0].result, Ok("opened https://example.com/pr/9".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr create --base main --head dx-update/cargo"));
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
        let fx = fixture("[{\"number\":7,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing(fx.work.clone(), None);
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(got[0].result, Ok("updated #7".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr edit 7 --title Update cargo dependencies"));
        assert!(!argv.contains("pr create"));
    }

    #[test]
    fn a_patch_group_turns_on_auto_merge() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        let plans = plans_to("1.0.1");
        publish(
            &fx.work,
            &fx.tools,
            &plans,
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr merge https://example.com/pr/9 --auto --squash"));
    }

    #[test]
    fn an_auto_merge_refusal_keeps_the_pr() {
        let fx = fixture_failing(
            "[]",
            None,
            Some((
                "merge",
                "Pull request Auto merge is not allowed for this repository",
            )),
        );
        let mut step = editing(fx.work.clone(), None);
        let plans = plans_to("1.0.1");
        let outcomes = publish(
            &fx.work,
            &fx.tools,
            &plans,
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let result = outcomes[0].result.clone().expect("ok");
        assert!(result.starts_with("opened https://example.com/pr/9; auto-merge not enabled"));
        assert!(result.contains(AUTO_MERGE_HINT));
    }

    #[test]
    fn an_actions_pr_refusal_names_the_setting() {
        let fx = fixture_failing(
            "[]",
            None,
            Some((
                "create",
                "GitHub Actions is not permitted to create or approve pull requests",
            )),
        );
        let mut step = editing(fx.work.clone(), None);
        let plans = plans_to("1.0.1");
        let outcomes = publish(
            &fx.work,
            &fx.tools,
            &plans,
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let error = outcomes[0].result.clone().expect_err("error");
        assert!(error.contains("Allow GitHub Actions to create and approve pull requests"));
    }

    #[test]
    fn a_prerelease_group_is_not_merged() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        let mut plans = plans_to("1.0.1");
        plans[0].items[0].candidate.prerelease = true;
        publish(
            &fx.work,
            &fx.tools,
            &plans,
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(!argv.contains("pr merge"));
    }

    #[test]
    fn a_major_group_is_not_merged() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(!argv.contains("pr merge"));
    }

    #[test]
    fn a_refreshed_major_group_turns_auto_merge_off() {
        let fx = fixture("[{\"number\":7,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing(fx.work.clone(), None);
        publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr merge 7 --disable-auto"));
    }

    #[test]
    fn the_cap_skips_new_groups_only() {
        let fx = fixture("[{\"number\":3,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing(fx.work.clone(), None);
        let rules = Rules {
            automerge: None,
            max_prs: 1,
        };
        let got = publish(
            &fx.work,
            &fx.tools,
            &two_plans(),
            &two_scope(),
            &rules,
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(got[1].result, Ok("skipped: --max-prs=1 reached".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(!argv.contains("pr create"));
    }

    #[test]
    fn the_cap_ignores_stale_pull_requests() {
        let fx = fixture("[{\"number\":3,\"headRefName\":\"dx-update/npm\"}]");
        let mut step = editing(fx.work.clone(), None);
        let rules = Rules {
            automerge: None,
            max_prs: 1,
        };
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules,
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(got[0].result, Ok("opened https://example.com/pr/9".into()));
        assert_eq!(got[1].result, Ok("closed #3".into()));
    }

    #[test]
    fn the_cap_ignores_groups_with_nothing_to_change() {
        let fx = fixture("[]");
        let work = fx.work.clone();
        let mut step = move |words: &[String]| {
            let applying = words.contains(&"--apply".to_owned()) && words.len() >= 3;
            if applying && words[1].starts_with("github-actions") {
                std::fs::write(work.join("Cargo.lock"), "a\nb\n").map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let rules = Rules {
            automerge: None,
            max_prs: 1,
        };
        let got = publish(
            &fx.work,
            &fx.tools,
            &two_plans(),
            &two_scope(),
            &rules,
            &[],
            &mut step,
        )
        .expect("publish");
        let result = |branch: &str| {
            got.iter()
                .find(|o| o.branch == branch)
                .expect("outcome")
                .result
                .clone()
        };
        assert_eq!(result("dx-update/cargo"), Ok("nothing to change".into()));
        assert_eq!(
            result("dx-update/github-actions"),
            Ok("opened https://example.com/pr/9".into())
        );
    }

    #[test]
    fn a_failed_verify_pushes_nothing() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), Some("test"));
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert!(got[0].result.is_err());
        let argv = std::fs::read_to_string(&fx.argv).unwrap_or_default();
        assert!(!argv.contains("pr create") && !argv.contains("pr edit"));
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
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        );
        assert!(got.is_err());
    }

    #[test]
    fn closes_a_stale_pull_request() {
        let fx = fixture(
            "[{\"number\":5,\"headRefName\":\"dx-update/npm\"},{\"number\":6,\"headRefName\":\"dx-update/cargo\"},{\"number\":8,\"headRefName\":\"dx-update/bazel\"}]",
        );
        let mut step = editing(fx.work.clone(), None);
        let got = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(got[1].result, Ok("closed #5".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr close 5 --delete-branch"));
        assert!(!argv.contains("pr close 8") && !argv.contains("pr close 6"));
        assert!(argv.contains("--base main"));
    }

    #[test]
    fn an_empty_plan_still_closes_stale() {
        let fx = fixture("[{\"number\":5,\"headRefName\":\"dx-update/npm\"}]");
        let mut step = editing(fx.work.clone(), None);
        let got =
            publish(&fx.work, &fx.tools, &[], &scope(), &rules(), &[], &mut step).expect("publish");
        assert_eq!(got[0].result, Ok("closed #5".into()));
    }

    #[test]
    fn an_unchanged_branch_is_not_pushed_again() {
        let fx = fixture("[{\"number\":7,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing(fx.work.clone(), None);
        let first = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(first[0].result, Ok("updated #7".into()));
        let second = publish(
            &fx.work,
            &fx.tools,
            &plans(),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(second[0].result, Ok("unchanged #7".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert_eq!(argv.matches("pr edit").count(), 1);
    }

    #[test]
    fn a_moved_base_rewrites_the_branch() {
        let fx = fixture("[{\"number\":7,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing(fx.work.clone(), None);
        let publish_once = |step: &mut dyn FnMut(&[String]) -> Result<(), String>| {
            publish(&fx.work, &fx.tools, &plans(), &scope(), &rules(), &[], step).expect("publish")
        };
        publish_once(&mut step);
        sh(&fx.work, &["commit", "-q", "--allow-empty", "-m", "moved"]);
        sh(&fx.work, &["push", "-q", "origin", "main"]);
        let second = publish_once(&mut step);
        assert_eq!(second[0].result, Ok("updated #7".into()));
        let main = sh(&fx.work, &["rev-parse", "main"]);
        let parent = sh(&fx.origin, &["rev-parse", "dx-update/cargo^"]);
        assert_eq!(parent.trim(), main.trim());
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert_eq!(argv.matches("pr edit").count(), 2);
    }

    #[test]
    fn a_deleted_remote_branch_does_not_fail_the_lease() {
        let fx = fixture("[]");
        let mut step = editing(fx.work.clone(), None);
        let publish_once = |step: &mut dyn FnMut(&[String]) -> Result<(), String>| {
            publish(&fx.work, &fx.tools, &plans(), &scope(), &rules(), &[], step).expect("publish")
        };
        publish_once(&mut step);
        sh(&fx.origin, &["branch", "-D", "dx-update/cargo"]);
        let second = publish_once(&mut step);
        assert!(second[0].result.is_ok(), "{:?}", second[0].result);
        let pushed = sh(&fx.origin, &["rev-parse", "--verify", "dx-update/cargo"]);
        assert!(!pushed.trim().is_empty());
    }

    #[test]
    fn a_newer_version_rewrites_the_branch() {
        let fx = fixture("[{\"number\":7,\"headRefName\":\"dx-update/cargo\"}]");
        let mut step = editing_version(fx.work.clone());
        let first = publish(
            &fx.work,
            &fx.tools,
            &plans_to("2.0.0"),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(first[0].result, Ok("updated #7".into()));
        let second = publish(
            &fx.work,
            &fx.tools,
            &plans_to("2.1.0"),
            &scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        assert_eq!(second[0].result, Ok("updated #7".into()));
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("cargo:serde 1.0.0 -> 2.1.0"));
        assert_eq!(argv.matches("pr edit").count(), 2);
    }

    #[test]
    fn run_with_dir_registries_closes_stale_when_current() {
        let fx = fixture("[{\"number\":5,\"headRefName\":\"dx-update/crates\"}]");
        let harness = super::super::test_support::Harness::new("update-pr-run");
        let root = &harness.workspace;
        std::fs::write(
            root.join("MODULE.bazel"),
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//:Cargo.lock\", lockfile = \"//:cargo-bazel-lock.json\", manifests = [\"//:Cargo.toml\"])\nuse_repo(crate, \"crates\")\n",
        )
        .expect("module");
        std::fs::write(root.join("Cargo.toml"), "[dependencies]\nserde = \"1\"\n")
            .expect("manifest");
        std::fs::write(root.join("Cargo.lock"), "a\n").expect("lock");
        let registry = harness.temp.join("registry");
        let index = registry.join("index.crates.io/se/rd");
        std::fs::create_dir_all(&index).expect("mkdir");
        std::fs::write(
            index.join("serde"),
            "{\"vers\":\"1.0.0\",\"yanked\":false}\n",
        )
        .expect("index");
        sh(root, &["init", "-q", "-b", "main"]);
        sh(root, &["config", "user.email", "t@example.com"]);
        sh(root, &["config", "user.name", "t"]);
        sh(root, &["add", "-A"]);
        sh(root, &["commit", "-q", "-m", "init"]);
        let origin = harness.temp.join("origin.git");
        sh(
            &harness.temp,
            &["init", "-q", "--bare", "-b", "main", "origin.git"],
        );
        sh(
            root,
            &["remote", "add", "origin", origin.to_str().expect("utf8")],
        );
        sh(root, &["push", "-q", "origin", "main"]);
        let words = ["update".to_owned(), "--pr".to_owned()];
        let inv = dx_plan::args::parse(&words).expect("parse");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(
            &inv,
            harness.env(&runner, &mut out, &mut err),
            &super::super::update_outdated::DirFetch(registry),
            &fx.tools,
        );
        let out = String::from_utf8_lossy(&out);
        assert_eq!(code, 0, "{out}{}", String::from_utf8_lossy(&err));
        assert!(out.contains("closed #5"), "{out}");
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(argv.contains("pr close 5"));
    }

    /// Runs `dx update --pr` in a git workspace made of `files`, with `mirror` as the registry.
    fn run_pr_with(files: &[(&str, &str)], mirror: &[(&str, &str)]) -> (i32, String, String) {
        let (code, out, err, argv) = run_pr_full(files, mirror);
        (code, format!("{out}{err}"), argv)
    }

    fn run_pr_full(
        files: &[(&str, &str)],
        mirror: &[(&str, &str)],
    ) -> (i32, String, String, String) {
        let fx = fixture("[]");
        let harness = super::super::test_support::Harness::new("update-pr-all");
        let root = &harness.workspace;
        for (name, text) in files {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        }
        let registry = harness.temp.join("registry");
        for (name, text) in mirror {
            let path = registry.join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, text).expect("write");
        }
        sh(root, &["init", "-q", "-b", "main"]);
        sh(root, &["config", "user.email", "t@example.com"]);
        sh(root, &["config", "user.name", "t"]);
        sh(root, &["add", "-A"]);
        sh(root, &["commit", "-q", "-m", "init"]);
        let origin = harness.temp.join("origin.git");
        sh(
            &harness.temp,
            &["init", "-q", "--bare", "-b", "main", "origin.git"],
        );
        sh(
            root,
            &["remote", "add", "origin", origin.to_str().expect("utf8")],
        );
        sh(root, &["push", "-q", "origin", "main"]);
        let words = ["update".to_owned(), "--pr".to_owned()];
        let inv = dx_plan::args::parse(&words).expect("parse");
        let runner = harness.runner();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(
            &inv,
            harness.env(&runner, &mut out, &mut err),
            &super::super::update_outdated::DirFetch(registry),
            &fx.tools,
        );
        let out = String::from_utf8_lossy(&out).into_owned();
        let err = String::from_utf8_lossy(&err).into_owned();
        let argv = std::fs::read_to_string(&fx.argv).unwrap_or_default();
        (code, out, err, argv)
    }

    #[test]
    fn run_covers_workflow_pins_and_bazelversion() {
        let release = "{\"tag_name\":\"v4.1.0\"}";
        let (_, out, argv) = run_pr_with(
            &[
                (
                    ".github/workflows/ci.yml",
                    "jobs:\n  a:\n    steps:\n      - uses: o/r@v4.0.0\n",
                ),
                (".bazelversion", "7.0.0\n"),
            ],
            &[
                ("api.github.com/repos/o/r/releases/latest", release),
                (
                    "api.github.com/repos/bazelbuild/bazel/releases/latest",
                    "{\"tag_name\":\"7.1.0\"}",
                ),
            ],
        );
        let seen = format!("{out}{argv}");
        assert!(seen.contains("dx-update/github-actions"), "{seen}");
        assert!(seen.contains("dx-update/bazel"), "{seen}");
        assert!(!seen.contains("no dependency sets"), "{seen}");
    }

    #[test]
    fn run_groups_go_sets() {
        let (code, out, argv) = run_pr_with(
            &[
                (
                    "MODULE.bazel",
                    "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\ngo_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//:go.mod\")\n",
                ),
                (
                    "go.mod",
                    "module m\n\ngo 1.22\n\nrequire example.com/x v1.0.0\n",
                ),
            ],
            &[("proxy.golang.org/example.com/x/@v/list", "v1.0.0\nv1.1.0\n")],
        );
        let _ = code;
        assert!(
            format!("{out}{argv}").contains("dx-update/go"),
            "{out}{argv}"
        );
    }

    #[test]
    fn run_plans_ruby_sets() {
        let (code, out, argv) = run_pr_with(
            &[
                (
                    "MODULE.bazel",
                    "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(gemfile = \"//:Gemfile\", gemfile_lock = \"//:Gemfile.lock\")\n",
                ),
                ("Gemfile", "gem 'rake', '1.0'\n"),
                ("Gemfile.lock", "a\n"),
            ],
            &[(
                "rubygems.org/api/v1/versions/rake.json",
                "[{\"number\":\"1.5\",\"prerelease\":false}]",
            )],
        );
        let _ = code;
        assert!(!out.contains("skipped bundle:rake"), "{out}");
        assert!(
            format!("{out}{argv}").contains("dx-update/bundle"),
            "{out}{argv}"
        );
    }

    fn two_plans() -> Vec<GroupPlan> {
        let item = |set: &str, package: &str| Item {
            candidate: Candidate {
                set: set.into(),
                package: package.into(),
                current: "1.0.0".into(),
                latest: "2.0.0".into(),
                published: None,
                prerelease: false,
                repo: None,
                tags: None,
            },
            registry: Registry::Crates,
            untagged: false,
        };
        plan(&[
            item("cargo", "serde"),
            item("github-actions", "actions/checkout"),
        ])
    }

    fn two_scope() -> Scope {
        Scope::new(vec!["cargo".into(), "github-actions".into()])
    }

    #[test]
    fn a_workflow_push_refusal_skips_the_group() {
        let fx = fixture_with_hook(
            "[]",
            Some(
                "refusing to allow a GitHub App to create or update workflow `.github/workflows/ci.yml` without `workflows` permission",
            ),
        );
        let mut step = editing(fx.work.clone(), None);
        let got = publish(
            &fx.work,
            &fx.tools,
            &two_plans(),
            &two_scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let skipped = got
            .iter()
            .find(|o| o.branch == "dx-update/github-actions")
            .expect("outcome");
        assert_eq!(skipped.result, Ok(WORKFLOW_SKIP.into()));
        let opened = got
            .iter()
            .find(|o| o.branch == "dx-update/cargo")
            .expect("outcome");
        assert!(
            opened
                .result
                .as_ref()
                .is_ok_and(|t| t.starts_with("opened"))
        );
        let argv = std::fs::read_to_string(&fx.argv).expect("argv");
        assert!(!argv.contains("--head dx-update/github-actions"));
        let remote = sh(&fx.origin, &["branch", "--list"]);
        assert!(!remote.contains("github-actions"));
        assert_eq!(
            sh(&fx.work, &["rev-parse", "--abbrev-ref", "HEAD"]).trim(),
            "main"
        );
        let mut out = Vec::new();
        emit(&mut out, &got, false, 0).expect("emit");
        assert!(
            String::from_utf8_lossy(&out)
                .contains("dx-update/github-actions: skipped: token cannot push workflow files")
        );
    }

    #[test]
    fn other_push_rejections_still_fail() {
        let fx = fixture_with_hook("[]", Some("denied by policy"));
        let mut step = editing(fx.work.clone(), None);
        let got = publish(
            &fx.work,
            &fx.tools,
            &two_plans(),
            &two_scope(),
            &rules(),
            &[],
            &mut step,
        )
        .expect("publish");
        let failed = got
            .iter()
            .find(|o| o.branch == "dx-update/github-actions")
            .expect("outcome");
        assert!(failed.result.is_err());
    }

    #[test]
    fn recognizes_workflow_refusals() {
        assert!(refused_workflow(
            "! [remote rejected] x (refusing to allow a GitHub App to create or update workflow `a.yml` without `workflows` permission)"
        ));
        assert!(refused_workflow(
            "refusing to allow an OAuth App to create or update workflow `a.yml` without `workflow` scope"
        ));
        assert!(refused_workflow(
            "refusing to allow a Personal Access Token to create or update workflow `a.yml` without `workflow` scope"
        ));
        assert!(!refused_workflow("denied by policy"));
        assert!(!refused_workflow("refusing to allow deletion"));
    }

    #[test]
    fn nothing_outdated_prints_one_line() {
        let mut out = Vec::new();
        emit(&mut out, &[], false, 0).expect("emit");
        assert_eq!(String::from_utf8_lossy(&out), "nothing is outdated\n");
    }
}
