use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quality_adapter::exec::{MirrorContents, MirrorFile};

use super::{
    BICEP_CONFIG_REL, BICEP_HOME_REL, RealBackend, RealTool, RunnerError, SELENE_STD_BYTES,
    SELENE_STD_FILE, Scratch, StagedPair, StagedScratch, execution, fresh_scratch,
    missing_tier_base, parent_rel, write_all,
};

pub(super) const TIER_BASE_TOOLS: &[&str] = &[
    "bicep",
    "biome",
    "checkmake",
    "credo",
    "detekt",
    "docstrfmt",
    "elvis",
    "ffmt",
    "gersemi",
    "gplint",
    "hadolint",
    "hlint",
    "luals",
    "mbake",
    "pmd",
    "redocly",
    "regal",
    "rustfmt",
    "scalafix",
    "scalafmt",
    "selene",
    "stylelint",
    "tflint",
];

const SHFMT_CONFIG_REL: &str = ".editorconfig";

fn shfmt_editorconfig(bound: Option<&[u8]>) -> Vec<u8> {
    let mut out = b"root = true\n".to_vec();
    out.extend_from_slice(bound.unwrap_or_default());
    out
}

const FANTOMAS_CONFIG_REL: &str = ".editorconfig";
const FANTOMAS_EDITORCONFIG: &[u8] = b"root = true\n\n[*.{fs,fsi,fsx}]\nend_of_line = lf\n";

const HELMFMT_CONFIG_REL: &str = ".helmfmt";

const FSHARPLINT_CONTEXT_REL: &str = "dx-fsharplint/context.json";

pub(super) struct FsharplintInputs {
    pub compile: Vec<PathBuf>,
    pub refs: Vec<PathBuf>,
    pub defines: Vec<String>,
    pub langversion: String,
}

fn strip_json_comments(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        let Some(&b) = bytes.get(i) else {
            break;
        };
        if in_string {
            out.push(b);
            if let (b'\\', Some(&next)) = (b, bytes.get(i + 1)) {
                out.push(next);
                i += 1;
            } else if b == b'"' {
                in_string = false;
            }
        } else if b == b'"' {
            in_string = true;
            out.push(b);
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while bytes.get(i).is_some_and(|c| *c != b'\n') {
                i += 1;
            }
            continue;
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && bytes.get(i..i + 2) != Some(b"*/".as_slice()) {
                i += 1;
            }
            i += 2;
            out.push(b' ');
            continue;
        } else {
            out.push(b);
        }
        i += 1;
    }
    out
}

fn pin_bicep_cache(bound: Option<Vec<u8>>, root: &Path) -> Vec<u8> {
    let mut config = match bound {
        None => serde_json::Value::Object(serde_json::Map::new()),
        Some(bytes) => {
            match serde_json::from_slice::<serde_json::Value>(&strip_json_comments(&bytes)) {
                Ok(value) if value.is_object() => value,
                _ => return bytes,
            }
        }
    };
    if let Some(object) = config.as_object_mut() {
        object.insert(
            "cacheRootDirectory".to_owned(),
            serde_json::Value::String(
                root.join(BICEP_HOME_REL)
                    .join(".bicep")
                    .to_string_lossy()
                    .into_owned(),
            ),
        );
    }
    let mut out = serde_json::to_vec_pretty(&config).unwrap_or_else(|_| b"{}\n".to_vec());
    out.push(b'\n');
    out
}

fn user_ships_selene_std<'a>(
    tool: &RealTool,
    staged: impl Iterator<Item = &'a MirrorFile>,
) -> bool {
    let config_dir = tool
        .config_rel
        .as_deref()
        .and_then(|rel| Path::new(rel).parent())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let rels: Vec<&Path> = staged.map(|mirror| mirror.mirror_rel.as_path()).collect();
    ["lua54.yml", "lua54.yaml", "lua54.toml"]
        .iter()
        .any(|name| {
            rels.contains(&Path::new(name)) || rels.contains(&config_dir.join(name).as_path())
        })
}

