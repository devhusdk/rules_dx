//! opam publisher: opam file identity check, lint with the pinned opam, and the opam-repository pull request.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, pages_redact,
    scrubbed_command,
};
use base64::Engine as _;
use std::io::Read;
use std::path::Path;

/// Owner approval gate for opam_deploy.
pub(crate) const OPAM_GATE: OwnerGate = OwnerGate {
    tool: "opam_deploy",
    action: "pull request",
    credentials: &["OPAM_API_TOKEN"],
    approved: "OPAM_PUBLISH_APPROVED",
};

const GITHUB_API: &str = "https://api.github.com";
const UPSTREAM: &str = "ocaml/opam-repository";
const UPSTREAM_BASE: &str = "master";

/// Reads the optional `name:` and `version:` fields of an opam file.
pub fn opam_read_identity(text: &str) -> (Option<String>, Option<String>) {
    let field = |key: &str| {
        text.lines().find_map(|line| {
            let value = line.strip_prefix(key)?.strip_prefix(':')?;
            let value = value.split(" #").next().unwrap_or("").trim();
            Some(value.trim_matches('"').to_owned())
        })
    };
    (field("name"), field("version"))
}

/// Checks the opam file's own name and version fields against the declared ones.
pub fn opam_check_identity(text: &str, package: &str, version: &str) -> Result<(), String> {
    let (name, declared) = opam_read_identity(text);
    if let Some(name) = name.filter(|name| name != package) {
        return Err(format!(
            "package mismatch: opam file declares {name}, BUILD declares {package}"
        ));
    }
    if let Some(declared) = declared.filter(|declared| declared != version) {
        return Err(format!(
            "version mismatch: opam file declares {declared}, BUILD declares {version}"
        ));
    }
    Ok(())
}

/// Path of the opam file inside opam-repository.
pub fn opam_repository_path(package: &str, version: &str) -> String {
    format!("packages/{package}/{package}.{version}/opam")
}

/// Git branch name for the release: characters outside `[A-Za-z0-9._-]` become `-`, dot runs collapse to one dot.
pub fn opam_branch_name(package: &str, version: &str) -> String {
    let mut branch = String::new();
    for ch in format!("{package}.{version}").chars() {
        let ch = if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            ch
        } else {
            '-'
        };
        if ch == '.' && branch.ends_with('.') {
            continue;
        }
        branch.push(ch);
    }
    let trimmed = branch.trim_matches(|ch| ch == '.' || ch == '-');
    let mut branch = trimmed.to_owned();
    if branch.ends_with(".lock") {
        branch.push_str("-release");
    }
    branch
}

/// Runs `opam lint` on the file with the pinned opam and an empty root.
fn opam_lint(opam: &Path, file: &Path, home: &Path) -> bool {
    let home_text = home.to_string_lossy().into_owned();
    let mut cmd = scrubbed_command(
        opam,
        &[PUBLISH_CHILD_ENV_KEEP],
        &[
            ("HOME", home_text.as_str()),
            ("OPAMROOT", home_text.as_str()),
            ("OPAMYES", "1"),
        ],
    );
    cmd.arg("lint").arg(file);
    match cmd.output() {
        Ok(out) => {
            print!("{}", String::from_utf8_lossy(&out.stdout));
            eprint!("{}", String::from_utf8_lossy(&out.stderr));
            out.status.success()
        }
        Err(error) => {
            eprintln!("opam_deploy: cannot run {}: {error}", opam.display());
            false
        }
    }
}

struct Github<'a> {
    agent: ureq::Agent,
    token: &'a str,
}

