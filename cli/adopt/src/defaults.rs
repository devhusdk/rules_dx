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

pub const KNOWN_OTHER_TABLES: [&str; 1] = ["hooks"];

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
    let loaded = load_consumer_config(start, false)?;
    Ok((loaded.defaults, loaded.legacy))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultOrigin {
    Flag,
    Env,
    Local,
    Committed,
    Legacy,
    #[default]
    Builtin,
}

impl DefaultOrigin {
    pub fn name(self) -> &'static str {
        match self {
            DefaultOrigin::Flag => "flag",
            DefaultOrigin::Env => "env",
            DefaultOrigin::Local => "local",
            DefaultOrigin::Committed => "committed",
            DefaultOrigin::Legacy => "legacy",
            DefaultOrigin::Builtin => "built-in",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileLayer {
    Local,
    Committed,
    Legacy,
}

impl FileLayer {
    pub fn origin(self) -> DefaultOrigin {
        match self {
            FileLayer::Local => DefaultOrigin::Local,
            FileLayer::Committed => DefaultOrigin::Committed,
            FileLayer::Legacy => DefaultOrigin::Legacy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileOrigins {
    pub workspace: Option<FileLayer>,
    pub output: Option<FileLayer>,
    pub verbose: Option<FileLayer>,
    pub color: Option<FileLayer>,
    pub quiet: Option<FileLayer>,
    pub dry_run: Option<FileLayer>,
    pub fail_on: Option<FileLayer>,
}

impl FileOrigins {
    pub fn get(&self, key: &str) -> Option<FileLayer> {
        match key {
            "workspace" => self.workspace,
            "output" => self.output,
            "verbose" => self.verbose,
            "color" => self.color,
            "quiet" => self.quiet,
            "dry-run" => self.dry_run,
            "fail-on" => self.fail_on,
            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, layer: FileLayer) {
        match key {
            "workspace" => self.workspace = Some(layer),
            "output" => self.output = Some(layer),
            "verbose" => self.verbose = Some(layer),
            "color" => self.color = Some(layer),
            "quiet" => self.quiet = Some(layer),
            "dry-run" => self.dry_run = Some(layer),
            "fail-on" => self.fail_on = Some(layer),
            _ => {}
        }
    }
}

pub const DEFAULT_KEYS: [&str; 7] = [
    "workspace",
    "output",
    "verbose",
    "color",
    "quiet",
    "dry-run",
    "fail-on",
];

impl FileDefaults {
    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            "workspace" => self.workspace.clone(),
            "output" => self.output.clone(),
            "verbose" => self.verbose.map(|value| value.to_string()),
            "color" => self.color.clone(),
            "quiet" => self.quiet.map(|value| value.to_string()),
            "dry-run" => self.dry_run.map(|value| value.to_string()),
            "fail-on" => self.fail_on.clone(),
            _ => None,
        }
    }

    pub fn is_set(&self, key: &str) -> bool {
        match key {
            "workspace" => self.workspace.is_some(),
            "output" => self.output.is_some(),
            "verbose" => self.verbose.is_some(),
            "color" => self.color.is_some(),
            "quiet" => self.quiet.is_some(),
            "dry-run" => self.dry_run.is_some(),
            "fail-on" => self.fail_on.is_some(),
            _ => false,
        }
    }

    fn set_merged(&mut self, key: &str, other: &FileDefaults, origins: &mut FileOrigins, layer: FileLayer) {
        if !self.is_set(key) {
            match key {
                "workspace" => self.workspace = other.workspace.clone(),
                "output" => self.output = other.output.clone(),
                "verbose" => self.verbose = other.verbose,
                "color" => self.color = other.color.clone(),
                "quiet" => self.quiet = other.quiet,
                "dry-run" => self.dry_run = other.dry_run,
                "fail-on" => self.fail_on = other.fail_on.clone(),
                _ => return,
            }
            if other.is_set(key) {
                origins.set(key, layer);
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefaultOrigins {
    pub workspace: DefaultOrigin,
    pub output: DefaultOrigin,
    pub verbose: DefaultOrigin,
    pub color: DefaultOrigin,
    pub quiet: DefaultOrigin,
    pub dry_run: DefaultOrigin,
    pub fail_on: DefaultOrigin,
}

impl DefaultOrigins {
    pub fn get(&self, key: &str) -> DefaultOrigin {
        match key {
            "workspace" => self.workspace,
            "output" => self.output,
            "verbose" => self.verbose,
            "color" => self.color,
            "quiet" => self.quiet,
            "dry-run" => self.dry_run,
            "fail-on" => self.fail_on,
            _ => DefaultOrigin::Builtin,
        }
    }

    pub fn set(&mut self, key: &str, origin: DefaultOrigin) {
        match key {
            "workspace" => self.workspace = origin,
            "output" => self.output = origin,
            "verbose" => self.verbose = origin,
            "color" => self.color = origin,
            "quiet" => self.quiet = origin,
            "dry-run" => self.dry_run = origin,
            "fail-on" => self.fail_on = origin,
            _ => {}
        }
    }
}

pub fn resolve_origin(flag_set: bool, env_set: bool, layer: Option<FileLayer>) -> DefaultOrigin {
    if flag_set {
        DefaultOrigin::Flag
    } else if env_set {
        DefaultOrigin::Env
    } else {
        layer.map(FileLayer::origin).unwrap_or(DefaultOrigin::Builtin)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyConflict {
    pub legacy: PathBuf,
    pub committed: Option<PathBuf>,
    pub local: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileConfig {
    pub defaults: FileDefaults,
    pub layers: FileOrigins,
    pub dir: PathBuf,
    pub committed: Option<PathBuf>,
    pub local: Option<PathBuf>,
    pub legacy: Option<PathBuf>,
    pub local_ignored_for_ci: bool,
    pub conflict: Option<LegacyConflict>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigSummary {
    pub config: FileConfig,
    pub origins: DefaultOrigins,
}

impl ConfigSummary {
    pub fn from_config(config: &FileConfig, origins: &DefaultOrigins) -> ConfigSummary {
        ConfigSummary {
            config: config.clone(),
            origins: origins.clone(),
        }
    }

    pub fn legacy_conflict(&self) -> bool {
        self.config.conflict.is_some()
    }

    pub fn legacy_only(&self) -> bool {
        self.config.legacy.is_some()
            && self.config.committed.is_none()
            && self.config.local.is_none()
    }
}

impl FileConfig {
    pub fn unattributed(defaults: &FileDefaults) -> FileConfig {
        let mut layers = FileOrigins::default();
        for key in DEFAULT_KEYS {
            if defaults.is_set(key) {
                layers.set(key, FileLayer::Legacy);
            }
        }
        FileConfig {
            defaults: defaults.clone(),
            layers,
            ..FileConfig::default()
        }
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumerFile {
    #[serde(default)]
    dx: Option<DxTable>,
    #[serde(default)]
    hooks: Option<toml::Value>,
}

pub fn parse_consumer_text(text: &str) -> Result<FileDefaults, super::AdoptError> {
    let parsed: ConsumerFile =
        toml::from_str(text).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: e.to_string(),
        })?;
    let ConsumerFile { dx, hooks } = parsed;
    let _ = hooks;
    let table = dx.unwrap_or_default();
    Ok(FileDefaults {
        workspace: non_empty(table.workspace),
        output: non_empty(table.output),
        verbose: table.verbose,
        color: non_empty(table.color),
        quiet: table.quiet,
        dry_run: table.dry_run,
        fail_on: non_empty(table.fail_on),
    })
}

fn read_consumer_file(path: &Path) -> Result<FileDefaults, super::AdoptError> {
    let text = std::fs::read_to_string(path).map_err(|e| super::AdoptError::InvalidDefaults {
        detail: format!("cannot read {}: {e}", path.display()),
    })?;
    parse_consumer_text(&text).map_err(|e| match e {
        super::AdoptError::InvalidDefaults { detail } => super::AdoptError::InvalidDefaults {
            detail: format!("{}: {detail}", path.display()),
        },
        other => other,
    })
}

pub fn find_consumer_files(start: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut committed = None;
    let mut local = None;
    for dir in start.ancestors() {
        if committed.is_none() {
            let candidate = dir.join(COMMITTED_CONFIG_REL);
            if candidate.is_file() {
                committed = Some(candidate);
            }
        }
        if local.is_none() {
            let candidate = dir.join(LOCAL_CONFIG_REL);
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

fn merge_layer(
    merged: &mut FileDefaults,
    origins: &mut FileOrigins,
    layer: &FileDefaults,
    file_layer: FileLayer,
) {
    for key in DEFAULT_KEYS {
        merged.set_merged(key, layer, origins, file_layer);
    }
}

pub fn load_consumer_config(
    start: &Path,
    ci: bool,
) -> Result<FileConfig, super::AdoptError> {
    let (committed, local) = find_consumer_files(start);
    let legacy = find_config(start);
    let mut config = FileConfig {
        dir: start.to_path_buf(),
        committed: committed.clone(),
        local: local.clone(),
        legacy,
        ..FileConfig::default()
    };
    if config.committed.is_some() || config.local.is_some() {
        if let Some(legacy_path) = config.legacy.clone() {
            config.conflict = Some(LegacyConflict {
                legacy: legacy_path,
                committed: committed.clone(),
                local: local.clone(),
            });
        }
        let mut merged = FileDefaults::default();
        let mut origins = FileOrigins::default();
        if let Some(path) = &committed {
            let layer = read_consumer_file(path)?;
            merge_layer(&mut merged, &mut origins, &layer, FileLayer::Committed);
        }
        if let Some(path) = &local {
            if ci {
                config.local_ignored_for_ci = true;
            } else {
                let layer = read_consumer_file(path)?;
                let mut local_merged = FileDefaults::default();
                let mut local_origins = FileOrigins::default();
                merge_layer(&mut local_merged, &mut local_origins, &layer, FileLayer::Local);
                for key in DEFAULT_KEYS {
                    if local_merged.is_set(key) {
                        match key {
                            "workspace" => merged.workspace = local_merged.workspace.clone(),
                            "output" => merged.output = local_merged.output.clone(),
                            "verbose" => merged.verbose = local_merged.verbose,
                            "color" => merged.color = local_merged.color.clone(),
                            "quiet" => merged.quiet = local_merged.quiet,
                            "dry-run" => merged.dry_run = local_merged.dry_run,
                            "fail-on" => merged.fail_on = local_merged.fail_on.clone(),
                            _ => {}
                        }
                        origins.set(key, FileLayer::Local);
                    }
                }
            }
        }
        config.defaults = merged;
        config.layers = origins;
        return Ok(config);
    }
    if let Some(path) = config.legacy.clone() {
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
        let mut origins = FileOrigins::default();
        for key in DEFAULT_KEYS {
            if defaults.is_set(key) {
                origins.set(key, FileLayer::Legacy);
            }
        }
        config.defaults = defaults;
        config.layers = origins;
    }
    Ok(config)
}

fn render_toml_value(key: &str, defaults: &FileDefaults) -> Option<String> {
    let rendered = match key {
        "workspace" => format!("{:?}", defaults.workspace.as_ref()?),
        "output" => format!("{:?}", defaults.output.as_ref()?),
        "verbose" => defaults.verbose?.to_string(),
        "color" => format!("{:?}", defaults.color.as_ref()?),
        "quiet" => defaults.quiet?.to_string(),
        "dry-run" => defaults.dry_run?.to_string(),
        "fail-on" => format!("{:?}", defaults.fail_on.as_ref()?),
        _ => return None,
    };
    Some(rendered)
}

pub fn render_committed_toml(defaults: &FileDefaults) -> String {
    let mut out = String::from("[dx]\n");
    for key in DEFAULT_KEYS {
        if let Some(value) = render_toml_value(key, defaults) {
            let stable = key.replace('-', "_");
            out.push_str(&format!("{stable} = {value}\n"));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigMigration {
    pub target: PathBuf,
    pub creates: bool,
    pub moving: Vec<(String, String)>,
    pub superseded: Vec<(String, String)>,
    pub removes: Vec<PathBuf>,
    pub body: String,
}

pub fn plan_config_migration(config: &FileConfig) -> Option<ConfigMigration> {
    let legacy_path = config.legacy.clone()?;
    let legacy_text = std::fs::read_to_string(&legacy_path).ok()?;
    let legacy = parse_file_text(&legacy_text).ok()?;
    let target = config
        .committed
        .clone()
        .unwrap_or_else(|| config.dir.join(COMMITTED_CONFIG_REL));
    let creates = !target.is_file();
    let mut merged = config.defaults.clone();
    let mut moving = Vec::new();
    let mut superseded = Vec::new();
    for key in DEFAULT_KEYS {
        if !legacy.is_set(key) {
            continue;
        }
        if merged.is_set(key) {
            superseded.push((
                key.to_owned(),
                legacy.get(key).unwrap_or_default(),
            ));
            continue;
        }
        moving.push((key.to_owned(), legacy.get(key).unwrap_or_default()));
        match key {
            "workspace" => merged.workspace = legacy.workspace.clone(),
            "output" => merged.output = legacy.output.clone(),
            "verbose" => merged.verbose = legacy.verbose,
            "color" => merged.color = legacy.color.clone(),
            "quiet" => merged.quiet = legacy.quiet,
            "dry-run" => merged.dry_run = legacy.dry_run,
            "fail-on" => merged.fail_on = legacy.fail_on.clone(),
            _ => {}
        }
    }
    let body = render_committed_toml(&merged);
    Some(ConfigMigration {
        target,
        creates,
        moving,
        superseded,
        removes: vec![legacy_path],
        body,
    })
}

pub fn apply_config_migration(
    migration: &ConfigMigration,
) -> Result<(), super::AdoptError> {
    if let Some(parent) = migration.target.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                super::AdoptError::InvalidDefaults {
                    detail: format!("cannot create {}: {e}", parent.display()),
                }
            })?;
        }
    }
    dx_atomic_fs::write_atomic(&migration.target, migration.body.as_bytes()).map_err(|e| {
        super::AdoptError::InvalidDefaults {
            detail: format!("cannot write {}: {e}", migration.target.display()),
        }
    })?;
    for legacy in &migration.removes {
        std::fs::remove_file(legacy).map_err(|e| super::AdoptError::InvalidDefaults {
            detail: format!("cannot remove {}: {e}", legacy.display()),
        })?;
    }
    Ok(())
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
    fn consumer_file_parses_dx_table_and_rejects_unknown_keys() {
        let parsed = parse_consumer_text("[dx]\noutput = \"json\"\nverbose = true\n")
            .expect("dx table parses");
        assert_eq!(parsed.output, Some("json".to_owned()));
        assert_eq!(parsed.verbose, Some(true));
        assert_eq!(parsed.quiet, None);
        let shared = parse_consumer_text("[dx]\noutput = \"json\"\n[hooks]\npre_commit = []\n")
            .expect("the hooks table belongs to another family");
        assert_eq!(shared.output, Some("json".to_owned()));
        for text in [
            "[dx]\nqiet = true\n",
            "[dx]\n[extra]\nquiet = true\n",
            "[dxx]\nquiet = true\n",
            "workspace = \"/repo\"\n",
            "[dx]\nworkspace = 42\n",
        ] {
            assert!(
                parse_consumer_text(text).is_err(),
                "{text:?} must fail closed"
            );
        }
        let error = parse_consumer_text("[dx]\nqiet = true\n").expect_err("typo");
        assert!(error.to_string().contains("qiet"), "{error}");
    }

    #[test]
    fn consumer_layers_merge_local_over_committed() {
        let scratch = dx_test_scratch::scratch("dx-defaults-layers-");
        let root = scratch.path().to_path_buf();
        std::fs::write(
            root.join("dx.toml"),
            "[dx]\noutput = \"json\"\nverbose = true\ncolor = \"never\"\n",
        )
        .expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"diff\"\n")
            .expect("local");
        let (committed, local) = find_consumer_files(&root);
        assert_eq!(committed, Some(root.join("dx.toml")));
        assert_eq!(local, Some(root.join("dx.local.toml")));
        let config = load_consumer_config(&root, false).expect("loads");
        assert_eq!(config.defaults.output, Some("diff".to_owned()));
        assert_eq!(config.defaults.verbose, Some(true));
        assert_eq!(config.defaults.color, Some("never".to_owned()));
        assert_eq!(config.layers.output, Some(FileLayer::Local));
        assert_eq!(config.layers.verbose, Some(FileLayer::Committed));
        assert_eq!(config.layers.quiet, None);
        assert!(config.conflict.is_none());
        assert!(!config.local_ignored_for_ci);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn consumer_local_is_ignored_in_ci_but_still_reported() {
        let scratch = dx_test_scratch::scratch("dx-defaults-ci-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"json\"\n").expect("committed");
        std::fs::write(root.join("dx.local.toml"), "[dx]\noutput = \"diff\"\n").expect("local");
        let config = load_consumer_config(&root, true).expect("loads in CI");
        assert_eq!(config.defaults.output, Some("json".to_owned()));
        assert_eq!(config.layers.output, Some(FileLayer::Committed));
        assert!(config.local_ignored_for_ci);
        assert_eq!(config.local, Some(root.join("dx.local.toml")));
        std::fs::write(root.join("dx.local.toml"), "not toml = [").expect("broken local");
        let config = load_consumer_config(&root, true).expect("broken local is unread in CI");
        assert_eq!(config.defaults.output, Some("json".to_owned()));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn legacy_is_a_fallback_and_conflicts_with_new_files() {
        let scratch = dx_test_scratch::scratch("dx-defaults-legacy-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config.toml"), "[dx]\noutput = \"json\"\n")
            .expect("legacy");
        let config = load_consumer_config(&root, false).expect("legacy loads");
        assert_eq!(config.defaults.output, Some("json".to_owned()));
        assert_eq!(config.layers.output, Some(FileLayer::Legacy));
        assert_eq!(config.legacy, Some(root.join(".dx/config.toml")));
        assert!(config.conflict.is_none());
        std::fs::write(root.join("dx.toml"), "[dx]\nverbose = true\n").expect("committed");
        let config = load_consumer_config(&root, false).expect("conflict loads");
        assert_eq!(config.defaults.verbose, Some(true));
        assert_eq!(config.defaults.output, None);
        let conflict = config.conflict.expect("conflict is recorded");
        assert_eq!(conflict.legacy, root.join(".dx/config.toml"));
        assert_eq!(conflict.committed, Some(root.join("dx.toml")));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn migration_moves_legacy_keys_and_removes_the_legacy_file() {
        let scratch = dx_test_scratch::scratch("dx-defaults-migrate-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(
            root.join(".dx/config.toml"),
            "[dx]\noutput = \"json\"\nverbose = true\n",
        )
        .expect("legacy");
        std::fs::write(root.join("dx.toml"), "[dx]\noutput = \"diff\"\n").expect("committed");
        let config = load_consumer_config(&root, false).expect("loads");
        let migration = plan_config_migration(&config).expect("plans");
        assert_eq!(migration.target, root.join("dx.toml"));
        assert!(!migration.creates);
        assert_eq!(
            migration.moving,
            vec![("verbose".to_owned(), "true".to_owned())]
        );
        assert_eq!(
            migration.superseded,
            vec![("output".to_owned(), "json".to_owned())]
        );
        assert_eq!(migration.removes, vec![root.join(".dx/config.toml")]);
        assert!(migration.body.contains("output = \"diff\""));
        assert!(migration.body.contains("verbose = true"));
        apply_config_migration(&migration).expect("applies");
        assert!(!root.join(".dx/config.toml").exists());
        let reread = load_consumer_config(&root, false).expect("reloads");
        assert!(reread.conflict.is_none());
        assert!(reread.legacy.is_none());
        assert_eq!(reread.defaults.output, Some("diff".to_owned()));
        assert_eq!(reread.defaults.verbose, Some(true));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn migration_creates_the_committed_file_when_absent() {
        let scratch = dx_test_scratch::scratch("dx-defaults-migrate-new-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/config"), "output = \"json\"\n").expect("legacy plain");
        let config = load_consumer_config(&root, false).expect("loads");
        assert_eq!(config.legacy, Some(root.join(".dx/config")));
        let migration = plan_config_migration(&config).expect("plans");
        assert!(migration.creates);
        assert_eq!(migration.target, root.join("dx.toml"));
        apply_config_migration(&migration).expect("applies");
        let body = std::fs::read_to_string(root.join("dx.toml")).expect("written");
        assert!(body.starts_with("[dx]\n"), "{body}");
        assert!(body.contains("output = \"json\""), "{body}");
        assert!(!root.join(".dx/config").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn no_legacy_means_no_migration() {
        let scratch = dx_test_scratch::scratch("dx-defaults-no-migrate-");
        let root = scratch.path().to_path_buf();
        let config = load_consumer_config(&root, false).expect("loads");
        assert!(plan_config_migration(&config).is_none());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn resolve_origin_ranks_flag_over_env_over_file() {
        use DefaultOrigin as Origin;
        assert_eq!(
            resolve_origin(true, true, Some(FileLayer::Local)),
            Origin::Flag
        );
        assert_eq!(
            resolve_origin(false, true, Some(FileLayer::Committed)),
            Origin::Env
        );
        assert_eq!(
            resolve_origin(false, false, Some(FileLayer::Local)),
            Origin::Local
        );
        assert_eq!(
            resolve_origin(false, false, Some(FileLayer::Committed)),
            Origin::Committed
        );
        assert_eq!(
            resolve_origin(false, false, Some(FileLayer::Legacy)),
            Origin::Legacy
        );
        assert_eq!(resolve_origin(false, false, None), Origin::Builtin);
        assert_eq!(Origin::Flag.name(), "flag");
        assert_eq!(Origin::Env.name(), "env");
        assert_eq!(Origin::Builtin.name(), "built-in");
    }
}
