use std::path::Path;

use super::{AdoptError, DX_VERSION, HOOK_BUDGET_SECS};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScaffoldFile {
    pub path: String,
    pub content: String,
}

pub const DEVCONTAINER_JSON: &str = concat!(
    "{\n",
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
    "  \"postCreateCommand\": \"bazel run //dx:env && bazel run @rules_dx//:dx -- setup\"\n",
    "}\n",
);

pub const ENVRC_CONTENT: &str = concat!(
    "# Committed direnv entry scaffolded absent-only by `dx init`.\n",
    "# PATH-tools-only: adds managed `.dx/bin` to PATH, nothing else; never invokes Bazel.\n",
    "if [ ! -d \".dx/bin\" ]; then\n",
    "  echo \"dx: missing .dx/bin; run `dx env` or `bazel run //dx:env` to regenerate\" >&2\n",
    "  return 1\n",
    "fi\n",
    "PATH_add .dx/bin\n",
    "watch_file .dx/bin\n",
);

pub fn editor_disposition(language: &str) -> &'static str {
    match language {
        "rust" => "automatic: rust-analyzer discovery plus flycheck (Bazel-backed, separate IDE output base)",
        "go" => "automatic: upstream GOPACKAGESDRIVER (Bazel-backed, pure-Go boundary, cgo out of scope)",
        "cpp" | "c" | "cc" => {
            "snapshot: action-derived compile commands via aquery plus managed clangd, manual dx setup refresh"
        }
        "python" => "projection: .venv interpreter plus imports via dx setup",
        "javascript" => "projection: node_modules via dx setup",
        "typescript" => "projection: node_modules plus tsdk via dx setup",
        "java" | "kotlin" => {
            "manual: setup projection plus checked-in native config, no automatic Bazel-backed driver yet"
        }
        "scala" => {
            "manual: setup projection plus semanticdb/classpath wiring via dx setup, no automatic Bazel-backed driver yet"
        }
        "csharp" | "c#" | "fsharp" | "f#" => {
            "manual: setup projection plus Paket lock wiring via dx setup, no automatic Bazel-backed driver yet"
        }
        _ => "unknown language",
    }
}

