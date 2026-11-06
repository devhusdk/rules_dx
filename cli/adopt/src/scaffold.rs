use std::collections::BTreeSet;
use std::path::Path;

use crate::Ownership;

use super::{plan_profile_files, release_pins, AdoptError, Profile, DX_VERSION};

/// How a scaffold file is treated when it already exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    WriteOnce,
    VscodeKeys,
    /// Adds missing ids to `recommendations`, never removing or reordering, skipping unwanted ones.
    VscodeRecommendations,
    /// Repins the `uses:` ref of the reusable workflow at this path.
    CiPin(&'static str),
    /// Refreshes the devcontainer `image` digest and bazel feature version while they still name
    /// what dx wrote; any other image or feature is left alone.
    DevcontainerPins,
    /// Rewritten whenever it differs from the planned content.
    Generated,
    /// A bare `.dx` line, which dx wrote, becomes the planned lines; every other line stays.
    Bazelignore,
    /// Sets the owned keys of `OWNED_EDITORCONFIG_SECTIONS` that the planned text has; other keys
    /// and sections stay.
    EditorconfigKeys,
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
pub const IDE_OUTPUT_BASE: &str = ".dx/local/ide/rust";

/// Bazel output base that rust-analyzer flycheck builds in.
pub const IDE_CHECK_OUTPUT_BASE: &str = ".dx/local/ide/rust_check";

/// The `.dx` subdirectories Bazel must skip, one per line in a new `.bazelignore`.
///
/// `.dx/local` holds the Bazel output bases (`IDE_OUTPUT_BASE`, `IDE_CHECK_OUTPUT_BASE`), the
/// managed setups, environments and generated trees, symlinks into setup artifacts that can ship
/// BUILD files. Everything else
/// under `.dx` holds only dx files, and `.dx/config` is checked-in source Bazel stages.
/// A new `.dx` writer that can hold BUILD files or an output base must be added here.
pub const BAZELIGNORE_LINES: [&str; 2] = [".dx/local", ".dx/bazelrc"];

/// `body` under the one-line header that says what dx does to `path`; `body` alone when the
/// format has no comments.
pub(crate) fn with_header(ownership: Ownership, path: &str, body: &str) -> String {
    match ownership.header(path, None) {
        Some(header) => format!("{header}\n{body}"),
        None => body.to_owned(),
    }
}

/// The `.bazelignore` lines `dx init` writes, without the header.
pub fn bazelignore_content() -> String {
    format!("{}\n", BAZELIGNORE_LINES.join("\n"))
}

fn is_bare_dx_line(line: &str) -> bool {
    matches!(
        line.trim(),
        dx_path::state::DX_DIR | ".dx/" | "./.dx" | "/.dx"
    )
}

/// Whether a `.bazelignore` still has a bare `.dx` line that init replaces with the list.
pub fn bazelignore_has_bare_dx(text: &str) -> bool {
    text.lines().any(is_bare_dx_line)
}

fn merge_bazelignore(existing: &str, generated: &str) -> String {
    let generated = generated
        .strip_prefix(&with_header(Ownership::Lines, ".bazelignore", ""))
        .unwrap_or(generated);
    let mut out = String::new();
    let mut replaced = false;
    for line in existing.split_inclusive('\n') {
        if !is_bare_dx_line(line) {
            out.push_str(line);
        } else if !replaced {
            replaced = true;
            out.push_str(generated);
        }
    }
    out
}

/// Content `dx init` writes to a new `.editorconfig`.
const EDITORCONFIG_CONTENT: &str = concat!(
    "root = true\n",
    "\n",
    "[*]\n",
    "end_of_line = lf\n",
    "insert_final_newline = true\n",
    "charset = utf-8\n",
    "indent_style = space\n",
    "\n",
    "[*.{rs,py,java,kt,kts,cs}]\n",
    "indent_size = 4\n",
    "\n",
    "[{BUILD,*.bzl,*.bazel,*.star,*.yaml,*.yml,*.json,*.ts,*.tsx}]\n",
    "indent_size = 2\n",
    "\n",
    "[*.go]\n",
    "indent_style = tab\n",
    "\n",
    "[{Makefile,makefile,GNUmakefile,*.mk}]\n",
    "indent_style = tab\n",
);

struct OwnedEditorconfigSection {
    extensions: &'static [&'static str],
    keys: &'static str,
}

/// Sections of `.editorconfig` whose keys dx sets while the tree has files of these extensions.
const OWNED_EDITORCONFIG_SECTIONS: [OwnedEditorconfigSection; 1] = [OwnedEditorconfigSection {
    extensions: &[".kt", ".kts"],
    keys: include_str!("../../../quality/tiers/ktlint/standard.editorconfig"),
}];

/// The section header and `key = value` pairs of an owned section's text.
fn owned_section_keys(text: &str) -> Option<(&str, Vec<(&str, &str)>)> {
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let header = lines.next()?.strip_prefix('[')?.strip_suffix(']')?;
    let keys = lines
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect();
    Some((header, keys))
}

fn editorconfig_content(extensions: &BTreeSet<String>) -> String {
    let mut text = EDITORCONFIG_CONTENT.to_owned();
    for owned in &OWNED_EDITORCONFIG_SECTIONS {
        if !owned.extensions.iter().any(|ext| extensions.contains(*ext)) {
            continue;
        }
        if let Some((section, keys)) = owned_section_keys(owned.keys) {
            text = dx_merge::editorconfig::set_keys(&text, section, &keys);
        }
    }
    text
}

