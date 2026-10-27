use std::path::Path;

use super::{AdoptError, InitPart, InitParts, DX_VERSION};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaffoldFile {
    pub path: String,
    pub content: String,
}

pub const DEVCONTAINER_JSON: &str = concat!(    "{\n",
    "  \"name\": \"rules_dx\",\n",
    "  \"image\": \"mcr.microsoft.com/devcontainers/base:ubuntu\",\n",
    "  \"features\": {\n",
    "    \"ghcr.io/devcontainers/features/bazel:1\": {}\n",
    "  },\n",
    "  \"customizations\": {\n",
    "    \"vscode\": {\n",
    "      \"extensions\": [\"rust-lang.rust-analyzer\",\"golang.go\",\"llvm-vs-code-extensions.vscode-clangd\",\"redhat.java\",\"fwcd.kotlin\",\"scalameta.metals\",\"ms-dotnettools.csharp\",\"ionide.ionide-fsharp\"]\n",
    "    }\n",
    "  },\n",
    "  \"postCreateCommand\": \"bazel run @rules_dx//:dx -- env --apply && bazel run @rules_dx//:dx -- codegen --apply\"\n",
    "}\n",
);

pub const ENVRC_CONTENT: &str = concat!(
    "# Committed direnv entry scaffolded absent-only by `dx init`.\n",
    "# PATH-tools-only: adds managed `.dx/bin` to PATH, nothing else; never invokes Bazel.\n",
    "if [ ! -d \".dx/bin\" ]; then\n",
    "  echo \"dx: missing .dx/bin; run `dx env --apply` to regenerate\" >&2\n",
    "  return 1\n",
    "fi\n",
    "PATH_add .dx/bin\n",
    "watch_file .dx/bin\n",
);

/// The `bazel_dep` floor `dx init` writes: `FLOOR_RULES_CC` in `modules/versions.bzl`.
pub const CONSUMER_RULES_CC_VERSION: &str = "0.2.27";

/// The `bazel_dep` floor `dx init` writes: `FLOOR_LLVM` in `modules/versions.bzl`.
pub const CONSUMER_LLVM_VERSION: &str = "0.8.23";

/// The `bazel_dep` floor `dx init` writes: `FLOOR_RULES_RUST` in `modules/versions.bzl`.
pub const CONSUMER_RULES_RUST_VERSION: &str = "0.74.0";

pub const CONSUMER_RUST_EDITION: &str = "2021";

pub const CONSUMER_RUST_VERSION: &str = "1.98.0";

pub const CONSUMER_RUSTFMT_VERSION: &str = "1.98.0";

/// The `bazel_dep` floor `dx init` writes: `FLOOR_RULES_RUBY` in `modules/versions.bzl`.
pub const CONSUMER_RULES_RUBY_VERSION: &str = "0.28.0";

pub const CONSUMER_RUBY_VERSION: &str = "3.4.9";

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
    format!(
        "bazel_dep(name = \"rules_dx\", version = \"{dx}\")\nbazel_dep(name = \"rules_cc\", version = \"{cc}\")\nbazel_dep(name = \"llvm\", version = \"{llvm}\")\nbazel_dep(name = \"rules_rust\", version = \"{rust_rules}\")\n\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    edition = \"{edition}\",\n    rustfmt_version = \"{rustfmt}\",\n    versions = [\"{rust}\"],\n)\nuse_repo(rust, \"rust_toolchains\")\n\nregister_toolchains(\"@llvm//toolchain:all\")\nregister_toolchains(\"@rust_toolchains//:all\")\n",
        dx = DX_VERSION,
        cc = CONSUMER_RULES_CC_VERSION,
        llvm = CONSUMER_LLVM_VERSION,
        rust_rules = CONSUMER_RULES_RUST_VERSION,
        edition = CONSUMER_RUST_EDITION,
        rustfmt = CONSUMER_RUSTFMT_VERSION,
        rust = CONSUMER_RUST_VERSION,
    )
}

