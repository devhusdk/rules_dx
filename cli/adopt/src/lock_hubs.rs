//! Plans the `MODULE.bazel` stanzas for the uv and pnpm lock files anywhere in the repo and the
//! root cargo lock file. Other cargo lock files are reported.
//!
//! A lock file counts when its manifest sits next to it. A lock that an existing stanza
//! already names is left alone, so a second run plans nothing.

use std::path::Path;

use dx_families::Hub;
use dx_module_bazel::{Arg, ModuleFile, NewTag, Value};

use crate::AdoptError;
use crate::module_pin::ModuleEdit;

const MODULE_FILE: &str = "MODULE.bazel";

/// The planned `MODULE.bazel` edit, if any, and what the planner could not do.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct LockHubs {
    pub edit: Option<ModuleEdit>,
    pub notes: Vec<String>,
}

fn registry(ecosystem: &str) -> &'static Hub {
    dx_families::hub(ecosystem).unwrap_or_else(|| panic!("no setup.hubs entry for {ecosystem}"))
}

/// The lock file name of the `setup.hubs` entry of `ecosystem`.
pub fn lock_of(ecosystem: &str) -> &'static str {
    registry(ecosystem)
        .lock
        .as_deref()
        .unwrap_or_else(|| panic!("setup.hubs entry {ecosystem} has no lock"))
}

pub fn manifest_of(ecosystem: &str) -> &'static str {
    &registry(ecosystem).manifest
}

/// The state file name of the `setup.hubs` entry of `ecosystem`.
pub fn state_of(ecosystem: &str) -> &'static str {
    registry(ecosystem)
        .state
        .as_deref()
        .unwrap_or_else(|| panic!("setup.hubs entry {ecosystem} has no state"))
}

fn hub_of(ecosystem: &str) -> Result<&'static Hub, AdoptError> {
    dx_families::hub(ecosystem)
        .ok_or_else(|| refuse(format!("no setup.hubs entry for {ecosystem}")))
}

fn binds_name(file: &ModuleFile, eco: &Hub) -> bool {
    let bindings: Vec<&str> = file
        .hub_bindings(&eco.extension, &eco.ecosystem)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    file.calls_named("use_repo").any(|call| {
        matches!(call.positional.first(), Some(Value::Ident(v)) if bindings.contains(&v.as_str()))
            && (call.named.iter().any(|(alias, _)| *alias == eco.hub)
                || call
                    .positional
                    .iter()
                    .any(|v| matches!(v, Value::Str(s) if s.value == eco.hub)))
    })
}

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

fn root_label(file: &str) -> String {
    format!("//:{file}")
}

fn dir_label(dir: &str, file: &str) -> String {
    format!("//{dir}:{file}")
}

fn string(value: &str) -> Arg {
    Arg::Str(value.to_owned())
}

fn tag(name: &str, args: Vec<(&str, Arg)>) -> NewTag {
    NewTag {
        name: name.to_owned(),
        args: args
            .into_iter()
            .map(|(key, arg)| (key.to_owned(), arg))
            .collect(),
    }
}

/// The root manifests that `dx generate --apply` can lock: a cargo manifest without a cargo lock
/// (an empty one counts as missing), a uv manifest without a uv lock, and a pnpm manifest
/// without a pnpm lock.
pub fn unlocked_manifests(root: &Path) -> Vec<&'static str> {
    let mut out = Vec::new();
    let cargo_lock = std::fs::metadata(root.join(lock_of("cargo")));
    if root.join(manifest_of("cargo")).is_file() && cargo_lock.map_or(true, |meta| meta.len() == 0)
    {
        out.push(manifest_of("cargo"));
    }
    let pyproject = std::fs::read_to_string(root.join(manifest_of("uv")))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok());
    if let Some(table) = pyproject {
        let tool = table.get("tool").and_then(|tool| tool.as_table());
        let other =
            tool.is_some_and(|tool| tool.contains_key("poetry") || tool.contains_key("pdm"));
        if table.contains_key("project") && !other && !root.join(lock_of("uv")).exists() {
            out.push(manifest_of("uv"));
        }
    }
    if wants_pnpm_lock(root) {
        out.push(manifest_of("npm"));
    }
    out
}

/// True when the npm manifest at `path` lists a package in a dependency table.
fn declares_dependency(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    [
        "dependencies",
        "devDependencies",
        "optionalDependencies",
        "peerDependencies",
    ]
    .iter()
    .any(|table| {
        json.get(table)
            .and_then(|deps| deps.as_object())
            .is_some_and(|deps| !deps.is_empty())
    })
}

fn wants_pnpm_lock(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join(manifest_of("npm"))) else {
        return false;
    };
    let declared = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|json| json.get("packageManager")?.as_str().map(str::to_owned))
        .is_some_and(|manager| manager.starts_with("pnpm@"));
    let other_lock = ["package-lock.json", "yarn.lock", lock_of("npm")]
        .iter()
        .any(|lock| root.join(lock).exists());
    let workspace = root.join("pnpm-workspace.yaml").is_file();
    let depends = declares_dependency(&root.join(manifest_of("npm")));
    ((declared && depends) || workspace) && !other_lock
}

/// Plans the `pnpm` extension and `@pnpm` repo that `bazel run @pnpm//:pnpm` needs.
pub fn plan_pnpm_tool(root: &Path) -> Result<LockHubs, AdoptError> {
    plan_with(root, |plan| plan.pnpm())
}

/// Plans the stanzas for the lock files at the root of `root`. Writes nothing.
pub fn plan_lock_hubs(root: &Path) -> Result<LockHubs, AdoptError> {
    plan_with(root, |plan| {
        plan.report_missing_locks();
        plan.cargo()?;
        plan.uv()?;
        plan.npm()?;
        plan.pub_()?;
        plan.go()?;
        for ecosystem in LOCK_TAG_ECOSYSTEMS {
            plan.lock_tag(ecosystem)?;
        }
        Ok(())
    })
}

