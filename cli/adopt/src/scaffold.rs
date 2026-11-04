use std::path::Path;

use super::{AdoptError, DX_VERSION};

/// How a scaffold file is treated when it already exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    WriteOnce,
    VscodeKeys,
    CiPin,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaffoldFile {
    pub path: String,
    pub content: String,
    pub kind: FileKind,
}

pub const DEVCONTAINER_JSON: &str = concat!(
    "{\n",
    "  \"image\": \"mcr.microsoft.com/devcontainers/base:ubuntu@sha256:b1e3b08610ae0b2599f57c3a492eebc93d1d26dbaefb4a0bad0caa7149e4ec95\",\n",
    "  \"features\": {\n",
    "    \"ghcr.io/devcontainers-community/features/bazel:1.2.2\": {}\n",
    "  },\n",
    "  \"mounts\": [\n",
    "    \"source=dx-bazel-cache,target=/home/vscode/.cache/bazel,type=volume\"\n",
    "  ],\n",
    "  \"onCreateCommand\": \"sudo chown -R vscode:vscode /home/vscode/.cache\",\n",
    "  \"postCreateCommand\": \"tools/dx env --apply\"\n",
    "}\n",
);

pub const ENVRC_CONTENT: &str = concat!(
    "# Loads the managed bin through the pinned dx.\n",
    "dx_env_lines=$(tools/dx env --shell=direnv) || return 1\n",
    "eval \"$dx_env_lines\"\n",
);

/// Bazel output base that rust-analyzer discovery runs on.
pub const IDE_OUTPUT_BASE: &str = ".dx/ide/rust";

/// Bazel output base that rust-analyzer flycheck builds in.
pub const IDE_CHECK_OUTPUT_BASE: &str = ".dx/ide/rust_check";

/// Public rules_rust target that prints the rust-analyzer project.
pub const RUST_DISCOVER_LABEL: &str =
    "@rules_rust//tools/rust_analyzer:discover_bazel_rust_project";

/// Public rules_rust target that runs the rust-analyzer check.
pub const RUST_FLYCHECK_LABEL: &str = "@rules_rust//tools/rust_analyzer:flycheck";

/// Content `dx init` writes to a new `.bazelignore`.
pub const BAZELIGNORE_CONTENT: &str = ".dx\n";

pub fn render_consumer_module_setup() -> String {
    format!("bazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\n")
}

/// Adds the include of the dx-written overrides right after the rules_dx `bazel_dep`.
fn with_overrides_include(setup: &str) -> Result<String, AdoptError> {
    let module = parse_init_snippet(setup)?;
    let included = module
        .insert_include(crate::overrides::OVERRIDES_INCLUDE, "rules_dx")
        .map_err(|error| AdoptError::InitSnippet {
            detail: error.to_string(),
        })?;
    Ok(included)
}

fn parse_init_snippet(setup: &str) -> Result<dx_module_bazel::ModuleFile, AdoptError> {
    dx_module_bazel::ModuleFile::parse(setup).map_err(|error| AdoptError::InitSnippet {
        detail: error.to_string(),
    })
}

pub fn validate_init_module(module_name: &str) -> Result<String, AdoptError> {
    let module = if module_name.is_empty() {
        "my_project"
    } else {
        module_name
    };
    let mut chars = module.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let rest_ok = chars
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_');
    if !first_ok || !rest_ok {
        return Err(AdoptError::InitInvalidModule {
            module: module_name.to_owned(),
            reason: "module names start with [a-z0-9] and use [a-z0-9._-] only".to_owned(),
        });
    }
    Ok(module.to_owned())
}

pub const CONSUMER_WORKFLOW_OWNER: &str = "devhusdk";

pub const CONSUMER_WORKFLOW_REPO: &str = "rules_dx";

pub const CONSUMER_WORKFLOW_PATH: &str = ".github/workflows/reusable-consumer.yml";

pub const CONSUMER_WORKFLOW_PIN: &str = "04bb90c9221b324fe86a16bee0a4e5dadb3adaf7";

pub const DEFAULT_BRANCH_FALLBACK: &str = "main";

/// The repo's default branch: `origin/HEAD`, then `init.defaultBranch`, then `main`.
pub fn detect_default_branch(root: &Path) -> String {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let from_origin = git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .and_then(|name| name.strip_prefix("origin/").map(str::to_owned));
    pick_default_branch(from_origin, git(&["config", "init.defaultBranch"]))
}

fn pick_default_branch(origin: Option<String>, configured: Option<String>) -> String {
    [origin, configured]
        .into_iter()
        .flatten()
        .find(|name| is_plain_branch(name))
        .unwrap_or_else(|| DEFAULT_BRANCH_FALLBACK.to_owned())
}

fn is_plain_branch(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'))
}

pub fn render_consumer_caller() -> String {
    render_consumer_caller_for(DEFAULT_BRANCH_FALLBACK)
}

pub fn render_consumer_caller_for(branch: &str) -> String {
    format!(
        "name: ci\non:\n  pull_request: {{}}\n  merge_group:\n    types: [checks_requested]\n  push:\n    branches: [{branch}]\n  workflow_dispatch: {{}}\njobs:\n  dx:\n    uses: {owner}/{repo}/{path}@{pin}\n    with:\n      rules_dx_version: \"{version}\"\n    secrets:\n      BUILDBUDDY_API_KEY: ${{{{ secrets.BUILDBUDDY_API_KEY }}}}\n    permissions:\n      contents: read\n      pull-requests: write\n",
        owner = CONSUMER_WORKFLOW_OWNER,
        repo = CONSUMER_WORKFLOW_REPO,
        path = CONSUMER_WORKFLOW_PATH,
        pin = CONSUMER_WORKFLOW_PIN,
        version = DX_VERSION,
    )
}

