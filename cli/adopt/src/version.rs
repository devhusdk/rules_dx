use std::path::Path;

use dx_module_bazel::ModuleFile;

use super::AdoptError;

pub const DX_VERSION: &str = "0.0.0";
pub const MODULE_VERSION: &str = "0.0.0";

pub fn pin_is_valid(pin: &str) -> bool {
    !pin.is_empty() && semver::Version::parse(pin).is_ok()
}

pub fn pin_rewrite_hint(version: &str) -> String {
    format!("write {version} to .dx/version")
}

pub fn version_move_hint(from: &str, to: &str) -> String {
    if version_pin_matches_module(from, to) {
        "dx version --check".to_owned()
    } else if crate::migrate::migrate_is_upgrade(from, to) {
        format!("dx upgrade --from {from} --to {to}")
    } else {
        "dx rollback".to_owned()
    }
}

pub fn pin_fix_hint(pinned: &str, module_version: &str) -> String {
    if !pin_is_valid(pinned) {
        pin_rewrite_hint(module_version)
    } else if version_pin_matches_module(pinned, module_version) {
        "dx version --check".to_owned()
    } else if crate::migrate::migrate_is_upgrade(pinned, module_version) {
        format!("dx upgrade --from {pinned} --to {module_version} --apply")
    } else {
        pin_rewrite_hint(module_version)
    }
}

pub fn version_pin_matches_module(dx_version: &str, module_version: &str) -> bool {
    if dx_version.is_empty() || module_version.is_empty() {
        return false;
    }
    let dx = match semver::Version::parse(dx_version) {
        Ok(dx) => dx,
        Err(_) => return false,
    };
    let module = match semver::Version::parse(module_version) {
        Ok(module) => module,
        Err(_) => return false,
    };
    dx == module
}

pub fn read_version_pin(root: &Path) -> Result<String, AdoptError> {
    let raw = std::fs::read_to_string(root.join(".dx/version")).map_err(|e| {
        AdoptError::ReadVersionPin {
            detail: e.to_string(),
        }
    })?;
    Ok(raw.trim().to_owned())
}

pub fn write_version_pin(root: &Path, version: &str) -> Result<(), AdoptError> {
    if version.is_empty() {
        return Err(AdoptError::EmptyVersion);
    }
    let dir = root.join(".dx");
    std::fs::create_dir_all(&dir).map_err(|e| AdoptError::CreateDxDir {
        detail: e.to_string(),
    })?;
    dx_atomic_fs::write_atomic(&dir.join("version"), format!("{version}\n").as_bytes()).map_err(
        |e| AdoptError::WriteVersionPin {
            detail: e.to_string(),
        },
    )?;
    Ok(())
}

pub const VERSION_HISTORY_REL: &str = ".dx/version-history.json";

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRecord {
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub previous: Option<String>,
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    pub current: Option<String>,
}

pub(crate) fn render_pin_record(record: &PinRecord) -> Result<String, AdoptError> {
    serde_json::to_string(record).map_err(|e| AdoptError::WritePinRecord {
        detail: e.to_string(),
    })
}

pub(crate) fn parse_pin_record(text: &str) -> Result<PinRecord, AdoptError> {
    serde_json::from_str::<PinRecord>(text).map_err(|e| AdoptError::PinRecordUnreadable {
        detail: e.to_string(),
    })
}

pub fn record_pin_operation(
    root: &Path,
    previous: Option<&str>,
    current: Option<&str>,
) -> Result<(), AdoptError> {
    let record = PinRecord {
        previous: previous.map(str::to_owned),
        current: current.map(str::to_owned),
    };
    let dir = root.join(".dx");
    std::fs::create_dir_all(&dir).map_err(|e| AdoptError::CreateDxDir {
        detail: e.to_string(),
    })?;
    let rendered = render_pin_record(&record)?;
    dx_atomic_fs::write_atomic(
        &dir.join("version-history.json"),
        format!("{rendered}\n").as_bytes(),
    )
    .map_err(|e| AdoptError::WritePinRecord {
        detail: e.to_string(),
    })?;
    Ok(())
}

