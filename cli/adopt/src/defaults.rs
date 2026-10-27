use std::path::{Path, PathBuf};

pub const DX_WORKSPACE_ENV: &str = "DX_WORKSPACE";
pub const DX_OUTPUT_ENV: &str = "DX_OUTPUT";
pub const DX_VERBOSE_ENV: &str = "DX_VERBOSE";
pub const DX_COLOR_ENV: &str = "DX_COLOR";
pub const DX_QUIET_ENV: &str = "DX_QUIET";
pub const DX_FAIL_ON_ENV: &str = "DX_FAIL_ON";
pub const DX_BUILD_SLOTS_ENV: &str = "DX_BUILD_SLOTS";

/// Environment variables that mark a process started by a coding agent.
pub const AGENT_ENVS: [&str; 5] = [
    "CLAUDECODE",
    "CODEX_THREAD_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "AI_AGENT",
];

/// Whether any agent marker is set to a non-empty value.
pub fn is_agent(get: &dyn Fn(&str) -> Option<String>) -> bool {
    AGENT_ENVS
        .iter()
        .any(|name| env_string(get, name).is_some())
}

/// Environment defaults, the flag each one defaults, and the value shape.
pub const ENV_DEFAULTS: [(&str, &str, &str); 7] = [
    (DX_WORKSPACE_ENV, "--workspace", "dir"),
    (DX_QUIET_ENV, "--quiet", "bool"),
    (DX_VERBOSE_ENV, "--verbose", "bool"),
    (DX_COLOR_ENV, "--color", "mode"),
    (DX_OUTPUT_ENV, "--output", "mode"),
    (DX_FAIL_ON_ENV, "--fail-on", "level"),
    (DX_BUILD_SLOTS_ENV, "--build-slots", "auto|n|off"),
];

/// Spellings that turn a boolean environment default on.
pub const TRUTHY: [&str; 1] = ["true"];

/// Spellings that turn a boolean environment default off.
pub const FALSEY: [&str; 1] = ["false"];

/// Every documented boolean spelling, on spellings first.
pub const BOOL_SPELLINGS: &str = "true|false";

/// The `dx.toml` key that carries the same default as an env var.
pub fn config_key(env: &str) -> Option<&'static str> {
    ENV_DEFAULTS
        .iter()
        .find(|(name, _, _)| *name == env)
        .map(|(_, flag, _)| flag.trim_start_matches("--"))
}

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
    pub fail_on: Option<String>,
    pub build_slots: Option<BuildSlots>,
    pub baseline: Option<String>,
}

/// Every documented `build-slots` spelling.
pub const BUILD_SLOTS_SPELLINGS: &str = "auto|<n>|off";

/// The largest explicit slot count; each take walks every index up to it.
pub const MAX_BUILD_SLOTS: usize = 1024;

/// How many build slots this dx process sizes its gate with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BuildSlots {
    /// Size from host memory, as dx always has.
    #[default]
    Auto,
    /// Exactly this many slots, with no memory-based clamp and no remote extras.
    Count(std::num::NonZeroUsize),
    /// Take no slot and pass no `--local_resources`.
    Off,
}

impl BuildSlots {
    /// Read `auto`, `off`, or a count from 1 to `MAX_BUILD_SLOTS`; case and surrounding space are ignored.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        match value.as_str() {
            "auto" => Some(BuildSlots::Auto),
            "off" => Some(BuildSlots::Off),
            digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => digits
                .parse::<usize>()
                .ok()
                .filter(|count| *count <= MAX_BUILD_SLOTS)
                .and_then(std::num::NonZeroUsize::new)
                .map(BuildSlots::Count),
            _ => None,
        }
    }
}

impl std::fmt::Display for BuildSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildSlots::Auto => f.write_str("auto"),
            BuildSlots::Count(count) => write!(f, "{count}"),
            BuildSlots::Off => f.write_str("off"),
        }
    }
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

const REMOVED_DRY_RUN_DEFAULT: &str = "dry_run was removed; delete it from the config file";
const DX_KEYS: [&str; 7] = [
    "workspace",
    "output",
    "verbose",
    "color",
    "quiet",
    "fail-on",
    "build-slots",
];

