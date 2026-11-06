//! Tool config profiles: the `standard` and `strict` bases `dx init` stages under
//! `.dx/config/<tool>/` and the root file that picks one.
//!
//! The profile is recorded nowhere. Both bases are always planned, and the root file is the
//! only thing that says which one a tool uses.

use std::collections::BTreeSet;
use std::path::Path;

use crate::scaffold::{FileKind, ScaffoldFile};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Profile {
    #[default]
    Standard,
    Strict,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }
}

/// How a tool's root file relates to the profile files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    /// The root file holds an extends line naming the chosen base.
    Inherit,
    /// The root file is a copy of the chosen base.
    Copy,
}

struct Entry {
    tool: &'static str,
    family: &'static str,
    /// Extensions that mean the tool applies, for tools no family owns.
    extensions: &'static [&'static str],
    ext: &'static str,
    root: &'static str,
    /// The root file of an inherit tool, with `{base}` for the chosen base path.
    root_template: &'static str,
    /// Prefix of the comment line in a generated base; `None` for formats without comments.
    comment: Option<&'static str>,
    mode: Mode,
    /// Whether `quality/native_config.bzl` has a `<tool>_config` rule for the tool.
    config_rule: bool,
    /// Root files that mean the repo already configures the tool; a trailing `*` matches a prefix.
    configured: &'static [&'static str],
    /// Table in `pyproject.toml` that also means the tool is configured.
    pyproject_table: Option<&'static str>,
    standard: &'static str,
    strict: &'static str,
}

/// A framework block of the eslint config, written when one of its extensions occurs.
struct Framework {
    name: &'static str,
    extensions: &'static [&'static str],
    template: &'static str,
}

const FRAMEWORKS: &[Framework] = &[
    Framework {
        name: "astro",
        extensions: &[".astro"],
        template: include_str!("../templates/config/eslint/frameworks/astro.mjs"),
    },
    Framework {
        name: "react",
        extensions: &[".jsx", ".tsx"],
        template: include_str!("../templates/config/eslint/frameworks/react.mjs"),
    },
    Framework {
        name: "svelte",
        extensions: &[".svelte"],
        template: include_str!("../templates/config/eslint/frameworks/svelte.mjs"),
    },
    Framework {
        name: "vue",
        extensions: &[".vue"],
        template: include_str!("../templates/config/eslint/frameworks/vue.mjs"),
    },
];

const NO_FRAMEWORKS: &str = include_str!("../templates/config/eslint/frameworks.mjs");