/// The ecosystems whose `setup.hubs` tag takes a `name` and a root `lock` label.
const LOCK_TAG_ECOSYSTEMS: [&str; 9] = [
    "composer", "luarocks", "dub", "zig", "swiftpm", "hex", "cabal", "opam", "cpan",
];

fn plan_with(
    root: &Path,
    run: impl FnOnce(&mut Planner) -> Result<(), AdoptError>,
) -> Result<LockHubs, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(LockHubs::default());
    };
    let read_include = |label: &str| -> Result<String, dx_error::DxError> {
        let rel = dx_module_bazel::include_rel("", label).map_err(dx_error::DxError::failure)?;
        std::fs::read_to_string(root.join(&rel))
            .map_err(|e| dx_error::DxError::failure(format!("cannot read {rel}: {e}")))
    };
    let loaded =
        dx_module_bazel::load_with_includes_labeled(&text, &read_include).map_err(refuse)?;
    let files: Vec<&ModuleFile> = loaded.iter().map(|(_, file)| file).collect();

    let mut plan = Planner {
        root,
        files: &files,
        text: text.clone(),
        notes: Vec::new(),
    };
    run(&mut plan)?;
    let edit = (plan.text != text).then(|| ModuleEdit {
        path: MODULE_FILE.to_owned(),
        old: text,
        new: plan.text.clone(),
    });
    Ok(LockHubs {
        edit,
        notes: plan.notes,
    })
}

struct Planner<'a> {
    root: &'a Path,
    files: &'a [&'a ModuleFile],
    text: String,
    notes: Vec<String>,
}