pub fn editor_language_supported(language: &str) -> bool {
    matches!(
        language,
        "rust"
            | "python"
            | "javascript"
            | "typescript"
            | "go"
            | "java"
            | "kotlin"
            | "scala"
            | "csharp"
            | "c#"
            | "fsharp"
            | "f#"
            | "c"
            | "cc"
            | "cpp"
    )
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

pub fn plan_init_files(module_name: &str) -> Result<Vec<ScaffoldFile>, AdoptError> {
    let module = validate_init_module(module_name)?;
    Ok(vec![
        ScaffoldFile {
            path: ".dx/version".to_owned(),
            content: format!("{DX_VERSION}\n"),
        },
        ScaffoldFile {
            path: "dx.local.toml".to_owned(),
            content: "# Local-only overrides (gitignored). See dx hooks status.\n[hooks]\n".to_owned(),
        },
        ScaffoldFile {
            path: "dx.hooks.toml".to_owned(),
            content: format!(
                "[hooks]\npre_commit = [\"format --check\", \"lint --check\"]\npre_push = [\"typecheck --check\", \"generate --check\"]\nbudget_secs = {HOOK_BUDGET_SECS}\n"
            ),
        },
        ScaffoldFile {
            path: ".devcontainer/devcontainer.json".to_owned(),
            content: DEVCONTAINER_JSON.to_owned(),
        },
        ScaffoldFile {
            path: ".envrc".to_owned(),
            content: ENVRC_CONTENT.to_owned(),
        },
        ScaffoldFile {
            path: ".vscode/settings.json".to_owned(),
            content: "{\"rust-analyzer.check.command\":\"bazel\",\"python.defaultInterpreterPath\":\".dx/setups/current/.venv/bin/python\",\"typescript.tsdk\":\".dx/setups/current/node_modules/typescript/lib\",\"go.toolsManagement.checkForUpdates\":\"off\",\"clangd.path\":\".dx/bin/clangd\",\"clangd.arguments\":[\"--compile-commands-dir=.dx/setups/current\"],\"java.configuration.updateBuildConfiguration\":\"manual\",\"kotlin.languageServer.enabled\":true}\n"
                .to_owned(),
        },
        ScaffoldFile {
            path: ".vscode/extensions.json".to_owned(),
            content: "{\"recommendations\":[\"rust-lang.rust-analyzer\",\"ms-python.python\",\"bradlc.vscode-tailwindcss\",\"golang.go\",\"llvm-vs-code-extensions.vscode-clangd\",\"redhat.java\",\"fwcd.kotlin\",\"scalameta.metals\",\"ms-dotnettools.csharp\",\"ionide.ionide-fsharp\"]}\n"
                .to_owned(),
        },
        ScaffoldFile {
            path: ".github/workflows/ci.yml".to_owned(),
            content: format!(
                "# Caller template: pins the qualified reusable workflow at a reviewed commit.\nname: ci\non:\n  push: {{}}\n  pull_request: {{}}\njobs:\n  dx:\n    uses: {module}/.github/workflows/reusable-consumer.yml@<reviewed-commit>\n"
            ),
        },
        ScaffoldFile {
            path: "MODULE.bazel.snippet".to_owned(),
            content: format!(
                "# Add to MODULE.bazel:\nbazel_dep(name = \"rules_dx\", version = \"{DX_VERSION}\")\n# module: {module}\n"
            ),
        },
    ])
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
    Ok(dest)
}

pub fn missing_scaffold_files(
    root: &Path,
    files: &[ScaffoldFile],
) -> Result<Vec<String>, AdoptError> {
    let mut missing = Vec::new();
    for file in files {
        let dest = scaffold_dest_within_root(root, &file.path)?;
        if !dest.exists() {
            missing.push(file.path.clone());
        }
    }
    Ok(missing)
}

pub fn apply_init(root: &Path, module_name: &str) -> Result<Vec<String>, AdoptError> {
    let files = plan_init_files(module_name)?;
    let mut staged = Vec::with_capacity(files.len());
    for file in &files {
        staged.push(scaffold_dest_within_root(root, &file.path)?);
    }
    let mut written = Vec::new();
    let mut refused = Vec::new();
    for (file, dest) in files.iter().zip(staged.iter()) {
        if dest.exists() {
            refused.push(format!("refused:{}", file.path));
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
        written.push(file.path.clone());
    }
    written.push("---".to_owned());
    written.extend(refused);
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::super::{
        apply_init, editor_disposition, editor_language_supported, plan_init_files,
        DEVCONTAINER_JSON,
    };
    use super::DEVCONTAINER_JSON as LOCAL_DEVCONTAINER;

    #[test]
    fn scaffold_reexports_match_local_definitions() {
        assert_eq!(DEVCONTAINER_JSON, LOCAL_DEVCONTAINER);
    }

    #[test]
    fn init_plans_nine_absent_only_files() {
        let files = plan_init_files("demo").expect("init plans");
        assert_eq!(files.len(), 9);
        assert!(files.iter().any(|f| f.path == ".dx/version"));
        assert!(files
            .iter()
            .any(|f| f.path == ".devcontainer/devcontainer.json"));
        assert!(files.iter().any(|f| f.path == ".vscode/settings.json"));
        assert!(files.iter().any(|f| f.path == ".envrc"));
    }

    #[test]
    fn envrc_scaffold_is_path_only_with_watch_and_regeneration_guard() {
        let files = plan_init_files("demo").expect("init plans");
        let envrc = files
            .iter()
            .find(|f| f.path == ".envrc")
            .expect("envrc scaffold");
        assert_eq!(envrc.content, super::ENVRC_CONTENT);
        assert!(envrc.content.contains("PATH_add .dx/bin"));
        assert!(envrc.content.contains("watch_file .dx/bin"));
        assert!(envrc.content.contains("dx env"));
        assert!(envrc.content.contains("bazel run //dx:env"));
        assert!(envrc.content.contains("never invokes Bazel"));
    }

    #[test]
    fn devcontainer_scaffold_runs_bootstrap_not_full_build() {
        let files = plan_init_files("demo").expect("init plans");
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
            post_create.contains("bazel run //dx:env"),
            "bootstrap first: {post_create}"
        );
        assert!(
            !post_create.contains("bazel build //..."),
            "no full build on create: {post_create}"
        );
        assert!(super::super::devcontainer_is_admissible(true, true, false));
    }

    #[test]
    fn devcontainer_scaffold_runs_dx_through_the_public_label() {
        assert!(
            DEVCONTAINER_JSON.contains("bazel run @rules_dx//:dx -- setup"),
            "{DEVCONTAINER_JSON}"
        );
        assert!(
            !DEVCONTAINER_JSON.contains("//cli/cli:dx"),
            "{DEVCONTAINER_JSON}"
        );
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
            serde_json::Value::from(".dx/bin/clangd")
        );
        assert_eq!(
            parsed["java.configuration.updateBuildConfiguration"],
            serde_json::Value::from("manual")
        );
        assert!(settings.content.contains("rust-analyzer"));
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
    fn editor_disposition_names_driver_or_snapshot_per_lang() {
        assert!(editor_disposition("rust").starts_with("automatic:"));
        assert!(editor_disposition("go").starts_with("automatic:"));
        assert!(editor_disposition("cpp").starts_with("snapshot:"));
        assert!(editor_disposition("c").starts_with("snapshot:"));
        assert!(editor_disposition("python").starts_with("projection:"));
        assert!(editor_disposition("java").starts_with("manual:"));
        assert!(editor_disposition("kotlin").starts_with("manual:"));
        assert!(editor_disposition("scala").starts_with("manual:"));
        assert!(editor_disposition("csharp").starts_with("manual:"));
        assert!(editor_disposition("fsharp").starts_with("manual:"));
        for lang in [
            "rust",
            "python",
            "javascript",
            "typescript",
            "go",
            "java",
            "kotlin",
            "scala",
            "csharp",
            "fsharp",
            "c",
            "cc",
            "cpp",
        ] {
            assert!(editor_language_supported(lang), "{lang}");
        }
        assert!(!editor_language_supported("ruby"));
        assert!(!editor_language_supported("swift"));
        assert_eq!(editor_disposition("ruby"), "unknown language");
    }

    #[test]
    fn apply_init_writes_absent_only_and_refuses_existing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-init-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(&root).expect("tmp");
        let first = apply_init(&root, "demo").expect("init");
        assert!(first.iter().any(|p| p == ".dx/version"));
        assert!(root.join(".dx/version").exists());
        std::fs::write(root.join(".dx/version"), "custom\n").expect("custom");
        let second = apply_init(&root, "demo").expect("init again");
        assert!(second.iter().any(|p| p == "refused:.dx/version"));
        assert_eq!(
            std::fs::read_to_string(root.join(".dx/version")).expect("read"),
            "custom\n"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn hooks_scaffold_budget_tracks_hook_budget_const() {
        let files = plan_init_files("demo").expect("init plans");
        let hooks = files
            .iter()
            .find(|f| f.path == "dx.hooks.toml")
            .expect("hooks scaffold");
        assert!(
            hooks
                .content
                .contains(&format!("budget_secs = {}", super::super::HOOK_BUDGET_SECS)),
            "{}",
            hooks.content
        );
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
    fn init_module_names_cannot_break_the_ci_template() {
        let files = plan_init_files("demo").expect("init plans");
        let ci = files
            .iter()
            .find(|f| f.path == ".github/workflows/ci.yml")
            .expect("ci scaffold");
        assert!(ci.content.contains("uses: demo/.github/workflows/"));
        let snippet = files
            .iter()
            .find(|f| f.path == "MODULE.bazel.snippet")
            .expect("snippet");
        assert!(snippet.content.contains("# module: demo\n"));
    }

    #[test]
    fn apply_init_writes_nothing_for_invalid_modules() {
        for hostile in ["Bad Name", "../evil", "with\nnewline"] {
            let scratch = dx_test_scratch::scratch("dx-adopt-init-reject-");
            let root = scratch.path().to_path_buf();
            std::fs::write(root.join("sentinel"), "stay").expect("sentinel");
            assert!(apply_init(&root, hostile).is_err(), "{hostile:?} must fail");
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
}
