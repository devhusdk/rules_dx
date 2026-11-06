//! Plans the `MODULE.bazel` stanzas for `uv.lock` and `pnpm-lock.yaml` files anywhere in the
//! repo and the root `Cargo.lock`. Other `Cargo.lock` files are reported.
//!
//! A lock file counts when its manifest sits next to it. A lock that an existing stanza
//! already names is left alone, so a second run plans nothing.

use std::path::Path;

use dx_module_bazel::{Arg, ModuleError, ModuleFile, NewTag, Value};

use crate::AdoptError;
use crate::module_pin::ModuleEdit;

const MODULE_FILE: &str = "MODULE.bazel";
const CARGO_LOCK: &str = "Cargo.lock";
const UV_LOCK: &str = "uv.lock";
const PNPM_LOCK: &str = "pnpm-lock.yaml";

/// The planned `MODULE.bazel` edit, if any, and what the planner could not do.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct LockHubs {
    pub edit: Option<ModuleEdit>,
    pub notes: Vec<String>,
}

struct Ecosystem {
    dep: &'static str,
    version: &'static str,
    bzl: &'static str,
    ext: &'static str,
    binding: &'static str,
    hub: &'static str,
}

const CARGO: Ecosystem = Ecosystem {
    dep: "rules_rust",
    version: dx_versions::RULES_RUST_VERSION,
    bzl: "@rules_rust//crate_universe:extensions.bzl",
    ext: "crate",
    binding: "crate",
    hub: "crates",
};
const UV: Ecosystem = Ecosystem {
    dep: "rules_py",
    version: dx_versions::RULES_PY_VERSION,
    bzl: "@rules_py//uv:extensions.bzl",
    ext: "uv",
    binding: "uv",
    hub: "pypi",
};
const NPM: Ecosystem = Ecosystem {
    dep: "aspect_rules_js",
    version: dx_versions::ASPECT_RULES_JS_VERSION,
    bzl: "@aspect_rules_js//npm:extensions.bzl",
    ext: "npm",
    binding: "npm",
    hub: "npm",
};
const TYPESCRIPT: Ecosystem = Ecosystem {
    dep: "aspect_rules_ts",
    version: dx_versions::ASPECT_RULES_TS_VERSION,
    bzl: "@aspect_rules_ts//ts:extensions.bzl",
    ext: "typescript",
    binding: "typescript",
    hub: "npm_typescript",
};
const TSCONFIG: &str = "tsconfig.json";

const PNPM: Ecosystem = Ecosystem {
    dep: "aspect_rules_js",
    version: dx_versions::ASPECT_RULES_JS_VERSION,
    bzl: "@aspect_rules_js//npm:extensions.bzl",
    ext: "pnpm",
    binding: "pnpm",
    hub: "pnpm",
};

