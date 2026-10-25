use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use dx_module_bazel::{load_labels, Call, ModuleFile, Value};

fn read(workspace: &Path, rel: &str) -> Result<Option<String>, String> {
    match std::fs::read_to_string(workspace.join(rel)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {rel}: {error}")),
    }
}

fn literal_arg<'a>(call: &'a Call, key: &str) -> Result<&'a str, String> {
    match call.positional.first().or_else(|| call.arg(key)) {
        Some(Value::Str(literal)) => Ok(&literal.value),
        _ => Err(format!(
            "{} at line {} has no literal file label",
            call.name, call.line
        )),
    }
}

fn main_repo(repo: &str, names: &[String]) -> bool {
    repo.is_empty() || names.iter().any(|name| name == repo)
}

fn label_path(label: &str, package: &str, names: &[String]) -> Option<(String, String)> {
    let (package, target) = if let Some(rest) = label.strip_prefix('@') {
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        let (repo, rest) = rest.split_once("//")?;
        if !main_repo(repo, names) {
            return None;
        }
        main_label(rest)
    } else if let Some(rest) = label.strip_prefix("//") {
        main_label(rest)
    } else {
        (
            package.to_owned(),
            label.strip_prefix(':').unwrap_or(label).to_owned(),
        )
    };
    if target.is_empty()
        || format!("{package}/{target}")
            .split('/')
            .any(|part| part == "..")
    {
        return None;
    }
    let path = if package.is_empty() {
        target.clone()
    } else {
        format!("{package}/{target}")
    };
    Some((path, package))
}

fn main_label(rest: &str) -> (String, String) {
    match rest.split_once(':') {
        Some((package, target)) => (package.to_owned(), target.to_owned()),
        None => (
            rest.to_owned(),
            rest.rsplit('/').next().unwrap_or(rest).to_owned(),
        ),
    }
}

fn own_names(file: &ModuleFile) -> Vec<String> {
    file.calls_named("module")
        .flat_map(|call| [call.str("name"), call.str("repo_name")])
        .flatten()
        .map(str::to_owned)
        .collect()
}

struct ModuleRoots {
    names: Vec<String>,
    extensions: Vec<(String, String)>,
    toolchains: Vec<(String, String)>,
}

fn module_roots(workspace: &Path) -> Result<ModuleRoots, String> {
    let mut roots = ModuleRoots {
        names: Vec::new(),
        extensions: Vec::new(),
        toolchains: Vec::new(),
    };
    let mut modules: Vec<(String, String)> = vec![("MODULE.bazel".to_owned(), String::new())];
    let mut seen_modules: BTreeSet<String> = BTreeSet::new();
    while let Some((rel, package)) = modules.pop() {
        if !seen_modules.insert(rel.clone()) {
            continue;
        }
        let Some(source) = read(workspace, &rel)? else {
            if rel == "MODULE.bazel" {
                return Ok(roots);
            }
            return Err(format!("{rel} is included but missing"));
        };
        let file = ModuleFile::parse(&source).map_err(|error| format!("{rel}: {error}"))?;
        if rel == "MODULE.bazel" {
            roots.names = own_names(&file);
        }
        for call in file.calls() {
            if matches!(
                call.name.as_str(),
                "register_toolchains" | "register_execution_platforms"
            ) {
                if call.unpacked {
                    return Err(no_pattern(&rel, call));
                }
                for value in &call.positional {
                    let Value::Str(pattern) = value else {
                        return Err(no_pattern(&rel, call));
                    };
                    roots
                        .toolchains
                        .push((pattern.value.clone(), package.clone()));
                }
                continue;
            }
            let key = match call.name.as_str() {
                "use_extension" => "extension_bzl_file",
                "use_repo_rule" => "repo_rule_bzl_file",
                "include" => "label",
                _ => continue,
            };
            let label = literal_arg(call, key).map_err(|error| format!("{rel}: {error}"))?;
            let Some(target) = label_path(label, &package, &roots.names) else {
                if call.name == "include" {
                    return Err(format!(
                        "{rel}: include {label:?} is not in the main repository"
                    ));
                }
                continue;
            };
            if call.name == "include" {
                modules.push(target);
            } else {
                roots.extensions.push(target);
            }
        }
    }
    Ok(roots)
}