/// Puts the CUDA stanza ahead of the consumer's `register_toolchains` lines.
///
/// The first registered toolchain wins, and the CUDA host compiler only applies when its
/// flag is on. If `@llvm//toolchain:all` came first, it would also match for nvcc's host
/// compilation and nvcc would get a clang it does not support.
fn with_cuda_stanza(setup: &str) -> String {
    let stanza = format!(
        "# CUDA: this workspace has .cu sources, so the pinned toolkit and its host compiler follow.\n{}\n",
        crate::cuda_stanza::render_cuda_module_stanza("rules_dx"),
    );
    match setup.find("register_toolchains(") {
        Some(at) => format!("{}{}{}", &setup[..at], stanza, &setup[at..]),
        None => format!("{setup}\n{stanza}"),
    }
}

/// Adds the include of the dx-written overrides right after the rules_dx `bazel_dep`.
fn with_overrides_include(setup: &str) -> String {
    match setup.split_once('\n') {
        Some((first, rest)) => format!(
            "{first}\ninclude(\"{}\")\n{rest}",
            crate::overrides::OVERRIDES_INCLUDE
        ),
        None => setup.to_owned(),
    }
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

pub const CONSUMER_WORKFLOW_PIN: &str = "e97ab7b550d08e047c35edc5f50c7d242227842f";

pub fn render_consumer_caller() -> String {
    format!(
        "name: ci\non:\n  pull_request: {{}}\n  merge_group:\n    types: [checks_requested]\n  push:\n    branches: [main]\n  workflow_dispatch: {{}}\njobs:\n  dx:\n    uses: {owner}/{repo}/{path}@{pin}\n    with:\n      rules_dx_version: \"{version}\"\n      disabled_checks: \"\"\n      platforms: '[\"linux_x86_64\"]'\n    secrets: inherit\n    permissions:\n      contents: read\n      pull-requests: write\n",
        owner = CONSUMER_WORKFLOW_OWNER,
        repo = CONSUMER_WORKFLOW_REPO,
        path = CONSUMER_WORKFLOW_PATH,
        pin = CONSUMER_WORKFLOW_PIN,
        version = DX_VERSION,
    )
}

pub fn render_editor_settings(windows: bool) -> String {
    let python = if windows {
        ".dx/setups/current/.venv/bin/python.exe"
    } else {
        ".dx/setups/current/.venv/bin/python"
    };
    let clangd = if windows {
        ".dx/bin/clangd.exe"
    } else {
        ".dx/bin/clangd"
    };
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
        "python.defaultInterpreterPath": python,
        "typescript.tsdk": ".dx/setups/current/node_modules/typescript/lib",
        "go.toolsManagement.checkForUpdates": "off",
        "clangd.path": clangd,
        "clangd.arguments": ["--compile-commands-dir=.dx/setups/current"],
        "java.configuration.updateBuildConfiguration": "manual",
        "kotlin.languageServer.enabled": true,
    });
    format!("{settings}\n")
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

pub fn plan_init_files(
    module_name: &str,
    parts: InitParts,
) -> Result<Vec<ScaffoldFile>, AdoptError> {
    plan_init_files_for_host(module_name, cfg!(windows), parts)
}

pub fn plan_init_files_for_host(
    module_name: &str,
    windows: bool,
    parts: InitParts,
) -> Result<Vec<ScaffoldFile>, AdoptError> {
    plan_init_files_with(module_name, windows, false, parts)
}