fn editor_settings_value() -> serde_json::Value {
    let base = format!("--output_base=${{workspaceFolder}}/{IDE_OUTPUT_BASE}");
    let excludes = serde_json::json!({"**/bazel-*/**": true, "**/.dx/ide/**": true});
    let settings = serde_json::json!({
        "rust-analyzer.workspace.discoverConfig": {
            "command": [
                "bazel",
                base,
                "run",
                RUST_DISCOVER_LABEL,
                "--",
                format!("--bazel_startup_option={base}"),
                "{arg}",
            ],
            "progressLabel": "rules_rust",
            "filesToWatch": ["BUILD", "BUILD.bazel", "MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel"],
        },
        "rust-analyzer.check.overrideCommand": [
            "bazel",
            base,
            "run",
            RUST_FLYCHECK_LABEL,
            "--",
            format!("--output-base=${{workspaceFolder}}/{IDE_CHECK_OUTPUT_BASE}"),
            "--saved-file",
            "$saved_file",
        ],
        "files.watcherExclude": excludes,
        "search.exclude": excludes,
        "python.defaultInterpreterPath": ".dx/ide/python/python",
        "typescript.tsdk": ".dx/ide/typescript/lib",
        "go.toolsManagement.checkForUpdates": "off",
        "clangd.path": ".dx/ide/cc/clangd",
        "clangd.arguments": ["--compile-commands-dir=.dx/ide/cc"],
        "java.configuration.updateBuildConfiguration": "manual",
        "java.jdt.ls.java.home": ".dx/ide/jdk/home",
        "kotlin.java.home": ".dx/ide/jdk/home",
        "kotlin.languageServer.enabled": true,
    });
    settings
}

pub fn render_editor_settings() -> String {
    format!("{}\n", editor_settings_value())
}

const NESTED_SETTINGS: [&str; 2] = ["files.watcherExclude", "search.exclude"];

/// The existing text with only the dx-owned values brought up to date.
fn merge_owned(kind: FileKind, existing: &str) -> Result<String, dx_merge::Error> {
    match kind {
        FileKind::WriteOnce => Ok(existing.to_owned()),
        FileKind::VscodeKeys => {
            let value = editor_settings_value();
            let mut edits: Vec<(Vec<&str>, serde_json::Value)> = Vec::new();
            for (key, setting) in value.as_object().into_iter().flatten() {
                match setting.as_object() {
                    Some(nested) if NESTED_SETTINGS.contains(&key.as_str()) => {
                        for (inner, leaf) in nested {
                            edits.push((vec![key.as_str(), inner.as_str()], leaf.clone()));
                        }
                    }
                    _ => edits.push((vec![key.as_str()], setting.clone())),
                }
            }
            let edits: Vec<(&[&str], serde_json::Value)> = edits
                .iter()
                .map(|(path, leaf)| (path.as_slice(), leaf.clone()))
                .collect();
            dx_merge::jsonc::set_keys(existing, &edits)
        }
        FileKind::CiPin => {
            let action = format!(
                "{CONSUMER_WORKFLOW_OWNER}/{CONSUMER_WORKFLOW_REPO}/{CONSUMER_WORKFLOW_PATH}"
            );
            let pinned =
                dx_merge::yaml_scalar::set_uses_ref(existing, &action, CONSUMER_WORKFLOW_PIN);
            dx_merge::yaml_scalar::set_value(&pinned, "rules_dx_version", DX_VERSION)
        }
    }
}

/// Whether a `.bazelignore` already keeps Bazel out of `.dx`.
pub fn bazelignore_covers_dx(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.strip_prefix("./").unwrap_or(line))
        .map(|line| line.strip_suffix('/').unwrap_or(line))
        .any(|line| line == ".dx" || line == ".dx/ide")
}

pub fn plan_init_files(module_name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    plan_init_files_full(module_name, DEFAULT_BRANCH_FALLBACK)
}

/// Plans the init files for `root`, with the repo's default branch in the caller workflow.
pub fn plan_init_files_at(root: &Path, module_name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let branch = if root.is_dir() {
        detect_default_branch(root)
    } else {
        DEFAULT_BRANCH_FALLBACK.to_owned()
    };
    plan_init_files_full(module_name, &branch)
}

fn plan_init_files_full(module_name: &str, branch: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let module = validate_init_module(module_name)?;
    let consumer_setup = render_consumer_module_setup();
    let setup = with_overrides_include(&consumer_setup)?;
    let files = vec![
        ScaffoldFile {
            path: ".dx/version".to_owned(),
            content: format!("{DX_VERSION}\n"),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".devcontainer/devcontainer.json".to_owned(),
            content: DEVCONTAINER_JSON.to_owned(),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".gitignore".to_owned(),
            content: format!("{}\n", crate::build_output::IGNORE_LINES.join("\n")),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".bazelignore".to_owned(),
            content: BAZELIGNORE_CONTENT.to_owned(),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".envrc".to_owned(),
            content: ENVRC_CONTENT.to_owned(),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".vscode/settings.json".to_owned(),
            content: render_editor_settings(),
            kind: FileKind::VscodeKeys,
        },
        ScaffoldFile {
            path: ".vscode/extensions.json".to_owned(),
            content: "{\"recommendations\":[\"rust-lang.rust-analyzer\",\"ms-python.python\",\"bradlc.vscode-tailwindcss\",\"golang.go\",\"llvm-vs-code-extensions.vscode-clangd\",\"redhat.java\",\"fwcd.kotlin\",\"scalameta.metals\",\"ms-dotnettools.csharp\",\"ionide.ionide-fsharp\"]}\n"
                .to_owned(),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".github/workflows/ci.yml".to_owned(),
            content: render_consumer_caller_for(branch),
            kind: FileKind::CiPin,
        },
        ScaffoldFile {
            path: "MODULE.bazel.snippet".to_owned(),
            content: format!(
                "# Add to MODULE.bazel: hermetic C/C++ and Rust defaults.\n# Own toolchains: keep the bazel_dep lines, drop the register_toolchains lines.\n# module: {module}\n{setup}",
            ),
            kind: FileKind::WriteOnce,
        },
    ];
    Ok(files)
}

