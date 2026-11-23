#![deny(
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::map_err_ignore,
    clippy::string_slice,
    clippy::unused_result_ok,
    clippy::wildcard_enum_match_arm
)]

mod docs_index;

use docs_index::{CODE_INDEX_FAILED, build_index, built_shards, execution_root};
use dx_exec_core::common::{
    Env, bazel_exit_event, bazel_failure_code, bep_error_code, check_stdout_write,
    collect_output_account, emit_built, emit_event, kept_child, operational, pre_exec,
    report_dx_error, resolve_failed, run_bazel, temp_path_not_utf8,
};
use dx_output::{FinishedCounts, OutputMode, command_finished, command_started, operation_event};
use dx_plan::args::Invocation;
use dx_plan::plan::{Consumer, DOCS_SITE_OUTPUT_GROUP, bep_path, plan_docs};
use dx_plan::resolve::resolve;
use std::path::{Path, PathBuf};

const DOCS_DEFAULT_PORT: u16 = 8000;

const DOCS_DEFAULT_HOST: &str = "127.0.0.1";

pub const CODE_SERVE_FAILED: &str = "serve_failed";

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

pub trait PreviewServer {
    fn bind(&self, host: &str, port: u16, root: &Path) -> std::io::Result<Box<dyn RunningPreview>>;
}

pub trait RunningPreview {
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

fn built_sites(bep: &Path, workspace: &Path) -> Result<Vec<PathBuf>, dx_error::DxError> {
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

#[expect(
    clippy::needless_pass_by_value,
    reason = "the value is built for this one call"
)]
fn serve_tree(
    sites: Vec<PathBuf>,
    index: Option<PathBuf>,
    scope_text: &str,
) -> Result<PathBuf, dx_error::DxError> {
    if let (true, Some(index)) = (sites.is_empty(), index) {
        return Ok(index);
    }
    let [dir] = sites.as_slice() else {
        return Err(dx_error::DxError::failure(format!(
            "{scope_text} built {} docs sites; serve needs exactly one (pass a single site target)",
            sites.len()
        )));
    };
    if !dir.join("index.html").is_file() {
        return Err(dx_error::DxError::failure(format!(
            "the built site for {scope_text} has no index.html entry page at {}",
            dir.display()
        )));
    }
    Ok(dir.clone())
}

pub fn execute_docs(invocation: &Invocation, env: Env<'_>) -> i32 {
    execute_docs_with(invocation, env, &StdPreview)
}

struct DocsPlan {
    argv: Vec<String>,
    bep: PathBuf,
    scope_text: String,
    labels: Vec<String>,
}

type BuiltSites = Result<Vec<PathBuf>, dx_error::DxError>;

pub fn execute_docs_with(
    invocation: &Invocation,
    mut env: Env<'_>,
    preview: &dyn PreviewServer,
) -> i32 {
    let json = invocation.output == OutputMode::Json;
    let DocsPlan {
        argv,
        bep,
        scope_text,
        labels,
    } = match plan_docs_build(invocation, &mut env) {
        Ok(planned) => planned,
        Err(exit) => return exit,
    };
    let summary = format!("Running docs build for {scope_text} (mdBook render)");
    if json {
        if let Err(exit) = emit_built(
            env.out,
            command_started(invocation.command.name(), "default"),
        ) {
            return exit;
        }
        for action in ["summary", "render"] {
            if let Err(exit) = emit_built(
                env.out,
                operation_event(invocation.command.name(), action, Some(&labels)),
            ) {
                return exit;
            }
        }
    }
    if !json
        && invocation.chatty()
        && let Err(exit) = check_stdout_write(writeln!(env.out, "{summary}"))
    {
        return exit;
    }
    let bazel_code = match run_bazel(
        invocation,
        env.out,
        env.err,
        env.workspace,
        env.runner,
        &argv,
        &[],
    ) {
        Ok(code) => code,
        Err(exit) => return exit,
    };
    if bazel_code != 0 {
        dx_atomic_fs::discard_file(&bep);
        if json {
            return report_docs_failure(&mut env, bazel_code, &scope_text);
        }
        return bazel_code;
    }
    let (sites, index) = match locate_sites(invocation, &mut env, &bep) {
        Ok(located) => located,
        Err(exit) => return exit,
    };
    if !invocation.serve {
        dx_atomic_fs::discard_file(&bep);
        if json
            && let Err(exit) = emit_event(env.out, &command_finished(0, &FinishedCounts::default()))
        {
            return exit;
        }
        return 0;
    }
    dx_atomic_fs::discard_file(&bep);
    serve_docs(invocation, &mut env, preview, sites, index, &scope_text)
}

fn plan_docs_build(invocation: &Invocation, env: &mut Env<'_>) -> Result<DocsPlan, i32> {
    let workspace = env.workspace;
    let resolved = resolve(
        &invocation.targets,
        workspace,
        env.query_runner,
        &invocation.bazel_startup_options,
    )
    .map_err(|error| resolve_failed(invocation.output, env.out, env.err, &error))?;
    let scope_text = if resolved.targets.is_empty() {
        "//...".to_owned()
    } else {
        resolved.targets.join(" ")
    };
    let labels = resolved.targets.clone();
    let bep = bep_path(env.temp_dir, env.pid, env.nonce);
    let Some(bep_text) = bep.to_str() else {
        return Err(pre_exec(
            invocation.output,
            env.out,
            env.err,
            &temp_path_not_utf8(),
        ));
    };
    let deps: Vec<String> = dx_adopt::module_deps(workspace).into_iter().collect();
    let extensions = (!deps.is_empty() && !dx_adopt::is_rules_dx_root(workspace))
        .then(|| dx_adopt::file_extensions(workspace));
    let consumer = extensions
        .as_ref()
        .map(|extensions| Consumer { extensions });
    let plan = plan_docs(
        &resolved,
        &invocation.quiet_bazel_options(),
        bep_text,
        &invocation.bazel_startup_options,
        consumer,
    )
    .map_err(|error| pre_exec(invocation.output, env.out, env.err, &format!("{error}")))?;
    Ok(DocsPlan {
        argv: plan.argv,
        bep,
        scope_text,
        labels,
    })
}