fn merge_editorconfig_keys(existing: &str, generated: &str) -> String {
    let mut text = existing.to_owned();
    for owned in &OWNED_EDITORCONFIG_SECTIONS {
        let Some((section, keys)) = owned_section_keys(owned.keys) else {
            continue;
        };
        let planned = generated
            .lines()
            .any(|line| line.trim() == format!("[{section}]"));
        if planned {
            text = dx_merge::editorconfig::set_keys(&text, section, &keys);
        }
    }
    text
}

/// Content `dx init` writes to a new `.gitattributes`.
const GITATTRIBUTES_CONTENT: &str = concat!(
    "* text=auto eol=lf\n",
    "*.bat text eol=crlf\n",
    "*.cmd text eol=crlf\n",
    "*.ps1 text eol=crlf\n",
);

/// VS Code extension that every workspace gets.
const BAZEL_EDITOR_EXTENSION: &str = "bazelbuild.vscode-bazel";

/// Language editor extensions, each with the file extensions that bring it in.
const LANGUAGE_EDITOR_EXTENSIONS: [(&str, &[&str]); 10] = [
    ("rust-lang.rust-analyzer", &[".rs"]),
    ("ms-python.python", &[".py", ".pyi"]),
    (
        "bradlc.vscode-tailwindcss",
        &[".js", ".jsx", ".ts", ".tsx", ".vue", ".svelte", ".astro"],
    ),
    ("golang.go", &[".go"]),
    (
        "llvm-vs-code-extensions.vscode-clangd",
        &[".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp"],
    ),
    ("redhat.java", &[".java"]),
    ("fwcd.kotlin", &[".kt", ".kts"]),
    ("scalameta.metals", &[".scala"]),
    ("ms-dotnettools.csharp", &[".cs"]),
    ("ionide.ionide-fsharp", &[".fs", ".fsi", ".fsx"]),
];

/// The `.vscode/extensions.json` text: Bazel, then each language whose files are in `extensions`.
fn render_editor_recommendations(extensions: &BTreeSet<String>) -> String {
    let mut ids = vec![BAZEL_EDITOR_EXTENSION];
    ids.extend(
        LANGUAGE_EDITOR_EXTENSIONS
            .iter()
            .filter(|(_, own)| own.iter().any(|ext| extensions.contains(*ext)))
            .map(|(id, _)| *id),
    );
    format!("{}\n", serde_json::json!({ "recommendations": ids }))
}

const DX_UPDATE_WORKFLOW: &str = include_str!("../../../.github/workflows/dx-update.yml");

pub fn render_consumer_module_setup() -> String {
    format!("bazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\n")
}

/// Adds the include of the dx-written overrides right after the rules_dx `bazel_dep`.
pub fn with_overrides_include(setup: &str) -> Result<String, AdoptError> {
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

pub const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/reusable-release.yml";

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
        .and_then(|name| name.strip_prefix("origin/").map(str::to_owned))
        .filter(|name| {
            let remote_ref = format!("refs/remotes/origin/{name}");
            git(&["rev-parse", "--verify", "--quiet", &remote_ref]).is_some()
        });
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
    let body = format!(
        "name: ci\non:\n  pull_request: {{}}\n  merge_group:\n    types: [checks_requested]\n  push:\n    branches: [{branch}]\n  workflow_dispatch: {{}}\njobs:\n  dx:\n    uses: {owner}/{repo}/{path}@{pin}\n    secrets:\n      BUILDBUDDY_API_KEY: ${{{{ secrets.BUILDBUDDY_API_KEY }}}}\n      REMOTE_CACHE_HEADER: ${{{{ secrets.REMOTE_CACHE_HEADER }}}}\n    permissions:\n      contents: read\n      pull-requests: write\n",
        owner = CONSUMER_WORKFLOW_OWNER,
        repo = CONSUMER_WORKFLOW_REPO,
        path = CONSUMER_WORKFLOW_PATH,
        pin = CONSUMER_WORKFLOW_PIN,
    );
    with_header(Ownership::Pin, ".github/workflows/ci.yml", &body)
}

pub fn render_release_caller() -> String {
    let body = format!(
        "name: release\non:\n  push:\n    tags: ['v*']\njobs:\n  release:\n    uses: {owner}/{repo}/{path}@{pin}\n    secrets: inherit\n    permissions:\n      contents: write\n      id-token: write\n      attestations: write\n      packages: write\n",
        owner = CONSUMER_WORKFLOW_OWNER,
        repo = CONSUMER_WORKFLOW_REPO,
        path = RELEASE_WORKFLOW_PATH,
        pin = CONSUMER_WORKFLOW_PIN,
    );
    with_header(Ownership::Pin, ".github/workflows/release.yml", &body)
}

fn editor_settings_value() -> serde_json::Value {
    let base = format!("--output_base=${{workspaceFolder}}/{IDE_OUTPUT_BASE}");
    let ide = format!("${{workspaceFolder}}/{}", dx_ide::IDE_DIR);
    let excludes = serde_json::json!({"**/bazel-*/**": true, "**/.dx/local/ide/**": true});
    let settings = serde_json::json!({
        "rust-analyzer.workspace.discoverConfig": {
            "command": [
                format!("{ide}/rust_analyzer/discover_bazel_rust_project.exe"),
                format!("--bazel_startup_option={base}"),
                "{arg}",
            ],
            "progressLabel": "rules_rust",
            "filesToWatch": ["BUILD", "BUILD.bazel", "MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel"],
        },
        "rust-analyzer.check.overrideCommand": [
            format!("{ide}/rust_analyzer/flycheck.exe"),
            format!("--output-base=${{workspaceFolder}}/{IDE_CHECK_OUTPUT_BASE}"),
            "--saved-file",
            "$saved_file",
        ],
        "files.watcherExclude": excludes,
        "search.exclude": excludes,
        "python.defaultInterpreterPath": format!("{ide}/python/python"),
        "typescript.tsdk": format!("{ide}/typescript/lib"),
        "go.toolsManagement.checkForUpdates": "off",
        "clangd.path": format!("{ide}/cc/clangd"),
        "clangd.arguments": [format!("--compile-commands-dir={ide}/cc")],
        "java.configuration.updateBuildConfiguration": "manual",
        "java.jdt.ls.java.home": format!("{ide}/jdk/home"),
        "kotlin.java.home": format!("{ide}/jdk/home"),
        "kotlin.languageServer.enabled": true,
    });
    settings
}