impl Planner<'_> {
    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).is_file()
    }

    fn report_missing_locks(&mut self) {
        let mut labels: Vec<String> = Vec::new();
        for file in self.files {
            for hub in file.cargo_hubs().unwrap_or_default() {
                labels.extend(hub.cargo_lockfile.map(str::to_owned));
            }
            for project in file.uv_projects().unwrap_or_default() {
                labels.push(project.lock.to_owned());
            }
            for lock in file.npm_translate_locks().unwrap_or_default() {
                labels.extend(lock.pnpm_lock.map(str::to_owned));
            }
        }
        for label in labels {
            if let Ok(path) = dx_module_bazel::label_path(&label)
                && !self.exists(&path)
            {
                self.notes
                    .push(format!("{MODULE_FILE} names {label}, but {path} is gone"));
            }
        }
    }

    fn report_extra_cargo_locks(&mut self) {
        for dir in self.lock_dirs(lock_of("cargo"), manifest_of("cargo")) {
            let label = dir_label(&dir, lock_of("cargo"));
            let named = self.files.iter().any(|file| {
                file.cargo_hubs().is_ok_and(|hubs| {
                    hubs.iter()
                        .any(|hub| hub.cargo_lockfile == Some(label.as_str()))
                })
            });
            if !dir.is_empty() && !named {
                self.notes.push(format!(
                    "{dir}/{} is not wired; add a crate.from_cargo for it",
                    lock_of("cargo")
                ));
            }
        }
    }

    fn cargo(&mut self) -> Result<(), AdoptError> {
        self.report_extra_cargo_locks();
        if !self.exists(lock_of("cargo")) || !self.exists(manifest_of("cargo")) {
            return Ok(());
        }
        let cargo = hub_of("cargo")?;
        let lock = root_label(lock_of("cargo"));
        let mut covered = false;
        for file in self.files {
            let Ok(hubs) = file.cargo_hubs() else {
                self.notes.push(format!(
                    "a crate.from_cargo has computed arguments; {} not wired",
                    lock_of("cargo")
                ));
                return Ok(());
            };
            for hub in hubs {
                covered |= hub.cargo_lockfile == Some(lock.as_str());
                if hub.name == cargo.hub && hub.cargo_lockfile != Some(lock.as_str()) {
                    self.notes.push(format!(
                        "crate.from_cargo \"{}\" reads another lock; {} not wired",
                        hub.name,
                        lock_of("cargo")
                    ));
                    return Ok(());
                }
            }
        }
        if covered {
            return Ok(());
        }
        let manifests = self.cargo_manifests();
        let tags = vec![tag(
            "from_cargo",
            vec![
                ("name", string(&cargo.hub)),
                ("cargo_lockfile", string(&lock)),
                ("manifests", Arg::List(manifests)),
            ],
        )];
        self.add(cargo, &tags, &[cargo.hub.as_str()])
    }

    fn cargo_manifests(&mut self) -> Vec<String> {
        let manifest = manifest_of("cargo");
        let mut manifests = vec![root_label(manifest)];
        let parsed = std::fs::read_to_string(self.root.join(manifest))
            .ok()
            .and_then(|text| text.parse::<toml::Table>().ok());
        let members = parsed
            .as_ref()
            .and_then(|table| table.get("workspace"))
            .and_then(|workspace| workspace.get("members"))
            .and_then(|members| members.as_array());
        for member in members.into_iter().flatten().filter_map(|m| m.as_str()) {
            let member = member.trim_matches('/');
            if member.contains(['*', '?', '[']) {
                self.notes.push(format!(
                    "{manifest} member \"{member}\" is a glob; add its {manifest} to crate.from_cargo manifests"
                ));
            } else if member != "." && self.exists(&format!("{member}/{manifest}")) {
                manifests.push(format!("//{member}:{manifest}"));
            } else if member != "." {
                self.notes
                    .push(format!("{manifest} member \"{member}\" has no {manifest}"));
            }
        }
        manifests
    }

    /// Directories under the root (as `""` or `a/b`) holding `lock` next to `manifest`.
    fn lock_dirs(&self, lock: &str, manifest: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut excluded: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut pending = vec![String::new()];
        while let Some(dir) = pending.pop() {
            let path = self.root.join(&dir);
            if !dir.is_empty() && (path.join(MODULE_FILE).is_file() || excluded.contains(&dir)) {
                continue;
            }
            if let Some(build) = ["BUILD.bazel", "BUILD"]
                .iter()
                .find_map(|name| std::fs::read_to_string(path.join(name)).ok())
            {
                for line in build.lines() {
                    let Some(rest) = line.trim().strip_prefix("# gazelle:exclude") else {
                        continue;
                    };
                    if let Some(target) = rest.split_whitespace().next() {
                        excluded.insert(if dir.is_empty() {
                            target.to_owned()
                        } else {
                            format!("{dir}/{target}")
                        });
                    }
                }
            }
            if path.join(lock).is_file() && path.join(manifest).is_file() {
                found.push(dir.clone());
            }
            let Ok(entries) = std::fs::read_dir(&path) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let skip = name.starts_with('.')
                    || name.starts_with("bazel-")
                    || matches!(
                        name.as_str(),
                        "node_modules"
                            | "target"
                            | "testdata"
                            | "fixtures"
                            | "fixture"
                            | "examples"
                    );
                if !skip && entry.file_type().is_ok_and(|t| t.is_dir()) {
                    pending.push(if dir.is_empty() {
                        name
                    } else {
                        format!("{dir}/{name}")
                    });
                }
            }
        }
        found.sort();
        found
    }

    fn uv(&mut self) -> Result<(), AdoptError> {
        let uv = hub_of("uv")?;
        let dirs = self.lock_dirs(lock_of("uv"), manifest_of("uv"));
        if dirs.is_empty() {
            return Ok(());
        }
        let mut named: Vec<String> = Vec::new();
        let mut declared = false;
        for file in self.files {
            if binds_name(file, uv) {
                return Ok(());
            }
            let Ok(projects) = file.uv_projects() else {
                self.notes.push(format!(
                    "a uv.project has computed arguments; {} not wired",
                    lock_of("uv")
                ));
                return Ok(());
            };
            named.extend(projects.iter().map(|project| project.lock.to_owned()));
            declared |= file
                .calls_named("uv.declare_hub")
                .any(|call| call.str("hub_name") == Some(uv.hub.as_str()));
        }
        let mut tags = Vec::new();
        for dir in dirs {
            let lock = dir_label(&dir, lock_of("uv"));
            if named.contains(&lock) {
                continue;
            }
            tags.push(tag(
                "project",
                vec![
                    ("hub_name", string(&uv.hub)),
                    ("pyproject", string(&dir_label(&dir, manifest_of("uv")))),
                    ("lock", string(&lock)),
                ],
            ));
        }
        if tags.is_empty() {
            return Ok(());
        }
        if !declared {
            tags.insert(0, tag("declare_hub", vec![("hub_name", string(&uv.hub))]));
        }
        self.add(uv, &tags, &[uv.hub.as_str()])
    }

    fn npm(&mut self) -> Result<(), AdoptError> {
        let npm = hub_of("npm")?;
        let dirs: Vec<String> = self
            .lock_dirs(lock_of("npm"), manifest_of("npm"))
            .into_iter()
            .filter(|dir| declares_dependency(&self.root.join(dir).join(manifest_of("npm"))))
            .collect();
        if dirs.is_empty() {
            return Ok(());
        }
        let mut existing: Vec<(String, Option<String>)> = Vec::new();
        for file in self.files {
            let Ok(locks) = file.npm_translate_locks() else {
                self.notes.push(format!(
                    "an npm.npm_translate_lock has computed arguments; {} not wired",
                    lock_of("npm")
                ));
                return Ok(());
            };
            existing.extend(
                locks
                    .iter()
                    .map(|l| (l.name.to_owned(), l.pnpm_lock.map(str::to_owned))),
            );
        }
        let mut tags = Vec::new();
        let mut hubs = Vec::new();
        for dir in dirs {
            let lock = dir_label(&dir, lock_of("npm"));
            let hub = if dir.is_empty() {
                npm.hub.clone()
            } else {
                format!("{}_{}", npm.hub, dir.replace('/', "_"))
            };
            if existing.iter().any(|(_, l)| l.as_deref() == Some(&lock)) {
                continue;
            }
            if existing.iter().any(|(name, _)| *name == hub) {
                self.notes.push(format!(
                    "npm.npm_translate_lock \"{hub}\" reads another lock; {} in {} not wired",
                    lock_of("npm"),
                    if dir.is_empty() { "." } else { &dir }
                ));
                continue;
            }
            tags.push(tag(
                "npm_translate_lock",
                vec![("name", string(&hub)), ("pnpm_lock", string(&lock))],
            ));
            hubs.push(hub);
        }
        if tags.is_empty() {
            return Ok(());
        }
        let hubs: Vec<&str> = hubs.iter().map(String::as_str).collect();
        self.add(npm, &tags, &hubs)
    }

    fn pub_(&mut self) -> Result<(), AdoptError> {
        let pubs = hub_of("pub")?;
        let dirs = self.lock_dirs(lock_of("pub"), &pubs.manifest);
        let Some(dir) = dirs.first() else {
            return Ok(());
        };
        let wired = self.files.iter().any(|file| {
            !file
                .hub_bindings(&pubs.extension, &pubs.ecosystem)
                .is_empty()
        });
        if wired {
            return Ok(());
        }
        for other in &dirs[1..] {
            self.notes.push(format!(
                "{other}/{} is not wired; add a pub.from_lock for it",
                lock_of("pub")
            ));
        }
        let tags = vec![tag(
            "from_lock",
            vec![
                ("name", string(&pubs.hub)),
                ("lock", string(&dir_label(dir, lock_of("pub")))),
            ],
        )];
        self.add(pubs, &tags, &[pubs.hub.as_str()])
    }

    fn go(&mut self) -> Result<(), AdoptError> {
        let go = hub_of("go")?;
        if !self.exists(&go.manifest) {
            return Ok(());
        }
        let manifest = root_label(&go.manifest);
        for file in self.files {
            let Ok(named) = file.go_deps_files() else {
                self.notes.push(format!(
                    "a go_deps.from_file has computed arguments; {} not wired",
                    go.manifest
                ));
                return Ok(());
            };
            if named.iter().any(|f| f.go_mod == manifest) {
                return Ok(());
            }
        }
        let tags = vec![tag("from_file", vec![("go_mod", string(&manifest))])];
        self.add(go, &tags, &[])
    }

    /// The root file matching `pattern`, which is a file name or `*<suffix>`.
    fn root_file(&self, pattern: &str) -> Option<String> {
        let Some(suffix) = pattern.strip_prefix('*') else {
            return self.exists(pattern).then(|| pattern.to_owned());
        };
        let mut names: Vec<String> = std::fs::read_dir(self.root)
            .ok()?
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(suffix) && name != suffix)
            .collect();
        names.sort();
        names.into_iter().next()
    }

    fn lock_tag(&mut self, ecosystem: &str) -> Result<(), AdoptError> {
        let eco = hub_of(ecosystem)?;
        let (Some(lock_pattern), Some(tag_name)) = (eco.lock.as_deref(), eco.tag.as_deref()) else {
            return Ok(());
        };
        let (Some(lock), Some(_)) = (self.root_file(lock_pattern), self.root_file(&eco.manifest))
        else {
            return Ok(());
        };
        if self
            .files
            .iter()
            .any(|file| !file.hub_bindings(&eco.extension, &eco.ecosystem).is_empty())
        {
            return Ok(());
        }
        let tags = vec![tag(
            tag_name,
            vec![
                ("name", string(&eco.hub)),
                ("lock", string(&root_label(&lock))),
            ],
        )];
        self.add(eco, &tags, &[eco.hub.as_str()])
    }

    fn pnpm(&mut self) -> Result<(), AdoptError> {
        let pnpm = hub_of("pnpm")?;
        if self.files.iter().any(|file| binds_name(file, pnpm)) {
            return Ok(());
        }
        self.add(pnpm, &[], &[pnpm.hub.as_str()])
    }

    fn add(&mut self, eco: &Hub, tags: &[NewTag], hubs: &[&str]) -> Result<(), AdoptError> {
        let file = ModuleFile::parse(&self.text).map_err(refuse)?;
        let binding = match file
            .extension_bindings(&eco.extension, &eco.ecosystem)
            .first()
        {
            Some((name, _)) => (*name).to_owned(),
            None => {
                let taken: Vec<&str> = file
                    .calls_named("use_extension")
                    .filter_map(|call| call.binding.as_deref())
                    .collect();
                if taken.contains(&eco.ecosystem.as_str()) {
                    (1..)
                        .map(|n| match n {
                            1 => format!("dx_{}", eco.ecosystem),
                            n => format!("dx_{}_{n}", eco.ecosystem),
                        })
                        .find(|name| !taken.contains(&name.as_str()))
                        .unwrap_or_default()
                } else {
                    eco.ecosystem.clone()
                }
            }
        };
        self.text = file
            .insert_extension_block(&binding, &eco.extension, &eco.ecosystem, tags, hubs)
            .map_err(refuse)?;
        Ok(())
    }
}