fn report_docs_failure(env: &mut Env<'_>, bazel_code: i32, scope_text: &str) -> i32 {
    if let Err(exit) = emit_built(
        env.out,
        bazel_exit_event(
            bazel_failure_code(bazel_code),
            &format!(
                "Bazel docs build failed with exit {bazel_code} for {scope_text} (see stderr diagnostics)"
            ),
            bazel_code,
            "execute",
        ),
    ) {
        return kept_child(env.err, bazel_code, exit);
    }
    if let Err(exit) = emit_event(
        env.out,
        &command_finished(bazel_code, &FinishedCounts::default()),
    ) {
        return kept_child(env.err, bazel_code, exit);
    }
    bazel_code
}

fn locate_sites(
    invocation: &Invocation,
    env: &mut Env<'_>,
    bep: &Path,
) -> Result<(BuiltSites, Option<PathBuf>), i32> {
    let workspace = env.workspace;
    let json = invocation.output == OutputMode::Json;
    let sites = built_sites(bep, workspace);
    let mut index = None;
    if matches!(&sites, Ok(sites) if sites.is_empty()) {
        match built_shards(bep, workspace) {
            Ok(shards) if shards.is_empty() => {}
            Ok(shards) => match build_index(
                bep,
                workspace,
                execution_root(
                    env.query_runner,
                    workspace,
                    &invocation.bazel_startup_options,
                )
                .as_deref(),
                &shards,
            ) {
                Ok(dir) => {
                    if !json && invocation.chatty() {
                        let page = dir.join("index.html");
                        check_stdout_write(writeln!(env.out, "Docs index: {}", page.display()))?;
                    }
                    index = Some(dir);
                }
                Err(error) => {
                    dx_atomic_fs::discard_file(bep);
                    return Err(report_dx_error(
                        invocation.output,
                        env.out,
                        env.err,
                        CODE_INDEX_FAILED,
                        &error,
                    ));
                }
            },
            Err(error) => {
                dx_atomic_fs::discard_file(bep);
                return Err(report_dx_error(
                    invocation.output,
                    env.out,
                    env.err,
                    bep_error_code(&error),
                    &error,
                ));
            }
        }
    }
    Ok((sites, index))
}

fn serve_docs(
    invocation: &Invocation,
    env: &mut Env<'_>,
    preview: &dyn PreviewServer,
    sites: BuiltSites,
    index: Option<PathBuf>,
    scope_text: &str,
) -> i32 {
    let json = invocation.output == OutputMode::Json;
    let port = invocation.port.unwrap_or(DOCS_DEFAULT_PORT);
    let host = invocation.host.as_deref().unwrap_or(DOCS_DEFAULT_HOST);
    let url = preview_url(host, port);
    let site = sites.and_then(|sites| serve_tree(sites, index, scope_text));
    let serve_dir = match site {
        Ok(dir) => dir,
        Err(error) => {
            return report_dx_error(
                invocation.output,
                env.out,
                env.err,
                CODE_SERVE_FAILED,
                &error,
            );
        }
    };
    let serve_dir_text = serve_dir.display().to_string();
    let running = match preview.bind(host, port, &serve_dir) {
        Ok(running) => running,
        Err(error) => {
            return operational(
                invocation,
                env.out,
                env.err,
                CODE_SERVE_FAILED,
                &format!(
                    "failed to bind docs preview at {url}: {error} (port in use? retry another --port)"
                ),
            );
        }
    };
    if !json && invocation.chatty() {
        if let Err(exit) = check_stdout_write(writeln!(
            env.out,
            "Serving docs at {url} ({serve_dir_text})"
        )) {
            return exit;
        }
        if invocation.open
            && let Err(exit) = check_stdout_write(writeln!(env.out, "Opening {url}"))
        {
            return exit;
        }
    }
    if invocation.open {
        open_browser(env, &url);
    }
    if let Err(error) = running.run() {
        return operational(
            invocation,
            env.out,
            env.err,
            CODE_SERVE_FAILED,
            &format!("docs preview at {url} stopped: {error}"),
        );
    }
    let code = dx_process::interrupted().map_or(0, dx_process::interrupted_code);
    if json
        && let Err(exit) = emit_event(env.out, &command_finished(code, &FinishedCounts::default()))
    {
        return exit;
    }
    code
}

fn open_browser(env: &mut Env<'_>, url: &str) {
    let open_argv = opener_argv(url);
    match env.runner.run(&open_argv, env.workspace, &[]) {
        Ok(status) if status.code == Some(0) => {}
        Ok(status) => match status.code {
            Some(code) => dx_output::write_stderr(
                env.err,
                format_args!(
                    "dx: warning: browser open exited with code {code} for {url} (continuing preview)\n"
                ),
            ),
            None => dx_output::write_stderr(
                env.err,
                format_args!(
                    "dx: warning: browser open was terminated by a signal for {url} (continuing preview)\n"
                ),
            ),
        },
        Err(error) => {
            dx_output::write_stderr(
                env.err,
                format_args!(
                    "dx: warning: failed to open browser for {url}: {error} (continuing preview)\n"
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CODE_SERVE_FAILED, opener_argv, opener_argv_for};
    use dx_testing::cli::strings;

    #[test]
    fn serve_failed_code_is_stable_single_source() {
        assert_eq!(CODE_SERVE_FAILED, "serve_failed");
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