impl RealBackend {
    pub(super) fn mirror_tool_files(
        tool_id: &str,
        tool: &RealTool,
        staged: &[MirrorFile],
        root: &Path,
    ) -> Vec<MirrorFile> {
        let mut mirrors = Vec::with_capacity(tool.tool_files.len() + 1);
        for (rel, bytes) in &tool.tool_files {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(rel),
                contents: MirrorContents::Bytes(bytes.clone()),
            });
        }
        if tool_id == "bicep" {
            let bound = tool.config_rel.as_deref().and_then(|rel| {
                tool.tool_files
                    .iter()
                    .find(|(file, _)| file == rel)
                    .map(|(_, bytes)| bytes.clone())
            });
            mirrors.retain(|mirror| mirror.mirror_rel != Path::new(BICEP_CONFIG_REL));
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(BICEP_CONFIG_REL),
                contents: MirrorContents::Bytes(pin_bicep_cache(bound, root)),
            });
        }
        if tool_id == "shfmt" {
            let bound = tool.config_rel.as_deref().and_then(|rel| {
                tool.tool_files
                    .iter()
                    .find(|(file, _)| file == rel)
                    .map(|(_, bytes)| bytes.as_slice())
            });
            mirrors.retain(|mirror| mirror.mirror_rel != Path::new(SHFMT_CONFIG_REL));
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(SHFMT_CONFIG_REL),
                contents: MirrorContents::Bytes(shfmt_editorconfig(bound)),
            });
        }
        if tool_id == "fantomas" {
            mirrors.retain(|mirror| mirror.mirror_rel != Path::new(FANTOMAS_CONFIG_REL));
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(FANTOMAS_CONFIG_REL),
                contents: MirrorContents::Bytes(FANTOMAS_EDITORCONFIG.to_vec()),
            });
        }
        if tool_id == "helmfmt" {
            let bound = tool.config_rel.as_deref().and_then(|rel| {
                tool.tool_files
                    .iter()
                    .find(|(file, _)| file == rel)
                    .map(|(_, bytes)| bytes.clone())
            });
            mirrors.retain(|mirror| mirror.mirror_rel != Path::new(HELMFMT_CONFIG_REL));
            if let Some(bytes) = bound {
                mirrors.push(MirrorFile {
                    mirror_rel: PathBuf::from(HELMFMT_CONFIG_REL),
                    contents: MirrorContents::Bytes(bytes),
                });
            }
        }
        if tool_id == "selene" && !user_ships_selene_std(tool, staged.iter().chain(mirrors.iter()))
        {
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(SELENE_STD_FILE),
                contents: MirrorContents::Bytes(SELENE_STD_BYTES.to_vec()),
            });
        }
        mirrors.retain(|mirror| {
            !staged
                .iter()
                .any(|done| done.mirror_rel == mirror.mirror_rel)
        });
        mirrors
    }

    pub(super) fn stage_scratch(
        &self,
        tool_id: &str,
        tool: &RealTool,
        files: &BTreeMap<String, String>,
        siblings: &BTreeMap<String, String>,
        resolve: &BTreeMap<String, String>,
    ) -> Result<StagedScratch, RunnerError> {
        let scratch = fresh_scratch(&self.scratch_parent, tool_id)?;
        let mut mirrors = Vec::with_capacity(
            files.len() + siblings.len() + resolve.len() + tool.tool_files.len() + 1,
        );
        let mut pairs = Vec::with_capacity(files.len());
        for (path, text) in files {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch file: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            pairs.push((path.clone(), absolute));
        }
        let mut sibling_pairs = Vec::with_capacity(siblings.len());
        for (path, text) in siblings {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch sibling: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            sibling_pairs.push((path.clone(), absolute));
        }
        let mut resolve_pairs = Vec::with_capacity(resolve.len());
        for (path, text) in resolve {
            let absolute = scratch
                .resolve(Path::new(path))
                .map_err(|err| execution(tool_id, format!("scratch resolve: {err}")))?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
            resolve_pairs.push((path.clone(), absolute));
        }
        let tools = Self::mirror_tool_files(tool_id, tool, &mirrors, scratch.root());
        mirrors.extend(tools);
        write_all(&scratch, tool_id, &mirrors)?;
        Ok(StagedScratch {
            scratch,
            pairs,
            sibling_pairs,
            resolve_pairs,
        })
    }

    #[expect(
        clippy::unused_self,
        reason = "same receiver shape as the sibling tool methods"
    )]
    pub(super) fn config_abs(
        &self,
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
    ) -> Result<Option<PathBuf>, RunnerError> {
        if TIER_BASE_TOOLS.contains(&tool_id) && tool.upstream_diagnostics.is_empty() {
            let Some(rel) = tool.config_rel.as_deref() else {
                return Err(missing_tier_base(tool_id));
            };
            return scratch
                .resolve(Path::new(rel))
                .map(Some)
                .map_err(|err| execution(tool_id, format!("scratch config: {err}")));
        }
        if matches!(tool_id, "kubeconform" | "helm") && tool.upstream_diagnostics.is_empty() {
            let Some(rel) = tool.config_rel.as_deref() else {
                return Err(execution(
                    tool_id,
                    format!("{tool_id} needs a kubernetes_schemas config"),
                ));
            };
            return scratch
                .resolve(Path::new(rel))
                .map(Some)
                .map_err(|err| execution(tool_id, format!("scratch config: {err}")));
        }
        tool.config_rel
            .as_deref()
            .map(|rel| {
                scratch
                    .resolve(Path::new(rel))
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))
            })
            .transpose()
    }

    pub(super) fn scalafix_classpath(
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
    ) -> Result<Vec<PathBuf>, RunnerError> {
        let jars = |prefix: &'static str| {
            tool.tool_files
                .iter()
                .filter(move |(rel, _)| rel.starts_with(prefix) && super::has_extension(rel, "jar"))
                .map(|(rel, _)| scratch.root().join(rel))
        };
        let semanticdb = scratch.root().join("dx-scalafix").join("semanticdb");
        let mut classpath = Vec::new();
        for (index, jar) in jars("dx-scalafix/owned/").enumerate() {
            let into = semanticdb.join(index.to_string());
            extract_semanticdb(&jar, &into).map_err(|err| {
                execution(tool_id, format!("semanticdb from {}: {err}", jar.display()))
            })?;
            classpath.push(into);
        }
        classpath.extend(jars("dx-scalafix/owned/"));
        classpath.extend(jars("dx-scalafix/aux/"));
        Ok(classpath)
    }

    pub(super) fn fsharplint_context(
        tool_id: &str,
        tool: &RealTool,
        scratch: &Scratch,
        pairs: &[(String, PathBuf)],
    ) -> Result<FsharplintInputs, RunnerError> {
        let bytes = tool
            .tool_files
            .iter()
            .find(|(rel, _)| rel == FSHARPLINT_CONTEXT_REL)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| {
                execution(
                    tool_id,
                    "fsharplint needs the target compile context; the F# aspect did not supply it"
                        .to_owned(),
                )
            })?;
        let doc: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|err| execution(tool_id, format!("fsharplint context: {err}")))?;
        let strings = |key: &str| -> Result<Vec<String>, RunnerError> {
            doc.get(key)
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| execution(tool_id, format!("fsharplint context: missing {key}")))?
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        execution(tool_id, format!("fsharplint context: bad {key} entry"))
                    })
                })
                .collect()
        };
        let mut compile = Vec::new();
        for source in strings("sources")? {
            let Some((_, absolute)) = pairs.iter().find(|(workspace, _)| *workspace == source)
            else {
                return Err(execution(
                    tool_id,
                    format!("fsharplint context: compile source {source} was not staged"),
                ));
            };
            compile.push(absolute.clone());
        }
        let refs = strings("refs")?
            .iter()
            .map(|rel| scratch.root().join(rel))
            .collect();
        Ok(FsharplintInputs {
            compile,
            refs,
            defines: strings("defines")?,
            langversion: doc
                .get("langversion")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    pub(super) fn rustfmt_edition(tool: &RealTool) -> Result<&str, RunnerError> {
        const TOOL_ID: &str = "rustfmt";
        tool.edition.as_deref().ok_or_else(|| {
            execution(
                TOOL_ID,
                "missing tool edition: the quality aspect must pass the CrateInfo edition as the request tool edition".to_owned(),
            )
        })
    }

    pub(super) fn gofumpt_lang(tool: &RealTool) -> Result<&str, RunnerError> {
        const TOOL_ID: &str = "gofumpt";
        tool.edition.as_deref().ok_or_else(|| {
            execution(
                TOOL_ID,
                "missing tool edition: the quality aspect must pass the Go language version via --tool-edition".to_owned(),
            )
        })
    }

    pub(super) fn latexindent_cruft(
        tool_id: &str,
        scratch: &Scratch,
    ) -> Result<PathBuf, RunnerError> {
        let dir = scratch.root().join(".dx-latexindent");
        std::fs::create_dir_all(&dir)
            .map_err(|err| execution(tool_id, format!("cruft dir: {err}")))?;
        Ok(dir)
    }

    pub(super) fn biome_config_dir(
        tool: &RealTool,
        scratch: &Scratch,
    ) -> Result<PathBuf, RunnerError> {
        const TOOL_ID: &str = "biome";
        let Some(rel) = tool.config_rel.as_deref() else {
            return Err(missing_tier_base(TOOL_ID));
        };
        let dir_rel = Path::new(rel)
            .parent()
            .and_then(|parent| parent.to_str())
            .unwrap_or_default();
        let dir = if dir_rel.is_empty() {
            scratch.root().to_path_buf()
        } else {
            scratch
                .resolve(Path::new(dir_rel))
                .map_err(|err| execution(TOOL_ID, format!("scratch config: {err}")))?
        };
        Ok(dir)
    }

    pub(super) fn ffmt_config_dir(tool: &RealTool) -> Result<String, RunnerError> {
        let Some(rel) = tool.config_rel.as_deref() else {
            return Err(missing_tier_base("ffmt"));
        };
        match Path::new(rel).file_name().and_then(|name| name.to_str()) {
            Some("ffmt.toml" | ".ffmt.toml") => Ok(parent_rel(rel)),
            _ => Err(execution(
                "ffmt",
                format!("config {rel} must be named ffmt.toml or .ffmt.toml"),
            )),
        }
    }

    pub(super) fn cwd_rel(tool_id: &str, config_rel: Option<&str>) -> String {
        match tool_id {
            "buildifier" | "vale" | "staticcheck" => config_rel.map(parent_rel).unwrap_or_default(),
            "ruff" => config_rel
                .filter(|rel| !rel.starts_with("_external/"))
                .map(parent_rel)
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
}

pub(super) fn ansible_collections_roots<'a>(
    rels: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<String>, String> {
    let mut owners: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut roots = std::collections::BTreeSet::new();
    for rel in rels {
        let parts: Vec<&str> = rel.split('/').collect();
        let Some(at) = parts.iter().position(|part| *part == "ansible_collections") else {
            continue;
        };
        if parts.len() < at + 4 {
            continue;
        }
        let (Some(prefix), Some(namespace), Some(name)) =
            (parts.get(..at), parts.get(at + 1), parts.get(at + 2))
        else {
            continue;
        };
        let root = prefix.join("/");
        let key = ((*namespace).to_owned(), (*name).to_owned());
        match owners.get(&key) {
            Some(first) if *first != root => {
                return Err(format!(
                    "ansible collection {}.{} is staged under two roots ({:?} and {:?})",
                    key.0, key.1, first, root
                ));
            }
            Some(_) => {}
            None => {
                owners.insert(key, root.clone());
            }
        }
        roots.insert(root);
    }
    Ok(roots.into_iter().collect())
}

pub(super) fn qml_import_roots(
    tool_id: &str,
    context: &[StagedPair],
) -> Result<Vec<PathBuf>, RunnerError> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for (workspace, absolute) in context {
        if absolute.file_name().and_then(|name| name.to_str()) != Some("qmldir") {
            continue;
        }
        let text = std::fs::read_to_string(absolute)
            .map_err(|err| execution(tool_id, format!("read {workspace}: {err}")))?;
        let Some(uri) = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("module "))
            .map(str::trim)
            .next()
        else {
            continue;
        };
        let mut root = absolute.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut mirrors = true;
        for segment in uri.rsplit('.') {
            if root.file_name().and_then(|name| name.to_str()) != Some(segment) {
                mirrors = false;
                break;
            }
            root.pop();
        }
        if !mirrors {
            continue;
        }
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}

pub(super) fn place_qmllint_settings(
    tool_id: &str,
    config: &Path,
    sources: &[StagedPair],
) -> Result<(), RunnerError> {
    let mut placed: Vec<PathBuf> = Vec::new();
    for (workspace, absolute) in sources {
        let Some(dir) = absolute.parent() else {
            continue;
        };
        let target = dir.join(".qmllint.ini");
        if target == config || placed.contains(&target) {
            continue;
        }
        dx_atomic_fs::copy_scratch(config, &target)
            .map_err(|err| execution(tool_id, format!("place settings for {workspace}: {err}")))?;
        placed.push(target);
    }
    Ok(())
}

fn extract_semanticdb(jar: &Path, into: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(into) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("clearing {}: {err}", into.display())),
    }
    dx_archive::extract_matching(
        jar,
        dx_archive::Format::Zip,
        into,
        dx_archive::Limits::default(),
        |name| name.starts_with("META-INF/semanticdb/"),
    )
    .map(drop)
    .map_err(|err| err.to_string())
}

