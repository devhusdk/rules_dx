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
    ext: &'static str,
    root: &'static str,
    /// The root file of an inherit tool, with `{base}` for the chosen base path.
    root_template: &'static str,
    /// Prefix of the comment line in a generated base; `None` for formats without comments.
    comment: Option<&'static str>,
    mode: Mode,
    /// Root files that mean the repo already configures the tool; a trailing `*` matches a prefix.
    configured: &'static [&'static str],
    /// Table in `pyproject.toml` that also means the tool is configured.
    pyproject_table: Option<&'static str>,
    standard: &'static str,
    strict: &'static str,
}

const TABLE: &[Entry] = &[
    Entry {
        tool: "ruff",
        family: "python",
        ext: "toml",
        root: "ruff.toml",
        root_template:
            "# Repo settings go here; the extend line picks the profile.\nextend = \"{base}\"\n",
        comment: Some("#"),
        mode: Mode::Inherit,
        configured: &["ruff.toml", ".ruff.toml"],
        pyproject_table: Some("ruff"),
        standard: include_str!("../templates/config/ruff/standard.toml"),
        strict: include_str!("../templates/config/ruff/strict.toml"),
    },
    Entry {
        tool: "eslint",
        family: "js",
        ext: "mjs",
        root: "eslint.config.mjs",
        root_template: "export { default } from \"./{base}\";\n",
        comment: Some("//"),
        mode: Mode::Inherit,
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
        ext: "json",
        root: "tsconfig.json",
        root_template: "{ \"extends\": \"./{base}\" }\n",
        comment: None,
        mode: Mode::Inherit,
        configured: &["tsconfig.json"],
        pyproject_table: None,
        standard: include_str!("../templates/config/tsconfig/standard.json"),
        strict: include_str!("../templates/config/tsconfig/strict.json"),
    },
    Entry {
        tool: "dart",
        family: "dart",
        ext: "yaml",
        root: "analysis_options.yaml",
        root_template: "include: {base}\n",
        comment: Some("#"),
        mode: Mode::Inherit,
        configured: &["analysis_options.yaml"],
        pyproject_table: None,
        standard: include_str!("../templates/config/dart/standard.yaml"),
        strict: include_str!("../templates/config/dart/strict.yaml"),
    },
    Entry {
        tool: "psscriptanalyzer",
        family: "powershell",
        ext: "psd1",
        root: "PSScriptAnalyzerSettings.psd1",
        root_template: "",
        comment: Some("#"),
        mode: Mode::Copy,
        configured: &["PSScriptAnalyzerSettings.psd1"],
        pyproject_table: None,
        standard: include_str!("../templates/config/psscriptanalyzer/standard.psd1"),
        strict: include_str!("../templates/config/psscriptanalyzer/strict.psd1"),
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
        if !already_configured(root, entry) {
            files.push(ScaffoldFile {
                path: entry.root.to_owned(),
                content: root_content(entry, profile),
                kind: FileKind::WriteOnce,
            });
        }
    }
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
    fn strict_on_python_and_go_writes_ruff_files_only() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(
            scratch.path(),
            &exts(&[".py", ".go"]),
            Some(Profile::Strict),
        );
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
    fn strict_on_typescript_writes_eslint_and_tsconfig_files() {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        let files = plan_profile_files(scratch.path(), &exts(&[".ts"]), Some(Profile::Strict));
        let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".dx/config/eslint/standard.mjs",
                ".dx/config/eslint/strict.mjs",
                "eslint.config.mjs",
                ".dx/config/tsconfig/standard.json",
                ".dx/config/tsconfig/strict.json",
                "tsconfig.json"
            ]
        );
        assert_eq!(
            files[2].content,
            "export { default } from \"./.dx/config/eslint/strict.mjs\";\n"
        );
        assert_eq!(
            files[5].content,
            "{ \"extends\": \"./.dx/config/tsconfig/strict.json\" }\n"
        );
        assert!(files[0].content.starts_with("// Generated by dx;"));
        assert!(files[3].content.starts_with('{'));
        assert!(files[4]
            .content
            .contains("\"extends\": \"./standard.json\""));
        assert_eq!(infer_profile_of(&files), Profile::Strict);
    }

    fn infer_profile_of(files: &[ScaffoldFile]) -> Profile {
        let scratch = dx_test_scratch::scratch("dx-adopt-profiles-");
        for file in files.iter().filter(|file| !file.path.starts_with(".dx/")) {
            std::fs::write(scratch.path().join(&file.path), &file.content).expect("write");
        }
        infer_profile(scratch.path(), &exts(&[".ts"]))
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
        let strict = plan_profile_files(scratch.path(), &python, Some(Profile::Strict));
        let root_file = strict.last().expect("root file");
        std::fs::write(scratch.path().join("ruff.toml"), &root_file.content).expect("write");
        let notes = profile_notes(scratch.path(), Profile::Standard);
        assert_eq!(
            notes,
            ["ruff.toml is left as is for --profile standard: point it at .dx/config/ruff/standard.toml"]
        );
        assert!(profile_notes(scratch.path(), Profile::Strict).is_empty());
        assert_eq!(infer_profile(scratch.path(), &python), Profile::Strict);
        let again = plan_profile_files(scratch.path(), &python, None);
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
    fn python_ts_go_gets_six_bases_and_three_root_files() {
        let scratch = mixed_repo();
        let root = scratch.path();
        let extensions = exts(&[".py", ".ts", ".go"]);
        let strict = plan_profile_files(root, &extensions, Some(Profile::Strict));
        let bases = strict.iter().filter(|f| f.path.starts_with(".dx/config/"));
        let roots: Vec<&str> = strict
            .iter()
            .filter(|f| f.kind == FileKind::WriteOnce)
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(bases.count(), 6);
        assert_eq!(roots, ["ruff.toml", "eslint.config.mjs", "tsconfig.json"]);
        assert!(strict.iter().all(|f| !f.path.contains("go")));
        let mut applied = crate::scaffold::Applied::default();
        crate::scaffold::apply_scaffold(root, &strict, &mut applied).expect("apply");
        let standard = plan_profile_files(root, &extensions, Some(Profile::Standard));
        assert!(standard.iter().all(|f| f.kind == FileKind::Generated));
        assert_eq!(standard.len(), 6);
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
            ext: "toml",
            root: "demo.toml",
            root_template: "",
            comment: Some("#"),
            mode: Mode::Copy,
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
}