pub fn render_editor_settings() -> String {
    format!("{}\n", editor_settings_value())
}

const NESTED_SETTINGS: [&str; 2] = ["files.watcherExclude", "search.exclude"];

/// The existing text with only the dx-owned values brought up to date.
fn merge_owned(kind: FileKind, existing: &str, generated: &str) -> Result<String, dx_merge::Error> {
    match kind {
        FileKind::WriteOnce => Ok(existing.to_owned()),
        FileKind::Generated => Ok(generated.to_owned()),
        FileKind::Bazelignore => Ok(merge_bazelignore(existing, generated)),
        FileKind::EditorconfigKeys => Ok(merge_editorconfig_keys(existing, generated)),
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
        FileKind::VscodeRecommendations => {
            let unwanted = dx_merge::jsonc::parse(existing)?
                .get("unwantedRecommendations")
                .and_then(|value| value.as_array())
                .cloned()
                .unwrap_or_default();
            let wanted = dx_merge::jsonc::parse(generated)?;
            let mut text = existing.to_owned();
            for id in wanted["recommendations"].as_array().into_iter().flatten() {
                let skipped = unwanted.iter().any(|other| {
                    other.as_str().map(str::to_lowercase) == id.as_str().map(str::to_lowercase)
                });
                if !skipped {
                    text =
                        dx_merge::jsonc::append_missing(&text, &["recommendations"], id.clone())?;
                }
            }
            Ok(text)
        }
        FileKind::DevcontainerPins => {
            let wanted = dx_merge::jsonc::parse(generated)?;
            let current = dx_merge::jsonc::parse(existing)?;
            let mut text = existing.to_owned();
            let image = wanted["image"].as_str().unwrap_or_default();
            if current["image"]
                .as_str()
                .is_some_and(|old| old != image && image_repository(old) == image_repository(image))
            {
                text = dx_merge::jsonc::set_keys(&text, &[(&["image"], image.into())])?;
            }
            for (feature, _) in wanted["features"].as_object().into_iter().flatten() {
                let base = feature_base(feature);
                text = dx_merge::jsonc::rename_key(
                    &text,
                    &["features"],
                    |name| feature_base(name) == base,
                    feature,
                )?;
            }
            Ok(text)
        }
        FileKind::CiPin(workflow) => {
            let action = format!("{CONSUMER_WORKFLOW_OWNER}/{CONSUMER_WORKFLOW_REPO}/{workflow}");
            let pinned =
                dx_merge::yaml_scalar::set_uses_ref(existing, &action, CONSUMER_WORKFLOW_PIN);
            Ok(pinned
                .split_inclusive('\n')
                .filter(|line| !line.trim_start().starts_with("rules_dx_version:"))
                .collect())
        }
    }
}

fn image_repository(image: &str) -> &str {
    let name = image.split_once('@').map_or(image, |(name, _)| name);
    match name.rsplit_once(':') {
        Some((repository, tag)) if !tag.contains('/') => repository,
        _ => name,
    }
}

fn feature_base(feature: &str) -> &str {
    match feature.rsplit_once(':') {
        Some((base, tag)) if !tag.contains('/') => base,
        _ => feature,
    }
}

/// Whether a `.bazelignore` already keeps Bazel out of `.dx`.
pub fn bazelignore_covers_dx(text: &str) -> bool {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.strip_prefix("./").unwrap_or(line))
        .map(|line| line.strip_suffix('/').unwrap_or(line))
        .any(|line| line == dx_path::state::DX_DIR || line == dx_path::state::LOCAL_DIR)
}

pub fn plan_init_files(module_name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    plan_init_files_full(module_name, DEFAULT_BRANCH_FALLBACK, &BTreeSet::new())
}

/// Plans the init files for `root`, with the repo's default branch in the caller workflow and
/// editor recommendations for the languages in the tree.
pub fn plan_init_files_at(
    root: &Path,
    module_name: &str,
    profile: Option<Profile>,
) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let (branch, extensions) = if root.is_dir() {
        (
            detect_default_branch(root),
            crate::families::file_extensions(root),
        )
    } else {
        (DEFAULT_BRANCH_FALLBACK.to_owned(), BTreeSet::new())
    };
    let mut files = plan_init_files_full(module_name, &branch, &extensions)?;
    if root.is_dir() {
        files.extend(plan_profile_files(root, &extensions, profile));
    }
    Ok(files)
}