pub fn read_pin_record(root: &Path) -> Result<PinRecord, AdoptError> {
    let raw = std::fs::read_to_string(root.join(VERSION_HISTORY_REL)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            return AdoptError::PinRecordMissing;
        }
        AdoptError::PinRecordUnreadable {
            detail: e.to_string(),
        }
    })?;
    parse_pin_record(&raw)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependency {
    pub workspace_module: Option<(String, Option<String>)>,
    pub declared: Option<String>,
    pub override_kind: Option<String>,
    pub override_source: Option<String>,
}

impl ModuleDependency {
    /// Returns the rules_dx version Bazel resolves.
    pub fn resolved_version(&self) -> Option<&str> {
        match (
            self.override_kind.as_deref(),
            self.override_source.as_deref(),
        ) {
            (Some("single_version_override"), Some(version)) if !version.is_empty() => {
                Some(version)
            }
            _ => self.declared.as_deref(),
        }
    }
}

/// Reads the rules_dx dependency from MODULE.bazel and every file it includes.
pub fn parse_module_dependency(
    text: &str,
    read_include: &dyn Fn(&str) -> Result<String, String>,
) -> Result<ModuleDependency, String> {
    let mut dependency = ModuleDependency {
        workspace_module: None,
        declared: None,
        override_kind: None,
        override_source: None,
    };
    let modules = dx_module_bazel::load_with_includes(text, read_include)?;
    for call in modules.iter().flat_map(ModuleFile::calls) {
        let name = call.name.as_str();
        if name == "module" {
            if dependency.workspace_module.is_none() {
                if let Some(module_name) = call.str("name") {
                    dependency.workspace_module = Some((
                        module_name.to_owned(),
                        call.str("version").map(str::to_owned),
                    ));
                }
            }
            continue;
        }
        if name == "bazel_dep" {
            if dependency.declared.is_none() && call.str("name") == Some("rules_dx") {
                dependency.declared = call.str("version").map(str::to_owned);
            }
            continue;
        }
        if !name.ends_with("_override")
            || dependency.override_kind.is_some()
            || call.str("module_name") != Some("rules_dx")
        {
            continue;
        }
        let mut source = call.str("path").or_else(|| call.str("remote"));
        let pinned = match (name, source, call.str("commit")) {
            ("git_override", Some(remote), Some(commit)) => Some(format!("{remote}@{commit}")),
            _ => None,
        };
        if source.is_none() {
            source = call.str("version");
        }
        dependency.override_kind = Some(name.to_owned());
        dependency.override_source = pinned.or_else(|| source.map(str::to_owned));
    }
    Ok(dependency)
}

pub fn local_path_overrides(text: &str) -> Vec<String> {
    let Ok(module) = ModuleFile::parse(text) else {
        return Vec::new();
    };
    module
        .calls_named("local_path_override")
        .filter_map(|call| call.str("path").map(str::to_owned))
        .collect()
}

pub fn read_module_dependency(root: &Path) -> Result<ModuleDependency, AdoptError> {
    let text = std::fs::read_to_string(root.join("MODULE.bazel")).map_err(|e| {
        AdoptError::ReadModuleFile {
            detail: e.to_string(),
        }
    })?;
    let read_include = |label: &str| -> Result<String, String> {
        let rel = dx_module_bazel::include_rel("", label)?;
        std::fs::read_to_string(root.join(rel))
            .map_err(|e| format!("cannot read included {label}: {e}"))
    };
    parse_module_dependency(&text, &read_include)
        .map_err(|detail| AdoptError::LoadModuleFile { detail })
}

pub fn module_dependency_status(dependency: &ModuleDependency) -> &'static str {
    if let Some((name, _)) = &dependency.workspace_module {
        if name == "rules_dx" {
            return "ok";
        }
    }
    match dependency.resolved_version() {
        Some(version) if version == MODULE_VERSION => "ok",
        Some(_) => "warn",
        None if dependency.override_kind.is_some() => "ok",
        None => "warn",
    }
}