impl Github<'_> {
    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{GITHUB_API}{path}");
        let header = |req: ureq::RequestBuilder<ureq::typestate::WithoutBody>| {
            req.header("Authorization", format!("Bearer {}", self.token))
                .header("Accept", "application/vnd.github+json")
                .header("User-Agent", "dx-opam-deploy")
        };
        let result = match (method, body) {
            ("GET", _) => header(self.agent.get(&url)).call(),
            (_, body) => {
                let payload = body.unwrap_or(serde_json::Value::Null).to_string();
                let request = match method {
                    "PUT" => self.agent.put(&url),
                    _ => self.agent.post(&url),
                };
                request
                    .header("Authorization", format!("Bearer {}", self.token))
                    .header("Accept", "application/vnd.github+json")
                    .header("User-Agent", "dx-opam-deploy")
                    .header("Content-Type", "application/json")
                    .send(payload.as_bytes())
            }
        };
        let mut response = result.map_err(|error| pages_redact(&error.to_string(), self.token))?;
        let status = response.status().as_u16();
        let mut text = Vec::new();
        let _ = response
            .body_mut()
            .as_reader()
            .take(1 << 20)
            .read_to_end(&mut text);
        let text = pages_redact(&String::from_utf8_lossy(&text), self.token);
        if !(200..300).contains(&status) {
            return Err(format!("{method} {path}: HTTP {status}: {}", text.trim()));
        }
        Ok((status, serde_json::from_str(&text).unwrap_or_default()))
    }
}

fn text_field(value: &serde_json::Value, pointer: &str) -> Result<String, String> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("response has no {pointer}"))
}

