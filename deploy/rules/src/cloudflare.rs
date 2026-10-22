use std::ffi::OsString;
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
const TAIL_LINES: usize = 20;
const REJECTED_ROOTS: [(&str, &str); 3] = [
    ("_worker.js", "an executable Pages worker"),
    ("functions", "executable Pages Functions"),
    ("_routes.json", "Pages Functions routing"),
];
const SKIPPED_NAMES: [&str; 2] = [".git", "node_modules"];
const PASSTHROUGH_ENV: [&str; 11] = [
    "PATH",
    "TMPDIR",
    "TMP",
    "TEMP",
    "SystemRoot",
    "RUNFILES_DIR",
    "RUNFILES_MANIFEST_FILE",
    "RUNFILES_MANIFEST_ONLY",
    "JAVA_RUNFILES",
    "LANG",
    "LC_ALL",
];

/// Names one Cloudflare Pages deployment target.
pub struct PagesLaunch<'a> {
    pub deploy_name: &'a str,
    pub account_id: &'a str,
    pub project: &'a str,
    pub branch: &'a str,
    pub production_branch: &'a str,
    pub site_rloc: &'a str,
    pub wrangler_rloc: &'a str,
}

/// Holds the runtime environment one Pages deploy reads.
pub struct PagesEnv {
    pub token: String,
    pub account_id: Option<String>,
    pub passthrough: Vec<(String, String)>,
    pub profile: String,
}

/// Lists one validated site file.
#[derive(Debug, PartialEq, Eq)]
pub struct PagesFile {
    pub path: String,
    pub source: PathBuf,
    pub size: u64,
}

/// Holds one reported Pages deployment.
#[derive(Debug, PartialEq, Eq)]
pub struct PagesDeployment {
    pub id: String,
    pub url: String,
    pub environment: String,
}

/// Returns whether the branch deploys to production or preview.
pub fn pages_intent(branch: &str, production_branch: &str) -> &'static str {
    if branch == production_branch {
        "production"
    } else {
        "preview"
    }
}

fn pages_name_ok(value: &str, max: usize, allowed: impl Fn(char) -> bool) -> bool {
    !value.is_empty() && value.len() <= max && value.chars().all(allowed)
}

/// Validates the project, account, branch and production branch values.
pub fn pages_identity_error(
    account_id: &str,
    project: &str,
    branch: &str,
    production_branch: &str,
) -> Option<String> {
    if account_id.len() != 32
        || !account_id
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    {
        return Some(format!(
            "invalid account_id '{account_id}': want 32 lowercase hex characters"
        ));
    }
    let project_ok = pages_name_ok(project, 58, |c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
    }) && !project.starts_with('-')
        && !project.ends_with('-');
    if !project_ok {
        return Some(format!(
            "invalid project '{project}': want 1-58 characters of [a-z0-9-] that start and end with a letter or digit"
        ));
    }
    for (what, value) in [("branch", branch), ("production_branch", production_branch)] {
        let ok = pages_name_ok(value, 100, |c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')
        }) && !value.starts_with('-')
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains("..")
            && !value.contains("//");
        if !ok {
            return Some(format!(
                "invalid {what} '{value}': want 1-100 characters of [A-Za-z0-9._/-] with no leading '-' and no '..'"
            ));
        }
    }
    None
}

fn pages_component_error(rel: &str, name: &str) -> Option<String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('\\') {
        return Some(format!("unsafe path '{rel}'"));
    }
    if name.chars().any(char::is_control) {
        return Some(format!("unsafe path '{rel}': control characters"));
    }
    None
}