pub fn render_module_dependency(dependency: &ModuleDependency) -> String {
    if let Some((name, version)) = &dependency.workspace_module {
        if name == "rules_dx" {
            return match version {
                Some(version) => format!("rules_dx {version} (workspace module)"),
                None => "rules_dx unknown version (workspace module)".to_owned(),
            };
        }
    }
    match (
        &dependency.declared,
        &dependency.override_kind,
        &dependency.override_source,
    ) {
        (Some(version), None, _) => format!("rules_dx {version}"),
        (Some(version), Some(kind), Some(source)) => {
            format!("rules_dx {version} via {kind} {source}")
        }
        (Some(version), Some(kind), None) => format!("rules_dx {version} via {kind}"),
        (None, Some(kind), Some(source)) if kind == "single_version_override" => {
            format!("rules_dx {source} via {kind}")
        }
        (None, Some(kind), Some(source)) => {
            format!("rules_dx unknown version via {kind} {source}")
        }
        (None, Some(kind), None) => format!("rules_dx unknown version via {kind}"),
        (None, None, _) => "unknown (no rules_dx dependency in MODULE.bazel)".to_owned(),
    }
}

#[cfg(test)]
pub(crate) fn repo_text(path: &str) -> String {
    let source = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    std::fs::read_to_string(Path::new(&source).join(workspace).join(path))
        .unwrap_or_else(|error| panic!("{path} ships as test data: {error}"))
}

#[cfg(test)]
pub(crate) fn repo_module_version() -> String {
    let module = ModuleFile::parse(&repo_text("MODULE.bazel")).expect("MODULE.bazel parses");
    let call = module
        .calls_named("module")
        .next()
        .expect("MODULE.bazel has a module() call");
    call.str("version")
        .expect("module() has a literal version argument")
        .to_owned()
}

#[cfg(test)]
pub(crate) mod consumer_setup {
    use dx_module_bazel::{Call, ModuleFile, Value};

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ConsumerSetup {
        pub deps: Vec<(String, String)>,
        pub extensions: Vec<(String, String)>,
        pub repos: Vec<(String, String)>,
        pub registered: Vec<String>,
        pub tags: Vec<Call>,
        pub repo_names: Vec<(String, String)>,
    }

    impl ConsumerSetup {
        pub fn dep_version(&self, name: &str) -> Option<&str> {
            self.deps
                .iter()
                .find(|(dep, _)| dep == name)
                .map(|(_, version)| version.as_str())
        }

        pub fn repo_name(&self, name: &str) -> Option<&str> {
            self.repo_names
                .iter()
                .find(|(dep, _)| dep == name)
                .map(|(_, repo)| repo.as_str())
        }

        pub fn tag_arg(&self, tag: &str, key: &str) -> Option<String> {
            self.tag(tag)
                .and_then(|call| call.str(key))
                .map(str::to_owned)
        }

        pub fn tag_list_arg(&self, tag: &str, key: &str) -> Vec<String> {
            self.tag(tag)
                .map(|call| call.str_list(key).into_iter().map(str::to_owned).collect())
                .unwrap_or_default()
        }

        fn tag(&self, tag: &str) -> Option<&Call> {
            self.tags.iter().find(|call| call.name == tag)
        }
    }

    fn literal(value: &Value) -> Option<String> {
        match value {
            Value::Str(literal) => Some(literal.value.clone()),
            _ => None,
        }
    }

    pub fn parse_consumer_setup(text: &str) -> ConsumerSetup {
        let mut setup = ConsumerSetup {
            deps: Vec::new(),
            extensions: Vec::new(),
            repos: Vec::new(),
            registered: Vec::new(),
            tags: Vec::new(),
            repo_names: Vec::new(),
        };
        let Ok(module) = ModuleFile::parse(text) else {
            return setup;
        };
        for call in module.calls() {
            match call.name.as_str() {
                "bazel_dep" => {
                    if let (Some(dep), Some(version)) = (call.str("name"), call.str("version")) {
                        setup.deps.push((dep.to_owned(), version.to_owned()));
                        if let Some(repo) = call.str("repo_name") {
                            setup.repo_names.push((dep.to_owned(), repo.to_owned()));
                        }
                    }
                }
                "use_extension" => {
                    if let [label, tag, ..] = call.positional.as_slice() {
                        if let (Some(label), Some(tag)) = (literal(label), literal(tag)) {
                            setup.extensions.push((label, tag));
                        }
                    }
                }
                "use_repo" => {
                    if let [Value::Ident(var), repos @ ..] = call.positional.as_slice() {
                        for repo in repos.iter().filter_map(literal) {
                            setup.repos.push((var.clone(), repo));
                        }
                    }
                }
                "register_toolchains" => {
                    setup
                        .registered
                        .extend(call.positional.iter().filter_map(literal));
                }
                name if name.contains('.') => setup.tags.push(call.clone()),
                _ => {}
            }
        }
        setup
    }
}