const TABLE: &[Entry] = &[
    Entry {
        tool: "ruff",
        family: "python",
        extensions: &[],
        ext: "toml",
        root: "ruff.toml",
        root_template:
            "# Repo settings go here; the extend line picks the profile.\nextend = \"{base}\"\n",
        comment: Some("#"),
        mode: Mode::Inherit,
        config_rule: true,
        configured: &["ruff.toml", ".ruff.toml"],
        pyproject_table: Some("ruff"),
        standard: include_str!("../templates/config/ruff/standard.toml"),
        strict: include_str!("../templates/config/ruff/strict.toml"),
    },
    Entry {
        tool: "eslint",
        family: "js",
        extensions: &[],
        ext: "mjs",
        root: "eslint.config.mjs",
        root_template: "export { default } from \"./{base}\";\n",
        comment: Some("//"),
        mode: Mode::Inherit,
        config_rule: true,
        configured: &[
            "eslint.config.js",
            "eslint.config.cjs",
            "eslint.config.mjs",
            "eslint.config.ts",
            ".eslintrc*",
        ],
        pyproject_table: None,
        standard: include_str!("../templates/config/eslint/standard.mjs"),
        strict: include_str!("../templates/config/eslint/strict.mjs"),
    },
    Entry {
        tool: "tsconfig",
        family: "js",
        extensions: &[],
        ext: "json",
        root: "tsconfig.json",
        root_template: "{ \"extends\": \"./{base}\" }\n",
        comment: None,
        mode: Mode::Inherit,
        config_rule: false,
        configured: &["tsconfig.json"],
        pyproject_table: None,
        standard: include_str!("../templates/config/tsconfig/standard.json"),
        strict: include_str!("../templates/config/tsconfig/strict.json"),
    },
    Entry {
        tool: "stylelint",
        family: "",
        extensions: &[".css"],
        ext: "json",
        root: ".stylelintrc.json",
        root_template: "{ \"extends\": [\"./{base}\"] }\n",
        comment: None,
        mode: Mode::Inherit,
        config_rule: true,
        configured: &[".stylelintrc*", "stylelint.config.*"],
        pyproject_table: None,
        standard: include_str!("../templates/config/stylelint/standard.json"),
        strict: include_str!("../templates/config/stylelint/strict.json"),
    },
    Entry {
        tool: "dart",
        family: "dart",
        extensions: &[],
        ext: "yaml",
        root: "analysis_options.yaml",
        root_template: "include: {base}\n",
        comment: Some("#"),
        mode: Mode::Inherit,
        config_rule: true,
        configured: &["analysis_options.yaml"],
        pyproject_table: None,
        standard: include_str!("../templates/config/dart/standard.yaml"),
        strict: include_str!("../templates/config/dart/strict.yaml"),
    },
    Entry {
        tool: "psscriptanalyzer",
        family: "powershell",
        extensions: &[],
        ext: "psd1",
        root: "PSScriptAnalyzerSettings.psd1",
        root_template: "",
        comment: Some("#"),
        mode: Mode::Copy,
        config_rule: true,
        configured: &["PSScriptAnalyzerSettings.psd1"],
        pyproject_table: None,
        standard: include_str!("../templates/config/psscriptanalyzer/standard.psd1"),
        strict: include_str!("../templates/config/psscriptanalyzer/strict.psd1"),
    },
    Entry {
        tool: "hlint",
        family: "haskell",
        extensions: &[],
        ext: "yaml",
        root: ".hlint.yaml",
        root_template: "",
        comment: Some("#"),
        mode: Mode::Copy,
        config_rule: true,
        configured: &[".hlint.yaml"],
        pyproject_table: None,
        standard: include_str!("../templates/config/hlint/standard.yaml"),
        strict: include_str!("../templates/config/hlint/strict.yaml"),
    },
    Entry {
        tool: "dscanner",
        family: "d",
        extensions: &[],
        ext: "ini",
        root: "dscanner.ini",
        root_template: "",
        comment: Some(";"),
        mode: Mode::Copy,
        config_rule: true,
        configured: &["dscanner.ini"],
        pyproject_table: None,
        standard: include_str!("../templates/config/dscanner/standard.ini"),
        strict: include_str!("../templates/config/dscanner/strict.ini"),
    },
];

impl Entry {
    fn base_path(&self, profile: Profile) -> String {
        format!(".dx/config/{}/{}.{}", self.tool, profile.name(), self.ext)
    }

    fn template(&self, profile: Profile) -> &'static str {
        match profile {
            Profile::Standard => self.standard,
            Profile::Strict => self.strict,
        }
    }
}

fn already_configured(root: &Path, entry: &Entry) -> bool {
    let named = |name: &str| match name.strip_suffix('*') {
        Some(prefix) => std::fs::read_dir(root).is_ok_and(|dir| {
            dir.flatten()
                .any(|file| file.file_name().to_string_lossy().starts_with(prefix))
        }),
        None => root.join(name).is_file(),
    };
    if entry.configured.iter().any(|name| named(name)) {
        return true;
    }
    let Some(table) = entry.pyproject_table else {
        return false;
    };
    std::fs::read_to_string(root.join("pyproject.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|parsed| {
            parsed
                .get("tool")
                .and_then(|tool| tool.get(table))
                .is_some()
        })
}

fn base_header(entry: &Entry) -> String {
    match entry.comment {
        Some(prefix) => format!(
            "{prefix} Generated by dx; dx upgrade rewrites this file. Edit {}.\n",
            entry.root
        ),
        None => String::new(),
    }
}

fn root_content(entry: &Entry, profile: Profile) -> String {
    match entry.mode {
        Mode::Inherit => entry
            .root_template
            .replace("{base}", &entry.base_path(profile)),
        Mode::Copy => entry.template(profile).to_owned(),
    }
}

fn detected(entry: &Entry, extensions: &BTreeSet<String>) -> bool {
    if entry.extensions.iter().any(|ext| extensions.contains(*ext)) {
        return true;
    }
    dx_families::families()
        .iter()
        .filter(|family| family.name == entry.family)
        .flat_map(|family| &family.file_extensions)
        .any(|ext| extensions.contains(&ext.to_ascii_lowercase()))
}