pub fn scaffold_dest_within_root(
    root: &Path,
    relative: &str,
) -> Result<std::path::PathBuf, AdoptError> {
    use std::path::Component;
    let mut dest = root.to_path_buf();
    let mut pushed = false;
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => {
                dest.push(part);
                pushed = true;
            }
            _ => {
                return Err(AdoptError::ScaffoldEscapesRoot {
                    path: relative.to_owned(),
                });
            }
        }
    }
    if !pushed {
        return Err(AdoptError::ScaffoldEscapesRoot {
            path: relative.to_owned(),
        });
    }
    ensure_physically_within(root, relative)?;
    Ok(dest)
}

fn ensure_physically_within(root: &Path, relative: &str) -> Result<(), AdoptError> {
    match dx_path::resolve_target_in_workspace(root, relative) {
        Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => {
            Err(AdoptError::ScaffoldEscapesRoot {
                path: relative.to_owned(),
            })
        }
        // A missing root, or a file where a directory belongs, is reported by the write itself.
        _ => Ok(()),
    }
}

/// Fails when a destination escapes the root or has a parent that is not a directory.
pub fn preflight_destinations(root: &Path, relatives: &[&str]) -> Result<(), AdoptError> {
    for relative in relatives {
        scaffold_dest_within_root(root, relative)?;
        let parts: Vec<_> = Path::new(relative).components().collect();
        let mut current = root.to_path_buf();
        let mut shown = std::path::PathBuf::new();
        for part in &parts[..parts.len().saturating_sub(1)] {
            current.push(part);
            shown.push(part);
            match std::fs::metadata(&current) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => {
                    return Err(AdoptError::NotADirectory {
                        path: shown.to_string_lossy().replace('\\', "/"),
                    });
                }
                Err(_) => break,
            }
        }
    }
    Ok(())
}

/// Files a run wrote and files it left alone, recorded as each lands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    pub wrote: Vec<String>,
    pub refused: Vec<String>,
}

/// Checks every destination, then writes each absent file and merges dx-owned values into existing ones.
pub fn apply_scaffold(
    root: &Path,
    files: &[ScaffoldFile],
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    apply_files(root, files, applied, true)
}

/// Brings dx-owned values in existing editor and CI files up to date; creates nothing.
pub fn refresh_owned_files(root: &Path) -> Result<Vec<String>, AdoptError> {
    let files = plan_init_files_full("refresh", DEFAULT_BRANCH_FALLBACK)?;
    let mut applied = Applied::default();
    apply_files(root, &files, &mut applied, false)?;
    Ok(applied.wrote)
}

/// Owned files whose dx-owned values are out of date, and files that cannot be parsed.
pub fn stale_owned_files(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut stale = Vec::new();
    let mut other = Vec::new();
    let Ok(files) = plan_init_files_full("refresh", DEFAULT_BRANCH_FALLBACK) else {
        return (stale, other);
    };
    for file in files.iter().filter(|f| f.kind != FileKind::WriteOnce) {
        let Ok(existing) = std::fs::read_to_string(root.join(&file.path)) else {
            continue;
        };
        match merge_owned(file.kind, &existing) {
            Ok(merged) if merged == existing => {}
            Ok(_) => stale.push(file.path.clone()),
            Err(error) => other.push(format!("{}: {error}", file.path)),
        }
    }
    (stale, other)
}