/// What a root `maven.install` lock needs before Bazel and Gazelle can read it: the
/// lock's package `BUILD.bazel`, the lock file itself, and the root `dx_external` line.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct MavenLocks {
    /// Files to create or change, in the order to write them.
    pub edits: Vec<ModuleEdit>,
    /// `(lock path, repository)` for each lock that is missing or empty, to fill with
    /// `bazel run @<repository>//:pin`.
    pub seeds: Vec<(String, String)>,
    pub notes: Vec<String>,
}

const ROOT_BUILD: &str = "BUILD.bazel";

/// Plans the files the root `maven.install` locks need. Only installs with a literal
/// `//package:file` lock and a plain `use_repo(maven, "<name>")` are wired. Writes nothing.
pub fn plan_maven_locks(root: &Path) -> Result<MavenLocks, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(MavenLocks::default());
    };
    let file = ModuleFile::parse(&text).map_err(refuse)?;
    let mut plan = MavenLocks::default();
    let Ok(installs) = file.maven_installs() else {
        plan.notes
            .push("a maven.install has computed arguments; its lock is not wired".into());
        return Ok(plan);
    };
    let proxies = file.maven_proxies();
    let old_build = std::fs::read_to_string(root.join(ROOT_BUILD)).unwrap_or_default();
    let mut new_build = old_build.clone();
    let mut packages: Vec<(String, Vec<String>)> = Vec::new();
    for install in installs {
        let Some(label) = install.lock_file else {
            continue;
        };
        let Ok(path) = dx_module_bazel::label_path(label) else {
            continue;
        };
        match maven_repo_import(&file, &proxies, install.name) {
            Some(MavenImport::Plain) => {}
            Some(MavenImport::Alias(alias)) => {
                plan.notes.push(format!(
                    "use_repo imports {} as {alias}; add the BUILD file, lock and dx_external line for {path} by hand",
                    install.name
                ));
                continue;
            }
            None => continue,
        }
        let empty = std::fs::metadata(root.join(&path)).map_or(true, |meta| meta.len() == 0);
        if empty {
            plan.seeds.push((path.clone(), install.name.to_owned()));
        }
        let Some((dir, name)) = path.rsplit_once('/') else {
            continue;
        };
        if !root.join(dir).join("BUILD.bazel").exists() && !root.join(dir).join("BUILD").exists() {
            match packages.iter_mut().find(|(known, _)| known == dir) {
                Some((_, names)) => names.push(name.to_owned()),
                None => packages.push((dir.to_owned(), vec![name.to_owned()])),
            }
        }
        if !declares_external(&old_build, &path) {
            if !new_build.is_empty() && !new_build.ends_with('\n') {
                new_build.push('\n');
            }
            new_build.push_str(&format!(
                "# gazelle:dx_external maven {path} {}\n",
                install.name
            ));
        }
    }
    for (dir, names) in packages {
        let names: Vec<String> = names.iter().map(|name| format!("{name:?}")).collect();
        plan.edits.push(ModuleEdit {
            path: format!("{dir}/BUILD.bazel"),
            old: String::new(),
            new: format!("exports_files([{}])\n", names.join(", ")),
        });
    }
    if new_build != old_build {
        plan.edits.push(ModuleEdit {
            path: ROOT_BUILD.to_owned(),
            old: old_build,
            new: new_build,
        });
    }
    Ok(plan)
}