fn entries<'a>(extensions: &'a BTreeSet<String>) -> impl Iterator<Item = &'static Entry> + 'a {
    TABLE.iter().filter(|entry| detected(entry, extensions))
}

/// The profile the root files of `root` already pick; `Standard` when they say nothing.
pub fn infer_profile(root: &Path, extensions: &BTreeSet<String>) -> Profile {
    let picks_strict = entries(extensions).any(|entry| {
        let text = std::fs::read_to_string(root.join(entry.root)).unwrap_or_default();
        match entry.mode {
            Mode::Inherit => text.contains(&entry.base_path(Profile::Strict)),
            Mode::Copy => text == entry.template(Profile::Strict),
        }
    });
    if picks_strict {
        Profile::Strict
    } else {
        Profile::Standard
    }
}

/// Both bases and the root file for every detected tool. Without a profile, the root files keep
/// the one they already pick.
pub fn plan_profile_files(
    root: &Path,
    extensions: &BTreeSet<String>,
    profile: Option<Profile>,
) -> Vec<ScaffoldFile> {
    let profile = profile.unwrap_or_else(|| infer_profile(root, extensions));
    let mut files = Vec::new();
    for entry in entries(extensions) {
        for base in [Profile::Standard, Profile::Strict] {
            files.push(ScaffoldFile {
                path: entry.base_path(base),
                content: format!("{}{}", base_header(entry), entry.template(base)),
                kind: FileKind::Generated,
            });
        }
        if entry.tool == "eslint" {
            files.extend(plan_framework_files(entry, extensions));
        }
        if !already_configured(root, entry) {
            files.push(ScaffoldFile {
                path: entry.root.to_owned(),
                content: root_content(entry, profile),
                kind: FileKind::WriteOnce,
            });
        }
    }
    files.extend(plan_config_build(root, extensions, &files));
    files
}

const CONFIG_BUILD: &str = ".dx/config/BUILD.bazel";
const ROOT_BUILD: &str = "BUILD.bazel";

/// The tools whose root file exists or is planned and that have a config rule.
fn config_entries(root: &Path, extensions: &BTreeSet<String>) -> Vec<&'static Entry> {
    let mut found: Vec<&Entry> = entries(extensions)
        .filter(|entry| {
            entry.config_rule
                && (!already_configured(root, entry) || root.join(entry.root).is_file())
        })
        .collect();
    found.sort_by_key(|entry| entry.tool);
    found
}

/// Whether the correctness layer selects the generated workspace policy: its package is planned or exists.
pub fn config_policy_wanted(root: &Path) -> bool {
    let extensions = crate::families::file_extensions(root);
    root.join(CONFIG_BUILD).is_file() || !config_entries(root, &extensions).is_empty()
}

fn bazel_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("        \"{item}\",\n"))
        .collect()
}

/// `.dx/config/BUILD.bazel` with a `<tool>_config` target for each tool and the policy that lists them,
/// and a root `BUILD.bazel` that exports the root files when the root has no BUILD file.
fn plan_config_build(
    root: &Path,
    extensions: &BTreeSet<String>,
    planned: &[ScaffoldFile],
) -> Vec<ScaffoldFile> {
    let found = config_entries(root, extensions);
    if found.is_empty() {
        return Vec::new();
    }
    let repo = dx_module_bazel::rules_dx_repo(root);
    let symbols: Vec<String> = found
        .iter()
        .map(|entry| format!("\"{}_config\"", entry.tool))
        .collect();
    let mut text = format!(
        "# Generated by dx; dx upgrade rewrites this file.\n\n\
         load(\"@{repo}//quality:native_config.bzl\", {})\n\
         load(\"@{repo}//quality:policy.bzl\", \"workspace_policy\")\n\n\
         package(default_visibility = [\"//visibility:public\"])\n",
        symbols.join(", ")
    );
    for entry in &found {
        let mut data: Vec<String> = planned
            .iter()
            .filter_map(|file| file.path.strip_prefix(".dx/config/"))
            .filter(|path| path.starts_with(&format!("{}/", entry.tool)))
            .map(str::to_owned)
            .collect();
        data.sort();
        data.dedup();
        text.push_str(&format!(
            "\n{}_config(\n    name = \"{}\",\n    src = \"//:{}\",\n    data = [\n{}    ],\n)\n",
            entry.tool,
            entry.tool,
            entry.root,
            bazel_list(&data)
        ));
    }
    let targets: Vec<String> = found
        .iter()
        .map(|entry| format!(":{}", entry.tool))
        .collect();
    text.push_str(&format!(
        "\nworkspace_policy(\n    name = \"policy\",\n    configs = [\n{}    ],\n    extends = \"@{repo}//quality:default_workspace_policy\",\n)\n",
        bazel_list(&targets)
    ));
    let mut files = vec![ScaffoldFile {
        path: CONFIG_BUILD.to_owned(),
        content: text,
        kind: FileKind::Generated,
    }];
    if !root.join("BUILD").exists() && !root.join(ROOT_BUILD).exists() {
        let roots: Vec<String> = found.iter().map(|entry| entry.root.to_owned()).collect();
        files.push(ScaffoldFile {
            path: ROOT_BUILD.to_owned(),
            content: format!(
                "# Written once by dx.\n\nexports_files([\n{}])\n",
                bazel_list(&roots).replace("        ", "    ")
            ),
            kind: FileKind::WriteOnce,
        });
    }
    files
}

