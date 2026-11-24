use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, pages_redact,
    scrubbed_command,
};
use base64::Engine as _;
use dx_atomic_fs::path_io;
use std::fmt;
use std::path::Path;

pub(crate) const OPAM_GATE: OwnerGate = OwnerGate {
    tool: "opam_deploy",
    action: "pull request",
    credentials: &["OPAM_API_TOKEN"],
    approved: "OPAM_PUBLISH_APPROVED",
};

const GITHUB_API: &str = "https://api.github.com";
const UPSTREAM: &str = "ocaml/opam-repository";
const UPSTREAM_BASE: &str = "master";

#[derive(Debug)]
pub enum OpamError {
    PackageMismatch {
        declared: String,
        expected: String,
    },
    VersionMismatch {
        declared: String,
        expected: String,
    },
    Client(dx_http::Error),
    Transport {
        message: String,
    },
    Status {
        method: String,
        path: String,
        status: u16,
        body: String,
    },
    NoField {
        pointer: String,
    },
    ForkNotReady {
        fork: String,
    },
}

impl OpamError {
    fn already_exists(&self) -> bool {
        matches!(self, Self::Status { status: 422, body, .. } if body.contains("already exists"))
    }
}

impl fmt::Display for OpamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageMismatch { declared, expected } => write!(
                f,
                "package mismatch: opam file declares {declared}, BUILD declares {expected}"
            ),
            Self::VersionMismatch { declared, expected } => write!(
                f,
                "version mismatch: opam file declares {declared}, BUILD declares {expected}"
            ),
            Self::Client(source) => write!(f, "{source}"),
            Self::Transport { message } => f.write_str(message),
            Self::Status {
                method,
                path,
                status,
                body,
            } => write!(f, "{method} {path}: HTTP {status}: {body}"),
            Self::NoField { pointer } => write!(f, "response has no {pointer}"),
            Self::ForkNotReady { fork } => write!(f, "fork {fork} did not become available"),
        }
    }
}

impl std::error::Error for OpamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(source) => Some(source),
            Self::PackageMismatch { .. }
            | Self::VersionMismatch { .. }
            | Self::Transport { .. }
            | Self::Status { .. }
            | Self::NoField { .. }
            | Self::ForkNotReady { .. } => None,
        }
    }
}

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

pub fn opam_check_identity(text: &str, package: &str, version: &str) -> Result<(), OpamError> {
    let (name, declared) = opam_read_identity(text);
    if let Some(name) = name.filter(|name| name != package) {
        return Err(OpamError::PackageMismatch {
            declared: name,
            expected: package.to_owned(),
        });
    }
    if let Some(declared) = declared.filter(|declared| declared != version) {
        return Err(OpamError::VersionMismatch {
            declared,
            expected: version.to_owned(),
        });
    }
    Ok(())
}

pub fn opam_repository_path(package: &str, version: &str) -> String {
    format!("packages/{package}/{package}.{version}/opam")
}

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
    client: dx_http::Client,
    token: &'a str,
}

impl Github<'_> {
    fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<(u16, serde_json::Value), OpamError> {
        let url = format!("{GITHUB_API}{path}");
        let authorization = format!("Bearer {}", self.token);
        let mut headers = vec![
            ("Authorization", authorization.as_str()),
            ("Accept", "application/vnd.github+json"),
            ("User-Agent", "dx-opam-deploy"),
        ];
        let payload;
        let (verb, body) = match (method, body) {
            ("GET", _) => (dx_http::Method::Get, dx_http::Body::Empty),
            (_, body) => {
                payload = body.unwrap_or(serde_json::Value::Null).to_string();
                headers.push(("Content-Type", "application/json"));
                let verb = match method {
                    "PUT" => dx_http::Method::Put,
                    _ => dx_http::Method::Post,
                };
                (verb, dx_http::Body::Bytes(payload.as_bytes()))
            }
        };
        let response = self
            .client
            .send(verb, &url, &headers, body, 1 << 20)
            .map_err(|error| OpamError::Transport {
                message: pages_redact(&error.to_string(), self.token),
            })?;
        let status = response.status;
        let text = pages_redact(&response.text(), self.token);
        if !(200..300).contains(&status) {
            return Err(OpamError::Status {
                method: method.to_owned(),
                path: path.to_owned(),
                status,
                body: text.trim().to_owned(),
            });
        }
        Ok((status, serde_json::from_str(&text).unwrap_or_default()))
    }
}

