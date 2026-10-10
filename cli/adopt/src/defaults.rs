use std::path::{Path, PathBuf};

pub const DX_WORKSPACE_ENV: &str = "DX_WORKSPACE";
pub const DX_OUTPUT_ENV: &str = "DX_OUTPUT";
pub const DX_VERBOSE_ENV: &str = "DX_VERBOSE";
pub const DX_COLOR_ENV: &str = "DX_COLOR";
pub const DX_QUIET_ENV: &str = "DX_QUIET";
pub const DX_DRY_RUN_ENV: &str = "DX_DRY_RUN";
pub const DX_FAIL_ON_ENV: &str = "DX_FAIL_ON";

/// Environment defaults, the flag each one defaults, and the value shape.
pub const ENV_DEFAULTS: [(&str, &str, &str); 7] = [
    (DX_WORKSPACE_ENV, "--workspace", "dir"),
    (DX_DRY_RUN_ENV, "--dry-run", "bool"),
    (DX_QUIET_ENV, "--quiet", "bool"),
    (DX_VERBOSE_ENV, "--verbose", "bool"),
    (DX_COLOR_ENV, "--color", "mode"),
    (DX_OUTPUT_ENV, "--output", "mode"),
    (DX_FAIL_ON_ENV, "--fail-on", "level"),
];

/// Spellings that turn a boolean environment default on.
pub const TRUTHY: [&str; 5] = ["1", "true", "yes", "y", "on"];

/// Spellings that turn a boolean environment default off.
pub const FALSEY: [&str; 5] = ["0", "false", "no", "n", "off"];

/// Every documented boolean spelling, on spellings first.
pub const BOOL_SPELLINGS: &str = "1|true|yes|y|on|0|false|no|n|off";

/// The `.dx/config.toml` key that carries the same default as an env var.
pub fn config_key(env: &str) -> Option<&'static str> {
    ENV_DEFAULTS
        .iter()
        .find(|(name, _, _)| *name == env)
        .map(|(_, flag, _)| flag.trim_start_matches("--"))
}

pub const CONFIG_TOML_REL: &str = ".dx/config.toml";
pub const CONFIG_REL: &str = ".dx/config";

pub const COMMITTED_CONFIG_REL: &str = "dx.toml";
pub const LOCAL_CONFIG_REL: &str = "dx.local.toml";

pub const DX_ALLOW_LOCAL_CONFIG_ENV: &str = "DX_ALLOW_LOCAL_CONFIG";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDefaults {
    pub workspace: Option<String>,
    pub output: Option<String>,
    pub verbose: Option<bool>,
    pub color: Option<String>,
    pub quiet: Option<bool>,
    pub dry_run: Option<bool>,
    pub fail_on: Option<String>,
}

pub fn is_truthy(value: &str) -> bool {
    parse_bool(value) == Some(true)
}

/// Read one boolean default, or `None` when the text is not a documented spelling.
pub fn parse_bool(value: &str) -> Option<bool> {
    let value = value.trim().to_ascii_lowercase();
    if TRUTHY.contains(&value.as_str()) {
        Some(true)
    } else if FALSEY.contains(&value.as_str()) {
        Some(false)
    } else {
        None
    }
}

pub fn env_string(get: &dyn Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    get(name).filter(|value| !value.is_empty())
}

/// Read one boolean environment default, refusing a value that is neither spelling.
pub fn env_bool(
    get: &dyn Fn(&str) -> Option<String>,
    name: &'static str,
) -> Result<Option<bool>, super::AdoptError> {
    let Some(value) = env_string(get, name) else {
        return Ok(None);
    };
    parse_bool(&value)
        .map(Some)
        .ok_or(super::AdoptError::InvalidEnvBool {
            name,
            value,
            spellings: BOOL_SPELLINGS,
        })
}

pub fn resolve_string(
    flag: Option<String>,
    env: Option<String>,
    file: Option<String>,
    fallback: &str,
) -> String {
    flag.or(env).or(file).unwrap_or_else(|| fallback.to_owned())
}

pub fn resolve_workspace(
    flag: Option<String>,
    env: Option<String>,
    file: Option<String>,
) -> Option<String> {
    flag.or(env).or(file)
}