/// The framework files for the extensions found and the `frameworks.mjs` that imports them.
fn plan_framework_files(entry: &Entry, extensions: &BTreeSet<String>) -> Vec<ScaffoldFile> {
    let found: Vec<&Framework> = FRAMEWORKS
        .iter()
        .filter(|framework| {
            framework
                .extensions
                .iter()
                .any(|ext| extensions.contains(*ext))
        })
        .collect();
    let mut files: Vec<ScaffoldFile> = found
        .iter()
        .map(|framework| ScaffoldFile {
            path: format!(".dx/config/eslint/frameworks/{}.mjs", framework.name),
            content: format!("{}{}", base_header(entry), framework.template),
            kind: FileKind::Generated,
        })
        .collect();
    let body = if found.is_empty() {
        NO_FRAMEWORKS.to_owned()
    } else {
        let imports: String = found
            .iter()
            .map(|framework| {
                format!(
                    "import {0} from \"./frameworks/{0}.mjs\";\n",
                    framework.name
                )
            })
            .collect();
        let spread: String = found
            .iter()
            .map(|framework| format!("  ...{},\n", framework.name))
            .collect();
        format!("{imports}\nexport default [\n{spread}];\n")
    };
    files.push(ScaffoldFile {
        path: ".dx/config/eslint/frameworks.mjs".to_owned(),
        content: format!("{}{body}", base_header(entry)),
        kind: FileKind::Generated,
    });
    files
}

fn is_ours(root: &Path, entry: &Entry) -> bool {
    std::fs::read_to_string(root.join(entry.root))
        .is_ok_and(|text| text.contains(&format!(".dx/config/{}/", entry.tool)))
}

/// One line for each tool whose own config was kept, so the root file is not planned.
pub fn kept_config_notes(root: &Path) -> Vec<String> {
    let extensions = crate::families::file_extensions(root);
    entries(&extensions)
        .filter(|entry| already_configured(root, entry) && !is_ours(root, entry))
        .map(|entry| {
            format!(
                "{}: existing config kept; bases are in .dx/config/{}/",
                entry.root, entry.tool
            )
        })
        .collect()
}