enum MavenImport {
    Plain,
    Alias(String),
}

fn maven_repo_import(file: &ModuleFile, proxies: &[&str], repo: &str) -> Option<MavenImport> {
    for call in file.calls_named("use_repo") {
        if !matches!(call.positional.first(), Some(Value::Ident(v)) if proxies.contains(&v.as_str()))
        {
            continue;
        }
        if call.positional[1..]
            .iter()
            .any(|value| matches!(value, Value::Str(s) if s.value == repo))
        {
            return Some(MavenImport::Plain);
        }
        for (alias, value) in &call.named {
            if matches!(value, Value::Str(s) if s.value == repo) {
                return Some(MavenImport::Alias(alias.clone()));
            }
        }
    }
    None
}

/// Whether `build` already has a maven `dx_external` line for `lock`.
fn declares_external(build: &str, lock: &str) -> bool {
    build.lines().any(|line| {
        let Some(rest) = line.trim().strip_prefix("# gazelle:dx_external") else {
            return false;
        };
        let mut fields = rest.split_whitespace();
        fields.next() == Some("maven")
            && fields
                .next()
                .is_some_and(|path| path.trim_start_matches("//") == lock)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};

    fn t(template: &str) -> &'static str {
        let mut text = template.to_owned();
        for eco in ["cargo", "npm", "uv"] {
            text = text.replace(&format!("{{{eco}:manifest}}"), manifest_of(eco));
            text = text.replace(&format!("{{{eco}:lock}}"), lock_of(eco));
        }
        Box::leak(text.into_boxed_str())
    }

    const MODULE: &str =
        "module(name = \"app\")\n\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n";

    fn plan(files: &[(&str, &str)]) -> LockHubs {
        let dir = scratch("lock_hubs");
        write(dir.path(), MODULE_FILE, MODULE);
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        plan_lock_hubs(dir.path()).expect("plan")
    }

    fn new_text(hubs: &LockHubs) -> &str {
        &hubs.edit.as_ref().expect("an edit").new
    }

    #[test]
    fn cargo_lock_gets_a_crates_hub_with_workspace_members() {
        let hubs = plan(&[
            (t("{cargo:lock}"), ""),
            (
                t("{cargo:manifest}"),
                "[workspace]\nmembers = [\"app\", \"libs/*\", \"gone\"]\n",
            ),
            (t("app/{cargo:manifest}"), ""),
        ]);
        let text = new_text(&hubs);
        assert!(text.starts_with(MODULE), "{text}");
        assert!(!text.contains("bazel_dep(name = \"rules_rust\""), "{text}");
        assert!(
            text.contains(
                t("cargo.from_cargo(\n    name = \"crates\",\n    cargo_lockfile = \"//:{cargo:lock}\",\n    manifests = [\n        \"//:{cargo:manifest}\",\n        \"//app:{cargo:manifest}\",\n    ],\n)\nuse_repo(cargo, \"crates\")\n")
            ),
            "{text}"
        );
        assert_eq!(hubs.notes.len(), 2, "{:?}", hubs.notes);
        assert!(hubs.notes[0].contains("libs/*"), "{:?}", hubs.notes);
        assert!(hubs.notes[1].contains("gone"), "{:?}", hubs.notes);
    }

    #[test]
    fn pubspec_lock_gets_a_pub_deps_hub() {
        let hubs = plan(&[(lock_of("pub"), ""), (manifest_of("pub"), "")]);
        let text = new_text(&hubs);
        assert!(!text.contains("bazel_dep(name = \"rules_dart\""), "{text}");
        assert!(
            text.contains(&format!("pub.from_lock(\n    name = \"pub_deps\",\n    lock = \"//:{}\",\n)\nuse_repo(pub, \"pub_deps\")\n", lock_of("pub"))),
            "{text}"
        );
    }

    #[test]
    fn lock_tag_ecosystems_get_a_hub_each() {
        for ecosystem in LOCK_TAG_ECOSYSTEMS {
            let eco = hub_of(ecosystem).expect("hub");
            let lock = eco.lock.as_deref().expect("lock").replace('*', "app");
            let manifest = eco.manifest.replace('*', "app");
            let hubs = plan(&[(lock.as_str(), ""), (manifest.as_str(), "")]);
            let text = new_text(&hubs);
            let tag = eco.tag.as_deref().expect("tag");
            assert!(
                text.contains(&format!(
                    "{}.{tag}(\n    name = \"{}\",\n    lock = \"//:{lock}\",\n)\nuse_repo({}, \"{}\")\n",
                    eco.ecosystem, eco.hub, eco.ecosystem, eco.hub
                )),
                "{ecosystem}: {text}"
            );
        }
    }

    #[test]
    fn a_wired_lock_tag_ecosystem_is_left_alone() {
        let dir = scratch("lock_hubs");
        let module = format!(
            "{MODULE}hex = use_extension(\"@rules_dx//elixir:extensions.bzl\", \"hex\")\nhex.lock(name = \"deps\", lock = \"//:mix.lock\")\nuse_repo(hex, \"deps\")\n"
        );
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), lock_of("hex"), "");
        write(dir.path(), manifest_of("hex"), "");
        assert_eq!(plan_lock_hubs(dir.path()).expect("plan").edit, None);
    }

    #[test]
    fn a_lock_tag_lock_without_its_manifest_is_ignored() {
        let hubs = plan(&[(lock_of("hex"), "")]);
        assert_eq!(hubs.edit, None);
    }

    #[test]
    fn go_mod_gets_a_go_from_file_tag() {
        let hubs = plan(&[(manifest_of("go"), "module example.com/app\n")]);
        let text = new_text(&hubs);
        assert!(
            text.contains(&format!(
                "go = use_extension(\"@rules_dx//go:extensions.bzl\", \"go\")\ngo.from_file(go_mod = \"//:{}\")\n",
                manifest_of("go")
            )) || text.contains(&format!("go.from_file(\n    go_mod = \"//:{}\",\n)\n", manifest_of("go"))),
            "{text}"
        );
        assert!(!text.contains("use_repo"), "{text}");
    }

    #[test]
    fn a_wired_go_mod_is_left_alone() {
        let dir = scratch("lock_hubs");
        write(
            dir.path(),
            MODULE_FILE,
            &format!(
                "{MODULE}go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//:{}\")\n",
                manifest_of("go")
            ),
        );
        write(dir.path(), manifest_of("go"), "module example.com/app\n");
        assert_eq!(plan_lock_hubs(dir.path()).expect("plan").edit, None);
    }

    #[test]
    fn a_pub_extension_from_this_repository_is_left_alone() {
        let dir = scratch("lock_hubs");
        write(
            dir.path(),
            MODULE_FILE,
            &format!(
                "{MODULE}pub = use_extension(\"//dart:extensions.bzl\", \"pub\")\npub.from_lock(\n    name = \"pub_deps\",\n    lock = \"//:{}\",\n)\nuse_repo(pub, \"pub_deps\")\n",
                lock_of("pub")
            ),
        );
        write(dir.path(), lock_of("pub"), "");
        write(dir.path(), manifest_of("pub"), "");
        assert_eq!(plan_lock_hubs(dir.path()).expect("plan").edit, None);
    }

    #[test]
    fn pubspec_lock_without_its_manifest_is_left_alone() {
        assert_eq!(plan(&[(lock_of("pub"), "")]).edit, None);
    }

    fn unlocked(files: &[(&str, &str)]) -> Vec<&'static str> {
        let dir = scratch("unlocked");
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        unlocked_manifests(dir.path())
    }

    #[test]
    fn a_nested_module_keeps_its_own_locks() {
        let hubs = plan(&[
            ("sub/MODULE.bazel", ""),
            (
                t("sub/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
            (t("sub/{npm:lock}"), ""),
            (t("sub/{cargo:manifest}"), ""),
            (t("sub/{cargo:lock}"), ""),
        ]);
        assert!(hubs.edit.is_none());
        assert!(hubs.notes.is_empty(), "{:?}", hubs.notes);
    }

    #[test]
    fn a_missing_or_empty_cargo_lock_is_unlocked() {
        assert_eq!(
            unlocked(&[(t("{cargo:manifest}"), "")]),
            [t("{cargo:manifest}")]
        );
        assert_eq!(
            unlocked(&[(t("{cargo:manifest}"), ""), (t("{cargo:lock}"), "")]),
            [t("{cargo:manifest}")]
        );
        assert!(
            unlocked(&[
                (t("{cargo:manifest}"), ""),
                (t("{cargo:lock}"), "version = 4\n")
            ])
            .is_empty()
        );
        assert!(unlocked(&[(t("{cargo:lock}"), "")]).is_empty());
    }

    #[test]
    fn only_uv_pyprojects_without_a_lock_are_unlocked() {
        let project = "[project]\nname = \"app\"\n";
        assert_eq!(
            unlocked(&[(t("{uv:manifest}"), project)]),
            [t("{uv:manifest}")]
        );
        assert!(unlocked(&[(t("{uv:manifest}"), project), (t("{uv:lock}"), "")]).is_empty());
        assert!(unlocked(&[(t("{uv:manifest}"), "[tool.ruff]\n")]).is_empty());
        for tool in ["[tool.poetry]\n", "[tool.pdm]\n"] {
            let text = format!("{project}{tool}");
            assert!(
                unlocked(&[(t("{uv:manifest}"), &text)]).is_empty(),
                "{tool}"
            );
        }
    }

    #[test]
    fn only_pnpm_packages_without_a_lock_are_unlocked() {
        let pnpm =
            "{\"packageManager\": \"pnpm@9.1.0\", \"dependencies\": {\"left-pad\": \"1.3.0\"}}";
        assert_eq!(
            unlocked(&[(t("{npm:manifest}"), pnpm)]),
            [t("{npm:manifest}")]
        );
        assert_eq!(
            unlocked(&[(t("{npm:manifest}"), "{}"), ("pnpm-workspace.yaml", "")]),
            [t("{npm:manifest}")]
        );
        assert!(unlocked(&[(t("{npm:manifest}"), "{}")]).is_empty());
        assert!(
            unlocked(&[(t("{npm:manifest}"), "{\"packageManager\": \"pnpm@9.1.0\"}")]).is_empty()
        );
        assert!(unlocked(&[(t("{npm:manifest}"), "{\"packageManager\": \"yarn@4\"}")]).is_empty());
        for lock in [t("{npm:lock}"), "package-lock.json", "yarn.lock"] {
            assert!(
                unlocked(&[(t("{npm:manifest}"), pnpm), (lock, "")]).is_empty(),
                "{lock}"
            );
        }
    }

    #[test]
    fn the_pnpm_tool_gets_an_extension_once() {
        let dir = scratch("pnpm_tool");
        write(dir.path(), "MODULE.bazel", MODULE);
        let plan = plan_pnpm_tool(dir.path()).expect("plan");
        let edit = plan.edit.expect("edit");
        assert!(
            !edit.new.contains("bazel_dep(name = \"aspect_rules_js\""),
            "{}",
            edit.new
        );
        assert!(
            edit.new.contains("pnpm = use_extension(\"@rules_dx//javascript:extensions.bzl\", \"pnpm\")\nuse_repo(pnpm, \"pnpm\")\n"),
            "{}",
            edit.new
        );
        write(dir.path(), "MODULE.bazel", &edit.new);
        assert_eq!(
            plan_pnpm_tool(dir.path()).expect("again"),
            LockHubs::default()
        );
    }

    #[test]
    fn uv_lock_gets_a_pypi_hub() {
        let hubs = plan(&[(t("{uv:lock}"), ""), (t("{uv:manifest}"), "")]);
        let text = new_text(&hubs);
        assert!(!text.contains("bazel_dep(name = \"rules_py\""), "{text}");
        assert!(
            text.contains("uv.declare_hub(\n    hub_name = \"pypi\",\n)"),
            "{text}"
        );
        assert!(text.contains(t("lock = \"//:{uv:lock}\"")), "{text}");
        assert!(
            text.contains(t("pyproject = \"//:{uv:manifest}\"")),
            "{text}"
        );
        assert!(text.ends_with("use_repo(uv, \"pypi\")\n"), "{text}");
    }

    #[test]
    fn pnpm_lock_gets_an_npm_hub() {
        let hubs = plan(&[
            (t("{npm:lock}"), ""),
            (
                t("{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
        ]);
        let text = new_text(&hubs);
        assert!(
            !text.contains("bazel_dep(name = \"aspect_rules_js\""),
            "{text}"
        );
        assert!(
            text.contains(t("npm.npm_translate_lock(\n    name = \"npm\",\n    pnpm_lock = \"//:{npm:lock}\",\n)\nuse_repo(npm, \"npm\")\n")),
            "{text}"
        );
    }

    #[test]
    fn a_pnpm_lock_without_a_dependency_gets_no_hub() {
        let hubs = plan(&[(t("{npm:lock}"), ""), (t("{npm:manifest}"), "{}")]);
        assert_eq!(hubs, LockHubs::default());
    }

    #[test]
    fn a_lock_without_its_manifest_is_ignored() {
        assert_eq!(
            plan(&[(t("{cargo:lock}"), ""), (t("{uv:lock}"), "")]),
            LockHubs::default()
        );
    }

    #[test]
    fn a_hand_written_stanza_is_left_alone() {
        let module = format!(
            "{MODULE}\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"web\",\n    pnpm_lock = \"//:{}\",\n)\nuse_repo(npm, \"web\")\n",
            lock_of("npm")
        );
        let dir = scratch("lock_hubs_manual");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), t("{npm:lock}"), "");
        write(
            dir.path(),
            t("{npm:manifest}"),
            "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
        );
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_second_run_plans_nothing() {
        let dir = scratch("lock_hubs_twice");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), t("{cargo:lock}"), "");
        write(dir.path(), t("{cargo:manifest}"), "");
        write(dir.path(), t("{uv:lock}"), "");
        write(dir.path(), t("{uv:manifest}"), "");
        write(dir.path(), t("{npm:lock}"), "");
        write(
            dir.path(),
            t("{npm:manifest}"),
            "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
        );
        let first = plan_lock_hubs(dir.path()).expect("plan");
        write(dir.path(), MODULE_FILE, new_text(&first));
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_nested_lock_beside_an_aspect_npm_binding_gets_its_own_binding() {
        let module = format!(
            "{MODULE}\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n"
        );
        let dir = scratch("lock_hubs_aspect_npm");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), t("web/{npm:lock}"), "");
        write(
            dir.path(),
            t("web/{npm:manifest}"),
            "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
        );
        let first = plan_lock_hubs(dir.path()).expect("plan");
        let text = new_text(&first);
        assert!(text.contains("dx_npm = use_extension("), "{text}");
        ModuleFile::parse(text).expect("re-parses");
        write(dir.path(), MODULE_FILE, text);
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn an_excluded_folder_is_not_planned() {
        let hubs = plan(&[
            ("BUILD.bazel", "# gazelle:exclude web\n"),
            (t("web/{npm:lock}"), ""),
            (
                t("web/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
        ]);
        assert_eq!(hubs, LockHubs::default());
    }

    #[test]
    fn a_nested_exclude_resolves_from_its_package() {
        let hubs = plan(&[
            (
                "a/BUILD.bazel",
                "# gazelle:exclude web\n# gazelle:exclude *\n",
            ),
            (t("a/web/{npm:lock}"), ""),
            (
                t("a/web/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
        ]);
        assert_eq!(hubs, LockHubs::default());
    }

    #[test]
    fn a_stanza_for_a_gone_lock_is_reported_and_kept() {
        let module = format!(
            "{MODULE}\nuv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(\n    hub_name = \"pypi\",\n    lock = \"//svc:{}\",\n    pyproject = \"//svc:{}\",\n)\n",
            lock_of("uv"),
            manifest_of("uv")
        );
        let dir = scratch("lock_hubs_gone");
        write(dir.path(), MODULE_FILE, &module);
        let hubs = plan_lock_hubs(dir.path()).expect("plan");
        assert_eq!(hubs.edit, None);
        assert_eq!(hubs.notes.len(), 1, "{:?}", hubs.notes);
        assert!(
            hubs.notes[0].contains(t("svc/{uv:lock}")),
            "{:?}",
            hubs.notes
        );
    }

    #[test]
    fn nested_uv_and_pnpm_locks_get_their_own_projects() {
        let hubs = plan(&[
            (t("{uv:lock}"), ""),
            (t("{uv:manifest}"), ""),
            (t("svc/{uv:lock}"), ""),
            (t("svc/{uv:manifest}"), ""),
            (t("web/app/{npm:lock}"), ""),
            (
                t("web/app/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
            (t("{npm:lock}"), ""),
            (
                t("{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
            (t("node_modules/x/{npm:lock}"), ""),
            (
                t("node_modules/x/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
        ]);
        let text = new_text(&hubs);
        assert!(text.contains(t("lock = \"//svc:{uv:lock}\"")), "{text}");
        assert!(text.contains(t("lock = \"//:{uv:lock}\"")), "{text}");
        assert_eq!(text.matches("declare_hub").count(), 1, "{text}");
        assert!(text.contains("name = \"npm_web_app\""), "{text}");
        assert!(
            text.contains(t("pnpm_lock = \"//web/app:{npm:lock}\"")),
            "{text}"
        );
        assert!(
            text.contains("use_repo(npm, \"npm\", \"npm_web_app\")"),
            "{text}"
        );
        assert!(!text.contains("node_modules"), "{text}");
    }

    #[test]
    fn an_aliased_pypi_hub_is_left_alone() {
        let module = format!(
            "{MODULE}\nuv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuse_repo(uv, pypi = \"dx_pypi\")\n"
        );
        let dir = scratch("lock_hubs_alias");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), t("{uv:lock}"), "");
        write(dir.path(), t("{uv:manifest}"), "");
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn fixture_testdata_and_example_locks_are_ignored() {
        let hubs = plan(&[
            (t("a/testdata/{uv:lock}"), ""),
            (t("a/testdata/{uv:manifest}"), ""),
            (t("fixtures/x/{npm:lock}"), ""),
            (
                t("fixtures/x/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
            (t("examples/app/{npm:lock}"), ""),
            (
                t("examples/app/{npm:manifest}"),
                "{\"dependencies\": {\"left-pad\": \"1.3.0\"}}",
            ),
        ]);
        assert_eq!(hubs, LockHubs::default());
    }

    #[test]
    fn an_extra_cargo_lock_is_reported_not_wired() {
        let hubs = plan(&[
            (t("tools/{cargo:lock}"), ""),
            (t("tools/{cargo:manifest}"), ""),
        ]);
        assert_eq!(hubs.edit, None);
        assert_eq!(hubs.notes.len(), 1, "{:?}", hubs.notes);
        assert!(
            hubs.notes[0].contains(t("tools/{cargo:lock}")),
            "{:?}",
            hubs.notes
        );
    }

    fn maven_module() -> String {
        format!(
            "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(\n    artifacts = [\"com.google.guava:guava:33.4.0-jre\"],\n    lock_file = \"//third_party:{}\",\n)\nuse_repo(maven, \"maven\")\n",
            lock_of("maven")
        )
    }

    fn maven_directive() -> String {
        format!(
            "# gazelle:dx_external maven third_party/{} maven\n",
            lock_of("maven")
        )
    }

    fn maven_workspace(files: &[(&str, &str)]) -> dx_test_scratch::TempDir {
        let dir = scratch("maven_locks");
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        dir
    }

    #[test]
    fn maven_lock_gets_its_package_lock_and_directive() {
        let dir = maven_workspace(&[("MODULE.bazel", &maven_module())]);
        let plan = plan_maven_locks(dir.path()).expect("plan");
        assert_eq!(plan.notes, Vec::<String>::new());
        assert_eq!(
            plan.seeds,
            [(
                format!("third_party/{}", lock_of("maven")),
                "maven".to_owned()
            )]
        );
        assert_eq!(
            plan.edits,
            [
                ModuleEdit {
                    path: "third_party/BUILD.bazel".to_owned(),
                    old: String::new(),
                    new: format!("exports_files([\"{}\"])\n", lock_of("maven")),
                },
                ModuleEdit {
                    path: "BUILD.bazel".to_owned(),
                    old: String::new(),
                    new: maven_directive(),
                },
            ]
        );
    }

    #[test]
    fn maven_lock_plans_nothing_the_second_time() {
        let dir = maven_workspace(&[("MODULE.bazel", &maven_module())]);
        let plan = plan_maven_locks(dir.path()).expect("plan");
        for edit in &plan.edits {
            crate::write_edit_or_create(dir.path(), edit).expect("write");
        }
        write(
            dir.path(),
            &format!("third_party/{}", lock_of("maven")),
            "{\"artifacts\": {}}",
        );
        assert_eq!(
            plan_maven_locks(dir.path()).expect("plan"),
            MavenLocks::default()
        );
    }

    #[test]
    fn maven_lock_keeps_the_root_build_and_skips_a_root_lock() {
        let dir = maven_workspace(&[
            ("MODULE.bazel", &maven_module()),
            ("third_party/BUILD", ""),
            (&format!("third_party/{}", lock_of("maven")), "{}"),
            (
                "BUILD.bazel",
                "package(default_visibility = [\"//visibility:public\"])",
            ),
        ]);
        let plan = plan_maven_locks(dir.path()).expect("plan");
        assert_eq!(plan.edits.len(), 1);
        assert_eq!(
            plan.edits[0].new,
            format!(
                "package(default_visibility = [\"//visibility:public\"])\n{}",
                maven_directive()
            )
        );

        let module = maven_module().replace("//third_party:", "//:");
        let dir = maven_workspace(&[("MODULE.bazel", &module), (lock_of("maven"), "{}")]);
        assert_eq!(
            plan_maven_locks(dir.path()).expect("plan"),
            MavenLocks::default()
        );
    }

    #[test]
    fn maven_lock_behind_a_renamed_repo_gets_a_note() {
        let module = maven_module().replace(
            "use_repo(maven, \"maven\")",
            "use_repo(maven, deps = \"maven\")",
        );
        let dir = maven_workspace(&[("MODULE.bazel", &module)]);
        let plan = plan_maven_locks(dir.path()).expect("plan");
        assert!(plan.edits.is_empty() && plan.seeds.is_empty(), "{plan:?}");
        assert_eq!(plan.notes.len(), 1);
        assert!(plan.notes[0].contains("as deps"), "{:?}", plan.notes);
    }
}