/// Forks opam-repository, commits the opam file on a branch and opens the pull request.
fn opam_live_pull_request(
    package: &str,
    version: &str,
    contents: &[u8],
    token: &str,
) -> Result<String, String> {
    let github = Github {
        agent: crate::http::publish_agent(),
        token,
    };
    let fork = text_field(
        &github
            .call(
                "POST",
                &format!("/repos/{UPSTREAM}/forks"),
                Some(serde_json::json!({})),
            )?
            .1,
        "/full_name",
    )?;
    let mut ready = false;
    for _ in 0..30 {
        if github.call("GET", &format!("/repos/{fork}"), None).is_ok() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    if !ready {
        return Err(format!("fork {fork} did not become available"));
    }
    github.call(
        "POST",
        &format!("/repos/{fork}/merge-upstream"),
        Some(serde_json::json!({"branch": UPSTREAM_BASE})),
    )?;
    let base = text_field(
        &github
            .call(
                "GET",
                &format!("/repos/{UPSTREAM}/git/ref/heads/{UPSTREAM_BASE}"),
                None,
            )?
            .1,
        "/object/sha",
    )?;
    let branch = opam_branch_name(package, version);
    let owner = fork.split('/').next().unwrap_or_default().to_owned();
    match github.call(
        "POST",
        &format!("/repos/{fork}/git/refs"),
        Some(serde_json::json!({"ref": format!("refs/heads/{branch}"), "sha": base})),
    ) {
        Ok(_) => {}
        Err(error) if error.contains("HTTP 422") && error.contains("already exists") => {}
        Err(error) => return Err(error),
    }
    let path = opam_repository_path(package, version);
    let existing = match github.call(
        "GET",
        &format!("/repos/{fork}/contents/{path}?ref={branch}"),
        None,
    ) {
        Ok((_, value)) => Some(text_field(&value, "/sha")?),
        Err(error) if error.contains("HTTP 404") => None,
        Err(error) => return Err(error),
    };
    let mut put = serde_json::json!({
        "message": format!("Add {package}.{version}"),
        "content": base64::engine::general_purpose::STANDARD.encode(contents),
        "branch": branch,
    });
    if let Some(sha) = existing {
        put["sha"] = serde_json::Value::String(sha);
    }
    github.call(
        "PUT",
        &format!("/repos/{fork}/contents/{path}"),
        Some(put),
    )?;
    let head = format!("{owner}:{branch}");
    match github.call(
        "POST",
        &format!("/repos/{UPSTREAM}/pulls"),
        Some(serde_json::json!({
            "title": format!("[new release] {package} ({version})"),
            "head": head,
            "base": UPSTREAM_BASE,
            "body": format!("Adds {package}.{version}."),
        })),
    ) {
        Ok((_, pull)) => text_field(&pull, "/html_url"),
        Err(error) if error.contains("HTTP 422") && error.contains("already exists") => {
            let (_, pulls) = github.call(
                "GET",
                &format!("/repos/{UPSTREAM}/pulls?state=open&head={head}"),
                None,
            )?;
            text_field(&pulls, "/0/html_url")
        }
        Err(error) => Err(error),
    }
}

/// Runs the opam deploy tool.
pub fn opam_main(
    opam_rloc: &str,
    opam_file_rloc: &str,
    target_name: &str,
    package: &str,
    version: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "opam_deploy",
        "the pull request adds exactly the opam file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    if opam_rloc.is_empty() {
        eprintln!("opam_deploy: {target_name}: no opam is pinned for this platform");
        return 1;
    }
    let (Some(opam), Some(file)) = (run.resolve(opam_rloc), run.resolve(opam_file_rloc)) else {
        return 1;
    };
    let contents = match std::fs::read(&file) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!(
                "opam_deploy: {target_name}: cannot read {}: {error}",
                file.display()
            );
            return 1;
        }
    };
    if let Err(error) = opam_check_identity(&String::from_utf8_lossy(&contents), package, version) {
        eprintln!("opam_deploy: {target_name}: {error}");
        return 1;
    }
    let home = run.outdir().join(format!("{target_name}-opam-home"));
    if let Err(error) = std::fs::create_dir_all(&home) {
        eprintln!(
            "opam_deploy: {target_name}: cannot create {}: {error}",
            home.display()
        );
        return 1;
    }
    if !opam_lint(&opam, &file, &home) {
        eprintln!("opam_deploy: {target_name}: opam lint failed");
        return 1;
    }
    if OPAM_GATE.requested(env_or_empty) {
        let secrets = match OPAM_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        return match opam_live_pull_request(
            package,
            version,
            &contents,
            secrets.get("OPAM_API_TOKEN"),
        ) {
            Ok(url) => {
                println!("opam_deploy: opened {url}");
                0
            }
            Err(error) => {
                eprintln!("opam_deploy: {target_name}: {error}");
                1
            }
        };
    }
    println!(
        "opam_deploy: would open a pull request to {UPSTREAM} adding {} (build_mode: {})",
        opam_repository_path(package, version),
        deploy_build_mode()
    );
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_checks_identity() {
        let text = "opam-version: \"2.0\"\nname: \"demo\"\nversion: \"1.0.0\" # pinned\n";
        assert_eq!(
            opam_read_identity(text),
            (Some("demo".to_owned()), Some("1.0.0".to_owned()))
        );
        assert_eq!(opam_check_identity(text, "demo", "1.0.0"), Ok(()));
        assert!(
            opam_check_identity(text, "other", "1.0.0")
                .expect_err("name")
                .contains("BUILD declares other")
        );
        assert!(
            opam_check_identity(text, "demo", "2.0.0")
                .expect_err("version")
                .contains("BUILD declares 2.0.0")
        );
    }

    #[test]
    fn identity_fields_are_optional() {
        let text = "opam-version: \"2.0\"\nsynopsis: \"x\"\n";
        assert_eq!(opam_read_identity(text), (None, None));
        assert_eq!(opam_check_identity(text, "demo", "1.0.0"), Ok(()));
    }

    #[test]
    fn branch_name_is_a_valid_git_ref() {
        assert_eq!(opam_branch_name("demo", "1.0.0"), "demo.1.0.0");
        assert_eq!(opam_branch_name("demo", "1.2.0~rc1"), "demo.1.2.0-rc1");
        assert_eq!(opam_branch_name("demo", "1..2"), "demo.1.2");
        assert_eq!(opam_branch_name("demo+x", "1.0.0+a b"), "demo-x.1.0.0-a-b");
        assert_eq!(opam_branch_name("demo", "1."), "demo.1");
        assert_eq!(opam_branch_name("demo", "lock"), "demo.lock-release");
    }

    #[test]
    fn repository_path_follows_the_layout() {
        assert_eq!(
            opam_repository_path("demo", "1.0.0"),
            "packages/demo/demo.1.0.0/opam"
        );
    }
}
