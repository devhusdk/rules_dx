//! The read-only "init plan" for a foreign `MODULE.bazel` (#3751): what adopting rules_dx
//! would raise or conflict with, found from the declared files alone. No Bazel runs and
//! nothing is written, so the result is a lower bound: transitive raises are not seen.

use std::path::Path;

use dx_module_bazel::{compare_module_versions, ModuleFile, Value};

use crate::overrides::{OVERRIDES, OVERRIDES_INCLUDE};

/// rules_dx's own non-dev `bazel_dep` versions, the floors a consumer's graph is raised to.
/// The drift test keeps this equal to `//:MODULE.bazel`.
pub const RULES_DX_DEPS: &[(&str, &str)] = &[
    ("rules_rust", "0.74.0"),
    ("rules_rust_wasm_bindgen", "0.74.0"),
    ("rules_cc", "0.2.27"),
    ("llvm", "0.8.25"),
    ("rules_cuda", "0.3.0"),
    ("rules_python", "1.9.0"),
    ("rules_go", "0.64.2"),
    ("gazelle", "0.54.0"),
    ("rules_shell", "0.8.0"),
    ("apple_support", "2.9.1"),
    ("rules_apple", "5.2.0"),
    ("platforms", "1.1.0"),
    ("bazel_skylib", "1.9.2"),
    ("rules_proto", "7.1.0"),
    ("protobuf", "34.0.bcr.1"),
    ("rules_rust_prost", "0.74.0"),
    ("rules_java", "9.9.0"),
    ("rules_android", "0.8.0"),
    ("rules_kotlin", "2.4.10"),
    ("rules_scala", "7.3.0"),
    ("rules_dotnet", "0.22.2"),
    ("bazel_lib", "3.7.2"),
    ("rules_jvm_external", "7.1"),
    ("rules_powershell", "0.3.1"),
    ("rules_ruby", "0.29.0"),
    ("rules_zig", "0.14.0"),
    ("rules_julia", "0.8.0"),
    ("rules_perl", "1.5.0"),
    ("lua", "5.4.6.bcr.1"),
    ("aspect_rules_py", "2.0.0-alpha.6"),
    ("aspect_rules_js", "3.5.2"),
    ("aspect_rules_ts", "3.10.0"),
    ("aspect_rules_jest", "0.26.0"),
    ("rules_nodejs", "6.7.7"),
];

/// What kind of finding it is; one plan section each.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    VersionRaise,
    OverrideConflict,
}

/// One planned change or conflict, naming the file and line that causes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub kind: Kind,
    pub subject: String,
    pub before: String,
    pub after: String,
    pub file: String,
    pub line: usize,
    pub hint: String,
    /// A hard finding makes `dx init` fail its check; advisory ones are only reported.
    pub hard: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Preflight {
    /// False when there is no plan to show (no `MODULE.bazel`, or the rules_dx workspace).
    pub applicable: bool,
    pub findings: Vec<Finding>,
    pub notes: Vec<String>,
}

impl Preflight {
    pub fn hard(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| f.hard)
    }
}

struct Source<'a> {
    file: String,
    module: &'a ModuleFile,
}

/// Plans against the `MODULE.bazel` under `root`, using `snippet` as what init would add.
pub fn preflight(root: &Path, snippet: &str) -> Preflight {
    let mut result = Preflight::default();
    let Ok(text) = std::fs::read_to_string(root.join("MODULE.bazel")) else {
        return result;
    };
    let read_include = |label: &str| -> Result<String, String> {
        if label == OVERRIDES_INCLUDE {
            return Ok(String::new());
        }
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| format!("cannot read included {label}: {e}"))
    };
    let loaded = match dx_module_bazel::load_with_includes_labeled(&text, &read_include) {
        Ok(loaded) => loaded,
        Err(detail) => {
            result.applicable = true;
            result
                .notes
                .push(format!("MODULE.bazel not compared: {detail}"));
            return result;
        }
    };
    let mut sources = Vec::new();
    for (label, module) in &loaded {
        let file = match label {
            None => "MODULE.bazel".to_owned(),
            Some(label) => {
                dx_module_bazel::include_rel("", label).unwrap_or_else(|_| label.clone())
            }
        };
        sources.push(Source { file, module });
    }
    if sources.iter().any(|s| {
        s.module
            .calls_named("module")
            .any(|c| c.str("name") == Some("rules_dx"))
    }) {
        return result;
    }
    result.applicable = true;
    let snippet_module = ModuleFile::parse(snippet).ok();
    result
        .findings
        .extend(version_raises(&sources, snippet_module.as_ref()));
    result.findings.extend(override_conflicts(&sources));
    result
}