#[cfg(test)]
mod semanticdb_tests {
    use super::{RealBackend, RealTool, extract_semanticdb};

    fn jar(dir: &std::path::Path, members: &[&str]) -> std::path::PathBuf {
        let path = dir.join("owned.jar");
        let mut writer = dx_archive_testing::RawZip::new();
        for name in members {
            writer.add_file(name, name.as_bytes()).expect("add");
        }
        std::fs::write(&path, writer.finish().expect("finish")).expect("jar");
        path
    }

    #[test]
    fn only_semanticdb_members_are_extracted_from_a_fat_jar() {
        let dir = tempfile::tempdir().expect("tempdir");
        let jar = jar(
            dir.path(),
            &[
                "META-INF/LICENSE",
                "META-INF/license/NOTICE",
                "META-INF/semanticdb/src/A.scala.semanticdb",
            ],
        );
        let into = dir.path().join("out");
        extract_semanticdb(&jar, &into).expect("extract");
        assert!(
            into.join("META-INF/semanticdb/src/A.scala.semanticdb")
                .is_file()
        );
        assert!(!into.join("META-INF/LICENSE").exists());
    }

    #[test]
    fn scalafix_classpath_is_repeatable_on_one_scratch() {
        let dir = tempfile::tempdir().expect("tempdir");
        let scratch = quality_adapter::exec::Scratch::create(dir.path()).expect("scratch");
        let rel = "dx-scalafix/owned/a.jar";
        let source = jar(dir.path(), &["META-INF/semanticdb/src/A.scala.semanticdb"]);
        let target = scratch.root().join(rel);
        std::fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
        std::fs::copy(&source, &target).expect("copy");
        let tool = RealTool {
            binary: std::path::PathBuf::new(),
            extra_env: Vec::new(),
            config_rel: None,
            edition: None,
            dialect: None,
            args: Vec::new(),
            tool_files: vec![(rel.to_owned(), Vec::new())],
            upstream_diagnostics: Vec::new(),
            search_paths: Vec::new(),
            path_tools: Vec::new(),
        };
        let first = RealBackend::scalafix_classpath("scalafix", &tool, &scratch).expect("first");
        let second = RealBackend::scalafix_classpath("scalafix", &tool, &scratch).expect("second");
        assert_eq!(first, second);
        assert!(
            first[0]
                .join("META-INF/semanticdb/src/A.scala.semanticdb")
                .is_file()
        );
    }

