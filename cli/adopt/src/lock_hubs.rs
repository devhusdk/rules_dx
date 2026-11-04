//! Plans the `MODULE.bazel` stanzas for the root `Cargo.lock`, `uv.lock` and `pnpm-lock.yaml`.
//!
//! A lock file counts when its manifest sits next to it. A lock that an existing stanza
//! already names is left alone, so a second run plans nothing.

use std::path::Path;

use dx_module_bazel::{Arg, ModuleError, ModuleFile, NewTag};

use crate::module_pin::ModuleEdit;
use crate::AdoptError;

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
    dep: "aspect_rules_py",
    version: dx_versions::ASPECT_RULES_PY_VERSION,
    bzl: "@aspect_rules_py//uv:extensions.bzl",
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

fn refuse(detail: impl std::fmt::Display) -> AdoptError {
    AdoptError::ModulePin {
        path: MODULE_FILE.to_owned(),
        detail: detail.to_string(),
    }
}

fn root_label(file: &str) -> String {
    format!("//:{file}")
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

/// Plans the stanzas for the lock files at the root of `root`. Writes nothing.
pub fn plan_lock_hubs(root: &Path) -> Result<LockHubs, AdoptError> {
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
    plan.report_missing_locks();
    plan.cargo()?;
    plan.uv()?;
    plan.npm()?;
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
            if let Ok(path) = dx_module_bazel::label_path(&label) {
                if !self.exists(&path) {
                    self.notes
                        .push(format!("{MODULE_FILE} names {label}, but {path} is gone"));
                }
            }
        }
    }

    fn cargo(&mut self) -> Result<(), AdoptError> {
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
        self.add(&CARGO, &tags)
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

    fn uv(&mut self) -> Result<(), AdoptError> {
        if !self.exists(UV_LOCK) || !self.exists("pyproject.toml") {
            return Ok(());
        }
        let lock = root_label(UV_LOCK);
        let mut declared = false;
        for file in self.files {
            let Ok(projects) = file.uv_projects() else {
                self.notes
                    .push("a uv.project has computed arguments; uv.lock not wired".into());
                return Ok(());
            };
            if projects.iter().any(|project| project.lock == lock) {
                return Ok(());
            }
            declared |= file
                .calls_named("uv.declare_hub")
                .any(|call| call.str("hub_name") == Some(UV.hub));
        }
        let mut tags = Vec::new();
        if !declared {
            tags.push(tag("declare_hub", vec![("hub_name", string(UV.hub))]));
        }
        tags.push(tag(
            "project",
            vec![
                ("hub_name", string(UV.hub)),
                ("pyproject", string(&root_label("pyproject.toml"))),
                ("lock", string(&lock)),
            ],
        ));
        self.add(&UV, &tags)
    }

    fn npm(&mut self) -> Result<(), AdoptError> {
        if !self.exists(PNPM_LOCK) || !self.exists("package.json") {
            return Ok(());
        }
        let lock = root_label(PNPM_LOCK);
        for file in self.files {
            let Ok(locks) = file.npm_translate_locks() else {
                self.notes.push(
                    "an npm.npm_translate_lock has computed arguments; pnpm-lock.yaml not wired"
                        .into(),
                );
                return Ok(());
            };
            for existing in locks {
                if existing.pnpm_lock == Some(lock.as_str()) {
                    return Ok(());
                }
                if existing.name == NPM.hub {
                    self.notes.push(format!(
                        "npm.npm_translate_lock \"{}\" reads another lock; pnpm-lock.yaml not wired",
                        existing.name
                    ));
                    return Ok(());
                }
            }
        }
        let tags = vec![tag(
            "npm_translate_lock",
            vec![("name", string(NPM.hub)), ("pnpm_lock", string(&lock))],
        )];
        self.add(&NPM, &tags)
    }

    fn add(&mut self, eco: &Ecosystem, tags: &[NewTag]) -> Result<(), AdoptError> {
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
            .insert_extension_block(&binding, eco.bzl, eco.ext, tags, &[eco.hub])
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

    #[test]
    fn uv_lock_gets_a_pypi_hub() {
        let hubs = plan(&[("uv.lock", ""), ("pyproject.toml", "")]);
        let text = new_text(&hubs);
        assert!(
            text.contains("bazel_dep(name = \"aspect_rules_py\""),
            "{text}"
        );
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
            "{MODULE}\nuv = use_extension(\"@aspect_rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(\n    hub_name = \"pypi\",\n    lock = \"//svc:uv.lock\",\n    pyproject = \"//svc:pyproject.toml\",\n)\n"
        );
        let dir = scratch("lock_hubs_gone");
        write(dir.path(), MODULE_FILE, &module);
        let hubs = plan_lock_hubs(dir.path()).expect("plan");
        assert_eq!(hubs.edit, None);
        assert_eq!(hubs.notes.len(), 1, "{:?}", hubs.notes);
        assert!(hubs.notes[0].contains("svc/uv.lock"), "{:?}", hubs.notes);
    }
}