fn apply_files(
    root: &Path,
    files: &[ScaffoldFile],
    applied: &mut Applied,
    create_missing: bool,
) -> Result<(), AdoptError> {
    let relatives: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    preflight_destinations(root, &relatives)?;
    let mut staged = Vec::with_capacity(files.len());
    for file in files {
        staged.push(scaffold_dest_within_root(root, &file.path)?);
    }
    for (file, dest) in files.iter().zip(staged.iter()) {
        if dest.exists() {
            if file.kind == FileKind::WriteOnce {
                if create_missing {
                    applied.refused.push(file.path.clone());
                }
                continue;
            }
            merge_existing(file, dest, applied)?;
            continue;
        }
        if !create_missing {
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
                parent: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        write_file(dest, &file.content)?;
        applied.wrote.push(file.path.clone());
    }
    Ok(())
}

fn merge_existing(
    file: &ScaffoldFile,
    dest: &Path,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    let existing = std::fs::read_to_string(dest).map_err(|e| AdoptError::ReadFile {
        path: dest.display().to_string(),
        detail: e.to_string(),
    })?;
    match merge_owned(file.kind, &existing) {
        Ok(merged) if merged == existing => {}
        Ok(merged) => {
            write_file(dest, &merged)?;
            applied.wrote.push(file.path.clone());
        }
        Err(_) => applied.refused.push(file.path.clone()),
    }
    Ok(())
}

fn write_file(dest: &Path, content: &str) -> Result<(), AdoptError> {
    dx_atomic_fs::write_atomic(dest, content.as_ref()).map_err(|e| AdoptError::WriteFile {
        path: dest.display().to_string(),
        detail: e.to_string(),
    })
}

pub fn apply_init(root: &Path, module_name: &str, applied: &mut Applied) -> Result<(), AdoptError> {
    apply_scaffold(root, &plan_init_files_at(root, module_name)?, applied)
}

/// Test view of a run: written paths, then `refused:` paths.
#[cfg(test)]
pub(crate) fn entries(
    run: impl FnOnce(&mut Applied) -> Result<(), AdoptError>,
) -> Result<Vec<String>, AdoptError> {
    let mut applied = Applied::default();
    run(&mut applied)?;
    let mut entries = applied.wrote;
    entries.extend(
        applied
            .refused
            .into_iter()
            .map(|path| format!("refused:{path}")),
    );
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_init, entries, plan_init_files, render_consumer_caller, render_consumer_caller_for,
        render_editor_settings, CONSUMER_WORKFLOW_OWNER, CONSUMER_WORKFLOW_PATH,
        CONSUMER_WORKFLOW_PIN, CONSUMER_WORKFLOW_REPO, DEVCONTAINER_JSON, DX_VERSION,
    };
    use crate::version::consumer_setup::parse_consumer_setup;

    #[test]
    fn init_plans_every_file() {
        let paths: Vec<String> = plan_init_files("demo")
            .expect("init plans")
            .into_iter()
            .map(|f| f.path)
            .collect();
        let mut want = [
            ".dx/version",
            ".devcontainer/devcontainer.json",
            ".gitignore",
            ".bazelignore",
            ".envrc",
            ".vscode/settings.json",
            ".vscode/extensions.json",
            ".github/workflows/ci.yml",
            "MODULE.bazel.snippet",
        ];
        want.sort_unstable();
        let mut got: Vec<&str> = paths.iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn envrc_scaffold_delegates_to_the_pinned_tools_dx() {
        let files = plan_init_files("demo").expect("init plans");
        let envrc = files
            .iter()
            .find(|f| f.path == ".envrc")
            .expect("envrc scaffold");
        assert_eq!(envrc.content, super::ENVRC_CONTENT);
        assert!(envrc.content.contains("tools/dx env --shell=direnv"));
        assert!(!envrc.content.contains(".dx"));
    }

    #[cfg(unix)]
    fn run_envrc(stub_body: &str) -> (std::process::ExitStatus, String) {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-adopt-envrc-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join("tools")).expect("tools");
        std::fs::write(root.join(".envrc"), super::ENVRC_CONTENT).expect("envrc");
        let stub = root.join("tools/dx");
        std::fs::write(&stub, format!("#!/bin/sh\n{stub_body}\n")).expect("stub");
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let script = "PATH_add() { PATH=\"$PWD/$1:$PATH\"; export PATH; }\nwatch_file() { :; }\n. ./.envrc\ns=$?\nprintf %s \"$PATH\"\nexit $s";
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .current_dir(root)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .expect("sh runs");
        scratch.close().expect("cleanup");
        (
            output.status,
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    }

    #[cfg(unix)]
    #[test]
    fn envrc_adds_the_managed_bin_to_path() {
        let (status, path) = run_envrc("echo 'PATH_add .dx/bin'\necho 'watch_file .dx/bin'");
        assert!(status.success(), "{status}");
        assert!(path.starts_with('/'), "{path}");
        assert!(path.ends_with("/.dx/bin:/usr/bin:/bin"), "{path}");
    }

    #[cfg(unix)]
    #[test]
    fn envrc_fails_when_the_pinned_dx_fails() {
        let (status, path) = run_envrc("echo 'dx env: missing .dx/bin' >&2\nexit 1");
        assert!(!status.success());
        assert_eq!(path, "/usr/bin:/bin");
    }

    #[test]
    fn devcontainer_scaffold_is_pinned_and_thin() {
        let files = plan_init_files("demo").expect("init plans");
        let scaffold = files
            .iter()
            .find(|f| f.path == ".devcontainer/devcontainer.json")
            .expect("devcontainer scaffold");
        assert_eq!(scaffold.content, DEVCONTAINER_JSON);
        let parsed: serde_json::Value =
            serde_json::from_str(&scaffold.content).expect("valid JSON");
        let image = parsed["image"].as_str().expect("image string");
        let (_, digest) = image
            .split_once("@sha256:")
            .expect("image pinned by digest");
        assert_eq!(digest.len(), 64, "{image}");
        for (feature, _) in parsed["features"].as_object().expect("features") {
            let (_, version) = feature.rsplit_once(':').expect("feature version");
            assert_eq!(version.split('.').count(), 3, "exact version: {feature}");
        }
        assert!(parsed.get("name").is_none());
        assert!(parsed["customizations"].is_null());
        assert!(parsed["mounts"][0]
            .as_str()
            .expect("cache mount")
            .contains("type=volume"));
        assert_eq!(parsed["postCreateCommand"], "tools/dx env --apply");
    }

    #[test]
    fn init_scaffold_covers_admitted_editors() {
        let files = plan_init_files("demo").expect("init plans");
        let settings = files
            .iter()
            .find(|f| f.path == ".vscode/settings.json")
            .expect("settings scaffold");
        let parsed: serde_json::Value =
            serde_json::from_str(&settings.content).expect("valid JSON");
        assert_eq!(
            parsed["clangd.path"],
            serde_json::Value::from(".dx/ide/cc/clangd")
        );
        assert_eq!(
            parsed["clangd.arguments"],
            serde_json::json!(["--compile-commands-dir=.dx/ide/cc"])
        );
        assert_eq!(
            parsed["java.configuration.updateBuildConfiguration"],
            serde_json::Value::from("manual")
        );
        assert!(settings
            .content
            .contains("rust-analyzer.workspace.discoverConfig"));
        assert!(settings
            .content
            .contains("rust-analyzer.check.overrideCommand"));
        assert!(!settings.content.contains("rust-analyzer.check.command"));
        assert!(settings.content.contains("go.toolsManagement"));
        let extensions = files
            .iter()
            .find(|f| f.path == ".vscode/extensions.json")
            .expect("extensions scaffold");
        for id in [
            "golang.go",
            "llvm-vs-code-extensions.vscode-clangd",
            "redhat.java",
            "fwcd.kotlin",
            "scalameta.metals",
            "ms-dotnettools.csharp",
            "ionide.ionide-fsharp",
        ] {
            assert!(extensions.content.contains(id), "missing {id}");
        }
        let devcontainer = files
            .iter()
            .find(|f| f.path == ".devcontainer/devcontainer.json")
            .expect("devcontainer scaffold");
        assert!(!devcontainer.content.contains("extensions"));
    }

    #[test]
    fn apply_init_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = entries(|a| apply_init(&root, "demo", a)).expect("init");
        assert!(first.iter().any(|p| p == ".dx/version"));
        assert!(root.join(".dx/version").exists());
        std::fs::write(root.join(".dx/version"), "custom\n").expect("custom");
        let second = entries(|a| apply_init(&root, "demo", a)).expect("init again");
        assert!(second.iter().any(|p| p == "refused:.dx/version"));
        assert_eq!(
            std::fs::read_to_string(root.join(".dx/version")).expect("read"),
            "custom\n"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn init_module_names_are_lowercase_module_shaped() {
        assert_eq!(
            super::validate_init_module("").expect("default"),
            "my_project"
        );
        for valid in ["demo", "my_project", "a.b-c_d", "x"] {
            assert_eq!(
                super::validate_init_module(valid).expect("valid"),
                valid,
                "{valid}"
            );
        }
        for hostile in [
            "MyApp",
            "UPPER",
            "-lead",
            "has space",
            "with/slash",
            "with\nnewline",
            "with\"quote",
            "semi;colon",
        ] {
            assert!(
                super::validate_init_module(hostile).is_err(),
                "{hostile:?} must not be a module"
            );
            assert!(
                plan_init_files(hostile).is_err(),
                "{hostile:?} must not plan"
            );
        }
        assert_eq!(
            super::validate_init_module("9lives").expect("leading digit"),
            "9lives"
        );
        assert_eq!(
            super::validate_init_module("Bad Name").unwrap_err().to_string(),
            "invalid module name for dx init: \"Bad Name\": module names start with [a-z0-9] and use [a-z0-9._-] only"
        );
    }

    #[test]
    fn init_module_names_never_become_the_workflow_owner() {
        let uses = format!(
            "uses: {CONSUMER_WORKFLOW_OWNER}/{CONSUMER_WORKFLOW_REPO}/{CONSUMER_WORKFLOW_PATH}@{CONSUMER_WORKFLOW_PIN}"
        );
        for module in ["demo", "my_project", "a.b-c_d"] {
            let files = plan_init_files(module).expect("init plans");
            let ci = files
                .iter()
                .find(|f| f.path == ".github/workflows/ci.yml")
                .expect("ci scaffold");
            assert_eq!(ci.content, render_consumer_caller(), "{module}");
            let line = ci
                .content
                .lines()
                .find(|line| line.trim_start().starts_with("uses:"))
                .expect("caller has a uses line");
            assert_eq!(line.trim(), uses, "{module}");
            assert!(
                !line.contains(module),
                "module name must not guess the workflow owner: {line}"
            );
        }
        let files = plan_init_files("demo").expect("init plans");
        let snippet = files
            .iter()
            .find(|f| f.path == "MODULE.bazel.snippet")
            .expect("snippet");
        assert!(snippet.content.contains("# module: demo\n"));
    }

    #[test]
    fn generated_caller_matches_the_checked_in_starter() {
        let source = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        let starter = std::fs::read_to_string(
            std::path::Path::new(&source)
                .join(workspace)
                .join("examples/consumer-ci/caller.yml"),
        )
        .expect("examples/consumer-ci/caller.yml ships as test data");
        let generated = render_consumer_caller();
        let without_defaults: String = starter
            .lines()
            .filter(|l| {
                !l.trim_start().starts_with("disabled_checks:")
                    && !l.trim_start().starts_with("platforms:")
            })
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(generated, without_defaults);
    }

    #[test]
    fn default_branch_prefers_origin_then_config_then_main() {
        let pick = super::pick_default_branch;
        assert_eq!(pick(Some("trunk".into()), Some("dev".into())), "trunk");
        assert_eq!(pick(None, Some("dev".into())), "dev");
        assert_eq!(pick(None, None), "main");
        assert_eq!(pick(Some("a b".into()), Some("".into())), "main");
    }

    #[test]
    fn detects_default_branch_from_git_config_and_falls_back() {
        let dir = std::env::temp_dir().join(format!("dx-adopt-branch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        };
        if git(&["init", "-q"]) && git(&["config", "init.defaultBranch", "trunk"]) {
            assert_eq!(super::detect_default_branch(&dir), "trunk");
            assert!(render_consumer_caller_for("trunk").contains("branches: [trunk]"));
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            super::detect_default_branch(std::path::Path::new("/nonexistent-dx-dir")),
            "main"
        );
    }

    #[test]
    fn caller_leaves_reusable_defaults_out() {
        let caller = render_consumer_caller();
        assert!(!caller.contains("platforms:"), "{caller}");
        assert!(!caller.contains("disabled_checks:"), "{caller}");
    }

    #[test]
    fn generated_caller_pin_is_newer_than_the_one_without_affected_on_pr() {
        assert_ne!(
            CONSUMER_WORKFLOW_PIN,
            "e97ab7b550d08e047c35edc5f50c7d242227842f"
        );
    }

    #[test]
    fn generated_caller_carries_the_required_inputs_permissions_and_secrets() {
        let caller = render_consumer_caller();
        let pin_ok = CONSUMER_WORKFLOW_PIN.len() == 40
            && CONSUMER_WORKFLOW_PIN
                .chars()
                .all(|ch| ch.is_ascii_digit() || ('a'..='f').contains(&ch));
        assert!(pin_ok, "caller must pin a full lowercase commit SHA");
        assert!(
            caller.contains(&format!(
                "uses: {CONSUMER_WORKFLOW_OWNER}/{CONSUMER_WORKFLOW_REPO}/{CONSUMER_WORKFLOW_PATH}@{CONSUMER_WORKFLOW_PIN}"
            )),
            "{caller}"
        );
        assert!(
            caller.contains(&format!("rules_dx_version: \"{DX_VERSION}\"")),
            "{caller}"
        );
        for required in [
            "merge_group:\n    types: [checks_requested]",
            "secrets:\n      BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}",
            "permissions:",
            "contents: read",
            "pull-requests: write",
        ] {
            assert!(
                caller.contains(required),
                "caller lacks {required}: {caller}"
            );
        }
        assert!(
            !caller.contains("  checks: "),
            "caller grants checks: {caller}"
        );
        assert!(!caller.contains("<reviewed-commit>"), "{caller}");
        for removed in [
            "scheduling_mode",
            "code_scanning_opt_in",
            "secrets: inherit",
        ] {
            assert!(
                !caller.contains(removed),
                "caller passes {removed}: {caller}"
            );
        }
    }

    #[test]
    fn editor_settings_use_plugin_paths() {
        let settings = render_editor_settings();
        let parsed: serde_json::Value = serde_json::from_str(&settings).expect("valid JSON");
        assert_eq!(
            parsed["python.defaultInterpreterPath"],
            serde_json::Value::from(".dx/ide/python/python")
        );
        assert_eq!(
            parsed["clangd.path"],
            serde_json::Value::from(".dx/ide/cc/clangd")
        );
        assert!(settings.ends_with('\n'), "{settings}");
        for key in [
            "rust-analyzer.workspace.discoverConfig",
            "typescript.tsdk",
            "go.toolsManagement.checkForUpdates",
            "clangd.arguments",
            "java.configuration.updateBuildConfiguration",
            "java.jdt.ls.java.home",
            "kotlin.java.home",
            "kotlin.languageServer.enabled",
        ] {
            assert!(settings.contains(key), "lost {key}");
        }
    }

    #[test]
    fn rust_analyzer_settings_match_rules_rust_setup_shape() {
        let excludes = serde_json::json!({"**/bazel-*/**": true, "**/.dx/ide/**": true});
        let discover_argv = serde_json::json!([
            "bazel",
            "--output_base=${workspaceFolder}/.dx/ide/rust",
            "run",
            "@rules_rust//tools/rust_analyzer:discover_bazel_rust_project",
            "--",
            "--bazel_startup_option=--output_base=${workspaceFolder}/.dx/ide/rust",
            "{arg}"
        ]);
        let check_argv = serde_json::json!([
            "bazel",
            "--output_base=${workspaceFolder}/.dx/ide/rust",
            "run",
            "@rules_rust//tools/rust_analyzer:flycheck",
            "--",
            "--output-base=${workspaceFolder}/.dx/ide/rust_check",
            "--saved-file",
            "$saved_file"
        ]);
        let parsed: serde_json::Value =
            serde_json::from_str(&render_editor_settings()).expect("json");
        let discover = &parsed["rust-analyzer.workspace.discoverConfig"];
        assert_eq!(discover["command"], discover_argv);
        assert_eq!(discover["progressLabel"], "rules_rust");
        assert_eq!(
            discover["filesToWatch"],
            serde_json::json!([
                "BUILD",
                "BUILD.bazel",
                "MODULE.bazel",
                "WORKSPACE",
                "WORKSPACE.bazel"
            ])
        );
        assert_eq!(parsed["rust-analyzer.check.overrideCommand"], check_argv);
        assert_eq!(parsed["files.watcherExclude"], excludes);
        assert_eq!(parsed["search.exclude"], excludes);
        assert!(parsed.get("rust-analyzer.check.command").is_none());
    }

    #[test]
    fn bazelignore_covers_dx_table() {
        for (text, covered) in [
            ("", false),
            (".dx\n", true),
            (".dx/\n", true),
            ("./.dx\n", true),
            ("  .dx  \n", true),
            (".dx/ide\n", true),
            ("node_modules\n.dx/\n", true),
            ("# .dx\n", false),
            (".dx/setups\n", false),
            (".dxx\n", false),
            ("node_modules\n", false),
        ] {
            assert_eq!(super::bazelignore_covers_dx(text), covered, "{text:?}");
        }
        assert!(super::bazelignore_covers_dx(super::BAZELIGNORE_CONTENT));
    }

    #[test]
    fn init_scaffold_editor_settings_come_from_render() {
        let files = plan_init_files("demo").expect("init plans");
        let settings = files
            .iter()
            .find(|file| file.path == ".vscode/settings.json")
            .expect("settings file");
        assert_eq!(settings.content, render_editor_settings());
    }

    #[test]
    fn init_snippet_is_a_complete_consumer_setup() {
        let scratch = dx_test_scratch::scratch("dx-adopt-consumer-setup-");
        let root = scratch.path().to_path_buf();
        entries(|a| apply_init(&root, "demo", a)).expect("init");
        let text =
            std::fs::read_to_string(root.join("MODULE.bazel.snippet")).expect("emitted snippet");
        assert!(text.contains(&format!(
            "bazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\ninclude(\"{}\")\n",
            crate::overrides::OVERRIDES_INCLUDE
        )));
        let setup = parse_consumer_setup(&text);
        assert_eq!(
            setup.deps,
            vec![("rules_dx".to_owned(), DX_VERSION.to_owned())]
        );
        assert!(setup.extensions.is_empty() && setup.registered.is_empty());
        let versions = text.matches(char::is_numeric).count();
        assert_eq!(versions, DX_VERSION.matches(char::is_numeric).count());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn snippet_is_the_same_with_or_without_cuda_sources() {
        let plain = dx_test_scratch::scratch("dx-adopt-init-nocuda-");
        entries(|a| apply_init(plain.path(), "demo", a)).expect("init");
        let with = dx_test_scratch::scratch("dx-adopt-init-cuda-");
        std::fs::write(with.path().join("a.cu"), "").expect("write");
        std::fs::write(with.path().join("a.rs"), "").expect("write");
        entries(|a| apply_init(with.path(), "demo", a)).expect("init");
        let read = |dir: &std::path::Path| {
            std::fs::read_to_string(dir.join("MODULE.bazel.snippet")).expect("read")
        };
        assert_eq!(read(plain.path()), read(with.path()));
        plain.close().expect("cleanup");
        with.close().expect("cleanup");
    }

    #[test]
    fn snippet_edits_follow_parsed_calls_not_text() {
        let setup = "# bazel_dep(
bazel_dep(name = \"rules_dx\", version = \"1\")\n";
        let included = super::with_overrides_include(setup).expect("include");
        assert!(included.contains(&format!(
            "bazel_dep(name = \"rules_dx\", version = \"1\")\ninclude(\"{}\")\n",
            crate::overrides::OVERRIDES_INCLUDE
        )));
    }

    #[test]
    fn apply_init_snippet_is_absent_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-snippet-absent-");
        let root = scratch.path().to_path_buf();
        entries(|a| apply_init(&root, "demo", a)).expect("init");
        let snippet = root.join("MODULE.bazel.snippet");
        std::fs::write(&snippet, "custom\n").expect("custom");
        let second = entries(|a| apply_init(&root, "demo", a)).expect("init again");
        assert!(
            second.iter().any(|p| p == "refused:MODULE.bazel.snippet"),
            "snippet must stay absent-only: {second:?}"
        );
        assert_eq!(std::fs::read_to_string(&snippet).expect("read"), "custom\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn apply_init_writes_nothing_for_invalid_modules() {
        for hostile in ["Bad Name", "../evil", "with\nnewline"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            assert!(
                entries(|a| apply_init(&root, hostile, a)).is_err(),
                "{hostile:?} must fail"
            );
            let mut entries: Vec<String> = Vec::new();
            for entry in std::fs::read_dir(&root).expect("read") {
                entries.push(
                    entry
                        .expect("entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            assert_eq!(entries, vec!["sentinel".to_owned()], "{hostile:?}");
            scratch.close().expect("cleanup");
        }
    }

    #[test]
    fn apply_scaffold_refuses_a_file_parent_before_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-file-parent-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join(".vscode"), "file\n").expect("blocker");
        let mut applied = super::Applied::default();
        let err = apply_init(&root, "demo", &mut applied).expect_err("file parent refused");
        assert_eq!(
            err,
            crate::AdoptError::NotADirectory {
                path: ".vscode".to_owned()
            }
        );
        assert_eq!(err.to_string(), ".vscode is not a directory");
        assert_eq!(applied, super::Applied::default());
        let names: Vec<String> = std::fs::read_dir(&root)
            .expect("read")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![".vscode".to_owned()]);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn init_leaves_existing_files_untouched() {
        let scratch = dx_test_scratch::scratch("dx-adopt-shared-collision-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(root.join(".envrc"), "mine\n").expect("handwritten");
        let init = entries(|a| apply_init(&root, "demo", a)).expect("init");
        assert!(init.contains(&"refused:.envrc".to_owned()), "{init:?}");
        assert!(init.contains(&".dx/version".to_owned()), "{init:?}");
        assert_eq!(
            std::fs::read_to_string(root.join(".envrc")).unwrap(),
            "mine\n"
        );
        scratch.close().expect("cleanup");
    }

    const OLD_SHA: &str = "0000000000000000000000000000000000000000";

    fn old_ci() -> String {
        render_consumer_caller()
            .replace(super::CONSUMER_WORKFLOW_PIN, OLD_SHA)
            .replace(&format!("\"{DX_VERSION}\""), "\"0.0.1\"")
    }

    #[test]
    fn owned_settings_keep_comments_and_user_keys() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-json-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".vscode")).expect("dir");
        let mine = "{\n  // mine\n  \"editor.tabSize\": 2,\n  \"go.toolsManagement.checkForUpdates\": \"on\",\n  \"search.exclude\": {\"**/mine\": true,},\n}\n";
        std::fs::write(root.join(".vscode/settings.json"), mine).expect("write");
        let wrote = super::refresh_owned_files(root).expect("refresh");
        assert_eq!(wrote, [".vscode/settings.json"]);
        let text = std::fs::read_to_string(root.join(".vscode/settings.json")).expect("read");
        assert!(text.contains("// mine"), "{text}");
        assert!(text.contains("\"editor.tabSize\": 2"), "{text}");
        assert!(text.contains("\"**/mine\": true"), "{text}");
        assert!(text.contains("\"go.toolsManagement.checkForUpdates\": \"off\""), "{text}");
        assert!(text.contains("clangd.path"), "{text}");
        assert!(!root.join(".github").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn second_apply_changes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-twice-");
        let root = scratch.path();
        let first = entries(|a| apply_init(root, "demo", a)).expect("first");
        assert!(first.contains(&".vscode/settings.json".to_owned()), "{first:?}");
        let before = std::fs::read(root.join(".vscode/settings.json")).expect("read");
        let ci = std::fs::read(root.join(".github/workflows/ci.yml")).expect("read");
        let second = entries(|a| apply_init(root, "demo", a)).expect("second");
        assert!(second.iter().all(|e| e.starts_with("refused:")), "{second:?}");
        assert!(!second.contains(&"refused:.vscode/settings.json".to_owned()), "{second:?}");
        assert_eq!(std::fs::read(root.join(".vscode/settings.json")).expect("read"), before);
        assert_eq!(std::fs::read(root.join(".github/workflows/ci.yml")).expect("read"), ci);
        assert!(super::refresh_owned_files(root).expect("refresh").is_empty());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn pin_update_changes_exactly_two_lines() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-pin-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let old = old_ci();
        std::fs::write(root.join(".github/workflows/ci.yml"), &old).expect("write");
        let wrote = super::refresh_owned_files(root).expect("refresh");
        assert_eq!(wrote, [".github/workflows/ci.yml"]);
        let new = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read");
        let changed = old.lines().zip(new.lines()).filter(|(a, b)| a != b).count();
        assert_eq!(changed, 2, "{new}");
        assert_eq!(old.lines().count(), new.lines().count());
        assert_eq!(new, render_consumer_caller());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn ci_without_the_version_input_is_refused() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-refused-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let mine = "name: ci\njobs: {}\n";
        std::fs::write(root.join(".github/workflows/ci.yml"), mine).expect("write");
        let ran = entries(|a| apply_init(root, "demo", a)).expect("init");
        assert!(ran.contains(&"refused:.github/workflows/ci.yml".to_owned()), "{ran:?}");
        assert_eq!(
            std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read"),
            mine
        );
        scratch.close().expect("cleanup");
    }

    #[cfg(unix)]
    mod symlinks {
        use crate::scaffold::{apply_init, entries as applied, scaffold_dest_within_root};
        use std::os::unix::fs::symlink;

        fn entries(dir: &std::path::Path) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(dir)
                .expect("read")
                .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        #[test]
        fn init_refuses_external_symlinked_ancestor_and_writes_nothing() {
            let scratch = dx_test_scratch::scratch("dx-adopt-symlink-out-");
            let root = scratch.path().join("root");
            let outside = scratch.path().join("outside");
            std::fs::create_dir_all(&root).expect("root");
            std::fs::create_dir_all(&outside).expect("outside");
            symlink(&outside, root.join(".github")).expect("symlink");
            let err = applied(|a| apply_init(&root, "demo", a)).expect_err("escape refused");
            assert!(
                err.to_string().contains(".github/workflows/ci.yml"),
                "{err}"
            );
            assert!(entries(&outside).is_empty());
            assert_eq!(entries(&root), vec![".github".to_owned()]);
            scratch.close().expect("cleanup");
        }

        #[test]
        fn dangling_symlink_is_refused() {
            let scratch = dx_test_scratch::scratch("dx-adopt-symlink-dangling-");
            let root = scratch.path().join("root");
            std::fs::create_dir_all(&root).expect("root");
            symlink(scratch.path().join("missing"), root.join(".github")).expect("symlink");
            assert!(scaffold_dest_within_root(&root, ".github/workflows/ci.yml").is_err());
            scratch.close().expect("cleanup");
        }

        #[test]
        fn internal_symlink_is_allowed() {
            let scratch = dx_test_scratch::scratch("dx-adopt-symlink-in-");
            let root = scratch.path().join("root");
            std::fs::create_dir_all(root.join("real")).expect("real");
            symlink(root.join("real"), root.join(".github")).expect("symlink");
            let written = applied(|a| apply_init(&root, "demo", a)).expect("internal link allowed");
            assert!(written.contains(&".github/workflows/ci.yml".to_owned()));
            assert!(root.join("real/workflows/ci.yml").is_file());
            scratch.close().expect("cleanup");
        }
    }
}