fn binds_name(file: &ModuleFile, eco: &Ecosystem) -> bool {
    let bindings: Vec<&str> = file
        .extension_bindings(eco.bzl, eco.ext)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    file.calls_named("use_repo").any(|call| {
        matches!(call.positional.first(), Some(Value::Ident(v)) if bindings.contains(&v.as_str()))
            && (call.named.iter().any(|(alias, _)| alias == eco.hub)
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

/// The root manifests that `dx generate --apply` can lock: `Cargo.toml` without a `Cargo.lock`
/// (an empty one counts as missing), a uv `pyproject.toml` without a `uv.lock`, and a pnpm
/// `package.json` without a `pnpm-lock.yaml`.
pub fn unlocked_manifests(root: &Path) -> Vec<&'static str> {
    let mut out = Vec::new();
    let cargo_lock = std::fs::metadata(root.join(CARGO_LOCK));
    if root.join("Cargo.toml").is_file() && cargo_lock.map_or(true, |meta| meta.len() == 0) {
        out.push("Cargo.toml");
    }
    let pyproject = std::fs::read_to_string(root.join("pyproject.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok());
    if let Some(table) = pyproject {
        let tool = table.get("tool").and_then(|tool| tool.as_table());
        let other =
            tool.is_some_and(|tool| tool.contains_key("poetry") || tool.contains_key("pdm"));
        if table.contains_key("project") && !other && !root.join(UV_LOCK).exists() {
            out.push("pyproject.toml");
        }
    }
    if wants_pnpm_lock(root) {
        out.push("package.json");
    }
    out
}

fn wants_pnpm_lock(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join("package.json")) else {
        return false;
    };
    let declared = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|json| json.get("packageManager")?.as_str().map(str::to_owned))
        .is_some_and(|manager| manager.starts_with("pnpm@"));
    let other_lock = ["package-lock.json", "yarn.lock", PNPM_LOCK]
        .iter()
        .any(|lock| root.join(lock).exists());
    (declared || root.join("pnpm-workspace.yaml").is_file()) && !other_lock
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
        plan.typescript()
    })
}

fn plan_with(
    root: &Path,
    run: impl FnOnce(&mut Planner) -> Result<(), AdoptError>,
) -> Result<LockHubs, AdoptError> {
    let Ok(text) = std::fs::read_to_string(root.join(MODULE_FILE)) else {
        return Ok(LockHubs::default());
    };
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(&rel)).map_err(|e| format!("cannot read {rel}: {e}"))
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
        for dir in self.lock_dirs(CARGO_LOCK, "Cargo.toml") {
            let label = dir_label(&dir, CARGO_LOCK);
            let named = self.files.iter().any(|file| {
                file.cargo_hubs().is_ok_and(|hubs| {
                    hubs.iter()
                        .any(|hub| hub.cargo_lockfile == Some(label.as_str()))
                })
            });
            if !dir.is_empty() && !named {
                self.notes.push(format!(
                    "{dir}/{CARGO_LOCK} is not wired; add a crate.from_cargo for it"
                ));
            }
        }
    }

    fn cargo(&mut self) -> Result<(), AdoptError> {
        self.report_extra_cargo_locks();
        if !self.exists(CARGO_LOCK) || !self.exists("Cargo.toml") {
            return Ok(());
        }
        let lock = root_label(CARGO_LOCK);
        let mut covered = false;
        for file in self.files {
            let Ok(hubs) = file.cargo_hubs() else {
                self.notes
                    .push("a crate.from_cargo has computed arguments; Cargo.lock not wired".into());
                return Ok(());
            };
            for hub in hubs {
                covered |= hub.cargo_lockfile == Some(lock.as_str());
                if hub.name == CARGO.hub && hub.cargo_lockfile != Some(lock.as_str()) {
                    self.notes.push(format!(
                        "crate.from_cargo \"{}\" reads another lock; Cargo.lock not wired",
                        hub.name
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
                ("name", string(CARGO.hub)),
                ("cargo_lockfile", string(&lock)),
                ("manifests", Arg::List(manifests)),
            ],
        )];
        self.add(&CARGO, &tags, &[CARGO.hub])
    }

    fn cargo_manifests(&mut self) -> Vec<String> {
        let mut manifests = vec![root_label("Cargo.toml")];
        let parsed = std::fs::read_to_string(self.root.join("Cargo.toml"))
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
                    "Cargo.toml member \"{member}\" is a glob; add its Cargo.toml to crate.from_cargo manifests"
                ));
            } else if member != "." && self.exists(&format!("{member}/Cargo.toml")) {
                manifests.push(format!("//{member}:Cargo.toml"));
            } else if member != "." {
                self.notes
                    .push(format!("Cargo.toml member \"{member}\" has no Cargo.toml"));
            }
        }
        manifests
    }

    /// Directories under the root (as `""` or `a/b`) holding `lock` next to `manifest`.
    fn lock_dirs(&self, lock: &str, manifest: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut pending = vec![String::new()];
        while let Some(dir) = pending.pop() {
            let path = self.root.join(&dir);
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
        let dirs = self.lock_dirs(UV_LOCK, "pyproject.toml");
        if dirs.is_empty() {
            return Ok(());
        }
        let mut named: Vec<String> = Vec::new();
        let mut declared = false;
        for file in self.files {
            if binds_name(file, &UV) {
                return Ok(());
            }
            let Ok(projects) = file.uv_projects() else {
                self.notes
                    .push("a uv.project has computed arguments; uv.lock not wired".into());
                return Ok(());
            };
            named.extend(projects.iter().map(|project| project.lock.to_owned()));
            declared |= file
                .calls_named("uv.declare_hub")
                .any(|call| call.str("hub_name") == Some(UV.hub));
        }
        let mut tags = Vec::new();
        for dir in dirs {
            let lock = dir_label(&dir, UV_LOCK);
            if named.contains(&lock) {
                continue;
            }
            tags.push(tag(
                "project",
                vec![
                    ("hub_name", string(UV.hub)),
                    ("pyproject", string(&dir_label(&dir, "pyproject.toml"))),
                    ("lock", string(&lock)),
                ],
            ));
        }
        if tags.is_empty() {
            return Ok(());
        }
        if !declared {
            tags.insert(0, tag("declare_hub", vec![("hub_name", string(UV.hub))]));
        }
        self.add(&UV, &tags, &[UV.hub])
    }

    fn npm(&mut self) -> Result<(), AdoptError> {
        let dirs = self.lock_dirs(PNPM_LOCK, "package.json");
        if dirs.is_empty() {
            return Ok(());
        }
        let mut existing: Vec<(String, Option<String>)> = Vec::new();
        for file in self.files {
            let Ok(locks) = file.npm_translate_locks() else {
                self.notes.push(
                    "an npm.npm_translate_lock has computed arguments; pnpm-lock.yaml not wired"
                        .into(),
                );
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
            let lock = dir_label(&dir, PNPM_LOCK);
            let hub = if dir.is_empty() {
                NPM.hub.to_owned()
            } else {
                format!("{}_{}", NPM.hub, dir.replace('/', "_"))
            };
            if existing.iter().any(|(_, l)| l.as_deref() == Some(&lock)) {
                continue;
            }
            if existing.iter().any(|(name, _)| *name == hub) {
                self.notes.push(format!(
                    "npm.npm_translate_lock \"{hub}\" reads another lock; {PNPM_LOCK} in {} not wired",
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
        self.add(&NPM, &tags, &hubs)
    }

    fn pnpm(&mut self) -> Result<(), AdoptError> {
        if self.files.iter().any(|file| binds_name(file, &PNPM)) {
            return Ok(());
        }
        self.add(&PNPM, &[], &[PNPM.hub])
    }

    fn typescript(&mut self) -> Result<(), AdoptError> {
        let wanted = self.exists(TSCONFIG)
            || self
                .lock_dirs(PNPM_LOCK, "package.json")
                .iter()
                .any(|dir| self.exists(&format!("{dir}/{TSCONFIG}")));
        if !wanted || self.files.iter().any(|file| binds_name(file, &TYPESCRIPT)) {
            return Ok(());
        }
        let tags = vec![tag(
            "deps",
            vec![
                ("version", string(dx_versions::TYPESCRIPT_VERSION)),
                ("integrity", string(dx_versions::TYPESCRIPT_INTEGRITY)),
            ],
        )];
        self.add(&TYPESCRIPT, &tags, &[TYPESCRIPT.hub])
    }

    fn add(&mut self, eco: &Ecosystem, tags: &[NewTag], hubs: &[&str]) -> Result<(), AdoptError> {
        let declared = self
            .files
            .iter()
            .any(|file| file.bazel_dep(eco.dep).is_ok());
        if !declared {
            let file = ModuleFile::parse(&self.text).map_err(refuse)?;
            match file.insert_bazel_dep(eco.dep, eco.version) {
                Ok(text) => self.text = text,
                Err(ModuleError::AlreadyDeclared { .. }) => {}
                Err(error) => return Err(refuse(error)),
            }
        }
        let file = ModuleFile::parse(&self.text).map_err(refuse)?;
        let binding = file
            .extension_bindings(eco.bzl, eco.ext)
            .first()
            .map_or(eco.binding, |(name, _)| *name)
            .to_owned();
        self.text = file
            .insert_extension_block(&binding, eco.bzl, eco.ext, tags, hubs)
            .map_err(refuse)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_test_scratch::{scratch, write};

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
            ("Cargo.lock", ""),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"app\", \"libs/*\", \"gone\"]\n",
            ),
            ("app/Cargo.toml", ""),
        ]);
        let text = new_text(&hubs);
        assert!(text.starts_with(MODULE), "{text}");
        assert!(text.contains("bazel_dep(name = \"rules_rust\""), "{text}");
        assert!(
            text.contains(
                "crate.from_cargo(\n    name = \"crates\",\n    cargo_lockfile = \"//:Cargo.lock\",\n    manifests = [\n        \"//:Cargo.toml\",\n        \"//app:Cargo.toml\",\n    ],\n)\nuse_repo(crate, \"crates\")\n"
            ),
            "{text}"
        );
        assert_eq!(hubs.notes.len(), 2, "{:?}", hubs.notes);
        assert!(hubs.notes[0].contains("libs/*"), "{:?}", hubs.notes);
        assert!(hubs.notes[1].contains("gone"), "{:?}", hubs.notes);
    }

    fn unlocked(files: &[(&str, &str)]) -> Vec<&'static str> {
        let dir = scratch("unlocked");
        for (rel, text) in files {
            write(dir.path(), rel, text);
        }
        unlocked_manifests(dir.path())
    }

    #[test]
    fn a_missing_or_empty_cargo_lock_is_unlocked() {
        assert_eq!(unlocked(&[("Cargo.toml", "")]), ["Cargo.toml"]);
        assert_eq!(
            unlocked(&[("Cargo.toml", ""), ("Cargo.lock", "")]),
            ["Cargo.toml"]
        );
        assert!(unlocked(&[("Cargo.toml", ""), ("Cargo.lock", "version = 4\n")]).is_empty());
        assert!(unlocked(&[("Cargo.lock", "")]).is_empty());
    }

    #[test]
    fn only_uv_pyprojects_without_a_lock_are_unlocked() {
        let project = "[project]\nname = \"app\"\n";
        assert_eq!(unlocked(&[("pyproject.toml", project)]), ["pyproject.toml"]);
        assert!(unlocked(&[("pyproject.toml", project), ("uv.lock", "")]).is_empty());
        assert!(unlocked(&[("pyproject.toml", "[tool.ruff]\n")]).is_empty());
        for tool in ["[tool.poetry]\n", "[tool.pdm]\n"] {
            let text = format!("{project}{tool}");
            assert!(unlocked(&[("pyproject.toml", &text)]).is_empty(), "{tool}");
        }
    }

    #[test]
    fn only_pnpm_packages_without_a_lock_are_unlocked() {
        let pnpm = "{\"packageManager\": \"pnpm@9.1.0\"}";
        assert_eq!(unlocked(&[("package.json", pnpm)]), ["package.json"]);
        assert_eq!(
            unlocked(&[("package.json", "{}"), ("pnpm-workspace.yaml", "")]),
            ["package.json"]
        );
        assert!(unlocked(&[("package.json", "{}")]).is_empty());
        assert!(unlocked(&[("package.json", "{\"packageManager\": \"yarn@4\"}")]).is_empty());
        for lock in ["pnpm-lock.yaml", "package-lock.json", "yarn.lock"] {
            assert!(
                unlocked(&[("package.json", pnpm), (lock, "")]).is_empty(),
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
            edit.new.contains("bazel_dep(name = \"aspect_rules_js\""),
            "{}",
            edit.new
        );
        assert!(
            edit.new.contains("pnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"pnpm\")\nuse_repo(pnpm, \"pnpm\")\n"),
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
        let hubs = plan(&[("uv.lock", ""), ("pyproject.toml", "")]);
        let text = new_text(&hubs);
        assert!(text.contains("bazel_dep(name = \"rules_py\""), "{text}");
        assert!(
            text.contains("uv.declare_hub(\n    hub_name = \"pypi\",\n)"),
            "{text}"
        );
        assert!(text.contains("lock = \"//:uv.lock\""), "{text}");
        assert!(text.contains("pyproject = \"//:pyproject.toml\""), "{text}");
        assert!(text.ends_with("use_repo(uv, \"pypi\")\n"), "{text}");
    }

    #[test]
    fn pnpm_lock_gets_an_npm_hub() {
        let hubs = plan(&[("pnpm-lock.yaml", ""), ("package.json", "{}")]);
        let text = new_text(&hubs);
        assert!(
            text.contains("bazel_dep(name = \"aspect_rules_js\""),
            "{text}"
        );
        assert!(
            text.contains("npm.npm_translate_lock(\n    name = \"npm\",\n    pnpm_lock = \"//:pnpm-lock.yaml\",\n)\nuse_repo(npm, \"npm\")\n"),
            "{text}"
        );
    }

    #[test]
    fn tsconfig_gets_the_typescript_hub() {
        let hubs = plan(&[
            ("pnpm-lock.yaml", ""),
            ("package.json", "{}"),
            ("tsconfig.json", "{}"),
        ]);
        let text = new_text(&hubs);
        assert!(
            text.contains("bazel_dep(name = \"aspect_rules_ts\""),
            "{text}"
        );
        assert!(
            text.contains("typescript = use_extension(\"@aspect_rules_ts//ts:extensions.bzl\", \"typescript\")\ntypescript.deps(\n"),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "version = \"{}\"",
                dx_versions::TYPESCRIPT_VERSION
            )),
            "{text}"
        );
        assert!(
            text.ends_with("use_repo(typescript, \"npm_typescript\")\n"),
            "{text}"
        );
    }

    #[test]
    fn no_tsconfig_means_no_typescript_hub() {
        let hubs = plan(&[("pnpm-lock.yaml", ""), ("package.json", "{}")]);
        assert!(!new_text(&hubs).contains("npm_typescript"));
    }

    #[test]
    fn a_second_typescript_run_plans_nothing() {
        let dir = scratch("lock_hubs_ts_twice");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "tsconfig.json", "{}");
        write(dir.path(), "pnpm-lock.yaml", "");
        write(dir.path(), "package.json", "{}");
        let first = plan_lock_hubs(dir.path()).expect("plan");
        assert!(new_text(&first).contains("npm_typescript"));
        write(dir.path(), MODULE_FILE, new_text(&first));
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_hand_written_typescript_stanza_is_left_alone() {
        let module = format!(
            "{MODULE}\nts = use_extension(\"@aspect_rules_ts//ts:extensions.bzl\", \"typescript\")\nts.deps(version = \"5.0.0\")\nuse_repo(ts, \"npm_typescript\")\n"
        );
        let dir = scratch("lock_hubs_ts_manual");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "tsconfig.json", "{}");
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_lock_without_its_manifest_is_ignored() {
        assert_eq!(
            plan(&[("Cargo.lock", ""), ("uv.lock", "")]),
            LockHubs::default()
        );
    }

    #[test]
    fn a_hand_written_stanza_is_left_alone() {
        let module = format!(
            "{MODULE}\nnpm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(\n    name = \"web\",\n    pnpm_lock = \"//:pnpm-lock.yaml\",\n)\nuse_repo(npm, \"web\")\n"
        );
        let dir = scratch("lock_hubs_manual");
        write(dir.path(), MODULE_FILE, &module);
        write(dir.path(), "pnpm-lock.yaml", "");
        write(dir.path(), "package.json", "{}");
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_second_run_plans_nothing() {
        let dir = scratch("lock_hubs_twice");
        write(dir.path(), MODULE_FILE, MODULE);
        write(dir.path(), "Cargo.lock", "");
        write(dir.path(), "Cargo.toml", "");
        write(dir.path(), "uv.lock", "");
        write(dir.path(), "pyproject.toml", "");
        write(dir.path(), "pnpm-lock.yaml", "");
        write(dir.path(), "package.json", "{}");
        let first = plan_lock_hubs(dir.path()).expect("plan");
        write(dir.path(), MODULE_FILE, new_text(&first));
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn a_stanza_for_a_gone_lock_is_reported_and_kept() {
        let module = format!(
            "{MODULE}\nuv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(\n    hub_name = \"pypi\",\n    lock = \"//svc:uv.lock\",\n    pyproject = \"//svc:pyproject.toml\",\n)\n"
        );
        let dir = scratch("lock_hubs_gone");
        write(dir.path(), MODULE_FILE, &module);
        let hubs = plan_lock_hubs(dir.path()).expect("plan");
        assert_eq!(hubs.edit, None);
        assert_eq!(hubs.notes.len(), 1, "{:?}", hubs.notes);
        assert!(hubs.notes[0].contains("svc/uv.lock"), "{:?}", hubs.notes);
    }

    #[test]
    fn nested_uv_and_pnpm_locks_get_their_own_projects() {
        let hubs = plan(&[
            ("uv.lock", ""),
            ("pyproject.toml", ""),
            ("svc/uv.lock", ""),
            ("svc/pyproject.toml", ""),
            ("web/app/pnpm-lock.yaml", ""),
            ("web/app/package.json", "{}"),
            ("pnpm-lock.yaml", ""),
            ("package.json", "{}"),
            ("node_modules/x/pnpm-lock.yaml", ""),
            ("node_modules/x/package.json", "{}"),
        ]);
        let text = new_text(&hubs);
        assert!(text.contains("lock = \"//svc:uv.lock\""), "{text}");
        assert!(text.contains("lock = \"//:uv.lock\""), "{text}");
        assert_eq!(text.matches("declare_hub").count(), 1, "{text}");
        assert!(text.contains("name = \"npm_web_app\""), "{text}");
        assert!(
            text.contains("pnpm_lock = \"//web/app:pnpm-lock.yaml\""),
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
        write(dir.path(), "uv.lock", "");
        write(dir.path(), "pyproject.toml", "");
        assert_eq!(
            plan_lock_hubs(dir.path()).expect("plan"),
            LockHubs::default()
        );
    }

    #[test]
    fn fixture_testdata_and_example_locks_are_ignored() {
        let hubs = plan(&[
            ("a/testdata/uv.lock", ""),
            ("a/testdata/pyproject.toml", ""),
            ("fixtures/x/pnpm-lock.yaml", ""),
            ("fixtures/x/package.json", "{}"),
            ("examples/app/pnpm-lock.yaml", ""),
            ("examples/app/package.json", "{}"),
        ]);
        assert_eq!(hubs, LockHubs::default());
    }

    #[test]
    fn an_extra_cargo_lock_is_reported_not_wired() {
        let hubs = plan(&[("tools/Cargo.lock", ""), ("tools/Cargo.toml", "")]);
        assert_eq!(hubs.edit, None);
        assert_eq!(hubs.notes.len(), 1, "{:?}", hubs.notes);
        assert!(
            hubs.notes[0].contains("tools/Cargo.lock"),
            "{:?}",
            hubs.notes
        );
    }
}
