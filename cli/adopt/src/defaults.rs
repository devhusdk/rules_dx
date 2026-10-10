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

pub const DX_TOML_REL: &str = "dx.toml";
pub const DX_LOCAL_TOML_REL: &str = "dx.local.toml";

pub const CONSUMER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDefaults {
    pub workspace: Option<String>,
    pub output: Option<String>,
    pub verbose: Option<bool>,
    pub color: Option<String>,
    pub quiet: Option<bool>,
    pub dry_run: Option<bool>,
    pub fail_on: Option<String>,
    pub quality_baseline: Option<String>,
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
    #[serde(default, alias = "quality-baseline")]
    quality_baseline: Option<String>,
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
    #[serde(default, alias = "quality-baseline")]
    quality_baseline: Option<String>,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    dependency_set: Option<Vec<toml::Value>>,
    #[serde(default)]
    hooks: Option<toml::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigSources {
    pub committed: Option<PathBuf>,
    pub local: Option<PathBuf>,
    pub legacy: Option<PathBuf>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

pub fn parse_file_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    let parsed: ConfigFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    check_consumer_schema(parsed.schema_version)?;
    let table = parsed.dx.unwrap_or_default();
    Ok(FileDefaults {
        workspace: non_empty(table.workspace.or(parsed.workspace)),
        output: non_empty(table.output.or(parsed.output)),
        verbose: table.verbose.or(parsed.verbose),
        color: non_empty(table.color.or(parsed.color)),
        quiet: table.quiet.or(parsed.quiet),
        dry_run: table.dry_run.or(parsed.dry_run),
        fail_on: non_empty(table.fail_on.or(parsed.fail_on)),
        quality_baseline: non_empty(table.quality_baseline.or(parsed.quality_baseline)),
    })
}

fn check_consumer_schema(version: Option<u32>) -> Result<(), super::AdoptError> {
    match version {
        None => Ok(()),
        Some(CONSUMER_SCHEMA_VERSION) => Ok(()),
        Some(other) => Err(super::AdoptError::InvalidDefaults {
            detail: format!("unsupported schema_version {other} (want {CONSUMER_SCHEMA_VERSION})"),
        }),
    }
}

pub fn shares_file_with_other_tables(text: &str) -> bool {
    let parsed: ConfigFile = toml::from_str(text).unwrap_or_default();
    parsed.dependency_set.is_some() || parsed.hooks.is_some()
}

pub fn merge_defaults(committed: FileDefaults, local: FileDefaults) -> FileDefaults {
    FileDefaults {
        workspace: local.workspace.or(committed.workspace),
        output: local.output.or(committed.output),
        verbose: local.verbose.or(committed.verbose),
        color: local.color.or(committed.color),
        quiet: local.quiet.or(committed.quiet),
        dry_run: local.dry_run.or(committed.dry_run),
        fail_on: local.fail_on.or(committed.fail_on),
        quality_baseline: local.quality_baseline.or(committed.quality_baseline),
    }
}

pub fn find_new_configs(start: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut committed = None;
    let mut local = None;
    for dir in start.ancestors() {
        if committed.is_none() {
            let candidate = dir.join(DX_TOML_REL);
            if candidate.is_file() {
                committed = Some(candidate);
            }
        }
        if local.is_none() {
            let candidate = dir.join(DX_LOCAL_TOML_REL);
            if candidate.is_file() {
                local = Some(candidate);
            }
        }
        if committed.is_some() && local.is_some() {
            break;
        }
    }
    (committed, local)
}

pub fn find_config_sources(start: &Path) -> ConfigSources {
    let (committed, local) = find_new_configs(start);
    let legacy = find_config(start);
    ConfigSources {
        committed,
        local,
        legacy,
    }
}

fn read_defaults_file(path: &Path) -> Result<FileDefaults, super::AdoptError> {
    let text = std::fs::read_to_string(path).map_err(|e| super::AdoptError::InvalidDefaults {
        detail: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_file_text(&text).map_err(|e| match e {
        super::AdoptError::InvalidDefaults { detail } => super::AdoptError::InvalidDefaults {
            detail: format!("{}: {detail}", path.display()),
        },
        other => other,
    })
}

pub fn load_defaults_with_mode(
    start: &Path,
    ignore_local: bool,
) -> Result<(FileDefaults, Option<PathBuf>), super::AdoptError> {
    let sources = find_config_sources(start);
    let committed = sources.committed.clone();
    let local = if ignore_local {
        None
    } else {
        sources.local.clone()
    };
    let legacy = sources.legacy.clone();
    if (committed.is_some() || local.is_some()) && legacy.is_some() {
        return Err(super::AdoptError::InvalidDefaults {
            detail: format!(
                "legacy {} and new {} both present; move [dx] keys into dx.toml (committed) or dx.local.toml (local-only), then remove {}; no automatic edits",
                legacy.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                [committed.as_ref(), local.as_ref()]
                    .into_iter()
                    .flatten()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" and "),
                legacy.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            ),
        });
    }
    if committed.is_some() || local.is_some() {
        let mut merged = FileDefaults::default();
        if let Some(path) = &committed {
            merged = merge_defaults(merged, read_defaults_file(path)?);
        }
        if let Some(path) = &local {
            merged = merge_defaults(merged, read_defaults_file(path)?);
        }
        let primary = local.or(committed);
        return Ok((merged, primary));
    }
    let Some(path) = legacy else {
        return Ok((FileDefaults::default(), None));
    };
    Ok((read_defaults_file(&path)?, Some(path)))
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

pub fn load_defaults(start: &Path) -> Result<(FileDefaults, Option<PathBuf>), super::AdoptError> {
    load_defaults_with_mode(start, false)
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
    fn file_parses_quality_baseline_selection() {
        let parsed = parse_file_text("[dx]\nquality_baseline = \"baselines/lint.json\"\n")
            .expect("baseline parses");
        assert_eq!(
            parsed.quality_baseline,
            Some("baselines/lint.json".to_owned())
        );
        let hyphen = parse_file_text("[dx]\n\"quality-baseline\" = \"baselines/lint.json\"\n")
            .expect("hyphen alias parses");
        assert_eq!(
            hyphen.quality_baseline,
            Some("baselines/lint.json".to_owned())
        );
        let top = parse_file_text("quality_baseline = \"baselines/lint.json\"\n")
            .expect("top level parses");
        assert_eq!(
            top.quality_baseline,
            Some("baselines/lint.json".to_owned())
        );
        let empty = parse_file_text("[dx]\nquality_baseline = \"\"\n").expect("empty parses");
        assert_eq!(empty.quality_baseline, None);
        let committed = parse_file_text("[dx]\nquality_baseline = \"a.json\"\n").expect("a");
        let local = parse_file_text("[dx]\nquality_baseline = \"b.json\"\n").expect("b");
        assert_eq!(
            merge_defaults(committed, local).quality_baseline,
            Some("b.json".to_owned())
        );
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
        let (defaults, path) = load_defaults(&sub).expect("loads nearest");
        assert_eq!(defaults.output, Some("diff".to_owned()));
        assert_eq!(path, Some(sub.join(".dx/config.toml")));
        std::fs::remove_file(sub.join(".dx/config.toml")).expect("remove sub");
        let (defaults, _) = load_defaults(&sub).expect("falls back upward");
        assert_eq!(defaults.output, Some("json".to_owned()));
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
        let (defaults, path) = load_defaults(&root.join("mine")).expect("loads the ancestor");
        assert_eq!(defaults.output, Some("json".to_owned()));
        assert_eq!(path, Some(root.join(".dx/config.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn load_missing_is_empty_and_invalid_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-defaults-missing-");
        let root = scratch.path().to_path_buf();
        let (defaults, path) = load_defaults(&root).expect("missing is empty");
        assert_eq!(defaults, FileDefaults::default());
        assert_eq!(path, None);
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
    fn committed_and_local_merge_with_local_winning() {
        let committed =
            parse_file_text("[dx]\noutput = \"json\"\nquiet = true\n").expect("committed");
        let local = parse_file_text("[dx]\noutput = \"text\"\n").expect("local");
        let merged = merge_defaults(committed, local);
        assert_eq!(merged.output, Some("text".to_owned()));
        assert_eq!(merged.quiet, Some(true));
        let empty = FileDefaults::default();
        let merged = merge_defaults(empty.clone(), empty.clone());
        assert_eq!(merged, FileDefaults::default());
    }

    #[test]
    fn new_files_ignore_sets_and_hooks_tables() {
        let text = "schema_version = 1\n[dx]\noutput = \"json\"\n[hooks]\npre_commit = []\n";
        let parsed = parse_file_text(text).expect("hooks ignored");
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert!(shares_file_with_other_tables(text));
        assert!(!shares_file_with_other_tables("[dx]\nquiet = true\n"));
        let text = "schema_version = 1\n[dx]\nquiet = true\n[[dependency_set]]\nname = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n";
        let parsed = parse_file_text(text).expect("sets ignored");
        assert_eq!(parsed.quiet, Some(true));
        assert!(shares_file_with_other_tables(text));
    }

    #[test]
    fn new_files_reject_unknown_keys_and_bad_schema() {
        assert!(parse_file_text("[dx]\nqiet = true\n").is_err());
        assert!(parse_file_text("[extra]\nquiet = true\n").is_err());
        assert!(parse_file_text("schema_version = 2\n[dx]\nquiet = true\n").is_err());
        let ok = parse_file_text("schema_version = 1\n[dx]\nquiet = true\n").expect("versioned");
        assert_eq!(ok.quiet, Some(true));
    }

    #[test]
    fn local_beats_committed_below_nothing_else() {
        let scratch = dx_test_scratch::scratch("dx-defaults-new-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("dx.toml"),
            "schema_version = 1\n[dx]\noutput = \"json\"\nquiet = true\n",
        )
        .expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"text\"\n").expect("local");
        let sources = find_config_sources(&root);
        assert_eq!(sources.committed, Some(root.join("dx.toml")));
        assert_eq!(sources.local, Some(root.join("dx.local.toml")));
        let (defaults, primary) = load_defaults(&root).expect("merges");
        assert_eq!(defaults.output, Some("text".to_owned()));
        assert_eq!(defaults.quiet, Some(true));
        assert_eq!(primary, Some(root.join("dx.local.toml")));
        let (defaults, _) = load_defaults_with_mode(&root, true).expect("ci skips local");
        assert_eq!(defaults.output, Some("json".to_owned()));
        assert_eq!(defaults.quiet, Some(true));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn committed_without_local_reports_committed_origin() {
        let scratch = dx_test_scratch::scratch("dx-defaults-committed-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        let (defaults, primary) = load_defaults(&root).expect("loads");
        assert_eq!(defaults.output, Some("json".to_owned()));
        assert_eq!(primary, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn legacy_and_new_together_fail_with_both_paths() {
        let scratch = dx_test_scratch::scratch("dx-defaults-conflict-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n").expect("legacy");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("new");
        let error = load_defaults(&root).expect_err("conflict fails");
        let rendered = error.to_string();
        assert!(rendered.contains(".dx/config.toml"), "{rendered}");
        assert!(rendered.contains("dx.toml"), "{rendered}");
        assert!(rendered.contains("dx.local.toml"), "{rendered}");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn deleting_dx_keeps_committed_behavior() {
        let scratch = dx_test_scratch::scratch("dx-defaults-nodelete-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        let (before, _) = load_defaults(&root).expect("before");
        assert_eq!(before.output, Some("json".to_owned()));
        std::fs::remove_dir_all(root.join(".dx")).expect("remove dx");
        let (after, primary) = load_defaults(&root).expect("after");
        assert_eq!(after, before);
        assert_eq!(primary, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }
}