fn no_pattern(rel: &str, call: &Call) -> String {
    format!(
        "{rel}: {} at line {} has no literal target pattern",
        call.name, call.line
    )
}

struct BazelrcRoots {
    aspects: Vec<String>,
    toolchains: Vec<String>,
}

fn bazelrc_roots(workspace: &Path) -> Result<BazelrcRoots, String> {
    let mut pending: Vec<String> = Vec::new();
    let entries =
        std::fs::read_dir(workspace).map_err(|error| format!("cannot read workspace: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read workspace: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".bazelrc") && workspace.join(&name).is_file() {
            pending.push(name);
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut roots = BazelrcRoots {
        aspects: Vec::new(),
        toolchains: Vec::new(),
    };
    while let Some(rel) = pending.pop() {
        if !seen.insert(rel.clone()) {
            continue;
        }
        let Some(text) = read(workspace, &rel)? else {
            continue;
        };
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or_default();
            let words: Vec<&str> = line.split_whitespace().collect();
            if let [verb, path] = words.as_slice() {
                if matches!(*verb, "import" | "try-import") {
                    if let Some(rest) = path.strip_prefix("%workspace%/") {
                        pending.push(rest.to_owned());
                    }
                    continue;
                }
            }
            for (index, word) in words.iter().enumerate() {
                let Some(flag) = word.strip_prefix("--") else {
                    continue;
                };
                let (flag, value) = match flag.split_once('=') {
                    Some(pair) => pair,
                    None => match words.get(index + 1) {
                        Some(value) => (flag, *value),
                        None => continue,
                    },
                };
                let list = match flag {
                    "aspects" => &mut roots.aspects,
                    "extra_toolchains"
                    | "extra_execution_platforms"
                    | "platforms"
                    | "host_platform" => &mut roots.toolchains,
                    _ => continue,
                };
                list.extend(
                    value
                        .split(',')
                        .map(|item| item.trim_matches(|c| c == '"' || c == '\''))
                        .filter(|item| !item.is_empty())
                        .map(str::to_owned),
                );
            }
        }
    }
    Ok(roots)
}

fn pattern_builds(
    pattern: &str,
    package: &str,
    names: &[String],
    builds: &[(String, String)],
) -> Vec<String> {
    let rest = if let Some(rest) = pattern.strip_prefix('@') {
        let rest = rest.strip_prefix('@').unwrap_or(rest);
        let Some((repo, rest)) = rest.split_once("//") else {
            return Vec::new();
        };
        if !main_repo(repo, names) {
            return Vec::new();
        }
        rest.to_owned()
    } else if let Some(rest) = pattern.strip_prefix("//") {
        rest.to_owned()
    } else if pattern.starts_with('-') {
        return Vec::new();
    } else if let Some(target) = pattern.strip_prefix(':') {
        format!("{package}:{target}")
    } else {
        let joined = if package.is_empty() {
            pattern.to_owned()
        } else {
            format!("{package}/{pattern}")
        };
        let mut found = pattern_builds(&format!("//{joined}"), "", names, builds);
        if !pattern.contains('/') && !pattern.contains(':') {
            found.extend(pattern_builds(&format!("//{package}"), "", names, builds));
        }
        return found;
    };
    let spec = rest.split_once(':').map_or(rest.as_str(), |(spec, _)| spec);
    if spec.split('/').any(|part| part == "..") {
        return Vec::new();
    }
    let recursive = if spec == "..." {
        Some("")
    } else {
        spec.strip_suffix("/...")
    };
    builds
        .iter()
        .filter(|(_, build_package)| match recursive {
            Some("") => true,
            Some(prefix) => {
                build_package == prefix
                    || build_package
                        .strip_prefix(prefix)
                        .is_some_and(|tail| tail.starts_with('/'))
            }
            None => build_package == spec,
        })
        .map(|(rel, _)| rel.clone())
        .collect()
}

fn read_bazelignore(workspace: &Path) -> Result<Vec<String>, String> {
    let Some(text) = read(workspace, ".bazelignore")? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("./").unwrap_or(line);
        let line = line.trim_end_matches('/');
        if line.is_empty() || line == "." {
            continue;
        }
        out.push(line.to_owned());
    }
    Ok(out)
}

