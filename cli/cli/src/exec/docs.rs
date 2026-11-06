use super::common::*;
use super::docs_index::{build_index, built_shards, execution_root};
use dx_output::{FinishedCounts, OutputMode, command_finished, command_started, operation_event};
use dx_plan::args::Invocation;
use dx_plan::plan::{DOCS_SITE_OUTPUT_GROUP, bep_path, plan_docs};
use dx_plan::resolve::resolve;
use std::path::{Path, PathBuf};

const DOCS_DEFAULT_PORT: u16 = 8000;

const DOCS_DEFAULT_HOST: &str = "127.0.0.1";

pub(crate) const CODE_SERVE_FAILED: &str = "serve_failed";

fn preview_url(host: &str, port: u16) -> String {
    format!("http://{host}:{port}/")
}

fn opener_argv_for(os: &str, url: &str) -> Vec<String> {
    let words: &[&str] = match os {
        "macos" => &["open"],
        "windows" => &["cmd", "/c", "start", ""],
        _ => &["xdg-open"],
    };
    let mut argv: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    argv.push(url.to_owned());
    argv
}

fn opener_argv(url: &str) -> Vec<String> {
    opener_argv_for(std::env::consts::OS, url)
}

/// Binds the docs preview. The seam lets tests swap the real server for a fake.
pub(crate) trait PreviewServer {
    fn bind(&self, host: &str, port: u16, root: &Path) -> std::io::Result<Box<dyn RunningPreview>>;
}

pub(crate) trait RunningPreview {
    fn run(self: Box<Self>) -> std::io::Result<()>;
}

struct StdPreview;

struct StdRunning(dx_docs_serve::Site);

impl PreviewServer for StdPreview {
    fn bind(&self, host: &str, port: u16, root: &Path) -> std::io::Result<Box<dyn RunningPreview>> {
        let site = dx_docs_serve::Site::bind(host, port, root.to_path_buf(), "")?;
        Ok(Box::new(StdRunning(site)))
    }
}

impl RunningPreview for StdRunning {
    fn run(self: Box<Self>) -> std::io::Result<()> {
        self.0.serve_until(&|| dx_process::interrupted().is_some())
    }
}

/// The site trees the docs aspect named in its site manifests.
fn built_sites(bep: &Path, workspace: &Path) -> Result<Vec<PathBuf>, (String, String)> {
    let account = collect_output_account(bep, DOCS_SITE_OUTPUT_GROUP, workspace)?;
    let mut sites: Vec<PathBuf> = account
        .targets
        .into_iter()
        .flat_map(|target| target.artifacts)
        .map(|manifest| workspace.join(String::from_utf8_lossy(&manifest.bytes).trim()))
        .collect();
    sites.sort();
    sites.dedup();
    Ok(sites)
}

/// The tree to preview: the one built site, else the API index, as a `(code, detail)` error otherwise.
fn serve_tree(
    sites: Vec<PathBuf>,
    index: Option<PathBuf>,
    scope_text: &str,
) -> Result<PathBuf, (String, String)> {
    if let (true, Some(index)) = (sites.is_empty(), index) {
        return Ok(index);
    }
    let [dir] = sites.as_slice() else {
        return Err((
            CODE_SERVE_FAILED.to_owned(),
            format!(
                "{scope_text} built {} docs sites; serve needs exactly one (pass a single site target)",
                sites.len()
            ),
        ));
    };
    if !dir.join("index.html").is_file() {
        return Err((
            CODE_SERVE_FAILED.to_owned(),
            format!(
                "the built site for {scope_text} has no index.html entry page at {}",
                dir.display()
            ),
        ));
    }
    Ok(dir.clone())
}

pub(crate) fn execute_docs(invocation: &Invocation, env: Env<'_>) -> i32 {
    execute_docs_with(invocation, env, &StdPreview)
}