fn text_field(value: &serde_json::Value, pointer: &str) -> Result<String, OpamError> {
    value
        .pointer(pointer)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| OpamError::NoField {
            pointer: pointer.to_owned(),
        })
}

fn opam_live_pull_request(
    package: &str,
    version: &str,
    contents: &[u8],
    token: &str,
) -> Result<String, OpamError> {
    let github = Github {
        client: crate::http::client().map_err(OpamError::Client)?,
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
        return Err(OpamError::ForkNotReady { fork });
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
        Err(error) if error.already_exists() => {}
        Err(error) => return Err(error),
    }
    let path = opam_repository_path(package, version);
    let existing = match github.call(
        "GET",
        &format!("/repos/{fork}/contents/{path}?ref={branch}"),
        None,
    ) {
        Ok((_, value)) => Some(text_field(&value, "/sha")?),
        Err(OpamError::Status { status: 404, .. }) => None,
        Err(error) => return Err(error),
    };
    let mut put = serde_json::json!({
        "message": format!("Add {package}.{version}"),
        "content": base64::engine::general_purpose::STANDARD.encode(contents),
        "branch": branch,
    });
    if let Some(sha) = existing
        && let Some(object) = put.as_object_mut()
    {
        object.insert("sha".to_owned(), serde_json::Value::String(sha));
    }
    github.call("PUT", &format!("/repos/{fork}/contents/{path}"), Some(put))?;
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
        Err(error) if error.already_exists() => {
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
    let contents = match path_io::read(&file) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!("opam_deploy: {target_name}: {error}");
            return 1;
        }
    };
    if let Err(error) = opam_check_identity(&String::from_utf8_lossy(&contents), package, version) {
        eprintln!("opam_deploy: {target_name}: {error}");
        return 1;
    }
    let home = run.outdir().join(format!("{target_name}-opam-home"));
    if let Err(error) = path_io::create_dir_all(&home) {
        eprintln!("opam_deploy: {target_name}: {error}");
        return 1;
    }
    if !opam_lint(&opam, &file, &home) {
        eprintln!("opam_deploy: {target_name}: opam lint failed");
        return 1;
    }
    if OwnerGate::requested(env_or_empty) {
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
        assert!(opam_check_identity(text, "demo", "1.0.0").is_ok());
        let name = opam_check_identity(text, "other", "1.0.0").expect_err("name");
        assert!(matches!(name, OpamError::PackageMismatch { .. }));
        assert_eq!(
            name.to_string(),
            "package mismatch: opam file declares demo, BUILD declares other"
        );
        let version = opam_check_identity(text, "demo", "2.0.0").expect_err("version");
        assert!(matches!(version, OpamError::VersionMismatch { .. }));
        assert_eq!(
            version.to_string(),
            "version mismatch: opam file declares 1.0.0, BUILD declares 2.0.0"
        );
    }

    #[test]
    fn identity_fields_are_optional() {
        let text = "opam-version: \"2.0\"\nsynopsis: \"x\"\n";
        assert_eq!(opam_read_identity(text), (None, None));
        assert!(opam_check_identity(text, "demo", "1.0.0").is_ok());
    }

    fn status(status: u16, body: &str) -> OpamError {
        OpamError::Status {
            method: "POST".to_owned(),
            path: "/repos/a/b/git/refs".to_owned(),
            status,
            body: body.to_owned(),
        }
    }

    #[test]
    fn status_errors_render_and_detect_existing_objects() {
        let exists = status(422, r#"{"message":"Reference already exists"}"#);
        assert_eq!(
            exists.to_string(),
            r#"POST /repos/a/b/git/refs: HTTP 422: {"message":"Reference already exists"}"#
        );
        assert!(exists.already_exists());
        assert!(
            status(
                422,
                r#"{"message":"A pull request already exists for a:b."}"#
            )
            .already_exists()
        );
        assert!(!status(422, r#"{"message":"Validation Failed"}"#).already_exists());
        assert!(!status(409, "already exists").already_exists());
        assert!(
            !OpamError::NoField {
                pointer: "/sha".to_owned()
            }
            .already_exists()
        );
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