fn is_ignored(rel: &str, ignores: &[String]) -> bool {
    ignores.iter().any(|prefix| {
        rel == prefix
            || rel
                .strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

fn build_files(workspace: &Path) -> Result<Vec<(String, String)>, String> {
    let ignores = read_bazelignore(workspace)?;
    let mut found: Vec<(String, String)> = Vec::new();
    let mut dirs: Vec<String> = vec![String::new()];
    while let Some(dir) = dirs.pop() {
        let shown = if dir.is_empty() { "." } else { dir.as_str() };
        let entries = std::fs::read_dir(workspace.join(&dir))
            .map_err(|error| format!("cannot read {shown}: {error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("cannot read {shown}: {error}"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if dir.is_empty() {
                name.clone()
            } else {
                format!("{dir}/{name}")
            };
            let kind = entry
                .file_type()
                .map_err(|error| format!("cannot read {rel}: {error}"))?;
            if kind.is_dir() {
                if name != ".git" && !is_ignored(&rel, &ignores) {
                    dirs.push(rel);
                }
            } else if matches!(name.as_str(), "BUILD" | "BUILD.bazel")
                && workspace.join(&rel).is_file()
            {
                found.push((rel, dir.clone()));
            }
        }
    }
    found.sort();
    Ok(found)
}

struct LoadGraph<'a> {
    workspace: &'a Path,
    names: &'a [String],
    loads: BTreeMap<String, Vec<String>>,
}

impl LoadGraph<'_> {
    fn add(&mut self, rel: &str, package: &str) -> Result<(), String> {
        let mut pending: Vec<(String, String)> = vec![(rel.to_owned(), package.to_owned())];
        while let Some((rel, package)) = pending.pop() {
            if self.loads.contains_key(&rel) {
                continue;
            }
            let Some(source) = read(self.workspace, &rel)? else {
                self.loads.insert(rel, Vec::new());
                continue;
            };
            let labels = load_labels(&source).map_err(|error| format!("{rel}: {error}"))?;
            let mut targets = Vec::new();
            for label in labels {
                if let Some(target) = label_path(&label, &package, self.names) {
                    targets.push(target.0.clone());
                    pending.push(target);
                }
            }
            self.loads.insert(rel, targets);
        }
        Ok(())
    }

    fn closure(&self, roots: &[String]) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<&str> = roots.iter().map(String::as_str).collect();
        while let Some(rel) = pending.pop() {
            if !seen.insert(rel.to_owned()) {
                continue;
            }
            if let Some(targets) = self.loads.get(rel) {
                pending.extend(targets.iter().map(String::as_str));
            }
        }
        seen
    }
}

/// How a changed .bzl file reaches the build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BzlImpact {
    Global(String),
    Packages(BTreeSet<String>),
}