    #[test]
    fn a_dot_dot_member_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let jar = jar(dir.path(), &["META-INF/semanticdb/../../../escape"]);
        let err = extract_semanticdb(&jar, &dir.path().join("out")).expect_err("refused");
        assert!(err.contains("'..'"), "{err}");
        assert!(!dir.path().join("escape").exists());
    }
}

#[cfg(test)]
mod ansible_roots_tests {
    use super::ansible_collections_roots;

    #[test]
    fn roots_are_sorted_unique_and_include_the_scratch_root() {
        let roots = ansible_collections_roots([
            "z/ansible_collections/a/b/x",
            "ansible_collections/c/d/y",
            "z/ansible_collections/a/b/y",
            "cfg/.ansible-lint",
        ])
        .expect("roots");
        assert_eq!(roots, ["", "z"]);
    }

    #[test]
    fn the_same_collection_under_two_roots_is_rejected() {
        let err = ansible_collections_roots([
            "a/ansible_collections/x/y/f",
            "b/ansible_collections/x/y/f",
        ])
        .expect_err("duplicate");
        assert!(err.contains("x.y"), "{err}");
    }
}

#[cfg(test)]
mod bicep_config_tests {
    use super::pin_bicep_cache;
    use std::path::Path;

    #[test]
    fn commented_config_is_still_pinned_to_scratch() {
        let bound = b"{\n // host cache\n \"cacheRootDirectory\": \"/host/x//y\", /* c */\n \"analyzers\": {}\n}".to_vec();
        let out = String::from_utf8(pin_bicep_cache(Some(bound), Path::new("/scratch"))).unwrap();
        assert!(!out.contains("/host"), "{out}");
        assert!(out.contains("/scratch"), "{out}");
        assert!(out.contains("analyzers"), "{out}");
    }
}