/// Plans the essential init files plus the files of each selected part.
///
/// `cuda` appends the pinned CUDA stanza to `MODULE.bazel.snippet`.
pub fn plan_init_files_with(
    module_name: &str,
    windows: bool,
    cuda: bool,
    parts: InitParts,
) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let module = validate_init_module(module_name)?;
    let files = vec![
        ScaffoldFile {
            path: ".dx/version".to_owned(),
            content: format!("{DX_VERSION}\n"),
        },
        ScaffoldFile {
            path: ".devcontainer/devcontainer.json".to_owned(),
            content: DEVCONTAINER_JSON.to_owned(),
        },
        ScaffoldFile {
            path: ".gitignore".to_owned(),
            content: format!("{}\n", crate::build_output::IGNORE_LINES.join("\n")),
        },
        ScaffoldFile {
            path: ".bazelignore".to_owned(),
            content: BAZELIGNORE_CONTENT.to_owned(),
        },
        ScaffoldFile {
            path: ".envrc".to_owned(),
            content: ENVRC_CONTENT.to_owned(),
        },
        ScaffoldFile {
            path: ".vscode/settings.json".to_owned(),
            content: render_editor_settings(windows),
        },
        ScaffoldFile {
            path: ".vscode/extensions.json".to_owned(),
            content: "{\"recommendations\":[\"rust-lang.rust-analyzer\",\"ms-python.python\",\"bradlc.vscode-tailwindcss\",\"golang.go\",\"llvm-vs-code-extensions.vscode-clangd\",\"redhat.java\",\"fwcd.kotlin\",\"scalameta.metals\",\"ms-dotnettools.csharp\",\"ionide.ionide-fsharp\"]}\n"
                .to_owned(),
        },
        ScaffoldFile {
            path: ".github/workflows/ci.yml".to_owned(),
            content: render_consumer_caller(),
        },
        ScaffoldFile {
            path: "MODULE.bazel.snippet".to_owned(),
            content: format!(
                "# Add to MODULE.bazel: hermetic C/C++ and Rust defaults.\n# Own toolchains: keep the bazel_dep lines, drop the register_toolchains lines.\n# module: {module}\n{setup}",
                setup = with_overrides_include(&if cuda {
                    with_cuda_stanza(&render_consumer_module_setup())
                } else {
                    render_consumer_module_setup()
                }),
            ),
        },
    ];
    Ok(files
        .into_iter()
        .filter(|file| {
            InitPart::ALL
                .into_iter()
                .find(|part| part.scaffold_paths().contains(&file.path.as_str()))
                .is_none_or(|part| parts.contains(part))
        })
        .collect())
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
    let escapes = || AdoptError::ScaffoldEscapesRoot {
        path: relative.to_owned(),
    };
    let canonical_root = match root.canonicalize() {
        Ok(path) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(escapes()),
    };
    let mut current = root.to_path_buf();
    for part in Path::new(relative).components() {
        current.push(part);
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let resolved = current.canonicalize().map_err(|_| escapes())?;
                if !resolved.starts_with(&canonical_root) {
                    return Err(escapes());
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
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

/// Checks every destination, then writes each absent file and records it in `applied` as it lands.
pub fn apply_scaffold(
    root: &Path,
    files: &[ScaffoldFile],
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    let relatives: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    preflight_destinations(root, &relatives)?;
    let mut staged = Vec::with_capacity(files.len());
    for file in files {
        staged.push(scaffold_dest_within_root(root, &file.path)?);
    }
    for (file, dest) in files.iter().zip(staged.iter()) {
        if dest.exists() {
            applied.refused.push(file.path.clone());
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdoptError::CreateParent {
                parent: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        dx_atomic_fs::write_atomic(dest, file.content.as_ref()).map_err(|e| {
            AdoptError::WriteFile {
                path: dest.display().to_string(),
                detail: e.to_string(),
            }
        })?;
        applied.wrote.push(file.path.clone());
    }
    Ok(())
}

pub fn apply_init(
    root: &Path,
    module_name: &str,
    parts: InitParts,
    applied: &mut Applied,
) -> Result<(), AdoptError> {
    let cuda = crate::load_graph::workspace_has_cuda_sources(root)?;
    apply_scaffold(
        root,
        &plan_init_files_with(module_name, cfg!(windows), cuda, parts)?,
        applied,
    )
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
        apply_init, entries, plan_init_files, plan_init_files_for_host, plan_init_files_with,
        render_consumer_caller, render_editor_settings, CONSUMER_LLVM_VERSION,
        CONSUMER_RUBY_VERSION, CONSUMER_RULES_CC_VERSION, CONSUMER_RULES_RUBY_VERSION,
        CONSUMER_RULES_RUST_VERSION, CONSUMER_RUSTFMT_VERSION, CONSUMER_RUST_EDITION,
        CONSUMER_RUST_VERSION, CONSUMER_WORKFLOW_OWNER, CONSUMER_WORKFLOW_PATH,
        CONSUMER_WORKFLOW_PIN, CONSUMER_WORKFLOW_REPO, DEVCONTAINER_JSON, DX_VERSION,
    };
    use crate::version::consumer_setup::parse_consumer_setup;

    fn planned_paths(parts: crate::InitParts) -> Vec<String> {
        plan_init_files("demo", parts)
            .expect("init plans")
            .into_iter()
            .map(|f| f.path)
            .collect()
    }

    const ESSENTIALS: [&str; 4] = [
        ".dx/version",
        ".gitignore",
        ".bazelignore",
        "MODULE.bazel.snippet",
    ];

    #[test]
    fn init_plans_essentials_by_default() {
        assert_eq!(planned_paths(crate::InitParts::NONE), ESSENTIALS);
    }

    #[test]
    fn each_part_adds_its_files() {
        for part in crate::InitPart::ALL {
            let mut paths = planned_paths(crate::InitParts::NONE.with(part));
            paths.retain(|path| !ESSENTIALS.contains(&path.as_str()));
            assert_eq!(paths, part.scaffold_paths(), "{}", part.name());
        }
    }

    #[test]
    fn full_plans_the_old_nine() {
        let paths = planned_paths(crate::InitParts::ALL);
        assert_eq!(paths.len(), 9);
        for part in crate::InitPart::ALL {
            for path in part.scaffold_paths() {
                assert!(paths.iter().any(|p| p == path), "{path}");
            }
        }
        for path in ESSENTIALS {
            assert!(paths.iter().any(|p| p == path), "{path}");
        }
    }

    #[test]
    fn envrc_scaffold_is_path_only_with_watch_and_regeneration_guard() {
        let files = plan_init_files("demo", crate::InitParts::ALL).expect("init plans");
        let envrc = files
            .iter()
            .find(|f| f.path == ".envrc")
            .expect("envrc scaffold");
        assert_eq!(envrc.content, super::ENVRC_CONTENT);
        assert!(envrc.content.contains("PATH_add .dx/bin"));
        assert!(envrc.content.contains("watch_file .dx/bin"));
        assert!(envrc.content.contains("dx env --apply"));
        assert!(!envrc.content.contains("dx:env"));
        assert!(envrc.content.contains("never invokes Bazel"));
    }

    #[test]
    fn devcontainer_scaffold_runs_bootstrap_not_full_build() {
        let files = plan_init_files("demo", crate::InitParts::ALL).expect("init plans");
        let scaffold = files
            .iter()
            .find(|f| f.path == ".devcontainer/devcontainer.json")
            .expect("devcontainer scaffold");
        assert_eq!(scaffold.content, DEVCONTAINER_JSON);
        let parsed: serde_json::Value =
            serde_json::from_str(&scaffold.content).expect("valid JSON");
        assert_eq!(parsed["name"], serde_json::Value::from("rules_dx"));
        assert_eq!(
            parsed["image"],
            serde_json::Value::from("mcr.microsoft.com/devcontainers/base:ubuntu")
        );
        assert!(
            parsed["features"]["ghcr.io/devcontainers/features/bazel:1"].is_object(),
            "bazel feature must stay"
        );
        let extensions = parsed["customizations"]["vscode"]["extensions"]
            .as_array()
            .expect("vscode extensions array");
        assert!(
            extensions.contains(&serde_json::Value::from("rust-lang.rust-analyzer")),
            "rust-analyzer extension must stay: {extensions:?}"
        );
        let post_create = parsed["postCreateCommand"]
            .as_str()
            .expect("postCreateCommand string");
        assert!(
            post_create.contains("bazel run @rules_dx//:dx -- env --apply && "),
            "bootstrap first: {post_create}"
        );
        assert!(
            post_create.contains("bazel run @rules_dx//:dx -- codegen --apply"),
            "{post_create}"
        );
        assert!(!post_create.contains("dx:env"), "{post_create}");
        assert!(!post_create.contains("setup"), "{post_create}");
        assert!(
            !post_create.contains("bazel build //..."),
            "no full build on create: {post_create}"
        );
    }

    #[test]
    fn devcontainer_scaffold_runs_dx_through_the_public_label() {
        assert!(
            DEVCONTAINER_JSON.contains("bazel run @rules_dx//:dx -- env --apply"),
            "{DEVCONTAINER_JSON}"
        );
        assert!(!DEVCONTAINER_JSON.contains("dx:env"), "{DEVCONTAINER_JSON}");
        assert!(
            DEVCONTAINER_JSON.contains("bazel run @rules_dx//:dx -- codegen --apply"),
            "{DEVCONTAINER_JSON}"
        );
        assert!(
            !DEVCONTAINER_JSON.contains("//cli/cli:dx"),
            "{DEVCONTAINER_JSON}"
        );
    }

    #[test]
    fn init_scaffold_covers_admitted_editors() {
        let files =
            plan_init_files_for_host("demo", false, crate::InitParts::ALL).expect("init plans");
        let settings = files
            .iter()
            .find(|f| f.path == ".vscode/settings.json")
            .expect("settings scaffold");
        let parsed: serde_json::Value =
            serde_json::from_str(&settings.content).expect("valid JSON");
        let clangd = if cfg!(windows) {
            ".dx/bin/clangd.exe"
        } else {
            ".dx/bin/clangd"
        };
        assert_eq!(parsed["clangd.path"], serde_json::Value::from(clangd));
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
        assert!(devcontainer.content.contains("rust-lang.rust-analyzer"));
        assert!(devcontainer
            .content
            .contains("llvm-vs-code-extensions.vscode-clangd"));
    }

    #[test]
    fn apply_init_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init");
        assert!(first.iter().any(|p| p == ".dx/version"));
        assert!(root.join(".dx/version").exists());
        std::fs::write(root.join(".dx/version"), "custom\n").expect("custom");
        let second =
            entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init again");
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
                plan_init_files(hostile, crate::InitParts::ALL).is_err(),
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
            let files = plan_init_files(module, crate::InitParts::ALL).expect("init plans");
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
        let files = plan_init_files("demo", crate::InitParts::ALL).expect("init plans");
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
        assert_eq!(render_consumer_caller(), starter);
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
            "disabled_checks:",
            "platforms: '[\"linux_x86_64\"]'",
            "secrets: inherit",
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
        for removed in ["scheduling_mode", "code_scanning_opt_in"] {
            assert!(
                !caller.contains(removed),
                "caller passes {removed}: {caller}"
            );
        }
    }

    #[test]
    fn editor_settings_use_host_native_interpreter_and_tool_names() {
        for (windows, python, clangd) in [
            (
                false,
                ".dx/setups/current/.venv/bin/python",
                ".dx/bin/clangd",
            ),
            (
                true,
                ".dx/setups/current/.venv/bin/python.exe",
                ".dx/bin/clangd.exe",
            ),
        ] {
            let settings = render_editor_settings(windows);
            let parsed: serde_json::Value = serde_json::from_str(&settings).expect("valid JSON");
            assert_eq!(
                parsed["python.defaultInterpreterPath"],
                serde_json::Value::from(python)
            );
            assert_eq!(parsed["clangd.path"], serde_json::Value::from(clangd));
            assert!(settings.ends_with('\n'), "{settings}");
        }
        let unix = render_editor_settings(false);
        let win = render_editor_settings(true);
        assert_ne!(unix, win);
        for key in [
            "rust-analyzer.workspace.discoverConfig",
            "typescript.tsdk",
            "go.toolsManagement.checkForUpdates",
            "clangd.arguments",
            "java.configuration.updateBuildConfiguration",
            "kotlin.languageServer.enabled",
        ] {
            assert!(unix.contains(key) && win.contains(key), "lost {key}");
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
        let unix: serde_json::Value =
            serde_json::from_str(&render_editor_settings(false)).expect("unix json");
        let win: serde_json::Value =
            serde_json::from_str(&render_editor_settings(true)).expect("windows json");
        for parsed in [&unix, &win] {
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
        for key in [
            "rust-analyzer.workspace.discoverConfig",
            "rust-analyzer.check.overrideCommand",
            "files.watcherExclude",
            "search.exclude",
        ] {
            assert_eq!(unix[key], win[key], "{key}");
        }
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
    fn init_scaffold_matches_on_both_hosts_except_native_names() {
        let unix =
            plan_init_files_for_host("demo", false, crate::InitParts::ALL).expect("unix plans");
        let win =
            plan_init_files_for_host("demo", true, crate::InitParts::ALL).expect("windows plans");
        assert_eq!(unix.len(), win.len());
        for (left, right) in unix.iter().zip(win.iter()) {
            assert_eq!(left.path, right.path);
            if left.path == ".vscode/settings.json" {
                assert_eq!(left.content, render_editor_settings(false));
                assert_eq!(right.content, render_editor_settings(true));
            } else {
                assert_eq!(left.content, right.content, "{}", left.path);
            }
        }
        let host = plan_init_files("demo", crate::InitParts::ALL).expect("host plans");
        let expected = plan_init_files_for_host("demo", cfg!(windows), crate::InitParts::ALL)
            .expect("host plans");
        assert_eq!(
            host.iter().map(|f| &f.path).collect::<Vec<_>>(),
            expected.iter().map(|f| &f.path).collect::<Vec<_>>()
        );
        assert_eq!(host, expected);
    }

    #[test]
    fn init_snippet_is_a_complete_consumer_setup() {
        let scratch = dx_test_scratch::scratch("dx-adopt-consumer-setup-");
        let root = scratch.path().to_path_buf();
        entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init");
        let text =
            std::fs::read_to_string(root.join("MODULE.bazel.snippet")).expect("emitted snippet");
        assert!(text.contains(&format!(
            "bazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\ninclude(\"{}\")\n",
            crate::overrides::OVERRIDES_INCLUDE
        )));
        let setup = parse_consumer_setup(&text);
        assert_eq!(
            setup.deps,
            vec![
                ("rules_dx".to_owned(), DX_VERSION.to_owned()),
                ("rules_cc".to_owned(), CONSUMER_RULES_CC_VERSION.to_owned()),
                ("llvm".to_owned(), CONSUMER_LLVM_VERSION.to_owned()),
                (
                    "rules_rust".to_owned(),
                    CONSUMER_RULES_RUST_VERSION.to_owned()
                ),
            ]
        );
        assert!(
            setup
                .extensions
                .iter()
                .any(|(label, tag)| label == "@rules_rust//rust:extensions.bzl" && tag == "rust"),
            "snippet must opt into the upstream rust extension: {setup:?}"
        );
        assert_eq!(
            setup.tag_arg("rust.toolchain", "edition").as_deref(),
            Some(CONSUMER_RUST_EDITION)
        );
        assert_eq!(
            setup
                .tag_arg("rust.toolchain", "rustfmt_version")
                .as_deref(),
            Some(CONSUMER_RUSTFMT_VERSION)
        );
        assert_eq!(
            setup.tag_list_arg("rust.toolchain", "versions"),
            vec![CONSUMER_RUST_VERSION.to_owned()]
        );
        assert!(
            setup
                .repos
                .iter()
                .any(|(var, repo)| var == "rust" && repo == "rust_toolchains"),
            "snippet must materialize the rust toolchains repo: {setup:?}"
        );
        assert!(
            setup
                .registered
                .contains(&"@llvm//toolchain:all".to_owned()),
            "snippet must register the hermetic LLVM toolchains: {setup:?}"
        );
        assert!(
            setup
                .registered
                .contains(&"@rust_toolchains//:all".to_owned()),
            "snippet must register the rust toolchains: {setup:?}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn cuda_snippet_adds_the_stanza_and_registers_the_host_compiler_first() {
        let plain =
            plan_init_files_with("demo", false, false, crate::InitParts::ALL).expect("plans");
        let cuda = plan_init_files_with("demo", false, true, crate::InitParts::ALL).expect("plans");
        let snippet = |files: &[super::ScaffoldFile]| {
            files
                .iter()
                .find(|file| file.path == "MODULE.bazel.snippet")
                .expect("snippet")
                .content
                .clone()
        };
        let (plain, cuda) = (snippet(&plain), snippet(&cuda));
        assert!(!plain.contains("cuda"), "non-CUDA snippet must not change");
        let setup = parse_consumer_setup(&cuda);
        let base = parse_consumer_setup(&plain);
        assert!(base.deps.iter().all(|dep| setup.deps.contains(dep)));
        assert!(setup.deps.iter().any(|(name, _)| name == "rules_cuda"));
        assert_eq!(
            setup.registered,
            [
                "@cuda_host_llvm//:all",
                "@llvm//toolchain:all",
                "@rust_toolchains//:all"
            ]
        );
        assert!(cuda.contains("cuda-eula"));
    }

    #[test]
    fn apply_init_adds_the_stanza_only_for_cuda_workspaces() {
        let plain = dx_test_scratch::scratch("dx-adopt-init-nocuda-");
        entries(|a| apply_init(plain.path(), "demo", crate::InitParts::ALL, a)).expect("init");
        let text =
            std::fs::read_to_string(plain.path().join("MODULE.bazel.snippet")).expect("read");
        assert!(!text.contains("rules_cuda"));
        plain.close().expect("cleanup");
        let with = dx_test_scratch::scratch("dx-adopt-init-cuda-");
        std::fs::create_dir_all(with.path().join("k")).expect("mkdir");
        std::fs::write(with.path().join("k/a.cu"), "").expect("write");
        entries(|a| apply_init(with.path(), "demo", crate::InitParts::ALL, a)).expect("init");
        let text = std::fs::read_to_string(with.path().join("MODULE.bazel.snippet")).expect("read");
        assert!(text.contains("bazel_dep(name = \"rules_cuda\""), "{text}");
        with.close().expect("cleanup");
    }

    #[test]
    fn apply_init_snippet_is_absent_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-snippet-absent-");
        let root = scratch.path().to_path_buf();
        entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init");
        let snippet = root.join("MODULE.bazel.snippet");
        std::fs::write(&snippet, "custom\n").expect("custom");
        let second =
            entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init again");
        assert!(
            second.iter().any(|p| p == "refused:MODULE.bazel.snippet"),
            "snippet must stay absent-only: {second:?}"
        );
        assert_eq!(std::fs::read_to_string(&snippet).expect("read"), "custom\n");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn consumer_pins_match_toolchain_metadata() {
        let source = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        let text = std::fs::read_to_string(
            std::path::Path::new(&source)
                .join(workspace)
                .join("modules/versions.bzl"),
        )
        .expect("modules/versions.bzl ships as test data");
        for (key, value) in [
            ("FLOOR_RULES_CC", CONSUMER_RULES_CC_VERSION),
            ("FLOOR_LLVM", CONSUMER_LLVM_VERSION),
            ("FLOOR_RULES_RUST", CONSUMER_RULES_RUST_VERSION),
            ("RUST_EDITION", CONSUMER_RUST_EDITION),
            ("RUST_VERSION", CONSUMER_RUST_VERSION),
            ("RUSTFMT_VERSION", CONSUMER_RUSTFMT_VERSION),
            ("FLOOR_RULES_RUBY", CONSUMER_RULES_RUBY_VERSION),
            ("RUBY_VERSION", CONSUMER_RUBY_VERSION),
        ] {
            assert!(
                text.contains(&format!("{key} = \"{value}\"")),
                "{key} drifts from modules/versions.bzl"
            );
        }
    }

    #[test]
    fn apply_init_writes_nothing_for_invalid_modules() {
        for hostile in ["Bad Name", "../evil", "with\nnewline"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            assert!(
                entries(|a| apply_init(&root, hostile, crate::InitParts::ALL, a)).is_err(),
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
        let err = apply_init(&root, "demo", crate::InitParts::ALL, &mut applied)
            .expect_err("file parent refused");
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
    fn init_and_new_share_absent_only_collision_behavior() {
        let scratch = dx_test_scratch::scratch("dx-adopt-shared-collision-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(root.join(".envrc"), "mine\n").expect("handwritten");
        let init = entries(|a| apply_init(&root, "demo", crate::InitParts::ALL, a)).expect("init");
        assert!(init.contains(&"refused:.envrc".to_owned()), "{init:?}");
        assert!(init.contains(&".dx/version".to_owned()), "{init:?}");
        assert_eq!(
            std::fs::read_to_string(root.join(".envrc")).unwrap(),
            "mine\n"
        );
        std::fs::create_dir_all(root.join("app")).expect("app");
        std::fs::write(root.join("app/Cargo.toml"), "# mine\n").expect("manual build");
        let new = entries(|a| super::super::apply_new(&root, "rust", "app", a)).expect("new");
        assert!(
            new.contains(&"refused:app/Cargo.toml".to_owned()),
            "{new:?}"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("app/Cargo.toml")).unwrap(),
            "# mine\n"
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
            let err = applied(|a| apply_init(&root, "demo", crate::InitParts::ALL, a))
                .expect_err("escape refused");
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
            let written = applied(|a| apply_init(&root, "demo", crate::InitParts::ALL, a))
                .expect("internal link allowed");
            assert!(written.contains(&".github/workflows/ci.yml".to_owned()));
            assert!(root.join("real/workflows/ci.yml").is_file());
            scratch.close().expect("cleanup");
        }
    }
}