#[cfg(test)]
mod tests {
    use super::consumer_setup::parse_consumer_setup;
    use super::*;

    #[test]
    fn pin_holds_only_on_equal_nonempty_versions() {
        assert!(version_pin_matches_module("1.2.3", "1.2.3"));
        assert!(!version_pin_matches_module("1.2.3", "1.2.4"));
        assert!(!version_pin_matches_module("", ""));
        assert!(!version_pin_matches_module("1.2.3", ""));
        assert!(!version_pin_matches_module("abc", "abc"));
        assert!(!version_pin_matches_module("v1.2.3", "v1.2.3"));
        assert!(!version_pin_matches_module("1.2", "1.2"));
        assert!(version_pin_matches_module("1.2.3-alpha.1", "1.2.3-alpha.1"));
        assert!(!version_pin_matches_module(
            "1.2.3-alpha.1",
            "1.2.3-alpha.2"
        ));
        assert!(!version_pin_matches_module("1.2.3", "1.2.3-alpha.1"));
    }

    #[test]
    fn pin_fix_hint_names_only_surviving_commands() {
        assert_eq!(pin_fix_hint("1.2.3", "1.2.3"), "dx version --check");
        assert_eq!(
            pin_fix_hint("1.2.3", "2.0.0"),
            "dx upgrade --from 1.2.3 --to 2.0.0 --apply"
        );
        assert_eq!(pin_fix_hint("2.0.0", "1.2.3"), "write 1.2.3 to .dx/version");
        for bad in ["", "v1", "1.2", "garbage"] {
            assert_eq!(pin_fix_hint(bad, "1.2.3"), "write 1.2.3 to .dx/version");
        }
        for (pin, module) in [
            ("1.2.3", "1.2.3"),
            ("1.2.3", "2.0.0"),
            ("2.0.0", "1.2.3"),
            ("bad", "1.2.3"),
        ] {
            assert!(!pin_fix_hint(pin, module).contains("dx rollback"));
        }
    }

    #[test]
    fn pin_is_valid_requires_semver() {
        assert!(pin_is_valid("1.2.3"));
        assert!(pin_is_valid("1.2.3-alpha.1"));
        assert!(!pin_is_valid(""));
        assert!(!pin_is_valid("v1.2.3"));
        assert!(!pin_is_valid("1.2"));
    }

    #[test]
    fn delivered_version_matches_module() {
        let module_version = repo_module_version();
        assert_eq!(DX_VERSION, module_version);
        assert_eq!(MODULE_VERSION, module_version);
        assert!(version_pin_matches_module(DX_VERSION, MODULE_VERSION));
    }