/// Resolve a boolean default across flag, environment, file, then off.
pub fn resolve_bool(flag: Option<bool>, env: Option<bool>, file: Option<bool>) -> bool {
    flag.or(env).or(file).unwrap_or(false)
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DxTable {
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    quiet: Option<bool>,
    #[serde(default, alias = "dry-run")]
    dry_run: Option<bool>,
    #[serde(default, alias = "fail-on")]
    fail_on: Option<String>,
}

/// A committed or local configuration file: only the `[dx]` table carries
/// invocation defaults. Every other table belongs to the family that owns
/// it, so the loader tolerates what it does not read.
#[derive(Debug, Default, serde::Deserialize)]
struct NewConfigFile {
    #[serde(default)]
    dx: Option<DxTable>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    dx: Option<DxTable>,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    verbose: Option<bool>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    quiet: Option<bool>,
    #[serde(default, alias = "dry-run")]
    dry_run: Option<bool>,
    #[serde(default, alias = "fail-on")]
    fail_on: Option<String>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn table_defaults(table: DxTable) -> FileDefaults {
    FileDefaults {
        workspace: non_empty(table.workspace),
        output: non_empty(table.output),
        verbose: table.verbose,
        color: non_empty(table.color),
        quiet: table.quiet,
        dry_run: table.dry_run,
        fail_on: non_empty(table.fail_on),
    }
}

pub fn parse_file_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    let parsed: ConfigFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    let table = parsed.dx.unwrap_or_default();
    Ok(FileDefaults {
        workspace: non_empty(table.workspace.or(parsed.workspace)),
        output: non_empty(table.output.or(parsed.output)),
        verbose: table.verbose.or(parsed.verbose),
        color: non_empty(table.color.or(parsed.color)),
        quiet: table.quiet.or(parsed.quiet),
        dry_run: table.dry_run.or(parsed.dry_run),
        fail_on: non_empty(table.fail_on.or(parsed.fail_on)),
    })
}

/// Read the `[dx]` table out of a committed or local configuration file.
/// Top-level keys stay owned by their families, so only `[dx]` applies here
/// while a misspelled `[dx]` key still fails.
pub fn parse_new_file_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    let parsed: NewConfigFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    Ok(table_defaults(parsed.dx.unwrap_or_default()))
}

/// Overlay local defaults over committed ones, one scalar at a time.
pub fn merge_committed_local(committed: FileDefaults, local: FileDefaults) -> FileDefaults {
    FileDefaults {
        workspace: local.workspace.or(committed.workspace),
        output: local.output.or(committed.output),
        verbose: local.verbose.or(committed.verbose),
        color: local.color.or(committed.color),
        quiet: local.quiet.or(committed.quiet),
        dry_run: local.dry_run.or(committed.dry_run),
        fail_on: local.fail_on.or(committed.fail_on),
    }
}

/// The nearest config file at or above `start`, `.dx/config.toml` before `.dx/config`.
///
/// The search reads `start` and then every ancestor directory up to the filesystem
/// root. It never reads a descendant, a sibling, or an unrelated workspace tree.
pub fn find_config(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let toml = dir.join(CONFIG_TOML_REL);
        if toml.is_file() {
            return Some(toml);
        }
        let plain = dir.join(CONFIG_REL);
        if plain.is_file() {
            return Some(plain);
        }
    }
    None
}

pub fn load_defaults(start: &Path) -> Result<LoadedDefaults, super::AdoptError> {
    let allow_local = local_config_applies(&|name| std::env::var(name).ok())?;
    load_defaults_with(start, allow_local)
}

/// Whether local preferences apply: the local file plus the DX_* preference
/// environment. Outside CI they always apply; under CI they apply only when
/// the explicit opt-in is a documented on spelling.
pub fn local_config_applies(
    get: &dyn Fn(&str) -> Option<String>,
) -> Result<bool, super::AdoptError> {
    if !dx_process::is_ci_value(env_string(get, "CI").as_deref()) {
        return Ok(true);
    }
    Ok(env_bool(get, DX_ALLOW_LOCAL_CONFIG_ENV)?.unwrap_or(false))
}

/// The configuration files one defaults load read, and where they came from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedDefaults {
    pub defaults: FileDefaults,
    pub committed: Option<PathBuf>,
    pub local: Option<PathBuf>,
    pub legacy: Option<PathBuf>,
    pub local_ignored: bool,
}