fn pages_walk(dir: &Path, prefix: &str, files: &mut Vec<PagesFile>) -> Result<(), String> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|raw| format!("unsafe path under {}: {raw:?} is not UTF-8", dir.display()))?;
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if let Some(error) = pages_component_error(&rel, &name) {
            return Err(error);
        }
        if prefix.is_empty() {
            for (reject, what) in REJECTED_ROOTS {
                if name == reject {
                    return Err(format!(
                        "site contains '{rel}', {what}; cloudflare_pages_deploy publishes static files only"
                    ));
                }
            }
        }
        if SKIPPED_NAMES.contains(&name.as_str()) {
            return Err(format!(
                "site contains '{rel}', which Wrangler would silently skip; remove it from the bundle"
            ));
        }
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot stat {rel}: {error}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "site contains symlink '{rel}'; bundles must hold regular files"
            ));
        }
        if meta.is_dir() {
            pages_walk(&path, &rel, files)?;
        } else if meta.is_file() {
            if meta.len() > MAX_FILE_BYTES {
                return Err(format!(
                    "'{rel}' is {} bytes; Cloudflare Pages allows at most {MAX_FILE_BYTES} bytes per file",
                    meta.len()
                ));
            }
            files.push(PagesFile {
                path: rel,
                source: path,
                size: meta.len(),
            });
        } else {
            return Err(format!("site contains special file '{rel}'"));
        }
        if files.len() > MAX_FILES {
            return Err(format!(
                "site holds more than {MAX_FILES} files; Cloudflare Pages allows at most {MAX_FILES}"
            ));
        }
    }
    Ok(())
}

/// Lists the files of one static site directory after validating them.
pub fn pages_scan_site(root: &Path) -> Result<Vec<PagesFile>, String> {
    let meta = std::fs::symlink_metadata(root)
        .map_err(|error| format!("site {} is missing: {error}", root.display()))?;
    if !meta.is_dir() {
        return Err(format!("site {} is not a directory", root.display()));
    }
    let mut files = Vec::new();
    pages_walk(root, "", &mut files)?;
    if files.is_empty() {
        return Err(format!("site {} holds no files", root.display()));
    }
    if !files.iter().any(|file| file.path == "index.html") {
        return Err(format!(
            "site {} has no index.html at its root",
            root.display()
        ));
    }
    Ok(files)
}

/// Builds the Wrangler arguments for one Pages deploy.
pub fn pages_wrangler_args(staged: &Path, project: &str, branch: &str) -> Vec<OsString> {
    vec![
        "pages".into(),
        "deploy".into(),
        staged.into(),
        "--project-name".into(),
        project.into(),
        "--branch".into(),
        branch.into(),
        "--commit-dirty=true".into(),
    ]
}

/// Removes the token and control characters from tool output.
pub fn pages_redact(text: &str, token: &str) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| *c == '\n' || !c.is_control())
        .collect();
    if token.is_empty() {
        cleaned
    } else {
        cleaned.replace(token, "[redacted]")
    }
}

/// Reads the deployment Wrangler wrote to its output file.
pub fn pages_parse_output(
    text: &str,
    project: &str,
    expected_environment: &str,
) -> Result<PagesDeployment, String> {
    let mut found = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: serde_json::Value = serde_json::from_str(line).map_err(|_| {
            "wrangler wrote malformed output; the deployment is unconfirmed".to_owned()
        })?;
        if value.get("type").and_then(serde_json::Value::as_str) == Some("pages-deploy-detailed") {
            found = Some(value);
        }
    }
    let Some(value) = found else {
        return Err(
            "wrangler output holds no pages deployment record; the deployment is unconfirmed"
                .to_owned(),
        );
    };
    let text_field = |key: &str| value.get(key).and_then(serde_json::Value::as_str);
    let id = text_field("deployment_id").unwrap_or_default();
    let id_ok = pages_name_ok(id, 64, |c| c.is_ascii_alphanumeric() || c == '-');
    if !id_ok {
        return Err("wrangler output has no valid deployment_id".to_owned());
    }
    let url = text_field("url").unwrap_or_default();
    let url_ok = url.starts_with("https://")
        && url.len() > "https://".len()
        && url.chars().all(|c| c.is_ascii_graphic());
    if !url_ok {
        return Err("wrangler output has no valid https url".to_owned());
    }
    if let Some(reported) = text_field("pages_project") {
        if reported != project {
            return Err(format!(
                "wrangler deployed to project '{reported}', expected '{project}'"
            ));
        }
    }
    let environment = match text_field("environment") {
        Some(reported) if reported == expected_environment => reported.to_owned(),
        Some(reported) => {
            return Err(format!(
                "deployment {id} landed in {reported}, expected {expected_environment}; check the project's production branch"
            ))
        }
        None => expected_environment.to_owned(),
    };
    Ok(PagesDeployment {
        id: id.to_owned(),
        url: url.to_owned(),
        environment,
    })
}