fn execute_docs_with(invocation: &Invocation, env: Env<'_>, preview: &dyn PreviewServer) -> i32 {
    let Env {
        workspace,
        runner,
        query_runner,
        temp_dir,
        pid,
        nonce,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    let resolved = match resolve(
        &invocation.targets,
        workspace,
        query_runner,
        &invocation.bazel_startup_options,
    ) {
        Ok(resolved) => resolved,
        Err(error) => return resolve_failed(invocation.output, out, err, &error),
    };
    let scope_text = if resolved.targets.is_empty() {
        "//...".to_owned()
    } else {
        resolved.targets.join(" ")
    };
    let labels = resolved.targets.clone();
    let actions: &[&str] = &["summary", "render"];
    let bep = bep_path(temp_dir, pid, nonce);
    let Some(bep_text) = bep.to_str() else {
        return pre_exec(
            invocation.output,
            out,
            err,
            &crate::startup::temp_path_not_utf8(),
        );
    };
    let plan = match plan_docs(
        &resolved,
        &invocation.quiet_bazel_options(),
        bep_text,
        &invocation.bazel_startup_options,
    ) {
        Ok(plan) => plan,
        Err(error) => return pre_exec(invocation.output, out, err, &format!("{error}")),
    };
    let argv = plan.argv;
    let summary = format!("Running docs build for {scope_text} (mdBook render)");
    if json {
        if let Err(exit) = emit_built(out, command_started(invocation.command.name(), "default")) {
            return exit;
        }
        for action in actions {
            if let Err(exit) = emit_built(
                out,
                operation_event(invocation.command.name(), action, Some(&labels)),
            ) {
                return exit;
            }
        }
    }
    if !json
        && invocation.chatty()
        && let Err(exit) = check_stdout_write(writeln!(out, "{summary}"))
    {
        return exit;
    }
    let bazel_code = match run_bazel(invocation, out, err, workspace, runner, &argv, &[]) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if bazel_code != 0 {
        let _ = std::fs::remove_file(&bep);
        if json {
            if let Err(exit) = emit_built(
                out,
                bazel_exit_event(
                    bazel_failure_code(bazel_code),
                    &format!(
                        "Bazel docs build failed with exit {bazel_code} for {scope_text} (see stderr diagnostics)"
                    ),
                    bazel_code,
                    "execute",
                ),
            ) {
                return kept_child(err, bazel_code, exit);
            }
            if let Err(exit) = emit_event(
                out,
                &command_finished(bazel_code, &FinishedCounts::default()),
            ) {
                return kept_child(err, bazel_code, exit);
            }
        }
        return bazel_code;
    }
    let sites = built_sites(&bep, workspace);
    let mut index = None;
    if matches!(&sites, Ok(sites) if sites.is_empty()) {
        match built_shards(&bep, workspace) {
            Ok(shards) if shards.is_empty() => {}
            Ok(shards) => match build_index(
                &bep,
                workspace,
                execution_root(query_runner, workspace, &invocation.bazel_startup_options)
                    .as_deref(),
                &shards,
                runner,
            ) {
                Ok(dir) => {
                    if !json && invocation.chatty() {
                        let page = dir.join("index.html");
                        if let Err(exit) =
                            check_stdout_write(writeln!(out, "Docs index: {}", page.display()))
                        {
                            return exit;
                        }
                    }
                    index = Some(dir);
                }
                Err((code, detail)) => {
                    let _ = std::fs::remove_file(&bep);
                    return operational(invocation, out, err, &code, &detail);
                }
            },
            Err((code, detail)) => {
                let _ = std::fs::remove_file(&bep);
                return operational(invocation, out, err, &code, &detail);
            }
        }
    }
    if !invocation.serve {
        let _ = std::fs::remove_file(&bep);
        if json && let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default()))
        {
            return exit;
        }
        return 0;
    }
    let port = invocation.port.unwrap_or(DOCS_DEFAULT_PORT);
    let host = invocation.host.as_deref().unwrap_or(DOCS_DEFAULT_HOST);
    let url = preview_url(host, port);
    let _ = std::fs::remove_file(&bep);
    let site = sites.and_then(|sites| serve_tree(sites, index, &scope_text));
    let serve_dir = match site {
        Ok(dir) => dir,
        Err((code, detail)) => {
            return operational(invocation, out, err, &code, &detail);
        }
    };
    let serve_dir_text = serve_dir.display().to_string();
    let running = match preview.bind(host, port, &serve_dir) {
        Ok(running) => running,
        Err(error) => {
            return operational(
                invocation,
                out,
                err,
                CODE_SERVE_FAILED,
                &format!(
                    "failed to bind docs preview at {url}: {error} (port in use? retry another --port)"
                ),
            );
        }
    };
    if !json && invocation.chatty() {
        if let Err(exit) =
            check_stdout_write(writeln!(out, "Serving docs at {url} ({serve_dir_text})"))
        {
            return exit;
        }
        if invocation.open
            && let Err(exit) = check_stdout_write(writeln!(out, "Opening {url}"))
        {
            return exit;
        }
    }
    if invocation.open {
        let open_argv = opener_argv(&url);
        match runner.run(&open_argv, workspace, &[]) {
            Ok(status) if status.code == Some(0) => {}
            Ok(status) => {
                let _ = match status.code {
                    Some(code) => writeln!(
                        err,
                        "dx: warning: browser open exited with code {code} for {url} (continuing preview)"
                    ),
                    None => writeln!(
                        err,
                        "dx: warning: browser open was terminated by a signal for {url} (continuing preview)"
                    ),
                };
            }
            Err(error) => {
                let _ = writeln!(
                    err,
                    "dx: warning: failed to open browser for {url}: {error} (continuing preview)",
                );
            }
        }
    }
    if let Err(error) = running.run() {
        return operational(
            invocation,
            out,
            err,
            CODE_SERVE_FAILED,
            &format!("docs preview at {url} stopped: {error}"),
        );
    }
    let code = dx_process::interrupted().map_or(0, dx_process::interrupted_code);
    if json && let Err(exit) = emit_event(out, &command_finished(code, &FinishedCounts::default()))
    {
        return exit;
    }
    code
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::{
        CODE_SERVE_FAILED, PreviewServer, RunningPreview, execute_docs_with, opener_argv,
        opener_argv_for,
    };
    use dx_plan::resolve::QueryResult;
    use dx_testing::cli::strings;

    /// One target per site, each reporting a manifest that names its tree.
    fn site_bep(harness: &Harness, rels: &[&str]) -> Vec<String> {
        let mut lines = Vec::new();
        for (index, rel) in rels.iter().enumerate() {
            let manifest = format!("bazel-bin/site{index}/site.dx_docs_site");
            harness.write_source(&manifest, rel);
            let uri = format!("file://{}", harness.workspace.join(&manifest).display());
            lines.push(
                serde_json::json!({
                    "id": {"namedSet": {"id": index.to_string()}},
                    "namedSetOfFiles": {"files": [{"uri": uri}]},
                })
                .to_string(),
            );
            lines.push(
                serde_json::json!({
                    "id": {"targetCompleted": {"label": format!("//site{index}:site")}},
                    "completed": {
                        "success": true,
                        "outputGroup": [{"name": "dx_docs_sites", "fileSets": [{"id": index.to_string()}]}],
                    },
                })
                .to_string(),
            );
        }
        lines
    }

    /// One target per group, each reporting the given files.
    fn group_bep(harness: &Harness, groups: &[(&str, &[&str])]) -> Vec<String> {
        let mut lines = Vec::new();
        for (index, (group, files)) in groups.iter().enumerate() {
            let files: Vec<_> = files
                .iter()
                .map(|file| {
                    serde_json::json!({
                        "uri": format!("file://{}", harness.workspace.join(file).display())
                    })
                })
                .collect();
            lines.push(
                serde_json::json!({
                    "id": {"namedSet": {"id": index.to_string()}},
                    "namedSetOfFiles": {"files": files},
                })
                .to_string(),
            );
            lines.push(
                serde_json::json!({
                    "id": {"targetCompleted": {"label": format!("//t{index}:t")}},
                    "completed": {
                        "success": true,
                        "outputGroup": [{"name": group, "fileSets": [{"id": index.to_string()}]}],
                    },
                })
                .to_string(),
            );
        }
        lines
    }

    fn shards_bep(harness: &Harness, with_site: bool) -> Vec<String> {
        harness.write_source(
            "bazel-bin/tools.dx_docs_tools",
            "aggregate\tbazel-bin/agg\nmdbook\tbazel-bin/mdbook\nsite_build\tbazel-bin/site_build\n",
        );
        harness.write_source("bazel-bin/a/a.shard", "{}");
        harness.write_source("bazel-bin/b/b.shard", "{}");
        let mut groups: Vec<(&str, &[&str])> = vec![
            ("dx_docs_tools", &["bazel-bin/tools.dx_docs_tools"]),
            (
                "dx_docs_shards",
                &["bazel-bin/a/a.shard", "bazel-bin/b/b.shard"],
            ),
        ];
        if with_site {
            harness.write_source("bazel-bin/site.dx_docs_site", "bazel-bin/site");
            harness.write_source("bazel-bin/site/index.html", "<!doctype html>\n");
            groups.push(("dx_docs_sites", &["bazel-bin/site.dx_docs_site"]));
        }
        group_bep(harness, &groups)
    }

    fn tool_runs(harness: &Harness) -> Vec<Vec<String>> {
        let runs = harness.build_argv.borrow();
        runs.iter()
            .filter(|argv| argv[0].ends_with("/agg") || argv[0].ends_with("/site_build"))
            .cloned()
            .collect()
    }

    #[test]
    fn docs_without_a_site_aggregates_every_shard_in_scope() {
        let mut harness = Harness::new("docs-index");
        harness.raw_bep = Some(shards_bep(&harness, false));
        let (code, out, err) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            err.contains("docs_index_failed") || err.contains("no index.html"),
            "{err}"
        );
        let runs = tool_runs(&harness);
        let aggregate = runs[0].join(" ").replace('\\', "/");
        assert!(
            aggregate.contains("--shard ") && aggregate.contains("/a/a.shard"),
            "{aggregate}"
        );
        assert!(aggregate.contains("/b/b.shard"), "{aggregate}");
    }

    #[test]
    fn docs_with_a_site_writes_no_index() {
        let mut harness = Harness::new("docs-index-site");
        harness.raw_bep = Some(shards_bep(&harness, true));
        let (code, out, err) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(tool_runs(&harness).is_empty());
        assert!(!harness.workspace.join(".dx/local/docs").exists());
    }

    #[test]
    fn docs_without_shards_or_sites_exits_zero() {
        let mut harness = Harness::new("docs-index-empty");
        harness.raw_bep = Some(Vec::new());
        let (code, out, err) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(tool_runs(&harness).is_empty());
    }

    fn script_site_tree(harness: &mut Harness, rel: &str) {
        harness.write_source(&format!("{rel}/index.html"), "<!doctype html>\n");
        harness.raw_bep = Some(site_bep(harness, &[rel]));
    }

    fn docs_argv(harness: &Harness, words: &[&str]) -> Vec<String> {
        let (code, out, err) = harness.run(words);
        assert_eq!(code, 0, "{words:?}: {out}{err}");
        let argv = harness.build_argv.borrow()[0].clone();
        assert!(
            argv.contains(
                &"--aspects=@rules_dx//quality:real_aspects.bzl%real_docs_aspect".to_owned()
            ),
            "{argv:?}"
        );
        assert!(
            argv.contains(&"--output_groups=dx_results,dx_docs_sites,dx_docs_shards,dx_docs_tools,dx_docs_tool_files".to_owned()),
            "{argv:?}"
        );
        assert!(
            !argv.iter().any(|arg| arg.contains("user_site")),
            "{argv:?}"
        );
        argv
    }

    #[test]
    fn docs_explicit_scopes_and_process_failures_keep_exit_contract() {
        for json in [false, true] {
            let output = if json {
                "--output=json"
            } else {
                "--output=text"
            };
            let harness = Harness::new("docs-scope");
            let (code, out, err) = harness.run(&["docs", "//docs:page", output]);
            assert_eq!(code, 0, "{out}{err}");
            assert!(out.contains("//docs:page"), "{out}");
            for signalled in [false, true] {
                let mut harness = Harness::new("docs-process-failure");
                harness.signalled = signalled;
                harness.io_error = !signalled;
                let (code, out, err) = harness.run(&["docs", output]);
                assert_eq!(code, 1, "{out}{err}");
                assert!(err.contains(if signalled { "signal" } else { "launch" }));
            }
        }
        let harness = Harness::new("docs-missing-scope");
        let (code, _, err) = harness.run(&["docs", "missing.md"]);
        assert_eq!(code, 2);
        assert!(!err.is_empty());
        assert!(harness.seen_env.borrow().is_empty());
    }

    #[test]
    fn docs_builds_and_renders() {
        let harness = Harness::new("docs-build");
        let (code, out, _) = harness.run(&["docs", "--output=text"]);
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("Running docs build for //... (mdBook render)"),
            "{out}"
        );
    }

    #[test]
    fn docs_no_scope_covers_the_repository() {
        let harness = Harness::new("docs-no-scope");
        let argv = docs_argv(&harness, &["docs", "--output=text"]);
        assert_eq!(argv.get(argv.len() - 2).map(String::as_str), Some("//..."));
    }

    #[test]
    fn docs_dir_scope_is_its_tree() {
        let harness = Harness::new("docs-dir-scope");
        harness.write_source("a/BUILD.bazel", "");
        let argv = docs_argv(&harness, &["docs", "a", "--output=text"]);
        assert_eq!(
            argv.get(argv.len() - 2).map(String::as_str),
            Some("//a/...")
        );
    }

    #[test]
    fn docs_file_scope_is_its_owning_rule() {
        let harness = Harness::new("docs-file-scope");
        harness.write_source("a/BUILD.bazel", "");
        harness.write_source("a/page.md", "# page\n");
        harness
            .query
            .script_graph(&[graph_rule("//a:site", &["//a:page.md"], &[])]);
        let argv = docs_argv(&harness, &["docs", "a/page.md", "--output=text"]);
        assert_eq!(
            argv.get(argv.len() - 2).map(String::as_str),
            Some("//a:site")
        );
    }

    #[test]
    fn docs_here_is_the_current_tree() {
        let mut harness = Harness::new("docs-here");
        harness.write_source("a/BUILD.bazel", "");
        harness.cwd = harness.workspace.join("a");
        let argv = docs_argv(&harness, &["docs", "--here", "--output=text"]);
        assert_eq!(
            argv.get(argv.len() - 2).map(String::as_str),
            Some("//a/...")
        );
    }

    #[test]
    fn docs_scope_without_docs_targets_exits_zero() {
        let mut harness = Harness::new("docs-empty-scope");
        harness.write_source("b/BUILD.bazel", "");
        harness.raw_bep = Some(Vec::new());
        let (code, out, err) = harness.run(&["docs", "b", "--output=text"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("Running docs build for //b/..."), "{out}");
    }

    #[test]
    fn docs_forwards_command_options_to_inner_bazel() {
        let harness = Harness::new("docs-forward");
        let inv = invocation(&["docs", "--", "--config=ci", "--jobs=4"]);
        let run = harness.probe_with(&inv, &[Some(0)]);
        assert_eq!(run.code, 0, "{run:?}");
        assert_eq!(run.argv.len(), 1, "{run:?}");
        assert!(run.argv[0].contains(&"--config=ci".to_owned()), "{run:?}");
        assert!(run.argv[0].contains(&"--jobs=4".to_owned()), "{run:?}");
    }

    #[test]
    fn docs_conflicting_and_startup_options_are_pre_exec() {
        for words in [
            vec!["docs", "--", "--home_rc"],
            vec!["docs", "--", "--test_arg=foo"],
        ] {
            let harness = Harness::new("docs-reject");
            let (code, _, err) = harness.run(&words);
            assert_eq!(code, 2, "{words:?}: {err}");
        }
    }

    #[test]
    fn docs_port_requires_serve() {
        let err = dx_plan::args::parse(&strings(&["docs", "--port=8080"]))
            .expect_err("port without serve must fail");
        assert!(err.to_string().contains("--port"), "{err}");
        for words in [
            vec!["docs", "--host=example.test"],
            vec!["docs", "--open"],
            vec!["docs", "--port=0", "--serve"],
        ] {
            assert!(dx_plan::args::parse(&strings(&words)).is_err(), "{words:?}");
        }
    }

    #[test]
    fn docs_rejects_apply() {
        let err = dx_plan::args::parse(&strings(&["docs", "--serve", "--apply"]))
            .expect_err("docs takes no --apply");
        assert!(err.to_string().contains("--apply"), "{err}");
    }

    #[test]
    fn docs_json_streams_started_operation_finished() {
        let harness = Harness::new("docs-json");
        let (code, out, _) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 0, "{out}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started", "{out}");
        assert!(kinds.contains(&"operation"), "{kinds:?}");
        assert_eq!(kinds[kinds.len() - 1], "command_finished", "{out}");
    }

    #[test]
    fn docs_json_pre_exec_errors_have_no_command_started() {
        let argv = &["docs", "--output=json", "--", "--home_rc"][..];
        let harness = Harness::new("docs-pre-exec");
        let (code, out, err) = harness.run(argv);
        assert_eq!(code, 2, "{argv:?}: {out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert!(!kinds.contains(&"command_started"), "{argv:?}: {kinds:?}");
        assert!(!kinds.contains(&"operation"), "{argv:?}: {kinds:?}");
        assert_eq!(kinds.first(), Some(&"error"), "{argv:?}: {out}");
        assert_eq!(
            events[0]["code"],
            serde_json::json!("invalid_arguments"),
            "{argv:?}: {out}"
        );
        assert_eq!(kinds.last(), Some(&"command_finished"), "{argv:?}: {out}");
        assert_eq!(
            events[events.len() - 1]["exit_code"],
            serde_json::json!(2),
            "{argv:?}: {out}"
        );
    }

    #[test]
    fn docs_scope_resolution_failures_use_resolve_codes() {
        let harness = Harness::new("docs-resolve-missing");
        let (code, out, _) = harness.run(&["docs", "missing.md", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        let events = json_events(&out);
        assert_eq!(
            events[0]["code"],
            serde_json::json!("invalid_scope"),
            "{out}"
        );
        assert!(!event_kinds(&events).contains(&"command_started"), "{out}");

        let harness = Harness::new("docs-resolve-launch");
        std::fs::create_dir_all(harness.workspace.join("docs")).expect("docs dir");
        std::fs::write(harness.workspace.join("docs/page.md"), "# page\n").expect("page");
        std::fs::write(harness.workspace.join("docs/BUILD.bazel"), "").expect("build");
        let (code, out, _) = harness.run(&["docs", "docs/page.md", "--output=json"]);
        assert_eq!(code, 1, "{out}");
        let events = json_events(&out);
        assert_eq!(
            events[0]["code"],
            serde_json::json!("query_launch_failed"),
            "{out}"
        );
        assert!(!event_kinds(&events).contains(&"command_started"), "{out}");

        let harness = Harness::new("docs-resolve-query");
        std::fs::create_dir_all(harness.workspace.join("docs")).expect("docs dir");
        std::fs::write(harness.workspace.join("docs/page.md"), "# page\n").expect("page");
        std::fs::write(harness.workspace.join("docs/BUILD.bazel"), "").expect("build");
        harness.query.outputs.borrow_mut().push(QueryResult {
            code: Some(1),
            stdout: Vec::new(),
            stderr: b"no such package\n".to_vec(),
        });
        let (code, out, _) = harness.run(&["docs", "docs/page.md", "--output=json"]);
        assert_eq!(code, 2, "{out}");
        let events = json_events(&out);
        assert_eq!(
            events[0]["code"],
            serde_json::json!("query_failed"),
            "{out}"
        );
    }

    #[test]
    fn docs_json_launch_failure_is_launch_failed() {
        let mut harness = Harness::new("docs-launch-failed");
        harness.io_error = true;
        let (code, out, err) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started", "{out}");
        let operation = kinds.iter().position(|kind| *kind == "operation");
        let error = kinds.iter().position(|kind| *kind == "error");
        assert!(operation.is_some() && operation < error, "{kinds:?}");
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(
            errors[0]["code"],
            serde_json::json!("launch_failed"),
            "{out}"
        );
        assert_eq!(
            errors[0]["cause"],
            serde_json::json!("tool_unavailable"),
            "{out}"
        );
        assert_eq!(kinds[kinds.len() - 1], "command_finished", "{out}");
        assert_eq!(
            events[events.len() - 1]["exit_code"],
            serde_json::json!(1),
            "{out}"
        );
    }

    #[test]
    fn docs_full_disk_is_disk_full_not_bazel_failed() {
        let harness = Harness {
            bazel_code: 36,
            free_disk_bytes: Some(1 << 20),
            ..Harness::new("docs-disk-full")
        };
        let (code, out, err) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 36, "{out}{err}");
        let events = json_events(&out);
        let errors = events_of_kind(&events, "error");
        assert_eq!(errors.len(), 1, "{out}");
        assert_eq!(errors[0]["code"], serde_json::json!("disk_full"), "{out}");
        assert_eq!(errors[0]["cause"], serde_json::json!("host"), "{out}");
        assert_eq!(errors[0]["status"], serde_json::json!(36), "{out}");
        assert!(!out.contains("bazel_failed"), "{out}");
        let finished = event(&events, "command_finished");
        assert_eq!(finished["exit_code"], serde_json::json!(36), "{out}");
    }

    #[test]
    fn docs_bazel_failure_names_unit_and_pin() {
        let mut harness = Harness::new("docs-fail");
        harness.bazel_code = 3;
        let (code, out, _) = harness.run(&["docs", "--output=json"]);
        assert_eq!(code, 3, "{out}");
        let events = json_events(&out);
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!("bazel_failed"), "{out}");
        assert_eq!(error["cause"], serde_json::json!("bazel"), "{out}");
        assert_eq!(error["status"], serde_json::json!(3), "{out}");
        assert_eq!(error["phase"], serde_json::json!("execute"), "{out}");
        assert!(out.contains("//..."), "{out}");
    }

    #[test]
    fn docs_serve_needs_exactly_one_site_tree() {
        for rels in [&[][..], &["bazel-bin/a/site", "bazel-bin/b/site"][..]] {
            let mut harness = Harness::new("docs-serve-site-count");
            harness.raw_bep = Some(site_bep(&harness, rels));
            let (code, _, err) = harness.run(&["docs", "--serve", "--output=text"]);
            assert_eq!(code, 1, "{err}");
            assert!(err.contains("serve_failed"), "{err}");
            assert!(err.contains("exactly one"), "{err}");
        }
    }

    #[test]
    fn docs_serve_needs_a_rendered_entry_page() {
        let mut harness = Harness::new("docs-serve-no-entry");
        harness.raw_bep = Some(site_bep(&harness, &["bazel-bin/a/site"]));
        let (code, _, err) = harness.run(&["docs", "--serve", "--output=text"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("serve_failed"), "{err}");
        assert!(err.contains("index.html"), "{err}");
    }

    #[test]
    fn serve_failed_code_is_stable_single_source() {
        assert_eq!(CODE_SERVE_FAILED, "serve_failed");
    }

    #[derive(Default)]
    struct FakePreview {
        bind_error: bool,
        run_error: bool,
        bound: std::cell::RefCell<Vec<(String, u16, std::path::PathBuf)>>,
    }

    struct FakeRunning {
        fail: bool,
    }

    impl PreviewServer for FakePreview {
        fn bind(
            &self,
            host: &str,
            port: u16,
            root: &std::path::Path,
        ) -> std::io::Result<Box<dyn RunningPreview>> {
            if self.bind_error {
                return Err(std::io::Error::other("address in use"));
            }
            self.bound
                .borrow_mut()
                .push((host.to_owned(), port, root.to_path_buf()));
            Ok(Box::new(FakeRunning {
                fail: self.run_error,
            }))
        }
    }

    impl RunningPreview for FakeRunning {
        fn run(self: Box<Self>) -> std::io::Result<()> {
            if self.fail {
                Err(std::io::Error::other("listener died"))
            } else {
                Ok(())
            }
        }
    }

    struct Served {
        code: i32,
        out: String,
        err: String,
        argvs: Vec<Vec<String>>,
    }

    fn serve(harness: &Harness, preview: &FakePreview, words: &[&str]) -> Served {
        let runner = harness.runner();
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute_docs_with(
            &invocation(words),
            harness.env(&runner, &mut out, &mut err),
            preview,
        );
        Served {
            code,
            out: String::from_utf8(out).expect("stdout"),
            err: String::from_utf8(err).expect("stderr"),
            argvs: harness.build_argv.borrow().clone(),
        }
    }

    #[test]
    fn docs_serve_previews_after_build() {
        let mut harness = Harness::new("docs-serve");
        script_site_tree(&mut harness, "bazel-bin/a/site");
        let preview = FakePreview::default();
        let run = serve(&harness, &preview, &["docs", "--serve", "--port=8080"]);
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(
            run.out.contains("Serving docs at http://127.0.0.1:8080/"),
            "{}",
            run.out
        );
        let bound = preview.bound.borrow();
        assert_eq!(bound.len(), 1, "{bound:?}");
        assert_eq!((bound[0].0.as_str(), bound[0].1), ("127.0.0.1", 8080));
        assert!(
            bound[0].2.ends_with("bazel-bin/a/site"),
            "the preview must serve the tree the site manifest names: {bound:?}"
        );
        assert_eq!(run.argvs.len(), 1, "only bazel runs: {:?}", run.argvs);
    }

    #[test]
    fn docs_serve_binds_host_and_opens_browser() {
        let mut harness = Harness::new("docs-serve-bind");
        script_site_tree(&mut harness, "bazel-bin/a/site");
        let preview = FakePreview::default();
        let run = serve(
            &harness,
            &preview,
            &[
                "docs",
                "--serve",
                "--port=8080",
                "--host=example.test",
                "--open",
            ],
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        assert!(
            run.out
                .contains("Serving docs at http://example.test:8080/"),
            "{}",
            run.out
        );
        assert!(
            run.out.contains("Opening http://example.test:8080/"),
            "{}",
            run.out
        );
        let bound = preview.bound.borrow();
        assert_eq!((bound[0].0.as_str(), bound[0].1), ("example.test", 8080));
        assert_eq!(run.argvs.len(), 2, "{:?}", run.argvs);
        let build = &run.argvs[0];
        for setting in dx_plan::plan::BAZEL_SETTINGS {
            assert!(build.contains(&setting.to_owned()), "{build:?}");
        }
        assert_eq!(
            run.argvs[1].last().map(String::as_str),
            Some("http://example.test:8080/")
        );
        assert!(
            run.argvs.iter().flatten().all(|arg| arg != "python3"),
            "no python3 may run: {:?}",
            run.argvs
        );
    }

    #[test]
    fn docs_serve_bind_failure_is_serve_failed() {
        let mut harness = Harness::new("docs-serve-bind-fail");
        script_site_tree(&mut harness, "bazel-bin/a/site");
        let preview = FakePreview {
            bind_error: true,
            ..FakePreview::default()
        };
        let run = serve(
            &harness,
            &preview,
            &["docs", "--serve", "--port=8080", "--open", "--output=json"],
        );
        assert_eq!(run.code, 1, "bind failure is operational");
        assert!(run.err.contains("serve_failed"), "{}", run.err);
        assert!(run.err.contains("port in use"), "{}", run.err);
        assert!(run.out.contains("serve_failed"), "{}", run.out);
        assert!(!run.out.contains("Serving"), "{}", run.out);
        assert_eq!(run.argvs.len(), 1, "no browser opens: {:?}", run.argvs);
    }

    #[test]
    fn docs_serve_run_failure_is_serve_failed() {
        let mut harness = Harness::new("docs-serve-run-fail");
        script_site_tree(&mut harness, "bazel-bin/a/site");
        let preview = FakePreview {
            run_error: true,
            ..FakePreview::default()
        };
        let run = serve(
            &harness,
            &preview,
            &["docs", "--serve", "--port=8080", "--output=json"],
        );
        assert_eq!(run.code, 1, "{}{}", run.out, run.err);
        let events = json_events(&run.out);
        let kinds = event_kinds(&events);
        assert_eq!(kinds[0], "command_started");
        assert_eq!(kinds[kinds.len() - 1], "command_finished");
        let error = event(&events, "error");
        assert_eq!(error["code"], serde_json::json!(CODE_SERVE_FAILED));
    }

    #[test]
    fn docs_serve_previews_the_built_site_tree() {
        let mut harness = Harness::new("docs-serve-foreign-tree");
        script_site_tree(&mut harness, "bazel-bin/other/site");
        let preview = FakePreview::default();
        let run = serve(
            &harness,
            &preview,
            &["docs", "//other:site", "--serve", "--output=text"],
        );
        assert_eq!(run.code, 0, "{}{}", run.out, run.err);
        let bound = preview.bound.borrow();
        assert!(bound[0].2.ends_with("bazel-bin/other/site"), "{bound:?}");
    }

    #[test]
    fn opener_argv_names_the_os_opener() {
        let url = "http://127.0.0.1:8000/";
        assert_eq!(opener_argv_for("macos", url), strings(&["open", url]));
        assert_eq!(
            opener_argv_for("windows", url),
            strings(&["cmd", "/c", "start", "", url])
        );
        for os in ["linux", "freebsd"] {
            assert_eq!(opener_argv_for(os, url), strings(&["xdg-open", url]));
        }
        assert!(!opener_argv(url).contains(&"python3".to_owned()));
    }
}
