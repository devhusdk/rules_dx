use super::common::*;
use dx_adopt::dependency_sets::{cargo_fetch_repository, Ecosystem, ResolvedTarget, Selection};
use dx_output::{notice_event, NoticeEvent};
use dx_plan::resolve::{run_label_query_file, QueryRunner};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

pub(crate) const CODE_UPDATE_UNUSED: &str = "update_unused";
pub(crate) const CODE_UPDATE_MISSING: &str = "update_missing";

const EXCEPTIONS_FILE: &str = "depcheck.toml";

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Unused {
    pub set: String,
    pub name: String,
    pub manifest: String,
    pub line: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Missing {
    pub set: String,
    pub name: String,
    pub via: Via,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Via {
    Import { file: String, line: usize },
    Label(String),
}

impl Via {
    fn describe(&self) -> String {
        match self {
            Via::Import { file, line } => format!("imported in {file}:{line}"),
            Via::Label(label) => format!("used by {label}"),
        }
    }
}

fn depcheck_ecosystem(ecosystem: Ecosystem) -> Option<dx_depcheck::Ecosystem> {
    match ecosystem {
        Ecosystem::Cargo => Some(dx_depcheck::Ecosystem::Rust),
        Ecosystem::Npm => Some(dx_depcheck::Ecosystem::Js),
        Ecosystem::Uv => Some(dx_depcheck::Ecosystem::Python),
        Ecosystem::Go => Some(dx_depcheck::Ecosystem::Go),
        Ecosystem::Ruby => Some(dx_depcheck::Ecosystem::Ruby),
        Ecosystem::Maven => Some(dx_depcheck::Ecosystem::Java),
        Ecosystem::Nuget => Some(dx_depcheck::Ecosystem::Csharp),
        _ => None,
    }
}

/// Crate name behind a label of a crate_universe hub: `serde-1.0.229` is `serde`, `cc` stays.
fn crate_name(label: &str) -> Option<String> {
    let (repository, rest) = label.split_once("//")?;
    if !repository.starts_with('@') {
        return None;
    }
    let target = rest.rsplit_once(':').map_or(rest, |(_, target)| target);
    let semver = |text: &str| {
        let core = text.split(['-', '+']).next().unwrap_or("");
        let parts: Vec<&str> = core.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    };
    let name = target
        .match_indices('-')
        .find(|(at, _)| semver(&target[at + 1..]))
        .map_or(target, |(at, _)| &target[..at]);
    (!name.is_empty()).then(|| name.to_owned())
}

/// Maven and nuget packages are not imported by name, so only the Bazel graph can tell.
fn graph_only(ecosystem: Ecosystem) -> bool {
    matches!(ecosystem, Ecosystem::Maven | Ecosystem::Nuget)
}

/// The hub of the `uv.project` whose pyproject is `manifest`.
fn uv_hub(workspace: &Path, manifest: &str) -> Option<String> {
    let text = std::fs::read_to_string(workspace.join("MODULE.bazel")).ok()?;
    let module = dx_module_bazel::ModuleFile::parse(&text).ok()?;
    let projects = module.uv_projects().ok()?;
    let project = projects.iter().find(|project| {
        dx_module_bazel::label_path(project.pyproject).is_ok_and(|p| p == manifest)
    })?;
    Some(project.hub_name.to_owned())
}

/// The `@paket.<group>` repositories of the generated `paket.<group>.bzl` locks.
fn paket_hubs(locks: &[String]) -> Vec<String> {
    locks
        .iter()
        .filter_map(|lock| lock.rsplit('/').next())
        .filter_map(|file| file.strip_suffix(".bzl"))
        .filter(|name| name.starts_with("paket."))
        .map(|name| format!("@{name}"))
        .collect()
}

/// Labels without repository and without a target that repeats the last package segment, so
/// `@pypi//numpy:numpy` and `@pypi//numpy` agree.
fn label_key(label: &str) -> Option<String> {
    let (_, rest) = label.split_once("//")?;
    let (package, name) = match rest.split_once(':') {
        Some(parts) => parts,
        None => (rest, rest.rsplit('/').next().unwrap_or(rest)),
    };
    if package.is_empty() {
        return Some(format!(":{name}"));
    }
    if package.rsplit('/').next() == Some(name) {
        return Some(package.to_owned());
    }
    Some(format!("{package}:{name}"))
}

fn manifest_dir(manifest: &str) -> &str {
    manifest.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Keys of the labels that some target in the workspace depends on, each with the first label
/// seen for it, or `None` when the set has no hub to ask or the query fails.
fn used_labels(
    target: &ResolvedTarget,
    manifest: &str,
    workspace: &Path,
    query: &dyn QueryRunner,
    startup: &[String],
) -> Option<BTreeMap<String, String>> {
    let scope = match target.set.ecosystem {
        Ecosystem::Cargo => {
            format!("@{}//...", cargo_fetch_repository(workspace, &target.set)?)
        }
        Ecosystem::Maven => {
            format!("@{}//:*", target.set.repository.as_deref()?)
        }
        Ecosystem::Uv => format!("@{}//...", uv_hub(workspace, manifest)?),
        Ecosystem::Npm => format!("//{}:*", manifest_dir(manifest)),
        Ecosystem::Nuget => {
            let hubs = paket_hubs(&target.set.locks);
            if hubs.is_empty() {
                return None;
            }
            hubs.iter()
                .map(|hub| format!("{hub}//..."))
                .collect::<Vec<_>>()
                .join(" + ")
        }
        _ => return None,
    };
    let expression = format!("deps(//..., 1) intersect ({scope})");
    let labels = run_label_query_file(&expression, &expression, workspace, query, startup).ok()?;
    if target.set.ecosystem == Ecosystem::Cargo {
        let mut used = BTreeMap::new();
        for label in &labels {
            if let Some(name) = crate_name(label) {
                used.entry(format!(":{name}")).or_insert_with(|| label.clone());
            }
        }
        return (!used.is_empty()).then_some(used);
    }
    let mut used = BTreeMap::new();
    for label in &labels {
        if let Some(key) = label_key(label) {
            used.entry(key).or_insert_with(|| label.clone());
        }
    }
    Some(used)
}

fn maven_declared(workspace: &Path, repository: &str) -> Option<Vec<dx_depcheck::DeclaredDep>> {
    let text = std::fs::read_to_string(workspace.join("MODULE.bazel")).ok()?;
    let module = dx_module_bazel::ModuleFile::parse(&text).ok()?;
    let mut out = Vec::new();
    for artifact in module.maven_artifacts_of(repository).ok()? {
        let mut parts = artifact.value.split(':');
        let (Some(group), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        let line = text[..artifact.span.start].matches('\n').count() + 1;
        if text
            .lines()
            .nth(line - 1)
            .is_some_and(|l| l.contains("# dx-keep"))
        {
            continue;
        }
        let coordinate = format!("{group}:{name}");
        out.push(dx_depcheck::DeclaredDep {
            name: coordinate.clone(),
            hub: coordinate,
            line,
        });
    }
    Some(out)
}

fn underscored(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The label keys under which the Bazel graph would show `dep`.
fn dep_keys(ecosystem: Ecosystem, dir: &str, dep: &dx_depcheck::DeclaredDep) -> Vec<String> {
    match ecosystem {
        Ecosystem::Cargo => vec![
            format!(":{}", dep.hub),
            format!(":{}", dep.hub.replace('-', "_")),
            format!(":{}", dep.hub.replace('_', "-")),
        ],
        Ecosystem::Uv => vec![dx_depcheck::normalize_py(&dep.hub)],
        Ecosystem::Npm => vec![format!("{dir}:node_modules/{}", dep.hub)],
        Ecosystem::Maven => vec![format!(":{}", underscored(&dep.hub))],
        Ecosystem::Nuget => vec![dep.hub.to_lowercase()],
        _ => Vec::new(),
    }
}

/// Declared dependencies no BUILD target depends on, read from the Bazel graph.
fn find_unused_in_graph(
    target: &ResolvedTarget,
    eco: dx_depcheck::Ecosystem,
    manifest: &str,
    path: &Path,
    workspace: &Path,
    used: &BTreeMap<String, String>,
) -> Option<Vec<dx_depcheck::UnusedDep>> {
    let ecosystem = target.set.ecosystem;
    let declared = if ecosystem == Ecosystem::Maven {
        maven_declared(workspace, target.set.repository.as_deref()?)?
    } else {
        dx_depcheck::declared(eco, path).ok()?
    };
    let dir = manifest_dir(manifest);
    Some(
        declared
            .into_iter()
            .filter(|dep| {
                !dep_keys(ecosystem, dir, dep)
                    .iter()
                    .any(|key| used.contains_key(key))
            })
            .map(|dep| dx_depcheck::UnusedDep {
                name: dep.name,
                line: dep.line,
            })
            .collect(),
    )
}

/// Declared dependencies of the resolved sets that nothing uses: no BUILD target for sets with a
/// hub, no source under the manifest's directory otherwise.
pub(super) fn find(
    workspace: &Path,
    resolved: &[ResolvedTarget],
    query: &dyn QueryRunner,
    startup: &[String],
) -> Vec<Unused> {
    let exceptions =
        dx_depcheck::parse_set_exceptions(&workspace.join(EXCEPTIONS_FILE)).unwrap_or_default();
    let mut out = Vec::new();
    for target in resolved {
        let Some(eco) = depcheck_ecosystem(target.set.ecosystem) else {
            continue;
        };
        let set = target.set.name.as_str();
        let excepted: Vec<String> = exceptions
            .iter()
            .filter(|exception| exception.set == set)
            .map(|exception| exception.name.clone())
            .collect();
        for manifest in &target.set.manifests {
            let path = workspace.join(manifest);
            let root = path.parent().unwrap_or(workspace);
            let from_graph = used_labels(target, manifest, workspace, query, startup)
                .and_then(|used| {
                    find_unused_in_graph(target, eco, manifest, &path, workspace, &used)
                })
                .map(|found| {
                    found
                        .into_iter()
                        .filter(|dep| !excepted.contains(&dep.name))
                        .collect()
                });
            let found = match from_graph {
                Some(found) => found,
                None if graph_only(target.set.ecosystem) => continue,
                None => match dx_depcheck::find_unused(eco, &path, root, &excepted) {
                    Ok(found) => found,
                    Err(_) => continue,
                },
            };
            for dep in found {
                let wanted = match &target.request {
                    Selection::Full => true,
                    Selection::Packages(names) => names.contains(&dep.name),
                };
                if wanted {
                    out.push(Unused {
                        set: set.to_owned(),
                        name: dep.name,
                        manifest: manifest.clone(),
                        line: dep.line,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| (&a.set, &a.name).cmp(&(&b.set, &b.name)));
    out.dedup();
    out
}

fn lock_file_name(ecosystem: Ecosystem) -> &'static str {
    match ecosystem {
        Ecosystem::Cargo => "Cargo.lock",
        Ecosystem::Npm => "pnpm-lock.yaml",
        Ecosystem::Uv => "uv.lock",
        _ => "Gemfile.lock",
    }
}

/// Imports under the manifest's directory that no declared dependency covers, and hub labels
/// that BUILD targets use but the manifest does not declare. Go reads `go.mod`; the other sets
/// compare their lock's packages with the manifest.
pub(super) fn find_missing(
    workspace: &Path,
    resolved: &[ResolvedTarget],
    query: &dyn QueryRunner,
    startup: &[String],
) -> Vec<Missing> {
    let exceptions =
        dx_depcheck::parse_set_exceptions(&workspace.join(EXCEPTIONS_FILE)).unwrap_or_default();
    let mut out = Vec::new();
    for target in resolved {
        let Some(eco) = depcheck_ecosystem(target.set.ecosystem) else {
            continue;
        };
        let ecosystem = target.set.ecosystem;
        let set = target.set.name.as_str();
        let excepted: Vec<String> = exceptions
            .iter()
            .filter(|exception| exception.set == set)
            .map(|exception| exception.name.clone())
            .collect();
        if ecosystem == Ecosystem::Cargo {
            missing_hubs(workspace, target, eco, &excepted, query, startup, &mut out);
        }
        let lock = target.set.locks.iter().find(|lock| {
            Path::new(lock.as_str())
                .file_name()
                .and_then(|n| n.to_str())
                == Some(lock_file_name(ecosystem))
        });
        if ecosystem != Ecosystem::Go && lock.is_none() {
            continue;
        }
        for manifest in &target.set.manifests {
            let path = workspace.join(manifest);
            let root = path.parent().unwrap_or(workspace);
            let found = match lock {
                Some(lock) if ecosystem != Ecosystem::Go => {
                    dx_depcheck::find_missing(eco, &path, &workspace.join(lock), root, &excepted)
                }
                _ => dx_depcheck::find_missing_go(&path, root, &excepted),
            };
            let Ok(found) = found else {
                continue;
            };
            let dir = Path::new(manifest).parent().unwrap_or(Path::new(""));
            for dep in found {
                let wanted = match &target.request {
                    Selection::Full => true,
                    Selection::Packages(names) => names.contains(&dep.name),
                };
                if !wanted {
                    continue;
                }
                out.push(Missing {
                    set: set.to_owned(),
                    name: dep.name,
                    via: Via::Import {
                        file: dir.join(&dep.file).to_string_lossy().replace('\\', "/"),
                        line: dep.line,
                    },
                });
            }
        }
    }
    out.sort_by(|a, b| (&a.set, &a.name).cmp(&(&b.set, &b.name)));
    out.dedup_by(|later, kept| {
        let same = later.set == kept.set && later.name == kept.name;
        if same && matches!(later.via, Via::Label(_)) {
            std::mem::swap(later, kept);
        }
        same
    });
    out
}

/// Hub labels that a BUILD target depends on and the manifest does not declare.
fn missing_hubs(
    workspace: &Path,
    target: &ResolvedTarget,
    eco: dx_depcheck::Ecosystem,
    excepted: &[String],
    query: &dyn QueryRunner,
    startup: &[String],
    out: &mut Vec<Missing>,
) {
    let ecosystem = target.set.ecosystem;
    for manifest in &target.set.manifests {
        let Some(used) = used_labels(target, manifest, workspace, query, startup) else {
            continue;
        };
        let Ok(declared) = dx_depcheck::declared(eco, &workspace.join(manifest)) else {
            continue;
        };
        let dir = manifest_dir(manifest);
        let known: Vec<String> = declared
            .iter()
            .flat_map(|dep| dep_keys(ecosystem, dir, dep))
            .collect();
        for (key, label) in &used {
            let name = key.trim_start_matches(':');
            if known.contains(key)
                || name.ends_with(".bzl")
                || name.ends_with(".bazel")
                || excepted.iter().any(|e| e == name)
            {
                continue;
            }
            let wanted = match &target.request {
                Selection::Full => true,
                Selection::Packages(names) => names.iter().any(|n| n == name),
            };
            if !wanted {
                continue;
            }
            let expression = format!("rdeps(//..., {label}, 1) - {label}");
            let user = run_label_query_file(&expression, &expression, workspace, query, startup)
                .ok()
                .and_then(|labels| labels.into_iter().next());
            out.push(Missing {
                set: target.set.name.clone(),
                name: name.to_owned(),
                via: Via::Label(user.unwrap_or_else(|| label.clone())),
            });
        }
    }
}

pub(super) fn emit_missing_text(out: &mut dyn Write, missing: &[Missing]) -> Result<(), i32> {
    for item in missing {
        check_stdout_write(writeln!(
            out,
            "missing: {}: {} ({})",
            item.set,
            item.name,
            item.via.describe()
        ))?;
    }
    Ok(())
}

pub(super) fn emit_missing_json(out: &mut dyn Write, missing: &[Missing]) -> Result<(), i32> {
    for item in missing {
        emit_built(
            out,
            notice_event(&NoticeEvent {
                scope: Some(vec![format!("{}:{}", item.set, item.name)]),
                ..NoticeEvent::warning(
                    CODE_UPDATE_MISSING,
                    format!(
                        "{} is {} but not declared ({})",
                        item.name,
                        match item.via {
                            Via::Import { .. } => "imported",
                            Via::Label(_) => "used",
                        },
                        item.via.describe()
                    ),
                )
            }),
        )?;
    }
    Ok(())
}

pub(super) fn emit_text(out: &mut dyn Write, unused: &[Unused]) -> Result<(), i32> {
    for item in unused {
        check_stdout_write(writeln!(
            out,
            "unused: {}: {} (declared in {}:{})",
            item.set, item.name, item.manifest, item.line
        ))?;
    }
    Ok(())
}

pub(super) fn emit_json(out: &mut dyn Write, unused: &[Unused]) -> Result<(), i32> {
    for item in unused {
        emit_built(
            out,
            notice_event(&NoticeEvent {
                scope: Some(vec![format!("{}:{}", item.set, item.name)]),
                ..NoticeEvent::warning(
                    CODE_UPDATE_UNUSED,
                    format!(
                        "{} is unused (declared in {}:{})",
                        item.name, item.manifest, item.line
                    ),
                )
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_adopt::dependency_sets::{DependencySet, ResolvedSet};

    fn target(set: &str, ecosystem: Ecosystem, manifest: &str) -> ResolvedTarget {
        ResolvedTarget {
            set: ResolvedSet::of(&DependencySet {
                name: set.to_owned(),
                ecosystem,
                manifests: vec![manifest.to_owned()],
                locks: Vec::new(),
                scopes: Vec::new(),
                repository: None,
            }),
            request: Selection::Full,
        }
    }

    struct NoQuery;

    impl QueryRunner for NoQuery {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &Path,
        ) -> std::io::Result<dx_plan::resolve::QueryResult> {
            Err(std::io::Error::other("no bazel"))
        }
    }

    struct Labels(&'static str);

    impl QueryRunner for Labels {
        fn run_query(
            &self,
            _argv: &[String],
            _cwd: &Path,
        ) -> std::io::Result<dx_plan::resolve::QueryResult> {
            Ok(dx_plan::resolve::QueryResult {
                code: Some(0),
                stdout: self.0.as_bytes().to_vec(),
                stderr: Vec::new(),
            })
        }
    }

    fn cargo_set(root: &Path, manifest: &str) -> Vec<ResolvedTarget> {
        cargo_set_named(root, manifest, "crates")
    }

    fn cargo_set_named(root: &Path, manifest: &str, visible: &str) -> Vec<ResolvedTarget> {
        write(root, "rs/Cargo.toml", manifest);
        write(
            root,
            "MODULE.bazel",
            &format!(
                "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"dx_crates\", cargo_lockfile = \"//rs:Cargo.lock\", manifests = [\"//rs:Cargo.toml\"])\nuse_repo(crate, {visible} = \"dx_crates\")\n"
            ),
        );
        let mut resolved = vec![target("crates", Ecosystem::Cargo, "rs/Cargo.toml")];
        resolved[0].set.repository = Some("dx_crates".to_owned());
        resolved[0].set.locks = vec!["rs/Cargo.lock".to_owned()];
        resolved
    }

    struct Recorded(std::sync::Mutex<Vec<String>>, &'static str);

    impl QueryRunner for Recorded {
        fn run_query(
            &self,
            argv: &[String],
            _cwd: &Path,
        ) -> std::io::Result<dx_plan::resolve::QueryResult> {
            for arg in argv {
                if let Some(file) = arg.strip_prefix("--query_file=") {
                    self.0
                        .lock()
                        .unwrap()
                        .push(std::fs::read_to_string(file).unwrap());
                }
            }
            Labels(self.1).run_query(argv, _cwd)
        }
    }

    #[test]
    fn crate_name_strips_a_trailing_version() {
        let name = |label: &str| crate_name(label).unwrap();
        assert_eq!(
            name("@@rules_rust++crate+crates//serde-1.0.229:serde-1.0.229"),
            "serde"
        );
        assert_eq!(
            name("@@rules_rust++crate+crates//x-0.3.0-alpha.1:x-0.3.0-alpha.1"),
            "x"
        );
        assert_eq!(name("@@rules_rust++crate+crates//:cc"), "cc");
        assert_eq!(name("@crates//sha-1-0.10.1:sha-1-0.10.1"), "sha-1");
        assert_eq!(name("@crates//:sha-1"), "sha-1");
    }

    #[test]
    fn graph_reads_per_crate_subpackage_labels() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let manifest =
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\ncc = \"1\"\nregex = \"1\"\n";
        let resolved = cargo_set(root, manifest);
        let query = Labels(
            "@@rules_rust++crate+crates//serde-1.0.229:serde-1.0.229\n@@rules_rust++crate+crates//:cc\n",
        );
        let found = find(root, &resolved, &query, &[]);
        let names: Vec<&str> = found.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["regex"]);
    }

    #[test]
    fn query_uses_the_use_repo_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let resolved = cargo_set_named(
            root,
            "[package]\nname = \"x\"\n\n[dependencies]\ncc = \"1\"\n",
            "deps",
        );
        let query = Recorded(Default::default(), "@@m++crate+dx_crates//:cc\n");
        assert!(find(root, &resolved, &query, &[]).is_empty());
        let argv = query.0.lock().unwrap().join(" ");
        assert!(argv.contains("@deps//..."), "{argv}");
    }

    #[test]
    fn query_without_hub_labels_falls_back_to_import_scanning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let resolved = cargo_set(
            root,
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nregex = \"1\"\n",
        );
        write(root, "rs/src/lib.rs", "use serde::Serialize;\n");
        let found = find(root, &resolved, &Labels("//app:lib\n"), &[]);
        let names: Vec<&str> = found.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["regex"]);
    }

    #[test]
    fn graph_keeps_a_build_script_crate_and_dx_keep_silences() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let manifest = "[package]\nname = \"x\"\n\n[dependencies]\ncc = \"1\"\nregex = \"1\"\n";
        let resolved = cargo_set(root, manifest);
        let query = Labels("@crates//:cc\n");
        let found = find(root, &resolved, &query, &[]);
        let names: Vec<&str> = found.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["regex"]);
        let kept = manifest.replace("regex = \"1\"", "regex = \"1\" # dx-keep");
        let resolved = cargo_set(root, &kept);
        assert!(find(root, &resolved, &query, &[]).is_empty());
    }

    fn unused_names(root: &Path, resolved: &[ResolvedTarget], labels: &'static str) -> Vec<String> {
        find(root, resolved, &Labels(labels), &[])
            .into_iter()
            .map(|u| u.name)
            .collect()
    }

    #[test]
    fn uv_set_asks_its_pip_hub() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "MODULE.bazel",
            "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"pypi\", lock = \"//py:uv.lock\", pyproject = \"//py:pyproject.toml\")\n",
        );
        write(
            root,
            "py/pyproject.toml",
            "[project]\nname = \"x\"\nversion = \"1\"\ndependencies = [\"numpy>=1\", \"Requests\"]\n",
        );
        let resolved = [target("pypi", Ecosystem::Uv, "py/pyproject.toml")];
        assert_eq!(
            unused_names(root, &resolved, "@@pypi//numpy:numpy\n"),
            ["Requests"]
        );
    }

    #[test]
    fn npm_set_asks_its_manifest_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "web/package.json",
            "{\n  \"dependencies\": {\n    \"left-pad\": \"1.0.0\",\n    \"react\": \"18.0.0\"\n  }\n}\n",
        );
        let resolved = [target("web", Ecosystem::Npm, "web/package.json")];
        assert_eq!(
            unused_names(root, &resolved, "//web:node_modules/react\n"),
            ["left-pad"]
        );
    }

    #[test]
    fn maven_set_reads_artifacts_and_asks_its_repository() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "MODULE.bazel",
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    name = \"maven\",\n    artifacts = [\n        \"com.google.guava:guava:33.0.0\",\n        \"org.slf4j:slf4j-api:2.0.0\",\n    ],\n)\n",
        );
        let mut resolved = [target("maven", Ecosystem::Maven, "MODULE.bazel")];
        resolved[0].set.repository = Some("maven".to_owned());
        let found = find(
            root,
            &resolved,
            &Labels("@maven//:com_google_guava_guava\n"),
            &[],
        );
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].name.as_str(), found[0].line),
            ("org.slf4j:slf4j-api", 6)
        );
        assert!(find(root, &resolved, &NoQuery, &[]).is_empty());
    }

    #[test]
    fn nuget_set_asks_its_paket_hubs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "dn/paket.dependencies",
            "source https://api.nuget.org/v3/index.json\nnuget Newtonsoft.Json 13.0.1\nnuget Serilog 3.0.0\n",
        );
        let mut resolved = [target("nuget", Ecosystem::Nuget, "dn/paket.dependencies")];
        resolved[0].set.locks = vec![
            "dn/paket.lock".to_owned(),
            "dn/deps/paket.main.bzl".to_owned(),
        ];
        assert_eq!(
            unused_names(
                root,
                &resolved,
                "@paket.main//newtonsoft.json:newtonsoft.json\n"
            ),
            ["Serilog"]
        );
    }

    #[test]
    fn failed_query_falls_back_to_import_scanning() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let resolved = cargo_set(
            root,
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nregex = \"1\"\n",
        );
        write(root, "rs/src/lib.rs", "use serde::Serialize;\n");
        let found = find(root, &resolved, &NoQuery, &[]);
        let names: Vec<&str> = found.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["regex"]);
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn lists_an_unused_crate_and_an_unimported_package() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "rs/Cargo.toml",
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\nregex = \"1\"\n",
        );
        write(root, "rs/src/lib.rs", "use serde::Serialize;\n");
        write(
            root,
            "web/package.json",
            "{\n  \"dependencies\": {\n    \"left-pad\": \"1.0.0\",\n    \"react\": \"18.0.0\"\n  }\n}\n",
        );
        write(root, "web/index.js", "import React from 'react';\n");
        let resolved = [
            target("crates", Ecosystem::Cargo, "rs/Cargo.toml"),
            target("npm", Ecosystem::Npm, "web/package.json"),
        ];
        let found = find(root, &resolved, &NoQuery, &[]);
        let mut out = Vec::new();
        emit_text(&mut out, &found).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out),
            "unused: crates: regex (declared in rs/Cargo.toml:6)\n\
             unused: npm: left-pad (declared in web/package.json:3)\n"
        );
        write(
            root,
            "depcheck.toml",
            "[[exception]]\nset = \"npm\"\nname = \"left-pad\"\nreason = \"loaded by a plugin\"\n",
        );
        assert_eq!(find(root, &resolved, &NoQuery, &[]).len(), 1);
    }

    #[test]
    fn lists_a_hub_label_the_manifest_does_not_declare() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let resolved = cargo_set(
            root,
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\n",
        );
        let query = Labels("@crates//:serde\n@crates//:regex\n");
        let found = find_missing(root, &resolved, &query, &[]);
        let mut out = Vec::new();
        emit_missing_text(&mut out, &found).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out),
            "missing: crates: regex (used by @crates//:serde)\n"
        );
        assert!(find_missing(root, &resolved, &NoQuery, &[]).is_empty());
    }

    #[test]
    fn a_crate_found_by_import_and_by_label_is_listed_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let resolved = cargo_set(
            root,
            "[package]\nname = \"x\"\n\n[dependencies]\nserde = \"1\"\n",
        );
        write(
            root,
            "rs/Cargo.lock",
            "version = 3\n\n[[package]]\nname = \"regex\"\nversion = \"1.0.0\"\n",
        );
        write(root, "rs/src/lib.rs", "use regex::Regex;\n");
        let by_import = find_missing(root, &resolved, &NoQuery, &[]);
        assert_eq!(by_import.len(), 1);
        assert!(matches!(by_import[0].via, Via::Import { .. }));
        let query = Labels("@crates//:regex\n");
        let found = find_missing(root, &resolved, &query, &[]);
        assert_eq!(found.len(), 1);
        assert!(matches!(found[0].via, Via::Label(_)));
    }

    #[test]
    fn lists_a_go_import_that_no_require_covers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "svc/go.mod", "module example.com/svc\n\ngo 1.22\n");
        write(
            root,
            "svc/main.go",
            "package main\n\nimport (\n\t\"fmt\"\n\t\"github.com/c/d\"\n)\n",
        );
        let resolved = [target("go", Ecosystem::Go, "svc/go.mod")];
        let found = find_missing(root, &resolved, &NoQuery, &[]);
        let mut out = Vec::new();
        emit_missing_text(&mut out, &found).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out),
            "missing: go: github.com/c/d (imported in svc/main.go:5)\n"
        );
    }
}