fn plan_init_files_full(
    module_name: &str,
    branch: &str,
    extensions: &BTreeSet<String>,
) -> Result<Vec<ScaffoldFile>, AdoptError> {
    validate_init_module(module_name)?;
    let files = vec![
        ScaffoldFile {
            path: ".dx/pins".to_owned(),
            content: release_pins(DX_VERSION)?.to_lines(DX_VERSION),
            kind: FileKind::Generated,
        },
        ScaffoldFile {
            path: ".devcontainer/devcontainer.json".to_owned(),
            content: DEVCONTAINER_JSON.to_owned(),
            kind: FileKind::DevcontainerPins,
        },
        ScaffoldFile {
            path: ".bazelignore".to_owned(),
            content: with_header(Ownership::Lines, ".bazelignore", &bazelignore_content()),
            kind: FileKind::Bazelignore,
        },
        ScaffoldFile {
            path: ".editorconfig".to_owned(),
            content: with_header(
                Ownership::Keys,
                ".editorconfig",
                &editorconfig_content(extensions),
            ),
            kind: FileKind::EditorconfigKeys,
        },
        ScaffoldFile {
            path: ".gitattributes".to_owned(),
            content: with_header(Ownership::Once, ".gitattributes", GITATTRIBUTES_CONTENT),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".envrc".to_owned(),
            content: with_header(Ownership::Once, ".envrc", ENVRC_CONTENT),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".vscode/settings.json".to_owned(),
            content: render_editor_settings(),
            kind: FileKind::VscodeKeys,
        },
        ScaffoldFile {
            path: ".vscode/extensions.json".to_owned(),
            content: render_editor_recommendations(extensions),
            kind: FileKind::VscodeRecommendations,
        },
        ScaffoldFile {
            path: ".github/workflows/ci.yml".to_owned(),
            content: render_consumer_caller_for(branch),
            kind: FileKind::CiPin(CONSUMER_WORKFLOW_PATH),
        },
        ScaffoldFile {
            path: ".github/workflows/dx-update.yml".to_owned(),
            content: with_header(
                Ownership::Once,
                ".github/workflows/dx-update.yml",
                DX_UPDATE_WORKFLOW,
            ),
            kind: FileKind::WriteOnce,
        },
        ScaffoldFile {
            path: ".github/workflows/release.yml".to_owned(),
            content: render_release_caller(),
            kind: FileKind::CiPin(RELEASE_WORKFLOW_PATH),
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
pub fn refresh_scaffold(
    root: &Path,
    files: &[ScaffoldFile],
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    apply_files(root, files, applied, false)
}

/// Whether an existing owned file lacks dx-owned values; `Err` when it cannot be read or parsed.
pub fn owned_file_drift(root: &Path, file: &ScaffoldFile) -> Result<bool, String> {
    if file.kind == FileKind::WriteOnce {
        return Ok(false);
    }
    let Ok(existing) = std::fs::read_to_string(root.join(&file.path)) else {
        return Ok(false);
    };
    match merge_owned(file.kind, &existing, &file.content) {
        Ok(merged) => Ok(merged != existing),
        Err(error) => Err(format!("{}: {error}", file.path)),
    }
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
    match merge_owned(file.kind, &existing, &file.content) {
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
        editor_settings_value, entries, owned_file_drift, plan_init_files, plan_init_files_at,
        refresh_scaffold, render_consumer_caller, render_consumer_caller_for,
        render_editor_settings, Applied, ScaffoldFile, CONSUMER_WORKFLOW_OWNER,
        CONSUMER_WORKFLOW_PATH, CONSUMER_WORKFLOW_PIN, CONSUMER_WORKFLOW_REPO, DEVCONTAINER_JSON,
        DX_VERSION, EDITORCONFIG_CONTENT,
    };

    pub(super) fn apply_init(
        root: &std::path::Path,
        module_name: &str,
        applied: &mut super::Applied,
    ) -> Result<(), super::AdoptError> {
        super::apply_scaffold(
            root,
            &super::plan_init_files_at(root, module_name, None)?,
            applied,
        )
    }

    #[test]
    fn init_plans_every_file() {
        let paths: Vec<String> = plan_init_files("demo")
            .expect("init plans")
            .into_iter()
            .map(|f| f.path)
            .collect();
        let mut want = [
            ".dx/pins",
            ".devcontainer/devcontainer.json",
            ".bazelignore",
            ".editorconfig",
            ".gitattributes",
            ".envrc",
            ".vscode/settings.json",
            ".vscode/extensions.json",
            ".github/workflows/ci.yml",
            ".github/workflows/dx-update.yml",
            ".github/workflows/release.yml",
        ];
        want.sort_unstable();
        let mut got: Vec<&str> = paths.iter().map(String::as_str).collect();
        got.sort_unstable();
        assert_eq!(got, want);
    }

    #[test]
    fn release_workflow_calls_the_pinned_reusable_release() {
        let files = plan_init_files("demo").expect("init plans");
        let file = files
            .iter()
            .find(|f| f.path == ".github/workflows/release.yml")
            .expect("release scaffold");
        let uses = format!(
            "uses: {CONSUMER_WORKFLOW_OWNER}/{CONSUMER_WORKFLOW_REPO}/.github/workflows/reusable-release.yml@{CONSUMER_WORKFLOW_PIN}"
        );
        for want in [
            "tags: ['v*']",
            uses.as_str(),
            "secrets: inherit",
            "id-token: write",
            "attestations: write",
        ] {
            assert!(file.content.contains(want), "missing {want}");
        }
    }

    #[test]
    fn dx_update_workflow_is_generic() {
        let files = plan_init_files("demo").expect("init plans");
        let file = files
            .iter()
            .find(|f| f.path == ".github/workflows/dx-update.yml")
            .expect("dx-update scaffold");
        for want in [
            "cron: \"0 4 * * 1\"",
            "workflow_dispatch",
            "contents: write",
            "pull-requests: write",
            "tools/dx update --pr",
            "DX_UPDATE_APP_ID",
            "DX_UPDATE_APP_PRIVATE_KEY",
        ] {
            assert!(file.content.contains(want), "missing {want}");
        }
        for banned in ["./.github/actions", "BUILDBUDDY"] {
            assert!(!file.content.contains(banned), "found {banned}");
        }
    }

    #[test]
    fn envrc_scaffold_delegates_to_the_pinned_tools_dx() {
        let files = plan_init_files("demo").expect("init plans");
        let envrc = files
            .iter()
            .find(|f| f.path == ".envrc")
            .expect("envrc scaffold");
        assert!(envrc.content.ends_with(super::ENVRC_CONTENT));
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
        let (status, path) =
            run_envrc("echo 'PATH_add .dx/local/bin'\necho 'watch_file .dx/local/bin'");
        assert!(status.success(), "{status}");
        assert!(path.starts_with('/'), "{path}");
        assert!(path.ends_with("/.dx/local/bin:/usr/bin:/bin"), "{path}");
    }

    #[cfg(unix)]
    #[test]
    fn envrc_fails_when_the_pinned_dx_fails() {
        let (status, path) = run_envrc("echo 'dx env: missing .dx/local/bin' >&2\nexit 1");
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
            serde_json::Value::from("${workspaceFolder}/.dx/local/ide/cc/clangd")
        );
        assert_eq!(
            parsed["clangd.arguments"],
            serde_json::json!(["--compile-commands-dir=${workspaceFolder}/.dx/local/ide/cc"])
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
        let devcontainer = files
            .iter()
            .find(|f| f.path == ".devcontainer/devcontainer.json")
            .expect("devcontainer scaffold");
        assert!(!devcontainer.content.contains("extensions"));
    }

    fn recommendations(files: &[(&str, &str)]) -> Vec<String> {
        let scratch = dx_test_scratch::scratch("dx-adopt-editor-recs-");
        let root = scratch.path().join("root");
        for (path, text) in files {
            let dest = root.join(path);
            std::fs::create_dir_all(dest.parent().expect("parent")).expect("dir");
            std::fs::write(dest, text).expect("file");
        }
        let plan = super::plan_init_files_at(&root, "demo", None).expect("init plans");
        let extensions = plan
            .iter()
            .find(|f| f.path == ".vscode/extensions.json")
            .expect("extensions scaffold");
        let parsed: serde_json::Value =
            serde_json::from_str(&extensions.content).expect("valid JSON");
        scratch.close().expect("cleanup");
        parsed["recommendations"]
            .as_array()
            .expect("array")
            .iter()
            .map(|id| id.as_str().expect("string").to_owned())
            .collect()
    }

    #[test]
    fn python_only_repo_recommends_bazel_and_python() {
        assert_eq!(
            recommendations(&[("app/main.py", "print(1)\n")]),
            ["bazelbuild.vscode-bazel", "ms-python.python"]
        );
    }

    #[test]
    fn repo_without_sources_recommends_only_bazel() {
        assert_eq!(
            recommendations(&[("README.md", "hi\n")]),
            ["bazelbuild.vscode-bazel"]
        );
    }

    #[test]
    fn every_language_in_the_tree_gets_its_extension() {
        let files = [
            ("a.rs", ""),
            ("b.go", ""),
            ("c.cpp", ""),
            ("d.java", ""),
            ("e.kt", ""),
            ("f.scala", ""),
            ("g.cs", ""),
            ("h.fsx", ""),
            ("i.tsx", ""),
        ];
        let got = recommendations(&files);
        assert_eq!(got.len(), 10, "{got:?}");
        assert_eq!(got[0], "bazelbuild.vscode-bazel");
        assert!(got.contains(&"rust-lang.rust-analyzer".to_owned()));
        assert!(got.contains(&"ionide.ionide-fsharp".to_owned()));
        assert!(!got.contains(&"ms-python.python".to_owned()));
    }

    #[test]
    fn apply_init_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = entries(|a| apply_init(&root, "demo", a)).expect("init");
        assert!(first.iter().any(|p| p == ".dx/pins"));
        assert!(root.join(".dx/pins").exists());
        std::fs::write(root.join(".envrc"), "custom\n").expect("custom");
        let second = entries(|a| apply_init(&root, "demo", a)).expect("init again");
        assert!(second.iter().any(|p| p == "refused:.envrc"));
        assert_eq!(
            std::fs::read_to_string(root.join(".envrc")).expect("read"),
            "custom\n"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn editorconfig_stays_and_gitattributes_is_written_once() {
        let scratch = dx_test_scratch::scratch("dx-adopt-editor-files-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = entries(|a| apply_init(&root, "demo", a)).expect("init");
        for path in [".editorconfig", ".gitattributes"] {
            assert!(first.iter().any(|p| p == path), "{path}");
        }
        std::fs::write(root.join(".editorconfig"), "mine\n").expect("custom");
        std::fs::write(root.join(".gitattributes"), "mine\n").expect("custom");
        let second = entries(|a| apply_init(&root, "demo", a)).expect("init again");
        assert!(second.iter().any(|p| p == "refused:.gitattributes"));
        assert!(!second.iter().any(|p| p.contains(".editorconfig")));
        assert_eq!(
            std::fs::read_to_string(root.join(".editorconfig")).expect("read"),
            "mine\n"
        );
        scratch.close().expect("cleanup");
    }

    fn editorconfig_file(root: &std::path::Path) -> ScaffoldFile {
        plan_init_files_at(root, "demo", None)
            .expect("plan")
            .into_iter()
            .find(|file| file.path == ".editorconfig")
            .expect("editorconfig")
    }

    #[test]
    fn editorconfig_gets_ktlint_keys_only_with_kotlin_sources() {
        let scratch = dx_test_scratch::scratch("dx-adopt-editorconfig-new-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        assert!(editorconfig_file(&root)
            .content
            .ends_with(EDITORCONFIG_CONTENT));
        std::fs::write(root.join("Main.kt"), "fun main() {}\n").expect("kt");
        let content = editorconfig_file(&root).content;
        assert!(content.contains(EDITORCONFIG_CONTENT));
        assert!(content.ends_with(
            "\n[*.{kt,kts}]\nktlint_code_style = intellij_idea\nij_kotlin_imports_layout = *\n\
             ktlint_standard_function-signature = disabled\n\
             ktlint_standard_trailing-comma-on-call-site = disabled\n\
             ktlint_standard_trailing-comma-on-declaration-site = disabled\n"
        ));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn editorconfig_refresh_appends_the_kotlin_section_once() {
        let scratch = dx_test_scratch::scratch("dx-adopt-editorconfig-refresh-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let mine = "root = true\n[*]\nindent_size = 3\n";
        std::fs::write(root.join(".editorconfig"), mine).expect("custom");

        let plain = editorconfig_file(&root);
        assert!(!owned_file_drift(&root, &plain).expect("drift"));

        std::fs::write(root.join("Main.kt"), "fun main() {}\n").expect("kt");
        let file = editorconfig_file(&root);
        assert!(owned_file_drift(&root, &file).expect("drift"));
        let mut applied = Applied::default();
        refresh_scaffold(&root, std::slice::from_ref(&file), &mut applied).expect("refresh");
        let text = std::fs::read_to_string(root.join(".editorconfig")).expect("read");
        assert!(text.starts_with(mine));
        assert!(text.contains("\n[*.{kt,kts}]\nktlint_code_style = intellij_idea\n"));
        assert!(!owned_file_drift(&root, &file).expect("drift"));

        let mut again = Applied::default();
        refresh_scaffold(&root, std::slice::from_ref(&file), &mut again).expect("refresh");
        assert!(again.wrote.is_empty());
        assert_eq!(
            std::fs::read_to_string(root.join(".editorconfig")).expect("read"),
            text
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn scaffolded_eol_and_indent_rules_match_the_formatters() {
        let files = plan_init_files("demo").expect("init plans");
        let text = |path: &str| {
            files
                .iter()
                .find(|f| f.path == path)
                .expect(path)
                .content
                .clone()
        };
        let attributes = text(".gitattributes");
        for line in [
            "* text=auto eol=lf",
            "*.bat text eol=crlf",
            "*.cmd text eol=crlf",
            "*.ps1 text eol=crlf",
        ] {
            assert!(attributes.lines().any(|l| l == line), "{line}");
        }
        let editorconfig = text(".editorconfig");
        assert!(editorconfig.contains("end_of_line = lf\n"));
        assert!(editorconfig.contains("[*.go]\nindent_style = tab\n"));
        assert!(
            editorconfig.contains("[{Makefile,makefile,GNUmakefile,*.mk}]\nindent_style = tab\n")
        );
    }

    #[test]
    fn the_version_pin_is_generated_and_rewritten_when_stale() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-pin-");
        let root = scratch.path().to_path_buf();
        let pin = root.join(".dx/pins");
        entries(|a| apply_init(&root, "demo", a)).expect("init");
        let expected = crate::release_pins(DX_VERSION)
            .expect("pins")
            .to_lines(DX_VERSION);
        assert_eq!(std::fs::read_to_string(&pin).expect("read"), expected);
        std::fs::write(&pin, "0.0.1\n").expect("stale");
        let again = entries(|a| apply_init(&root, "demo", a)).expect("init again");
        assert!(again.iter().any(|p| p == ".dx/pins"));
        assert_eq!(std::fs::read_to_string(&pin).expect("read"), expected);
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
                l.trim() != "with:"
                    && !l.trim_start().starts_with("disabled_checks:")
                    && !l.trim_start().starts_with("platforms:")
            })
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(
            generated.lines().skip(1).collect::<Vec<_>>(),
            without_defaults.lines().collect::<Vec<_>>()
        );
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
    fn ignores_origin_head_pointing_at_missing_branch() {
        let dir = std::env::temp_dir().join(format!("dx-adopt-stale-head-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
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
        if git(&["init", "-q"])
            && git(&["config", "init.defaultBranch", "trunk"])
            && git(&[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/master",
            ])
        {
            assert_eq!(super::detect_default_branch(&dir), "trunk");
        }
        let _ = std::fs::remove_dir_all(&dir);
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
        assert!(!caller.contains("rules_dx_version"), "{caller}");
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
            serde_json::Value::from("${workspaceFolder}/.dx/local/ide/python/python")
        );
        assert_eq!(
            parsed["clangd.path"],
            serde_json::Value::from("${workspaceFolder}/.dx/local/ide/cc/clangd")
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
    fn editor_settings_point_only_at_plugin_outputs() {
        let value = editor_settings_value();
        let base = format!("${{workspaceFolder}}/{}/", dx_ide::IDE_DIR);
        for key in [
            "python.defaultInterpreterPath",
            "typescript.tsdk",
            "clangd.path",
            "java.jdt.ls.java.home",
            "kotlin.java.home",
        ] {
            let path = value[key].as_str().expect(key);
            let rel = path.strip_prefix(&base).expect(path);
            assert!(dx_ide::OUTPUTS.contains(&rel), "{path}");
        }
        for command in [
            &value["rust-analyzer.workspace.discoverConfig"]["command"][0],
            &value["rust-analyzer.check.overrideCommand"][0],
        ] {
            let path = command.as_str().expect("rust command");
            let rel = path.strip_prefix(&base).expect(path);
            assert!(dx_ide::OUTPUTS.contains(&rel), "{path}");
        }
        let arg = value["clangd.arguments"][0].as_str().expect("arguments");
        let dir = arg.strip_prefix("--compile-commands-dir=").expect(arg);
        let rel = dir.strip_prefix(&base).expect(dir);
        assert!(dx_ide::OUTPUTS.contains(&rel), "{arg}");
    }

    #[test]
    fn rust_analyzer_settings_match_rules_rust_setup_shape() {
        let excludes = serde_json::json!({"**/bazel-*/**": true, "**/.dx/local/ide/**": true});
        let discover_argv = serde_json::json!([
            "${workspaceFolder}/.dx/local/ide/rust_analyzer/discover_bazel_rust_project.exe",
            "--bazel_startup_option=--output_base=${workspaceFolder}/.dx/local/ide/rust",
            "{arg}"
        ]);
        let check_argv = serde_json::json!([
            "${workspaceFolder}/.dx/local/ide/rust_analyzer/flycheck.exe",
            "--output-base=${workspaceFolder}/.dx/local/ide/rust_check",
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
    fn bazelignore_bare_dx_becomes_the_list_and_stays_put() {
        let content = super::bazelignore_content();
        let merged = super::merge_bazelignore(".dx\n", &content);
        assert_eq!(merged, content);
        let mixed = super::merge_bazelignore("node_modules\n/.dx\n# x\n.dx/\n", &content);
        assert_eq!(mixed, format!("node_modules\n{content}# x\n"));
        assert_eq!(super::merge_bazelignore(&mixed, &content), mixed);
        assert!(super::bazelignore_has_bare_dx("a\n./.dx\n"));
        assert!(!super::bazelignore_has_bare_dx(&content));
    }

    #[test]
    fn bazelignore_covers_dx_table() {
        for (text, covered) in [
            ("", false),
            (".dx\n", true),
            (".dx/\n", true),
            ("./.dx\n", true),
            ("  .dx  \n", true),
            (".dx/local/ide\n", false),
            ("node_modules\n.dx/\n", true),
            ("# .dx\n", false),
            (".dx/local\n", true),
            (".dxx\n", false),
            ("node_modules\n", false),
        ] {
            assert_eq!(super::bazelignore_covers_dx(text), covered, "{text:?}");
        }
        let content = super::bazelignore_content();
        assert!(super::bazelignore_covers_dx(&content));
        let listed: Vec<&str> = content.lines().collect();
        for line in crate::build_output::IGNORE_LINES {
            if let Some(dir) = line
                .strip_prefix("/.dx/")
                .and_then(|rest| rest.strip_suffix('/'))
            {
                assert!(listed.contains(&format!(".dx/{dir}").as_str()), "{dir}");
            }
        }
        assert!(!listed
            .iter()
            .any(|line| line.starts_with(".dx/config") || *line == ".dx/pins"));
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
        assert!(init.contains(&".dx/pins".to_owned()), "{init:?}");
        assert_eq!(
            std::fs::read_to_string(root.join(".envrc")).unwrap(),
            "mine\n"
        );
        scratch.close().expect("cleanup");
    }

    const OLD_SHA: &str = "0000000000000000000000000000000000000000";

    fn old_ci() -> String {
        render_consumer_caller().replace(super::CONSUMER_WORKFLOW_PIN, OLD_SHA)
    }

    fn refreshed(root: &std::path::Path) -> Vec<String> {
        let files = plan_init_files("refresh").expect("plan");
        let mut applied = super::Applied::default();
        super::refresh_scaffold(root, &files, &mut applied).expect("refresh");
        applied.wrote
    }

    #[test]
    fn devcontainer_refresh_changes_only_the_pins() {
        let scratch = dx_test_scratch::scratch("dx-adopt-devcontainer-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".devcontainer")).expect("dir");
        let path = root.join(".devcontainer/devcontainer.json");
        let old = DEVCONTAINER_JSON
            .replace(
                &"b1e3b08610ae0b2599f57c3a492eebc93d1d26dbaefb4a0bad0caa7149e4ec95",
                &"0".repeat(64),
            )
            .replace("bazel:1.2.2", "bazel:1.0.0")
            .replace(
                "  \"mounts\"",
                "  // mine\n  \"customizations\": {\"vscode\": {}},\n  \"mounts\"",
            );
        std::fs::write(&path, &old).expect("write");
        refreshed(root);
        let expected = DEVCONTAINER_JSON.replace(
            "  \"mounts\"",
            "  // mine\n  \"customizations\": {\"vscode\": {}},\n  \"mounts\"",
        );
        assert_eq!(std::fs::read_to_string(&path).expect("read"), expected);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn devcontainer_refresh_leaves_other_images_alone() {
        let scratch = dx_test_scratch::scratch("dx-adopt-devcontainer-other-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".devcontainer")).expect("dir");
        let path = root.join(".devcontainer/devcontainer.json");
        let mine = "{\n  \"image\": \"example.com/mine:1@sha256:abc\",\n  \"features\": {}\n}\n";
        std::fs::write(&path, mine).expect("write");
        refreshed(root);
        assert_eq!(std::fs::read_to_string(&path).expect("read"), mine);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn recommendations_gain_detected_ids_and_respect_unwanted() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-recs-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".vscode")).expect("dir");
        std::fs::write(root.join("main.go"), "package main\n").expect("go");
        std::fs::write(root.join("a.rs"), "").expect("rs");
        let mine = "{\n  // mine\n  \"recommendations\": [\"me.mine\", \"ms-python.python\"],\n  \"unwantedRecommendations\": [\"rust-lang.rust-analyzer\"]\n}\n";
        std::fs::write(root.join(".vscode/extensions.json"), mine).expect("write");
        let files = super::plan_init_files_at(root, "refresh", None).expect("plan");
        let mut applied = super::Applied::default();
        super::refresh_scaffold(root, &files, &mut applied).expect("refresh");
        let text = std::fs::read_to_string(root.join(".vscode/extensions.json")).expect("read");
        assert!(text.contains("// mine"), "{text}");
        let ids = dx_merge::jsonc::parse(&text).expect("json")["recommendations"].clone();
        assert_eq!(
            ids,
            serde_json::json!([
                "me.mine",
                "ms-python.python",
                "bazelbuild.vscode-bazel",
                "golang.go"
            ])
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn owned_settings_keep_comments_and_user_keys() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-json-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".vscode")).expect("dir");
        let mine = "{\n  // mine\n  \"editor.tabSize\": 2,\n  \"go.toolsManagement.checkForUpdates\": \"on\",\n  \"search.exclude\": {\"**/mine\": true,},\n}\n";
        std::fs::write(root.join(".vscode/settings.json"), mine).expect("write");
        let wrote = refreshed(root);
        assert_eq!(wrote, [".vscode/settings.json"]);
        let text = std::fs::read_to_string(root.join(".vscode/settings.json")).expect("read");
        assert!(text.contains("// mine"), "{text}");
        assert!(text.contains("\"editor.tabSize\": 2"), "{text}");
        assert!(text.contains("\"**/mine\": true"), "{text}");
        assert!(
            text.contains("\"go.toolsManagement.checkForUpdates\": \"off\""),
            "{text}"
        );
        assert!(text.contains("clangd.path"), "{text}");
        assert!(!root.join(".github").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn an_old_bazel_run_rust_command_is_replaced_by_the_launcher() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-rust-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".vscode")).expect("dir");
        let old = "{\"rust-analyzer.check.overrideCommand\": [\"bazel\", \"run\", \"@rules_rust//tools/rust_analyzer:flycheck\", \"--\", \"--saved-file\", \"$saved_file\"]}\n";
        std::fs::write(root.join(".vscode/settings.json"), old).expect("write");
        refreshed(root);
        let text = std::fs::read_to_string(root.join(".vscode/settings.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(
            parsed["rust-analyzer.check.overrideCommand"],
            editor_settings_value()["rust-analyzer.check.overrideCommand"]
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn second_apply_changes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-twice-");
        let root = scratch.path();
        let first = entries(|a| apply_init(root, "demo", a)).expect("first");
        assert!(
            first.contains(&".vscode/settings.json".to_owned()),
            "{first:?}"
        );
        let before = std::fs::read(root.join(".vscode/settings.json")).expect("read");
        let ci = std::fs::read(root.join(".github/workflows/ci.yml")).expect("read");
        let second = entries(|a| apply_init(root, "demo", a)).expect("second");
        assert!(
            second.iter().all(|e| e.starts_with("refused:")),
            "{second:?}"
        );
        assert!(
            !second.contains(&"refused:.vscode/settings.json".to_owned()),
            "{second:?}"
        );
        assert_eq!(
            std::fs::read(root.join(".vscode/settings.json")).expect("read"),
            before
        );
        assert_eq!(
            std::fs::read(root.join(".github/workflows/ci.yml")).expect("read"),
            ci
        );
        assert!(refreshed(root).is_empty());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn pin_update_drops_the_retired_version_input() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-legacy-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let legacy = old_ci().replace(
            "    secrets:",
            "    with:\n      rules_dx_version: \"0.0.1\"\n    secrets:",
        );
        std::fs::write(root.join(".github/workflows/ci.yml"), legacy).expect("write");
        refreshed(root);
        let new = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read");
        assert!(!new.contains("rules_dx_version"), "{new}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn pin_update_changes_exactly_one_line() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-pin-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let old = old_ci();
        std::fs::write(root.join(".github/workflows/ci.yml"), &old).expect("write");
        let wrote = refreshed(root);
        assert_eq!(wrote, [".github/workflows/ci.yml"]);
        let new = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read");
        let changed = old.lines().zip(new.lines()).filter(|(a, b)| a != b).count();
        assert_eq!(changed, 1, "{new}");
        assert_eq!(old.lines().count(), new.lines().count());
        assert_eq!(new, render_consumer_caller());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn pin_update_repins_release_and_keeps_added_jobs() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-release-pin-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let release = root.join(".github/workflows/release.yml");
        let stale = "0123456789abcdef0123456789abcdef01234567";
        let old = format!(
            "{}  extra:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo hi\n",
            super::render_release_caller().replace(CONSUMER_WORKFLOW_PIN, stale)
        );
        std::fs::write(&release, &old).expect("write");
        refreshed(root);
        let new = std::fs::read_to_string(&release).expect("read");
        assert_eq!(new, old.replace(stale, CONSUMER_WORKFLOW_PIN));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn ci_without_the_workflow_call_is_left_untouched() {
        let scratch = dx_test_scratch::scratch("dx-adopt-owned-refused-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".github/workflows")).expect("dir");
        let mine = "name: ci\njobs: {}\n";
        std::fs::write(root.join(".github/workflows/ci.yml"), mine).expect("write");
        entries(|a| apply_init(root, "demo", a)).expect("init");
        assert_eq!(
            std::fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read"),
            mine
        );
        scratch.close().expect("cleanup");
    }

    #[cfg(unix)]
    mod symlinks {
        use crate::scaffold::tests::apply_init;
        use crate::scaffold::{entries as applied, scaffold_dest_within_root};
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