fn pages_token_error(token: &str) -> Option<&'static str> {
    if token.is_empty() {
        return Some("CLOUDFLARE_API_TOKEN is not set; publishing nothing");
    }
    if token.len() > 512 || token.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("CLOUDFLARE_API_TOKEN is malformed; publishing nothing");
    }
    None
}

fn pages_stage(files: &[PagesFile], staged: &Path) -> Result<(), String> {
    for file in files {
        let dest = staged.join(&file.path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot stage {}: {error}", file.path))?;
        }
        std::fs::copy(&file.source, &dest)
            .map_err(|error| format!("cannot stage {}: {error}", file.path))?;
    }
    Ok(())
}

fn pages_failure_text(output: &std::process::Output, token: &str) -> String {
    let mut text = String::from_utf8_lossy(&output.stderr).into_owned();
    text.push('\n');
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    let redacted = pages_redact(&text, token);
    let lines: Vec<&str> = redacted
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let start = lines.len().saturating_sub(TAIL_LINES);
    let mut tail = lines[start..].join("\n");
    let lower = redacted.to_lowercase();
    if lower.contains("authentication error")
        || lower.contains("invalid access token")
        || lower.contains("unauthorized")
        || lower.contains("code: 10000")
    {
        tail.push_str(
            "\nauthentication failed: check that CLOUDFLARE_API_TOKEN is valid and can edit Cloudflare Pages for this account",
        );
    }
    tail
}

fn pages_fail(err: &mut dyn std::io::Write, name: &str, message: &str) -> i32 {
    let _ = writeln!(err, "cloudflare_pages_deploy: {name}: {message}");
    1
}