/// The packages that load each changed .bzl file, or why it affects the whole workspace.
pub fn bzl_impact(
    workspace: &Path,
    changed_bzl: &[String],
    aspect_labels: &[String],
) -> Result<BTreeMap<String, BzlImpact>, String> {
    let roots = module_roots(workspace)?;
    let mut graph = LoadGraph {
        workspace,
        names: &roots.names,
        loads: BTreeMap::new(),
    };
    let mut extension_roots = Vec::new();
    for (rel, package) in &roots.extensions {
        graph.add(rel, package)?;
        extension_roots.push(rel.clone());
    }
    let mut aspect_roots = Vec::new();
    let bazelrc = bazelrc_roots(workspace)?;
    let mut aspects = aspect_labels.to_vec();
    aspects.extend(bazelrc.aspects);
    for aspect in &aspects {
        let file = aspect.split('%').next().unwrap_or(aspect);
        if let Some((rel, package)) = label_path(file, "", &roots.names) {
            graph.add(&rel, &package)?;
            aspect_roots.push(rel);
        }
    }
    let builds = build_files(workspace)?;
    for (rel, package) in &builds {
        graph.add(rel, package)?;
    }
    let mut toolchain_roots = Vec::new();
    let patterns = roots
        .toolchains
        .iter()
        .map(|(pattern, package)| (pattern.as_str(), package.as_str()))
        .chain(
            bazelrc
                .toolchains
                .iter()
                .map(|pattern| (pattern.as_str(), "")),
        );
    for (pattern, package) in patterns {
        toolchain_roots.extend(pattern_builds(pattern, package, &roots.names, &builds));
    }
    let extension_inputs = graph.closure(&extension_roots);
    let aspect_inputs = graph.closure(&aspect_roots);
    let toolchain_inputs = graph.closure(&toolchain_roots);
    let mut loaders: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (rel, targets) in &graph.loads {
        for target in targets {
            loaders
                .entry(target.as_str())
                .or_default()
                .push(rel.as_str());
        }
    }
    let packages: BTreeMap<&str, &str> = builds
        .iter()
        .map(|(rel, package)| (rel.as_str(), package.as_str()))
        .collect();
    let mut impact = BTreeMap::new();
    for path in changed_bzl {
        let effect = if extension_inputs.contains(path) {
            BzlImpact::Global(format!("module extension input {path}"))
        } else if aspect_inputs.contains(path) {
            BzlImpact::Global(format!("aspect input {path}"))
        } else if toolchain_inputs.contains(path) {
            BzlImpact::Global(format!("toolchain input {path}"))
        } else {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            let mut pending: Vec<&str> = vec![path.as_str()];
            let mut selected = BTreeSet::new();
            while let Some(rel) = pending.pop() {
                if !seen.insert(rel) {
                    continue;
                }
                if let Some(package) = packages.get(rel) {
                    selected.insert(format!("//{package}:all"));
                }
                if let Some(from) = loaders.get(rel) {
                    pending.extend(from.iter().copied());
                }
            }
            BzlImpact::Packages(selected)
        };
        impact.insert(path.clone(), effect);
    }
    Ok(impact)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let full = root.join(rel);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(full, text).expect("write");
    }

    fn impact(root: &Path, changed: &[&str], aspects: &[&str]) -> Vec<(String, BzlImpact)> {
        let changed: Vec<String> = changed.iter().map(ToString::to_string).collect();
        let aspects: Vec<String> = aspects.iter().map(ToString::to_string).collect();
        bzl_impact(root, &changed, &aspects)
            .expect("load graph")
            .into_iter()
            .collect()
    }

    fn global(path: &str, reason: &str) -> (String, BzlImpact) {
        (
            path.to_owned(),
            BzlImpact::Global(format!("{reason} {path}")),
        )
    }

    fn packages(path: &str, packages: &[&str]) -> (String, BzlImpact) {
        (
            path.to_owned(),
            BzlImpact::Packages(packages.iter().map(ToString::to_string).collect()),
        )
    }

    #[test]
    fn extension_files_name_direct_extensions_and_repo_rules() {
        let scratch = dx_test_scratch::scratch("dx-ext-direct-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools/ext:x.bzl\", \"x\")\nr = use_repo_rule(repo_rule_bzl_file = \"@@//tools:repo.bzl\", repo_rule_name = \"r\")\ny = use_extension(\":root.bzl\", \"y\")\n",
        );
        write(root, "tools/ext/x.bzl", "X = 1\n");
        write(root, "tools/repo.bzl", "R = 1\n");
        write(root, "root.bzl", "Y = 1\n");
        write(root, "other.bzl", "O = 1\n");
        assert_eq!(
            impact(
                root,
                &["other.bzl", "root.bzl", "tools/ext/x.bzl", "tools/repo.bzl"],
                &[]
            ),
            [
                packages("other.bzl", &[]),
                global("root.bzl", "module extension input"),
                global("tools/ext/x.bzl", "module extension input"),
                global("tools/repo.bzl", "module extension input"),
            ]
        );
    }

    #[test]
    fn extension_files_follow_helper_loads() {
        let scratch = dx_test_scratch::scratch("dx-ext-helper-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\", repo_name = \"own\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\n",
        );
        write(
            root,
            "tools/ext.bzl",
            "load(\":helper.bzl\", \"h\")\nload(\"@demo//lib:shared.bzl\", \"s\")\nload(\"@own//lib:own.bzl\", \"o\")\nload(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_archive\")\n",
        );
        write(
            root,
            "tools/helper.bzl",
            "load(\"//lib:deep.bzl\", \"d\")\n",
        );
        write(root, "lib/shared.bzl", "S = 1\n");
        write(root, "lib/own.bzl", "O = 1\n");
        write(root, "lib/deep.bzl", "D = 1\n");
        write(root, "lib/unused.bzl", "U = 1\n");
        assert_eq!(
            impact(
                root,
                &[
                    "lib/deep.bzl",
                    "lib/own.bzl",
                    "lib/shared.bzl",
                    "lib/unused.bzl",
                    "tools/helper.bzl"
                ],
                &[]
            ),
            [
                global("lib/deep.bzl", "module extension input"),
                global("lib/own.bzl", "module extension input"),
                global("lib/shared.bzl", "module extension input"),
                packages("lib/unused.bzl", &[]),
                global("tools/helper.bzl", "module extension input"),
            ]
        );
    }

    #[test]
    fn extension_files_read_included_module_files() {
        let scratch = dx_test_scratch::scratch("dx-ext-include-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\ninclude(\"//deps:go.MODULE.bazel\")\n",
        );
        write(
            root,
            "deps/go.MODULE.bazel",
            "go = use_extension(\"//deps:go.bzl\", \"go\")\n",
        );
        write(root, "deps/go.bzl", "G = 1\n");
        assert_eq!(
            impact(root, &["deps/go.bzl"], &[]),
            [global("deps/go.bzl", "module extension input")]
        );
    }

    #[test]
    fn extension_files_skip_external_extensions_and_missing_module() {
        let scratch = dx_test_scratch::scratch("dx-ext-external-");
        let root = scratch.path();
        write(root, "rust/extensions.bzl", "R = 1\n");
        assert_eq!(
            impact(root, &["rust/extensions.bzl"], &[]),
            [packages("rust/extensions.bzl", &[])]
        );
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\n",
        );
        assert_eq!(
            impact(root, &["rust/extensions.bzl"], &[]),
            [packages("rust/extensions.bzl", &[])]
        );
    }

    #[test]
    fn extension_files_fail_on_parse_errors() {
        let scratch = dx_test_scratch::scratch("dx-ext-error-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\n",
        );
        write(root, "tools/ext.bzl", "load(\":a.bzl\"\n");
        let changed = ["tools/a.bzl".to_owned()];
        let error = bzl_impact(root, &changed, &[]).expect_err("parse error");
        assert!(error.starts_with("tools/ext.bzl: "), "{error}");
        write(root, "MODULE.bazel", "module(name = \n");
        let error = bzl_impact(root, &changed, &[]).expect_err("module parse error");
        assert!(error.starts_with("MODULE.bazel: "), "{error}");
    }

    #[test]
    fn load_graph_selects_direct_and_transitive_loaders() {
        let scratch = dx_test_scratch::scratch("dx-load-transitive-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "tools/BUILD.bazel", "");
        write(root, "tools/a.bzl", "load(\":b.bzl\", \"b\")\n");
        write(root, "tools/b.bzl", "B = 1\n");
        write(root, "app/BUILD.bazel", "load(\"//tools:a.bzl\", \"a\")\n");
        write(root, "lib/BUILD", "load(\"//tools:b.bzl\", \"b\")\n");
        write(root, "BUILD.bazel", "load(\"//tools:a.bzl\", \"a\")\n");
        assert_eq!(
            impact(root, &["tools/a.bzl", "tools/b.bzl"], &[]),
            [
                packages("tools/a.bzl", &["//:all", "//app:all"]),
                packages("tools/b.bzl", &["//:all", "//app:all", "//lib:all"]),
            ]
        );
    }

    #[test]
    fn relative_loads_in_a_bzl_resolve_against_its_label_package() {
        let scratch = dx_test_scratch::scratch("dx-load-relative-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "tools/BUILD.bazel", "");
        write(root, "tools/sub/a.bzl", "load(\":helper.bzl\", \"h\")\n");
        write(root, "tools/helper.bzl", "H = 1\n");
        write(root, "tools/sub/helper.bzl", "H = 2\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//tools:sub/a.bzl\", \"a\")\n",
        );
        assert_eq!(
            impact(root, &["tools/helper.bzl", "tools/sub/helper.bzl"], &[]),
            [
                packages("tools/helper.bzl", &["//app:all"]),
                packages("tools/sub/helper.bzl", &[]),
            ]
        );
    }

    #[test]
    fn walk_skips_ignored_dirs_and_symlinks() {
        let scratch = dx_test_scratch::scratch("dx-load-skip-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, ".bazelignore", "# comment\n./node_modules/\n");
        write(root, "defs.bzl", "D = 1\n");
        write(
            root,
            "node_modules/x/BUILD.bazel",
            "load(\"//:defs.bzl\", \"d\")\n",
        );
        write(root, "real/BUILD.bazel", "load(\"//:defs.bzl\", \"d\")\n");
        write(root, "nested/MODULE.bazel", "module(name = \"nested\")\n");
        write(
            root,
            "nested/BUILD.bazel",
            "load(\"@demo//:defs.bzl\", \"d\")\n",
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(root.join("real"), root.join("bazel-out")).expect("symlink");
        assert_eq!(
            impact(root, &["defs.bzl"], &[]),
            [packages("defs.bzl", &["//nested:all", "//real:all"])]
        );
    }

    #[test]
    fn aspects_from_bazelrc_and_caller_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-aspects-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            ".bazelrc",
            "import %workspace%/tools/preset.bazelrc\ntry-import %workspace%/missing.bazelrc\n",
        );
        write(
            root,
            "tools/preset.bazelrc",
            "build --aspects=//rc:aspect.bzl%rc_aspect,@rules_x//:x.bzl%x # lint\n",
        );
        write(root, "rc/aspect.bzl", "load(\":rc_helper.bzl\", \"h\")\n");
        write(root, "rc/rc_helper.bzl", "H = 1\n");
        write(root, "quality/aspects.bzl", "A = 1\n");
        write(root, "plain.bzl", "P = 1\n");
        assert_eq!(
            impact(
                root,
                &["plain.bzl", "quality/aspects.bzl", "rc/rc_helper.bzl"],
                &["@demo//quality:aspects.bzl%lint_aspect"]
            ),
            [
                packages("plain.bzl", &[]),
                global("quality/aspects.bzl", "aspect input"),
                global("rc/rc_helper.bzl", "aspect input"),
            ]
        );
    }

    #[test]
    fn toolchain_packages_from_module_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchains-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\n    \"//tc:all\",\n    dev_dependency = True,\n)\nregister_execution_platforms(\":plat\")\n",
        );
        write(root, "tc/BUILD.bazel", "load(\"//rules:tc.bzl\", \"t\")\n");
        write(root, "rules/tc.bzl", "load(\":helper.bzl\", \"h\")\n");
        write(root, "rules/helper.bzl", "H = 1\n");
        write(root, "BUILD.bazel", "load(\"//rules:plat.bzl\", \"p\")\n");
        write(root, "rules/plat.bzl", "P = 1\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//rules:app.bzl\", \"a\")\n",
        );
        write(root, "rules/app.bzl", "A = 1\n");
        assert_eq!(
            impact(
                root,
                &[
                    "rules/app.bzl",
                    "rules/helper.bzl",
                    "rules/plat.bzl",
                    "rules/tc.bzl"
                ],
                &[]
            ),
            [
                packages("rules/app.bzl", &["//app:all"]),
                global("rules/helper.bzl", "toolchain input"),
                global("rules/plat.bzl", "toolchain input"),
                global("rules/tc.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn toolchain_patterns_from_includes_and_recursive_patterns() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-include-");
        let root = scratch.path();
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\ninclude(\"//deps:tc.MODULE.bazel\")\n",
        );
        write(
            root,
            "deps/tc.MODULE.bazel",
            "register_toolchains(\"//tcs/...\", \"@other//:all\", \"-//tcs/skip:all\")\n",
        );
        write(root, ".bazelignore", "tcs/ignored\n");
        write(root, "tcs/a/BUILD.bazel", "load(\"//lib:a.bzl\", \"a\")\n");
        write(root, "tcs/a/b/BUILD", "load(\"//lib:b.bzl\", \"b\")\n");
        write(
            root,
            "tcs/ignored/BUILD.bazel",
            "load(\"//lib:ignored.bzl\", \"i\")\n",
        );
        write(root, "tcsx/BUILD.bazel", "load(\"//lib:x.bzl\", \"x\")\n");
        write(root, "lib/a.bzl", "A = 1\n");
        write(root, "lib/b.bzl", "B = 1\n");
        write(root, "lib/ignored.bzl", "I = 1\n");
        write(root, "lib/x.bzl", "X = 1\n");
        assert_eq!(
            impact(
                root,
                &["lib/a.bzl", "lib/b.bzl", "lib/ignored.bzl", "lib/x.bzl"],
                &[]
            ),
            [
                global("lib/a.bzl", "toolchain input"),
                global("lib/b.bzl", "toolchain input"),
                packages("lib/ignored.bzl", &[]),
                packages("lib/x.bzl", &["//tcsx:all"]),
            ]
        );
    }

    #[test]
    fn toolchains_from_bazelrc_widen() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-rc-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            ".bazelrc",
            "import %workspace%/tools/preset.bazelrc\n",
        );
        write(
            root,
            "tools/preset.bazelrc",
            "common:windows --extra_execution_platforms=//plat:win,@platforms//host\nbuild --extra_toolchains //tc:x\nbuild --host_platform=//host # host\n",
        );
        write(
            root,
            "plat/BUILD.bazel",
            "load(\"//lib:plat.bzl\", \"p\")\n",
        );
        write(root, "tc/BUILD.bazel", "load(\"//lib:tc.bzl\", \"t\")\n");
        write(
            root,
            "host/BUILD.bazel",
            "load(\"//lib:host.bzl\", \"h\")\n",
        );
        write(root, "app/BUILD.bazel", "load(\"//lib:app.bzl\", \"a\")\n");
        write(root, "lib/plat.bzl", "P = 1\n");
        write(root, "lib/tc.bzl", "T = 1\n");
        write(root, "lib/host.bzl", "H = 1\n");
        write(root, "lib/app.bzl", "A = 1\n");
        assert_eq!(
            impact(
                root,
                &["lib/app.bzl", "lib/host.bzl", "lib/plat.bzl", "lib/tc.bzl"],
                &[]
            ),
            [
                packages("lib/app.bzl", &["//app:all"]),
                global("lib/host.bzl", "toolchain input"),
                global("lib/plat.bzl", "toolchain input"),
                global("lib/tc.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn computed_toolchain_pattern_fails() {
        let scratch = dx_test_scratch::scratch("dx-load-toolchain-computed-");
        let root = scratch.path();
        let changed = ["defs.bzl".to_owned()];
        for call in [
            "register_toolchains(*TCS)",
            "register_toolchains(NAME)",
            "register_execution_platforms(\"//p:all\", NAME)",
        ] {
            write(
                root,
                "MODULE.bazel",
                &format!("module(name = \"demo\")\n{call}\n"),
            );
            let error = bzl_impact(root, &changed, &[]).expect_err("computed pattern");
            let name = call.split('(').next().unwrap_or_default();
            assert!(
                error.starts_with(&format!(
                    "MODULE.bazel: {name} at line 2 has no literal target pattern"
                )),
                "{error}"
            );
        }
    }

    #[test]
    fn deleted_bzl_still_matches_its_loaders() {
        let scratch = dx_test_scratch::scratch("dx-load-deleted-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(
            root,
            "app/BUILD.bazel",
            "load(\"//tools:gone.bzl\", \"g\")\n",
        );
        assert_eq!(
            impact(root, &["tools/gone.bzl"], &[]),
            [packages("tools/gone.bzl", &["//app:all"])]
        );
        write(
            root,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\"//tc:all\")\n",
        );
        write(
            root,
            "tc/BUILD.bazel",
            "load(\"//tools:tc_gone.bzl\", \"t\")\n",
        );
        assert_eq!(
            impact(root, &["tools/gone.bzl", "tools/tc_gone.bzl"], &[]),
            [
                packages("tools/gone.bzl", &["//app:all"]),
                global("tools/tc_gone.bzl", "toolchain input"),
            ]
        );
    }

    #[test]
    fn build_parse_errors_fail() {
        let scratch = dx_test_scratch::scratch("dx-load-build-error-");
        let root = scratch.path();
        write(root, "MODULE.bazel", "module(name = \"demo\")\n");
        write(root, "app/BUILD.bazel", "load(\"//:defs.bzl\"\n");
        let error = bzl_impact(root, &["defs.bzl".to_owned()], &[]).expect_err("parse error");
        assert!(error.starts_with("app/BUILD.bazel: "), "{error}");
    }
}
