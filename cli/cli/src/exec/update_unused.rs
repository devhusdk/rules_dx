use super::common::*;
use dx_adopt::dependency_sets::{Ecosystem, ResolvedTarget, Selection};
use dx_output::{notice_event, NoticeEvent};
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
    pub file: String,
    pub line: usize,
}

fn depcheck_ecosystem(ecosystem: Ecosystem) -> Option<dx_depcheck::Ecosystem> {
    match ecosystem {
        Ecosystem::Cargo => Some(dx_depcheck::Ecosystem::Rust),
        Ecosystem::Npm => Some(dx_depcheck::Ecosystem::Js),
        Ecosystem::Uv => Some(dx_depcheck::Ecosystem::Python),
        Ecosystem::Go => Some(dx_depcheck::Ecosystem::Go),
        Ecosystem::Ruby => Some(dx_depcheck::Ecosystem::Ruby),
        _ => None,
    }
}

/// Declared dependencies of the resolved sets that no source under the manifest's directory uses.
pub(super) fn find(workspace: &Path, resolved: &[ResolvedTarget]) -> Vec<Unused> {
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
            let Ok(found) = dx_depcheck::find_unused(eco, &path, root, &excepted) else {
                continue;
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

/// Imports under the manifest's directory that no declared dependency covers; Go sets only.
pub(super) fn find_missing(workspace: &Path, resolved: &[ResolvedTarget]) -> Vec<Missing> {
    let exceptions =
        dx_depcheck::parse_set_exceptions(&workspace.join(EXCEPTIONS_FILE)).unwrap_or_default();
    let mut out = Vec::new();
    for target in resolved {
        if target.set.ecosystem != Ecosystem::Go || target.request != Selection::Full {
            continue;
        }
        let set = target.set.name.as_str();
        let excepted: Vec<String> = exceptions
            .iter()
            .filter(|exception| exception.set == set)
            .map(|exception| exception.name.clone())
            .collect();
        for manifest in &target.set.manifests {
            let path = workspace.join(manifest);
            let root = path.parent().unwrap_or(workspace);
            let Ok(found) = dx_depcheck::find_missing_go(&path, root, &excepted) else {
                continue;
            };
            let dir = Path::new(manifest).parent().unwrap_or(Path::new(""));
            for dep in found {
                out.push(Missing {
                    set: set.to_owned(),
                    name: dep.name,
                    file: dir.join(&dep.file).to_string_lossy().replace('\\', "/"),
                    line: dep.line,
                });
            }
        }
    }
    out.sort_by(|a, b| (&a.set, &a.name).cmp(&(&b.set, &b.name)));
    out.dedup();
    out
}

pub(super) fn emit_missing_text(out: &mut dyn Write, missing: &[Missing]) -> Result<(), i32> {
    for item in missing {
        check_stdout_write(writeln!(
            out,
            "missing: {}: {} (imported in {}:{})",
            item.set, item.name, item.file, item.line
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
                        "{} is imported but not declared (imported in {}:{})",
                        item.name, item.file, item.line
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
        let found = find(root, &resolved);
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
        assert_eq!(find(root, &resolved).len(), 1);
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
        let found = find_missing(root, &resolved);
        let mut out = Vec::new();
        emit_missing_text(&mut out, &found).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out),
            "missing: go: github.com/c/d (imported in svc/main.go:5)\n"
        );
    }
}