fn floors(snippet: Option<&ModuleFile>) -> Vec<(String, String)> {
    let mut floors: Vec<(String, String)> = RULES_DX_DEPS
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect();
    for call in snippet.iter().flat_map(|m| m.calls_named("bazel_dep")) {
        let (Some(name), Some(version)) = (call.str("name"), call.str("version")) else {
            continue;
        };
        match floors.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) if compare_module_versions(version, v).is_gt() => *v = version.to_owned(),
            Some(_) => {}
            None => floors.push((name.to_owned(), version.to_owned())),
        }
    }
    floors
}

fn version_raises(sources: &[Source], snippet: Option<&ModuleFile>) -> Vec<Finding> {
    let floors = floors(snippet);
    let mut out = Vec::new();
    for source in sources {
        for call in source.module.calls_named("bazel_dep") {
            let (Some(name), Some(have)) = (call.str("name"), call.str("version")) else {
                continue;
            };
            if name == "rules_dx" {
                continue;
            }
            let Some((_, floor)) = floors.iter().find(|(n, _)| n == name) else {
                continue;
            };
            if compare_module_versions(have, floor).is_lt() {
                out.push(Finding {
                    kind: Kind::VersionRaise,
                    subject: name.to_owned(),
                    before: have.to_owned(),
                    after: floor.clone(),
                    file: source.file.clone(),
                    line: call.line,
                    hint: format!("rules_dx needs {name} {floor}; Bazel will select it"),
                    hard: false,
                });
            }
        }
    }
    out
}

fn override_conflicts(sources: &[Source]) -> Vec<Finding> {
    let mut out = Vec::new();
    for source in sources {
        for call in source.module.calls() {
            if !call.name.ends_with("_override") {
                continue;
            }
            let Some(target) = call.str("module_name") else {
                continue;
            };
            let owned = OVERRIDES.iter().any(|row| row.module == target);
            let floor = RULES_DX_DEPS.iter().find(|(n, _)| *n == target);
            if !owned && floor.is_none() {
                continue;
            }
            let before = match call.arg("version") {
                Some(Value::Str(v)) => format!("{} {}", call.name, v.value),
                _ => call.name.clone(),
            };
            let (after, hint, hard) = if owned {
                (
                    "dx patched override".to_owned(),
                    "dx cannot add its patches while you own this override; drop yours or port the patches"
                        .to_owned(),
                    true,
                )
            } else {
                (
                    format!("rules_dx floor {}", floor.map_or("", |(_, v)| v)),
                    "your override wins in the root module; check it satisfies rules_dx".to_owned(),
                    false,
                )
            };
            out.push(Finding {
                kind: Kind::OverrideConflict,
                subject: target.to_owned(),
                before,
                after,
                file: source.file.clone(),
                line: call.line,
                hint,
                hard,
            });
        }
    }
    out
}