    #[test]
    fn pin_record_round_trips_through_json() {
        for record in [
            PinRecord {
                previous: None,
                current: Some("0.1.0".to_owned()),
            },
            PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            },
            PinRecord {
                previous: Some("0.1.0".to_owned()),
                current: None,
            },
            PinRecord {
                previous: None,
                current: None,
            },
            PinRecord {
                previous: Some("a\"b\\c\nd\teé".to_owned()),
                current: Some("\u{1}tail".to_owned()),
            },
        ] {
            let rendered = render_pin_record(&record).expect("render");
            assert_eq!(parse_pin_record(&rendered).expect("round trip"), record);
            assert_eq!(
                parse_pin_record(&format!("  {rendered} \n")).expect("padded"),
                record
            );
        }
        assert_eq!(
            render_pin_record(&PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            })
            .expect("render"),
            "{\"previous\":\"0.0.0\",\"current\":\"0.1.0\"}"
        );
        assert_eq!(
            render_pin_record(&PinRecord {
                previous: None,
                current: None,
            })
            .expect("render"),
            "{\"previous\":null,\"current\":null}"
        );
        assert_eq!(
            render_pin_record(&PinRecord {
                previous: Some("\u{1}a/b".to_owned()),
                current: None,
            })
            .expect("render"),
            "{\"previous\":\"\\u0001a/b\",\"current\":null}"
        );
        assert_eq!(
            parse_pin_record("{\"previous\":\"\\u0008\",\"current\":\"\\u000c\"}")
                .expect("legacy escapes"),
            PinRecord {
                previous: Some("\u{8}".to_owned()),
                current: Some("\u{c}".to_owned()),
            }
        );
        assert_eq!(
            parse_pin_record("{\"previous\":\"\\ud83d\\ude00\",\"current\":null}")
                .expect("surrogate pair"),
            PinRecord {
                previous: Some("\u{1f600}".to_owned()),
                current: None,
            }
        );
    }

    #[test]
    fn pin_record_rejects_anything_but_its_shape() {
        for text in [
            "",
            "{}",
            "{\"previous\":null}",
            "{\"previous\":null,\"current\":null",
            "{\"previous\":null,\"current\":null}}",
            "{\"previous\":null,\"current\":null} trailing",
            "{\"previous\":\"0.0.0\"}",
            "{\"previous\":0,\"current\":null}",
            "{\"previous\":null,\"current\":1.5}",
            "{\"previous\":null,\"CURRENT\":null}",
            "{\"previous\":null,\"previous\":null,\"current\":null}",
            "{\"previous\":\"0.1",
            "{\"current\":null}",
            "{\"previous\":\"\\ud800\",\"current\":null}",
            "{\"previous\":\"a\u{1}b\",\"current\":null}",
            "{\"previous\":\"unterminated,\"current\":null}",
            "{\"previous\":\"bad\\escape\",\"current\":null}",
            "{\"previous\":\"bad\\u12\",\"current\":null}",
            "{\"previous\":\"bad\\uzzzz\",\"current\":null}",
            "{\"previous\":nul,\"current\":null}",
            "{\"previous\":null,\"current\":null,\"extra\":1}",
            "null",
        ] {
            assert!(parse_pin_record(text).is_err(), "{text:?}");
        }
        assert_eq!(
            parse_pin_record("{\"previous\":null,\"current\":null}").expect("canonical"),
            PinRecord {
                previous: None,
                current: None,
            }
        );
        assert_eq!(
            parse_pin_record("{\"current\":\"0.1.0\",\"previous\":\"0.0.0\"}").expect("reordered"),
            PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            }
        );
    }

    #[test]
    fn pin_record_error_strings_name_the_blocker() {
        assert_eq!(
            AdoptError::PinRecordMissing.to_string(),
            "no recorded pin operation: nothing to roll back"
        );
        assert_eq!(
            AdoptError::PinRecordUnreadable {
                detail: "denied".to_owned(),
            }
            .to_string(),
            "unreadable pin record: denied"
        );
        assert_eq!(
            AdoptError::WritePinRecord {
                detail: "denied".to_owned(),
            }
            .to_string(),
            "write pin record: denied"
        );
        assert_eq!(
            AdoptError::NoQualifiedManifest {
                from: "1.2.3".to_owned(),
                to: "2.0.0".to_owned(),
                manifest: "migrate-v1-to-v2.json".to_owned(),
            }
            .to_string(),
            "no qualified migration 1.2.3 -> 2.0.0: manifest migrate-v1-to-v2.json is not shipped"
        );
    }

    #[test]
    fn pin_record_file_round_trips_and_missing_is_explicit() {
        let root =
            std::env::temp_dir().join(format!("dx-adopt-record-{}-roundtrip", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            read_pin_record(&root).unwrap_err(),
            AdoptError::PinRecordMissing
        );
        record_pin_operation(&root, Some("0.0.0"), Some("0.1.0")).expect("record");
        assert_eq!(
            read_pin_record(&root).expect("read back"),
            PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            }
        );
        record_pin_operation(&root, None, None).expect("blank record");
        assert_eq!(
            read_pin_record(&root).expect("blank back"),
            PinRecord {
                previous: None,
                current: None,
            }
        );
        std::fs::write(root.join(VERSION_HISTORY_REL), "not json").expect("corrupt");
        assert!(read_pin_record(&root).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn no_includes(label: &str) -> Result<String, String> {
        Err(format!("unexpected include {label}"))
    }

    fn dep(text: &str) -> ModuleDependency {
        parse_module_dependency(text, &no_includes).expect("parses")
    }

    #[test]
    fn module_dependency_reads_includes_and_fails_loudly() {
        let read = |_: &str| -> Result<String, String> {
            Ok("bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\ngit_override(module_name = \"rules_dx\", remote = \"https://example.test/dx\", commit = \"abc123\")\n".to_owned())
        };
        let included =
            parse_module_dependency("include(\"//:dx.MODULE.bazel\")\n", &read).expect("parses");
        assert_eq!(
            render_module_dependency(&included),
            "rules_dx 1.2.3 via git_override https://example.test/dx@abc123"
        );
        let failing = |label: &str| -> Result<String, String> {
            Err(format!("cannot read included {label}: gone"))
        };
        let error = parse_module_dependency("include(\"//:dx.MODULE.bazel\")\n", &failing)
            .expect_err("fails");
        assert!(error.contains("cannot read included"), "{error}");
    }

    #[test]
    fn module_dependency_names_self_declared_and_overrides() {
        let me = dep("module(name = \"rules_dx\", version = \"0.0.0\")\n");
        assert_eq!(
            me.workspace_module,
            Some(("rules_dx".to_owned(), Some("0.0.0".to_owned())))
        );
        assert_eq!(
            render_module_dependency(&me),
            "rules_dx 0.0.0 (workspace module)"
        );
        let bare = dep("module(name = \"rules_dx\")\n");
        assert_eq!(
            render_module_dependency(&bare),
            "rules_dx unknown version (workspace module)"
        );
        let consumer =
            dep("module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n");
        assert_eq!(consumer.declared, Some("1.2.3".to_owned()));
        assert_eq!(consumer.override_kind, None);
        assert_eq!(render_module_dependency(&consumer), "rules_dx 1.2.3");
        let local = dep(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nlocal_path_override(module_name = \"rules_dx\", path = \"../rules_dx\")\n",
        );
        assert_eq!(local.override_kind, Some("local_path_override".to_owned()));
        assert_eq!(local.override_source, Some("../rules_dx".to_owned()));
        assert_eq!(
            render_module_dependency(&local),
            "rules_dx 1.2.3 via local_path_override ../rules_dx"
        );
        let git = dep(
            "module(name = \"demo\")\ngit_override(module_name = \"rules_dx\", remote = \"https://example.test/dx\", commit = \"abc123\")\n",
        );
        assert_eq!(
            render_module_dependency(&git),
            "rules_dx unknown version via git_override https://example.test/dx@abc123"
        );
        let single = dep(
            "module(name = \"demo\")\nsingle_version_override(module_name = \"rules_dx\", version = \"2.0.0\")\n",
        );
        assert_eq!(
            render_module_dependency(&single),
            "rules_dx 2.0.0 via single_version_override"
        );
        assert_eq!(module_dependency_status(&me), "ok");
        assert_eq!(module_dependency_status(&bare), "ok");
        assert_eq!(module_dependency_status(&consumer), "warn");
        assert_eq!(module_dependency_status(&local), "warn");
        assert_eq!(module_dependency_status(&git), "ok");
        assert_eq!(module_dependency_status(&single), "warn");
        assert_eq!(single.resolved_version(), Some("2.0.0"));
        assert_eq!(git.resolved_version(), None);
        assert_eq!(local.resolved_version(), Some("1.2.3"));
        let pinned = dep(&format!(
            "module(name = \"demo\")\nsingle_version_override(module_name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"
        ));
        assert_eq!(module_dependency_status(&pinned), "ok");
        let replaced = dep(&format!(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nsingle_version_override(module_name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"
        ));
        assert_eq!(module_dependency_status(&replaced), "ok");
        let skewed = dep(&format!(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\nsingle_version_override(module_name = \"rules_dx\", version = \"2.0.0\")\n"
        ));
        assert_eq!(module_dependency_status(&skewed), "warn");
        let patched = dep(&format!(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\nsingle_version_override(module_name = \"rules_dx\", patch_strip = 1)\n"
        ));
        assert_eq!(module_dependency_status(&patched), "ok");
        let blank = dep(&format!(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\nsingle_version_override(module_name = \"rules_dx\", version = \"\")\n"
        ));
        assert_eq!(blank.resolved_version(), Some(MODULE_VERSION));
        assert_eq!(module_dependency_status(&blank), "ok");
        let matching = dep(&format!(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"{MODULE_VERSION}\")\n"
        ));
        assert_eq!(module_dependency_status(&matching), "ok");
        let none = dep("module(name = \"demo\")\n");
        assert_eq!(module_dependency_status(&none), "warn");
    }

    #[test]
    fn local_path_overrides_lists_every_override_path() {
        let paths = local_path_overrides(
            "module(name = \"demo\")\nlocal_path_override(module_name = \"a\", path = \"../a\")\ngit_override(module_name = \"b\", remote = \"r\")\nlocal_path_override(module_name = \"c\", path = \"third_party/c\")\n",
        );
        assert_eq!(paths, vec!["../a".to_owned(), "third_party/c".to_owned()]);
        assert!(local_path_overrides("not starlark (").is_empty());
    }

    #[test]
    fn module_dependency_ignores_noise_and_unknowns() {
        let noisy = dep(
            "# leading comment\nmodule( name = 'demo' ) # trailing\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nbazel_dep(\n    name = \"rules_dx\",\n    version = \"1.2.3\", # pinned\n)\nlocal_path_override(module_name = \"other\", path = \"elsewhere\")\n",
        );
        assert_eq!(noisy.workspace_module, Some(("demo".to_owned(), None)));
        assert_eq!(noisy.declared, Some("1.2.3".to_owned()));
        assert_eq!(noisy.override_kind, None);
        assert_eq!(render_module_dependency(&noisy), "rules_dx 1.2.3");
        let quoted = dep(
            "module(name = \"demo # not a comment\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3#kept\")\n",
        );
        assert_eq!(quoted.declared, Some("1.2.3#kept".to_owned()));
        let empty = dep("");
        assert_eq!(empty.declared, None);
        assert_eq!(
            render_module_dependency(&empty),
            "unknown (no rules_dx dependency in MODULE.bazel)"
        );
        assert!(parse_module_dependency("bazel_dep(name = \"rules_dx\"", &no_includes).is_err());
        let root =
            std::env::temp_dir().join(format!("dx-adopt-moduledep-{}-missing", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("dir");
        assert!(matches!(
            read_module_dependency(&root),
            Err(AdoptError::ReadModuleFile { .. })
        ));
        std::fs::create_dir_all(root.join("MODULE.bazel")).expect("module dir");
        assert!(matches!(
            read_module_dependency(&root),
            Err(AdoptError::ReadModuleFile { .. })
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn module_dependency_reads_only_real_literal_declarations() {
        let fake = dep(
            "\"\"\"bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\"\"\"\n# bazel_dep(name = \"rules_dx\", version = \"8.8.8\")\nNOTE = 'local_path_override(module_name = \"rules_dx\", path = \"x\")'\nmodule(name = 'demo')\n",
        );
        assert_eq!(fake.declared, None);
        assert_eq!(fake.override_kind, None);
        assert_eq!(fake.workspace_module, Some(("demo".to_owned(), None)));
        let multiline = dep(
            "bazel_dep(\n    version = '''1.2.3''',  # ) ( \" '\n    repo_name = \"dx\",\n    name = 'rules_dx',\n)\ngit_override(\n    commit = \"abc123\",\n    module_name = \"rules_dx\",\n    remote = \"https://example.test/dx\",\n)\n",
        );
        assert_eq!(
            render_module_dependency(&multiline),
            "rules_dx 1.2.3 via git_override https://example.test/dx@abc123"
        );
        let computed = dep("V = \"1.2.3\"\nbazel_dep(name = \"rules_dx\", version = V)\n");
        assert_eq!(computed.declared, None);
        assert_eq!(
            render_module_dependency(&computed),
            "unknown (no rules_dx dependency in MODULE.bazel)"
        );
        let unterminated = parse_module_dependency(
            "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nbazel_dep(name = \"x\", version = \"1.0.0\"\n",
            &no_includes,
        );
        assert!(unterminated.is_err());
    }

    #[test]
    fn consumer_setup_parses_the_default_snippet_shape() {
        let setup = parse_consumer_setup(
            "# Add to MODULE.bazel.\nmodule(name = \"demo\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nbazel_dep(name = \"llvm\", version = \"0.8.19\")\nbazel_dep(name = 'rules_rust', version = '0.75.0')\n\nrust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    edition = \"2021\",\n    rustfmt_version = \"1.98.1\",\n    versions = [\"1.98.1\"],\n)\nuse_repo(\n    rust,\n    \"rust_toolchains\",\n)\n\nregister_toolchains(\"@llvm//toolchain:all\")\nregister_toolchains('@rust_toolchains//:all')\n",
        );
        assert_eq!(
            setup.deps,
            vec![
                ("rules_dx".to_owned(), "0.0.0".to_owned()),
                ("rules_cc".to_owned(), "0.2.22".to_owned()),
                ("llvm".to_owned(), "0.8.19".to_owned()),
                ("rules_rust".to_owned(), "0.75.0".to_owned()),
            ]
        );
        assert_eq!(
            setup.extensions,
            vec![(
                "@rules_rust//rust:extensions.bzl".to_owned(),
                "rust".to_owned()
            )]
        );
        assert_eq!(
            setup.repos,
            vec![("rust".to_owned(), "rust_toolchains".to_owned())]
        );
        assert_eq!(
            setup.registered,
            vec![
                "@llvm//toolchain:all".to_owned(),
                "@rust_toolchains//:all".to_owned(),
            ]
        );
        assert_eq!(
            setup.tag_arg("rust.toolchain", "edition").as_deref(),
            Some("2021")
        );
        assert_eq!(
            setup
                .tag_arg("rust.toolchain", "rustfmt_version")
                .as_deref(),
            Some("1.98.1")
        );
        assert_eq!(
            setup.tag_list_arg("rust.toolchain", "versions"),
            vec!["1.98.1".to_owned()]
        );
        assert_eq!(setup.dep_version("llvm"), Some("0.8.19"));
        assert_eq!(setup.dep_version("missing"), None);
        assert_eq!(setup.tag_arg("rust.toolchain", "missing"), None);
        assert!(setup.tag_list_arg("rust.toolchain", "missing").is_empty());
        assert!(setup.tag_list_arg("missing.tag", "versions").is_empty());
    }

    #[test]
    fn consumer_setup_records_repo_name_renames() {
        let setup = parse_consumer_setup(
            "bazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\nbazel_dep(name = 'rules_cc', version = '0.2.22')\n",
        );
        assert_eq!(
            setup.repo_names,
            vec![("rules_dx".to_owned(), "my_dx".to_owned())]
        );
        assert_eq!(setup.repo_name("rules_dx"), Some("my_dx"));
        assert_eq!(setup.repo_name("rules_cc"), None);
        assert_eq!(setup.repo_name("missing"), None);
        assert_eq!(setup.dep_version("rules_dx"), Some("0.0.0"));
    }

    #[test]
    fn consumer_setup_ignores_assignments_and_unknown_calls() {
        let setup = parse_consumer_setup(
            "rust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nsingle_version_override(module_name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_dx\")\nregister_toolchains()\n",
        );
        assert_eq!(
            setup.extensions,
            vec![(
                "@rules_rust//rust:extensions.bzl".to_owned(),
                "rust".to_owned()
            )]
        );
        assert!(setup.deps.is_empty());
        assert!(setup.registered.is_empty());
        assert!(setup.tags.is_empty());
    }
}