/// Stages one site, runs Wrangler and reports the deployment.
pub fn pages_deploy_run(
    launch: &PagesLaunch<'_>,
    site: &Path,
    wrangler: &Path,
    env: &PagesEnv,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> i32 {
    let name = launch.deploy_name;
    if let Some(message) = pages_identity_error(
        launch.account_id,
        launch.project,
        launch.branch,
        launch.production_branch,
    ) {
        return pages_fail(err, name, &message);
    }
    if let Some(message) = pages_token_error(&env.token) {
        return pages_fail(err, name, message);
    }
    if let Some(account) = env.account_id.as_deref().filter(|value| !value.is_empty()) {
        if account != launch.account_id {
            return pages_fail(
                err,
                name,
                "CLOUDFLARE_ACCOUNT_ID does not match the account_id of this target; publishing nothing",
            );
        }
    }
    let files = match pages_scan_site(site) {
        Ok(files) => files,
        Err(message) => return pages_fail(err, name, &message),
    };
    let intent = pages_intent(launch.branch, launch.production_branch);
    let work = match super::tempfile_stage_dir("cloudflare-pages-") {
        Ok(dir) => dir,
        Err(error) => {
            return pages_fail(
                err,
                name,
                &format!("cannot create a work directory: {error}"),
            )
        }
    };
    let code = pages_deploy_staged(launch, &files, wrangler, env, &work, intent, out, err);
    let _ = std::fs::remove_dir_all(&work);
    code
}

#[allow(clippy::too_many_arguments)]
fn pages_deploy_staged(
    launch: &PagesLaunch<'_>,
    files: &[PagesFile],
    wrangler: &Path,
    env: &PagesEnv,
    work: &Path,
    intent: &str,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> i32 {
    let name = launch.deploy_name;
    let staged = work.join("site");
    let home = work.join("home");
    let result_file = work.join("wrangler-output.ndjson");
    let log_dir = work.join("logs");
    for dir in [&staged, &home, &log_dir] {
        if let Err(error) = std::fs::create_dir_all(dir) {
            return pages_fail(
                err,
                name,
                &format!("cannot create {}: {error}", dir.display()),
            );
        }
    }
    if let Err(message) = pages_stage(files, &staged) {
        return pages_fail(err, name, &message);
    }
    let mut command = std::process::Command::new(wrangler);
    command
        .args(pages_wrangler_args(&staged, launch.project, launch.branch))
        .env_clear()
        .current_dir(work)
        .stdin(std::process::Stdio::null());
    for (key, value) in &env.passthrough {
        command.env(key, value);
    }
    command
        .env("CLOUDFLARE_API_TOKEN", &env.token)
        .env("CLOUDFLARE_ACCOUNT_ID", launch.account_id)
        .env("CI", "1")
        .env("WRANGLER_SEND_METRICS", "false")
        .env("WRANGLER_OUTPUT_FILE_PATH", &result_file)
        .env("WRANGLER_LOG_PATH", &log_dir)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"));
    let output = match command.output() {
        Ok(output) => output,
        Err(error) => {
            return pages_fail(
                err,
                name,
                &format!("cannot run wrangler {}: {error}", wrangler.display()),
            )
        }
    };
    if !output.status.success() {
        let code = output.status.code().unwrap_or(1);
        let tail = pages_failure_text(&output, &env.token);
        let _ = writeln!(
            err,
            "cloudflare_pages_deploy: {name}: wrangler failed with exit {code}; nothing is confirmed published\n{tail}"
        );
        return if code == 0 { 1 } else { code };
    }
    let text = std::fs::read_to_string(&result_file).unwrap_or_default();
    let deployment = match pages_parse_output(&text, launch.project, intent) {
        Ok(deployment) => deployment,
        Err(message) => return pages_fail(err, name, &pages_redact(&message, &env.token)),
    };
    let bytes: u64 = files.iter().map(|file| file.size).sum();
    let _ = writeln!(
        out,
        "cloudflare_pages_deploy: deployed {} (branch {}, {}, profile: {})",
        launch.project, launch.branch, deployment.environment, env.profile
    );
    let _ = writeln!(out, "  account: {}", launch.account_id);
    let _ = writeln!(out, "  project: {}", launch.project);
    let _ = writeln!(out, "  branch: {}", launch.branch);
    let _ = writeln!(out, "  intent: {}", deployment.environment);
    let _ = writeln!(out, "  files: {} ({bytes} bytes)", files.len());
    let _ = writeln!(out, "  deployment: {}", deployment.id);
    let _ = writeln!(out, "  url: {}", deployment.url);
    0
}

/// Runs the Cloudflare Pages deploy launcher.
pub fn cloudflare_pages_main(launch: &PagesLaunch<'_>, argv: &[String]) -> i32 {
    let name = launch.deploy_name;
    if argv.len() > 1 {
        eprintln!(
            "cloudflare_pages_deploy: {name}: this deploy target takes no arguments; the site, project and branch are pinned at analysis time"
        );
        return 1;
    }
    if std::env::var_os("CLOUDFLARE_PAGES_DRY_RUN").is_some() {
        eprintln!(
            "cloudflare_pages_deploy: {name}: CLOUDFLARE_PAGES_DRY_RUN is not supported; check with dx deploy and publish with dx deploy --apply"
        );
        return 1;
    }
    let site = match super::resolve_runfile(launch.site_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("cloudflare_pages_deploy: {name}: {error}");
            return 1;
        }
    };
    let wrangler = match super::resolve_runfile(launch.wrangler_rloc) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("cloudflare_pages_deploy: {name}: {error}");
            return 1;
        }
    };
    let passthrough = PASSTHROUGH_ENV
        .iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| ((*key).to_owned(), value))
        })
        .collect();
    let env = PagesEnv {
        token: std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
        account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID").ok(),
        passthrough,
        profile: std::env::var("DX_PROFILE").unwrap_or_else(|_| "<unset>".to_owned()),
    };
    pages_deploy_run(
        launch,
        &site,
        &wrangler,
        &env,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}