/// Refuse the removed `DX_DRY_RUN` environment default whatever its value.
pub fn reject_removed_env(get: &dyn Fn(&str) -> Option<String>) -> Result<(), super::AdoptError> {
    if env_string(get, "DX_DRY_RUN").is_some() {
        return Err(super::AdoptError::InvalidDefaults {
            detail: "DX_DRY_RUN was removed; unset it".to_owned(),
        });
    }
    Ok(())
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
    #[serde(default, rename = "fail-on")]
    fail_on: Option<String>,
    #[serde(default, rename = "build-slots")]
    build_slots: Option<toml::Value>,
}

/// Read the `[dx] build-slots` value: an integer, or a string spelling.
fn file_build_slots(value: Option<toml::Value>) -> Result<Option<BuildSlots>, super::AdoptError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = match &value {
        toml::Value::Integer(count) => BuildSlots::parse(&count.to_string()),
        toml::Value::String(text) if text.trim().is_empty() => return Ok(None),
        toml::Value::String(text) => BuildSlots::parse(text),
        _ => None,
    };
    parsed
        .map(Some)
        .ok_or_else(|| super::AdoptError::InvalidDefaults {
            detail: format!(
                "invalid build-slots {value}: want {BUILD_SLOTS_SPELLINGS} (n from 1 to {MAX_BUILD_SLOTS})"
            ),
        })
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct QualityTable {
    #[serde(default)]
    baseline: Option<String>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    dx: Option<DxTable>,
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    quality: Option<QualityTable>,
    #[serde(default, rename = "dependency_set")]
    _dependency_set: Option<Vec<toml::Value>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigSources {
    pub committed: Option<PathBuf>,
    pub local: Option<PathBuf>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn reject_obsolete_keys(text: &str) -> Result<(), super::AdoptError> {
    let Ok(root) = text.parse::<toml::Table>() else {
        return Ok(());
    };
    let invalid = |detail: String| super::AdoptError::InvalidDefaults { detail };
    for key in ["dry_run", "dry-run"] {
        let in_dx = root
            .get("dx")
            .and_then(toml::Value::as_table)
            .is_some_and(|table| table.contains_key(key));
        if in_dx || root.contains_key(key) {
            return Err(invalid(REMOVED_DRY_RUN_DEFAULT.to_owned()));
        }
    }
    for key in DX_KEYS {
        if root.contains_key(key) {
            return Err(invalid(format!(
                "top-level {key} is not supported; set it under [dx]"
            )));
        }
    }
    if root.contains_key("fail_on") {
        return Err(invalid(
            "top-level fail_on is not supported; set fail-on under [dx]".to_owned(),
        ));
    }
    if root
        .get("dx")
        .and_then(toml::Value::as_table)
        .is_some_and(|table| table.contains_key("fail_on"))
    {
        return Err(invalid("fail_on is not supported; use fail-on".to_owned()));
    }
    Ok(())
}

pub fn parse_file_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    reject_obsolete_keys(text)?;
    let parsed: ConfigFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    check_consumer_schema(parsed.schema_version)?;
    let table = parsed.dx.unwrap_or_default();
    let baseline = check_quality_baseline(parsed.quality.map(|quality| quality.baseline))?;
    Ok(FileDefaults {
        workspace: non_empty(table.workspace),
        output: non_empty(table.output),
        verbose: table.verbose,
        color: non_empty(table.color),
        quiet: table.quiet,
        fail_on: non_empty(table.fail_on),
        build_slots: file_build_slots(table.build_slots)?,
        baseline,
    })
}

pub fn check_quality_baseline(
    baseline: Option<Option<String>>,
) -> Result<Option<String>, super::AdoptError> {
    let Some(rel) = baseline.flatten() else {
        return Ok(None);
    };
    if rel.is_empty() {
        return Err(super::AdoptError::InvalidDefaults {
            detail: "invalid quality baseline: want a non-empty workspace-relative JSON path"
                .to_owned(),
        });
    }
    if let Some(reason) = dx_path::reject_reason(&rel) {
        return Err(super::AdoptError::InvalidDefaults {
            detail: format!("invalid quality baseline {rel:?}: {reason}"),
        });
    }
    if !rel.ends_with(".json") {
        return Err(super::AdoptError::InvalidDefaults {
            detail: format!("invalid quality baseline {rel:?}: want a .json file"),
        });
    }
    Ok(Some(rel))
}

pub fn join_quality_baseline(workspace: &Path, rel: &str) -> PathBuf {
    workspace.join(rel)
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

pub fn merge_defaults(committed: FileDefaults, local: FileDefaults) -> FileDefaults {
    FileDefaults {
        workspace: local.workspace.or(committed.workspace),
        output: local.output.or(committed.output),
        verbose: local.verbose.or(committed.verbose),
        color: local.color.or(committed.color),
        quiet: local.quiet.or(committed.quiet),
        fail_on: local.fail_on.or(committed.fail_on),
        build_slots: local.build_slots.or(committed.build_slots),
        baseline: local.baseline.or(committed.baseline),
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
    ConfigSources { committed, local }
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
    Ok((FileDefaults::default(), None))
}

/// The `[dx] output` the config files name, read even when the rest of a file is invalid.
pub fn raw_output(start: &Path, ignore_local: bool) -> Option<String> {
    let sources = find_config_sources(start);
    let local = if ignore_local { None } else { sources.local };
    [local, sources.committed]
        .into_iter()
        .flatten()
        .find_map(|path| {
            let text = std::fs::read_to_string(path).ok()?;
            let root = text.parse::<toml::Table>().ok()?;
            root.get("dx")?
                .as_table()?
                .get("output")?
                .as_str()
                .map(str::to_owned)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_output_survives_an_invalid_key_and_prefers_the_local_file() {
        let scratch = dx_test_scratch::scratch("dx-defaults-raw-output-");
        let root = scratch.path();
        std::fs::write(root.join("MODULE.bazel"), "").expect("module");
        assert_eq!(raw_output(root, false), None);
        std::fs::write(
            root.join("dx.toml"),
            "[dx]\noutput = \"json\"\nlog-level = \"debug\"\n",
        )
        .expect("config");
        assert!(load_defaults_with_mode(root, false).is_err());
        assert_eq!(raw_output(root, false), Some("json".to_owned()));
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"text\"\n").expect("local");
        assert_eq!(raw_output(root, false), Some("text".to_owned()));
        assert_eq!(raw_output(root, true), Some("json".to_owned()));
        std::fs::write(root.join("dx.toml"), "[dx\noutput = \"json\"\n").expect("broken");
        assert_eq!(raw_output(root, true), None);
    }

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
        for truthy in ["true", "TRUE", " True ", " true"] {
            assert_eq!(parse_bool(truthy), Some(true), "{truthy:?} must enable");
        }
        for falsy in [
            "", "1", "yes", "y", "on", "0", "no", "off", "tru", "2", "maybe",
        ] {
            assert_ne!(parse_bool(falsy), Some(true), "{falsy:?} must not enable");
        }
    }

    #[test]
    fn falsy_spellings_disable_and_anything_else_is_unreadable() {
        for falsy in ["false", "FALSE", " False ", " false"] {
            assert_eq!(parse_bool(falsy), Some(false), "{falsy:?} must disable");
        }
        for on in TRUTHY {
            assert_eq!(parse_bool(on), Some(true), "{on:?} must enable");
        }
        for unreadable in [
            "1", "yes", "y", "on", "0", "no", "n", "off", "", " ", "tru", "ture", "fals", "2",
            "-1", "maybe", "enabled",
        ] {
            assert_eq!(parse_bool(unreadable), None, "{unreadable:?}");
        }
        assert_eq!(BOOL_SPELLINGS, "true|false");
    }

    #[test]
    fn env_bool_refuses_a_value_outside_the_documented_spellings() {
        let get = env_of(&[(DX_QUIET_ENV, "tru")]);
        let error = env_bool(&get, DX_QUIET_ENV).expect_err("typo is not a boolean");
        assert_eq!(
            error,
            super::super::AdoptError::InvalidEnvBool {
                name: DX_QUIET_ENV,
                value: "tru".to_owned(),
                spellings: BOOL_SPELLINGS,
            }
        );
        assert_eq!(
            error.to_string(),
            "invalid invocation default DX_QUIET=\"tru\": want one of true|false"
        );
        for (name, value, wanted) in [
            (DX_QUIET_ENV, "false", false),
            (DX_VERBOSE_ENV, "FALSE", false),
            (DX_VERBOSE_ENV, "true", true),
            (DX_QUIET_ENV, " True ", true),
        ] {
            let pairs = [(name, value)];
            let get = env_of(&pairs);
            assert_eq!(
                env_bool(&get, name).expect("documented spelling"),
                Some(wanted)
            );
        }
        let get = env_of(&[(DX_VERBOSE_ENV, ""), (DX_QUIET_ENV, "true")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(None));
        assert_eq!(env_bool(&get, DX_QUIET_ENV), Ok(Some(true)));
        assert_eq!(env_bool(&env_of(&[]), DX_VERBOSE_ENV), Ok(None));
    }

    #[test]
    fn env_defaults_cover_every_documented_variable() {
        assert_eq!(
            ENV_DEFAULTS.map(|(env, _, _)| env),
            [
                DX_WORKSPACE_ENV,
                DX_QUIET_ENV,
                DX_VERBOSE_ENV,
                DX_COLOR_ENV,
                DX_OUTPUT_ENV,
                DX_FAIL_ON_ENV,
                DX_BUILD_SLOTS_ENV
            ]
        );
        assert_eq!(TRUTHY, ["true"]);
        assert_eq!(FALSEY, ["false"]);
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
        let get = env_of(&[(DX_VERBOSE_ENV, "true")]);
        assert_eq!(env_bool(&get, DX_VERBOSE_ENV), Ok(Some(true)));
        let get = env_of(&[(DX_VERBOSE_ENV, "false")]);
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
    fn file_parses_dx_table() {
        let parsed = parse_file_text(
            "[dx]\nworkspace = \"/repo\"\noutput = \"json\"\nverbose = true\ncolor = \"never\"\nquiet = false\nfail-on = \"error\"\n",
        )
        .expect("dx table parses");
        assert_eq!(parsed.workspace, Some("/repo".to_owned()));
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert_eq!(parsed.verbose, Some(true));
        assert_eq!(parsed.color, Some("never".to_owned()));
        assert_eq!(parsed.quiet, Some(false));
        assert_eq!(parsed.fail_on, Some("error".to_owned()));
        for text in [
            "workspace = \"/top\"\n",
            "verbose = true\n",
            "output = \"text\"\n[dx]\noutput = \"json\"\n",
            "fail-on = \"error\"\n",
        ] {
            let error = parse_file_text(text).expect_err(text);
            assert!(error.to_string().contains("[dx]"), "{text:?}: {error}");
        }
        let empty = parse_file_text("").expect("empty parses");
        assert_eq!(empty, FileDefaults::default());
        assert!(parse_file_text("not toml = [").is_err());
        assert!(parse_file_text("[dx]\nverbose = \"yes\"\n").is_err());
    }

    #[test]
    fn file_hyphen_aliases_and_empty_strings_are_absent() {
        let parsed =
            parse_file_text("[dx]\n\"fail-on\" = \"info\"\n").expect("hyphen aliases parse");
        assert_eq!(parsed.fail_on, Some("info".to_owned()));
        let parsed = parse_file_text("[dx]\nworkspace = \"\"\noutput = \"\"\ncolor = \"\"\n")
            .expect("empty parses");
        assert_eq!(parsed.workspace, None);
        assert_eq!(parsed.output, None);
        assert_eq!(parsed.color, None);
    }

    #[test]
    fn removed_dry_run_defaults_are_refused_even_when_false() {
        for text in [
            "dry_run = false\n",
            "dry-run = true\n",
            "[dx]\ndry_run = false\n",
            "[dx]\n\"dry-run\" = true\n",
        ] {
            let error = parse_file_text(text).expect_err(text);
            assert!(error.to_string().contains("removed"), "{text:?}: {error}");
        }
        for value in ["true", "false", "1", "x"] {
            let pairs = [("DX_DRY_RUN", value)];
            let error = reject_removed_env(&env_of(&pairs)).expect_err(value);
            assert!(error.to_string().contains("DX_DRY_RUN"), "{error}");
        }
        assert_eq!(reject_removed_env(&env_of(&[("DX_DRY_RUN", "")])), Ok(()));
        assert_eq!(reject_removed_env(&env_of(&[])), Ok(()));
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
                "--quiet" | "--verbose" => "true",
                "--fail-on" => "\"error\"",
                "--color" => "\"never\"",
                "--output" => "\"json\"",
                "--build-slots" => "\"auto\"",
                _ => "\"/repo\"",
            };
            parse_file_text(&format!("[dx]\n{key} = {literal}\n"))
                .unwrap_or_else(|error| panic!("{key} must stay a real key: {error}"));
            let underscored = key.replace('-', "_");
            if underscored != key {
                let error = parse_file_text(&format!("[dx]\n{underscored} = {literal}\n"))
                    .expect_err("underscore spelling is refused");
                assert!(error.to_string().contains(key), "{error}");
            }
            let error =
                parse_file_text(&format!("{key} = {literal}\n")).expect_err("top level is refused");
            assert!(error.to_string().contains("[dx]"), "{error}");
        }
    }

    #[test]
    fn quality_baseline_selection_parses_and_validates() {
        let parsed = parse_file_text("[quality]\nbaseline = \"quality/baseline.json\"\n")
            .expect("baseline parses");
        assert_eq!(parsed.baseline.as_deref(), Some("quality/baseline.json"));
        let absent = parse_file_text("[dx]\noutput = \"json\"\n").expect("no baseline");
        assert_eq!(absent.baseline, None);
        for text in [
            "[quality]\nbaseline = \"\"\n",
            "[quality]\nbaseline = \"/abs.json\"\n",
            "[quality]\nbaseline = \"../escape.json\"\n",
            "[quality]\nbaseline = \"a\\\\b.json\"\n",
            "[quality]\nbaseline = \"quality/baseline.toml\"\n",
            "[quality]\nbaseline = 7\n",
            "[quality]\nunknown = true\n",
        ] {
            let error = parse_file_text(text).expect_err(text);
            assert!(
                error.to_string().contains("baseline") || error.to_string().contains("unknown"),
                "{text:?} must fail on the baseline selection: {error}"
            );
        }
        let committed = parse_file_text("[quality]\nbaseline = \"a.json\"\n").expect("committed");
        let local = parse_file_text("[quality]\nbaseline = \"b.json\"\n").expect("local");
        assert_eq!(
            merge_defaults(committed.clone(), local).baseline.as_deref(),
            Some("b.json")
        );
        assert_eq!(
            merge_defaults(committed, FileDefaults::default())
                .baseline
                .as_deref(),
            Some("a.json")
        );
        let root = std::path::Path::new("/ws");
        assert_eq!(
            join_quality_baseline(root, "quality/baseline.json"),
            root.join("quality/baseline.json")
        );
    }

    #[test]
    fn file_boolean_keys_keep_the_documented_precedence_order() {
        let both = parse_file_text("[dx]\nquiet = false\n").expect("false parses");
        assert_eq!(
            both.quiet,
            Some(false),
            "an explicit false is not an absent value"
        );
        let top_true = parse_file_text("[dx]\nverbose = true\n").expect("verbose true");
        assert_eq!(top_true.verbose, Some(true));
    }

    #[test]
    fn find_and_load_prefers_toml_and_walks_up() {
        let scratch = dx_test_scratch::scratch("dx-defaults-");
        let root = scratch.path().to_path_buf();
        let sub = root.join("sub/dir");
        std::fs::create_dir_all(sub.join(".dx")).expect("dirs");
        std::fs::create_dir_all(root.join(".dx")).expect("root dx");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("root config");
        std::fs::write(sub.join("dx.toml"), "[dx]\noutput = \"diff\"\n").expect("sub config");
        assert_eq!(
            find_new_configs(&sub).0,
            Some(sub.join("dx.toml")),
            "nearest file wins"
        );
        let (defaults, path) = load_defaults_with_mode(&sub, false).expect("loads nearest");
        assert_eq!(defaults.output, Some("diff".to_owned()));
        assert_eq!(path, Some(sub.join("dx.toml")));
        std::fs::remove_file(sub.join("dx.toml")).expect("remove sub");
        let (defaults, _) = load_defaults_with_mode(&sub, false).expect("falls back upward");
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
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("root config");
        std::fs::write(neighbor.join("dx.toml"), "[dx]\noutput = \"diff\"\n")
            .expect("neighbor config");
        std::fs::write(below.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("deeper config");
        assert_eq!(
            find_new_configs(&below).0,
            Some(below.join("dx.toml")),
            "the nearest ancestor supplies the defaults"
        );
        assert_eq!(
            find_new_configs(&mine).0,
            Some(root.join("dx.toml")),
            "a descendant never supplies the defaults"
        );
        let found = find_new_configs(&root.join("mine"))
            .0
            .expect("parent discovery is intentional");
        assert_eq!(found, root.join("dx.toml"));
        assert!(
            !found.starts_with(&neighbor),
            "a neighboring workspace never supplies the defaults: {found:?}"
        );
        let (defaults, path) =
            load_defaults_with_mode(&root.join("mine"), false).expect("loads the ancestor");
        assert_eq!(defaults.output, Some("json".to_owned()));
        assert_eq!(path, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn load_missing_is_empty_and_invalid_fails_closed() {
        let scratch = dx_test_scratch::scratch("dx-defaults-missing-");
        let root = scratch.path().to_path_buf();
        let (defaults, path) = load_defaults_with_mode(&root, false).expect("missing is empty");
        assert_eq!(defaults, FileDefaults::default());
        assert_eq!(path, None);
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join("dx.toml"), "not toml = [").expect("bad config");
        assert!(load_defaults_with_mode(&root, false).is_err());
        std::fs::write(root.join("dx.toml"), "[dx]\nqiet = true\n").expect("typo");
        let error =
            load_defaults_with_mode(&root, false).expect_err("a misspelled key is not ignored");
        let rendered = error.to_string();
        assert!(
            rendered.contains("unknown field") && rendered.contains("qiet"),
            "the diagnostic names the file and the key: {rendered}"
        );
        assert!(
            rendered.contains(&root.join("dx.toml").display().to_string()),
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
    fn new_files_ignore_sets_tables() {
        let text = "schema_version = 1\n[dx]\nquiet = true\n[[dependency_set]]\nname = \"a\"\necosystem = \"uv\"\nmanifests = [\"a/pyproject.toml\"]\nlocks = [\"a/uv.lock\"]\nscopes = [\"a\"]\n";
        let parsed = parse_file_text(text).expect("sets ignored");
        assert_eq!(parsed.quiet, Some(true));
    }

    #[test]
    fn new_files_reject_a_non_array_dependency_set() {
        let err = parse_file_text("schema_version = 1\ndependency_set = 3\n")
            .expect_err("dependency_set must be an array of tables")
            .to_string();
        assert!(err.contains("dependency_set"), "{err}");
    }

    #[test]
    fn new_files_reject_unknown_keys_and_bad_schema() {
        assert!(parse_file_text("[dx]\nqiet = true\n").is_err());
        assert!(parse_file_text("[extra]\nquiet = true\n").is_err());
        assert!(parse_file_text("[hooks]\npre_commit = []\n").is_err());
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
        let (defaults, primary) = load_defaults_with_mode(&root, false).expect("merges");
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
        let (defaults, primary) = load_defaults_with_mode(&root, false).expect("loads");
        assert_eq!(defaults.output, Some("json".to_owned()));
        assert_eq!(primary, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn obsolete_dx_config_files_are_never_read() {
        let scratch = dx_test_scratch::scratch("dx-defaults-obsolete-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        let bad = "not toml = [";
        std::fs::write(root.join(".dx/config.toml"), bad).expect("obsolete toml");
        std::fs::write(root.join(".dx/config"), bad).expect("obsolete plain");
        let (defaults, path) =
            load_defaults_with_mode(&root, false).expect("obsolete files are ignored");
        assert_eq!(defaults, FileDefaults::default());
        assert_eq!(path, None);
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"text\"\n").expect("new");
        let (defaults, path) = load_defaults_with_mode(&root, false).expect("new file loads");
        assert_eq!(defaults.output, Some("text".to_owned()));
        assert_eq!(path, Some(root.join("dx.toml")));
        for name in ["config.toml", "config"] {
            let bytes = std::fs::read_to_string(root.join(".dx").join(name)).expect("read");
            assert_eq!(bytes, bad);
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn deleting_dx_keeps_committed_behavior() {
        let scratch = dx_test_scratch::scratch("dx-defaults-nodelete-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        let (before, _) = load_defaults_with_mode(&root, false).expect("before");
        assert_eq!(before.output, Some("json".to_owned()));
        std::fs::remove_dir_all(root.join(".dx")).expect("remove dx");
        let (after, primary) = load_defaults_with_mode(&root, false).expect("after");
        assert_eq!(after, before);
        assert_eq!(primary, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn build_slots_parse_accepts_only_auto_off_and_counts() {
        let count = |n: usize| BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"));
        for (text, want) in [
            ("auto", Some(BuildSlots::Auto)),
            ("AUTO", Some(BuildSlots::Auto)),
            (" off ", Some(BuildSlots::Off)),
            ("2", Some(count(2))),
            ("1024", Some(count(1024))),
            ("0", None),
            ("-1", None),
            ("+2", None),
            ("many", None),
            ("2.5", None),
            ("1025", None),
            ("", None),
        ] {
            assert_eq!(BuildSlots::parse(text), want, "{text:?}");
        }
        for setting in [BuildSlots::Auto, BuildSlots::Off, count(7)] {
            assert_eq!(BuildSlots::parse(&setting.to_string()), Some(setting));
        }
    }

    #[test]
    fn build_slots_file_values_are_typed_and_checked() {
        let count = |n: usize| BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"));
        for (value, want) in [
            ("2", count(2)),
            ("\"2\"", count(2)),
            ("\"auto\"", BuildSlots::Auto),
            ("\"OFF\"", BuildSlots::Off),
        ] {
            let parsed = parse_file_text(&format!("[dx]\nbuild-slots = {value}\n")).expect(value);
            assert_eq!(parsed.build_slots, Some(want), "{value}");
        }
        let empty = parse_file_text("[dx]\nbuild-slots = \"\"\n").expect("empty");
        assert_eq!(empty.build_slots, None);
        for value in ["0", "-1", "1025", "2.5", "true", "\"many\""] {
            let error = parse_file_text(&format!("[dx]\nbuild-slots = {value}\n"))
                .expect_err(value)
                .to_string();
            assert!(
                error.contains("build-slots") && error.contains(BUILD_SLOTS_SPELLINGS),
                "{value}: {error}"
            );
        }
        let top = parse_file_text("build-slots = 2\n")
            .expect_err("top level")
            .to_string();
        assert!(top.contains("set it under [dx]"), "{top}");
    }

    #[test]
    fn build_slots_local_file_beats_committed_and_ci_drops_it() {
        let scratch = dx_test_scratch::scratch("dx-defaults-build-slots-");
        let root = scratch.path();
        std::fs::write(root.join("dx.toml"), "[dx]\nbuild-slots = 3\n").expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\nbuild-slots = \"off\"\n").expect("local");
        let (local, _) = load_defaults_with_mode(root, false).expect("local");
        assert_eq!(local.build_slots, Some(BuildSlots::Off));
        let (ci, _) = load_defaults_with_mode(root, true).expect("ci");
        assert_eq!(
            ci.build_slots,
            Some(BuildSlots::Count(
                std::num::NonZeroUsize::new(3).expect("3")
            ))
        );
        scratch.close().expect("cleanup");
    }
}