/// One line for each root file that exists and does not pick `profile`.
pub fn profile_notes(root: &Path, profile: Profile) -> Vec<String> {
    let extensions = crate::families::file_extensions(root);
    entries(&extensions)
        .filter_map(|entry| {
            if !is_ours(root, entry) {
                return None;
            }
            let existing = std::fs::read_to_string(root.join(entry.root)).ok()?;
            let wanted = root_content(entry, profile);
            if existing == wanted {
                return None;
            }
            let change = match entry.mode {
                Mode::Inherit => format!("point it at {}", entry.base_path(profile)),
                Mode::Copy => format!("copy {}", entry.base_path(profile)),
            };
            Some(format!(
                "{} is left as is for --profile {}: {change}",
                entry.root,
                profile.name()
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn without_build(mut files: Vec<ScaffoldFile>) -> Vec<ScaffoldFile> {
        files.retain(|file| !file.path.ends_with("BUILD.bazel"));
        files
    }

    fn exts(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|ext| (*ext).to_owned()).collect()
    }

    #[test]
    fn strict_on_dart_and_powershell_writes_include_and_copy_roots() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(
            scratch.path(),
            &exts(&[".dart", ".ps1"]),
            Some(Profile::Strict),
        );
        let find = |path: &str| files.iter().find(|file| file.path == path).expect(path);
        assert_eq!(
            find("analysis_options.yaml").content,
            "include: .dx/config/dart/strict.yaml\n"
        );
        assert_eq!(
            find("PSScriptAnalyzerSettings.psd1").content,
            include_str!("../templates/config/psscriptanalyzer/strict.psd1")
        );
        find(".dx/config/dart/standard.yaml");
        find(".dx/config/psscriptanalyzer/strict.psd1");
    }

    #[test]
    fn vue_fixture_writes_the_vue_block_and_imports_only_it() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(scratch.path(), &exts(&[".ts", ".vue"]), None);
        let find = |path: &str| files.iter().find(|file| file.path == path);
        let vue = find(".dx/config/eslint/frameworks/vue.mjs").expect("vue block");
        assert_eq!(vue.kind, FileKind::Generated);
        assert!(vue.content.contains("eslint-plugin-vue"));
        let index = find(".dx/config/eslint/frameworks.mjs").expect("frameworks.mjs");
        assert_eq!(index.kind, FileKind::Generated);
        assert!(index
            .content
            .contains("import vue from \"./frameworks/vue.mjs\";"));
        assert!(index.content.contains("  ...vue,\n"));
        assert_eq!(index.content.matches("import ").count(), 1);
        assert!(find(".dx/config/eslint/frameworks/react.mjs").is_none());
    }

    #[test]
    fn react_covers_jsx_and_tsx() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(scratch.path(), &exts(&[".tsx"]), None);
        assert!(files
            .iter()
            .any(|file| file.path == ".dx/config/eslint/frameworks/react.mjs"));
    }

    #[test]
    fn typescript_only_fixture_gets_the_empty_frameworks_file() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(scratch.path(), &exts(&[".ts"]), None);
        assert!(!files
            .iter()
            .any(|file| file.path.starts_with(".dx/config/eslint/frameworks/")));
        let index = files
            .iter()
            .find(|file| file.path == ".dx/config/eslint/frameworks.mjs")
            .expect("frameworks.mjs");
        assert!(index.content.ends_with("export default [];\n"));
    }

    #[test]
    fn strict_on_python_and_go_writes_ruff_files_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = without_build(plan_profile_files(
            scratch.path(),
            &exts(&[".py", ".go"]),
            Some(Profile::Strict),
        ));
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".dx/config/ruff/standard.toml",
                ".dx/config/ruff/strict.toml",
                "ruff.toml"
            ]
        );
        let root = files.last().expect("root file");
        assert_eq!(root.kind, FileKind::WriteOnce);
        assert!(root
            .content
            .ends_with("extend = \".dx/config/ruff/strict.toml\"\n"));
        assert_eq!(root.content.lines().count(), 2);
        assert!(files[..2]
            .iter()
            .all(|file| file.kind == FileKind::Generated));
        assert!(files[0].content.starts_with("# Generated by dx;"));
    }

    #[test]
    fn strict_on_css_writes_stylelint_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = without_build(plan_profile_files(
            scratch.path(),
            &exts(&[".css"]),
            Some(Profile::Strict),
        ));
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".dx/config/stylelint/standard.json",
                ".dx/config/stylelint/strict.json",
                ".stylelintrc.json"
            ]
        );
        assert_eq!(
            files[2].content,
            "{ \"extends\": [\"./.dx/config/stylelint/strict.json\"] }\n"
        );
        assert_eq!(infer_profile_of(&files), Profile::Strict);
    }

    #[test]
    fn strict_on_typescript_writes_eslint_and_tsconfig_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = without_build(plan_profile_files(
            scratch.path(),
            &exts(&[".ts"]),
            Some(Profile::Strict),
        ));
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".dx/config/eslint/standard.mjs",
                ".dx/config/eslint/strict.mjs",
                ".dx/config/eslint/frameworks.mjs",
                "eslint.config.mjs",
                ".dx/config/tsconfig/standard.json",
                ".dx/config/tsconfig/strict.json",
                "tsconfig.json"
            ]
        );
        assert_eq!(
            files[3].content,
            "export { default } from \"./.dx/config/eslint/strict.mjs\";\n"
        );
        assert_eq!(
            files[6].content,
            "{ \"extends\": \"./.dx/config/tsconfig/strict.json\" }\n"
        );
        assert!(files[0].content.starts_with("// Generated by dx;"));
        assert!(files[5].content.starts_with('{'));
        assert!(files[5]
            .content
            .contains("\"extends\": \"./standard.json\""));
        assert_eq!(infer_profile_of(&files), Profile::Strict);
    }

    fn infer_profile_of(files: &[ScaffoldFile]) -> Profile {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        for file in files.iter().filter(|file| !file.path.starts_with(".dx/")) {
            std::fs::write(scratch.path().join(&file.path), &file.content).expect("write");
        }
        infer_profile(scratch.path(), &exts(&[".ts", ".css"]))
    }

    #[test]
    fn no_python_writes_nothing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        assert!(plan_profile_files(scratch.path(), &exts(&[".go"]), None).is_empty());
    }

    #[test]
    fn rerun_with_another_profile_notes_and_keeps_the_root_file() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        std::fs::write(scratch.path().join("main.py"), "").expect("python source");
        let python = exts(&[".py"]);
        let strict = without_build(plan_profile_files(
            scratch.path(),
            &python,
            Some(Profile::Strict),
        ));
        let root_file = strict.last().expect("root file");
        std::fs::write(scratch.path().join("ruff.toml"), &root_file.content).expect("write");
        let notes = profile_notes(scratch.path(), Profile::Standard);
        assert_eq!(
            notes,
            ["ruff.toml is left as is for --profile standard: point it at .dx/config/ruff/standard.toml"]
        );
        assert!(profile_notes(scratch.path(), Profile::Strict).is_empty());
        assert_eq!(infer_profile(scratch.path(), &python), Profile::Strict);
        let again = without_build(plan_profile_files(scratch.path(), &python, None));
        assert_eq!(again.len(), 2);
        assert!(again.iter().all(|file| file.kind == FileKind::Generated));
    }

    fn mixed_repo() -> dx_test_scratch::TempDir {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        for name in ["main.py", "app.ts", "main.go"] {
            std::fs::write(scratch.path().join(name), "").expect("source");
        }
        scratch
    }

    #[test]
    fn python_ts_go_gets_seven_bases_and_three_root_files() {
        let scratch = mixed_repo();
        let root = scratch.path();
        let extensions = exts(&[".py", ".ts", ".go"]);
        let strict = without_build(plan_profile_files(root, &extensions, Some(Profile::Strict)));
        let bases = strict.iter().filter(|f| f.path.starts_with(".dx/config/"));
        let roots: Vec<&str> = strict
            .iter()
            .filter(|f| f.kind == FileKind::WriteOnce)
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(bases.count(), 7);
        assert_eq!(roots, ["ruff.toml", "eslint.config.mjs", "tsconfig.json"]);
        assert!(strict.iter().all(|f| !f.path.contains("go")));
        let mut applied = crate::scaffold::Applied::default();
        crate::scaffold::apply_scaffold(root, &strict, &mut applied).expect("apply");
        let standard = without_build(plan_profile_files(
            root,
            &extensions,
            Some(Profile::Standard),
        ));
        assert!(standard.iter().all(|f| f.kind == FileKind::Generated));
        assert_eq!(standard.len(), 7);
        assert_eq!(profile_notes(root, Profile::Standard).len(), 3);
        assert!(profile_notes(root, Profile::Strict).is_empty());
    }

    #[test]
    fn pyproject_ruff_table_keeps_its_config_and_plans_only_bases() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let root = scratch.path();
        std::fs::write(
            root.join("pyproject.toml"),
            "[tool.ruff]\nline-length = 100\n",
        )
        .expect("toml");
        let files = plan_profile_files(root, &exts(&[".py"]), None);
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".dx/config/ruff/standard.toml",
                ".dx/config/ruff/strict.toml"
            ]
        );
        assert_eq!(
            kept_config_notes(root),
            Vec::<String>::new(),
            "no python source, so no tool is detected"
        );
        std::fs::write(root.join("main.py"), "").expect("python source");
        assert_eq!(
            kept_config_notes(root),
            ["ruff.toml: existing config kept; bases are in .dx/config/ruff/"]
        );
    }

    #[test]
    fn eslintrc_and_plain_pyproject_count_correctly() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let root = scratch.path();
        std::fs::write(root.join("pyproject.toml"), "[project]\nname = \"a\"\n").expect("toml");
        std::fs::write(root.join(".eslintrc.json"), "{}").expect("eslintrc");
        let configured: Vec<&str> = TABLE
            .iter()
            .filter(|entry| already_configured(root, entry))
            .map(|entry| entry.tool)
            .collect();
        assert_eq!(configured, ["eslint"]);
    }

    #[test]
    fn refresh_rewrites_an_edited_base_and_leaves_the_root_file() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let root = scratch.path();
        let extensions = exts(&[".py"]);
        let files = plan_profile_files(root, &extensions, Some(Profile::Strict));
        let mut applied = crate::scaffold::Applied::default();
        crate::scaffold::apply_scaffold(root, &files, &mut applied).expect("apply");
        let base = root.join(".dx/config/ruff/strict.toml");
        let root_file = root.join("ruff.toml");
        std::fs::write(&base, "edited = true\n").expect("edit base");
        std::fs::write(&root_file, "# mine\n").expect("edit root");
        let mut again = crate::scaffold::Applied::default();
        crate::scaffold::refresh_scaffold(root, &files, &mut again).expect("refresh");
        assert!(std::fs::read_to_string(&base)
            .expect("base")
            .starts_with("# Generated by dx;"));
        assert_eq!(
            std::fs::read_to_string(&root_file).expect("root"),
            "# mine\n"
        );
    }

    #[test]
    fn every_row_has_both_templates_that_parse() {
        for entry in TABLE {
            for profile in [Profile::Standard, Profile::Strict] {
                let text = entry.template(profile);
                assert!(!text.is_empty(), "{} {}", entry.tool, profile.name());
                match entry.ext {
                    "toml" => {
                        text.parse::<toml::Table>().expect("toml base");
                    }
                    "json" => {
                        serde_json::from_str::<serde_json::Value>(text).expect("json base");
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn copy_mode_copies_the_chosen_base_to_the_root_path() {
        let entry = Entry {
            tool: "demo",
            family: "python",
            extensions: &[],
            ext: "toml",
            root: "demo.toml",
            root_template: "",
            comment: Some("#"),
            mode: Mode::Copy,
            config_rule: false,
            configured: &[],
            pyproject_table: None,
            standard: "a = 1\n",
            strict: "a = 2\n",
        };
        assert_eq!(root_content(&entry, Profile::Strict), "a = 2\n");
        assert_eq!(
            entry.base_path(Profile::Strict),
            ".dx/config/demo/strict.toml"
        );
    }

    #[test]
    fn python_tree_gets_a_config_build_and_a_root_export() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(scratch.path(), &exts(&[".py"]), None);
        let find = |path: &str| files.iter().find(|file| file.path == path).expect(path);
        let build = find(".dx/config/BUILD.bazel");
        assert_eq!(build.kind, FileKind::Generated);
        assert!(build
            .content
            .contains("ruff_config(\n    name = \"ruff\",\n    src = \"//:ruff.toml\","));
        assert!(build.content.contains("\"ruff/strict.toml\""));
        assert!(build.content.contains("\":ruff\""));
        let root = find("BUILD.bazel");
        assert_eq!(root.kind, FileKind::WriteOnce);
        assert!(root
            .content
            .contains("exports_files([\n    \"ruff.toml\",\n])"));
    }

    #[test]
    fn an_existing_root_build_file_is_left_alone() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        std::fs::write(scratch.path().join("BUILD"), "").expect("build");
        let files = plan_profile_files(scratch.path(), &exts(&[".py"]), None);
        assert!(files
            .iter()
            .any(|file| file.path == ".dx/config/BUILD.bazel"));
        assert!(!files.iter().any(|file| file.path == "BUILD.bazel"));
    }

    #[test]
    fn pyproject_config_gets_no_target() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        std::fs::write(scratch.path().join("pyproject.toml"), "[tool.ruff]\n").expect("pyproject");
        let files = plan_profile_files(scratch.path(), &exts(&[".py"]), None);
        assert!(!files.iter().any(|file| file.path.ends_with("BUILD.bazel")));
        assert!(!config_policy_wanted(scratch.path()));
    }
}