/// Checks one Pages deployment locally and writes its manifest.
pub fn cloudflare_pages_verify_main(argv: &[String]) -> i32 {
    if argv.len() != 7 {
        return super::bin_usage(
            "cloudflare_verify",
            "<account_id> <project> <branch> <production_branch> <site> <out>",
        );
    }
    let (account, project, branch, production) = (&argv[1], &argv[2], &argv[3], &argv[4]);
    if let Some(message) = pages_identity_error(account, project, branch, production) {
        eprintln!("cloudflare_pages_deploy: {message}");
        return 1;
    }
    let files = match pages_scan_site(Path::new(&argv[5])) {
        Ok(files) => files,
        Err(message) => {
            eprintln!("cloudflare_pages_deploy: {message}");
            return 1;
        }
    };
    let manifest = serde_json::json!({
        "account_id": account,
        "project": project,
        "branch": branch,
        "intent": pages_intent(branch, production),
        "files": files
            .iter()
            .map(|file| serde_json::json!({"path": file.path, "size": file.size}))
            .collect::<Vec<_>>(),
    });
    let text = manifest.to_string() + "\n";
    match std::fs::write(&argv[6], text) {
        Ok(()) => 0,
        Err(error) => super::bin_cannot("cloudflare_verify", "write", Path::new(&argv[6]), error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCOUNT: &str = "0123456789abcdef0123456789abcdef";
    const TOKEN: &str = "cf-secret-token-123";

    fn launch<'a>(branch: &'a str) -> PagesLaunch<'a> {
        PagesLaunch {
            deploy_name: "pages_demo",
            account_id: ACCOUNT,
            project: "my-site",
            branch,
            production_branch: "main",
            site_rloc: "",
            wrangler_rloc: "",
        }
    }

    fn site(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("site");
        for (rel, body) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, body).expect("write");
        }
        dir
    }

    fn env(token: &str) -> PagesEnv {
        PagesEnv {
            token: token.to_owned(),
            account_id: None,
            passthrough: vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())],
            profile: "release".to_owned(),
        }
    }

    fn fake_wrangler(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("wrangler");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    const RECORD_AND_SUCCEED: &str = r#"
{
  echo "argv: $*"
  echo "cwd: $(pwd)"
  echo "token: $CLOUDFLARE_API_TOKEN"
  echo "account: $CLOUDFLARE_ACCOUNT_ID"
  echo "ci: $CI"
  echo "metrics: $WRANGLER_SEND_METRICS"
  echo "home: $HOME"
  echo "leak: ${AMBIENT_SECRET:-unset}"
  echo "closure:"
  (cd "$3" && find . -type f | sort)
} > "$RECORD"
printf '%s\n' '{"type":"wrangler-session","version":1}' > "$WRANGLER_OUTPUT_FILE_PATH"
printf '%s\n' "{\"type\":\"pages-deploy-detailed\",\"version\":1,\"pages_project\":\"my-site\",\"deployment_id\":\"dep-123\",\"url\":\"https://abc.my-site.pages.dev\",\"environment\":\"$ENVIRONMENT\"}" >> "$WRANGLER_OUTPUT_FILE_PATH"
"#;

    fn run_capture(
        branch: &str,
        files: &[(&str, &str)],
        body: &str,
        environment: &str,
        token: &str,
    ) -> (i32, String, String, String) {
        let tools = tempfile::TempDir::new().expect("tools");
        let record = tools.path().join("record.txt");
        let script = format!(
            "RECORD={}\nENVIRONMENT={environment}\n{body}",
            record.display()
        );
        let wrangler = fake_wrangler(tools.path(), &script);
        let site = site(files);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let run_env = env(token);
        let code = pages_deploy_run(
            &launch(branch),
            site.path(),
            &wrangler,
            &run_env,
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8_lossy(&out).into_owned(),
            String::from_utf8_lossy(&err).into_owned(),
            std::fs::read_to_string(&record).unwrap_or_default(),
        )
    }

    const SITE: [(&str, &str); 3] = [
        ("index.html", "<html></html>"),
        ("assets/data.txt", "data"),
        ("app/app_bg.wasm", "wasm"),
    ];

    #[test]
    fn preview_deploy_passes_pinned_argv_and_reports_identity() {
        let (code, out, err, record) =
            run_capture("feature/x", &SITE, RECORD_AND_SUCCEED, "preview", TOKEN);
        assert_eq!(code, 0, "{err}");
        assert!(record.contains("--project-name my-site --branch feature/x --commit-dirty=true"));
        assert!(record.starts_with("argv: pages deploy "), "{record}");
        assert!(record.contains(&format!("account: {ACCOUNT}")));
        assert!(record.contains("ci: 1"));
        assert!(record.contains("metrics: false"));
        assert!(record.contains("leak: unset"));
        assert!(
            record.contains("cwd: ") && record.contains("cloudflare-pages-"),
            "{record}"
        );
        assert!(out.contains("deployed my-site (branch feature/x, preview, profile: release)"));
        assert!(out.contains(&format!("  account: {ACCOUNT}")));
        assert!(out.contains("  project: my-site"));
        assert!(out.contains("  intent: preview"));
        assert!(out.contains("  deployment: dep-123"));
        assert!(out.contains("  url: https://abc.my-site.pages.dev"));
        assert!(out.contains("  files: 3 (21 bytes)"));
        assert!(!out.contains(TOKEN) && !err.contains(TOKEN));
    }

    #[test]
    fn staged_closure_is_exactly_the_declared_site() {
        let (code, _, err, record) =
            run_capture("main", &SITE, RECORD_AND_SUCCEED, "production", TOKEN);
        assert_eq!(code, 0, "{err}");
        let closure = record.split("closure:\n").nth(1).expect("closure");
        assert_eq!(
            closure.lines().collect::<Vec<_>>(),
            vec!["./app/app_bg.wasm", "./assets/data.txt", "./index.html"]
        );
    }

    #[test]
    fn production_branch_reports_production() {
        let (code, out, err, _) =
            run_capture("main", &SITE, RECORD_AND_SUCCEED, "production", TOKEN);
        assert_eq!(code, 0, "{err}");
        assert!(
            out.contains("(branch main, production, profile: release)"),
            "{out}"
        );
        assert!(out.contains("  intent: production"));
    }

    #[test]
    fn environment_mismatch_is_refused() {
        let (code, out, err, _) = run_capture("main", &SITE, RECORD_AND_SUCCEED, "preview", TOKEN);
        assert_eq!(code, 1);
        assert!(out.is_empty(), "{out}");
        assert!(
            err.contains("landed in preview, expected production"),
            "{err}"
        );
    }

    #[test]
    fn missing_token_runs_nothing() {
        let (code, out, err, record) =
            run_capture("main", &SITE, RECORD_AND_SUCCEED, "production", "");
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.contains("CLOUDFLARE_API_TOKEN is not set"), "{err}");
        assert!(record.is_empty(), "wrangler must not run");
    }

    #[test]
    fn malformed_token_runs_nothing() {
        let (code, _, err, record) =
            run_capture("main", &SITE, RECORD_AND_SUCCEED, "production", "a b");
        assert_eq!(code, 1);
        assert!(err.contains("CLOUDFLARE_API_TOKEN is malformed"), "{err}");
        assert!(!err.contains("a b"));
        assert!(record.is_empty());
    }

    #[test]
    fn mismatched_ambient_account_runs_nothing() {
        let tools = tempfile::TempDir::new().expect("tools");
        let wrangler = fake_wrangler(tools.path(), "exit 0");
        let site = site(&SITE);
        let mut run_env = env(TOKEN);
        run_env.account_id = Some("f".repeat(32));
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = pages_deploy_run(
            &launch("main"),
            site.path(),
            &wrangler,
            &run_env,
            &mut out,
            &mut err,
        );
        assert_eq!(code, 1);
        assert!(String::from_utf8_lossy(&err).contains("CLOUDFLARE_ACCOUNT_ID does not match"));
    }

    #[test]
    fn authentication_failure_propagates_exit_and_redacts() {
        let body = r#"
echo "Authentication error [code: 10000] for token $CLOUDFLARE_API_TOKEN" >&2
exit 7
"#;
        let (code, out, err, _) = run_capture("main", &SITE, body, "production", TOKEN);
        assert_eq!(code, 7);
        assert!(out.is_empty());
        assert!(err.contains("wrangler failed with exit 7"), "{err}");
        assert!(err.contains("[redacted]"), "{err}");
        assert!(
            err.contains("authentication failed: check that CLOUDFLARE_API_TOKEN"),
            "{err}"
        );
        assert!(!err.contains(TOKEN), "{err}");
    }

    #[test]
    fn malformed_wrangler_output_is_unconfirmed() {
        let body = "echo 'not json' > \"$WRANGLER_OUTPUT_FILE_PATH\"";
        let (code, out, err, _) = run_capture("main", &SITE, body, "production", TOKEN);
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert!(err.contains("malformed output"), "{err}");
    }

    #[test]
    fn missing_output_record_is_unconfirmed() {
        let (code, _, err, _) = run_capture("main", &SITE, "exit 0", "production", TOKEN);
        assert_eq!(code, 1);
        assert!(err.contains("no pages deployment record"), "{err}");
    }

    #[test]
    fn parse_output_rejects_bad_fields() {
        let ok = r#"{"type":"pages-deploy-detailed","deployment_id":"d-1","url":"https://x.pages.dev","pages_project":"p"}"#;
        assert_eq!(
            pages_parse_output(ok, "p", "preview"),
            Ok(PagesDeployment {
                id: "d-1".to_owned(),
                url: "https://x.pages.dev".to_owned(),
                environment: "preview".to_owned()
            })
        );
        let bad_url = ok.replace("https://x.pages.dev", "http://x.pages.dev");
        assert!(pages_parse_output(&bad_url, "p", "preview").is_err());
        let bad_id = ok.replace("d-1", "d 1");
        assert!(pages_parse_output(&bad_id, "p", "preview").is_err());
        let other = ok.replace("\"p\"", "\"other\"");
        assert!(pages_parse_output(&other, "p", "preview")
            .expect_err("project")
            .contains("project 'other'"));
    }

    #[test]
    fn scan_rejects_executable_payloads() {
        for bad in ["_worker.js", "functions/api.js", "_routes.json"] {
            let dir = site(&[("index.html", "x"), (bad, "x")]);
            let error = pages_scan_site(dir.path()).expect_err(bad);
            assert!(error.contains("static files only"), "{bad}: {error}");
        }
    }

    #[test]
    fn scan_rejects_missing_index_empty_and_skipped_names() {
        let dir = site(&[("a.txt", "x")]);
        assert!(pages_scan_site(dir.path())
            .expect_err("index")
            .contains("no index.html"));
        let empty = tempfile::TempDir::new().expect("empty");
        assert!(pages_scan_site(empty.path())
            .expect_err("empty")
            .contains("holds no files"));
        let skipped = site(&[("index.html", "x"), ("node_modules/a.js", "x")]);
        assert!(pages_scan_site(skipped.path())
            .expect_err("skip")
            .contains("silently skip"));
        let missing = empty.path().join("absent");
        assert!(pages_scan_site(&missing)
            .expect_err("missing")
            .contains("is missing"));
        let file = dir.path().join("a.txt");
        assert!(pages_scan_site(&file)
            .expect_err("file")
            .contains("not a directory"));
    }

    #[test]
    fn scan_rejects_symlinks_and_oversize_files() {
        let dir = site(&[("index.html", "x")]);
        std::os::unix::fs::symlink("/etc/passwd", dir.path().join("link")).expect("symlink");
        assert!(pages_scan_site(dir.path())
            .expect_err("symlink")
            .contains("symlink 'link'"));
        let big = site(&[("index.html", "x")]);
        let file = std::fs::File::create(big.path().join("big.bin")).expect("big");
        file.set_len(MAX_FILE_BYTES + 1).expect("len");
        assert!(pages_scan_site(big.path())
            .expect_err("big")
            .contains("at most"));
    }

    #[test]
    fn scan_lists_files_sorted_with_sizes() {
        let dir = site(&SITE);
        let files = pages_scan_site(dir.path()).expect("scan");
        let listed: Vec<(&str, u64)> = files.iter().map(|f| (f.path.as_str(), f.size)).collect();
        assert_eq!(
            listed,
            vec![
                ("app/app_bg.wasm", 4),
                ("assets/data.txt", 4),
                ("index.html", 13)
            ]
        );
    }

    #[test]
    fn identity_validation() {
        assert_eq!(
            pages_identity_error(ACCOUNT, "my-site", "main", "main"),
            None
        );
        for (account, project, branch, prod, want) in [
            ("short", "p", "b", "main", "invalid account_id"),
            (ACCOUNT, "My_Site", "b", "main", "invalid project"),
            (ACCOUNT, "-p", "b", "main", "invalid project"),
            (ACCOUNT, "p", "", "main", "invalid branch"),
            (ACCOUNT, "p", "--evil", "main", "invalid branch"),
            (ACCOUNT, "p", "a b", "main", "invalid branch"),
            (ACCOUNT, "p", "b", "a..b", "invalid production_branch"),
        ] {
            let error = pages_identity_error(account, project, branch, prod).expect(want);
            assert!(error.contains(want), "{error}");
        }
    }

    #[test]
    fn intent_follows_the_production_branch() {
        assert_eq!(pages_intent("main", "main"), "production");
        assert_eq!(pages_intent("dev", "main"), "preview");
    }

    #[test]
    fn redact_strips_token_and_control_characters() {
        assert_eq!(
            pages_redact("a tok b \u{1b}[31mred\u{7}", "tok"),
            "a [redacted] b [31mred"
        );
        assert_eq!(pages_redact("plain", ""), "plain");
    }

    #[test]
    fn main_refuses_arguments_and_dry_run() {
        let launch = launch("main");
        let argv = vec!["prog".to_owned(), "--dry-run".to_owned()];
        assert_eq!(cloudflare_pages_main(&launch, &argv), 1);
        let argv = vec!["prog".to_owned()];
        assert_eq!(cloudflare_pages_main(&launch, &argv), 1);
    }

    #[test]
    fn verify_main_writes_the_manifest_and_rejects_bad_sites() {
        let dir = site(&SITE);
        let out = tempfile::TempDir::new().expect("out");
        let manifest = out.path().join("m.json");
        let argv = |site: &Path, branch: &str| {
            vec![
                "v".to_owned(),
                ACCOUNT.to_owned(),
                "my-site".to_owned(),
                branch.to_owned(),
                "main".to_owned(),
                site.to_string_lossy().into_owned(),
                manifest.to_string_lossy().into_owned(),
            ]
        };
        assert_eq!(cloudflare_pages_verify_main(&argv(dir.path(), "dev")), 0);
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest).expect("read")).expect("json");
        assert_eq!(value["intent"], "preview");
        assert_eq!(value["project"], "my-site");
        assert_eq!(value["files"].as_array().map(Vec::len), Some(3));
        assert_eq!(cloudflare_pages_verify_main(&argv(dir.path(), "--x")), 1);
        let bad = site(&[("index.html", "x"), ("_worker.js", "x")]);
        assert_eq!(cloudflare_pages_verify_main(&argv(bad.path(), "dev")), 1);
        assert_eq!(cloudflare_pages_verify_main(&["v".to_owned()]), 1);
    }
}