/// The plan as text: one section per kind, `none` when empty, then the next step.
pub fn render_plan_text(module: &str, plan: &Preflight) -> String {
    let mut out = format!("init plan: {module}\n");
    for (title, kind) in [
        ("version raises", Kind::VersionRaise),
        ("override conflicts", Kind::OverrideConflict),
    ] {
        out.push_str(&format!("{title}:\n"));
        let items: Vec<&Finding> = plan.findings.iter().filter(|f| f.kind == kind).collect();
        if items.is_empty() {
            out.push_str("  none\n");
        }
        for f in items {
            let level = if f.hard { "hard" } else { "advisory" };
            out.push_str(&format!(
                "  {} {} -> {} ({level}, {}:{})\n    {}\n",
                f.subject, f.before, f.after, f.file, f.line, f.hint
            ));
        }
    }
    for note in &plan.notes {
        out.push_str(&format!("note: {note}\n"));
    }
    out.push_str("note: only declared deps were compared; the resolved graph was not\n");
    let hard = plan.hard().count();
    if hard > 0 {
        out.push_str(&format!(
            "next: resolve {hard} hard conflict(s), then run `dx init {module} --apply`\n"
        ));
    } else {
        out.push_str(&format!("next: run `dx init {module} --apply`\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(files: &[(&str, &str)]) -> dx_test_scratch::TempDir {
        let dir = dx_test_scratch::scratch("dx-preflight-");
        for (path, content) in files {
            let dest = dir.path().join(path);
            std::fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            std::fs::write(dest, content).expect("write");
        }
        dir
    }

    fn snippet() -> String {
        crate::render_consumer_module_setup()
    }

    #[test]
    fn deps_match_the_repo_module_bazel() {
        let text = crate::version::repo_text("MODULE.bazel");
        let module = ModuleFile::parse(&text).expect("MODULE.bazel parses");
        let mut expected: Vec<(String, String)> = Vec::new();
        for call in module.calls_named("bazel_dep") {
            if matches!(call.arg("dev_dependency"), Some(Value::Ident(v)) if v == "True") {
                continue;
            }
            expected.push((
                call.str("name").expect("name").to_owned(),
                call.str("version").expect("version").to_owned(),
            ));
        }
        let actual: Vec<(String, String)> = RULES_DX_DEPS
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect();
        let rows: String = expected
            .iter()
            .map(|(n, v)| format!("    (\"{n}\", \"{v}\"),\n"))
            .collect();
        assert_eq!(actual, expected, "update RULES_DX_DEPS to:\n{rows}");
    }

    #[test]
    fn no_module_is_not_applicable() {
        let dir = scratch(&[]);
        assert!(!preflight(dir.path(), &snippet()).applicable);
    }

    #[test]
    fn clean_module_has_no_findings() {
        let dir = scratch(&[("MODULE.bazel", "module(name = \"app\")\n")]);
        let plan = preflight(dir.path(), &snippet());
        assert!(plan.applicable && plan.findings.is_empty());
        assert!(render_plan_text("app", &plan).contains("version raises:\n  none\n"));
    }

    #[test]
    fn older_dep_is_raised_and_newer_is_not() {
        let dir = scratch(&[(
            "MODULE.bazel",
            "bazel_dep(name = \"rules_rust\", version = \"0.60.0\")\nbazel_dep(name = \"rules_cc\", version = \"99.0.0\")\nbazel_dep(name = \"protobuf\", version = \"34.0.bcr.2\")\n",
        )]);
        let plan = preflight(dir.path(), &snippet());
        assert_eq!(plan.findings.len(), 1, "{plan:?}");
        let f = &plan.findings[0];
        assert_eq!(
            (f.subject.as_str(), f.before.as_str(), f.line),
            ("rules_rust", "0.60.0", 1)
        );
        assert!(!f.hard);
        assert!(render_plan_text("app", &plan).contains("rules_rust 0.60.0 -> 0.74.0"));
    }

    #[test]
    fn owned_override_is_hard_and_names_the_included_file() {
        let dir = scratch(&[
            ("MODULE.bazel", "include(\"//tools:mine.MODULE.bazel\")\n"),
            (
                "tools/mine.MODULE.bazel",
                "\nsingle_version_override(module_name = \"rules_rust\", version = \"0.74.0\")\n",
            ),
        ]);
        let plan = preflight(dir.path(), &snippet());
        let f = plan.hard().next().expect("hard finding");
        assert_eq!((f.file.as_str(), f.line), ("tools/mine.MODULE.bazel", 2));
    }

    #[test]
    fn other_override_is_advisory() {
        let dir = scratch(&[(
            "MODULE.bazel",
            "git_override(module_name = \"rules_go\", remote = \"x\", commit = \"y\")\n",
        )]);
        let plan = preflight(dir.path(), &snippet());
        assert_eq!(plan.findings.len(), 1);
        assert!(!plan.findings[0].hard);
    }

    #[test]
    fn dx_own_include_is_not_a_conflict() {
        let dir = scratch(&[(
            "MODULE.bazel",
            &format!("include(\"{OVERRIDES_INCLUDE}\")\n"),
        )]);
        assert!(preflight(dir.path(), &snippet()).findings.is_empty());
    }

    #[test]
    fn rules_dx_workspace_is_not_applicable() {
        let dir = scratch(&[("MODULE.bazel", "module(name = \"rules_dx\")\n")]);
        assert!(!preflight(dir.path(), &snippet()).applicable);
    }

    #[test]
    fn preflight_never_writes() {
        let dir = scratch(&[(
            "MODULE.bazel",
            "bazel_dep(name = \"rules_rust\", version = \"0.1.0\")\n",
        )]);
        let path = dir.path().join("MODULE.bazel");
        let before = (
            std::fs::read(&path).expect("read"),
            std::fs::metadata(&path)
                .expect("meta")
                .modified()
                .expect("mtime"),
        );
        let _ = preflight(dir.path(), &snippet());
        let after = (
            std::fs::read(&path).expect("read"),
            std::fs::metadata(&path)
                .expect("meta")
                .modified()
                .expect("mtime"),
        );
        assert_eq!(before, after);
        assert_eq!(std::fs::read_dir(dir.path()).expect("dir").count(), 1);
    }
}