/// The nearest directory at or above `start` holding a committed or local
/// configuration file, with whichever of the two it holds.
pub fn find_config_files(start: &Path) -> Option<(PathBuf, Option<PathBuf>, Option<PathBuf>)> {
    for dir in start.ancestors() {
        let committed = dir.join(COMMITTED_CONFIG_REL);
        let local = dir.join(LOCAL_CONFIG_REL);
        let has_committed = committed.is_file();
        let has_local = local.is_file();
        if has_committed || has_local {
            return Some((
                dir.to_path_buf(),
                has_committed.then(|| committed.clone()),
                has_local.then(|| local.clone()),
            ));
        }
    }
    None
}

fn read_new_file(path: &Path) -> Result<FileDefaults, super::AdoptError> {
    let text = std::fs::read_to_string(path).map_err(|e| super::AdoptError::InvalidDefaults {
        detail: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_new_file_text(&text).map_err(|e| match e {
        super::AdoptError::InvalidDefaults { detail } => super::AdoptError::InvalidDefaults {
            detail: format!("{}: {detail}", path.display()),
        },
        other => other,
    })
}

/// Load invocation defaults with an explicit local-preference gate. The
/// committed file and, unless gated off, the local file merge by scalar;
/// legacy `.dx` files only apply when no new file exists, and a legacy file
/// at or below the new configuration errors with its migration.
pub fn load_defaults_with(
    start: &Path,
    allow_local: bool,
) -> Result<LoadedDefaults, super::AdoptError> {
    let legacy = find_config(start);
    let found = find_config_files(start);
    let Some((dir, committed, local)) = found else {
        let (defaults, legacy) = match legacy {
            Some(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| {
                    super::AdoptError::InvalidDefaults {
                        detail: format!("cannot read {}: {e}", path.display()),
                    }
                })?;
                let defaults = parse_file_text(&text).map_err(|e| match e {
                    super::AdoptError::InvalidDefaults { detail } => {
                        super::AdoptError::InvalidDefaults {
                            detail: format!("{}: {detail}", path.display()),
                        }
                    }
                    other => other,
                })?;
                (defaults, Some(path))
            }
            None => (FileDefaults::default(), None),
        };
        return Ok(LoadedDefaults {
            defaults,
            committed: None,
            local: None,
            legacy,
            local_ignored: false,
        });
    };
    if let Some(legacy_path) = &legacy {
        let below = legacy_path
            .parent()
            .and_then(|parent| parent.parent())
            .is_some_and(|legacy_dir| legacy_dir.starts_with(&dir));
        if below {
            return Err(super::AdoptError::ConfigConflict {
                legacy: legacy_path.display().to_string(),
                dir: dir.display().to_string(),
            });
        }
    }
    let committed_defaults = match committed.clone() {
        Some(path) => read_new_file(&path)?,
        None => FileDefaults::default(),
    };
    let (local_defaults, local_used, local_ignored) = match local.clone() {
        Some(path) if allow_local => (read_new_file(&path)?, Some(path), false),
        Some(_) => (FileDefaults::default(), None, true),
        None => (FileDefaults::default(), None, false),
    };
    Ok(LoadedDefaults {
        defaults: merge_committed_local(committed_defaults, local_defaults),
        committed,
        local: local_used,
        legacy: None,
        local_ignored,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn truthy_spellings_enable_only_documented_values() {
        for truthy in [
            "1", "true", "TRUE", " True ", "yes", "YES", "y", "Y", "on", "ON",
        ] {
            assert!(is_truthy(truthy), "{truthy:?} must enable");
        }
        for falsy in ["", "0", "false", "no", "off", "tru", "2", "maybe"] {
            assert!(!is_truthy(falsy), "{falsy:?} must not enable");
        }
    }

    #[test]
    fn falsy_spellings_disable_and_anything_else_is_unreadable() {
        for falsy in [
            "0", "false", "FALSE", " False ", "no", "NO", "n", "N", "off", "OFF",
        ] {
            assert_eq!(parse_bool(falsy), Some(false), "{falsy:?} must disable");
            assert!(!is_truthy(falsy), "{falsy:?} must not enable");
        }
        for on in TRUTHY {
            assert_eq!(parse_bool(on), Some(true), "{on:?} must enable");
        }
        for unreadable in [
            "", " ", "tru", "ture", "fals", "2", "-1", "maybe", "enabled",
        ] {
            assert_eq!(parse_bool(unreadable), None, "{unreadable:?}");
        }
        assert_eq!(BOOL_SPELLINGS, "1|true|yes|y|on|0|false|no|n|off");
    }

    #[test]
    fn env_bool_refuses_a_value_outside_the_documented_spellings() {
        let get = env_of(&[(DX_DRY_RUN_ENV, "tru")]);
        let error = env_bool(&get, DX_DRY_RUN_ENV).expect_err("typo is not a boolean");
        assert_eq!(
            error,
            super::super::AdoptError::InvalidEnvBool {
                name: DX_DRY_RUN_ENV,
                value: "tru".to_owned(),
                spellings: BOOL_SPELLINGS,
            }
        );
        assert_eq!(
            error.to_string(),
            "invalid invocation default DX_DRY_RUN=\"tru\": want one of 1|true|yes|y|on|0|false|no|n|off"
        );
        for (name, value, wanted) in [
            (DX_QUIET_ENV, "off", false),
            (DX_VERBOSE_ENV, "NO", false),
            (DX_DRY_RUN_ENV, "0", false),
        ] {
            let pairs = [(name, value)];
            let get = env_of(&pairs);
            assert_eq!(
                env_bool(&get, name).expect("documented spelling"),
                Some(wanted)
            );
        }
        let get = env_of(&[(DX_DRY_RUN_ENV, ""), (DX_QUIET_ENV, "1")]);
        assert_eq!(env_bool(&get, DX_DRY_RUN_ENV), Ok(None));
        assert_eq!(env_bool(&get, DX_QUIET_ENV), Ok(Some(true)));
        assert_eq!(env_bool(&env_of(&[]), DX_DRY_RUN_ENV), Ok(None));
    }

    #[test]
    fn env_defaults_cover_every_documented_variable() {
        assert_eq!(
            ENV_DEFAULTS.map(|(env, _, _)| env),
            [
                DX_WORKSPACE_ENV,
                DX_DRY_RUN_ENV,
                DX_QUIET_ENV,
                DX_VERBOSE_ENV,
                DX_COLOR_ENV,
                DX_OUTPUT_ENV,
                DX_FAIL_ON_ENV
            ]
        );
        assert_eq!(TRUTHY, ["1", "true", "yes", "y", "on"]);
        assert_eq!(FALSEY, ["0", "false", "no", "n", "off"]);
        for (env, flag, _) in ENV_DEFAULTS {
            assert_eq!(
                config_key(env),
                Some(flag.trim_start_matches("--")),
                "{env}"
            );
        }
        assert_eq!(config_key("DX_NOPE"), None);
    }

    #[test]
    fn env_helpers_ignore_missing_and_empty_strings() {
        let get = env_of(&[(DX_WORKSPACE_ENV, "/repo"), (DX_OUTPUT_ENV, "")]);
        assert_eq!(env_string(&get, DX_WORKSPACE_ENV), Some("/repo".to_owned()));
        assert_eq!(env_string(&get, DX_OUTPUT_ENV), None);
        assert_eq!(env_string(&get, DX_VERBOSE_ENV), None);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(None));
        let get = env_of(&[(DX_VERBOSE_ENV, "yes")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(Some(true)));
        let get = env_of(&[(DX_VERBOSE_ENV, "0")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(Some(false)));
        assert_eq!(env_bool(&env_of(&[]), DX_QUIET_ENV), Ok(None));
    }

    #[test]
    fn precedence_resolves_flag_over_env_over_file() {
        assert_eq!(
            resolve_string(
                Some("flag".to_owned()),
                Some("env".to_owned()),
                Some("file".to_owned()),
                "fallback"
            ),
            "flag"
        );
        assert_eq!(
            resolve_string(
                None,
                Some("env".to_owned()),
                Some("file".to_owned()),
                "fallback"
            ),
            "env"
        );
        assert_eq!(
            resolve_string(None, None, Some("file".to_owned()), "fallback"),
            "file"
        );
        assert_eq!(resolve_string(None, None, None, "fallback"), "fallback");
        assert_eq!(
            resolve_workspace(
                Some("flag".to_owned()),
                Some("env".to_owned()),
                Some("file".to_owned())
            ),
            Some("flag".to_owned())
        );
        assert_eq!(resolve_workspace(None, None, None), None);
        assert!(resolve_bool(Some(true), Some(false), Some(false)));
        assert!(resolve_bool(None, Some(true), Some(false)));
        assert!(!resolve_bool(None, Some(false), Some(true)));
        assert!(resolve_bool(None, None, Some(true)));
        assert!(!resolve_bool(None, None, None));
        assert!(!resolve_bool(Some(false), Some(true), Some(true)));
        assert!(!resolve_bool(None, Some(false), Some(true)));
        assert!(!resolve_bool(None, None, Some(false)));
    }

    #[test]
    fn file_parses_dx_table_with_top_level_alias() {
        let parsed = parse_file_text(
            "[dx]\nworkspace = \"/repo\"\noutput = \"json\"\nverbose = true\ncolor = \"never\"\nquiet = false\ndry_run = true\nfail_on = \"error\"\n",
        )
        .expect("dx table parses");
        assert_eq!(parsed.workspace, Some("/repo".to_owned()));
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert_eq!(parsed.verbose, Some(true));
        assert_eq!(parsed.color, Some("never".to_owned()));
        assert_eq!(parsed.quiet, Some(false));
        assert_eq!(parsed.dry_run, Some(true));
        assert_eq!(parsed.fail_on, Some("error".to_owned()));
        let top = parse_file_text("workspace = \"/top\"\nverbose = true\n").expect("top parses");
        assert_eq!(top.workspace, Some("/top".to_owned()));
        assert_eq!(top.verbose, Some(true));
        let both =
            parse_file_text("output = \"text\"\n[dx]\noutput = \"json\"\n").expect("table wins");
        assert_eq!(both.output, Some("json".to_owned()));
        let both_color =
            parse_file_text("color = \"never\"\n[dx]\ncolor = \"always\"\n").expect("table wins");
        assert_eq!(both_color.color, Some("always".to_owned()));
        let empty = parse_file_text("").expect("empty parses");
        assert_eq!(empty, FileDefaults::default());
        assert!(parse_file_text("not toml = [").is_err());
        assert!(parse_file_text("[dx]\nverbose = \"yes\"\n").is_err());
    }

    #[test]
    fn file_hyphen_aliases_and_empty_strings_are_absent() {
        let parsed = parse_file_text("[dx]\n\"dry-run\" = true\n\"fail-on\" = \"info\"\n")
            .expect("hyphen aliases parse");
        assert_eq!(parsed.dry_run, Some(true));
        assert_eq!(parsed.fail_on, Some("info".to_owned()));
        let parsed = parse_file_text("workspace = \"\"\noutput = \"\"\ncolor = \"\"\n")
            .expect("empty parses");
        assert_eq!(parsed.workspace, None);
        assert_eq!(parsed.output, None);
        assert_eq!(parsed.color, None);
    }

    #[test]
    fn file_rejects_a_misspelled_key_and_keeps_the_documented_spellings() {
        for text in [
            "[dx]\nqiet = true\n",
            "[dx]\n\"dry-ruun\" = true\n",
            "[dx]\nverbse = false\n",
            "[dx]\noutpt = \"json\"\n",
            "workspce = \"/repo\"\n",
            "[dx]\n[extra]\nquiet = true\n",
            "[extra]\nquiet = true\n",
        ] {
            let error = parse_file_text(text).expect_err(text);
            let rendered = error.to_string();
            assert!(
                rendered.contains("unknown field"),
                "{text:?} must name the unknown field: {rendered}"
            );
        }
        let error = parse_file_text("[dx]\nqiet = true\n").expect_err("typo");
        assert!(
            error.to_string().contains("qiet"),
            "the diagnostic names the misspelled key: {error}"
        );
        for (env, flag, _) in ENV_DEFAULTS {
            let key = config_key(env).expect("every default has a key");
            let literal = match flag {
                "--dry-run" | "--quiet" | "--verbose" => "true",
                "--fail-on" => "\"error\"",
                "--color" => "\"never\"",
                "--output" => "\"json\"",
                _ => "\"/repo\"",
            };
            let parsed = parse_file_text(&format!("[dx]\n{key} = {literal}\n"))
                .unwrap_or_else(|error| panic!("{key} must stay a real key: {error}"));
            let underscored = key.replace('-', "_");
            if underscored != key {
                parse_file_text(&format!("[dx]\n{underscored} = {literal}\n"))
                    .unwrap_or_else(|error| panic!("{underscored} must stay an alias: {error}"));
            }
            let top = parse_file_text(&format!("{key} = {literal}\n"))
                .unwrap_or_else(|error| panic!("{key} at the top level: {error}"));
            let both = parse_file_text(&format!("{key} = {literal}\n[dx]\n{key} = {literal}\n"))
                .expect("both layers parse");
            assert_eq!(parsed, both, "{key} in both layers");
            assert_eq!(parsed, top, "{key} at the top level");
        }
    }

    #[test]
    fn file_boolean_keys_keep_the_documented_precedence_order() {
        let both = parse_file_text("quiet = false\n[dx]\nquiet = false\n").expect("false parses");
        assert_eq!(
            both.quiet,
            Some(false),
            "an explicit false is not an absent value"
        );
        let top_only = parse_file_text("dry_run = false\n").expect("top false");
        assert_eq!(top_only.dry_run, Some(false));
        let top_true = parse_file_text("verbose = true\n").expect("top true");
        assert_eq!(top_true.verbose, Some(true));
        assert!(
            parse_file_text("quiet = false\n[dx]\nquiet = true\n")
                .expect("both")
                .quiet
                == Some(true),
            "[dx] wins over the top level"
        );
    }

    #[test]
    fn find_and_load_prefers_toml_and_walks_up() {
        let scratch = dx_test_scratch::scratch("dx-defaults-");
        let root = scratch.path().to_path_buf();
        let sub = root.join("sub/dir");
        std::fs::create_dir_all(sub.join(".dx")).expect("dirs");
        std::fs::create_dir_all(root.join(".dx")).expect("root dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("root config");
        std::fs::write(sub.join(".dx/config.toml"), "[dx]\noutput = \"diff\"\n")
            .expect("sub config");
        assert_eq!(
            find_config(&sub),
            Some(sub.join(".dx/config.toml")),
            "nearest file wins"
        );
        let loaded = load_defaults(&sub).expect("loads nearest");
        assert_eq!(loaded.defaults.output, Some("diff".to_owned()));
        assert_eq!(loaded.legacy, Some(sub.join(".dx/config.toml")));
        assert_eq!(loaded.committed, None);
        assert_eq!(loaded.local, None);
        std::fs::remove_file(sub.join(".dx/config.toml")).expect("remove sub");
        let loaded = load_defaults(&sub).expect("falls back upward");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn discovery_reads_parents_and_never_another_workspace_tree() {
        let scratch = dx_test_scratch::scratch("dx-defaults-boundary-");
        let root = scratch.path().to_path_buf();
        let neighbor = root.join("neighbor");
        let mine = root.join("mine/deep");
        let below = root.join("mine/deep/deeper");
        std::fs::create_dir_all(root.join(".dx")).expect("root dx");
        std::fs::create_dir_all(neighbor.join(".dx")).expect("neighbor dx");
        std::fs::create_dir_all(&mine).expect("mine");
        std::fs::create_dir_all(below.join(".dx")).expect("deeper dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("root config");
        std::fs::write(
            neighbor.join(".dx/config.toml"),
            "[dx]\noutput = \"diff\"\n",
        )
        .expect("neighbor config");
        std::fs::write(below.join(".dx/config.toml"), "[dx]\noutput = \"text\"\n")
            .expect("deeper config");
        assert_eq!(
            find_config(&below),
            Some(below.join(".dx/config.toml")),
            "the nearest ancestor supplies the defaults"
        );
        assert_eq!(
            find_config(&mine),
            Some(root.join(".dx/config.toml")),
            "a descendant never supplies the defaults"
        );
        let found = find_config(&root.join("mine")).expect("parent discovery is intentional");
        assert_eq!(found, root.join(".dx/config.toml"));
        assert!(
            !found.starts_with(&neighbor),
            "a neighboring workspace never supplies the defaults: {found:?}"
        );
        let loaded = load_defaults(&root.join("mine")).expect("loads the ancestor");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        assert_eq!(loaded.legacy, Some(root.join(".dx/config.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn load_missing_is_empty_and_invalid_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-defaults-missing-");
        let root = scratch.path().to_path_buf();
        let loaded = load_defaults(&root).expect("missing is empty");
        assert_eq!(loaded.defaults, FileDefaults::default());
        assert_eq!(loaded.legacy, None);
        assert_eq!(loaded.committed, None);
        assert_eq!(loaded.local, None);
        assert!(!loaded.local_ignored);
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "not toml = [").expect("bad config");
        assert!(load_defaults(&root).is_err());
        std::fs::write(root.join(".dx/config.toml"), "[dx]\nqiet = true\n").expect("typo");
        let error = load_defaults(&root).expect_err("a misspelled key is not ignored");
        let rendered = error.to_string();
        assert!(
            rendered.contains("unknown field") && rendered.contains("qiet"),
            "the diagnostic names the file and the key: {rendered}"
        );
        assert!(
            rendered.contains(&root.join(".dx/config.toml").display().to_string()),
            "the diagnostic names the file: {rendered}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn new_files_parse_dx_table_and_ignore_other_families() {
        let parsed =
            parse_new_file_text("schema_version = 1\n[dx]\noutput = \"json\"\nquiet = true\n")
                .expect("committed parses");
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert_eq!(parsed.quiet, Some(true));
        assert_eq!(parsed.workspace, None);
        let overlay = parse_new_file_text("[hooks]\nbudget_secs = 3\n[dx]\noutput = \"text\"\n")
            .expect("overlay parses");
        assert_eq!(overlay.output, Some("text".to_owned()));
        let empty = parse_new_file_text("schema_version = 1\n").expect("no dx table");
        assert_eq!(empty, FileDefaults::default());
        assert!(parse_new_file_text("not toml = [").is_err());
        let error = parse_new_file_text("[dx]\nqiet = true\n").expect_err("a misspelled key fails");
        assert!(
            error.to_string().contains("unknown field") && error.to_string().contains("qiet"),
            "the diagnostic names the key: {error}"
        );
    }

    #[test]
    fn committed_and_local_merge_by_scalar_with_local_winning() {
        let committed = parse_new_file_text("[dx]\noutput = \"json\"\nquiet = true\n").expect("c");
        let local = parse_new_file_text("[dx]\nquiet = false\ncolor = \"never\"\n").expect("l");
        let merged = merge_committed_local(committed, local);
        assert_eq!(merged.output, Some("json".to_owned()));
        assert_eq!(merged.quiet, Some(false));
        assert_eq!(merged.color, Some("never".to_owned()));
        assert_eq!(merged.verbose, None);
    }

    #[test]
    fn new_files_load_from_the_nearest_directory_holding_either() {
        let scratch = dx_test_scratch::scratch("dx-defaults-new-");
        let root = scratch.path().to_path_buf();
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).expect("sub");
        std::fs::write(
            root.join("dx.toml"),
            "schema_version = 1\n[dx]\noutput = \"json\"\n",
        )
        .expect("committed");
        std::fs::write(sub.join("dx.local.toml"), "[dx]\noutput = \"text\"\n").expect("local");
        let loaded = load_defaults(&sub).expect("nearest directory wins");
        assert_eq!(loaded.defaults.output, Some("text".to_owned()));
        assert_eq!(loaded.committed, None);
        assert_eq!(loaded.local, Some(sub.join("dx.local.toml")));
        assert_eq!(loaded.legacy, None);
        std::fs::remove_file(sub.join("dx.local.toml")).expect("remove local");
        let loaded = load_defaults(&sub).expect("falls back upward");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        assert_eq!(loaded.committed, Some(root.join("dx.toml")));
        assert_eq!(loaded.local, None);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn local_overrides_committed_key_by_key() {
        let scratch = dx_test_scratch::scratch("dx-defaults-merge-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("dx.toml"),
            "[dx]\noutput = \"json\"\nquiet = true\n",
        )
        .expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\nquiet = false\n").expect("local");
        let loaded = load_defaults(&root).expect("merges");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        assert_eq!(loaded.defaults.quiet, Some(false));
        assert_eq!(loaded.committed, Some(root.join("dx.toml")));
        assert_eq!(loaded.local, Some(root.join("dx.local.toml")));
        assert!(!loaded.local_ignored);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn disallowed_local_is_ignored_but_recorded() {
        let scratch = dx_test_scratch::scratch("dx-defaults-gated-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"text\"\n").expect("local");
        let loaded = load_defaults_with(&root, false).expect("loads");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        assert_eq!(loaded.local, None);
        assert!(loaded.local_ignored);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn legacy_and_new_files_conflict_with_migration() {
        let scratch = dx_test_scratch::scratch("dx-defaults-conflict-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n").expect("legacy");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("committed");
        let error = load_defaults(&root).expect_err("overlap conflicts");
        assert_eq!(
            error,
            super::super::AdoptError::ConfigConflict {
                legacy: root.join(".dx/config.toml").display().to_string(),
                dir: root.display().to_string(),
            }
        );
        let rendered = error.to_string();
        assert!(rendered.contains("dx.local.toml"), "{rendered}");
        assert!(rendered.contains("never rewrites"), "{rendered}");
        std::fs::remove_file(root.join("dx.toml")).expect("remove committed");
        let loaded = load_defaults(&root).expect("legacy alone still reads");
        assert_eq!(loaded.defaults.output, Some("json".to_owned()));
        assert_eq!(loaded.legacy, Some(root.join(".dx/config.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn nested_legacy_below_new_configuration_conflicts() {
        let scratch = dx_test_scratch::scratch("dx-defaults-nested-");
        let root = scratch.path().to_path_buf();
        let sub = root.join("sub");
        std::fs::create_dir_all(sub.join(".dx")).expect("sub dx");
        std::fs::write(sub.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n").expect("legacy");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("committed");
        let error = load_defaults(&sub).expect_err("nested overlap conflicts");
        assert!(
            error
                .to_string()
                .contains(&sub.join(".dx/config.toml").display().to_string()),
            "{error}"
        );
        let loaded = load_defaults(&root).expect("root never sees the nested legacy");
        assert_eq!(loaded.defaults.output, Some("text".to_owned()));
        assert_eq!(loaded.legacy, None);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn malformed_new_files_fail_naming_the_file() {
        let scratch = dx_test_scratch::scratch("dx-defaults-new-bad-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "not toml = [").expect("bad committed");
        let error = load_defaults(&root).expect_err("malformed committed fails");
        assert!(
            error
                .to_string()
                .contains(&root.join("dx.toml").display().to_string()),
            "{error}"
        );
        std::fs::remove_file(root.join("dx.toml")).expect("remove committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\nqiet = true\n").expect("typo");
        let error = load_defaults(&root).expect_err("misspelled local key fails");
        let rendered = error.to_string();
        assert!(rendered.contains("qiet"), "{rendered}");
        assert!(
            rendered.contains(&root.join("dx.local.toml").display().to_string()),
            "{rendered}"
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    fn local_applies_outside_ci_and_needs_opt_in_under_ci() {
        let outside = |_: &str| None;
        assert!(local_config_applies(&outside).expect("outside CI applies"));
        let ci = |name: &str| match name {
            "CI" => Some("true".to_owned()),
            _ => None,
        };
        assert!(!local_config_applies(&ci).expect("CI ignores local by default"));
        let opted = |name: &str| match name {
            "CI" => Some("true".to_owned()),
            "DX_ALLOW_LOCAL_CONFIG" => Some("1".to_owned()),
            _ => None,
        };
        assert!(local_config_applies(&opted).expect("opt-in applies"));
        let refused = |name: &str| match name {
            "CI" => Some("true".to_owned()),
            "DX_ALLOW_LOCAL_CONFIG" => Some("0".to_owned()),
            _ => None,
        };
        assert!(!local_config_applies(&refused).expect("explicit off ignores"));
        let bad = |name: &str| match name {
            "CI" => Some("true".to_owned()),
            "DX_ALLOW_LOCAL_CONFIG" => Some("maybe".to_owned()),
            _ => None,
        };
        let error = local_config_applies(&bad).expect_err("a typo is not a boolean");
        assert!(
            error.to_string().contains(DX_ALLOW_LOCAL_CONFIG_ENV),
            "{error}"
        );
    }
}
