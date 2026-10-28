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
    ToolchainConflict,
    BazelrcConflict,
}

impl Kind {
    /// Stable id for machine output.
    pub fn id(self) -> &'static str {
        match self {
            Kind::VersionRaise => "version_raise",
            Kind::OverrideConflict => "override_conflict",
            Kind::ToolchainConflict => "toolchain_conflict",
            Kind::BazelrcConflict => "bazelrc_conflict",
        }
    }
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
        .findings
        .extend(toolchain_conflicts(&sources, snippet_module.as_ref()));
    result.findings.extend(bazelrc_conflicts(root));
    result
}

/// Flag lines in the root `.bazelrc` (and files it imports from `%workspace%`) that repeat a
/// line the dx preset layers own. Advisory: `dx init` will not import the layers while the
/// root file repeats them, so the file and line are named for the user to drop.
fn bazelrc_conflicts(root: &Path) -> Vec<Finding> {
    let owned = crate::preset_fragment::rendered_flag_lines(&crate::preset_fragment::render_all());
    let mut out = Vec::new();
    let mut pending = vec![".bazelrc".to_owned()];
    let mut seen = std::collections::BTreeSet::new();
    while let Some(rel) = pending.pop() {
        if rel.starts_with(".dx/bazelrc/") || !seen.insert(rel.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let words: Vec<&str> = line.split_whitespace().collect();
            if let [verb, path] = words.as_slice() {
                if matches!(*verb, "import" | "try-import") {
                    if let Some(next) = path.strip_prefix("%workspace%/") {
                        pending.push(next.to_owned());
                    }
                    continue;
                }
            }
            if owned.contains(line) {
                out.push(Finding {
                    kind: Kind::BazelrcConflict,
                    subject: line.to_owned(),
                    before: "in your rc".to_owned(),
                    after: "dx preset".to_owned(),
                    file: rel.clone(),
                    line: index + 1,
                    hint: "the dx preset layers set this flag; drop the line so dx can import them"
                        .to_owned(),
                    hard: false,
                });
            }
        }
    }
    out.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    out
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Eco {
    Rust,
    Llvm,
    Cc,
    Python,
}

fn eco_of(call: &dx_module_bazel::Call) -> Option<Eco> {
    let [Value::Str(label), Value::Str(name), ..] = call.positional.as_slice() else {
        return None;
    };
    let (label, name) = (label.value.as_str(), name.value.as_str());
    if label == "@rules_rust//rust:extensions.bzl" && name == "rust" {
        Some(Eco::Rust)
    } else if label.starts_with("@toolchains_llvm//") {
        Some(Eco::Llvm)
    } else if name == "cc_configure_extension" {
        Some(Eco::Cc)
    } else if label.starts_with("@rules_python//") && name == "python" {
        Some(Eco::Python)
    } else {
        None
    }
}

fn label_repo(label: &str) -> &str {
    let rest = label.trim_start_matches('@');
    rest.split("//").next().unwrap_or(rest)
}

/// Repos imported from a recognised extension, and the bindings by ecosystem.
type EcoMap = Vec<(String, Eco)>;

fn eco_repos(module: &ModuleFile) -> (EcoMap, EcoMap) {
    let mut repos = Vec::new();
    let mut bindings = Vec::new();
    for call in module.calls_named("use_extension") {
        let (Some(eco), Some(binding)) = (eco_of(call), call.binding.as_deref()) else {
            continue;
        };
        bindings.push((binding.to_owned(), eco));
        for (repo, _) in module.use_repos(binding) {
            repos.push((repo.to_owned(), eco));
        }
    }
    (repos, bindings)
}

fn registered_labels(module: &ModuleFile) -> Vec<(&str, usize)> {
    let mut out = Vec::new();
    for call in module.calls_named("register_toolchains") {
        for value in &call.positional {
            if let Value::Str(label) = value {
                out.push((label.value.as_str(), call.line));
            }
        }
    }
    out
}

/// Normalised argument text of a call, without the binding name.
fn args_text(module: &ModuleFile, call: &dx_module_bazel::Call) -> String {
    let text = module.source().get(call.span.clone()).unwrap_or_default();
    let args = text.find('(').map_or(text, |i| &text[i..]);
    args.chars().filter(|c| !c.is_whitespace()).collect()
}

/// True when every `rust.toolchain` tag of the user is one the snippet also carries, so
/// the snippet would add nothing new on the extension.
fn rust_tags_adopted(sources: &[Source], snippet: &ModuleFile) -> bool {
    let tags = |module: &ModuleFile| -> Vec<String> {
        let (_, bindings) = eco_repos(module);
        bindings
            .iter()
            .filter(|(_, e)| *e == Eco::Rust)
            .flat_map(|(b, _)| {
                let name = format!("{b}.toolchain");
                module
                    .calls_named(&name)
                    .map(|c| args_text(module, c))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let ours = tags(snippet);
    sources
        .iter()
        .flat_map(|s| tags(s.module))
        .all(|t| ours.contains(&t))
}

/// Toolchain registrations of the user that would compete with what the snippet adds.
/// Measured against the merged module: snippet registrations the user already has are
/// not "added", so an adopted repo shows nothing.
fn toolchain_conflicts(sources: &[Source], snippet: Option<&ModuleFile>) -> Vec<Finding> {
    let mut out = Vec::new();
    let user_labels: Vec<&str> = sources
        .iter()
        .flat_map(|s| registered_labels(s.module))
        .map(|(l, _)| l)
        .collect();
    if let Some(snippet) = snippet {
        let (snippet_repos, _) = eco_repos(snippet);
        for (label, _) in registered_labels(snippet) {
            let repo = label_repo(label);
            let eco = snippet_repos
                .iter()
                .find(|(r, _)| r == repo)
                .map(|(_, e)| *e)
                .or_else(|| (repo == "llvm").then_some(Eco::Llvm));
            let Some(eco) = eco else { continue };
            if user_labels.contains(&label)
                && (eco != Eco::Rust || rust_tags_adopted(sources, snippet))
            {
                continue;
            }
            let mut first: Option<(String, usize, String)> = None;
            for source in sources {
                let (repos, bindings) = eco_repos(source.module);
                let mut hits: Vec<(usize, String)> = Vec::new();
                if eco == Eco::Rust {
                    for (binding, e) in &bindings {
                        if *e == Eco::Rust {
                            for call in source.module.calls_named(&format!("{binding}.toolchain")) {
                                hits.push((call.line, format!("{binding}.toolchain")));
                            }
                        }
                    }
                }
                for (user_label, line) in registered_labels(source.module) {
                    let ours = repos
                        .iter()
                        .find(|(r, _)| r == label_repo(user_label))
                        .map(|(_, e)| *e);
                    let same = matches!(
                        (eco, ours),
                        (Eco::Rust, Some(Eco::Rust)) | (Eco::Llvm, Some(Eco::Llvm | Eco::Cc))
                    );
                    if same {
                        hits.push((line, format!("register_toolchains(\"{user_label}\")")));
                    }
                }
                if let Some((line, what)) = hits.into_iter().min_by_key(|(l, _)| *l) {
                    first = Some((source.file.clone(), line, what));
                    break;
                }
            }
            let Some((file, line, what)) = first else {
                continue;
            };
            let subject = if eco == Eco::Rust { "rust" } else { "llvm/cc" };
            out.push(Finding {
                kind: Kind::ToolchainConflict,
                subject: subject.to_owned(),
                before: format!("{what} ({file}:{line})"),
                after: format!("register_toolchains(\"{label}\")"),
                file,
                line,
                hint: format!(
                    "Bazel prefers the first registration in the root module, so yours wins over the line dx appends, and --extra_toolchains in .bazelrc beats both; keep the bazel_dep lines and drop the snippet's {} lines",
                    if eco == Eco::Rust {
                        "rust.toolchain/use_repo/register_toolchains"
                    } else {
                        "register_toolchains"
                    }
                ),
                hard: true,
            });
        }
    }
    for source in sources {
        let (_, bindings) = eco_repos(source.module);
        for (binding, eco) in bindings {
            if eco != Eco::Python {
                continue;
            }
            for call in source.module.calls_named(&format!("{binding}.toolchain")) {
                out.push(Finding {
                    kind: Kind::ToolchainConflict,
                    subject: "python".to_owned(),
                    before: format!("{binding}.toolchain ({}:{})", source.file, call.line),
                    after: "rules_dx python.toolchain (as a dependency)".to_owned(),
                    file: source.file.clone(),
                    line: call.line,
                    hint: "root-module toolchains win over dependency modules, so yours is used; nothing to change".to_owned(),
                    hard: false,
                });
            }
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
        ("toolchain conflicts", Kind::ToolchainConflict),
        ("bazelrc conflicts", Kind::BazelrcConflict),
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
    fn bazelrc_repeat_is_advisory_with_line() {
        let dir = scratch(&[
            ("MODULE.bazel", "module(name = \"app\")\n"),
            (
                ".bazelrc",
                "# c\ntry-import %workspace%/extra.bazelrc\nbuild --jobs=2\n",
            ),
            ("extra.bazelrc", "\nbuild --verbose_failures\n"),
        ]);
        let plan = preflight(dir.path(), &snippet());
        let f: Vec<_> = plan
            .findings
            .iter()
            .filter(|f| f.kind == Kind::BazelrcConflict)
            .collect();
        assert_eq!(f.len(), 1, "{plan:?}");
        assert_eq!(
            (f[0].file.as_str(), f[0].line, f[0].hard),
            ("extra.bazelrc", 2, false)
        );
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

    fn toolchain_findings(module: &str) -> Vec<Finding> {
        let dir = scratch(&[("MODULE.bazel", module)]);
        preflight(dir.path(), &snippet())
            .findings
            .into_iter()
            .filter(|f| f.kind == Kind::ToolchainConflict)
            .collect()
    }

    const RUST_EXT: &str = "my = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\n";

    #[test]
    fn no_toolchains_no_findings() {
        assert!(toolchain_findings("module(name = \"app\")\n").is_empty());
    }

    #[test]
    fn user_rust_toolchain_tag_is_hard() {
        let module = format!(
            "module(name = \"app\")\n{RUST_EXT}my.toolchain(versions = [\"1.80.0\"])\nuse_repo(my, \"rust_toolchains\")\n"
        );
        let found = toolchain_findings(&module);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].hard);
        assert_eq!(found[0].subject, "rust");
        assert_eq!((found[0].file.as_str(), found[0].line), ("MODULE.bazel", 3));
    }

    #[test]
    fn registration_of_own_rust_repo_is_hard() {
        let module = format!(
            "{RUST_EXT}use_repo(my, \"my_rust\")\nregister_toolchains(\"@my_rust//:all\")\n"
        );
        let found = toolchain_findings(&module);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].hard && found[0].before.contains("@my_rust//:all"));
    }

    #[test]
    fn llvm_and_cc_configure_registrations_are_hard() {
        for ext in [
            "x = use_extension(\"@toolchains_llvm//toolchain/extensions:llvm.bzl\", \"llvm\")\n",
            "x = use_extension(\"@bazel_tools//tools/cpp:cc_configure.bzl\", \"cc_configure_extension\")\n",
        ] {
            let module = format!("{ext}use_repo(x, \"cc_x\")\nregister_toolchains(\"@cc_x//:all\")\n");
            let found = toolchain_findings(&module);
            assert_eq!(found.len(), 1, "{found:?}");
            assert!(found[0].hard && found[0].subject == "llvm/cc");
            assert!(found[0].hint.contains("first registration"));
        }
    }

    #[test]
    fn python_toolchain_is_advisory() {
        let module = "py = use_extension(\"@rules_python//python/extensions:python.bzl\", \"python\")\npy.toolchain(python_version = \"3.12\")\n";
        let found = toolchain_findings(module);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(!found[0].hard && found[0].subject == "python");
    }

    #[test]
    fn unknown_registration_is_not_flagged() {
        assert!(toolchain_findings("register_toolchains(\"@whatever//:all\")\n").is_empty());
    }

    #[test]
    fn conflict_in_an_included_file_names_that_file() {
        let dir = scratch(&[
            ("MODULE.bazel", "include(\"//:tc.MODULE.bazel\")\n"),
            (
                "tc.MODULE.bazel",
                "my = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nmy.toolchain(versions = [\"1.80.0\"])\n",
            ),
        ]);
        let plan = preflight(dir.path(), &snippet());
        let f = plan
            .findings
            .iter()
            .find(|f| f.kind == Kind::ToolchainConflict)
            .expect("finding");
        assert_eq!((f.file.as_str(), f.line), ("tc.MODULE.bazel", 2));
    }

    #[test]
    fn canonical_rust_setup_with_other_version_is_hard() {
        let module = format!(
            "module(name = \"app\")\n{RUST_EXT}my.toolchain(versions = [\"1.80.0\"])\nuse_repo(my, \"rust_toolchains\")\nregister_toolchains(\"@rust_toolchains//:all\")\n"
        );
        let found = toolchain_findings(&module);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].hard && found[0].subject == "rust");
    }

    #[test]
    fn adopted_repo_has_no_toolchain_findings() {
        let module = format!("module(name = \"app\")\n{}", snippet());
        assert!(toolchain_findings(&module).is_empty());
    }

    #[test]
    fn toolchain_section_renders() {
        let module = format!("{RUST_EXT}my.toolchain(versions = [\"1.80.0\"])\n");
        let dir = scratch(&[("MODULE.bazel", &module)]);
        let text = render_plan_text("m", &preflight(dir.path(), &snippet()));
        assert!(text.contains("toolchain conflicts:\n  rust "), "{text}");
        let clean = render_plan_text("m", &Preflight::default());
        assert!(clean.contains("toolchain conflicts:\n  none\n"), "{clean}");
    }
}
